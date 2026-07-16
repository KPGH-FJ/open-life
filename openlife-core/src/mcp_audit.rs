use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::mcp_audit_payload_codec::{
    open_authenticated_audit_payload, seal_minimized_audit_receipt_v1, McpAuditPayloadBindingV1,
    McpAuditPayloadRole, MinimizedAuditReceiptV1, MCP_AUDIT_MAX_ENVELOPE_ENCODED_BYTES,
    MCP_AUDIT_PAYLOAD_FORMAT_V1,
};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use ring::digest::{Context as DigestContext, SHA256};

const MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION: i64 = MCP_AUDIT_PAYLOAD_FORMAT_V1 as i64;
const MCP_AUDIT_SCHEMA_VERSION: i64 = 4;
const MCP_AUDIT_ROW_CONTEXT_DOMAIN: &[u8] = b"openlife:mcp-audit:row-context:v1";
const MCP_AUDIT_PRE_D064_STORE_BINDING_DOMAIN: &[u8] =
    b"openlife:mcp-audit:canonical-slot-binding:v1";
const MCP_AUDIT_LEGACY_MAX_ENCODED_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
const MCP_AUDIT_RECORD_ID_CANONICAL_BYTES: usize = 36;
pub const MCP_AUDIT_TOOL_NAME_MAX_BYTES: usize = 512;
pub const MCP_AUDIT_CREATED_AT_MAX_BYTES: usize = 64;

/// Product contract ceiling for MCP audit retention. The current raw `i64`
/// cleanup path does not enforce this yet; D063 keeps that behavior RED until
/// `McpAuditRetentionDays` becomes the domain mutation boundary.
pub const MCP_AUDIT_RETENTION_MAX_DAYS: i64 = 3_650;

#[cfg(test)]
fn audit_arguments_receipt(arguments: &Value) -> Result<String> {
    MinimizedAuditReceiptV1::for_arguments(arguments)
        .map_err(anyhow::Error::new)?
        .to_json_string()
        .map_err(anyhow::Error::new)
}

#[cfg(test)]
fn audit_result_receipt(result: &str) -> String {
    MinimizedAuditReceiptV1::for_result(result)
        .and_then(|receipt| receipt.to_json_string())
        .expect("a bounded UTF-8 MCP audit result always produces a v1 receipt")
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone)]
struct StoredAuditRow {
    id: i64,
    audit_record_id: Option<String>,
    tool_name: String,
    arguments_encrypted: String,
    result_encrypted: String,
    success: bool,
    pii_found: bool,
    created_at: String,
    key_epoch: u64,
    payload_version: i64,
}

#[derive(Debug)]
struct AuditPayloadMigration {
    source: StoredAuditRow,
    audit_record_id: Uuid,
    arguments_encrypted: String,
    result_encrypted: String,
}

#[derive(Debug)]
struct AuditPayloadPreflight {
    schema_columns: HashSet<String>,
    unique_record_index: bool,
    recorded_schema_version: Option<i64>,
    row_count: usize,
    /// The read-only pass validates every legacy row but retains only its
    /// stable primary key. Keeping old and newly sealed ciphertexts for the
    /// whole table would make startup memory proportional to total payload
    /// bytes. The writable transaction reloads and revalidates these rows
    /// before applying their exact-source CAS updates.
    legacy_row_ids: Vec<i64>,
}

/// Encrypted SQLite-backed store for MCP call logs with configurable key management.
#[derive(Clone)]
pub struct McpAuditStore {
    db_path: PathBuf,
    read_only: bool,
    unavailable_reason: Option<String>,
    /// Compatibility seam until D064 supplies its authenticated random store
    /// identity. This is derived from the already-canonical SQLite slot and is
    /// never serialized as a second key/reference authority.
    payload_store_identity_digest: [u8; 32],
    key: [u8; 32],
    key_config: AuditKeyConfig,
    keyring: HashMap<u64, [u8; 32]>,
    key_configs: Vec<AuditKeyConfig>,
}

impl McpAuditStore {
    fn payload_store_identity_digest(db_path: &Path) -> Result<[u8; 32]> {
        let absolute = if db_path.is_absolute() {
            db_path.to_path_buf()
        } else {
            std::env::current_dir()
                .context("resolve MCP audit current directory")?
                .join(db_path)
        };
        let parent = absolute
            .parent()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_database_parent_missing"))?;
        let canonical_parent = std::fs::canonicalize(parent).with_context(|| {
            format!(
                "canonicalize MCP audit database parent {}",
                parent.display()
            )
        })?;
        let file_name = absolute
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_database_file_name_missing"))?;
        let canonical_slot = canonical_parent.join(file_name);
        let canonical_slot = canonical_slot
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_database_path_not_utf8"))?;
        let mut context = DigestContext::new(&SHA256);
        context.update(MCP_AUDIT_PRE_D064_STORE_BINDING_DOMAIN);
        context.update(&(canonical_slot.len() as u64).to_be_bytes());
        context.update(canonical_slot.as_bytes());
        let digest = context.finish();
        let mut output = [0_u8; 32];
        output.copy_from_slice(digest.as_ref());
        Ok(output)
    }

    /// Test/fixture-only constructor for the historical deterministic key.
    /// Product code must hydrate a random keychain epoch and call
    /// `with_key_materials`.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let config = AuditKeyConfig::default();
        Self::with_config(db_path, config)
    }

    #[cfg(not(any(test, feature = "test-utils")))]
    pub(crate) fn new(db_path: impl Into<PathBuf>) -> Self {
        let config = AuditKeyConfig::default();
        Self::with_config(db_path, config)
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_config(db_path: impl Into<PathBuf>, config: AuditKeyConfig) -> Self {
        Self::with_keyring(db_path, vec![config])
    }

    #[cfg(not(any(test, feature = "test-utils")))]
    pub(crate) fn with_config(db_path: impl Into<PathBuf>, config: AuditKeyConfig) -> Self {
        Self::with_keyring(db_path, vec![config])
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn with_keyring(db_path: impl Into<PathBuf>, configs: Vec<AuditKeyConfig>) -> Self {
        Self::with_legacy_keyring_unchecked(db_path, configs)
    }

    #[cfg(not(any(test, feature = "test-utils")))]
    pub(crate) fn with_keyring(db_path: impl Into<PathBuf>, configs: Vec<AuditKeyConfig>) -> Self {
        Self::with_legacy_keyring_unchecked(db_path, configs)
    }

    fn with_legacy_keyring_unchecked(
        db_path: impl Into<PathBuf>,
        configs: Vec<AuditKeyConfig>,
    ) -> Self {
        let path = db_path.into();
        let (payload_store_identity_digest, identity_error) =
            match Self::payload_store_identity_digest(&path) {
                Ok(digest) => (digest, None),
                Err(error) => ([0_u8; 32], Some(error)),
            };
        let mut configs = if configs.is_empty() {
            vec![AuditKeyConfig::default()]
        } else {
            configs
        };
        let epoch_error = configs
            .iter()
            .find_map(|config| Self::sqlite_key_epoch(config.epoch).err());
        configs.sort_by_key(|config| config.epoch);
        configs.dedup_by_key(|config| config.epoch);
        let config = configs.last().cloned().unwrap_or_default();
        let key = Self::derive_key(&config);
        let keyring = configs
            .iter()
            .map(|config| (config.epoch, Self::derive_key(config)))
            .collect();
        let mut store = Self {
            db_path: path,
            read_only: false,
            unavailable_reason: None,
            payload_store_identity_digest,
            key,
            key_config: config,
            keyring,
            key_configs: configs,
        };
        let initialization = match identity_error.or(epoch_error) {
            Some(error) => Err(error),
            None => store.init_tables(),
        };
        if let Err(error) = initialization {
            store.read_only = true;
            store.unavailable_reason = Some(error.to_string());
        }
        store
    }

    pub fn with_key_materials(
        db_path: impl Into<PathBuf>,
        mut materials: Vec<AuditKeyMaterial>,
    ) -> Result<Self> {
        if materials.is_empty() {
            anyhow::bail!("MCP audit key material is empty");
        }
        for material in &materials {
            Self::sqlite_key_epoch(material.config.epoch)?;
        }
        materials.sort_by_key(|material| material.config.epoch);
        for pair in materials.windows(2) {
            if pair[0].config.epoch == pair[1].config.epoch {
                anyhow::bail!("duplicate MCP audit key epoch");
            }
        }
        let active = materials.last().cloned().expect("non-empty key materials");
        if active.config.mode != KeyMode::Keychain || active.config.key_ref.is_none() {
            anyhow::bail!(
                "active MCP audit key must be random keychain material; legacy modes are read-only migration keys"
            );
        }
        let keyring = materials
            .iter()
            .map(|material| (material.config.epoch, material.key))
            .collect::<HashMap<_, _>>();
        let key_configs = materials
            .iter()
            .map(|material| material.config.clone())
            .collect::<Vec<_>>();
        let db_path = db_path.into();
        let payload_store_identity_digest = Self::payload_store_identity_digest(&db_path)?;
        let store = Self {
            db_path,
            read_only: false,
            unavailable_reason: None,
            payload_store_identity_digest,
            key: active.key,
            key_config: active.config,
            keyring,
            key_configs,
        };
        store.init_tables()?;
        Ok(store)
    }

    /// Seed a historical keychain-encrypted row without granting the legacy
    /// reference current writable-owner authority. D064 uses this test-only
    /// fixture to construct a real pre-migration SQLite artifact, then opens
    /// that artifact through the product constructor with a new active key.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn write_historical_keychain_fixture(
        db_path: impl Into<PathBuf>,
        material: AuditKeyMaterial,
        tool_name: &str,
        arguments: &Value,
        result: &str,
        success: bool,
        pii_found: bool,
    ) -> Result<()> {
        if material.config.mode != KeyMode::Keychain || material.config.key_ref.is_none() {
            anyhow::bail!("historical MCP audit fixture requires keychain material");
        }
        let epoch = material.config.epoch;
        Self::sqlite_key_epoch(epoch)?;
        let key = material.key;
        let store = Self {
            db_path: db_path.into(),
            read_only: false,
            unavailable_reason: None,
            payload_store_identity_digest: [0_u8; 32],
            key,
            key_config: material.config.clone(),
            keyring: HashMap::from([(epoch, key)]),
            key_configs: vec![material.config],
        };
        let payload_store_identity_digest = Self::payload_store_identity_digest(&store.db_path)?;
        let mut store = store;
        store.payload_store_identity_digest = payload_store_identity_digest;
        store.init_tables()?;
        store.insert_log(tool_name, arguments, result, success, pii_found)?;
        Ok(())
    }

    pub fn open_read_only_existing_with_key_materials(
        db_path: impl Into<PathBuf>,
        mut materials: Vec<AuditKeyMaterial>,
    ) -> Result<Self> {
        if materials.is_empty() {
            anyhow::bail!("MCP audit key material is empty");
        }
        for material in &materials {
            Self::sqlite_key_epoch(material.config.epoch)?;
        }
        materials.sort_by_key(|material| material.config.epoch);
        for pair in materials.windows(2) {
            if pair[0].config.epoch == pair[1].config.epoch {
                anyhow::bail!("duplicate MCP audit key epoch");
            }
        }
        let active = materials.last().cloned().expect("non-empty key materials");
        if active.config.mode != KeyMode::Keychain || active.config.key_ref.is_none() {
            anyhow::bail!("active MCP audit key must be random keychain material");
        }
        let db_path = db_path.into();
        let payload_store_identity_digest = Self::payload_store_identity_digest(&db_path)?;
        crate::sqlite_migration::open_existing_read_only(
            &db_path,
            "mcp_audit_store",
            &["mcp_log"],
        )?;
        Ok(Self {
            db_path,
            read_only: true,
            unavailable_reason: None,
            payload_store_identity_digest,
            key: active.key,
            key_config: active.config,
            keyring: materials
                .iter()
                .map(|material| (material.config.epoch, material.key))
                .collect(),
            key_configs: materials
                .into_iter()
                .map(|material| material.config)
                .collect(),
        })
        .and_then(|store| {
            store.validate_read_only_payloads()?;
            Ok(store)
        })
    }

    pub fn unavailable_sentinel(reason: impl Into<String>) -> Self {
        Self {
            db_path: PathBuf::new(),
            read_only: true,
            unavailable_reason: Some(reason.into()),
            payload_store_identity_digest: [0_u8; 32],
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
        Self::sqlite_key_epoch(material.config.epoch)?;
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

    fn database_has_bytes(&self) -> Result<bool> {
        match std::fs::symlink_metadata(&self.db_path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    anyhow::bail!("mcp_audit_database_slot_not_regular_file");
                }
                Ok(metadata.len() > 0)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error).context("inspect MCP audit database slot"),
        }
    }

    fn open_preflight_connection(&self) -> Result<Option<Connection>> {
        if !self.database_has_bytes()? {
            return Ok(None);
        }
        Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map(Some)
            .context("open MCP audit database for zero-write payload preflight")
    }

    fn audit_table_exists(connection: &Connection) -> Result<bool> {
        connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'mcp_log'",
                [],
                |_| Ok(()),
            )
            .optional()
            .map(|value| value.is_some())
            .context("inspect MCP audit table")
    }

    fn audit_schema_columns(connection: &Connection) -> Result<HashSet<String>> {
        let mut statement = connection.prepare("PRAGMA table_info(mcp_log)")?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<HashSet<_>, _>>()
            .context("inspect MCP audit schema columns")?;
        Ok(columns)
    }

    fn recorded_schema_version(connection: &Connection) -> Result<Option<i64>> {
        let registry_exists = connection
            .query_row(
                "SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'openlife_schema_versions'",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !registry_exists {
            return Ok(None);
        }
        let version = connection
            .query_row(
                "SELECT version FROM openlife_schema_versions
                 WHERE component = 'mcp_audit_store'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("read MCP audit schema version")?;
        if version.is_some_and(|version| version > MCP_AUDIT_SCHEMA_VERSION) {
            anyhow::bail!("mcp_audit_schema_version_newer_than_runtime");
        }
        Ok(version)
    }

    fn unique_record_index_exists(connection: &Connection) -> Result<bool> {
        let mut statement = connection.prepare("PRAGMA index_list(mcp_log)")?;
        let indexes = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        for index in indexes {
            let (name, unique, partial) = index?;
            if name == "idx_mcp_log_audit_record_id" {
                // SQLite unique indexes allow multiple NULL values, so the
                // migration does not need a predicate for legacy rows. A full
                // index is stronger and avoids trusting an attacker-chosen
                // partial-index WHERE clause that merely shares this name.
                if unique != 1 || partial != 0 {
                    anyhow::bail!("mcp_audit_record_identity_index_invalid");
                }
                let mut columns = connection.prepare(&format!("PRAGMA index_info({name})"))?;
                let indexed = columns
                    .query_map([], |row| row.get::<_, String>(2))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if indexed != ["audit_record_id"] {
                    anyhow::bail!("mcp_audit_record_identity_index_columns_invalid");
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn visit_stored_rows(
        connection: &Connection,
        columns: &HashSet<String>,
        mut visit: impl FnMut(StoredAuditRow) -> Result<()>,
    ) -> Result<usize> {
        let audit_record_id = if columns.contains("audit_record_id") {
            "audit_record_id"
        } else {
            "NULL"
        };
        let bounded_audit_record_id = if columns.contains("audit_record_id") {
            format!(
                "CASE
                    WHEN audit_record_id IS NULL THEN NULL
                    WHEN octet_length(audit_record_id) <= {record_id_max}
                        THEN audit_record_id
                    ELSE ''
                 END",
                record_id_max = MCP_AUDIT_RECORD_ID_CANONICAL_BYTES,
            )
        } else {
            "NULL".to_string()
        };
        let bounded_tool_name = format!(
            "CASE WHEN octet_length(tool_name) <= {tool_name_max}
                  THEN tool_name ELSE NULL END",
            tool_name_max = MCP_AUDIT_TOOL_NAME_MAX_BYTES,
        );
        let bounded_created_at = format!(
            "CASE WHEN octet_length(created_at) <= {created_at_max}
                  THEN created_at ELSE NULL END",
            created_at_max = MCP_AUDIT_CREATED_AT_MAX_BYTES,
        );
        let key_epoch = if columns.contains("key_epoch") {
            "key_epoch"
        } else {
            "0"
        };
        let payload_version = if columns.contains("payload_minimized_version") {
            "payload_minimized_version"
        } else {
            "0"
        };
        // Do not first materialize attacker-controlled SQLite TEXT and only
        // then ask the codec whether it is too large. Returning NULL from the
        // SQL projection keeps oversized payloads out of Rust heap allocation.
        // Legacy rows retain their explicit migration ceiling; rows claiming
        // current authenticated identity receive the much smaller envelope
        // ceiling.
        let arguments_encrypted = format!(
            "CASE
                WHEN {audit_record_id} IS NULL
                     AND octet_length(arguments_encrypted) <= {legacy_max}
                    THEN arguments_encrypted
                WHEN {audit_record_id} IS NOT NULL
                     AND octet_length(arguments_encrypted) <= {current_max}
                    THEN arguments_encrypted
                ELSE NULL
             END",
            legacy_max = MCP_AUDIT_LEGACY_MAX_ENCODED_PAYLOAD_BYTES,
            current_max = MCP_AUDIT_MAX_ENVELOPE_ENCODED_BYTES,
        );
        let result_encrypted = format!(
            "CASE
                WHEN {audit_record_id} IS NULL
                     AND octet_length(result_encrypted) <= {legacy_max}
                    THEN result_encrypted
                WHEN {audit_record_id} IS NOT NULL
                     AND octet_length(result_encrypted) <= {current_max}
                    THEN result_encrypted
                ELSE NULL
             END",
            legacy_max = MCP_AUDIT_LEGACY_MAX_ENCODED_PAYLOAD_BYTES,
            current_max = MCP_AUDIT_MAX_ENVELOPE_ENCODED_BYTES,
        );
        let sql = format!(
            "SELECT id, {bounded_audit_record_id}, {bounded_tool_name},
                    {arguments_encrypted}, {result_encrypted}, success, pii_found,
                    {bounded_created_at},
                    {key_epoch}, {payload_version}
             FROM mcp_log ORDER BY id ASC"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            let success = row.get::<_, i64>(5)?;
            let pii_found = row.get::<_, i64>(6)?;
            let key_epoch = row.get::<_, i64>(8)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                success,
                pii_found,
                row.get::<_, Option<String>>(7)?,
                key_epoch,
                row.get::<_, i64>(9)?,
            ))
        })?;

        let mut row_count = 0_usize;
        for row in rows {
            let (
                id,
                audit_record_id,
                tool_name,
                arguments_encrypted,
                result_encrypted,
                success,
                pii_found,
                created_at,
                key_epoch,
                payload_version,
            ) = row?;
            if audit_record_id.as_deref() == Some("") {
                anyhow::bail!("mcp_audit_record_id_exceeds_storage_bound:{id}");
            }
            let tool_name = tool_name
                .ok_or_else(|| anyhow::anyhow!("mcp_audit_tool_name_exceeds_storage_bound:{id}"))?;
            let created_at = created_at.ok_or_else(|| {
                anyhow::anyhow!("mcp_audit_created_at_exceeds_storage_bound:{id}")
            })?;
            Self::validate_row_metadata(
                id,
                &tool_name,
                success,
                pii_found,
                &created_at,
                key_epoch,
            )?;
            let payload_kind = if audit_record_id.is_some() {
                "current"
            } else {
                "legacy"
            };
            let arguments_encrypted = arguments_encrypted.ok_or_else(|| {
                anyhow::anyhow!(
                    "mcp_audit_{payload_kind}_ciphertext_exceeds_storage_bound:{id}:arguments"
                )
            })?;
            let result_encrypted = result_encrypted.ok_or_else(|| {
                anyhow::anyhow!(
                    "mcp_audit_{payload_kind}_ciphertext_exceeds_storage_bound:{id}:result"
                )
            })?;
            visit(StoredAuditRow {
                id,
                audit_record_id,
                tool_name,
                arguments_encrypted,
                result_encrypted,
                success: success == 1,
                pii_found: pii_found == 1,
                created_at,
                key_epoch: key_epoch as u64,
                payload_version,
            })?;
            row_count = row_count
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("mcp_audit_row_count_overflow"))?;
        }
        Ok(row_count)
    }

    fn stored_row_by_id(connection: &Connection, id: i64) -> Result<StoredAuditRow> {
        let sql = format!(
            "SELECT id,
                    CASE
                        WHEN audit_record_id IS NULL THEN NULL
                        WHEN octet_length(audit_record_id) <= {record_id_max}
                            THEN audit_record_id
                        ELSE ''
                    END,
                    CASE WHEN octet_length(tool_name) <= {tool_name_max}
                         THEN tool_name ELSE NULL END,
                    CASE
                        WHEN audit_record_id IS NULL
                             AND octet_length(arguments_encrypted) <= {legacy_max}
                            THEN arguments_encrypted
                        WHEN audit_record_id IS NOT NULL
                             AND octet_length(arguments_encrypted) <= {current_max}
                            THEN arguments_encrypted
                        ELSE NULL
                    END,
                    CASE
                        WHEN audit_record_id IS NULL
                             AND octet_length(result_encrypted) <= {legacy_max}
                            THEN result_encrypted
                        WHEN audit_record_id IS NOT NULL
                             AND octet_length(result_encrypted) <= {current_max}
                            THEN result_encrypted
                        ELSE NULL
                    END,
                    success, pii_found,
                    CASE WHEN octet_length(created_at) <= {created_at_max}
                         THEN created_at ELSE NULL END,
                    key_epoch, payload_minimized_version
             FROM mcp_log WHERE id = ?1",
            record_id_max = MCP_AUDIT_RECORD_ID_CANONICAL_BYTES,
            tool_name_max = MCP_AUDIT_TOOL_NAME_MAX_BYTES,
            created_at_max = MCP_AUDIT_CREATED_AT_MAX_BYTES,
            legacy_max = MCP_AUDIT_LEGACY_MAX_ENCODED_PAYLOAD_BYTES,
            current_max = MCP_AUDIT_MAX_ENVELOPE_ENCODED_BYTES,
        );
        let row = connection
            .query_row(&sql, [id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_payload_migration_source_missing:{id}"))?;
        let (
            id,
            audit_record_id,
            tool_name,
            arguments_encrypted,
            result_encrypted,
            success,
            pii_found,
            created_at,
            key_epoch,
            payload_version,
        ) = row;
        if audit_record_id.as_deref() == Some("") {
            anyhow::bail!("mcp_audit_record_id_exceeds_storage_bound:{id}");
        }
        let tool_name = tool_name
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_tool_name_exceeds_storage_bound:{id}"))?;
        let created_at = created_at
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_created_at_exceeds_storage_bound:{id}"))?;
        Self::validate_row_metadata(id, &tool_name, success, pii_found, &created_at, key_epoch)?;
        let payload_kind = if audit_record_id.is_some() {
            "current"
        } else {
            "legacy"
        };
        let arguments_encrypted = arguments_encrypted.ok_or_else(|| {
            anyhow::anyhow!(
                "mcp_audit_{payload_kind}_ciphertext_exceeds_storage_bound:{id}:arguments"
            )
        })?;
        let result_encrypted = result_encrypted.ok_or_else(|| {
            anyhow::anyhow!("mcp_audit_{payload_kind}_ciphertext_exceeds_storage_bound:{id}:result")
        })?;
        Ok(StoredAuditRow {
            id,
            audit_record_id,
            tool_name,
            arguments_encrypted,
            result_encrypted,
            success: success == 1,
            pii_found: pii_found == 1,
            created_at,
            key_epoch: key_epoch as u64,
            payload_version,
        })
    }

    fn append_context_field(context: &mut DigestContext, tag: u8, value: &[u8]) {
        context.update(&[tag]);
        context.update(&(value.len() as u64).to_be_bytes());
        context.update(value);
    }

    fn validate_tool_name_and_timestamp(tool_name: &str, created_at: &str) -> Result<()> {
        if tool_name.is_empty()
            || tool_name.len() > MCP_AUDIT_TOOL_NAME_MAX_BYTES
            || tool_name.chars().any(char::is_control)
        {
            anyhow::bail!("mcp_audit_tool_name_invalid");
        }
        if created_at.is_empty()
            || created_at.len() > MCP_AUDIT_CREATED_AT_MAX_BYTES
            || chrono::DateTime::parse_from_rfc3339(created_at).is_err()
        {
            anyhow::bail!("mcp_audit_created_at_invalid");
        }
        Ok(())
    }

    fn validate_row_metadata(
        id: i64,
        tool_name: &str,
        success: i64,
        pii_found: i64,
        created_at: &str,
        key_epoch: i64,
    ) -> Result<()> {
        if id <= 0 || !matches!(success, 0 | 1) || !matches!(pii_found, 0 | 1) || key_epoch < 0 {
            anyhow::bail!("mcp_audit_row_metadata_invalid:{id}");
        }
        Self::validate_tool_name_and_timestamp(tool_name, created_at)
            .map_err(|error| anyhow::anyhow!("{error}:{id}"))
    }

    fn sqlite_key_epoch(epoch: u64) -> Result<i64> {
        i64::try_from(epoch)
            .map_err(|_| anyhow::anyhow!("mcp_audit_key_epoch_exceeds_sqlite_range:{epoch}"))
    }

    fn canonical_audit_record_id(value: &str, row_id: i64) -> Result<Uuid> {
        let record_id = Uuid::parse_str(value)
            .map_err(|_| anyhow::anyhow!("mcp_audit_record_id_invalid:{row_id}"))?;
        if record_id.to_string() != value {
            anyhow::bail!("mcp_audit_record_id_noncanonical:{row_id}");
        }
        Ok(record_id)
    }

    fn row_context_digest(
        tool_name: &str,
        success: bool,
        pii_found: bool,
        created_at: &str,
    ) -> [u8; 32] {
        let mut context = DigestContext::new(&SHA256);
        context.update(MCP_AUDIT_ROW_CONTEXT_DOMAIN);
        Self::append_context_field(&mut context, 1, tool_name.as_bytes());
        Self::append_context_field(&mut context, 2, &[u8::from(success)]);
        Self::append_context_field(&mut context, 3, &[u8::from(pii_found)]);
        Self::append_context_field(&mut context, 4, created_at.as_bytes());
        let digest = context.finish();
        let mut output = [0_u8; 32];
        output.copy_from_slice(digest.as_ref());
        output
    }

    fn payload_binding(
        &self,
        audit_record_id: Uuid,
        key_epoch: u64,
        tool_name: &str,
        success: bool,
        pii_found: bool,
        created_at: &str,
        role: McpAuditPayloadRole,
    ) -> Result<McpAuditPayloadBindingV1> {
        McpAuditPayloadBindingV1::new(
            self.payload_store_identity_digest,
            audit_record_id,
            key_epoch,
            Self::row_context_digest(tool_name, success, pii_found, created_at),
            role,
        )
        .map_err(anyhow::Error::new)
    }

    fn key_for_epoch(&self, key_epoch: u64) -> Result<&[u8; 32]> {
        self.keyring
            .get(&key_epoch)
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_key_epoch_unavailable:{key_epoch}"))
    }

    fn authenticate_current_row(
        &self,
        row: &StoredAuditRow,
    ) -> Result<(MinimizedAuditReceiptV1, MinimizedAuditReceiptV1)> {
        if row.payload_version != MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION {
            anyhow::bail!(
                "mcp_audit_payload_version_unsupported:{}",
                row.payload_version
            );
        }
        let record_id = row
            .audit_record_id
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_record_id_missing:{}", row.id))
            .and_then(|value| Self::canonical_audit_record_id(value, row.id))?;
        let key = self.key_for_epoch(row.key_epoch)?;
        let arguments_binding = self.payload_binding(
            record_id,
            row.key_epoch,
            &row.tool_name,
            row.success,
            row.pii_found,
            &row.created_at,
            McpAuditPayloadRole::Arguments,
        )?;
        let result_binding = self.payload_binding(
            record_id,
            row.key_epoch,
            &row.tool_name,
            row.success,
            row.pii_found,
            &row.created_at,
            McpAuditPayloadRole::Result,
        )?;
        let arguments =
            open_authenticated_audit_payload(key, &arguments_binding, &row.arguments_encrypted)
                .map_err(anyhow::Error::new)?;
        let result = open_authenticated_audit_payload(key, &result_binding, &row.result_encrypted)
            .map_err(anyhow::Error::new)?;
        if arguments.format_version() as i64 != row.payload_version
            || result.format_version() as i64 != row.payload_version
            || arguments.role() != McpAuditPayloadRole::Arguments
            || result.role() != McpAuditPayloadRole::Result
        {
            anyhow::bail!(
                "mcp_audit_authenticated_payload_binding_mismatch:{}",
                row.id
            );
        }
        Ok((arguments.receipt().clone(), result.receipt().clone()))
    }

    fn decode_legacy_receipts(
        &self,
        row: &StoredAuditRow,
    ) -> Result<(MinimizedAuditReceiptV1, MinimizedAuditReceiptV1)> {
        if row.audit_record_id.is_some() {
            anyhow::bail!("mcp_audit_legacy_row_has_record_id:{}", row.id);
        }
        let key = self.key_for_epoch(row.key_epoch)?;
        let arguments_plaintext = self.decrypt_with_key(&row.arguments_encrypted, key)?;
        let result_plaintext = self.decrypt_with_key(&row.result_encrypted, key)?;
        match row.payload_version {
            0 => {
                let arguments = serde_json::from_str::<Value>(&arguments_plaintext)
                    .context("decode legacy MCP audit arguments")?;
                Ok((
                    MinimizedAuditReceiptV1::for_arguments(&arguments)
                        .map_err(anyhow::Error::new)?,
                    MinimizedAuditReceiptV1::for_result(&result_plaintext)
                        .map_err(anyhow::Error::new)?,
                ))
            }
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION => Ok((
                MinimizedAuditReceiptV1::decode_strict(
                    arguments_plaintext.as_bytes(),
                    McpAuditPayloadRole::Arguments,
                )
                .map_err(anyhow::Error::new)?,
                MinimizedAuditReceiptV1::decode_strict(
                    result_plaintext.as_bytes(),
                    McpAuditPayloadRole::Result,
                )
                .map_err(anyhow::Error::new)?,
            )),
            version => anyhow::bail!("mcp_audit_payload_version_unsupported:{version}"),
        }
    }

    fn build_payload_migration(&self, row: StoredAuditRow) -> Result<AuditPayloadMigration> {
        let (arguments_receipt, result_receipt) = self.decode_legacy_receipts(&row)?;

        let audit_record_id = Uuid::new_v4();
        let arguments_binding = self.payload_binding(
            audit_record_id,
            self.key_config.epoch,
            &row.tool_name,
            row.success,
            row.pii_found,
            &row.created_at,
            McpAuditPayloadRole::Arguments,
        )?;
        let result_binding = self.payload_binding(
            audit_record_id,
            self.key_config.epoch,
            &row.tool_name,
            row.success,
            row.pii_found,
            &row.created_at,
            McpAuditPayloadRole::Result,
        )?;
        Ok(AuditPayloadMigration {
            source: row,
            audit_record_id,
            arguments_encrypted: seal_minimized_audit_receipt_v1(
                &self.key,
                &arguments_binding,
                &arguments_receipt,
            )
            .map_err(anyhow::Error::new)?,
            result_encrypted: seal_minimized_audit_receipt_v1(
                &self.key,
                &result_binding,
                &result_receipt,
            )
            .map_err(anyhow::Error::new)?,
        })
    }

    fn preflight_payloads(&self, connection: &Connection) -> Result<AuditPayloadPreflight> {
        let recorded_schema_version = Self::recorded_schema_version(connection)?;
        if !Self::audit_table_exists(connection)? {
            return Ok(AuditPayloadPreflight {
                schema_columns: HashSet::new(),
                unique_record_index: false,
                recorded_schema_version,
                row_count: 0,
                legacy_row_ids: Vec::new(),
            });
        }
        let schema_columns = Self::audit_schema_columns(connection)?;
        let unique_record_index = Self::unique_record_index_exists(connection)?;
        let mut legacy_row_ids = Vec::new();
        let mut record_ids = HashSet::new();
        let row_count = Self::visit_stored_rows(connection, &schema_columns, |row| {
            if row.audit_record_id.is_some() {
                let record_id = row
                    .audit_record_id
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("mcp_audit_record_id_missing:{}", row.id))
                    .and_then(|value| Self::canonical_audit_record_id(value, row.id))?;
                if !record_ids.insert(record_id) {
                    anyhow::bail!("mcp_audit_record_id_duplicate:{record_id}");
                }
                self.authenticate_current_row(&row)?;
            } else {
                self.decode_legacy_receipts(&row)?;
                legacy_row_ids.push(row.id);
            }
            Ok(())
        })?;
        Ok(AuditPayloadPreflight {
            schema_columns,
            unique_record_index,
            recorded_schema_version,
            row_count,
            legacy_row_ids,
        })
    }

    fn existing_payload_preflight(&self) -> Result<Option<AuditPayloadPreflight>> {
        self.open_preflight_connection()?
            .map(|connection| self.preflight_payloads(&connection))
            .transpose()
    }

    fn target_payload_schema(columns: &HashSet<String>) -> bool {
        ["key_epoch", "payload_minimized_version", "audit_record_id"]
            .iter()
            .all(|column| columns.contains(*column))
    }

    fn validate_read_only_payloads(&self) -> Result<()> {
        let connection = self
            .open_preflight_connection()?
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_database_missing"))?;
        let preflight = self.preflight_payloads(&connection)?;
        if !Self::target_payload_schema(&preflight.schema_columns)
            || !preflight.unique_record_index
            || preflight.recorded_schema_version != Some(MCP_AUDIT_SCHEMA_VERSION)
            || !preflight.legacy_row_ids.is_empty()
        {
            anyhow::bail!("mcp_audit_read_only_payload_migration_required");
        }
        Ok(())
    }

    fn init_tables(&self) -> Result<()> {
        let preflight = self.existing_payload_preflight()?;
        if preflight.as_ref().is_some_and(|preflight| {
            Self::target_payload_schema(&preflight.schema_columns)
                && preflight.unique_record_index
                && preflight.recorded_schema_version == Some(MCP_AUDIT_SCHEMA_VERSION)
                && preflight.legacy_row_ids.is_empty()
        }) {
            return Ok(());
        }

        let expected_row_count = preflight.as_ref().map_or(0, |value| value.row_count);
        let legacy_row_ids = preflight
            .map(|value| value.legacy_row_ids)
            .unwrap_or_default();
        let mut connection = self.conn()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if Self::audit_table_exists(&transaction)? {
            let observed_count =
                transaction.query_row("SELECT COUNT(*) FROM mcp_log", [], |row| {
                    row.get::<_, i64>(0)
                })?;
            if observed_count < 0 || observed_count as usize != expected_row_count {
                anyhow::bail!("mcp_audit_payload_preflight_row_count_changed");
            }
        } else if expected_row_count != 0 {
            anyhow::bail!("mcp_audit_payload_preflight_table_disappeared");
        }
        transaction.execute(
            "CREATE TABLE IF NOT EXISTS mcp_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                audit_record_id TEXT,
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
            &transaction,
            "mcp_log",
            "key_epoch",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        crate::sqlite_migration::ensure_column(
            &transaction,
            "mcp_log",
            "payload_minimized_version",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        crate::sqlite_migration::ensure_column(&transaction, "mcp_log", "audit_record_id", "TEXT")?;

        for row_id in legacy_row_ids {
            let source = Self::stored_row_by_id(&transaction, row_id)?;
            let migration = self.build_payload_migration(source)?;
            let source = &migration.source;
            let active_key_epoch = Self::sqlite_key_epoch(self.key_config.epoch)?;
            let source_key_epoch = Self::sqlite_key_epoch(source.key_epoch)?;
            let changed = transaction.execute(
                "UPDATE mcp_log
                 SET audit_record_id = ?1,
                     arguments_encrypted = ?2,
                     result_encrypted = ?3,
                     key_epoch = ?4,
                     payload_minimized_version = ?5
                 WHERE id = ?6
                   AND audit_record_id IS NULL
                   AND tool_name = ?7
                   AND arguments_encrypted = ?8
                   AND result_encrypted = ?9
                   AND success = ?10
                   AND pii_found = ?11
                   AND created_at = ?12
                   AND key_epoch = ?13
                   AND payload_minimized_version = ?14",
                params![
                    migration.audit_record_id.to_string(),
                    migration.arguments_encrypted,
                    migration.result_encrypted,
                    active_key_epoch,
                    MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
                    source.id,
                    source.tool_name,
                    source.arguments_encrypted,
                    source.result_encrypted,
                    i64::from(source.success),
                    i64::from(source.pii_found),
                    source.created_at,
                    source_key_epoch,
                    source.payload_version,
                ],
            )?;
            if changed != 1 {
                anyhow::bail!("mcp_audit_payload_migration_source_changed:{}", source.id);
            }
        }
        transaction.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_mcp_log_audit_record_id
             ON mcp_log(audit_record_id)",
            [],
        )?;
        crate::sqlite_migration::record_schema_version(
            &transaction,
            "mcp_audit_store",
            MCP_AUDIT_SCHEMA_VERSION,
        )?;
        let verified = self.preflight_payloads(&transaction)?;
        if verified.row_count != expected_row_count
            || !verified.unique_record_index
            || verified.recorded_schema_version != Some(MCP_AUDIT_SCHEMA_VERSION)
            || !verified.legacy_row_ids.is_empty()
        {
            anyhow::bail!("mcp_audit_payload_migration_verification_failed");
        }
        transaction.commit()?;
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.key).map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let nonce_bytes = rand::random::<[u8; 12]>();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("encrypt failed: {:?}", e))?;
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(general_purpose::STANDARD.encode(&combined))
    }

    /// D068 RED-fixture seam for the exact historical, pre-envelope wire
    /// format. Keep this test-only helper bound to the production legacy
    /// decoder when the authenticated current envelope is introduced.
    #[cfg(any(test, feature = "test-utils"))]
    fn d068_encrypt_legacy_payload_fixture_for_test(&self, plaintext: &str) -> Result<String> {
        self.encrypt(plaintext)
    }

    fn decrypt_with_key(&self, combined_b64: &str, key: &[u8; 32]) -> Result<String> {
        if combined_b64.len() > MCP_AUDIT_LEGACY_MAX_ENCODED_PAYLOAD_BYTES {
            anyhow::bail!("legacy MCP audit ciphertext exceeds migration bound");
        }
        let combined = general_purpose::STANDARD
            .decode(combined_b64)
            .context("invalid base64")?;
        if combined.len() < 12 {
            return Err(anyhow::anyhow!("ciphertext too short"));
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| anyhow::anyhow!("{:?}", e))?;
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("decrypt failed: {:?}", e))?;
        String::from_utf8(plaintext).context("utf8 decode")
    }

    pub fn insert_log(
        &self,
        tool_name: &str,
        arguments: &Value,
        result: &str,
        success: bool,
        pii_found: bool,
    ) -> Result<i64> {
        let created_at = chrono::Utc::now().to_rfc3339();
        self.insert_log_at(
            tool_name,
            arguments,
            result,
            success,
            pii_found,
            &created_at,
        )
    }

    fn insert_log_at(
        &self,
        tool_name: &str,
        arguments: &Value,
        result: &str,
        success: bool,
        pii_found: bool,
        created_at: &str,
    ) -> Result<i64> {
        Self::validate_tool_name_and_timestamp(tool_name, created_at)?;
        let key_epoch = Self::sqlite_key_epoch(self.key_config.epoch)?;
        let audit_record_id = Uuid::new_v4();
        let arguments_receipt =
            MinimizedAuditReceiptV1::for_arguments(arguments).map_err(anyhow::Error::new)?;
        let result_receipt =
            MinimizedAuditReceiptV1::for_result(result).map_err(anyhow::Error::new)?;
        let arguments_binding = self.payload_binding(
            audit_record_id,
            self.key_config.epoch,
            tool_name,
            success,
            pii_found,
            created_at,
            McpAuditPayloadRole::Arguments,
        )?;
        let result_binding = self.payload_binding(
            audit_record_id,
            self.key_config.epoch,
            tool_name,
            success,
            pii_found,
            created_at,
            McpAuditPayloadRole::Result,
        )?;
        let args_enc =
            seal_minimized_audit_receipt_v1(&self.key, &arguments_binding, &arguments_receipt)
                .map_err(anyhow::Error::new)?;
        let res_enc = seal_minimized_audit_receipt_v1(&self.key, &result_binding, &result_receipt)
            .map_err(anyhow::Error::new)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO mcp_log (
                audit_record_id, tool_name, arguments_encrypted, result_encrypted,
                success, pii_found, created_at, key_epoch, payload_minimized_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                audit_record_id.to_string(),
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

    #[cfg(test)]
    fn d068_payload_role_for_test(role: &str) -> Result<McpAuditPayloadRole> {
        match role {
            "arguments" => Ok(McpAuditPayloadRole::Arguments),
            "result" => Ok(McpAuditPayloadRole::Result),
            _ => anyhow::bail!("invalid D068 fixture role"),
        }
    }

    /// Insert an adversarial *authenticated* current-format row. This fixture
    /// shares the production record-id/context binding and differs only in
    /// allowing a chosen envelope version or invalid receipt plaintext.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn d068_insert_current_payload_fixture_for_test(
        &self,
        tool_name: &str,
        arguments_role: &str,
        arguments_format_version: i64,
        arguments_receipt_json: &str,
        result_role: &str,
        result_format_version: i64,
        result_receipt_json: &str,
        database_version: i64,
    ) -> Result<i64> {
        let created_at = "2026-07-13T12:00:00Z";
        let key_epoch = Self::sqlite_key_epoch(self.key_config.epoch)?;
        let audit_record_id = Uuid::new_v4();
        let arguments_role = Self::d068_payload_role_for_test(arguments_role)?;
        let result_role = Self::d068_payload_role_for_test(result_role)?;
        let arguments_binding = self.payload_binding(
            audit_record_id,
            self.key_config.epoch,
            tool_name,
            true,
            true,
            created_at,
            arguments_role,
        )?;
        let result_binding = self.payload_binding(
            audit_record_id,
            self.key_config.epoch,
            tool_name,
            true,
            true,
            created_at,
            result_role,
        )?;
        let arguments_encrypted = crate::mcp_audit_payload_codec::seal_payload_fixture_for_test(
            &self.key,
            u32::try_from(arguments_format_version)
                .context("convert D068 arguments fixture version")?,
            &arguments_binding,
            arguments_receipt_json.as_bytes(),
        )
        .map_err(anyhow::Error::new)?;
        let result_encrypted = crate::mcp_audit_payload_codec::seal_payload_fixture_for_test(
            &self.key,
            u32::try_from(result_format_version).context("convert D068 result fixture version")?,
            &result_binding,
            result_receipt_json.as_bytes(),
        )
        .map_err(anyhow::Error::new)?;
        let connection = self.conn()?;
        connection.execute(
            "INSERT INTO mcp_log (
                audit_record_id, tool_name, arguments_encrypted, result_encrypted,
                success, pii_found, created_at, key_epoch, payload_minimized_version
             ) VALUES (?1, ?2, ?3, ?4, 1, 1, ?5, ?6, ?7)",
            params![
                audit_record_id.to_string(),
                tool_name,
                arguments_encrypted,
                result_encrypted,
                created_at,
                key_epoch,
                database_version,
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    /// Test-utils-only constructor for the source-backed version-zero payload
    /// format used by the D068 bootstrap attack. The authenticated-envelope
    /// implementation must keep this bound to the real legacy decoder; it is
    /// not a product write path.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn d068_insert_legacy_payload_fixture_for_test(
        &self,
        tool_name: &str,
        raw_arguments: &Value,
        raw_result: &str,
    ) -> Result<i64> {
        let key_epoch = Self::sqlite_key_epoch(self.key_config.epoch)?;
        let connection = self.conn()?;
        connection.execute(
            "INSERT INTO mcp_log (
                tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                created_at, key_epoch, payload_minimized_version
             ) VALUES (?1, ?2, ?3, 1, 1, ?4, ?5, 0)",
            params![
                tool_name,
                self.d068_encrypt_legacy_payload_fixture_for_test(&serde_json::to_string(
                    raw_arguments
                )?)?,
                self.d068_encrypt_legacy_payload_fixture_for_test(raw_result)?,
                "2026-07-13T12:00:00Z",
                key_epoch,
            ],
        )?;
        Ok(connection.last_insert_rowid())
    }

    /// Test-utils-only tamper operation: mutate only the plaintext format
    /// column so a version-zero legacy ciphertext claims to be current.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn d068_flip_payload_version_to_current_for_test(&self, row_id: i64) -> Result<()> {
        self.conn()?.execute(
            "UPDATE mcp_log SET payload_minimized_version = ?1 WHERE id = ?2",
            params![MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION, row_id],
        )?;
        Ok(())
    }

    pub fn list_logs(&self, limit: usize) -> Result<Vec<McpLogEntry>> {
        let conn = self.conn()?;
        // Product reads only admit current envelopes. The CASE projection is
        // deliberately inside SQLite so a tampered oversized TEXT value never
        // becomes a Rust String before it is rejected.
        let sql = format!(
            "SELECT id,
                    CASE
                        WHEN audit_record_id IS NULL THEN NULL
                        WHEN octet_length(audit_record_id) <= {record_id_max}
                            THEN audit_record_id
                        ELSE ''
                    END,
                    CASE WHEN octet_length(tool_name) <= {tool_name_max}
                         THEN tool_name ELSE NULL END,
                    CASE WHEN octet_length(arguments_encrypted) <= {current_max}
                         THEN arguments_encrypted ELSE NULL END,
                    CASE WHEN octet_length(result_encrypted) <= {current_max}
                         THEN result_encrypted ELSE NULL END,
                    success, pii_found,
                    CASE WHEN octet_length(created_at) <= {created_at_max}
                         THEN created_at ELSE NULL END,
                    key_epoch,
                    payload_minimized_version
             FROM mcp_log
             ORDER BY id DESC
             LIMIT ?1",
            record_id_max = MCP_AUDIT_RECORD_ID_CANONICAL_BYTES,
            tool_name_max = MCP_AUDIT_TOOL_NAME_MAX_BYTES,
            created_at_max = MCP_AUDIT_CREATED_AT_MAX_BYTES,
            current_max = MCP_AUDIT_MAX_ENVELOPE_ENCODED_BYTES,
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([limit], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (
                id,
                audit_record_id,
                tool_name,
                arguments_encrypted,
                result_encrypted,
                success,
                pii_found,
                created_at,
                key_epoch,
                payload_version,
            ) = row?;
            if audit_record_id.as_deref() == Some("") {
                anyhow::bail!("mcp_audit_record_id_exceeds_storage_bound:{id}");
            }
            let tool_name = tool_name
                .ok_or_else(|| anyhow::anyhow!("mcp_audit_tool_name_exceeds_storage_bound:{id}"))?;
            let created_at = created_at.ok_or_else(|| {
                anyhow::anyhow!("mcp_audit_created_at_exceeds_storage_bound:{id}")
            })?;
            let arguments_encrypted = arguments_encrypted.ok_or_else(|| {
                anyhow::anyhow!("mcp_audit_current_ciphertext_exceeds_storage_bound:{id}:arguments")
            })?;
            let result_encrypted = result_encrypted.ok_or_else(|| {
                anyhow::anyhow!("mcp_audit_current_ciphertext_exceeds_storage_bound:{id}:result")
            })?;
            Self::validate_row_metadata(
                id,
                &tool_name,
                success,
                pii_found,
                &created_at,
                key_epoch,
            )?;
            let row = StoredAuditRow {
                id,
                audit_record_id,
                tool_name,
                arguments_encrypted,
                result_encrypted,
                success: success == 1,
                pii_found: pii_found == 1,
                created_at,
                key_epoch: key_epoch as u64,
                payload_version,
            };
            let (arguments, result) = self.authenticate_current_row(&row)?;
            out.push(McpLogEntry {
                id: row.id,
                tool_name: row.tool_name,
                arguments: arguments.to_json_string().map_err(anyhow::Error::new)?,
                result: result.to_json_string().map_err(anyhow::Error::new)?,
                success: row.success,
                pii_found: row.pii_found,
                created_at: row.created_at,
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
        "dev" => "ai.openlife.app.dev",
        "qa" => "ai.openlife.app.qa",
        _ => "ai.openlife.app",
    };
    dirs::data_dir()
        .map(|d| d.join(app_dir_name))
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap()
                .join(format!(".{}", app_dir_name))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut new_config = AuditKeyConfig::default();
        new_config.mode = KeyMode::Passphrase;
        new_config.salt_b64 = Some(general_purpose::STANDARD.encode(b"newsalt123456789"));
        new_config.epoch = old_config.epoch + 1;
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
    fn audit_store_export_and_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        let old_timestamp = (chrono::Utc::now() - chrono::Duration::days(2)).to_rfc3339();
        store
            .insert_log_at(
                "tool_a",
                &serde_json::json!({}),
                "result",
                true,
                false,
                &old_timestamp,
            )
            .unwrap();

        let export = store.export_logs(30).unwrap();
        assert_eq!(export.entry_count, 1);
        assert_eq!(export.entries[0].tool_name, "tool_a");

        let cleaned = store
            .cleanup(
                1_i64
                    .try_into()
                    .unwrap_or_else(|_| panic!("one day is valid MCP audit retention")),
            )
            .unwrap();
        assert_eq!(cleaned, 1);
        let logs = store.list_logs(10).unwrap();
        assert!(logs.is_empty());
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
                    McpAuditStore::sqlite_key_epoch(store.key_config().epoch).unwrap(),
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
    fn pre_envelope_minimized_v1_rows_migrate_without_losing_history() {
        const PRIVATE_ARGUMENT: &str = "MEDICAL-NOTE-OLD-V1-1842";
        const PRIVATE_RESULT: &str = "FINANCE-NOTE-OLD-V1-7291";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        let expected_arguments =
            audit_arguments_receipt(&serde_json::json!({ "note": PRIVATE_ARGUMENT })).unwrap();
        let expected_result = audit_result_receipt(PRIVATE_RESULT);
        let arguments_encrypted = store.encrypt(&expected_arguments).unwrap();
        let result_encrypted = store.encrypt(&expected_result).unwrap();
        let configs = store.key_configs().to_vec();
        store
            .conn()
            .unwrap()
            .execute(
                "INSERT INTO mcp_log (
                    tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                    created_at, key_epoch, payload_minimized_version
                 ) VALUES (?1, ?2, ?3, 1, 1, ?4, ?5, ?6)",
                params![
                    "pre_envelope_v1_tool",
                    arguments_encrypted,
                    result_encrypted,
                    chrono::Utc::now().to_rfc3339(),
                    McpAuditStore::sqlite_key_epoch(store.key_config().epoch).unwrap(),
                    MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
                ],
            )
            .unwrap();
        drop(store);

        let restarted = McpAuditStore::with_keyring(&path, configs);
        let logs = restarted.list_logs(10).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].arguments, expected_arguments);
        assert_eq!(logs[0].result, expected_result);
        let serialized = serde_json::to_string(&logs).unwrap();
        assert!(!serialized.contains(PRIVATE_ARGUMENT));
        assert!(!serialized.contains(PRIVATE_RESULT));
        let record_id: String = restarted
            .conn()
            .unwrap()
            .query_row(
                "SELECT audit_record_id FROM mcp_log WHERE tool_name = 'pre_envelope_v1_tool'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let parsed = Uuid::parse_str(&record_id).unwrap();
        assert_eq!(parsed.get_version(), Some(uuid::Version::Random));
        assert_eq!(parsed.to_string(), record_id);
    }

    #[test]
    fn authenticated_record_identity_rejects_whole_row_replay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        store
            .insert_log(
                "replay_target",
                &serde_json::json!({"bounded": true}),
                "bounded-result",
                true,
                false,
            )
            .unwrap();
        let configs = store.key_configs().to_vec();
        let connection = store.conn().unwrap();
        connection
            .execute("DROP INDEX idx_mcp_log_audit_record_id", [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO mcp_log (
                    audit_record_id, tool_name, arguments_encrypted, result_encrypted,
                    success, pii_found, created_at, key_epoch, payload_minimized_version
                 )
                 SELECT audit_record_id, tool_name, arguments_encrypted, result_encrypted,
                        success, pii_found, created_at, key_epoch, payload_minimized_version
                 FROM mcp_log WHERE tool_name = 'replay_target'",
                [],
            )
            .unwrap();
        drop(connection);
        drop(store);

        let restarted = McpAuditStore::with_keyring(&path, configs);
        let error = restarted.list_logs(10).unwrap_err().to_string();
        assert!(error.contains("mcp_audit_record_id_duplicate"), "{error}");
        let connection = Connection::open(&path).unwrap();
        let row_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM mcp_log", [], |row| row.get(0))
            .unwrap();
        let index_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_mcp_log_audit_record_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 2);
        assert_eq!(index_count, 0, "failed startup must not repair or rewrite");
    }

    #[test]
    fn named_partial_identity_index_cannot_impersonate_the_full_uniqueness_guard() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        store
            .insert_log(
                "partial_index_target",
                &serde_json::json!({"bounded": true}),
                "bounded-result",
                true,
                false,
            )
            .unwrap();
        let configs = store.key_configs().to_vec();
        let connection = store.conn().unwrap();
        connection
            .execute("DROP INDEX idx_mcp_log_audit_record_id", [])
            .unwrap();
        connection
            .execute(
                "CREATE UNIQUE INDEX idx_mcp_log_audit_record_id
                 ON mcp_log(audit_record_id) WHERE 0",
                [],
            )
            .unwrap();
        drop(connection);
        drop(store);
        let before = std::fs::read(&path).unwrap();

        let restarted = McpAuditStore::with_keyring(&path, configs);
        let error = restarted.list_logs(10).unwrap_err().to_string();
        assert!(
            error.contains("mcp_audit_record_identity_index_invalid"),
            "{error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let connection = Connection::open(&path).unwrap();
        let partial: i64 = connection
            .query_row(
                "SELECT partial FROM pragma_index_list('mcp_log')
                 WHERE name = 'idx_mcp_log_audit_record_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(partial, 1, "failed startup must not replace the weak index");
    }

    #[test]
    fn missing_schema_registry_entry_is_repaired_only_after_payload_preflight() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        store
            .insert_log(
                "schema_registry_target",
                &serde_json::json!({"bounded": true}),
                "bounded-result",
                true,
                false,
            )
            .unwrap();
        let configs = store.key_configs().to_vec();
        drop(store);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "DELETE FROM openlife_schema_versions
                 WHERE component = 'mcp_audit_store'",
                [],
            )
            .unwrap();
        drop(connection);

        let restarted = McpAuditStore::with_keyring(&path, configs);
        assert_eq!(restarted.list_logs(10).unwrap().len(), 1);
        let version: i64 = restarted
            .conn()
            .unwrap()
            .query_row(
                "SELECT version FROM openlife_schema_versions
                 WHERE component = 'mcp_audit_store'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, MCP_AUDIT_SCHEMA_VERSION);
    }

    #[test]
    fn newer_schema_registry_version_fails_closed_without_downgrade() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        let configs = store.key_configs().to_vec();
        drop(store);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE openlife_schema_versions SET version = 999
                 WHERE component = 'mcp_audit_store'",
                [],
            )
            .unwrap();
        drop(connection);
        let before = std::fs::read(&path).unwrap();

        let restarted = McpAuditStore::with_keyring(&path, configs);
        let error = restarted.list_logs(10).unwrap_err().to_string();
        assert!(
            error.contains("mcp_audit_schema_version_newer_than_runtime"),
            "{error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let connection = Connection::open(&path).unwrap();
        let version: i64 = connection
            .query_row(
                "SELECT version FROM openlife_schema_versions
                 WHERE component = 'mcp_audit_store'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 999);
    }

    #[test]
    fn product_audit_writes_enforce_bounded_tool_identity_and_rfc3339_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        for invalid_name in [
            String::new(),
            "x".repeat(MCP_AUDIT_TOOL_NAME_MAX_BYTES + 1),
            "line\nbreak".to_string(),
        ] {
            assert!(store
                .insert_log(
                    &invalid_name,
                    &serde_json::json!({"bounded": true}),
                    "bounded-result",
                    true,
                    false,
                )
                .unwrap_err()
                .to_string()
                .contains("mcp_audit_tool_name_invalid"));
        }
        assert!(store
            .insert_log_at(
                "valid_tool",
                &serde_json::json!({"bounded": true}),
                "bounded-result",
                true,
                false,
                "not-a-time",
            )
            .unwrap_err()
            .to_string()
            .contains("mcp_audit_created_at_invalid"));
        assert!(store.list_logs(10).unwrap().is_empty());

        let max_name = "x".repeat(MCP_AUDIT_TOOL_NAME_MAX_BYTES);
        store
            .insert_log(
                &max_name,
                &serde_json::json!({"bounded": true}),
                "bounded-result",
                true,
                false,
            )
            .unwrap();
        assert_eq!(store.list_logs(10).unwrap().len(), 1);
    }

    #[test]
    fn invalid_legacy_row_metadata_fails_preflight_without_rewrite() {
        for (label, column, value, reason) in [
            (
                "tool_name",
                "tool_name",
                "x".repeat(MCP_AUDIT_TOOL_NAME_MAX_BYTES + 1),
                "mcp_audit_tool_name_exceeds_storage_bound",
            ),
            (
                "created_at_bytes",
                "created_at",
                "x".repeat(MCP_AUDIT_CREATED_AT_MAX_BYTES + 1),
                "mcp_audit_created_at_exceeds_storage_bound",
            ),
            (
                "created_at",
                "created_at",
                "not-a-time".to_string(),
                "mcp_audit_created_at_invalid",
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(format!("audit-{label}.db"));
            let store = McpAuditStore::new(&path);
            store
                .d068_insert_legacy_payload_fixture_for_test(
                    "legacy_metadata_target",
                    &serde_json::json!({"bounded": true}),
                    "bounded-result",
                )
                .unwrap();
            let configs = store.key_configs().to_vec();
            let connection = store.conn().unwrap();
            connection
                .execute(&format!("UPDATE mcp_log SET {column} = ?1"), [value])
                .unwrap();
            drop(connection);
            drop(store);
            let before = std::fs::read(&path).unwrap();

            let restarted = McpAuditStore::with_keyring(&path, configs);
            let error = restarted.list_logs(10).unwrap_err().to_string();
            assert!(error.contains(reason), "{label}: {error}");
            assert_eq!(std::fs::read(&path).unwrap(), before, "{label}");
        }
    }

    #[test]
    fn oversized_current_record_identity_fails_before_uuid_materialization() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit-record-id.db");
        let store = McpAuditStore::new(&path);
        let row_id = store
            .insert_log(
                "record_identity_target",
                &serde_json::json!({"bounded": true}),
                "bounded-result",
                true,
                false,
            )
            .unwrap();
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE mcp_log SET audit_record_id = ?1 WHERE id = ?2",
                params!["x".repeat(MCP_AUDIT_RECORD_ID_CANONICAL_BYTES + 1), row_id,],
            )
            .unwrap();

        let live_error = store.list_logs(10).unwrap_err().to_string();
        assert!(
            live_error.contains("mcp_audit_record_id_exceeds_storage_bound"),
            "{live_error}"
        );
        let configs = store.key_configs().to_vec();
        drop(store);
        let before = std::fs::read(&path).unwrap();
        let restarted = McpAuditStore::with_keyring(&path, configs);
        let restart_error = restarted.list_logs(10).unwrap_err().to_string();
        assert!(
            restart_error.contains("mcp_audit_record_id_exceeds_storage_bound"),
            "{restart_error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn oversized_current_ciphertext_is_rejected_at_the_sqlite_projection_boundary() {
        for column in ["arguments_encrypted", "result_encrypted"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(format!("audit-{column}.db"));
            let store = McpAuditStore::new(&path);
            let row_id = store
                .insert_log(
                    "bounded_ciphertext_target",
                    &serde_json::json!({"bounded": true}),
                    "bounded-result",
                    true,
                    false,
                )
                .unwrap();
            store
                .conn()
                .unwrap()
                .execute(
                    &format!("UPDATE mcp_log SET {column} = ?1 WHERE id = ?2"),
                    params!["A".repeat(MCP_AUDIT_MAX_ENVELOPE_ENCODED_BYTES + 1), row_id,],
                )
                .unwrap();

            let live_list_error = store.list_logs(10).unwrap_err().to_string();
            let live_export_error = store.export_logs(30).unwrap_err().to_string();
            assert!(
                live_list_error.contains("mcp_audit_current_ciphertext_exceeds_storage_bound"),
                "{column}: {live_list_error}"
            );
            assert!(
                live_export_error.contains("mcp_audit_current_ciphertext_exceeds_storage_bound"),
                "{column}: {live_export_error}"
            );

            let configs = store.key_configs().to_vec();
            drop(store);
            let before = std::fs::read(&path).unwrap();
            let restarted = McpAuditStore::with_keyring(&path, configs);
            let restart_error = restarted.list_logs(10).unwrap_err().to_string();
            assert!(
                restart_error.contains("mcp_audit_current_ciphertext_exceeds_storage_bound"),
                "{column}: {restart_error}"
            );
            assert_eq!(
                std::fs::read(&path).unwrap(),
                before,
                "{column}: failed preflight must not rewrite the SQLite file"
            );
        }
    }
}

#[cfg(test)]
#[path = "mcp_audit/d068_authenticated_payload_tests.rs"]
mod d068_authenticated_payload_tests;

/// Shareable handle.
pub type SharedMcpAuditStore = Arc<Mutex<McpAuditStore>>;
