use anyhow::{Context, Result};
use rusqlite::{params, Connection};
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
}

/// Key management configuration for MCP audit logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditKeyConfig {
    pub mode: KeyMode,
    /// For Passphrase mode: base64-encoded salt (16 bytes)
    pub salt_b64: Option<String>,
    /// For Env mode: environment variable name
    pub env_var: Option<String>,
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

/// Encrypted SQLite-backed store for MCP call logs with configurable key management.
pub struct McpAuditStore {
    db_path: PathBuf,
    key: [u8; 32],
    key_config: AuditKeyConfig,
    keyring: HashMap<u64, [u8; 32]>,
    key_configs: Vec<AuditKeyConfig>,
}

impl McpAuditStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let config = AuditKeyConfig::default();
        Self::with_config(db_path, config)
    }

    pub fn with_config(db_path: impl Into<PathBuf>, config: AuditKeyConfig) -> Self {
        Self::with_keyring(db_path, vec![config])
    }

    pub fn with_keyring(db_path: impl Into<PathBuf>, configs: Vec<AuditKeyConfig>) -> Self {
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
            key,
            key_config: config,
            keyring,
            key_configs: configs,
        };
        let _ = store.init_tables();
        store
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
        }
    }

    /// Rotate to a new key configuration for future writes.
    ///
    /// Existing entries remain readable while this store is initialized with
    /// the full keyring returned by `key_configs`.
    pub fn rotate_key(&mut self, new_config: AuditKeyConfig) {
        let new_key = Self::derive_key(&new_config);
        self.key = new_key;
        self.key_config = new_config;
        self.keyring.insert(self.key_config.epoch, new_key);
        self.key_configs
            .retain(|config| config.epoch != self.key_config.epoch);
        self.key_configs.push(self.key_config.clone());
        self.key_configs.sort_by_key(|config| config.epoch);
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
        Connection::open(&self.db_path).context("open mcp audit db")
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mcp_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_name TEXT NOT NULL,
                arguments_encrypted TEXT NOT NULL,
                result_encrypted TEXT NOT NULL,
                success INTEGER NOT NULL,
                pii_found INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                key_epoch INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        let _ = conn.execute(
            "ALTER TABLE mcp_log ADD COLUMN key_epoch INTEGER NOT NULL DEFAULT 0",
            [],
        );
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

    fn decrypt(&self, combined_b64: &str) -> Result<String> {
        self.decrypt_with_key(combined_b64, &self.key)
    }

    fn decrypt_for_epoch(&self, combined_b64: &str, key_epoch: u64) -> Result<String> {
        if let Some(key) = self.keyring.get(&key_epoch) {
            return self.decrypt_with_key(combined_b64, key);
        }
        self.decrypt(combined_b64)
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

    pub fn insert_log(
        &self,
        tool_name: &str,
        arguments: &Value,
        result: &str,
        success: bool,
        pii_found: bool,
    ) -> Result<i64> {
        let conn = self.conn()?;
        let args_enc = self.encrypt(&arguments.to_string())?;
        let res_enc = self.encrypt(result)?;
        let created_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO mcp_log (tool_name, arguments_encrypted, result_encrypted, success, pii_found, created_at, key_epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                tool_name,
                args_enc,
                res_enc,
                success as i32,
                pii_found as i32,
                created_at,
                self.key_config.epoch as i64,
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
                key_epoch as u64,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (id, tool_name, args_enc, res_enc, success, pii_found, created_at, key_epoch) = r?;
            let arguments = self
                .decrypt_for_epoch(&args_enc, key_epoch)
                .unwrap_or_else(|_| "[decrypt failed]".into());
            let result = self
                .decrypt_for_epoch(&res_enc, key_epoch)
                .unwrap_or_else(|_| "[decrypt failed]".into());
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

impl Default for McpAuditStore {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .map(|d| d.join("ai.openlife.desktop"))
            .unwrap_or_else(|| std::env::current_dir().unwrap().join(".openlife"));
        Self::new(data_dir.join("mcp_audit.db"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_store_key_rotation_keeps_old_logs_readable_with_keyring() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = McpAuditStore::new(dir.path().join("audit.db"));
        store
            .insert_log("test_tool", &serde_json::json!({"x": 1}), "ok", true, false)
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
        assert!(logs.iter().any(|l| l.tool_name == "test_tool"
            && l.arguments == r#"{"x":1}"#
            && l.result == "ok"));
        assert!(logs.iter().any(|l| l.tool_name == "test_tool2"
            && l.arguments == r#"{"y":2}"#
            && l.result == "done"));

        let restarted =
            McpAuditStore::with_keyring(dir.path().join("audit.db"), store.key_configs().to_vec());
        let restarted_logs = restarted.list_logs(10).unwrap();
        assert!(restarted_logs.iter().any(|l| l.tool_name == "test_tool"
            && l.arguments == r#"{"x":1}"#
            && l.result == "ok"));
        assert!(restarted_logs.iter().any(|l| l.tool_name == "test_tool2"
            && l.arguments == r#"{"y":2}"#
            && l.result == "done"));
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
}

/// Shareable handle.
pub type SharedMcpAuditStore = Arc<Mutex<McpAuditStore>>;
