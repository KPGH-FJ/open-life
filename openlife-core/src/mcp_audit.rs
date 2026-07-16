use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use ring::digest::{Context as DigestContext, SHA256};

const MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION: i64 = 1;

fn audit_payload_receipt(kind: &str, value_type: &str, bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&SHA256, bytes);
    serde_json::json!({
        "kind": kind,
        "payloadStored": false,
        "valueType": value_type,
        "bytes": bytes.len(),
        "digest": format!(
            "sha256:{}",
            general_purpose::STANDARD_NO_PAD.encode(digest.as_ref())
        ),
    })
    .to_string()
}

fn audit_arguments_receipt(arguments: &Value) -> Result<String> {
    let encoded = serde_json::to_vec(arguments).context("serialize MCP argument receipt input")?;
    let value_type = match arguments {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    Ok(audit_payload_receipt("arguments", value_type, &encoded))
}

fn audit_result_receipt(result: &str) -> String {
    audit_payload_receipt("result", "string", result.as_bytes())
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

fn decrypt_mcp_audit_ciphertext(combined_b64: &str, key: &[u8; 32]) -> Result<String> {
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
        .decrypt(nonce, ciphertext)
        .map_err(|error| anyhow::anyhow!("decrypt failed: {error:?}"))?;
    String::from_utf8(plaintext).context("utf8 decode")
}

fn validate_mcp_audit_ciphertext_row(
    id: i64,
    arguments_encrypted: &str,
    result_encrypted: &str,
    key_epoch: i64,
    keyring: &HashMap<u64, [u8; 32]>,
) -> Result<u64> {
    let key_epoch = u64::try_from(key_epoch)
        .with_context(|| format!("MCP audit row {id} contains a negative key epoch"))?;
    let key = keyring.get(&key_epoch).ok_or_else(|| {
        anyhow::anyhow!("MCP audit row {id} requires unavailable key epoch {key_epoch}")
    })?;
    decrypt_mcp_audit_ciphertext(arguments_encrypted, key).with_context(|| {
        format!("authenticate MCP audit arguments for row {id} epoch {key_epoch}")
    })?;
    decrypt_mcp_audit_ciphertext(result_encrypted, key)
        .with_context(|| format!("authenticate MCP audit result for row {id} epoch {key_epoch}"))?;
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
        let pending_predicate = if columns.contains("payload_minimized_version") {
            format!(" WHERE payload_minimized_version < {MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION}")
        } else {
            String::new()
        };
        let pending_sql = format!(
            "SELECT id, arguments_encrypted, result_encrypted, {epoch_expression} FROM mcp_log{pending_predicate} ORDER BY id ASC"
        );
        let mut pending_statement = conn.prepare(&pending_sql)?;
        let pending_rows = pending_statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let mut authenticated_epochs = HashSet::new();
        for row in pending_rows {
            let (id, arguments, result, epoch) = row?;
            authenticated_epochs.insert(validate_mcp_audit_ciphertext_row(
                id, &arguments, &result, epoch, &keyring,
            )?);
        }
        for epoch in &inspection.key_epochs {
            if authenticated_epochs.contains(epoch) {
                continue;
            }
            let sample_sql = if columns.contains("key_epoch") {
                "SELECT id, arguments_encrypted, result_encrypted, key_epoch FROM mcp_log WHERE key_epoch = ?1 ORDER BY id ASC LIMIT 1"
            } else {
                "SELECT id, arguments_encrypted, result_encrypted, 0 FROM mcp_log ORDER BY id ASC LIMIT 1"
            };
            let mut statement = conn.prepare(sample_sql)?;
            if columns.contains("key_epoch") {
                let epoch_sql = i64::try_from(*epoch)
                    .context("MCP audit key epoch exceeds SQLite integer range")?;
                let row = statement.query_row([epoch_sql], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })?;
                validate_mcp_audit_ciphertext_row(row.0, &row.1, &row.2, row.3, &keyring)?;
            } else {
                let row = statement.query_row([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })?;
                validate_mcp_audit_ciphertext_row(row.0, &row.1, &row.2, row.3, &keyring)?;
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
        crate::sqlite_migration::record_schema_version(&tx, "mcp_audit_store", 3)?;
        tx.commit()?;
        self.migrate_legacy_payloads()?;
        Ok(())
    }

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

    fn decrypt_for_epoch(&self, combined_b64: &str, key_epoch: u64) -> Result<String> {
        let key = self
            .keyring
            .get(&key_epoch)
            .ok_or_else(|| anyhow::anyhow!("MCP audit key epoch {key_epoch} is unavailable"))?;
        self.decrypt_with_key(combined_b64, key)
    }

    fn decrypt_with_key(&self, combined_b64: &str, key: &[u8; 32]) -> Result<String> {
        decrypt_mcp_audit_ciphertext(combined_b64, key)
    }

    fn migrate_legacy_payloads(&self) -> Result<()> {
        let mut conn = self.conn()?;
        let rows = {
            let mut statement = conn.prepare(
                "SELECT id, arguments_encrypted, result_encrypted, key_epoch
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
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        if rows.is_empty() {
            return Ok(());
        }

        let mut migrated = Vec::with_capacity(rows.len());
        for (id, arguments_encrypted, result_encrypted, key_epoch) in rows {
            let key_epoch = u64::try_from(key_epoch).with_context(|| {
                format!("legacy MCP audit row {id} contains a negative key epoch")
            })?;
            let arguments_plaintext = self
                .decrypt_for_epoch(&arguments_encrypted, key_epoch)
                .with_context(|| {
                    format!("decrypt legacy MCP audit arguments for row {id} epoch {key_epoch}")
                })?;
            let arguments_receipt = serde_json::from_str::<Value>(&arguments_plaintext)
                .ok()
                .map(|value| audit_arguments_receipt(&value))
                .transpose()?
                .unwrap_or_else(|| {
                    audit_payload_receipt(
                        "arguments",
                        "unparseable_legacy",
                        arguments_plaintext.as_bytes(),
                    )
                });
            let result_plaintext = self
                .decrypt_for_epoch(&result_encrypted, key_epoch)
                .with_context(|| {
                    format!("decrypt legacy MCP audit result for row {id} epoch {key_epoch}")
                })?;
            migrated.push((
                id,
                self.encrypt(&arguments_receipt)?,
                self.encrypt(&audit_result_receipt(&result_plaintext))?,
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
        let args_enc = self.encrypt(&audit_arguments_receipt(arguments)?)?;
        let res_enc = self.encrypt(&audit_result_receipt(result))?;
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
            "SELECT id, tool_name, arguments_encrypted, result_encrypted, success, pii_found, created_at, key_epoch
             FROM mcp_log
             ORDER BY id DESC
             LIMIT ?1"
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
            Ok((
                id,
                tool_name,
                args_enc,
                res_enc,
                success != 0,
                pii_found != 0,
                created_at,
                key_epoch,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (id, tool_name, args_enc, res_enc, success, pii_found, created_at, key_epoch) = r?;
            let key_epoch = u64::try_from(key_epoch)
                .with_context(|| format!("MCP audit row {id} contains a negative key epoch"))?;
            let arguments = self
                .decrypt_for_epoch(&args_enc, key_epoch)
                .with_context(|| {
                    format!("decrypt MCP audit arguments for row {id} epoch {key_epoch}")
                })?;
            let result = self
                .decrypt_for_epoch(&res_enc, key_epoch)
                .with_context(|| {
                    format!("decrypt MCP audit result for row {id} epoch {key_epoch}")
                })?;
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
        let before = std::fs::read(&path).unwrap();
        let wrong = keychain_material(9, [0x42; 32]);

        let error = match McpAuditStore::with_key_materials(&path, vec![wrong]) {
            Ok(_) => panic!("wrong key material must fail before writable open"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("authenticate MCP audit"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
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
}

/// Shareable handle.
pub type SharedMcpAuditStore = Arc<Mutex<McpAuditStore>>;
