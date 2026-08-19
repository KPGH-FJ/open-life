use crate::life_model::patch::{LifeModelPatch, PatchConflict, PatchStatus};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchProjectionStageState {
    Prepared,
    Applied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchProjectionOperation {
    pub operation_id: String,
    pub before_digest: String,
    pub after_digest: String,
    pub state: PatchProjectionStageState,
    pub patch_count: usize,
}

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
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory patches db")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn open_read_only_existing(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = crate::sqlite_migration::open_existing_read_only(
            &db_path,
            "patch_store",
            &["life_model_patches"],
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
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

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS life_model_patch_projection_refs (
                operation_id TEXT NOT NULL,
                patch_id TEXT NOT NULL,
                before_digest TEXT NOT NULL,
                after_digest TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY(operation_id, patch_id),
                UNIQUE(patch_id),
                FOREIGN KEY(patch_id) REFERENCES life_model_patches(id)
                    ON DELETE CASCADE
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_lifemodel_patch_projection_operation
             ON life_model_patch_projection_refs(operation_id, created_at);",
        )?;

        Ok(())
    }

    #[cfg(test)]
    pub fn create_patch(&self, patch: &LifeModelPatch) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Self::insert_patch_row(&conn, patch, false)?;
        Ok(())
    }

    /// Persist accepted patch content in its existing canonical owner before
    /// the LifeModel file rename. The staging table stores references and
    /// digests only; it is a recovery protocol, not a second content owner and
    /// not a cross-database transaction.
    pub fn stage_materialization_patches(
        &self,
        operation_id: &str,
        before_digest: &str,
        after_digest: &str,
        patches: &[LifeModelPatch],
    ) -> Result<()> {
        validate_stage_identifier("operation_id", operation_id)?;
        validate_stage_digest(before_digest)?;
        validate_stage_digest(after_digest)?;
        if before_digest == after_digest {
            anyhow::bail!("LifeModel patch projection digests must differ");
        }
        if patches.is_empty() {
            anyhow::bail!("LifeModel patch projection batch must not be empty");
        }
        let mut unique_patch_ids = HashSet::new();
        for patch in patches {
            validate_stage_identifier("patch_id", &patch.id)?;
            if patch.status != PatchStatus::Pending || patch.applied_at.is_some() {
                anyhow::bail!("LifeModel patch must be pending before projection staging");
            }
            if !unique_patch_ids.insert(patch.id.as_str()) {
                anyhow::bail!("duplicate LifeModel patch id in projection batch");
            }
        }

        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().to_rfc3339();
        for patch in patches {
            Self::insert_patch_row(&tx, patch, true)?;
            let existing = Self::get_patch_in_transaction(&tx, &patch.id)?
                .ok_or_else(|| anyhow::anyhow!("staged LifeModel patch row is missing"))?;
            if immutable_patch_identity(&existing)? != immutable_patch_identity(patch)? {
                anyhow::bail!("LifeModel patch id already belongs to different content");
            }
            tx.execute(
                "INSERT OR IGNORE INTO life_model_patch_projection_refs (
                    operation_id, patch_id, before_digest, after_digest, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![operation_id, patch.id, before_digest, after_digest, now],
            )?;
            let staged: Option<(String, String)> = tx
                .query_row(
                    "SELECT before_digest, after_digest
                     FROM life_model_patch_projection_refs
                     WHERE operation_id = ?1 AND patch_id = ?2",
                    params![operation_id, patch.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if staged.as_ref() != Some(&(before_digest.to_string(), after_digest.to_string())) {
                anyhow::bail!("LifeModel patch projection stage conflicts with prior operation");
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn apply_materialization_operation(&self, operation_id: &str) -> Result<usize> {
        self.finish_materialization_operation(operation_id)
    }

    /// Remove only this operation's internal Pending prepare records when the
    /// file journal proves the canonical write was never committed. This is not
    /// a user rejection and therefore must not manufacture `Rejected` history.
    pub fn discard_not_committed_materialization_operation(
        &self,
        operation_id: &str,
    ) -> Result<usize> {
        validate_stage_identifier("operation_id", operation_id)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let patch_ids = Self::materialization_patch_ids(&tx, operation_id)?;
        if patch_ids.is_empty() {
            return Ok(0);
        }
        for patch_id in &patch_ids {
            let status: String = tx.query_row(
                "SELECT status FROM life_model_patches WHERE id = ?1",
                [patch_id],
                |row| row.get(0),
            )?;
            if status != "pending" {
                anyhow::bail!(
                    "cannot discard non-pending LifeModel patch {patch_id} for not-committed operation"
                );
            }
        }
        tx.execute(
            "DELETE FROM life_model_patch_projection_refs WHERE operation_id = ?1",
            [operation_id],
        )?;
        for patch_id in &patch_ids {
            tx.execute(
                "DELETE FROM life_model_patches WHERE id = ?1 AND status = 'pending'",
                [patch_id],
            )?;
        }
        tx.commit()?;
        Ok(patch_ids.len())
    }

    pub fn list_open_materialization_operations(
        &self,
        limit: usize,
    ) -> Result<Vec<PatchProjectionOperation>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT refs.operation_id, refs.before_digest, refs.after_digest, COUNT(*)
             FROM life_model_patch_projection_refs refs
             JOIN life_model_patches patches ON patches.id = refs.patch_id
             WHERE patches.status = 'pending'
             GROUP BY refs.operation_id, refs.before_digest, refs.after_digest
             ORDER BY MIN(refs.created_at) ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit.clamp(1, 500) as i64], |row| {
            let count = usize::try_from(row.get::<_, i64>(3)?).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })?;
            Ok(PatchProjectionOperation {
                operation_id: row.get(0)?,
                before_digest: row.get(1)?,
                after_digest: row.get(2)?,
                state: PatchProjectionStageState::Prepared,
                patch_count: count,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn materialization_operation_state(
        &self,
        operation_id: &str,
    ) -> Result<Option<PatchProjectionStageState>> {
        validate_stage_identifier("operation_id", operation_id)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let raw: Option<(i64, i64)> = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(CASE WHEN patches.status = 'applied' THEN 1 ELSE 0 END), 0)
                 FROM life_model_patch_projection_refs refs
                 JOIN life_model_patches patches ON patches.id = refs.patch_id
                 WHERE refs.operation_id = ?1",
                [operation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match raw {
            None | Some((0, _)) => Ok(None),
            Some((_total, 0)) => Ok(Some(PatchProjectionStageState::Prepared)),
            Some((total, applied)) if applied == total => {
                Ok(Some(PatchProjectionStageState::Applied))
            }
            Some(_) => anyhow::bail!("LifeModel patch projection operation is partially applied"),
        }
    }

    pub fn materialization_operation_proposal_id(
        &self,
        operation_id: &str,
    ) -> Result<Option<String>> {
        validate_stage_identifier("operation_id", operation_id)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT DISTINCT patches.proposal_id
             FROM life_model_patch_projection_refs refs
             JOIN life_model_patches patches ON patches.id = refs.patch_id
             WHERE refs.operation_id = ?1",
        )?;
        let values = statement
            .query_map([operation_id], |row| row.get::<_, Option<String>>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        match values.as_slice() {
            [] => Ok(None),
            [Some(proposal_id)] => Ok(Some(proposal_id.clone())),
            [None] => anyhow::bail!("staged LifeModel patch is missing proposal id"),
            _ => anyhow::bail!("staged LifeModel patch operation spans multiple proposals"),
        }
    }

    #[cfg(test)]
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

    pub fn patch_count(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM life_model_patches", [], |row| {
            row.get(0)
        })?;
        Ok(count as usize)
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

    fn insert_patch_row(
        conn: &Connection,
        patch: &LifeModelPatch,
        ignore_existing: bool,
    ) -> Result<()> {
        let insert = if ignore_existing {
            "INSERT OR IGNORE INTO life_model_patches"
        } else {
            "INSERT INTO life_model_patches"
        };
        conn.execute(
            &format!(
                "{insert} (
                    id, proposal_id, path_pointer, path_display, operation,
                    before_json, after_json, source, reason, confidence,
                    risk_level, status, created_at, applied_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"
            ),
            params![
                patch.id,
                patch.proposal_id,
                patch.path_pointer,
                patch.path_display,
                patch.operation.to_string(),
                patch
                    .before
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                serde_json::to_string(&patch.after)?,
                patch.source.to_string(),
                patch.reason,
                patch.confidence,
                patch.risk_level.to_string(),
                patch.status.to_string(),
                patch.created_at.to_rfc3339(),
                patch.applied_at.map(|timestamp| timestamp.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    fn get_patch_in_transaction(
        tx: &Transaction<'_>,
        patch_id: &str,
    ) -> Result<Option<LifeModelPatch>> {
        tx.query_row(
            "SELECT id, proposal_id, path_pointer, path_display, operation,
                    before_json, after_json, source, reason, confidence,
                    risk_level, status, created_at, applied_at
             FROM life_model_patches WHERE id = ?1",
            [patch_id],
            Self::row_to_patch,
        )
        .optional()
        .map_err(Into::into)
    }

    fn finish_materialization_operation(&self, operation_id: &str) -> Result<usize> {
        validate_stage_identifier("operation_id", operation_id)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let patch_ids = Self::materialization_patch_ids(&tx, operation_id)?;
        if patch_ids.is_empty() {
            anyhow::bail!("LifeModel patch projection operation is missing");
        }
        let now = chrono::Utc::now().to_rfc3339();
        for patch_id in &patch_ids {
            let current: String = tx.query_row(
                "SELECT status FROM life_model_patches WHERE id = ?1",
                [patch_id],
                |row| row.get(0),
            )?;
            if !matches!(current.as_str(), "pending" | "applied") {
                anyhow::bail!(
                    "LifeModel patch {patch_id} has incompatible terminal status {current}"
                );
            }
            tx.execute(
                "UPDATE life_model_patches
                 SET status = ?2,
                     applied_at = CASE
                         WHEN status = 'applied' THEN applied_at
                         ELSE ?3
                     END
                 WHERE id = ?1",
                params![patch_id, PatchStatus::Applied.to_string(), now],
            )?;
        }
        tx.commit()?;
        Ok(patch_ids.len())
    }

    fn materialization_patch_ids(tx: &Transaction<'_>, operation_id: &str) -> Result<Vec<String>> {
        let mut statement = tx.prepare(
            "SELECT patch_id FROM life_model_patch_projection_refs
             WHERE operation_id = ?1 ORDER BY patch_id ASC",
        )?;
        let patch_ids = statement
            .query_map([operation_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(patch_ids)
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
            "chat_conversation" => PatchSource::ChatConversation,
            "skill_runtime" => PatchSource::SkillRuntime,
            "memory_governance" => PatchSource::MemoryGovernance,
            "planning_session" => PatchSource::PlanningSession,
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

fn validate_stage_identifier(label: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 256
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        anyhow::bail!("invalid LifeModel patch projection {label}");
    }
    Ok(())
}

fn validate_stage_digest(value: &str) -> Result<()> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        anyhow::bail!("LifeModel patch projection digest must be sha256");
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        anyhow::bail!("LifeModel patch projection digest is malformed");
    }
    Ok(())
}

fn immutable_patch_identity(patch: &LifeModelPatch) -> Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id": patch.id,
        "proposalId": patch.proposal_id,
        "pathPointer": patch.path_pointer,
        "pathDisplay": patch.path_display,
        "operation": patch.operation.to_string(),
        "before": patch.before,
        "after": patch.after,
        "source": patch.source.to_string(),
        "reason": patch.reason,
        "confidence": patch.confidence,
        "riskLevel": patch.risk_level.to_string(),
        "createdAt": patch.created_at.to_rfc3339(),
    }))
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

    #[test]
    fn materialization_batch_stages_and_applies_atomically_and_idempotently() {
        let store = PatchStore::new_in_memory().unwrap();
        let patches = [
            LifeModelPatch::new(
                "/identity/name",
                "Identity > Name",
                PatchOp::Replace,
                serde_json::json!("Ada"),
                "accepted proposal",
                RiskLevel::Medium,
                PatchSource::Manual,
            ),
            LifeModelPatch::new(
                "/state/energy_level",
                "State > Energy",
                PatchOp::Replace,
                serde_json::json!(7),
                "accepted proposal",
                RiskLevel::Low,
                PatchSource::Manual,
            ),
        ];
        let before = crate::persistence_outbox::metadata_digest("before");
        let after = crate::persistence_outbox::metadata_digest("after");

        store
            .stage_materialization_patches("file-outbox:op-1", &before, &after, &patches)
            .unwrap();
        store
            .stage_materialization_patches("file-outbox:op-1", &before, &after, &patches)
            .unwrap();
        assert_eq!(
            store
                .materialization_operation_state("file-outbox:op-1")
                .unwrap(),
            Some(PatchProjectionStageState::Prepared)
        );
        assert!(patches.iter().all(|patch| {
            store.get_patch(&patch.id).unwrap().unwrap().status == PatchStatus::Pending
        }));

        assert_eq!(
            store
                .apply_materialization_operation("file-outbox:op-1")
                .unwrap(),
            2
        );
        let applied_at = patches
            .iter()
            .map(|patch| store.get_patch(&patch.id).unwrap().unwrap().applied_at)
            .collect::<Vec<_>>();
        store
            .apply_materialization_operation("file-outbox:op-1")
            .unwrap();
        assert_eq!(
            applied_at,
            patches
                .iter()
                .map(|patch| store.get_patch(&patch.id).unwrap().unwrap().applied_at)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            store
                .materialization_operation_state("file-outbox:op-1")
                .unwrap(),
            Some(PatchProjectionStageState::Applied)
        );
    }

    #[test]
    fn not_committed_stage_is_removed_without_manufacturing_rejected_history() {
        let store = PatchStore::new_in_memory().unwrap();
        let patch = LifeModelPatch::new(
            "/identity/name",
            "Identity > Name",
            PatchOp::Replace,
            serde_json::json!("Ada"),
            "accepted proposal",
            RiskLevel::Medium,
            PatchSource::Manual,
        );
        store
            .stage_materialization_patches(
                "file-outbox:op-2",
                &crate::persistence_outbox::metadata_digest("before"),
                &crate::persistence_outbox::metadata_digest("after"),
                std::slice::from_ref(&patch),
            )
            .unwrap();

        assert_eq!(
            store
                .discard_not_committed_materialization_operation("file-outbox:op-2")
                .unwrap(),
            1
        );
        assert!(store.get_patch(&patch.id).unwrap().is_none());
        assert_eq!(
            store
                .materialization_operation_state("file-outbox:op-2")
                .unwrap(),
            None
        );
    }

    #[test]
    fn reused_patch_id_with_different_content_is_rejected_without_partial_batch() {
        let store = PatchStore::new_in_memory().unwrap();
        let first = LifeModelPatch::new(
            "/identity/name",
            "Identity > Name",
            PatchOp::Replace,
            serde_json::json!("Ada"),
            "accepted proposal",
            RiskLevel::Medium,
            PatchSource::Manual,
        );
        let mut conflicting = first.clone();
        conflicting.after = serde_json::json!("Grace");
        store
            .stage_materialization_patches(
                "file-outbox:op-3",
                &crate::persistence_outbox::metadata_digest("before"),
                &crate::persistence_outbox::metadata_digest("after"),
                std::slice::from_ref(&first),
            )
            .unwrap();
        assert!(store
            .stage_materialization_patches(
                "file-outbox:op-3",
                &crate::persistence_outbox::metadata_digest("before"),
                &crate::persistence_outbox::metadata_digest("after"),
                &[conflicting],
            )
            .is_err());
        assert_eq!(store.patch_count().unwrap(), 1);
        assert_eq!(
            store.get_patch(&first.id).unwrap().unwrap().after,
            first.after
        );
    }

    #[test]
    fn not_committed_cleanup_cannot_erase_an_applied_projection() {
        let store = PatchStore::new_in_memory().unwrap();
        let patch = LifeModelPatch::new(
            "/identity/name",
            "Identity > Name",
            PatchOp::Replace,
            serde_json::json!("Ada"),
            "accepted proposal",
            RiskLevel::Medium,
            PatchSource::Manual,
        );
        store
            .stage_materialization_patches(
                "file-outbox:op-4",
                &crate::persistence_outbox::metadata_digest("before"),
                &crate::persistence_outbox::metadata_digest("after"),
                std::slice::from_ref(&patch),
            )
            .unwrap();
        store
            .apply_materialization_operation("file-outbox:op-4")
            .unwrap();

        assert!(store
            .discard_not_committed_materialization_operation("file-outbox:op-4")
            .is_err());
        assert_eq!(
            store.get_patch(&patch.id).unwrap().unwrap().status,
            PatchStatus::Applied
        );
    }
}
