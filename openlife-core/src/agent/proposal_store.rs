use crate::agent::types::{AgentProposal, ProposalSource, ProposalStatus, ProposalType, RiskLevel};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct ProposalStore {
    conn: Mutex<Connection>,
}

impl ProposalStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open proposals db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory proposals db")?;
        let store = Self {
            conn: Mutex::new(conn),
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
            "CREATE TABLE IF NOT EXISTS proposals (
                id TEXT PRIMARY KEY,
                run_id TEXT,
                proposal_type TEXT NOT NULL,
                source TEXT NOT NULL,
                source_detail TEXT,
                affected_path TEXT NOT NULL,
                before_json TEXT,
                after_json TEXT NOT NULL,
                reason TEXT NOT NULL,
                confidence REAL NOT NULL,
                risk_level TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                expires_at TEXT
            )",
            [],
        )?;
        // Migration: add new columns if table exists without them
        let _ = conn.execute("ALTER TABLE proposals ADD COLUMN run_id TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE proposals ADD COLUMN source TEXT DEFAULT 'manual'",
            [],
        );
        let _ = conn.execute("ALTER TABLE proposals ADD COLUMN source_detail TEXT", []);
        let _ = conn.execute("ALTER TABLE proposals ADD COLUMN expires_at TEXT", []);
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_proposals_status ON proposals(status, created_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_proposals_expires ON proposals(expires_at) WHERE status = 'pending'",
            [],
        )?;
        Ok(())
    }

    pub fn create_proposal(&self, proposal: &AgentProposal) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO proposals (id, run_id, proposal_type, source, source_detail, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                proposal.id,
                proposal.run_id.as_ref(),
                proposal.proposal_type.to_string(),
                proposal.source,
                proposal.source_detail.as_ref(),
                proposal.affected_path,
                proposal.before.as_ref().map(|b| serde_json::to_string(b).unwrap_or_default()),
                serde_json::to_string(&proposal.after).unwrap_or_default(),
                proposal.reason,
                proposal.confidence,
                proposal.risk_level.to_string(),
                proposal.status.to_string(),
                proposal.created_at.to_rfc3339(),
                proposal.resolved_at.map(|t| t.to_rfc3339()),
                proposal.expires_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn update_proposal(&self, proposal: &AgentProposal) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "UPDATE proposals SET
                status = ?2,
                after_json = ?3,
                resolved_at = ?4
            WHERE id = ?1",
            params![
                proposal.id,
                proposal.status.to_string(),
                serde_json::to_string(&proposal.after).unwrap_or_default(),
                proposal.resolved_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn get_proposal(&self, id: &str) -> Result<Option<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals WHERE id = ?1"
        )?;
        let row = stmt.query_row([id], |row| Self::row_to_proposal(row));
        match row {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_pending_proposals(&self, limit: i64) -> Result<Vec<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals
             WHERE status = 'pending'
             ORDER BY created_at DESC
             LIMIT ?1"
        )?;
        let proposals = stmt.query_map([limit], Self::row_to_proposal)?;
        proposals
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn list_all_proposals(&self, limit: i64, offset: i64) -> Result<Vec<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals
             ORDER BY created_at DESC
             LIMIT ?1 OFFSET ?2"
        )?;
        let proposals = stmt.query_map([limit, offset], Self::row_to_proposal)?;
        proposals
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn list_proposals_filtered(
        &self,
        status: Option<ProposalStatus>,
        proposal_type: Option<ProposalType>,
        risk_level: Option<RiskLevel>,
        limit: i64,
    ) -> Result<Vec<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(s) = status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(s.to_string()));
        }
        if let Some(t) = proposal_type {
            conditions.push("proposal_type = ?".to_string());
            params.push(Box::new(t.to_string()));
        }
        if let Some(r) = risk_level {
            conditions.push("risk_level = ?".to_string());
            params.push(Box::new(r.to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, run_id, proposal_type, source, source_detail, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals
             {}
             ORDER BY created_at DESC
             LIMIT ?",
            where_clause
        );

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let proposals = stmt.query_map(
            rusqlite::params_from_iter(
                param_refs
                    .iter()
                    .copied()
                    .chain(std::iter::once(&limit as &dyn rusqlite::ToSql)),
            ),
            Self::row_to_proposal,
        )?;
        proposals
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn batch_accept_low_risk(&self) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = chrono::Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE proposals SET status = 'accepted', resolved_at = ?1
             WHERE status = 'pending' AND risk_level = 'low'",
            [&now],
        )?;
        Ok(updated as i64)
    }

    pub fn count_by_status_and_risk(
        &self,
        status: ProposalStatus,
        min_risk: Option<RiskLevel>,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;

        let sql = if let Some(risk) = min_risk {
            let risk_order = match risk {
                RiskLevel::Low => "risk_level IN ('low', 'medium', 'high', 'critical')",
                RiskLevel::Medium => "risk_level IN ('medium', 'high', 'critical')",
                RiskLevel::High => "risk_level IN ('high', 'critical')",
                RiskLevel::Critical => "risk_level = 'critical'",
            };
            format!(
                "SELECT COUNT(*) FROM proposals WHERE status = '{}' AND {}",
                status.to_string(),
                risk_order
            )
        } else {
            format!(
                "SELECT COUNT(*) FROM proposals WHERE status = '{}'",
                status.to_string()
            )
        };

        let count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn pending_count(&self) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposals WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_proposals_by_run_id(&self, run_id: &str) -> Result<Vec<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals
             WHERE run_id = ?1
             ORDER BY created_at DESC"
        )?;
        let proposals = stmt.query_map([run_id], Self::row_to_proposal)?;
        proposals
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Cleanup expired proposals and return count
    pub fn cleanup_expired_proposals(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE proposals SET status = 'expired' WHERE status = 'pending' AND expires_at < ?1",
            [&now],
        )?;
        Ok(rows)
    }

    /// List proposals expiring within given days
    pub fn list_expiring_soon(&self, days: i64) -> Result<Vec<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let cutoff = (chrono::Utc::now() + chrono::Duration::days(days)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals
             WHERE status = 'pending' AND expires_at < ?1
             ORDER BY expires_at ASC"
        )?;
        let proposals = stmt.query_map([&cutoff], Self::row_to_proposal)?;
        proposals
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    fn row_to_proposal(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentProposal> {
        let run_id: Option<String> = row.get(1)?;
        let type_str: String = row.get(2)?;
        let source: ProposalSource = row.get(3)?;
        let source_detail: Option<String> = row.get(4)?;
        let before_json: Option<String> = row.get(6)?;
        let after_json: String = row.get(7)?;
        let risk_str: String = row.get(10)?;
        let status_str: String = row.get(11)?;
        let created_at_str: String = row.get(12)?;
        let resolved_at_str: Option<String> = row.get(13)?;
        let expires_at_str: Option<String> = row.get(14)?;

        let proposal_type = match type_str.as_str() {
            "goal_update" => ProposalType::GoalUpdate,
            "state_update" => ProposalType::StateUpdate,
            "preference_update" => ProposalType::PreferenceUpdate,
            "capability_update" => ProposalType::CapabilityUpdate,
            "memory_write" => ProposalType::MemoryWrite,
            "memory_archive" => ProposalType::MemoryArchive,
            "tool_permission" => ProposalType::ToolPermission,
            "schedule_checkin" => ProposalType::ScheduleCheckin,
            "life_model_update" => ProposalType::LifeModelUpdate,
            _ => ProposalType::LifeModelUpdate,
        };

        let risk_level = match risk_str.as_str() {
            "low" => RiskLevel::Low,
            "medium" => RiskLevel::Medium,
            "high" => RiskLevel::High,
            "critical" => RiskLevel::Critical,
            _ => RiskLevel::Medium,
        };

        let status = match status_str.as_str() {
            "pending" => ProposalStatus::Pending,
            "accepted" => ProposalStatus::Accepted,
            "rejected" => ProposalStatus::Rejected,
            "edited" => ProposalStatus::Edited,
            "postponed" => ProposalStatus::Postponed,
            _ => ProposalStatus::Pending,
        };

        let before = before_json.and_then(|s| serde_json::from_str(&s).ok());
        let after = serde_json::from_str(&after_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    12,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&chrono::Utc);
        let resolved_at = resolved_at_str
            .map(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .flatten()
            .map(|dt| dt.with_timezone(&chrono::Utc));
        let expires_at = expires_at_str
            .map(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .flatten()
            .map(|dt| dt.with_timezone(&chrono::Utc));

        Ok(AgentProposal {
            id: row.get(0)?,
            run_id,
            proposal_type,
            source,
            source_detail,
            affected_path: row.get(5)?,
            before,
            after,
            reason: row.get(8)?,
            confidence: row.get(9)?,
            risk_level,
            status,
            created_at,
            resolved_at,
            expires_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_proposal() {
        let store = ProposalStore::new_in_memory().unwrap();
        let proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("New Name"),
            "Builder suggested new name",
            0.85,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        store.create_proposal(&proposal).unwrap();

        let fetched = store.get_proposal(&proposal.id).unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, proposal.id);
        assert_eq!(fetched.status, ProposalStatus::Pending);
        assert_eq!(fetched.source, ProposalSource::BuilderReview);
        assert!(fetched.expires_at.is_some());
    }

    #[test]
    fn test_accept_proposal() {
        let store = ProposalStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::StateUpdate,
            "identity.name",
            serde_json::json!("New Name"),
            "Builder suggested new name",
            0.85,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        store.create_proposal(&proposal).unwrap();

        proposal.accept();
        store.update_proposal(&proposal).unwrap();

        let fetched = store.get_proposal(&proposal.id).unwrap().unwrap();
        assert_eq!(fetched.status, ProposalStatus::Accepted);
    }

    #[test]
    fn test_list_pending_proposals() {
        let store = ProposalStore::new_in_memory().unwrap();
        for i in 0..3 {
            let proposal = AgentProposal::new(
                ProposalType::GoalUpdate,
                &format!("path.{}", i),
                serde_json::json!(format!("value{}", i)),
                "test",
                0.5,
                RiskLevel::Low,
                ProposalSource::Manual,
            );
            store.create_proposal(&proposal).unwrap();
        }

        let pending = store.list_pending_proposals(10).unwrap();
        assert_eq!(pending.len(), 3);

        let count = store.pending_count().unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_expired_proposal() {
        let store = ProposalStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("New Name"),
            "Test",
            0.5,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        // Set expiration in the past
        proposal.expires_at = Some(chrono::Utc::now() - chrono::Duration::days(1));
        store.create_proposal(&proposal).unwrap();

        let cleaned = store.cleanup_expired_proposals().unwrap();
        assert_eq!(cleaned, 1);

        let fetched = store.get_proposal(&proposal.id).unwrap().unwrap();
        assert_eq!(fetched.status, ProposalStatus::Pending); // Status is still pending in DB, but is_expired() returns true
        assert!(fetched.is_expired());
    }
}
