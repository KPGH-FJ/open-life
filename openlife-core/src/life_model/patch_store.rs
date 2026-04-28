use crate::life_model::patch::{LifeModelPatch, PatchConflict, PatchStatus};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct PatchStore {
    conn: Mutex<Connection>,
}

impl PatchStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open patches db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory patches db")?;
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
            "CREATE TABLE IF NOT EXISTS life_model_patches (
                id TEXT PRIMARY KEY,
                proposal_id TEXT,
                path_pointer TEXT NOT NULL,
                path_display TEXT,
                operation TEXT NOT NULL,
                before_json TEXT,
                after_json TEXT NOT NULL,
                source TEXT NOT NULL,
                reason TEXT NOT NULL,
                confidence REAL,
                risk_level TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                applied_at TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_patches_proposal ON life_model_patches(proposal_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_patches_status ON life_model_patches(status, created_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_patches_path ON life_model_patches(path_pointer)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS patch_conflicts (
                id TEXT PRIMARY KEY,
                patch_id_1 TEXT NOT NULL,
                patch_id_2 TEXT NOT NULL,
                conflict_type TEXT NOT NULL,
                resolution TEXT,
                resolved_at TEXT
            )",
            [],
        )?;

        Ok(())
    }

    pub fn create_patch(&self, patch: &LifeModelPatch) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO life_model_patches (
                id, proposal_id, path_pointer, path_display, operation,
                before_json, after_json, source, reason, confidence,
                risk_level, status, created_at, applied_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                patch.id,
                patch.proposal_id,
                patch.path_pointer,
                patch.path_display,
                patch.operation.to_string(),
                patch
                    .before
                    .as_ref()
                    .map(|b| serde_json::to_string(b).unwrap_or_default()),
                serde_json::to_string(&patch.after).unwrap_or_default(),
                patch.source.to_string(),
                patch.reason,
                patch.confidence,
                patch.risk_level.to_string(),
                patch.status.to_string(),
                patch.created_at.to_rfc3339(),
                patch.applied_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn update_patch_status(&self, patch_id: &str, status: PatchStatus) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE life_model_patches SET status = ?2, applied_at = ?3 WHERE id = ?1",
            params![patch_id, status.to_string(), now],
        )?;
        Ok(())
    }

    pub fn get_patch(&self, patch_id: &str) -> Result<Option<LifeModelPatch>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, proposal_id, path_pointer, path_display, operation,
                    before_json, after_json, source, reason, confidence,
                    risk_level, status, created_at, applied_at
             FROM life_model_patches WHERE id = ?1",
        )?;
        let row = stmt.query_row([patch_id], Self::row_to_patch);
        match row {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_patches_by_proposal(&self, proposal_id: &str) -> Result<Vec<LifeModelPatch>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, proposal_id, path_pointer, path_display, operation,
                    before_json, after_json, source, reason, confidence,
                    risk_level, status, created_at, applied_at
             FROM life_model_patches WHERE proposal_id = ?1
             ORDER BY created_at DESC",
        )?;
        let patches = stmt.query_map([proposal_id], Self::row_to_patch)?;
        patches.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn list_applied_patches(&self, limit: i64) -> Result<Vec<LifeModelPatch>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, proposal_id, path_pointer, path_display, operation,
                    before_json, after_json, source, reason, confidence,
                    risk_level, status, created_at, applied_at
             FROM life_model_patches WHERE status = 'applied'
             ORDER BY applied_at DESC
             LIMIT ?1",
        )?;
        let patches = stmt.query_map([limit], Self::row_to_patch)?;
        patches.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn record_conflict(&self, conflict: &PatchConflict) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO patch_conflicts (id, patch_id_1, patch_id_2, conflict_type, resolution, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                uuid::Uuid::new_v4().to_string(),
                conflict.patch_id_1,
                conflict.patch_id_2,
                format!("{:?}", conflict.conflict_type),
                conflict.resolution.map(|r| format!("{:?}", r)),
                conflict.resolved_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    fn row_to_patch(row: &rusqlite::Row<'_>) -> rusqlite::Result<LifeModelPatch> {
        use crate::agent::types::RiskLevel;
        use crate::life_model::patch::{PatchOp, PatchSource, PatchStatus};

        let operation_str: String = row.get(4)?;
        let source_str: String = row.get(7)?;
        let risk_level_str: String = row.get(10)?;
        let status_str: String = row.get(11)?;

        let operation = match operation_str.as_str() {
            "replace" => PatchOp::Replace,
            "merge" => PatchOp::Merge,
            "append" => PatchOp::Append,
            "insert" => PatchOp::Insert,
            "delete" => PatchOp::Delete,
            _ => PatchOp::Replace,
        };

        let source = match source_str.as_str() {
            "builder_review" => PatchSource::BuilderReview,
            "calibration" => PatchSource::Calibration,
            "feedback" => PatchSource::Feedback,
            "manual" => PatchSource::Manual,
            "evolution" => PatchSource::Evolution,
            _ => PatchSource::Manual,
        };

        let risk_level = match risk_level_str.as_str() {
            "low" => RiskLevel::Low,
            "medium" => RiskLevel::Medium,
            "high" => RiskLevel::High,
            "critical" => RiskLevel::Critical,
            _ => RiskLevel::Medium,
        };

        let status = match status_str.as_str() {
            "pending" => PatchStatus::Pending,
            "applied" => PatchStatus::Applied,
            "rejected" => PatchStatus::Rejected,
            "superseded" => PatchStatus::Superseded,
            _ => PatchStatus::Pending,
        };

        let before_json: Option<String> = row.get(5)?;
        let after_json: String = row.get(6)?;
        let created_at_str: String = row.get(12)?;
        let applied_at_str: Option<String> = row.get(13)?;

        Ok(LifeModelPatch {
            id: row.get(0)?,
            proposal_id: row.get(1)?,
            path_pointer: row.get(2)?,
            path_display: row.get(3)?,
            operation,
            before: before_json.and_then(|s| serde_json::from_str(&s).ok()),
            after: serde_json::from_str(&after_json).unwrap_or_default(),
            source,
            reason: row.get(8)?,
            confidence: row.get(9)?,
            risk_level,
            status,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        12,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
                .with_timezone(&chrono::Utc),
            applied_at: applied_at_str
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::RiskLevel;
    use crate::life_model::patch::{LifeModelPatch, PatchOp, PatchSource};

    #[test]
    fn test_create_and_get_patch() {
        let store = PatchStore::new_in_memory().unwrap();
        let patch = LifeModelPatch::new(
            "/identity/values/0/weight",
            "Identity > Values > [0] > Weight",
            PatchOp::Replace,
            serde_json::json!(80),
            "Test patch",
            RiskLevel::Medium,
            PatchSource::Manual,
        );

        store.create_patch(&patch).unwrap();
        let fetched = store.get_patch(&patch.id).unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.path_pointer, "/identity/values/0/weight");
        assert_eq!(fetched.operation, PatchOp::Replace);
    }

    #[test]
    fn test_update_patch_status() {
        let store = PatchStore::new_in_memory().unwrap();
        let patch = LifeModelPatch::new(
            "/goals/short_term",
            "Goals > Short Term",
            PatchOp::Append,
            serde_json::json!({"name": "Test"}),
            "Test",
            RiskLevel::Low,
            PatchSource::Manual,
        );

        store.create_patch(&patch).unwrap();
        store
            .update_patch_status(&patch.id, PatchStatus::Applied)
            .unwrap();

        let fetched = store.get_patch(&patch.id).unwrap().unwrap();
        assert_eq!(fetched.status, PatchStatus::Applied);
        assert!(fetched.applied_at.is_some());
    }
}
