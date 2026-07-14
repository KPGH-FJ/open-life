use anyhow::{Context, Result};
use chrono::Datelike;
use rusqlite::{params, Connection, StatementStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use ring::digest::{Context as DigestContext, SHA256};

const MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION: i64 = 1;

/// Product contract ceiling for MCP audit retention. The current raw `i64`
/// cleanup path does not enforce this yet; D063 keeps that behavior RED until
/// `McpAuditRetentionDays` becomes the domain mutation boundary.
pub const MCP_AUDIT_RETENTION_MAX_DAYS: i64 = 3_650;
pub const MCP_AUDIT_EXPORT_MAX_ENTRIES: usize = 10_000;
const MCP_AUDIT_EXPORT_CANDIDATE_LIMIT: usize = MCP_AUDIT_EXPORT_MAX_ENTRIES + 1;

fn strict_mcp_audit_rfc3339(value: &str) -> Result<chrono::DateTime<chrono::Utc>> {
    let bytes = value.as_bytes();
    if !value.is_ascii() || !(20..=64).contains(&bytes.len()) {
        anyhow::bail!("mcp_audit_created_at_invalid");
    }
    for index in [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18] {
        if !bytes[index].is_ascii_digit() {
            anyhow::bail!("mcp_audit_created_at_invalid");
        }
    }
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        anyhow::bail!("mcp_audit_created_at_invalid");
    }

    let suffix_start = if bytes[19] == b'.' {
        let suffix_start = bytes[20..]
            .iter()
            .position(|byte| matches!(byte, b'Z' | b'+' | b'-'))
            .map(|offset| offset + 20)
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_created_at_invalid"))?;
        if suffix_start == 20 || !bytes[20..suffix_start].iter().all(u8::is_ascii_digit) {
            anyhow::bail!("mcp_audit_created_at_invalid");
        }
        suffix_start
    } else {
        19
    };
    let suffix = &bytes[suffix_start..];
    let suffix_valid = suffix == b"Z"
        || (suffix.len() == 6
            && matches!(suffix[0], b'+' | b'-')
            && suffix[1].is_ascii_digit()
            && suffix[2].is_ascii_digit()
            && suffix[3] == b':'
            && suffix[4].is_ascii_digit()
            && suffix[5].is_ascii_digit());
    if !suffix_valid {
        anyhow::bail!("mcp_audit_created_at_invalid");
    }
    let seconds = u32::from(bytes[17] - b'0') * 10 + u32::from(bytes[18] - b'0');
    if seconds > 59 {
        anyhow::bail!("mcp_audit_created_at_invalid");
    }

    let timestamp = chrono::DateTime::parse_from_rfc3339(value)
        .map_err(|_| anyhow::anyhow!("mcp_audit_created_at_invalid"))?;
    if timestamp.year() < 1 {
        anyhow::bail!("mcp_audit_created_at_invalid");
    }
    Ok(timestamp.with_timezone(&chrono::Utc))
}

/// Validated product window for a bounded MCP audit export. Keeping the raw
/// `i64` outside the store prevents extreme WebView input from reaching
/// `chrono::Duration::days`, whose arithmetic is not an input validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpAuditExportDays(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpAuditExportDaysOutOfRange;

impl std::fmt::Display for McpAuditExportDaysOutOfRange {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("MCP audit export days must be between 1 and 3650")
    }
}

impl std::error::Error for McpAuditExportDaysOutOfRange {}

impl TryFrom<i64> for McpAuditExportDays {
    type Error = McpAuditExportDaysOutOfRange;

    fn try_from(value: i64) -> std::result::Result<Self, Self::Error> {
        if (1..=MCP_AUDIT_RETENTION_MAX_DAYS).contains(&value) {
            Ok(Self(value))
        } else {
            Err(McpAuditExportDaysOutOfRange)
        }
    }
}

impl McpAuditExportDays {
    pub fn get(self) -> i64 {
        self.0
    }
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditExportIncompleteReason {
    ScanLimit,
    EntryLimit,
    ScanAndEntryLimit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditExport {
    pub exported_at: String,
    pub entry_count: usize,
    pub days: i64,
    /// True only when every row in the requested time window is represented
    /// in `entries`. A bounded export must never imply completeness merely
    /// because it reached the product ceiling.
    pub complete: bool,
    /// Explicit inverse of `complete` for consumers that need to surface a
    /// partial-export warning without inferring it from `entry_count`.
    pub truncated: bool,
    pub incomplete_reason: Option<AuditExportIncompleteReason>,
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

struct PersistedMcpLogRow {
    id: i64,
    tool_name: String,
    arguments_encrypted: String,
    result_encrypted: String,
    success: bool,
    pii_found: bool,
    created_at: String,
    key_epoch: i64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct AuditSqlStatementStats {
    fullscan_steps: i32,
    sort_operations: i32,
    vm_steps: i32,
}

impl AuditSqlStatementStats {
    fn capture(statement: &rusqlite::Statement<'_>) -> Self {
        Self {
            fullscan_steps: statement.get_status(StatementStatus::FullscanStep),
            sort_operations: statement.get_status(StatementStatus::Sort),
            vm_steps: statement.get_status(StatementStatus::VmStep),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuditExportQueryStats {
    snapshot_max: AuditSqlStatementStats,
    candidate_scan: AuditSqlStatementStats,
    unscanned_probe: AuditSqlStatementStats,
    post_scan_max: AuditSqlStatementStats,
    candidate_rows: usize,
    snapshot_max_id: i64,
    post_scan_max_id: i64,
}

impl AuditExportQueryStats {
    #[cfg(test)]
    fn total_fullscan_steps(self) -> i32 {
        self.snapshot_max.fullscan_steps
            + self.candidate_scan.fullscan_steps
            + self.unscanned_probe.fullscan_steps
            + self.post_scan_max.fullscan_steps
    }

    #[cfg(test)]
    fn total_sort_operations(self) -> i32 {
        self.snapshot_max.sort_operations
            + self.candidate_scan.sort_operations
            + self.unscanned_probe.sort_operations
            + self.post_scan_max.sort_operations
    }

    #[cfg(test)]
    fn total_vm_steps(self) -> i32 {
        self.snapshot_max.vm_steps
            + self.candidate_scan.vm_steps
            + self.unscanned_probe.vm_steps
            + self.post_scan_max.vm_steps
    }
}

#[derive(Clone)]
pub struct AuditKeyMaterial {
    pub config: AuditKeyConfig,
    pub key: [u8; 32],
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

impl McpAuditStore {
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
        let mut configs = if configs.is_empty() {
            vec![AuditKeyConfig::default()]
        } else {
            configs
        };
        configs.sort_by_key(|config| config.epoch);
        configs.dedup_by_key(|config| config.epoch);
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
        mut materials: Vec<AuditKeyMaterial>,
    ) -> Result<Self> {
        if materials.is_empty() {
            anyhow::bail!("MCP audit key material is empty");
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
        let store = Self {
            db_path: db_path.into(),
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
        let key = material.key;
        let store = Self {
            db_path: db_path.into(),
            read_only: false,
            unavailable_reason: None,
            key,
            key_config: material.config.clone(),
            keyring: HashMap::from([(epoch, key)]),
            key_configs: vec![material.config],
        };
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
            keyring: materials
                .iter()
                .map(|material| (material.config.epoch, material.key))
                .collect(),
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
    pub fn export_logs(&self, window: McpAuditExportDays) -> Result<AuditExport> {
        self.export_logs_with_query_stats(window)
            .map(|(export, _stats)| export)
    }

    fn export_logs_with_query_stats(
        &self,
        window: McpAuditExportDays,
    ) -> Result<(AuditExport, AuditExportQueryStats)> {
        self.export_logs_with_query_stats_and_hook(window, None, || {})
    }

    #[cfg(test)]
    fn export_logs_at_with_query_stats(
        &self,
        window: McpAuditExportDays,
        exported_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(AuditExport, AuditExportQueryStats)> {
        self.export_logs_with_query_stats_and_hook(window, Some(exported_at), || {})
    }

    fn export_logs_with_query_stats_and_hook<AfterScan>(
        &self,
        window: McpAuditExportDays,
        exported_at_override: Option<chrono::DateTime<chrono::Utc>>,
        after_scan: AfterScan,
    ) -> Result<(AuditExport, AuditExportQueryStats)>
    where
        AfterScan: FnOnce(),
    {
        let days = window.get();
        let mut conn = self.conn()?;
        let transaction = conn.transaction()?;
        // MAX(id) is the first read in the transaction and therefore anchors
        // the SQLite snapshot. `exported_at` is recorded immediately after
        // that snapshot exists, before any candidate rows are materialized.
        let mut snapshot_statement =
            transaction.prepare("SELECT COALESCE(MAX(id), 0) FROM mcp_log")?;
        let snapshot_max_id = snapshot_statement.query_row([], |row| row.get::<_, i64>(0))?;
        let snapshot_max_stats = AuditSqlStatementStats::capture(&snapshot_statement);
        drop(snapshot_statement);
        let exported_at = exported_at_override.unwrap_or_else(chrono::Utc::now);
        let cutoff = exported_at
            .checked_sub_signed(chrono::Duration::days(days))
            .ok_or_else(|| anyhow::anyhow!("mcp_audit_export_cutoff_out_of_range"))?;
        let mut statement = transaction.prepare(
            "SELECT id, tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                    created_at, key_epoch
             FROM mcp_log
             WHERE id <= ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let mut rows = statement.query_map(
            params![snapshot_max_id, MCP_AUDIT_EXPORT_CANDIDATE_LIMIT as i64],
            Self::persisted_log_row,
        )?;
        let mut scanned_rows = Vec::with_capacity(MCP_AUDIT_EXPORT_CANDIDATE_LIMIT);
        for row in &mut rows {
            scanned_rows.push(row?);
        }
        drop(rows);
        let query_stats = AuditExportQueryStats {
            snapshot_max: snapshot_max_stats,
            candidate_scan: AuditSqlStatementStats::capture(&statement),
            unscanned_probe: AuditSqlStatementStats::default(),
            post_scan_max: AuditSqlStatementStats::default(),
            candidate_rows: scanned_rows.len(),
            snapshot_max_id,
            post_scan_max_id: snapshot_max_id,
        };
        drop(statement);
        after_scan();
        let (has_unscanned_rows, unscanned_probe_stats) =
            if scanned_rows.len() == MCP_AUDIT_EXPORT_CANDIDATE_LIMIT {
                let last_scanned_id = scanned_rows.last().expect("candidate limit is non-zero").id;
                let mut unscanned_statement = transaction
                    .prepare("SELECT EXISTS(SELECT 1 FROM mcp_log WHERE id < ?1 LIMIT 1)")?;
                let has_unscanned_rows = unscanned_statement
                    .query_row([last_scanned_id], |row| row.get::<_, bool>(0))?;
                let stats = AuditSqlStatementStats::capture(&unscanned_statement);
                drop(unscanned_statement);
                (has_unscanned_rows, stats)
            } else {
                (false, AuditSqlStatementStats::default())
            };
        let mut post_scan_statement =
            transaction.prepare("SELECT COALESCE(MAX(id), 0) FROM mcp_log")?;
        let post_scan_max_id = post_scan_statement.query_row([], |row| row.get::<_, i64>(0))?;
        let post_scan_max_stats = AuditSqlStatementStats::capture(&post_scan_statement);
        drop(post_scan_statement);
        if post_scan_max_id != snapshot_max_id {
            anyhow::bail!("mcp_audit_export_snapshot_changed");
        }
        let query_stats = AuditExportQueryStats {
            unscanned_probe: unscanned_probe_stats,
            post_scan_max: post_scan_max_stats,
            post_scan_max_id,
            ..query_stats
        };
        transaction.commit()?;

        let mut eligible_rows = Vec::with_capacity(scanned_rows.len());
        for row in scanned_rows {
            let created_at = strict_mcp_audit_rfc3339(&row.created_at)
                .with_context(|| format!("validate MCP audit created_at for row {}", row.id))?;
            if created_at >= cutoff {
                eligible_rows.push(row);
            }
        }
        let entry_limited = eligible_rows.len() > MCP_AUDIT_EXPORT_MAX_ENTRIES;
        eligible_rows.truncate(MCP_AUDIT_EXPORT_MAX_ENTRIES);
        let mut entries = Vec::with_capacity(eligible_rows.len());
        for row in eligible_rows {
            let entry = self.decrypt_persisted_log_row(row)?;
            entries.push(ExportedAuditEntry {
                id: entry.id,
                tool_name: entry.tool_name,
                arguments: entry.arguments,
                result: entry.result,
                success: entry.success,
                pii_found: entry.pii_found,
                created_at: entry.created_at,
            });
        }
        let incomplete_reason = match (has_unscanned_rows, entry_limited) {
            (false, false) => None,
            (true, false) => Some(AuditExportIncompleteReason::ScanLimit),
            (false, true) => Some(AuditExportIncompleteReason::EntryLimit),
            (true, true) => Some(AuditExportIncompleteReason::ScanAndEntryLimit),
        };
        let truncated = incomplete_reason.is_some();

        Ok((
            AuditExport {
                exported_at: exported_at.to_rfc3339(),
                entry_count: entries.len(),
                days,
                complete: !truncated,
                truncated,
                incomplete_reason,
                entries,
            },
            query_stats,
        ))
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

    /// D068 RED-fixture seam for the exact historical, pre-envelope wire
    /// format. Keep this test-only helper bound to the production legacy
    /// decoder when the authenticated current envelope is introduced.
    #[cfg(any(test, feature = "test-utils"))]
    fn d068_encrypt_legacy_payload_fixture_for_test(&self, plaintext: &str) -> Result<String> {
        self.encrypt(plaintext)
    }

    /// D068 RED-fixture seam for a current-format encrypted payload. The RED
    /// implementation cannot yet bind the role or format version; the target
    /// implementation must route this through the same authenticated envelope
    /// encoder used by `insert_log`. A valid-control test prevents this helper
    /// from being left on the legacy encoder while invalid-envelope tests are
    /// made green by rejecting every fixture.
    #[cfg(test)]
    fn d068_encrypt_current_payload_fixture_for_test(
        &self,
        role: &str,
        format_version: i64,
        receipt_json: &str,
    ) -> Result<String> {
        let _ = (role, format_version);
        self.encrypt(receipt_json)
    }

    fn decrypt(&self, combined_b64: &str) -> Result<String> {
        self.decrypt_with_key(combined_b64, &self.key)
    }

    fn decrypt_for_epoch(&self, combined_b64: &str, key_epoch: u64) -> Result<String> {
        if let Some(key) = self.keyring.get(&key_epoch) {
            return self.decrypt_with_key(combined_b64, key);
        }
        self.decrypt(combined_b64)
    }

    /// Decrypt a canonical persisted row only with the key explicitly named by
    /// that row. Unlike the legacy migration helper above, this read path must
    /// never reinterpret a missing or malformed epoch as the active key.
    fn decrypt_for_persisted_epoch(
        &self,
        combined_b64: &str,
        persisted_key_epoch: i64,
    ) -> Result<String> {
        let key_epoch = u64::try_from(persisted_key_epoch).with_context(|| {
            format!("MCP audit row has negative key epoch {persisted_key_epoch}")
        })?;
        let key = self.keyring.get(&key_epoch).ok_or_else(|| {
            anyhow::anyhow!("MCP audit row key epoch {key_epoch} is not covered by the keyring")
        })?;
        self.decrypt_with_key(combined_b64, key)
    }

    fn decrypt_with_key(&self, combined_b64: &str, key: &[u8; 32]) -> Result<String> {
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
                        row.get::<_, i64>(3)?.max(0) as u64,
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
            let arguments_plaintext = self
                .decrypt_for_epoch(&arguments_encrypted, key_epoch)
                .unwrap_or(arguments_encrypted);
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
                .unwrap_or(result_encrypted);
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
                self.key_config.epoch as i64,
                MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            ],
        )?;
        Ok(conn.last_insert_rowid())
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
                self.key_config.epoch as i64,
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

    fn persisted_log_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PersistedMcpLogRow> {
        Ok(PersistedMcpLogRow {
            id: row.get(0)?,
            tool_name: row.get(1)?,
            arguments_encrypted: row.get(2)?,
            result_encrypted: row.get(3)?,
            success: row.get::<_, i32>(4)? != 0,
            pii_found: row.get::<_, i32>(5)? != 0,
            created_at: row.get(6)?,
            key_epoch: row.get(7)?,
        })
    }

    fn decrypt_persisted_log_row(&self, row: PersistedMcpLogRow) -> Result<McpLogEntry> {
        let arguments = self
            .decrypt_for_persisted_epoch(&row.arguments_encrypted, row.key_epoch)
            .with_context(|| {
                format!(
                    "decrypt MCP audit arguments for row {} at key epoch {}",
                    row.id, row.key_epoch
                )
            })?;
        let result = self
            .decrypt_for_persisted_epoch(&row.result_encrypted, row.key_epoch)
            .with_context(|| {
                format!(
                    "decrypt MCP audit result for row {} at key epoch {}",
                    row.id, row.key_epoch
                )
            })?;
        Ok(McpLogEntry {
            id: row.id,
            tool_name: row.tool_name,
            arguments,
            result,
            success: row.success,
            pii_found: row.pii_found,
            created_at: row.created_at,
        })
    }

    pub fn list_logs(&self, limit: usize) -> Result<Vec<McpLogEntry>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, tool_name, arguments_encrypted, result_encrypted, success, pii_found, created_at, key_epoch
             FROM mcp_log
             ORDER BY id DESC
             LIMIT ?1"
        )?;
        let rows = stmt.query_map([limit], Self::persisted_log_row)?;

        let mut out = Vec::new();
        for row in rows {
            out.push(self.decrypt_persisted_log_row(row?)?);
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

    fn insert_receipt_rows(store: &McpAuditStore, prefix: &str, count: usize, created_at: &str) {
        let arguments_encrypted = store
            .encrypt(&audit_arguments_receipt(&serde_json::json!({"bounded": true})).unwrap())
            .unwrap();
        let result_encrypted = store
            .encrypt(&audit_result_receipt("bounded-result"))
            .unwrap();
        let key_epoch = store.key_config().epoch as i64;
        let mut connection = store.conn().unwrap();
        let transaction = connection.transaction().unwrap();
        {
            let mut insert = transaction
                .prepare(
                    "INSERT INTO mcp_log (
                        tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                        created_at, key_epoch, payload_minimized_version
                     ) VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?6)",
                )
                .unwrap();
            for index in 0..count {
                insert
                    .execute(params![
                        format!("{prefix}-{index}"),
                        &arguments_encrypted,
                        &result_encrypted,
                        created_at,
                        key_epoch,
                        MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
                    ])
                    .unwrap();
            }
        }
        transaction.commit().unwrap();
    }

    fn set_created_at(store: &McpAuditStore, tool_name: &str, created_at: &str) {
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE mcp_log SET created_at = ?1 WHERE tool_name = ?2",
                params![created_at, tool_name],
            )
            .unwrap();
    }

    #[test]
    fn minimized_receipt_digest_is_canonical_sha256_standard_base64_without_padding() {
        let receipt: Value = serde_json::from_str(&audit_result_receipt("canonical-digest-probe"))
            .expect("parse minimized audit receipt");
        let digest = receipt["digest"]
            .as_str()
            .expect("receipt digest is a string");
        let encoded = digest
            .strip_prefix("sha256:")
            .expect("receipt digest algorithm prefix");
        let decoded = general_purpose::STANDARD_NO_PAD
            .decode(encoded)
            .expect("receipt digest is unpadded standard Base64");

        assert_eq!(decoded.len(), 32);
        assert_eq!(general_purpose::STANDARD_NO_PAD.encode(&decoded), encoded);
        assert!(!encoded
            .chars()
            .any(|value| matches!(value, '-' | '_' | '=')));
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
    fn audit_store_export_and_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        store
            .insert_log("tool_a", &serde_json::json!({}), "result", true, false)
            .unwrap();
        let old_timestamp = (chrono::Utc::now() - chrono::Duration::days(2)).to_rfc3339();
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE mcp_log SET created_at = ?1 WHERE tool_name = ?2",
                params![old_timestamp, "tool_a"],
            )
            .unwrap();

        let export = store
            .export_logs(McpAuditExportDays::try_from(30).unwrap())
            .unwrap();
        assert_eq!(export.entry_count, 1);
        assert!(export.complete);
        assert!(!export.truncated);
        assert_eq!(export.incomplete_reason, None);
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
    fn audit_export_reports_exact_completeness_at_and_above_the_entry_ceiling() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("audit.db");
        let store = McpAuditStore::new(&path);
        let created_at = chrono::Utc::now().to_rfc3339();
        insert_receipt_rows(
            &store,
            "bounded-export",
            MCP_AUDIT_EXPORT_MAX_ENTRIES,
            &created_at,
        );

        let exact = store
            .export_logs(McpAuditExportDays::try_from(30).unwrap())
            .unwrap();
        assert_eq!(exact.entry_count, MCP_AUDIT_EXPORT_MAX_ENTRIES);
        assert_eq!(exact.entries.len(), MCP_AUDIT_EXPORT_MAX_ENTRIES);
        assert!(exact.complete);
        assert!(!exact.truncated);
        assert_eq!(exact.incomplete_reason, None);
        assert_eq!(exact.entries[0].tool_name, "bounded-export-9999");
        assert_eq!(exact.entries[9_999].tool_name, "bounded-export-0");

        store
            .insert_log(
                "bounded-export-overflow",
                &serde_json::json!({"bounded": true}),
                "bounded-result",
                true,
                false,
            )
            .unwrap();
        let overflow = store
            .export_logs(McpAuditExportDays::try_from(30).unwrap())
            .unwrap();
        assert_eq!(overflow.entry_count, MCP_AUDIT_EXPORT_MAX_ENTRIES);
        assert_eq!(overflow.entries.len(), MCP_AUDIT_EXPORT_MAX_ENTRIES);
        assert!(!overflow.complete);
        assert!(overflow.truncated);
        assert_eq!(
            overflow.incomplete_reason,
            Some(AuditExportIncompleteReason::EntryLimit)
        );
        assert_eq!(overflow.entries[0].tool_name, "bounded-export-overflow");
        assert_eq!(overflow.entries[9_999].tool_name, "bounded-export-1");
        assert!(
            overflow
                .entries
                .iter()
                .all(|entry| entry.tool_name != "bounded-export-0"),
            "the 10001st candidate must be omitted only with explicit truncation truth"
        );
    }

    #[test]
    fn audit_timestamp_parser_preserves_submicrosecond_precision_and_extreme_years() {
        let before = strict_mcp_audit_rfc3339("2026-07-13T00:00:00.0000001Z").unwrap();
        let after = strict_mcp_audit_rfc3339("2026-07-13T00:00:00.0000009Z").unwrap();
        assert!(
            before < after,
            "sub-microsecond ordering must not be truncated"
        );

        assert!(strict_mcp_audit_rfc3339("0001-01-01T00:00:00Z").is_ok());
        assert!(strict_mcp_audit_rfc3339("9999-12-31T23:59:59.999999999Z").is_ok());
        for invalid in [
            "2026-02-29T00:00:00Z",
            "2026-07-14T00:00:00",
            "2026-07-14t00:00:00z",
            "2026-07-14T00:00:60Z",
            "2026-07-14T00:00:00+24:00",
        ] {
            assert!(
                strict_mcp_audit_rfc3339(invalid).is_err(),
                "invalid timestamp must fail closed: {invalid}"
            );
        }
    }

    #[test]
    fn audit_export_filters_offsets_and_submicroseconds_against_the_exact_cutoff() {
        let directory = tempfile::tempdir().unwrap();
        let store = McpAuditStore::new(directory.path().join("audit.db"));
        let exported_at = chrono::DateTime::parse_from_rfc3339("2026-07-14T00:00:00.000000500Z")
            .unwrap()
            .with_timezone(&chrono::Utc);

        insert_receipt_rows(
            &store,
            "before-submicrosecond-cutoff",
            1,
            "2026-07-13T00:00:00.0000001Z",
        );
        insert_receipt_rows(
            &store,
            "after-submicrosecond-cutoff",
            1,
            "2026-07-13T00:00:00.0000009Z",
        );
        insert_receipt_rows(
            &store,
            "equal-cutoff-with-offset",
            1,
            "2026-07-12T19:00:00.000000500-05:00",
        );
        insert_receipt_rows(
            &store,
            "before-cutoff-with-offset",
            1,
            "2026-07-12T18:59:59.999999999-05:00",
        );

        let (export, _stats) = store
            .export_logs_at_with_query_stats(McpAuditExportDays::try_from(1).unwrap(), exported_at)
            .unwrap();
        let names = export
            .entries
            .iter()
            .map(|entry| entry.tool_name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "equal-cutoff-with-offset-0",
                "after-submicrosecond-cutoff-0"
            ]
        );
        assert!(export.complete);
        assert!(!export.truncated);
        assert_eq!(export.incomplete_reason, None);
    }

    #[test]
    fn malformed_timestamp_in_the_bounded_scan_fails_export_closed() {
        let directory = tempfile::tempdir().unwrap();
        let store = McpAuditStore::new(directory.path().join("audit.db"));
        store
            .insert_log(
                "malformed-created-at",
                &serde_json::json!({}),
                "result",
                true,
                false,
            )
            .unwrap();
        set_created_at(&store, "malformed-created-at", "2026-02-29T00:00:00Z");

        let error = store
            .export_logs(McpAuditExportDays::try_from(30).unwrap())
            .expect_err("malformed canonical timestamps must never become silent exclusions")
            .to_string();

        assert!(error.contains("validate MCP audit created_at for row"));
    }

    #[test]
    fn audit_export_bounds_the_scan_and_never_hides_eligible_rows_as_complete() {
        let directory = tempfile::tempdir().unwrap();
        let store = McpAuditStore::new(directory.path().join("audit.db"));
        let exported_at = chrono::DateTime::parse_from_rfc3339("2026-07-14T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let window = McpAuditExportDays::try_from(30).unwrap();

        insert_receipt_rows(
            &store,
            "hidden-eligible",
            MCP_AUDIT_EXPORT_CANDIDATE_LIMIT,
            "2026-07-13T00:00:00Z",
        );
        let (entry_limited, baseline_stats) = store
            .export_logs_at_with_query_stats(window, exported_at)
            .unwrap();
        assert_eq!(entry_limited.entry_count, MCP_AUDIT_EXPORT_MAX_ENTRIES);
        assert_eq!(
            entry_limited.incomplete_reason,
            Some(AuditExportIncompleteReason::EntryLimit)
        );
        assert_eq!(
            baseline_stats.candidate_rows,
            MCP_AUDIT_EXPORT_CANDIDATE_LIMIT
        );
        assert_eq!(baseline_stats.total_sort_operations(), 0);
        assert!(baseline_stats.snapshot_max.vm_steps > 0);
        assert!(baseline_stats.candidate_scan.vm_steps > 0);
        assert!(baseline_stats.unscanned_probe.vm_steps > 0);
        assert!(baseline_stats.post_scan_max.vm_steps > 0);

        insert_receipt_rows(
            &store,
            "newer-outside-window",
            20_000,
            "2020-01-01T00:00:00Z",
        );
        let (scan_limited, expanded_stats) = store
            .export_logs_at_with_query_stats(window, exported_at)
            .unwrap();

        assert_eq!(scan_limited.entry_count, 0);
        assert!(scan_limited.entries.is_empty());
        assert!(!scan_limited.complete);
        assert!(scan_limited.truncated);
        assert_eq!(
            scan_limited.incomplete_reason,
            Some(AuditExportIncompleteReason::ScanLimit)
        );
        assert_eq!(
            expanded_stats.candidate_rows,
            MCP_AUDIT_EXPORT_CANDIDATE_LIMIT
        );
        assert_eq!(expanded_stats.total_sort_operations(), 0);
        assert_eq!(
            expanded_stats.candidate_scan,
            baseline_stats.candidate_scan,
            "the bounded candidate statement must do identical work after 20000 unscanned rows are added"
        );
        assert_eq!(
            expanded_stats.total_fullscan_steps(),
            baseline_stats.total_fullscan_steps(),
            "the complete selector path must not add linear fullscan work beyond the fixed ceiling"
        );
        assert!(
            expanded_stats.total_vm_steps() <= baseline_stats.total_vm_steps() + 16,
            "snapshot, candidate, completeness and post-snapshot VM work must remain constant/bounded: baseline={baseline_stats:?}, expanded={expanded_stats:?}"
        );
    }

    #[test]
    fn audit_export_completeness_uses_one_wal_snapshot_during_a_concurrent_insert() {
        let directory = tempfile::tempdir().unwrap();
        let store = McpAuditStore::new(directory.path().join("audit.db"));
        let journal_mode = store
            .conn()
            .unwrap()
            .query_row("PRAGMA journal_mode = WAL", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        insert_receipt_rows(
            &store,
            "snapshot-old",
            MCP_AUDIT_EXPORT_MAX_ENTRIES,
            "2020-01-01T00:00:00Z",
        );
        insert_receipt_rows(
            &store,
            "snapshot-visible",
            1,
            &chrono::Utc::now().to_rfc3339(),
        );

        let (scanned_tx, scanned_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let export_store = store.clone();
        let before_export = chrono::Utc::now();
        let export_thread = std::thread::spawn(move || {
            export_store.export_logs_with_query_stats_and_hook(
                McpAuditExportDays::try_from(30).unwrap(),
                None,
                || {
                    scanned_tx.send(()).unwrap();
                    release_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .expect("release snapshot export after concurrent insert");
                },
            )
        });

        scanned_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("export reached the post-scan snapshot barrier");
        let after_scan = chrono::Utc::now();
        store
            .insert_log(
                "snapshot-concurrent-insert",
                &serde_json::json!({}),
                "result",
                true,
                false,
            )
            .expect("WAL writer commits while the export read snapshot remains open");
        release_tx.send(()).unwrap();

        let (export, stats) = export_thread.join().unwrap().unwrap();
        let recorded_exported_at = strict_mcp_audit_rfc3339(&export.exported_at).unwrap();
        assert!(recorded_exported_at >= before_export);
        assert!(recorded_exported_at <= after_scan);
        assert_eq!(stats.snapshot_max_id, stats.post_scan_max_id);
        assert!(export.complete);
        assert!(!export.truncated);
        assert_eq!(export.incomplete_reason, None);
        assert_eq!(export.entry_count, 1);
        assert_eq!(export.entries[0].tool_name, "snapshot-visible-0");
        assert!(export
            .entries
            .iter()
            .all(|entry| entry.tool_name != "snapshot-concurrent-insert"));
        assert_eq!(
            store.list_logs(1).unwrap()[0].tool_name,
            "snapshot-concurrent-insert",
            "the control proves the concurrent row committed outside the export snapshot"
        );
    }

    #[test]
    fn d063_rfc3339_text_cleanup_predicate_has_a_timezone_offset_counterexample() {
        let cutoff = "2026-07-14T00:00:00+00:00";
        let chronologically_newer = "2026-07-13T20:00:00-05:00";
        assert!(
            strict_mcp_audit_rfc3339(chronologically_newer).unwrap()
                > strict_mcp_audit_rfc3339(cutoff).unwrap(),
            "control: the candidate row is one hour newer than the cutoff"
        );
        assert!(
            chronologically_newer < cutoff,
            "the same RFC3339 values sort in the opposite order as TEXT"
        );

        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute("CREATE TABLE audit(created_at TEXT NOT NULL)", [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO audit(created_at) VALUES (?1)",
                [chronologically_newer],
            )
            .unwrap();
        let wrongly_removed = connection
            .execute("DELETE FROM audit WHERE created_at < ?1", [cutoff])
            .unwrap();
        assert_eq!(
            wrongly_removed, 1,
            "D063 remains open: SQLite TEXT ordering can delete a chronologically newer offset timestamp"
        );
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
}

#[cfg(test)]
#[path = "mcp_audit/d068_authenticated_payload_tests.rs"]
mod d068_authenticated_payload_tests;

/// Shareable handle.
pub type SharedMcpAuditStore = Arc<Mutex<McpAuditStore>>;
