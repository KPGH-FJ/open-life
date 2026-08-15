use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use ring::digest::{Context as DigestContext, SHA256};

const MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION: i64 = 2;
const MCP_AUDIT_LEGACY_MINIMIZED_VERSION: i64 = 1;
const MCP_AUDIT_RECEIPT_SCHEMA_VERSION: u64 = 2;

#[derive(Debug)]
struct McpAuditPayloadIntegrityError {
    reason_code: &'static str,
}

impl std::fmt::Display for McpAuditPayloadIntegrityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "MCP audit payload integrity validation failed: {}",
            self.reason_code
        )
    }
}

impl std::error::Error for McpAuditPayloadIntegrityError {}

fn mcp_audit_payload_integrity_error(reason_code: &'static str) -> anyhow::Error {
    anyhow::Error::new(McpAuditPayloadIntegrityError { reason_code })
}

/// Distinguish a proven payload/envelope integrity failure from unavailable or
/// unproven key material. Bootstrap uses this only for truthful store-health
/// attribution; either class still keeps the audit store unavailable and
/// blocks effects.
pub fn is_payload_integrity_failure(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<McpAuditPayloadIntegrityError>()
            .is_some()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AuditPayloadKind {
    Arguments,
    Result,
}

impl AuditPayloadKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Arguments => "arguments",
            Self::Result => "result",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AuditPayloadValueType {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
    UnparseableLegacy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuditPayloadReceiptV1 {
    kind: AuditPayloadKind,
    payload_stored: bool,
    value_type: AuditPayloadValueType,
    bytes: u64,
    digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuditPayloadReceiptV2 {
    schema_version: u64,
    kind: AuditPayloadKind,
    payload_stored: bool,
    value_type: AuditPayloadValueType,
    bytes: u64,
    digest: String,
}

impl AuditPayloadReceiptV2 {
    fn validate(&self, expected_kind: AuditPayloadKind) -> Result<()> {
        if self.schema_version != MCP_AUDIT_RECEIPT_SCHEMA_VERSION {
            anyhow::bail!(
                "MCP audit receipt schema version {} is unsupported",
                self.schema_version
            );
        }
        validate_audit_payload_receipt_fields(
            self.kind,
            expected_kind,
            self.payload_stored,
            self.value_type,
            &self.digest,
        )
    }

    fn canonical_json(&self) -> Result<String> {
        serde_json::to_string(self).context("serialize strict MCP audit receipt")
    }
}

impl AuditPayloadReceiptV1 {
    fn into_current(self, expected_kind: AuditPayloadKind) -> Result<AuditPayloadReceiptV2> {
        validate_audit_payload_receipt_fields(
            self.kind,
            expected_kind,
            self.payload_stored,
            self.value_type,
            &self.digest,
        )?;
        Ok(AuditPayloadReceiptV2 {
            schema_version: MCP_AUDIT_RECEIPT_SCHEMA_VERSION,
            kind: self.kind,
            payload_stored: self.payload_stored,
            value_type: self.value_type,
            bytes: self.bytes,
            digest: self.digest,
        })
    }
}

fn validate_audit_payload_receipt_fields(
    actual_kind: AuditPayloadKind,
    expected_kind: AuditPayloadKind,
    payload_stored: bool,
    value_type: AuditPayloadValueType,
    digest: &str,
) -> Result<()> {
    if actual_kind != expected_kind {
        anyhow::bail!(
            "MCP audit receipt kind mismatch: expected {}, observed {}",
            expected_kind.as_str(),
            actual_kind.as_str()
        );
    }
    if payload_stored {
        anyhow::bail!("MCP audit receipt cannot store payload bytes");
    }
    match expected_kind {
        AuditPayloadKind::Arguments => {}
        AuditPayloadKind::Result if value_type == AuditPayloadValueType::String => {}
        AuditPayloadKind::Result => {
            anyhow::bail!("MCP audit result receipt must have string valueType")
        }
    }
    let encoded = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow::anyhow!("MCP audit receipt digest prefix is invalid"))?;
    let decoded = general_purpose::STANDARD_NO_PAD
        .decode(encoded)
        .context("MCP audit receipt digest is invalid base64")?;
    if decoded.len() != 32 {
        anyhow::bail!("MCP audit receipt digest length is invalid");
    }
    Ok(())
}

fn audit_payload_receipt(
    kind: AuditPayloadKind,
    value_type: AuditPayloadValueType,
    bytes: &[u8],
) -> AuditPayloadReceiptV2 {
    let digest = ring::digest::digest(&SHA256, bytes);
    AuditPayloadReceiptV2 {
        schema_version: MCP_AUDIT_RECEIPT_SCHEMA_VERSION,
        kind,
        payload_stored: false,
        value_type,
        bytes: bytes.len() as u64,
        digest: format!(
            "sha256:{}",
            general_purpose::STANDARD_NO_PAD.encode(digest.as_ref())
        ),
    }
}

fn audit_arguments_receipt(arguments: &Value) -> Result<AuditPayloadReceiptV2> {
    let encoded = serde_json::to_vec(arguments).context("serialize MCP argument receipt input")?;
    let value_type = match arguments {
        Value::Null => AuditPayloadValueType::Null,
        Value::Bool(_) => AuditPayloadValueType::Bool,
        Value::Number(_) => AuditPayloadValueType::Number,
        Value::String(_) => AuditPayloadValueType::String,
        Value::Array(_) => AuditPayloadValueType::Array,
        Value::Object(_) => AuditPayloadValueType::Object,
    };
    Ok(audit_payload_receipt(
        AuditPayloadKind::Arguments,
        value_type,
        &encoded,
    ))
}

fn audit_result_receipt(result: &str) -> AuditPayloadReceiptV2 {
    audit_payload_receipt(
        AuditPayloadKind::Result,
        AuditPayloadValueType::String,
        result.as_bytes(),
    )
}

/// Key management mode for MCP audit encryption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KeyMode {
    /// Derive key from a fixed app secret (default, backward-compatible)
    #[default]
    Derived,
    /// User-provided passphrase (more secure, user-controlled)
    Passphrase,
    /// Environment variable sourced key
    Env,
    /// Random key material held by the operating-system credential store.
    Keychain,
}

/// Key management configuration for MCP audit logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditKeyConfig {
    pub mode: KeyMode,
    /// For Passphrase mode: base64-encoded salt (16 bytes)
    pub salt_b64: Option<String>,
    /// For Env mode: environment variable name
    pub env_var: Option<String>,
    /// Opaque credential-store reference. The secret itself is never serialized here.
    #[serde(default)]
    pub key_ref: Option<String>,
    /// Key rotation epoch (monotonically increasing)
    pub epoch: u64,
    /// When this key config was created
    pub created_at: String,
}

impl Default for AuditKeyConfig {
    fn default() -> Self {
        Self {
            mode: KeyMode::Derived,
            salt_b64: None,
            env_var: None,
            key_ref: None,
            epoch: 0,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Export payload for audit logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditExport {
    pub exported_at: String,
    pub entry_count: usize,
    pub days: i64,
    pub entries: Vec<ExportedAuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedAuditEntry {
    pub id: i64,
    pub tool_name: String,
    pub arguments: String,
    pub result: String,
    pub success: bool,
    pub pii_found: bool,
    pub created_at: String,
}

/// One encrypted MCP audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLogEntry {
    pub id: i64,
    pub tool_name: String,
    pub arguments: String,
    pub result: String,
    pub success: bool,
    pub pii_found: bool,
    pub created_at: String,
}

#[derive(Clone)]
pub struct AuditKeyMaterial {
    pub config: AuditKeyConfig,
    pub key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpAuditDatabaseInspection {
    pub exists: bool,
    pub row_count: u64,
    pub key_epochs: Vec<u64>,
}

impl McpAuditDatabaseInspection {
    pub fn is_empty_or_absent(&self) -> bool {
        self.row_count == 0
    }
}

/// Encrypted SQLite-backed store for MCP call logs with configurable key management.
#[derive(Clone)]
pub struct McpAuditStore {
    db_path: PathBuf,
    read_only: bool,
    unavailable_reason: Option<String>,
    key: [u8; 32],
    key_config: AuditKeyConfig,
    keyring: HashMap<u64, [u8; 32]>,
    key_configs: Vec<AuditKeyConfig>,
}

fn mcp_audit_log_columns(conn: &Connection) -> Result<HashSet<String>> {
    let mut statement = conn.prepare("PRAGMA table_info(mcp_log)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    columns
        .collect::<std::result::Result<HashSet<_>, _>>()
        .map_err(Into::into)
}

fn mcp_audit_material_keyring(materials: &[AuditKeyMaterial]) -> Result<HashMap<u64, [u8; 32]>> {
    let mut previous_epoch = None;
    let mut keyring = HashMap::new();
    for material in materials {
        i64::try_from(material.config.epoch)
            .context("MCP audit key epoch exceeds the SQLite integer range")?;
        if previous_epoch.is_some_and(|epoch| epoch >= material.config.epoch) {
            anyhow::bail!("MCP audit key materials must be strictly increasing by epoch");
        }
        if material.key.iter().all(|byte| *byte == 0) {
            anyhow::bail!("MCP audit key material must not be all-zero");
        }
        if keyring
            .insert(material.config.epoch, material.key)
            .is_some()
        {
            anyhow::bail!("duplicate MCP audit key epoch");
        }
        previous_epoch = Some(material.config.epoch);
    }
    Ok(keyring)
}

fn mcp_audit_payload_aad(payload_version: i64, kind: AuditPayloadKind, key_epoch: u64) -> Vec<u8> {
    format!(
        "openlife.mcp_audit|payload_version={payload_version}|kind={}|key_epoch={key_epoch}",
        kind.as_str()
    )
    .into_bytes()
}

fn encrypt_mcp_audit_ciphertext(plaintext: &str, key: &[u8; 32], aad: &[u8]) -> Result<String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext.as_bytes(),
                aad,
            },
        )
        .map_err(|error| anyhow::anyhow!("encrypt failed: {error:?}"))?;
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(general_purpose::STANDARD.encode(&combined))
}

fn decrypt_mcp_audit_ciphertext(combined_b64: &str, key: &[u8; 32], aad: &[u8]) -> Result<String> {
    let combined = general_purpose::STANDARD
        .decode(combined_b64)
        .context("invalid base64")?;
    if combined.len() < 12 {
        anyhow::bail!("ciphertext too short");
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|error| anyhow::anyhow!("decrypt failed: {error:?}"))?;
    String::from_utf8(plaintext).context("utf8 decode")
}

fn decode_legacy_minimized_receipt(
    plaintext: &str,
    expected_kind: AuditPayloadKind,
) -> Result<AuditPayloadReceiptV2> {
    let receipt = serde_json::from_str::<AuditPayloadReceiptV1>(plaintext)
        .map_err(|_| mcp_audit_payload_integrity_error("legacy_receipt_schema_invalid"))?;
    receipt
        .into_current(expected_kind)
        .map_err(|_| mcp_audit_payload_integrity_error("legacy_receipt_semantics_invalid"))
}

fn decode_current_audit_receipt(
    plaintext: &str,
    expected_kind: AuditPayloadKind,
) -> Result<AuditPayloadReceiptV2> {
    let receipt = serde_json::from_str::<AuditPayloadReceiptV2>(plaintext)
        .map_err(|_| mcp_audit_payload_integrity_error("current_receipt_schema_invalid"))?;
    receipt
        .validate(expected_kind)
        .map_err(|_| mcp_audit_payload_integrity_error("current_receipt_semantics_invalid"))?;
    Ok(receipt)
}

fn decrypt_current_audit_receipt(
    combined_b64: &str,
    key: &[u8; 32],
    key_epoch: u64,
    payload_version: i64,
    expected_kind: AuditPayloadKind,
) -> Result<AuditPayloadReceiptV2> {
    if payload_version != MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION {
        anyhow::bail!(
            "MCP audit payload version {payload_version} requires migration before current receipt decoding"
        );
    }
    let aad = mcp_audit_payload_aad(payload_version, expected_kind, key_epoch);
    let plaintext = match decrypt_mcp_audit_ciphertext(combined_b64, key, &aad) {
        Ok(plaintext) => plaintext,
        Err(authentication_error) => {
            // Historical v0/v1 ciphertext used the same key and nonce layout
            // without AAD. A successful legacy authentication here proves the
            // key material while proving that the plaintext version column was
            // promoted without the required authenticated envelope.
            if decrypt_mcp_audit_ciphertext(combined_b64, key, &[]).is_ok() {
                return Err(mcp_audit_payload_integrity_error(
                    "authenticated_format_binding_mismatch",
                ));
            }
            return Err(authentication_error);
        }
    };
    decode_current_audit_receipt(&plaintext, expected_kind)
}

fn validate_mcp_audit_ciphertext_row(
    id: i64,
    arguments_encrypted: &str,
    result_encrypted: &str,
    key_epoch: i64,
    payload_version: i64,
    keyring: &HashMap<u64, [u8; 32]>,
) -> Result<u64> {
    let key_epoch = u64::try_from(key_epoch)
        .with_context(|| format!("MCP audit row {id} contains a negative key epoch"))?;
    let key = keyring.get(&key_epoch).ok_or_else(|| {
        anyhow::anyhow!("MCP audit row {id} requires unavailable key epoch {key_epoch}")
    })?;
    match payload_version {
        0 => {
            decrypt_mcp_audit_ciphertext(arguments_encrypted, key, &[]).with_context(|| {
                format!("authenticate legacy MCP audit arguments for row {id} epoch {key_epoch}")
            })?;
            decrypt_mcp_audit_ciphertext(result_encrypted, key, &[]).with_context(|| {
                format!("authenticate legacy MCP audit result for row {id} epoch {key_epoch}")
            })?;
        }
        MCP_AUDIT_LEGACY_MINIMIZED_VERSION => {
            let arguments =
                decrypt_mcp_audit_ciphertext(arguments_encrypted, key, &[]).with_context(|| {
                    format!(
                        "authenticate legacy minimized MCP audit arguments for row {id} epoch {key_epoch}"
                    )
                })?;
            decode_legacy_minimized_receipt(&arguments, AuditPayloadKind::Arguments).with_context(
                || format!("validate legacy minimized MCP audit arguments for row {id}"),
            )?;
            let result =
                decrypt_mcp_audit_ciphertext(result_encrypted, key, &[]).with_context(|| {
                    format!(
                        "authenticate legacy minimized MCP audit result for row {id} epoch {key_epoch}"
                    )
                })?;
            decode_legacy_minimized_receipt(&result, AuditPayloadKind::Result).with_context(
                || format!("validate legacy minimized MCP audit result for row {id}"),
            )?;
        }
        MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION => {
            decrypt_current_audit_receipt(
                arguments_encrypted,
                key,
                key_epoch,
                payload_version,
                AuditPayloadKind::Arguments,
            )
            .with_context(|| {
                format!("authenticate current MCP audit arguments for row {id} epoch {key_epoch}")
            })?;
            decrypt_current_audit_receipt(
                result_encrypted,
                key,
                key_epoch,
                payload_version,
                AuditPayloadKind::Result,
            )
            .with_context(|| {
                format!("authenticate current MCP audit result for row {id} epoch {key_epoch}")
            })?;
        }
        other => anyhow::bail!("MCP audit row {id} has unsupported payload version {other}"),
    }
    Ok(key_epoch)
}

impl McpAuditStore {
    pub fn inspect_existing_database(path: impl AsRef<Path>) -> Result<McpAuditDatabaseInspection> {
        let path = path.as_ref();
        match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(McpAuditDatabaseInspection {
                    exists: false,
                    row_count: 0,
                    key_epochs: Vec::new(),
                });
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspect MCP audit database at {}", path.display()));
            }
            Ok(metadata) if metadata.file_type().is_symlink() => {
                anyhow::bail!("MCP audit database symlinks are not accepted");
            }
            Ok(metadata) if !metadata.is_file() => {
                anyhow::bail!("MCP audit database path is not a regular file");
            }
            Ok(_) => {}
        }

        let conn = crate::sqlite_migration::open_existing_read_only(
            path,
            "mcp_audit_store_preflight",
            &["mcp_log"],
        )?;
        let columns = mcp_audit_log_columns(&conn)?;
        for required in [
            "id",
            "tool_name",
            "arguments_encrypted",
            "result_encrypted",
            "success",
            "pii_found",
            "created_at",
        ] {
            if !columns.contains(required) {
                anyhow::bail!("MCP audit database is missing required column {required}");
            }
        }
        let row_count = conn.query_row("SELECT COUNT(*) FROM mcp_log", [], |row| {
            row.get::<_, i64>(0)
        })?;
        let row_count =
            u64::try_from(row_count).context("MCP audit database reported a negative row count")?;
        let key_epochs = if row_count == 0 {
            Vec::new()
        } else if columns.contains("key_epoch") {
            let mut statement =
                conn.prepare("SELECT DISTINCT key_epoch FROM mcp_log ORDER BY key_epoch ASC")?;
            let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
            let mut epochs = Vec::new();
            for epoch in rows {
                epochs.push(
                    u64::try_from(epoch?)
                        .context("MCP audit database contains a negative key epoch")?,
                );
            }
            epochs
        } else {
            vec![0]
        };
        Ok(McpAuditDatabaseInspection {
            exists: true,
            row_count,
            key_epochs,
        })
    }

    pub fn preflight_existing_database_key_materials(
        path: impl AsRef<Path>,
        materials: &[AuditKeyMaterial],
    ) -> Result<McpAuditDatabaseInspection> {
        let path = path.as_ref();
        let inspection = Self::inspect_existing_database(path)?;
        if !inspection.exists || inspection.row_count == 0 {
            return Ok(inspection);
        }
        let keyring = mcp_audit_material_keyring(materials)?;
        for epoch in &inspection.key_epochs {
            if !keyring.contains_key(epoch) {
                anyhow::bail!("MCP audit database requires uncovered key epoch {epoch}");
            }
        }

        let conn = crate::sqlite_migration::open_existing_read_only(
            path,
            "mcp_audit_store_key_preflight",
            &["mcp_log"],
        )?;
        let columns = mcp_audit_log_columns(&conn)?;
        let epoch_expression = if columns.contains("key_epoch") {
            "key_epoch"
        } else {
            "0"
        };
        let payload_version_expression = if columns.contains("payload_minimized_version") {
            "payload_minimized_version"
        } else {
            "0"
        };
        let pending_predicate = if columns.contains("payload_minimized_version") {
            format!(" WHERE payload_minimized_version < {MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION}")
        } else {
            String::new()
        };
        let pending_sql = format!(
            "SELECT id, arguments_encrypted, result_encrypted, {epoch_expression}, {payload_version_expression}
             FROM mcp_log{pending_predicate} ORDER BY id ASC"
        );
        let mut pending_statement = conn.prepare(&pending_sql)?;
        let pending_rows = pending_statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        let mut authenticated_epochs = HashSet::new();
        for row in pending_rows {
            let (id, arguments, result, epoch, payload_version) = row?;
            authenticated_epochs.insert(validate_mcp_audit_ciphertext_row(
                id,
                &arguments,
                &result,
                epoch,
                payload_version,
                &keyring,
            )?);
        }
        for epoch in &inspection.key_epochs {
            if authenticated_epochs.contains(epoch) {
                continue;
            }
            let sample_sql = if columns.contains("key_epoch") {
                format!(
                    "SELECT id, arguments_encrypted, result_encrypted, key_epoch, {payload_version_expression}
                     FROM mcp_log WHERE key_epoch = ?1 ORDER BY id ASC LIMIT 1"
                )
            } else {
                format!(
                    "SELECT id, arguments_encrypted, result_encrypted, 0, {payload_version_expression}
                     FROM mcp_log ORDER BY id ASC LIMIT 1"
                )
            };
            let mut statement = conn.prepare(&sample_sql)?;
            if columns.contains("key_epoch") {
                let epoch_sql = i64::try_from(*epoch)
                    .context("MCP audit key epoch exceeds SQLite integer range")?;
                let row = statement.query_row([epoch_sql], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })?;
                validate_mcp_audit_ciphertext_row(row.0, &row.1, &row.2, row.3, row.4, &keyring)?;
            } else {
                let row = statement.query_row([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })?;
                validate_mcp_audit_ciphertext_row(row.0, &row.1, &row.2, row.3, row.4, &keyring)?;
            }
        }
        Ok(inspection)
    }

    /// Test/fixture-only constructor for the historical deterministic key.
    /// Product code must hydrate a random keychain epoch and call
    /// `with_key_materials`.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let config = AuditKeyConfig::default();
        Self::with_config(db_path, config)
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_config(db_path: impl Into<PathBuf>, config: AuditKeyConfig) -> Self {
        Self::with_keyring(db_path, vec![config])
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_keyring(db_path: impl Into<PathBuf>, configs: Vec<AuditKeyConfig>) -> Self {
        Self::with_legacy_keyring_unchecked(db_path, configs)
    }

    #[cfg(any(test, feature = "test-utils"))]
    fn with_legacy_keyring_unchecked(
        db_path: impl Into<PathBuf>,
        configs: Vec<AuditKeyConfig>,
    ) -> Self {
        let path = db_path.into();
        let mut configs = if configs.is_empty() {
            vec![AuditKeyConfig::default()]
        } else {
            configs
        };
        configs.sort_by_key(|config| config.epoch);
        assert!(
            configs.windows(2).all(|pair| pair[0].epoch < pair[1].epoch),
            "legacy MCP audit fixture keyring contains duplicate epochs"
        );
        let config = configs.last().cloned().unwrap_or_default();
        let key = Self::derive_key(&config);
        let keyring = configs
            .iter()
            .map(|config| (config.epoch, Self::derive_key(config)))
            .collect();
        let store = Self {
            db_path: path,
            read_only: false,
            unavailable_reason: None,
            key,
            key_config: config,
            keyring,
            key_configs: configs,
        };
        let _ = store.init_tables();
        store
    }

    pub fn with_key_materials(
        db_path: impl Into<PathBuf>,
        materials: Vec<AuditKeyMaterial>,
    ) -> Result<Self> {
        if materials.is_empty() {
            anyhow::bail!("MCP audit key material is empty");
        }
        let keyring = mcp_audit_material_keyring(&materials)?;
        let active = materials.last().cloned().expect("non-empty key materials");
        if active.config.mode != KeyMode::Keychain || active.config.key_ref.is_none() {
            anyhow::bail!(
                "active MCP audit key must be random keychain material; legacy modes are read-only migration keys"
            );
        }
        let db_path = db_path.into();
        Self::preflight_existing_database_key_materials(&db_path, &materials)?;
        let key_configs = materials
            .iter()
            .map(|material| material.config.clone())
            .collect::<Vec<_>>();
        let store = Self {
            db_path,
            read_only: false,
            unavailable_reason: None,
            key: active.key,
            key_config: active.config,
            keyring,
            key_configs,
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn open_read_only_existing_with_key_materials(
        db_path: impl Into<PathBuf>,
        materials: Vec<AuditKeyMaterial>,
    ) -> Result<Self> {
        if materials.is_empty() {
            anyhow::bail!("MCP audit key material is empty");
        }
        let keyring = mcp_audit_material_keyring(&materials)?;
        let active = materials.last().cloned().expect("non-empty key materials");
        if active.config.mode != KeyMode::Keychain || active.config.key_ref.is_none() {
            anyhow::bail!("active MCP audit key must be random keychain material");
        }
        let db_path = db_path.into();
        Self::preflight_existing_database_key_materials(&db_path, &materials)?;
        crate::sqlite_migration::open_existing_read_only(
            &db_path,
            "mcp_audit_store",
            &["mcp_log"],
        )?;
        Ok(Self {
            db_path,
            read_only: true,
            unavailable_reason: None,
            key: active.key,
            key_config: active.config,
            keyring,
            key_configs: materials
                .into_iter()
                .map(|material| material.config)
                .collect(),
        })
    }

    pub fn unavailable_sentinel(reason: impl Into<String>) -> Self {
        Self {
            db_path: PathBuf::new(),
            read_only: true,
            unavailable_reason: Some(reason.into()),
            key: [0; 32],
            key_config: AuditKeyConfig::default(),
            keyring: HashMap::new(),
            key_configs: Vec::new(),
        }
    }

    /// Hydrate a historical deterministic key solely so existing audit rows
    /// can be read and re-encrypted during migration. `with_key_materials`
    /// rejects this material as the active write epoch.
    pub fn legacy_read_only_key_material(config: AuditKeyConfig) -> Result<AuditKeyMaterial> {
        if config.mode == KeyMode::Keychain {
            anyhow::bail!("keychain audit config requires externally hydrated key material");
        }
        Ok(AuditKeyMaterial {
            key: Self::derive_key(&config),
            config,
        })
    }

    /// Derive the 32-byte AES key from the current key configuration.
    fn derive_key(config: &AuditKeyConfig) -> [u8; 32] {
        match config.mode {
            KeyMode::Derived => {
                let mut context = DigestContext::new(&SHA256);
                context.update(b"openlife_mcp_log_secret_v1");
                let digest = context.finish();
                let mut key_arr = [0u8; 32];
                key_arr.copy_from_slice(digest.as_ref());
                key_arr
            }
            KeyMode::Passphrase => {
                let salt = config
                    .salt_b64
                    .as_ref()
                    .and_then(|s| general_purpose::STANDARD.decode(s).ok())
                    .unwrap_or_else(|| b"openlife_default_salt".to_vec());
                let mut context = DigestContext::new(&SHA256);
                context.update(&salt);
                context.update(b"openlife_mcp_log_passphrase_v1");
                let digest = context.finish();
                let mut key_arr = [0u8; 32];
                key_arr.copy_from_slice(digest.as_ref());
                key_arr
            }
            KeyMode::Env => {
                let env_key = config
                    .env_var
                    .as_ref()
                    .and_then(|var| std::env::var(var).ok())
                    .unwrap_or_default();
                let mut context = DigestContext::new(&SHA256);
                context.update(env_key.as_bytes());
                context.update(b"openlife_mcp_log_env_v1");
                let digest = context.finish();
                let mut key_arr = [0u8; 32];
                key_arr.copy_from_slice(digest.as_ref());
                key_arr
            }
            KeyMode::Keychain => {
                panic!("keychain audit keys must be supplied as hydrated key material")
            }
        }
    }

    /// Rotate to a new key configuration for future writes.
    ///
    /// Existing entries remain readable while this store is initialized with
    /// the full keyring returned by `key_configs`.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn rotate_key(&mut self, new_config: AuditKeyConfig) {
        assert_ne!(
            new_config.mode,
            KeyMode::Keychain,
            "keychain rotation requires rotate_key_material"
        );
        let new_key = Self::derive_key(&new_config);
        self.key = new_key;
        self.key_config = new_config;
        self.keyring.insert(self.key_config.epoch, new_key);
        self.key_configs
            .retain(|config| config.epoch != self.key_config.epoch);
        self.key_configs.push(self.key_config.clone());
        self.key_configs.sort_by_key(|config| config.epoch);
    }

    pub fn rotate_key_material(&mut self, material: AuditKeyMaterial) -> Result<()> {
        if material.config.epoch <= self.key_config.epoch {
            anyhow::bail!("MCP audit key epoch must increase monotonically");
        }
        if material.config.mode != KeyMode::Keychain || material.config.key_ref.is_none() {
            anyhow::bail!("MCP audit rotation requires a keychain reference");
        }
        self.key = material.key;
        self.key_config = material.config.clone();
        self.keyring.insert(material.config.epoch, material.key);
        self.key_configs.push(material.config);
        self.key_configs.sort_by_key(|config| config.epoch);
        Ok(())
    }

    pub fn key_config(&self) -> &AuditKeyConfig {
        &self.key_config
    }

    pub fn key_configs(&self) -> &[AuditKeyConfig] {
        &self.key_configs
    }

    /// Export decrypted audit logs for the given time range.
    pub fn export_logs(&self, days: i64) -> Result<AuditExport> {
        let entries = self.list_logs(10000)?;
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let filtered: Vec<_> = entries
            .into_iter()
            .filter(|e| {
                chrono::DateTime::parse_from_rfc3339(&e.created_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc) >= cutoff)
                    .unwrap_or(true)
            })
            .map(|e| ExportedAuditEntry {
                id: e.id,
                tool_name: e.tool_name,
                arguments: e.arguments,
                result: e.result,
                success: e.success,
                pii_found: e.pii_found,
                created_at: e.created_at,
            })
            .collect();
        Ok(AuditExport {
            exported_at: chrono::Utc::now().to_rfc3339(),
            entry_count: filtered.len(),
            days,
            entries: filtered,
        })
    }

    /// Cleanup strategy: remove logs older than retention_days and return count removed.
    pub fn cleanup(&self, retention_days: i64) -> Result<usize> {
        self.clear_old_logs(retention_days)
    }

    fn conn(&self) -> Result<Connection> {
        if let Some(reason) = &self.unavailable_reason {
            anyhow::bail!("mcp_audit_store_unavailable:{reason}");
        }
        if self.read_only {
            crate::sqlite_migration::open_existing_read_only(
                &self.db_path,
                "mcp_audit_store",
                &["mcp_log"],
            )
        } else {
            Connection::open(&self.db_path).context("open mcp audit db")
        }
    }

    fn init_tables(&self) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS mcp_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_name TEXT NOT NULL,
                arguments_encrypted TEXT NOT NULL,
                result_encrypted TEXT NOT NULL,
                success INTEGER NOT NULL,
                pii_found INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                key_epoch INTEGER NOT NULL DEFAULT 0,
                payload_minimized_version INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "mcp_log",
            "key_epoch",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "mcp_log",
            "payload_minimized_version",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        crate::sqlite_migration::record_schema_version(&tx, "mcp_audit_store", 4)?;
        tx.commit()?;
        self.migrate_legacy_payloads()?;
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    fn encrypt(&self, plaintext: &str) -> Result<String> {
        encrypt_mcp_audit_ciphertext(plaintext, &self.key, &[])
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn d068_insert_legacy_payload_fixture_for_test(
        &self,
        tool_name: &str,
        arguments: &Value,
        result: &str,
    ) -> Result<i64> {
        let arguments_encrypted = self.encrypt(
            &serde_json::to_string(arguments)
                .context("serialize legacy audit fixture arguments")?,
        )?;
        let result_encrypted = self.encrypt(result)?;
        let key_epoch = i64::try_from(self.key_config.epoch)
            .context("MCP audit key epoch exceeds the SQLite integer range")?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO mcp_log (
                tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                created_at, key_epoch, payload_minimized_version
             ) VALUES (?1, ?2, ?3, 1, 1, ?4, ?5, 0)",
            params![
                tool_name,
                arguments_encrypted,
                result_encrypted,
                chrono::Utc::now().to_rfc3339(),
                key_epoch,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn d068_flip_payload_version_to_current_for_test(&self, id: i64) -> Result<()> {
        self.conn()?.execute(
            "UPDATE mcp_log
             SET payload_minimized_version = ?1
             WHERE id = ?2",
            params![MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION, id],
        )?;
        Ok(())
    }

    fn encrypt_current_receipt(&self, receipt: &AuditPayloadReceiptV2) -> Result<String> {
        receipt.validate(receipt.kind)?;
        let plaintext = receipt.canonical_json()?;
        let aad = mcp_audit_payload_aad(
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            receipt.kind,
            self.key_config.epoch,
        );
        encrypt_mcp_audit_ciphertext(&plaintext, &self.key, &aad)
    }

    #[cfg(test)]
    fn encrypt_current_plaintext_for_test(
        &self,
        plaintext: &str,
        kind: AuditPayloadKind,
    ) -> Result<String> {
        let aad = mcp_audit_payload_aad(
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            kind,
            self.key_config.epoch,
        );
        encrypt_mcp_audit_ciphertext(plaintext, &self.key, &aad)
    }

    fn decrypt_legacy_for_epoch(&self, combined_b64: &str, key_epoch: u64) -> Result<String> {
        let key = self
            .keyring
            .get(&key_epoch)
            .ok_or_else(|| anyhow::anyhow!("MCP audit key epoch {key_epoch} is unavailable"))?;
        decrypt_mcp_audit_ciphertext(combined_b64, key, &[])
    }

    fn decrypt_current_receipt_for_epoch(
        &self,
        combined_b64: &str,
        key_epoch: u64,
        payload_version: i64,
        expected_kind: AuditPayloadKind,
    ) -> Result<AuditPayloadReceiptV2> {
        let key = self
            .keyring
            .get(&key_epoch)
            .ok_or_else(|| anyhow::anyhow!("MCP audit key epoch {key_epoch} is unavailable"))?;
        decrypt_current_audit_receipt(combined_b64, key, key_epoch, payload_version, expected_kind)
    }

    fn migrate_legacy_payloads(&self) -> Result<()> {
        let mut conn = self.conn()?;
        let rows = {
            let mut statement = conn.prepare(
                "SELECT id, arguments_encrypted, result_encrypted, key_epoch,
                        payload_minimized_version
                 FROM mcp_log
                 WHERE payload_minimized_version < ?1",
            )?;
            let rows = statement
                .query_map([MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        if rows.is_empty() {
            return Ok(());
        }

        let mut migrated = Vec::with_capacity(rows.len());
        for (id, arguments_encrypted, result_encrypted, key_epoch, payload_version) in rows {
            let key_epoch = u64::try_from(key_epoch).with_context(|| {
                format!("legacy MCP audit row {id} contains a negative key epoch")
            })?;
            let arguments_plaintext = self
                .decrypt_legacy_for_epoch(&arguments_encrypted, key_epoch)
                .with_context(|| {
                    format!("decrypt legacy MCP audit arguments for row {id} epoch {key_epoch}")
                })?;
            let result_plaintext = self
                .decrypt_legacy_for_epoch(&result_encrypted, key_epoch)
                .with_context(|| {
                    format!("decrypt legacy MCP audit result for row {id} epoch {key_epoch}")
                })?;
            let (arguments_receipt, result_receipt) = match payload_version {
                0 => {
                    let arguments_receipt = serde_json::from_str::<Value>(&arguments_plaintext)
                        .ok()
                        .map(|value| audit_arguments_receipt(&value))
                        .transpose()?
                        .unwrap_or_else(|| {
                            audit_payload_receipt(
                                AuditPayloadKind::Arguments,
                                AuditPayloadValueType::UnparseableLegacy,
                                arguments_plaintext.as_bytes(),
                            )
                        });
                    (arguments_receipt, audit_result_receipt(&result_plaintext))
                }
                MCP_AUDIT_LEGACY_MINIMIZED_VERSION => (
                    decode_legacy_minimized_receipt(
                        &arguments_plaintext,
                        AuditPayloadKind::Arguments,
                    )
                    .with_context(|| {
                        format!("validate legacy minimized MCP audit arguments for row {id}")
                    })?,
                    decode_legacy_minimized_receipt(&result_plaintext, AuditPayloadKind::Result)
                        .with_context(|| {
                            format!("validate legacy minimized MCP audit result for row {id}")
                        })?,
                ),
                other => anyhow::bail!(
                    "legacy MCP audit row {id} has unsupported payload version {other}"
                ),
            };
            migrated.push((
                id,
                self.encrypt_current_receipt(&arguments_receipt)?,
                self.encrypt_current_receipt(&result_receipt)?,
            ));
        }

        let transaction = conn.transaction()?;
        for (id, arguments_encrypted, result_encrypted) in migrated {
            transaction.execute(
                "UPDATE mcp_log
                 SET arguments_encrypted = ?1,
                     result_encrypted = ?2,
                     key_epoch = ?3,
                     payload_minimized_version = ?4
                 WHERE id = ?5",
                params![
                    arguments_encrypted,
                    result_encrypted,
                    self.key_config.epoch as i64,
                    MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
                    id,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn insert_log(
        &self,
        tool_name: &str,
        arguments: &Value,
        result: &str,
        success: bool,
        pii_found: bool,
    ) -> Result<i64> {
        let conn = self.conn()?;
        let args_enc = self.encrypt_current_receipt(&audit_arguments_receipt(arguments)?)?;
        let res_enc = self.encrypt_current_receipt(&audit_result_receipt(result))?;
        let created_at = chrono::Utc::now().to_rfc3339();
        let key_epoch = i64::try_from(self.key_config.epoch)
            .context("MCP audit key epoch exceeds the SQLite integer range")?;
        conn.execute(
            "INSERT INTO mcp_log (
                tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                created_at, key_epoch, payload_minimized_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                tool_name,
                args_enc,
                res_enc,
                success as i32,
                pii_found as i32,
                created_at,
                key_epoch,
                MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_logs(&self, limit: usize) -> Result<Vec<McpLogEntry>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                    created_at, key_epoch, payload_minimized_version
             FROM mcp_log
             ORDER BY id DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| {
            let id: i64 = row.get(0)?;
            let tool_name: String = row.get(1)?;
            let args_enc: String = row.get(2)?;
            let res_enc: String = row.get(3)?;
            let success: i32 = row.get(4)?;
            let pii_found: i32 = row.get(5)?;
            let created_at: String = row.get(6)?;
            let key_epoch: i64 = row.get(7)?;
            let payload_version: i64 = row.get(8)?;
            Ok((
                id,
                tool_name,
                args_enc,
                res_enc,
                success != 0,
                pii_found != 0,
                created_at,
                key_epoch,
                payload_version,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (
                id,
                tool_name,
                args_enc,
                res_enc,
                success,
                pii_found,
                created_at,
                key_epoch,
                payload_version,
            ) = r?;
            let key_epoch = u64::try_from(key_epoch)
                .with_context(|| format!("MCP audit row {id} contains a negative key epoch"))?;
            let arguments = self
                .decrypt_current_receipt_for_epoch(
                    &args_enc,
                    key_epoch,
                    payload_version,
                    AuditPayloadKind::Arguments,
                )
                .with_context(|| {
                    format!("decrypt MCP audit arguments for row {id} epoch {key_epoch}")
                })?
                .canonical_json()?;
            let result = self
                .decrypt_current_receipt_for_epoch(
                    &res_enc,
                    key_epoch,
                    payload_version,
                    AuditPayloadKind::Result,
                )
                .with_context(|| {
                    format!("decrypt MCP audit result for row {id} epoch {key_epoch}")
                })?
                .canonical_json()?;
            out.push(McpLogEntry {
                id,
                tool_name,
                arguments,
                result,
                success,
                pii_found,
                created_at,
            });
        }
        Ok(out)
    }

    pub fn clear_old_logs(&self, days: i64) -> Result<usize> {
        let conn = self.conn()?;
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
        let rows = conn.execute(
            "DELETE FROM mcp_log WHERE created_at < ?1",
            [cutoff.to_rfc3339()],
        )?;
        Ok(rows)
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl Default for McpAuditStore {
    fn default() -> Self {
        let data_dir = openlife_default_data_dir();
        Self::new(data_dir.join("mcp_audit.db"))
    }
}

#[cfg(any(test, feature = "test-utils"))]
fn openlife_default_data_dir() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("OPENLIFE_DATA_DIR") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return std::path::PathBuf::from(trimmed);
        }
    }
    let profile = std::env::var("OPENLIFE_PROFILE")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "release".to_string());
    let app_dir_name = match profile.as_str() {
        "dev" => "ai.openlife.desktop.dev",
        "qa" => "ai.openlife.desktop.qa",
        _ => "ai.openlife.desktop",
    };
    dirs::data_dir()
        .map(|d| d.join(app_dir_name))
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap()
                .join(format!(".{}", app_dir_name))
        })
}

/// Shareable handle.
pub type SharedMcpAuditStore = Arc<Mutex<McpAuditStore>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn durable_database_bytes(path: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
        [
            path.to_path_buf(),
            PathBuf::from(format!("{}-wal", path.display())),
            PathBuf::from(format!("{}-shm", path.display())),
            PathBuf::from(format!("{}-journal", path.display())),
        ]
        .into_iter()
        .map(|candidate| {
            let bytes = match std::fs::read(&candidate) {
                Ok(bytes) => Some(bytes),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => panic!("read {}: {error}", candidate.display()),
            };
            (candidate, bytes)
        })
        .collect()
    }

    fn flip_authenticated_ciphertext(ciphertext: &str) -> String {
        let mut bytes = general_purpose::STANDARD.decode(ciphertext).unwrap();
        assert!(bytes.len() > 28);
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        general_purpose::STANDARD.encode(bytes)
    }

    fn keychain_material(epoch: u64, key: [u8; 32]) -> AuditKeyMaterial {
        AuditKeyMaterial {
            config: AuditKeyConfig {
                mode: KeyMode::Keychain,
                salt_b64: None,
                env_var: None,
                key_ref: Some(format!("test-mcp-audit-key-{epoch}")),
                epoch,
                created_at: "2026-07-16T00:00:00Z".into(),
            },
            key,
        }
    }

    #[test]
    fn legacy_key_material_cannot_become_the_active_product_write_epoch() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = McpAuditStore::legacy_read_only_key_material(AuditKeyConfig::default())
            .expect("legacy migration material");

        let error =
            match McpAuditStore::with_key_materials(dir.path().join("audit.db"), vec![legacy]) {
                Ok(_) => panic!("legacy key modes must never authorize new product writes"),
                Err(error) => error,
            };

        assert!(error
            .to_string()
            .contains("legacy modes are read-only migration keys"));
    }

    #[test]
    fn audit_store_key_rotation_keeps_old_logs_readable_with_keyring() {
        const PRIVATE_ARGUMENT: &str = "THERAPY-CASE-74291-ORCHID";
        const PRIVATE_RESULT: &str = "ACCOUNT-NOTE-55318-CEDAR";
        let dir = tempfile::tempdir().unwrap();
        let mut store = McpAuditStore::new(dir.path().join("audit.db"));
        store
            .insert_log(
                "test_tool",
                &serde_json::json!({"x": PRIVATE_ARGUMENT}),
                PRIVATE_RESULT,
                true,
                true,
            )
            .unwrap();

        let old_config = store.key_config().clone();
        let new_config = AuditKeyConfig {
            mode: KeyMode::Passphrase,
            salt_b64: Some(general_purpose::STANDARD.encode(b"newsalt123456789")),
            epoch: old_config.epoch + 1,
            ..Default::default()
        };
        store.rotate_key(new_config.clone());

        // New writes use new key
        store
            .insert_log(
                "test_tool2",
                &serde_json::json!({"y": 2}),
                "done",
                true,
                false,
            )
            .unwrap();

        // All logs still readable
        let logs = store.list_logs(10).unwrap();
        assert_eq!(logs.len(), 2);
        let serialized = serde_json::to_string(&logs).unwrap();
        assert!(!serialized.contains(PRIVATE_ARGUMENT));
        assert!(!serialized.contains(PRIVATE_RESULT));
        assert!(serialized.contains("payloadStored"));
        assert!(serialized.contains("sha256:"));

        let restarted =
            McpAuditStore::with_keyring(dir.path().join("audit.db"), store.key_configs().to_vec());
        let restarted_logs = restarted.list_logs(10).unwrap();
        let restarted_serialized = serde_json::to_string(&restarted_logs).unwrap();
        assert!(!restarted_serialized.contains(PRIVATE_ARGUMENT));
        assert!(!restarted_serialized.contains(PRIVATE_RESULT));
        assert!(restarted_serialized.contains("payloadStored"));
    }

    #[test]
    fn keychain_epochs_remain_readable_after_product_store_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let first = keychain_material(21, [0x21; 32]);
        let second = keychain_material(22, [0x22; 32]);
        let mut store = McpAuditStore::with_key_materials(&path, vec![first.clone()]).unwrap();
        store
            .insert_log(
                "first-epoch",
                &serde_json::json!({"value": "first"}),
                "first-result",
                true,
                false,
            )
            .unwrap();
        store.rotate_key_material(second.clone()).unwrap();
        store
            .insert_log(
                "second-epoch",
                &serde_json::json!({"value": "second"}),
                "second-result",
                true,
                false,
            )
            .unwrap();
        drop(store);

        let restarted = McpAuditStore::with_key_materials(&path, vec![first, second]).unwrap();
        let logs = restarted.list_logs(10).unwrap();
        let epochs = restarted
            .conn()
            .unwrap()
            .prepare("SELECT key_epoch FROM mcp_log ORDER BY id ASC")
            .unwrap()
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].tool_name, "second-epoch");
        assert_eq!(logs[1].tool_name, "first-epoch");
        assert_eq!(epochs, vec![21, 22]);
    }

    #[test]
    fn audit_store_export_and_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let store = McpAuditStore::new(dir.path().join("audit.db"));
        store
            .insert_log("tool_a", &serde_json::json!({}), "result", true, false)
            .unwrap();

        let export = store.export_logs(30).unwrap();
        assert_eq!(export.entry_count, 1);
        assert_eq!(export.entries[0].tool_name, "tool_a");

        let cleaned = store.cleanup(0).unwrap();
        assert_eq!(cleaned, 1);
        let logs = store.list_logs(10).unwrap();
        assert!(logs.is_empty());
    }

    #[test]
    fn d068_current_receipts_remain_exactly_readable_and_exportable() {
        const PRIVATE_ARGUMENT: &str = "D068-CURRENT-MEDICAL-ARGUMENT";
        const PRIVATE_RESULT: &str = "D068-CURRENT-FINANCIAL-RESULT";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let material = keychain_material(3, [0x23; 32]);
        let arguments = serde_json::json!({"private": PRIVATE_ARGUMENT, "bounded": true});
        let expected_arguments = audit_arguments_receipt(&arguments)
            .unwrap()
            .canonical_json()
            .unwrap();
        let expected_result = audit_result_receipt(PRIVATE_RESULT)
            .canonical_json()
            .unwrap();
        let store = McpAuditStore::with_key_materials(&path, vec![material.clone()]).unwrap();
        store
            .insert_log("d068-current", &arguments, PRIVATE_RESULT, true, true)
            .unwrap();
        drop(store);

        let restarted = McpAuditStore::with_key_materials(&path, vec![material]).unwrap();
        let logs = restarted.list_logs(10).unwrap();
        let export = restarted.export_logs(30).unwrap();

        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].arguments, expected_arguments);
        assert_eq!(logs[0].result, expected_result);
        assert_eq!(export.entry_count, 1);
        assert_eq!(export.entries[0].arguments, logs[0].arguments);
        assert_eq!(export.entries[0].result, logs[0].result);
        let serialized = serde_json::to_string(&(logs, export)).unwrap();
        assert!(!serialized.contains(PRIVATE_ARGUMENT));
        assert!(!serialized.contains(PRIVATE_RESULT));
    }

    #[test]
    fn legacy_reversible_payloads_are_migrated_to_receipts_on_restart() {
        const LEGACY_ARGUMENT: &str = "MEDICAL-NOTE-31057-MAPLE";
        const LEGACY_RESULT: &str = "FINANCE-NOTE-88241-ASH";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        let arguments_encrypted = store
            .encrypt(&serde_json::json!({ "note": LEGACY_ARGUMENT }).to_string())
            .unwrap();
        let result_encrypted = store.encrypt(LEGACY_RESULT).unwrap();
        let configs = store.key_configs().to_vec();
        store
            .conn()
            .unwrap()
            .execute(
                "INSERT INTO mcp_log (
                    tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                    created_at, key_epoch, payload_minimized_version
                 ) VALUES (?1, ?2, ?3, 1, 1, ?4, ?5, 0)",
                params![
                    "legacy_tool",
                    arguments_encrypted,
                    result_encrypted,
                    chrono::Utc::now().to_rfc3339(),
                    store.key_config().epoch as i64,
                ],
            )
            .unwrap();
        drop(store);

        let restarted = McpAuditStore::with_keyring(&path, configs);
        let serialized = serde_json::to_string(&restarted.list_logs(10).unwrap()).unwrap();

        assert!(!serialized.contains(LEGACY_ARGUMENT));
        assert!(!serialized.contains(LEGACY_RESULT));
        assert!(serialized.contains("payloadStored"));
        assert!(serialized.contains("sha256:"));
        let version: i64 = restarted
            .conn()
            .unwrap()
            .query_row(
                "SELECT payload_minimized_version FROM mcp_log WHERE tool_name = 'legacy_tool'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION);
    }

    #[test]
    fn legal_v1_minimized_receipts_migrate_to_authenticated_v2() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let material = keychain_material(5, [0x35; 32]);
        let store = McpAuditStore::with_key_materials(&path, vec![material.clone()]).unwrap();
        let current_arguments =
            audit_arguments_receipt(&serde_json::json!({"legacy": "receipt-only"})).unwrap();
        let current_result = audit_result_receipt("legacy-result");
        let legacy_arguments = AuditPayloadReceiptV1 {
            kind: current_arguments.kind,
            payload_stored: current_arguments.payload_stored,
            value_type: current_arguments.value_type,
            bytes: current_arguments.bytes,
            digest: current_arguments.digest,
        };
        let legacy_result = AuditPayloadReceiptV1 {
            kind: current_result.kind,
            payload_stored: current_result.payload_stored,
            value_type: current_result.value_type,
            bytes: current_result.bytes,
            digest: current_result.digest,
        };
        let arguments_encrypted = store
            .encrypt(&serde_json::to_string(&legacy_arguments).unwrap())
            .unwrap();
        let result_encrypted = store
            .encrypt(&serde_json::to_string(&legacy_result).unwrap())
            .unwrap();
        store
            .conn()
            .unwrap()
            .execute(
                "INSERT INTO mcp_log (
                    tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                    created_at, key_epoch, payload_minimized_version
                 ) VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?6)",
                params![
                    "legacy-v1",
                    arguments_encrypted,
                    result_encrypted,
                    chrono::Utc::now().to_rfc3339(),
                    5_i64,
                    MCP_AUDIT_LEGACY_MINIMIZED_VERSION,
                ],
            )
            .unwrap();
        drop(store);

        let restarted = McpAuditStore::with_key_materials(&path, vec![material]).unwrap();
        let logs = restarted.list_logs(10).unwrap();
        assert_eq!(logs.len(), 1);
        assert!(logs[0].arguments.contains("\"schemaVersion\":2"));
        assert!(logs[0].result.contains("\"schemaVersion\":2"));
        let version: i64 = restarted
            .conn()
            .unwrap()
            .query_row(
                "SELECT payload_minimized_version FROM mcp_log WHERE tool_name = 'legacy-v1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION);
    }

    #[test]
    fn missing_epoch_never_falls_back_to_active_audit_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store =
            McpAuditStore::with_key_materials(&path, vec![keychain_material(7, [0x31; 32])])
                .unwrap();
        let id = store
            .insert_log(
                "epoch-test",
                &serde_json::json!({"x": 1}),
                "ok",
                true,
                false,
            )
            .unwrap();
        store
            .conn()
            .unwrap()
            .execute("UPDATE mcp_log SET key_epoch = 8 WHERE id = ?1", [id])
            .unwrap();

        let error = store.list_logs(10).unwrap_err();
        assert!(format!("{error:#}").contains("key epoch 8 is unavailable"));
    }

    #[test]
    fn wrong_key_preflight_is_read_only_and_preserves_database_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let config = keychain_material(9, [0x41; 32]);
        let store = McpAuditStore::with_key_materials(&path, vec![config.clone()]).unwrap();
        store
            .insert_log(
                "wrong-key-test",
                &serde_json::json!({"secret": "receipt only"}),
                "ok",
                true,
                false,
            )
            .unwrap();
        drop(store);
        let before = durable_database_bytes(&path);
        let wrong = keychain_material(9, [0x42; 32]);

        let error = match McpAuditStore::with_key_materials(&path, vec![wrong]) {
            Ok(_) => panic!("wrong key material must fail before writable open"),
            Err(error) => error,
        };

        let detail = format!("{error:#}");
        assert!(detail.contains("MCP audit"));
        assert!(detail.contains("decrypt failed"));
        assert!(!is_payload_integrity_failure(&error));
        assert_eq!(durable_database_bytes(&path), before);
    }

    #[test]
    fn negative_epoch_fails_without_rewriting_ciphertext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store =
            McpAuditStore::with_key_materials(&path, vec![keychain_material(11, [0x51; 32])])
                .unwrap();
        let id = store
            .insert_log(
                "negative-epoch-test",
                &serde_json::json!({"x": 1}),
                "ok",
                true,
                false,
            )
            .unwrap();
        let conn = store.conn().unwrap();
        conn.execute("UPDATE mcp_log SET key_epoch = -1 WHERE id = ?1", [id])
            .unwrap();
        let before: (String, String, i64) = conn
            .query_row(
                "SELECT arguments_encrypted, result_encrypted, payload_minimized_version
                 FROM mcp_log WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        drop(conn);

        assert!(store
            .list_logs(10)
            .unwrap_err()
            .to_string()
            .contains("negative key epoch"));
        let after: (String, String, i64) = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT arguments_encrypted, result_encrypted, payload_minimized_version
                 FROM mcp_log WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn key_epoch_outside_sqlite_range_is_rejected_before_database_creation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let material = keychain_material(i64::MAX as u64 + 1, [0x61; 32]);

        let error = match McpAuditStore::with_key_materials(&path, vec![material]) {
            Ok(_) => panic!("out-of-range epochs must not create a writable audit store"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("SQLite integer range"));
        assert!(!path.exists());
    }

    #[test]
    fn d068_plaintext_version_flip_cannot_expose_legacy_sensitive_payload() {
        const LEGACY_ARGUMENT: &str = "D068-RAW-MEDICAL-ARGUMENT";
        const LEGACY_RESULT: &str = "D068-RAW-FINANCIAL-RESULT";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let material = keychain_material(31, [0x71; 32]);
        let store = McpAuditStore::with_key_materials(&path, vec![material.clone()]).unwrap();
        let arguments_encrypted = store
            .encrypt(&serde_json::json!({"secret": LEGACY_ARGUMENT}).to_string())
            .unwrap();
        let result_encrypted = store.encrypt(LEGACY_RESULT).unwrap();
        store
            .conn()
            .unwrap()
            .execute(
                "INSERT INTO mcp_log (
                    tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                    created_at, key_epoch, payload_minimized_version
                 ) VALUES (?1, ?2, ?3, 1, 1, ?4, ?5, 0)",
                params![
                    "d068-version-flip",
                    arguments_encrypted,
                    result_encrypted,
                    chrono::Utc::now().to_rfc3339(),
                    31_i64,
                ],
            )
            .unwrap();
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE mcp_log SET payload_minimized_version = ?1
                 WHERE tool_name = 'd068-version-flip'",
                [MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION],
            )
            .unwrap();
        drop(store);
        let before = durable_database_bytes(&path);

        let error = match McpAuditStore::with_key_materials(&path, vec![material]) {
            Ok(store) => {
                let exposed = serde_json::to_string(&store.list_logs(10).unwrap()).unwrap();
                panic!(
                    "plaintext version flip must not authorize current receipt decoding: {exposed}"
                );
            }
            Err(error) => error,
        };

        assert!(is_payload_integrity_failure(&error));
        assert!(format!("{error:#}").contains("authenticated_format_binding_mismatch"));
        assert_eq!(durable_database_bytes(&path), before);
    }

    #[test]
    fn d068_current_receipt_schema_rejects_unknown_fields_for_list_and_export() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let material = keychain_material(41, [0x81; 32]);
        let store = McpAuditStore::with_key_materials(&path, vec![material.clone()]).unwrap();
        let id = store
            .insert_log(
                "d068-schema",
                &serde_json::json!({"safe": true}),
                "ok",
                true,
                false,
            )
            .unwrap();
        let forged_receipt = store
            .encrypt_current_plaintext_for_test(
                &serde_json::json!({
                    "schemaVersion": MCP_AUDIT_RECEIPT_SCHEMA_VERSION,
                    "kind": "arguments",
                    "payloadStored": false,
                    "valueType": "object",
                    "bytes": 13,
                    "digest": "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    "unexpectedRawPayload": "must-never-be-returned",
                })
                .to_string(),
                AuditPayloadKind::Arguments,
            )
            .unwrap();
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE mcp_log SET arguments_encrypted = ?1 WHERE id = ?2",
                params![forged_receipt, id],
            )
            .unwrap();
        let before = durable_database_bytes(&path);

        let list_error = store
            .list_logs(10)
            .expect_err("unknown current receipt fields must fail closed");
        let export_error = store
            .export_logs(30)
            .expect_err("export must reuse the same strict current receipt decoder");

        assert!(format!("{list_error:#}").contains("MCP audit"));
        assert!(format!("{export_error:#}").contains("MCP audit"));
        drop(store);
        let restart_error = match McpAuditStore::with_key_materials(&path, vec![material]) {
            Ok(_) => panic!("invalid current receipt schema must fail before writable restart"),
            Err(error) => error,
        };
        assert!(format!("{restart_error:#}").contains("MCP audit"));
        assert_eq!(durable_database_bytes(&path), before);
    }

    #[test]
    fn d068_ciphertext_corruption_and_column_swap_fail_closed_without_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store =
            McpAuditStore::with_key_materials(&path, vec![keychain_material(45, [0x85; 32])])
                .unwrap();
        let id = store
            .insert_log(
                "d068-envelope",
                &serde_json::json!({"safe": true}),
                "ok",
                true,
                false,
            )
            .unwrap();
        let (valid_arguments, valid_result): (String, String) = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT arguments_encrypted, result_encrypted FROM mcp_log WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let cases = [
            (
                "arguments_bit_flip",
                flip_authenticated_ciphertext(&valid_arguments),
                valid_result.clone(),
            ),
            (
                "result_bit_flip",
                valid_arguments.clone(),
                flip_authenticated_ciphertext(&valid_result),
            ),
            (
                "role_column_swap",
                valid_result.clone(),
                valid_arguments.clone(),
            ),
        ];

        for (label, arguments_encrypted, result_encrypted) in cases {
            store
                .conn()
                .unwrap()
                .execute(
                    "UPDATE mcp_log
                     SET arguments_encrypted = ?1, result_encrypted = ?2
                     WHERE id = ?3",
                    params![arguments_encrypted, result_encrypted, id],
                )
                .unwrap();
            let before = durable_database_bytes(&path);
            assert!(store.list_logs(10).is_err(), "label={label}");
            assert!(store.export_logs(30).is_err(), "label={label}");
            assert_eq!(durable_database_bytes(&path), before, "label={label}");
        }
    }

    #[test]
    fn d068_legacy_migration_authentication_failure_is_atomic_and_zero_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let material = keychain_material(47, [0x87; 32]);
        let store = McpAuditStore::with_key_materials(&path, vec![material.clone()]).unwrap();
        store
            .d068_insert_legacy_payload_fixture_for_test(
                "d068-legacy-valid-first",
                &serde_json::json!({"first": "valid"}),
                "first-valid",
            )
            .unwrap();
        let corrupt_id = store
            .d068_insert_legacy_payload_fixture_for_test(
                "d068-legacy-corrupt-second",
                &serde_json::json!({"second": "corrupt"}),
                "second-corrupt",
            )
            .unwrap();
        let ciphertext: String = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT result_encrypted FROM mcp_log WHERE id = ?1",
                [corrupt_id],
                |row| row.get(0),
            )
            .unwrap();
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE mcp_log SET result_encrypted = ?1 WHERE id = ?2",
                params![flip_authenticated_ciphertext(&ciphertext), corrupt_id],
            )
            .unwrap();
        drop(store);
        let before = durable_database_bytes(&path);

        let error = match McpAuditStore::with_key_materials(&path, vec![material]) {
            Ok(_) => panic!("one corrupt legacy row must abort the entire migration"),
            Err(error) => error,
        };

        assert!(format!("{error:#}").contains("decrypt failed"));
        assert_eq!(durable_database_bytes(&path), before);
    }

    #[test]
    fn d068_strict_current_decoder_rejects_each_receipt_dimension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store =
            McpAuditStore::with_key_materials(&path, vec![keychain_material(51, [0x91; 32])])
                .unwrap();
        let id = store
            .insert_log(
                "d068-dimensions",
                &serde_json::json!({"safe": true}),
                "ok",
                true,
                false,
            )
            .unwrap();
        let (valid_arguments, valid_result): (String, String) = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT arguments_encrypted, result_encrypted FROM mcp_log WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let valid_digest = audit_arguments_receipt(&serde_json::json!({"safe": true}))
            .unwrap()
            .digest;
        let invalid_cases = vec![
            (
                "kind",
                "arguments_encrypted",
                AuditPayloadKind::Arguments,
                serde_json::json!({
                    "schemaVersion": 2,
                    "kind": "result",
                    "payloadStored": false,
                    "valueType": "string",
                    "bytes": 2,
                    "digest": valid_digest.clone(),
                }),
            ),
            (
                "payloadStored",
                "arguments_encrypted",
                AuditPayloadKind::Arguments,
                serde_json::json!({
                    "schemaVersion": 2,
                    "kind": "arguments",
                    "payloadStored": true,
                    "valueType": "object",
                    "bytes": 13,
                    "digest": valid_digest.clone(),
                }),
            ),
            (
                "valueType",
                "arguments_encrypted",
                AuditPayloadKind::Arguments,
                serde_json::json!({
                    "schemaVersion": 2,
                    "kind": "arguments",
                    "payloadStored": false,
                    "valueType": "binary",
                    "bytes": 13,
                    "digest": valid_digest.clone(),
                }),
            ),
            (
                "bytes",
                "arguments_encrypted",
                AuditPayloadKind::Arguments,
                serde_json::json!({
                    "schemaVersion": 2,
                    "kind": "arguments",
                    "payloadStored": false,
                    "valueType": "object",
                    "bytes": "13",
                    "digest": valid_digest.clone(),
                }),
            ),
            (
                "digest",
                "arguments_encrypted",
                AuditPayloadKind::Arguments,
                serde_json::json!({
                    "schemaVersion": 2,
                    "kind": "arguments",
                    "payloadStored": false,
                    "valueType": "object",
                    "bytes": 13,
                    "digest": "sha256:short",
                }),
            ),
            (
                "schemaVersion",
                "arguments_encrypted",
                AuditPayloadKind::Arguments,
                serde_json::json!({
                    "schemaVersion": 99,
                    "kind": "arguments",
                    "payloadStored": false,
                    "valueType": "object",
                    "bytes": 13,
                    "digest": valid_digest.clone(),
                }),
            ),
            (
                "resultValueType",
                "result_encrypted",
                AuditPayloadKind::Result,
                serde_json::json!({
                    "schemaVersion": 2,
                    "kind": "result",
                    "payloadStored": false,
                    "valueType": "object",
                    "bytes": 2,
                    "digest": audit_result_receipt("ok").digest,
                }),
            ),
        ];

        for (label, column, kind, invalid_receipt) in invalid_cases {
            let forged = store
                .encrypt_current_plaintext_for_test(&invalid_receipt.to_string(), kind)
                .unwrap();
            let conn = store.conn().unwrap();
            conn.execute(
                "UPDATE mcp_log
                 SET arguments_encrypted = ?1, result_encrypted = ?2
                 WHERE id = ?3",
                params![valid_arguments, valid_result, id],
            )
            .unwrap();
            conn.execute(
                &format!("UPDATE mcp_log SET {column} = ?1 WHERE id = ?2"),
                params![forged, id],
            )
            .unwrap();
            drop(conn);
            let before = durable_database_bytes(&path);
            let error = store
                .list_logs(10)
                .expect_err("each invalid current receipt dimension must fail closed");
            assert!(
                format!("{error:#}").contains("MCP audit"),
                "label={label}, error={error:#}"
            );
            assert_eq!(durable_database_bytes(&path), before, "label={label}");
        }
    }
}
