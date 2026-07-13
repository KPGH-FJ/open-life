//! Authenticated wire codec for minimized MCP audit payload receipts.
//!
//! This module is deliberately storage-agnostic. It does not read SQLite
//! metadata, select a key epoch, migrate rows, or decide whether a database
//! column is authoritative. A successful decode returns the format version and
//! payload role that were authenticated by AES-GCM so the storage owner can
//! cross-check those facts against its independently loaded row metadata.

use std::fmt;

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use ring::digest::SHA256;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The only receipt format understood by this codec slice.
pub const MCP_AUDIT_PAYLOAD_FORMAT_V1: u32 = 1;

const ENVELOPE_MAGIC: &[u8; 8] = b"OLAUDIT\0";
const FORMAT_VERSION_BYTES: usize = std::mem::size_of::<u32>();
const ROLE_BYTES: usize = 1;
const ENVELOPE_HEADER_BYTES: usize = ENVELOPE_MAGIC.len() + FORMAT_VERSION_BYTES + ROLE_BYTES;
const AES_GCM_NONCE_BYTES: usize = 12;
const AES_GCM_TAG_BYTES: usize = 16;
const AAD_DOMAIN: &[u8] = b"openlife:mcp-audit:minimized-receipt";
/// A v1 receipt has five bounded scalar fields and serializes far below this
/// ceiling. The hard bound prevents an attacker-controlled database row from
/// causing an unbounded allocation before authentication.
const MAX_ENVELOPE_WIRE_BYTES: usize = 1_024;
const MAX_ENVELOPE_ENCODED_BYTES: usize = (MAX_ENVELOPE_WIRE_BYTES + 2) / 3 * 4;

/// The semantic position of one encrypted receipt in an MCP audit row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpAuditPayloadRole {
    Arguments,
    Result,
}

impl McpAuditPayloadRole {
    fn wire_tag(self) -> u8 {
        match self {
            Self::Arguments => 1,
            Self::Result => 2,
        }
    }

    fn from_wire_tag(tag: u8) -> Result<Self, McpAuditPayloadCodecError> {
        match tag {
            1 => Ok(Self::Arguments),
            2 => Ok(Self::Result),
            _ => Err(McpAuditPayloadCodecError::InvalidEnvelopeRole),
        }
    }
}

/// Exact JSON value categories admitted by a minimized receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpAuditValueType {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}

/// Strict v1 schema stored inside the authenticated payload envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MinimizedAuditReceiptV1 {
    kind: McpAuditPayloadRole,
    payload_stored: bool,
    value_type: McpAuditValueType,
    bytes: u64,
    digest: String,
}

impl MinimizedAuditReceiptV1 {
    /// Issue an arguments receipt from the exact serialized JSON bytes.
    pub fn for_arguments(arguments: &Value) -> Result<Self, McpAuditPayloadCodecError> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|_| McpAuditPayloadCodecError::ReceiptSerializationFailed)?;
        let value_type = match arguments {
            Value::Null => McpAuditValueType::Null,
            Value::Bool(_) => McpAuditValueType::Bool,
            Value::Number(_) => McpAuditValueType::Number,
            Value::String(_) => McpAuditValueType::String,
            Value::Array(_) => McpAuditValueType::Array,
            Value::Object(_) => McpAuditValueType::Object,
        };
        Self::issue(McpAuditPayloadRole::Arguments, value_type, &encoded)
    }

    /// Issue a result receipt from the exact UTF-8 result bytes.
    pub fn for_result(result: &str) -> Result<Self, McpAuditPayloadCodecError> {
        Self::issue(
            McpAuditPayloadRole::Result,
            McpAuditValueType::String,
            result.as_bytes(),
        )
    }

    fn issue(
        kind: McpAuditPayloadRole,
        value_type: McpAuditValueType,
        payload: &[u8],
    ) -> Result<Self, McpAuditPayloadCodecError> {
        let bytes = u64::try_from(payload.len())
            .map_err(|_| McpAuditPayloadCodecError::PayloadLengthOverflow)?;
        let digest = ring::digest::digest(&SHA256, payload);
        Ok(Self {
            kind,
            payload_stored: false,
            value_type,
            bytes,
            digest: format!(
                "sha256:{}",
                general_purpose::STANDARD_NO_PAD.encode(digest.as_ref())
            ),
        })
    }

    fn decode_strict(
        plaintext: &[u8],
        authenticated_role: McpAuditPayloadRole,
    ) -> Result<Self, McpAuditPayloadCodecError> {
        let receipt: Self = serde_json::from_slice(plaintext)
            .map_err(|_| McpAuditPayloadCodecError::InvalidReceiptSchema)?;
        receipt.validate(authenticated_role)?;
        Ok(receipt)
    }

    fn validate(
        &self,
        authenticated_role: McpAuditPayloadRole,
    ) -> Result<(), McpAuditPayloadCodecError> {
        if self.kind != authenticated_role {
            return Err(McpAuditPayloadCodecError::ReceiptRoleMismatch {
                authenticated: authenticated_role,
                receipt: self.kind,
            });
        }
        if self.payload_stored {
            return Err(McpAuditPayloadCodecError::PayloadStoredMustBeFalse);
        }
        if self.kind == McpAuditPayloadRole::Result && self.value_type != McpAuditValueType::String
        {
            return Err(McpAuditPayloadCodecError::ResultValueTypeMustBeString);
        }
        if !is_canonical_sha256_digest(&self.digest) {
            return Err(McpAuditPayloadCodecError::InvalidReceiptDigest);
        }
        Ok(())
    }

    pub fn kind(&self) -> McpAuditPayloadRole {
        self.kind
    }

    pub fn payload_stored(&self) -> bool {
        self.payload_stored
    }

    pub fn value_type(&self) -> McpAuditValueType {
        self.value_type
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn to_json(&self) -> Result<String, McpAuditPayloadCodecError> {
        serde_json::to_string(self)
            .map_err(|_| McpAuditPayloadCodecError::ReceiptSerializationFailed)
    }
}

/// Authenticated facts returned by the codec. Neither field is sourced from a
/// database column or caller hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedMinimizedAuditPayload {
    format_version: u32,
    role: McpAuditPayloadRole,
    receipt: MinimizedAuditReceiptV1,
}

impl AuthenticatedMinimizedAuditPayload {
    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    pub fn role(&self) -> McpAuditPayloadRole {
        self.role
    }

    pub fn receipt(&self) -> &MinimizedAuditReceiptV1 {
        &self.receipt
    }

    pub fn into_receipt(self) -> MinimizedAuditReceiptV1 {
        self.receipt
    }
}

/// Sanitized codec failures. No variant includes plaintext, ciphertext, key
/// material, or the supplied digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpAuditPayloadCodecError {
    InvalidEnvelopeEncoding,
    InvalidEnvelopeStructure,
    EnvelopeTooLarge,
    InvalidEnvelopeRole,
    AuthenticationFailed,
    UnsupportedFormatVersion(u32),
    InvalidReceiptSchema,
    ReceiptRoleMismatch {
        authenticated: McpAuditPayloadRole,
        receipt: McpAuditPayloadRole,
    },
    PayloadStoredMustBeFalse,
    ResultValueTypeMustBeString,
    InvalidReceiptDigest,
    PayloadLengthOverflow,
    ReceiptSerializationFailed,
    EncryptionFailed,
}

impl fmt::Display for McpAuditPayloadCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidEnvelopeEncoding => "invalid MCP audit payload envelope encoding",
            Self::InvalidEnvelopeStructure => "invalid MCP audit payload envelope structure",
            Self::EnvelopeTooLarge => "MCP audit payload envelope exceeds the v1 size limit",
            Self::InvalidEnvelopeRole => "invalid MCP audit payload envelope role",
            Self::AuthenticationFailed => "MCP audit payload authentication failed",
            Self::UnsupportedFormatVersion(_) => {
                "unsupported authenticated MCP audit payload format version"
            }
            Self::InvalidReceiptSchema => "invalid minimized MCP audit receipt schema",
            Self::ReceiptRoleMismatch { .. } => {
                "minimized MCP audit receipt role does not match authenticated role"
            }
            Self::PayloadStoredMustBeFalse => {
                "minimized MCP audit receipt must not store payload content"
            }
            Self::ResultValueTypeMustBeString => {
                "minimized MCP audit result receipt value type must be string"
            }
            Self::InvalidReceiptDigest => "invalid minimized MCP audit receipt digest",
            Self::PayloadLengthOverflow => "MCP audit payload length exceeds receipt capacity",
            Self::ReceiptSerializationFailed => "minimized MCP audit receipt serialization failed",
            Self::EncryptionFailed => "MCP audit payload encryption failed",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for McpAuditPayloadCodecError {}

/// Encrypt a validated v1 receipt. The wire header is authenticated as AEAD
/// associated data and therefore cannot be changed independently of the
/// ciphertext.
pub fn seal_minimized_audit_receipt_v1(
    key: &[u8; 32],
    receipt: &MinimizedAuditReceiptV1,
) -> Result<String, McpAuditPayloadCodecError> {
    receipt.validate(receipt.kind)?;
    let plaintext = serde_json::to_vec(receipt)
        .map_err(|_| McpAuditPayloadCodecError::ReceiptSerializationFailed)?;
    seal_plaintext(key, MCP_AUDIT_PAYLOAD_FORMAT_V1, receipt.kind, &plaintext)
}

/// Decode one envelope without accepting any database-provided version or role
/// hint. The returned metadata came from the authenticated header and is the
/// only codec fact a storage owner may cross-check against its row columns.
pub fn open_authenticated_audit_payload(
    key: &[u8; 32],
    encoded_envelope: &str,
) -> Result<AuthenticatedMinimizedAuditPayload, McpAuditPayloadCodecError> {
    if encoded_envelope.len() > MAX_ENVELOPE_ENCODED_BYTES {
        return Err(McpAuditPayloadCodecError::EnvelopeTooLarge);
    }
    let envelope = decode_canonical_base64(encoded_envelope)?;
    if envelope.len() > MAX_ENVELOPE_WIRE_BYTES {
        return Err(McpAuditPayloadCodecError::EnvelopeTooLarge);
    }
    if envelope.len() < ENVELOPE_HEADER_BYTES + AES_GCM_NONCE_BYTES + AES_GCM_TAG_BYTES
        || &envelope[..ENVELOPE_MAGIC.len()] != ENVELOPE_MAGIC
    {
        return Err(McpAuditPayloadCodecError::InvalidEnvelopeStructure);
    }

    let header = &envelope[..ENVELOPE_HEADER_BYTES];
    let nonce_start = ENVELOPE_HEADER_BYTES;
    let ciphertext_start = nonce_start + AES_GCM_NONCE_BYTES;
    let nonce = Nonce::from_slice(&envelope[nonce_start..ciphertext_start]);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| McpAuditPayloadCodecError::AuthenticationFailed)?;
    let aad = authenticated_aad(header);
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: &envelope[ciphertext_start..],
                aad: &aad,
            },
        )
        .map_err(|_| McpAuditPayloadCodecError::AuthenticationFailed)?;

    let version_start = ENVELOPE_MAGIC.len();
    let version_end = version_start + FORMAT_VERSION_BYTES;
    let format_version = u32::from_be_bytes(
        header[version_start..version_end]
            .try_into()
            .map_err(|_| McpAuditPayloadCodecError::InvalidEnvelopeStructure)?,
    );
    let role = McpAuditPayloadRole::from_wire_tag(header[version_end])?;
    if format_version != MCP_AUDIT_PAYLOAD_FORMAT_V1 {
        return Err(McpAuditPayloadCodecError::UnsupportedFormatVersion(
            format_version,
        ));
    }
    let receipt = MinimizedAuditReceiptV1::decode_strict(&plaintext, role)?;

    Ok(AuthenticatedMinimizedAuditPayload {
        format_version,
        role,
        receipt,
    })
}

fn seal_plaintext(
    key: &[u8; 32],
    format_version: u32,
    role: McpAuditPayloadRole,
    plaintext: &[u8],
) -> Result<String, McpAuditPayloadCodecError> {
    let header = envelope_header(format_version, role);
    let nonce_bytes = rand::random::<[u8; AES_GCM_NONCE_BYTES]>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| McpAuditPayloadCodecError::EncryptionFailed)?;
    let aad = authenticated_aad(&header);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| McpAuditPayloadCodecError::EncryptionFailed)?;

    let mut envelope = Vec::with_capacity(header.len() + nonce_bytes.len() + ciphertext.len());
    envelope.extend_from_slice(&header);
    envelope.extend_from_slice(&nonce_bytes);
    envelope.extend_from_slice(&ciphertext);
    if envelope.len() > MAX_ENVELOPE_WIRE_BYTES {
        return Err(McpAuditPayloadCodecError::EnvelopeTooLarge);
    }
    Ok(general_purpose::STANDARD.encode(envelope))
}

fn envelope_header(format_version: u32, role: McpAuditPayloadRole) -> [u8; ENVELOPE_HEADER_BYTES] {
    let mut header = [0_u8; ENVELOPE_HEADER_BYTES];
    header[..ENVELOPE_MAGIC.len()].copy_from_slice(ENVELOPE_MAGIC);
    let version_start = ENVELOPE_MAGIC.len();
    let version_end = version_start + FORMAT_VERSION_BYTES;
    header[version_start..version_end].copy_from_slice(&format_version.to_be_bytes());
    header[version_end] = role.wire_tag();
    header
}

fn authenticated_aad(header: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(AAD_DOMAIN.len() + header.len());
    aad.extend_from_slice(AAD_DOMAIN);
    aad.extend_from_slice(header);
    aad
}

fn decode_canonical_base64(encoded: &str) -> Result<Vec<u8>, McpAuditPayloadCodecError> {
    let decoded = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| McpAuditPayloadCodecError::InvalidEnvelopeEncoding)?;
    if general_purpose::STANDARD.encode(&decoded) != encoded {
        return Err(McpAuditPayloadCodecError::InvalidEnvelopeEncoding);
    }
    Ok(decoded)
}

fn is_canonical_sha256_digest(value: &str) -> bool {
    let Some(encoded) = value.strip_prefix("sha256:") else {
        return false;
    };
    if encoded.len() != 43 {
        return false;
    }
    let Ok(decoded) = general_purpose::STANDARD_NO_PAD.decode(encoded) else {
        return false;
    };
    decoded.len() == 32 && general_purpose::STANDARD_NO_PAD.encode(decoded) == encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const KEY: [u8; 32] = [0x68; 32];

    fn seal_authenticated_fixture(
        format_version: u32,
        role: McpAuditPayloadRole,
        value: Value,
    ) -> String {
        seal_plaintext(&KEY, format_version, role, value.to_string().as_bytes()).unwrap()
    }

    fn valid_receipt_value(role: McpAuditPayloadRole) -> Value {
        let receipt = match role {
            McpAuditPayloadRole::Arguments => {
                MinimizedAuditReceiptV1::for_arguments(&json!({"bounded": true})).unwrap()
            }
            McpAuditPayloadRole::Result => {
                MinimizedAuditReceiptV1::for_result("bounded-result").unwrap()
            }
        };
        serde_json::to_value(receipt).unwrap()
    }

    #[test]
    fn d068_codec_round_trips_authenticated_version_role_and_exact_receipt() {
        let argument_value = json!({"bounded": true, "count": 7});
        let argument_bytes = serde_json::to_vec(&argument_value).unwrap();
        let arguments = MinimizedAuditReceiptV1::for_arguments(&argument_value).unwrap();
        let result_text = "bounded-result";
        let result = MinimizedAuditReceiptV1::for_result(result_text).unwrap();

        for receipt in [&arguments, &result] {
            let envelope = seal_minimized_audit_receipt_v1(&KEY, receipt).unwrap();
            let authenticated = open_authenticated_audit_payload(&KEY, &envelope).unwrap();
            assert_eq!(authenticated.format_version(), MCP_AUDIT_PAYLOAD_FORMAT_V1);
            assert_eq!(authenticated.role(), receipt.kind());
            assert_eq!(authenticated.receipt(), receipt);
            assert!(!authenticated.receipt().payload_stored());
        }

        assert_eq!(arguments.value_type(), McpAuditValueType::Object);
        assert_eq!(arguments.bytes(), argument_bytes.len() as u64);
        assert_eq!(
            arguments.digest(),
            format!(
                "sha256:{}",
                general_purpose::STANDARD_NO_PAD
                    .encode(ring::digest::digest(&SHA256, &argument_bytes).as_ref())
            )
        );
        assert_eq!(result.value_type(), McpAuditValueType::String);
        assert_eq!(result.bytes(), result_text.len() as u64);
        assert_eq!(
            result.digest(),
            format!(
                "sha256:{}",
                general_purpose::STANDARD_NO_PAD
                    .encode(ring::digest::digest(&SHA256, result_text.as_bytes()).as_ref())
            )
        );
    }

    #[test]
    fn d068_codec_header_version_and_role_are_aead_authenticated() {
        let receipt = MinimizedAuditReceiptV1::for_arguments(&json!({"bounded": true})).unwrap();
        let envelope = seal_minimized_audit_receipt_v1(&KEY, &receipt).unwrap();
        let original = general_purpose::STANDARD.decode(&envelope).unwrap();

        let version_start = ENVELOPE_MAGIC.len();
        let version_end = version_start + FORMAT_VERSION_BYTES;
        let mut version_tampered = original.clone();
        version_tampered[version_start..version_end].copy_from_slice(&2_u32.to_be_bytes());
        let version_tampered = general_purpose::STANDARD.encode(version_tampered);
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &version_tampered),
            Err(McpAuditPayloadCodecError::AuthenticationFailed)
        );

        let mut role_tampered = original;
        role_tampered[version_end] = McpAuditPayloadRole::Result.wire_tag();
        let role_tampered = general_purpose::STANDARD.encode(role_tampered);
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &role_tampered),
            Err(McpAuditPayloadCodecError::AuthenticationFailed)
        );

        let unsupported = seal_authenticated_fixture(
            2,
            McpAuditPayloadRole::Arguments,
            valid_receipt_value(McpAuditPayloadRole::Arguments),
        );
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &unsupported),
            Err(McpAuditPayloadCodecError::UnsupportedFormatVersion(2))
        );
    }

    #[test]
    fn d068_codec_strict_receipt_schema_rejects_all_field_shape_failures() {
        let mut invalid = Vec::new();
        for role in [McpAuditPayloadRole::Arguments, McpAuditPayloadRole::Result] {
            let valid = valid_receipt_value(role);
            for field in ["kind", "payloadStored", "valueType", "bytes", "digest"] {
                let mut candidate = valid.clone();
                candidate.as_object_mut().unwrap().remove(field);
                invalid.push((role, candidate));
            }

            for (field, value) in [
                ("kind", json!(false)),
                ("payloadStored", json!("false")),
                ("valueType", json!(false)),
                ("bytes", json!("1")),
                ("digest", json!(false)),
            ] {
                let mut candidate = valid.clone();
                candidate
                    .as_object_mut()
                    .unwrap()
                    .insert(field.into(), value);
                invalid.push((role, candidate));
            }

            let mut unknown = valid.clone();
            unknown
                .as_object_mut()
                .unwrap()
                .insert("raw".into(), json!("forbidden"));
            invalid.push((role, unknown));

            for value in [json!(-1), json!(1.5), json!(1e100)] {
                let mut candidate = valid.clone();
                candidate
                    .as_object_mut()
                    .unwrap()
                    .insert("bytes".into(), value);
                invalid.push((role, candidate));
            }

            let mut invalid_type = valid.clone();
            invalid_type
                .as_object_mut()
                .unwrap()
                .insert("valueType".into(), json!("secret_object"));
            invalid.push((role, invalid_type));
        }

        for (role, candidate) in invalid {
            let envelope = seal_authenticated_fixture(MCP_AUDIT_PAYLOAD_FORMAT_V1, role, candidate);
            assert_eq!(
                open_authenticated_audit_payload(&KEY, &envelope),
                Err(McpAuditPayloadCodecError::InvalidReceiptSchema)
            );
        }
    }

    #[test]
    fn d068_codec_enforces_receipt_role_payload_stored_value_type_and_digest() {
        let cases = [
            (
                McpAuditPayloadRole::Arguments,
                json!({
                    "kind": "result",
                    "payloadStored": false,
                    "valueType": "string",
                    "bytes": 1,
                    "digest": "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                }),
                McpAuditPayloadCodecError::ReceiptRoleMismatch {
                    authenticated: McpAuditPayloadRole::Arguments,
                    receipt: McpAuditPayloadRole::Result,
                },
            ),
            (
                McpAuditPayloadRole::Arguments,
                json!({
                    "kind": "arguments",
                    "payloadStored": true,
                    "valueType": "object",
                    "bytes": 1,
                    "digest": "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                }),
                McpAuditPayloadCodecError::PayloadStoredMustBeFalse,
            ),
            (
                McpAuditPayloadRole::Result,
                json!({
                    "kind": "result",
                    "payloadStored": false,
                    "valueType": "object",
                    "bytes": 1,
                    "digest": "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                }),
                McpAuditPayloadCodecError::ResultValueTypeMustBeString,
            ),
            (
                McpAuditPayloadRole::Result,
                json!({
                    "kind": "result",
                    "payloadStored": false,
                    "valueType": "string",
                    "bytes": 1,
                    "digest": "sha256:not-a-sha256-digest"
                }),
                McpAuditPayloadCodecError::InvalidReceiptDigest,
            ),
            (
                McpAuditPayloadRole::Result,
                json!({
                    "kind": "result",
                    "payloadStored": false,
                    "valueType": "string",
                    "bytes": 1,
                    "digest": "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                }),
                McpAuditPayloadCodecError::InvalidReceiptDigest,
            ),
        ];

        for (role, value, expected) in cases {
            let envelope = seal_authenticated_fixture(MCP_AUDIT_PAYLOAD_FORMAT_V1, role, value);
            assert_eq!(
                open_authenticated_audit_payload(&KEY, &envelope),
                Err(expected)
            );
        }
    }

    #[test]
    fn d068_codec_rejects_ciphertext_corruption_wrong_keys_and_noncanonical_wire() {
        let envelope = [1_usize, 10, 100]
            .into_iter()
            .find_map(|result_bytes| {
                let result = "x".repeat(result_bytes);
                let receipt = MinimizedAuditReceiptV1::for_result(&result).unwrap();
                let candidate = seal_minimized_audit_receipt_v1(&KEY, &receipt).unwrap();
                candidate.ends_with('=').then_some(candidate)
            })
            .expect("one of three consecutive receipt sizes requires base64 padding");
        let mut corrupted = general_purpose::STANDARD.decode(&envelope).unwrap();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        let corrupted = general_purpose::STANDARD.encode(corrupted);
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &corrupted),
            Err(McpAuditPayloadCodecError::AuthenticationFailed)
        );
        assert_eq!(
            open_authenticated_audit_payload(&[0x69; 32], &envelope),
            Err(McpAuditPayloadCodecError::AuthenticationFailed)
        );

        let unpadded = envelope.trim_end_matches('=');
        assert_eq!(
            open_authenticated_audit_payload(&KEY, unpadded),
            Err(McpAuditPayloadCodecError::InvalidEnvelopeEncoding)
        );
        assert_eq!(
            open_authenticated_audit_payload(&KEY, "not base64"),
            Err(McpAuditPayloadCodecError::InvalidEnvelopeEncoding)
        );
    }

    #[test]
    fn d068_codec_rejects_oversized_envelopes_before_or_after_base64_decode() {
        let oversized_encoded = "A".repeat(MAX_ENVELOPE_ENCODED_BYTES + 1);
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &oversized_encoded),
            Err(McpAuditPayloadCodecError::EnvelopeTooLarge)
        );

        // 1,025 zero bytes have a canonical padded Base64 representation with
        // the same encoded length as the largest admitted 1,024-byte wire.
        // This proves the decoded-wire check is independent of the cheap
        // pre-decode bound.
        let oversized_wire = vec![0_u8; MAX_ENVELOPE_WIRE_BYTES + 1];
        let canonical_oversized = general_purpose::STANDARD.encode(&oversized_wire);
        assert_eq!(canonical_oversized.len(), MAX_ENVELOPE_ENCODED_BYTES);
        assert_eq!(
            general_purpose::STANDARD
                .decode(&canonical_oversized)
                .unwrap(),
            oversized_wire
        );
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &canonical_oversized),
            Err(McpAuditPayloadCodecError::EnvelopeTooLarge)
        );
    }
}
