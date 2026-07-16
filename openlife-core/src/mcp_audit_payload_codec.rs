//! Authenticated wire codec for minimized MCP audit payload receipts.
//!
//! This module is deliberately storage-agnostic. It does not read SQLite
//! metadata, select a key epoch, migrate rows, or decide whether a database
//! column is authoritative. The storage owner must construct an expected
//! binding from its canonical store identity and exact row facts; that binding
//! participates directly in AES-GCM authentication, so a mismatch fails before
//! any receipt can become product truth.

use std::{fmt, io::Write};

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use ring::digest::{Context as DigestContext, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::{Uuid, Variant, Version};

/// The only receipt format understood by this codec slice.
pub(crate) const MCP_AUDIT_PAYLOAD_FORMAT_V1: u32 = 1;

const ENVELOPE_MAGIC: &[u8; 8] = b"OLAUDIT\0";
const FORMAT_VERSION_BYTES: usize = std::mem::size_of::<u32>();
const ROLE_BYTES: usize = 1;
const ENVELOPE_HEADER_BYTES: usize = ENVELOPE_MAGIC.len() + FORMAT_VERSION_BYTES + ROLE_BYTES;
const AES_GCM_NONCE_BYTES: usize = 12;
const AES_GCM_TAG_BYTES: usize = 16;
const AAD_DOMAIN: &[u8] = b"openlife:mcp-audit:minimized-receipt";
const AAD_DOMAIN_TAG: u8 = 1;
const AAD_HEADER_TAG: u8 = 2;
const AAD_STORE_IDENTITY_TAG: u8 = 3;
const AAD_RECORD_ID_TAG: u8 = 4;
const AAD_KEY_EPOCH_TAG: u8 = 5;
const AAD_ROW_CONTEXT_TAG: u8 = 6;
const AAD_EXPECTED_ROLE_TAG: u8 = 7;
/// A v1 receipt has five bounded scalar fields and serializes far below this
/// ceiling. The hard bound prevents an attacker-controlled database row from
/// causing an unbounded allocation before authentication.
const MAX_ENVELOPE_WIRE_BYTES: usize = 1_024;
/// Storage owners must apply this bound before materializing SQLite TEXT as a
/// Rust `String`. The codec repeats the check so callers outside SQLite cannot
/// bypass the allocation boundary.
pub(crate) const MCP_AUDIT_MAX_ENVELOPE_ENCODED_BYTES: usize =
    (MAX_ENVELOPE_WIRE_BYTES + 2) / 3 * 4;

/// The semantic position of one encrypted receipt in an MCP audit row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum McpAuditPayloadRole {
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
pub(crate) enum McpAuditValueType {
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
pub(crate) struct MinimizedAuditReceiptV1 {
    kind: McpAuditPayloadRole,
    payload_stored: bool,
    value_type: McpAuditValueType,
    bytes: u64,
    digest: String,
}

impl MinimizedAuditReceiptV1 {
    /// Issue an arguments receipt from the exact serialized JSON bytes.
    pub(crate) fn for_arguments(arguments: &Value) -> Result<Self, McpAuditPayloadCodecError> {
        let value_type = match arguments {
            Value::Null => McpAuditValueType::Null,
            Value::Bool(_) => McpAuditValueType::Bool,
            Value::Number(_) => McpAuditValueType::Number,
            Value::String(_) => McpAuditValueType::String,
            Value::Array(_) => McpAuditValueType::Array,
            Value::Object(_) => McpAuditValueType::Object,
        };
        let mut digest_writer = ReceiptDigestWriter::new();
        let serialization = serde_json::to_writer(&mut digest_writer, arguments);
        if digest_writer.length_overflowed() {
            return Err(McpAuditPayloadCodecError::PayloadLengthOverflow);
        }
        serialization.map_err(|_| McpAuditPayloadCodecError::ReceiptSerializationFailed)?;
        let (bytes, digest) = digest_writer.finish();
        Ok(Self::issue_from_digest(
            McpAuditPayloadRole::Arguments,
            value_type,
            bytes,
            digest.as_ref(),
        ))
    }

    /// Issue a result receipt from the exact UTF-8 result bytes.
    pub(crate) fn for_result(result: &str) -> Result<Self, McpAuditPayloadCodecError> {
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
        Ok(Self::issue_from_digest(
            kind,
            value_type,
            bytes,
            digest.as_ref(),
        ))
    }

    fn issue_from_digest(
        kind: McpAuditPayloadRole,
        value_type: McpAuditValueType,
        bytes: u64,
        digest: &[u8],
    ) -> Self {
        Self {
            kind,
            payload_stored: false,
            value_type,
            bytes,
            digest: format!("sha256:{}", general_purpose::STANDARD_NO_PAD.encode(digest)),
        }
    }

    pub(crate) fn decode_strict(
        plaintext: &[u8],
        authenticated_role: McpAuditPayloadRole,
    ) -> Result<Self, McpAuditPayloadCodecError> {
        let receipt: Self = serde_json::from_slice(plaintext)
            .map_err(|_| McpAuditPayloadCodecError::InvalidReceiptSchema)?;
        receipt.validate(authenticated_role)?;
        Ok(receipt)
    }

    pub(crate) fn to_json_string(&self) -> Result<String, McpAuditPayloadCodecError> {
        serde_json::to_string(self)
            .map_err(|_| McpAuditPayloadCodecError::ReceiptSerializationFailed)
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

    #[cfg(test)]
    pub(crate) fn kind(&self) -> McpAuditPayloadRole {
        self.kind
    }

    #[cfg(test)]
    pub(crate) fn payload_stored(&self) -> bool {
        self.payload_stored
    }

    #[cfg(test)]
    pub(crate) fn value_type(&self) -> McpAuditValueType {
        self.value_type
    }

    #[cfg(test)]
    pub(crate) fn bytes(&self) -> u64 {
        self.bytes
    }

    #[cfg(test)]
    pub(crate) fn digest(&self) -> &str {
        &self.digest
    }
}

struct ReceiptDigestWriter {
    digest: DigestContext,
    bytes: u64,
    length_overflowed: bool,
}

impl ReceiptDigestWriter {
    fn new() -> Self {
        Self {
            digest: DigestContext::new(&SHA256),
            bytes: 0,
            length_overflowed: false,
        }
    }

    fn length_overflowed(&self) -> bool {
        self.length_overflowed
    }

    fn finish(self) -> (u64, ring::digest::Digest) {
        (self.bytes, self.digest.finish())
    }
}

impl Write for ReceiptDigestWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let increment = match u64::try_from(buffer.len()) {
            Ok(increment) => increment,
            Err(_) => {
                self.length_overflowed = true;
                return Err(std::io::Error::other("MCP audit receipt length overflow"));
            }
        };
        let Some(bytes) = self.bytes.checked_add(increment) else {
            self.length_overflowed = true;
            return Err(std::io::Error::other("MCP audit receipt length overflow"));
        };
        self.digest.update(buffer);
        self.bytes = bytes;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Immutable authenticated context for one payload position in one audit row.
/// The storage owner will supply the canonical store identity and row-context
/// digests once D064/D068 storage integration is performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct McpAuditPayloadBindingV1 {
    store_identity_digest: [u8; 32],
    audit_record_id: Uuid,
    key_epoch: u64,
    row_context_digest: [u8; 32],
    role: McpAuditPayloadRole,
}

impl McpAuditPayloadBindingV1 {
    pub(crate) fn new(
        store_identity_digest: [u8; 32],
        audit_record_id: Uuid,
        key_epoch: u64,
        row_context_digest: [u8; 32],
        role: McpAuditPayloadRole,
    ) -> Result<Self, McpAuditPayloadCodecError> {
        if audit_record_id.is_nil()
            || audit_record_id.get_variant() != Variant::RFC4122
            || audit_record_id.get_version() != Some(Version::Random)
        {
            return Err(McpAuditPayloadCodecError::InvalidAuditRecordId);
        }
        Ok(Self {
            store_identity_digest,
            audit_record_id,
            key_epoch,
            row_context_digest,
            role,
        })
    }

    pub(crate) fn store_identity_digest(&self) -> &[u8; 32] {
        &self.store_identity_digest
    }

    pub(crate) fn audit_record_id(&self) -> Uuid {
        self.audit_record_id
    }

    pub(crate) fn key_epoch(&self) -> u64 {
        self.key_epoch
    }

    pub(crate) fn row_context_digest(&self) -> &[u8; 32] {
        &self.row_context_digest
    }

    pub(crate) fn role(&self) -> McpAuditPayloadRole {
        self.role
    }
}

/// Authenticated facts returned by the codec. Neither field is sourced from a
/// database column or caller hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthenticatedMinimizedAuditPayload {
    format_version: u32,
    role: McpAuditPayloadRole,
    receipt: MinimizedAuditReceiptV1,
}

impl AuthenticatedMinimizedAuditPayload {
    pub(crate) fn format_version(&self) -> u32 {
        self.format_version
    }

    pub(crate) fn role(&self) -> McpAuditPayloadRole {
        self.role
    }

    pub(crate) fn receipt(&self) -> &MinimizedAuditReceiptV1 {
        &self.receipt
    }
}

/// Sanitized codec failures. No variant includes plaintext, ciphertext, key
/// material, or the supplied digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum McpAuditPayloadCodecError {
    InvalidEnvelopeEncoding,
    InvalidEnvelopeStructure,
    EnvelopeTooLarge,
    InvalidEnvelopeRole,
    InvalidAuditRecordId,
    AuthenticationFailed,
    UnsupportedFormatVersion(u32),
    InvalidReceiptSchema,
    ReceiptRoleMismatch {
        authenticated: McpAuditPayloadRole,
        receipt: McpAuditPayloadRole,
    },
    EnvelopeBindingRoleMismatch {
        authenticated: McpAuditPayloadRole,
        expected: McpAuditPayloadRole,
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
            Self::InvalidAuditRecordId => {
                "MCP audit payload binding requires a non-nil RFC 4122 UUIDv4 record id"
            }
            Self::AuthenticationFailed => "MCP audit payload authentication failed",
            Self::UnsupportedFormatVersion(_) => {
                "unsupported authenticated MCP audit payload format version"
            }
            Self::InvalidReceiptSchema => "invalid minimized MCP audit receipt schema",
            Self::ReceiptRoleMismatch { .. } => {
                "minimized MCP audit receipt role does not match authenticated role"
            }
            Self::EnvelopeBindingRoleMismatch { .. } => {
                "authenticated MCP audit envelope role does not match expected binding"
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

/// Encrypt a validated v1 receipt. The wire header and the complete typed
/// storage binding are authenticated as AEAD associated data and therefore
/// cannot be changed independently of the ciphertext.
pub(crate) fn seal_minimized_audit_receipt_v1(
    key: &[u8; 32],
    binding: &McpAuditPayloadBindingV1,
    receipt: &MinimizedAuditReceiptV1,
) -> Result<String, McpAuditPayloadCodecError> {
    receipt.validate(binding.role())?;
    let plaintext = serde_json::to_vec(receipt)
        .map_err(|_| McpAuditPayloadCodecError::ReceiptSerializationFailed)?;
    seal_plaintext(key, MCP_AUDIT_PAYLOAD_FORMAT_V1, binding, &plaintext)
}

/// Decode one envelope using the storage owner's exact expected binding. The
/// envelope header and expected binding jointly form AEAD associated data;
/// substitution therefore fails at authentication rather than relying on a
/// caller to compare metadata after decryption.
pub(crate) fn open_authenticated_audit_payload(
    key: &[u8; 32],
    expected_binding: &McpAuditPayloadBindingV1,
    encoded_envelope: &str,
) -> Result<AuthenticatedMinimizedAuditPayload, McpAuditPayloadCodecError> {
    if encoded_envelope.len() > MCP_AUDIT_MAX_ENVELOPE_ENCODED_BYTES {
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
    let aad = authenticated_aad(header, expected_binding);
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
    if role != expected_binding.role() {
        // The expected role already participates in AEAD AAD. Retain this
        // invariant check so an internally forged envelope cannot bypass the
        // binding contract even when produced by code holding the key.
        return Err(McpAuditPayloadCodecError::EnvelopeBindingRoleMismatch {
            authenticated: role,
            expected: expected_binding.role(),
        });
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
    binding: &McpAuditPayloadBindingV1,
    plaintext: &[u8],
) -> Result<String, McpAuditPayloadCodecError> {
    let header = envelope_header(format_version, binding.role());
    let nonce_bytes = rand::random::<[u8; AES_GCM_NONCE_BYTES]>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| McpAuditPayloadCodecError::EncryptionFailed)?;
    let aad = authenticated_aad(&header, binding);
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

/// Test-only fixture issuer for adversarial envelopes. Product code can only
/// seal a validated `MinimizedAuditReceiptV1`; the D068 matrix additionally
/// needs authenticated but structurally invalid plaintext to prove the strict
/// decoder is the rejecting authority.
#[cfg(test)]
pub(crate) fn seal_payload_fixture_for_test(
    key: &[u8; 32],
    format_version: u32,
    binding: &McpAuditPayloadBindingV1,
    plaintext: &[u8],
) -> Result<String, McpAuditPayloadCodecError> {
    seal_plaintext(key, format_version, binding, plaintext)
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

fn authenticated_aad(header: &[u8], binding: &McpAuditPayloadBindingV1) -> Vec<u8> {
    let key_epoch = binding.key_epoch().to_be_bytes();
    let role = [binding.role().wire_tag()];
    let audit_record_id = binding.audit_record_id();
    let mut aad = Vec::with_capacity(
        AAD_DOMAIN.len()
            + header.len()
            + binding.store_identity_digest().len()
            + audit_record_id.as_bytes().len()
            + key_epoch.len()
            + binding.row_context_digest().len()
            + role.len()
            + 7 * (1 + std::mem::size_of::<u32>()),
    );
    append_aad_field(&mut aad, AAD_DOMAIN_TAG, AAD_DOMAIN);
    append_aad_field(&mut aad, AAD_HEADER_TAG, header);
    append_aad_field(
        &mut aad,
        AAD_STORE_IDENTITY_TAG,
        binding.store_identity_digest(),
    );
    append_aad_field(&mut aad, AAD_RECORD_ID_TAG, audit_record_id.as_bytes());
    append_aad_field(&mut aad, AAD_KEY_EPOCH_TAG, &key_epoch);
    append_aad_field(&mut aad, AAD_ROW_CONTEXT_TAG, binding.row_context_digest());
    append_aad_field(&mut aad, AAD_EXPECTED_ROLE_TAG, &role);
    aad
}

fn append_aad_field(target: &mut Vec<u8>, tag: u8, value: &[u8]) {
    let length = u32::try_from(value.len()).expect("D068 AAD fields have fixed bounded lengths");
    target.push(tag);
    target.extend_from_slice(&length.to_be_bytes());
    target.extend_from_slice(value);
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

    fn record_id(suffix: u64) -> Uuid {
        Uuid::parse_str(&format!("00000000-0000-4000-8000-{suffix:012x}"))
            .expect("fixed test UUIDv4")
    }

    fn binding_with(
        role: McpAuditPayloadRole,
        store_byte: u8,
        record_suffix: u64,
        key_epoch: u64,
        row_context_byte: u8,
    ) -> McpAuditPayloadBindingV1 {
        McpAuditPayloadBindingV1::new(
            [store_byte; 32],
            record_id(record_suffix),
            key_epoch,
            [row_context_byte; 32],
            role,
        )
        .expect("valid fixed test binding")
    }

    fn binding(role: McpAuditPayloadRole) -> McpAuditPayloadBindingV1 {
        binding_with(role, 0x51, 1, 7, 0x61)
    }

    fn seal_authenticated_fixture(
        format_version: u32,
        binding: &McpAuditPayloadBindingV1,
        value: Value,
    ) -> String {
        seal_plaintext(&KEY, format_version, binding, value.to_string().as_bytes()).unwrap()
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
    fn mcp_audit_payload_codec_v1_round_trips_authenticated_version_role_and_exact_receipt() {
        let argument_value = json!({"bounded": true, "count": 7});
        let argument_bytes = serde_json::to_vec(&argument_value).unwrap();
        let arguments = MinimizedAuditReceiptV1::for_arguments(&argument_value).unwrap();
        let result_text = "bounded-result";
        let result = MinimizedAuditReceiptV1::for_result(result_text).unwrap();

        for receipt in [&arguments, &result] {
            let binding = binding(receipt.kind());
            let envelope = seal_minimized_audit_receipt_v1(&KEY, &binding, receipt).unwrap();
            let authenticated =
                open_authenticated_audit_payload(&KEY, &binding, &envelope).unwrap();
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
    fn mcp_audit_payload_codec_v1_streaming_argument_receipts_match_serde_json_bytes() {
        let values = [
            Value::Null,
            json!(true),
            json!(-9_223_372_036_854_775_808_i64),
            json!("Unicode 雪 and escapes: \"quoted\" \\ slash\nline"),
            json!([null, false, 42, "nested", {"key": "value"}]),
            json!({
                "object": {"alpha": 1, "beta": [2, 3]},
                "control": "\u{0008}\u{000c}\r\t"
            }),
        ];

        for value in values {
            let expected_bytes = serde_json::to_vec(&value).unwrap();
            let expected_digest = ring::digest::digest(&SHA256, &expected_bytes);
            let receipt = MinimizedAuditReceiptV1::for_arguments(&value).unwrap();

            assert_eq!(receipt.bytes(), expected_bytes.len() as u64, "{value:?}");
            assert_eq!(
                receipt.digest(),
                format!(
                    "sha256:{}",
                    general_purpose::STANDARD_NO_PAD.encode(expected_digest.as_ref())
                ),
                "{value:?}"
            );
        }
    }

    #[test]
    fn mcp_audit_payload_codec_v1_digest_writer_overflow_preserves_prior_count_and_digest() {
        let prefix = b"already-authenticated-prefix";
        let mut writer = ReceiptDigestWriter::new();
        assert_eq!(std::io::Write::write(&mut writer, &prefix[..8]).unwrap(), 8);
        assert_eq!(
            std::io::Write::write(&mut writer, &prefix[8..]).unwrap(),
            prefix.len() - 8
        );
        writer.bytes = u64::MAX;

        let error = std::io::Write::write(&mut writer, b"!")
            .expect_err("a write beyond the receipt counter must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert!(writer.length_overflowed());

        let (bytes, digest) = writer.finish();
        assert_eq!(bytes, u64::MAX, "the failing write must not wrap the count");
        assert_eq!(
            digest.as_ref(),
            ring::digest::digest(&SHA256, prefix).as_ref(),
            "the failing write must not mutate the accumulated digest"
        );
    }

    #[test]
    fn mcp_audit_payload_codec_v1_header_version_and_role_are_aead_authenticated() {
        let receipt = MinimizedAuditReceiptV1::for_arguments(&json!({"bounded": true})).unwrap();
        let binding = binding(McpAuditPayloadRole::Arguments);
        let envelope = seal_minimized_audit_receipt_v1(&KEY, &binding, &receipt).unwrap();
        let original = general_purpose::STANDARD.decode(&envelope).unwrap();

        let version_start = ENVELOPE_MAGIC.len();
        let version_end = version_start + FORMAT_VERSION_BYTES;
        let mut version_tampered = original.clone();
        version_tampered[version_start..version_end].copy_from_slice(&2_u32.to_be_bytes());
        let version_tampered = general_purpose::STANDARD.encode(version_tampered);
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &binding, &version_tampered),
            Err(McpAuditPayloadCodecError::AuthenticationFailed)
        );

        let mut role_tampered = original;
        role_tampered[version_end] = McpAuditPayloadRole::Result.wire_tag();
        let role_tampered = general_purpose::STANDARD.encode(role_tampered);
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &binding, &role_tampered),
            Err(McpAuditPayloadCodecError::AuthenticationFailed)
        );

        let unsupported = seal_authenticated_fixture(
            2,
            &binding,
            valid_receipt_value(McpAuditPayloadRole::Arguments),
        );
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &binding, &unsupported),
            Err(McpAuditPayloadCodecError::UnsupportedFormatVersion(2))
        );
    }

    #[test]
    fn mcp_audit_payload_codec_v1_binding_rejects_nil_non_v4_and_non_rfc4122_record_ids() {
        let invalid_ids = [
            Uuid::nil(),
            Uuid::parse_str("00000000-0000-5000-8000-000000000001").unwrap(),
            Uuid::parse_str("00000000-0000-4000-0000-000000000001").unwrap(),
        ];

        for audit_record_id in invalid_ids {
            assert_eq!(
                McpAuditPayloadBindingV1::new(
                    [0x51; 32],
                    audit_record_id,
                    7,
                    [0x61; 32],
                    McpAuditPayloadRole::Arguments,
                ),
                Err(McpAuditPayloadCodecError::InvalidAuditRecordId)
            );
        }
    }

    #[test]
    fn mcp_audit_payload_codec_v1_aad_is_canonical_length_delimited_and_complete() {
        let binding = binding(McpAuditPayloadRole::Arguments);
        let header = envelope_header(MCP_AUDIT_PAYLOAD_FORMAT_V1, McpAuditPayloadRole::Arguments);
        let aad = authenticated_aad(&header, &binding);
        let audit_record_id = binding.audit_record_id();
        let key_epoch = binding.key_epoch().to_be_bytes();
        let role = [binding.role().wire_tag()];
        let expected = [
            (AAD_DOMAIN_TAG, AAD_DOMAIN),
            (AAD_HEADER_TAG, header.as_slice()),
            (
                AAD_STORE_IDENTITY_TAG,
                binding.store_identity_digest().as_slice(),
            ),
            (AAD_RECORD_ID_TAG, audit_record_id.as_bytes().as_slice()),
            (AAD_KEY_EPOCH_TAG, key_epoch.as_slice()),
            (AAD_ROW_CONTEXT_TAG, binding.row_context_digest().as_slice()),
            (AAD_EXPECTED_ROLE_TAG, role.as_slice()),
        ];

        let mut cursor = 0;
        for (expected_tag, expected_value) in expected {
            assert_eq!(aad[cursor], expected_tag);
            cursor += 1;
            let value_length = u32::from_be_bytes(
                aad[cursor..cursor + std::mem::size_of::<u32>()]
                    .try_into()
                    .unwrap(),
            ) as usize;
            cursor += std::mem::size_of::<u32>();
            assert_eq!(value_length, expected_value.len());
            assert_eq!(&aad[cursor..cursor + value_length], expected_value);
            cursor += value_length;
        }
        assert_eq!(cursor, aad.len(), "AAD contains no unparsed trailing bytes");
    }

    #[test]
    fn mcp_audit_payload_codec_v1_expected_binding_blocks_same_role_and_column_replay() {
        let arguments_receipt =
            MinimizedAuditReceiptV1::for_arguments(&json!({"bounded": true})).unwrap();
        let result_receipt = MinimizedAuditReceiptV1::for_result("bounded-result").unwrap();
        let arguments_binding = binding(McpAuditPayloadRole::Arguments);
        let result_binding = binding(McpAuditPayloadRole::Result);
        let arguments_envelope =
            seal_minimized_audit_receipt_v1(&KEY, &arguments_binding, &arguments_receipt).unwrap();
        let result_envelope =
            seal_minimized_audit_receipt_v1(&KEY, &result_binding, &result_receipt).unwrap();

        let counterfactuals = [
            (
                "same_role_cross_row",
                binding_with(McpAuditPayloadRole::Arguments, 0x51, 2, 7, 0x61),
                &arguments_envelope,
            ),
            (
                "cross_epoch",
                binding_with(McpAuditPayloadRole::Arguments, 0x51, 1, 8, 0x61),
                &arguments_envelope,
            ),
            (
                "cross_store",
                binding_with(McpAuditPayloadRole::Arguments, 0x52, 1, 7, 0x61),
                &arguments_envelope,
            ),
            (
                "row_metadata_digest_swap",
                binding_with(McpAuditPayloadRole::Arguments, 0x51, 1, 7, 0x62),
                &arguments_envelope,
            ),
            (
                "arguments_into_result_column",
                result_binding,
                &arguments_envelope,
            ),
            (
                "result_into_arguments_column",
                arguments_binding,
                &result_envelope,
            ),
        ];

        let mut failures = Vec::new();
        for (label, expected_binding, envelope) in counterfactuals {
            let observed = open_authenticated_audit_payload(&KEY, &expected_binding, envelope);
            if observed != Err(McpAuditPayloadCodecError::AuthenticationFailed) {
                failures.push((label, observed));
            }
        }
        assert!(
            failures.is_empty(),
            "binding substitutions did not fail at AEAD authentication: {failures:?}"
        );
    }

    #[test]
    fn mcp_audit_payload_codec_v1_strict_receipt_schema_rejects_all_field_shape_failures() {
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
            let binding = binding(role);
            let envelope =
                seal_authenticated_fixture(MCP_AUDIT_PAYLOAD_FORMAT_V1, &binding, candidate);
            assert_eq!(
                open_authenticated_audit_payload(&KEY, &binding, &envelope),
                Err(McpAuditPayloadCodecError::InvalidReceiptSchema)
            );
        }
    }

    #[test]
    fn mcp_audit_payload_codec_v1_enforces_receipt_role_payload_stored_value_type_and_digest() {
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
            let binding = binding(role);
            let envelope = seal_authenticated_fixture(MCP_AUDIT_PAYLOAD_FORMAT_V1, &binding, value);
            assert_eq!(
                open_authenticated_audit_payload(&KEY, &binding, &envelope),
                Err(expected)
            );
        }
    }

    #[test]
    fn mcp_audit_payload_codec_v1_rejects_ciphertext_corruption_wrong_keys_and_noncanonical_wire() {
        let binding = binding(McpAuditPayloadRole::Result);
        let envelope = [1_usize, 10, 100]
            .into_iter()
            .find_map(|result_bytes| {
                let result = "x".repeat(result_bytes);
                let receipt = MinimizedAuditReceiptV1::for_result(&result).unwrap();
                let candidate = seal_minimized_audit_receipt_v1(&KEY, &binding, &receipt).unwrap();
                candidate.ends_with('=').then_some(candidate)
            })
            .expect("one of three consecutive receipt sizes requires base64 padding");
        let mut corrupted = general_purpose::STANDARD.decode(&envelope).unwrap();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        let corrupted = general_purpose::STANDARD.encode(corrupted);
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &binding, &corrupted),
            Err(McpAuditPayloadCodecError::AuthenticationFailed)
        );
        assert_eq!(
            open_authenticated_audit_payload(&[0x69; 32], &binding, &envelope),
            Err(McpAuditPayloadCodecError::AuthenticationFailed)
        );

        let unpadded = envelope.trim_end_matches('=');
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &binding, unpadded),
            Err(McpAuditPayloadCodecError::InvalidEnvelopeEncoding)
        );
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &binding, "not base64"),
            Err(McpAuditPayloadCodecError::InvalidEnvelopeEncoding)
        );
    }

    #[test]
    fn mcp_audit_payload_codec_v1_rejects_oversized_envelopes_before_or_after_base64_decode() {
        let binding = binding(McpAuditPayloadRole::Arguments);
        let oversized_encoded = "A".repeat(MCP_AUDIT_MAX_ENVELOPE_ENCODED_BYTES + 1);
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &binding, &oversized_encoded),
            Err(McpAuditPayloadCodecError::EnvelopeTooLarge)
        );

        // 1,025 zero bytes have a canonical padded Base64 representation with
        // the same encoded length as the largest admitted 1,024-byte wire.
        // This proves the decoded-wire check is independent of the cheap
        // pre-decode bound.
        let oversized_wire = vec![0_u8; MAX_ENVELOPE_WIRE_BYTES + 1];
        let canonical_oversized = general_purpose::STANDARD.encode(&oversized_wire);
        assert_eq!(
            canonical_oversized.len(),
            MCP_AUDIT_MAX_ENVELOPE_ENCODED_BYTES
        );
        assert_eq!(
            general_purpose::STANDARD
                .decode(&canonical_oversized)
                .unwrap(),
            oversized_wire
        );
        assert_eq!(
            open_authenticated_audit_payload(&KEY, &binding, &canonical_oversized),
            Err(McpAuditPayloadCodecError::EnvelopeTooLarge)
        );
    }
}
