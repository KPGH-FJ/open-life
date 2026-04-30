use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPermissionPolicy {
    Allow,
    Deny,
    AskEveryTime,
    AllowOnce,
    AllowUntilRevoked,
}

impl std::fmt::Display for ToolPermissionPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allow => write!(f, "allow"),
            Self::Deny => write!(f, "deny"),
            Self::AskEveryTime => write!(f, "ask_every_time"),
            Self::AllowOnce => write!(f, "allow_once"),
            Self::AllowUntilRevoked => write!(f, "allow_until_revoked"),
        }
    }
}

impl std::str::FromStr for ToolPermissionPolicy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "allow" => Ok(Self::Allow),
            "deny" => Ok(Self::Deny),
            "ask_every_time" => Ok(Self::AskEveryTime),
            "allow_once" => Ok(Self::AllowOnce),
            "allow_until_revoked" => Ok(Self::AllowUntilRevoked),
            other => Err(anyhow::anyhow!("unknown tool permission policy: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPermissionRecord {
    pub id: String,
    pub tool_name: String,
    pub source: String,
    pub risk_level: String,
    pub action_type: String,
    pub policy: ToolPermissionPolicy,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPermissionDecision {
    pub allowed: bool,
    pub requires_confirmation: bool,
    pub decision: String,
    pub reason: String,
    pub policy_id: Option<String>,
}

pub struct ToolPermissionStore {
    conn: Mutex<Connection>,
}

impl ToolPermissionStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = Self {
            conn: Mutex::new(Connection::open(&db_path)?),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let store = Self {
            conn: Mutex::new(Connection::open_in_memory()?),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_permissions (
                id TEXT PRIMARY KEY,
                tool_name TEXT NOT NULL,
                source TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                action_type TEXT NOT NULL,
                policy TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT,
                consumed_at TEXT
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tool_permissions_lookup
             ON tool_permissions(tool_name, source, risk_level, action_type)",
            [],
        )?;
        Ok(())
    }

    pub fn grant(
        &self,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
        policy: ToolPermissionPolicy,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ToolPermissionRecord> {
        let record = ToolPermissionRecord {
            id: Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            source: source.to_string(),
            risk_level: risk_level.to_string(),
            action_type: action_type.to_string(),
            policy,
            created_at: Utc::now(),
            expires_at,
            consumed_at: None,
        };
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO tool_permissions
             (id, tool_name, source, risk_level, action_type, policy, created_at, expires_at, consumed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id,
                record.tool_name,
                record.source,
                record.risk_level,
                record.action_type,
                record.policy.to_string(),
                record.created_at.to_rfc3339(),
                record.expires_at.map(|t| t.to_rfc3339()),
                record.consumed_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(record)
    }

    pub fn list(&self) -> Result<Vec<ToolPermissionRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, tool_name, source, risk_level, action_type, policy, created_at, expires_at, consumed_at
             FROM tool_permissions ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_record)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn revoke(&self, id: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Ok(conn.execute("DELETE FROM tool_permissions WHERE id = ?1", [id])? > 0)
    }

    pub fn check(
        &self,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
        capabilities: &[String],
    ) -> Result<ToolPermissionDecision> {
        let record = self.find_best(tool_name, source, risk_level, action_type)?;
        let Some(record) = record else {
            let asks = risk_level == "high"
                || capabilities.iter().any(|c| {
                    matches!(
                        c.as_str(),
                        "write" | "memory" | "lifemodel" | "filesystem" | "external_side_effect"
                    )
                });
            return Ok(ToolPermissionDecision {
                allowed: !asks,
                requires_confirmation: asks,
                decision: if asks { "ask_every_time" } else { "allow" }.to_string(),
                reason: if asks {
                    "no policy for high-risk/write action".to_string()
                } else {
                    "low-risk read action allowed by default".to_string()
                },
                policy_id: None,
            });
        };

        if record
            .expires_at
            .is_some_and(|expires| expires < Utc::now())
        {
            return Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "expired".to_string(),
                reason: "matching policy expired".to_string(),
                policy_id: Some(record.id),
            });
        }

        match record.policy {
            ToolPermissionPolicy::Allow | ToolPermissionPolicy::AllowUntilRevoked => {
                Ok(ToolPermissionDecision {
                    allowed: true,
                    requires_confirmation: false,
                    decision: record.policy.to_string(),
                    reason: "matching allow policy".to_string(),
                    policy_id: Some(record.id),
                })
            }
            ToolPermissionPolicy::Deny => Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: false,
                decision: "deny".to_string(),
                reason: "matching deny policy".to_string(),
                policy_id: Some(record.id),
            }),
            ToolPermissionPolicy::AskEveryTime => Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "ask_every_time".to_string(),
                reason: "matching ask-every-time policy".to_string(),
                policy_id: Some(record.id),
            }),
            ToolPermissionPolicy::AllowOnce => {
                self.consume(&record.id)?;
                Ok(ToolPermissionDecision {
                    allowed: true,
                    requires_confirmation: false,
                    decision: "allow_once".to_string(),
                    reason: "matching allow-once policy consumed".to_string(),
                    policy_id: Some(record.id),
                })
            }
        }
    }

    fn consume(&self, id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "UPDATE tool_permissions SET consumed_at = ?2 WHERE id = ?1",
            params![id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn find_best(
        &self,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
    ) -> Result<Option<ToolPermissionRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT id, tool_name, source, risk_level, action_type, policy, created_at, expires_at, consumed_at
             FROM tool_permissions
             WHERE consumed_at IS NULL
               AND (tool_name = ?1 OR tool_name = '*')
               AND (source = ?2 OR source = '*')
               AND (risk_level = ?3 OR risk_level = '*')
               AND (action_type = ?4 OR action_type = '*')
             ORDER BY
               CASE WHEN tool_name = ?1 THEN 0 ELSE 1 END,
               CASE WHEN source = ?2 THEN 0 ELSE 1 END,
               CASE WHEN risk_level = ?3 THEN 0 ELSE 1 END,
               CASE WHEN action_type = ?4 THEN 0 ELSE 1 END,
               created_at DESC
             LIMIT 1",
            params![tool_name, source, risk_level, action_type],
            row_to_record,
        )
        .optional()
        .context("failed to query tool permission")
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<ToolPermissionRecord> {
    let policy: String = row.get(5)?;
    let created_at: String = row.get(6)?;
    let expires_at: Option<String> = row.get(7)?;
    let consumed_at: Option<String> = row.get(8)?;
    Ok(ToolPermissionRecord {
        id: row.get(0)?,
        tool_name: row.get(1)?,
        source: row.get(2)?,
        risk_level: row.get(3)?,
        action_type: row.get(4)?,
        policy: policy.parse().map_err(|_| rusqlite::Error::InvalidQuery)?,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .with_timezone(&Utc),
        expires_at: expires_at
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .map(|dt| dt.with_timezone(&Utc)),
        consumed_at: consumed_at
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .map(|dt| dt.with_timezone(&Utc)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_list_revoke_permission() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let record = store
            .grant(
                "builtin_echo",
                "builtin",
                "low",
                "mcp_tool_call",
                ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        assert!(store.revoke(&record.id).unwrap());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn high_risk_without_policy_requires_confirmation() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let decision = store
            .check(
                "write_file",
                "mcp",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(!decision.allowed);
        assert!(decision.requires_confirmation);
    }

    #[test]
    fn allow_once_consumes_policy_and_second_check_asks() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        store
            .grant(
                "write_file",
                "builtin",
                "high",
                "mcp_tool_call",
                ToolPermissionPolicy::AllowOnce,
                None,
            )
            .unwrap();
        // First check: allowed, policy consumed
        let first = store
            .check(
                "write_file",
                "builtin",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(first.allowed);
        assert_eq!(first.decision, "allow_once");
        // Second check: no matching policy, falls back to default heuristic (high-risk + write capability requires confirmation)
        let second = store
            .check(
                "write_file",
                "builtin",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(!second.allowed);
        assert!(second.requires_confirmation);
        assert_eq!(second.policy_id, None);
    }

    #[test]
    fn replay_uses_same_source_canonical_format_as_normal() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        // Grant with canonical source format "mcp:filesystem"
        store
            .grant(
                "write_file",
                "mcp:filesystem",
                "high",
                "mcp_tool_call",
                ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        // Normal execution check with canonical format
        let normal = store
            .check(
                "write_file",
                "mcp:filesystem",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(normal.allowed);
        assert_eq!(normal.decision, "allow_until_revoked");
        // Replay uses the same source format from tool_scope
        let replay = store
            .check(
                "write_file",
                "mcp:filesystem",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(replay.allowed);
        assert_eq!(replay.decision, "allow_until_revoked");
        // Mismatched source format should not match
        let mismatched = store
            .check(
                "write_file",
                "mcp",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(!mismatched.allowed);
        assert!(mismatched.requires_confirmation);
    }

    #[test]
    fn source_canonical_format_builtin_vs_mcp() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        // Grant builtin tool with canonical format (high risk to test mismatch blocking)
        store
            .grant(
                "write_file",
                "builtin",
                "high",
                "mcp_tool_call",
                ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        // Builtin check passes
        let builtin_check = store
            .check(
                "write_file",
                "builtin",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(builtin_check.allowed);
        // MCP check with same tool name but different source does not match builtin grant
        // Since no policy matches for mcp:memory, high-risk + write capability requires confirmation
        let mcp_check = store
            .check(
                "write_file",
                "mcp:memory",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(!mcp_check.allowed);
        assert!(mcp_check.requires_confirmation);
    }
}
