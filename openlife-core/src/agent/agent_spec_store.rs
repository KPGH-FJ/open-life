#[cfg(test)]
use crate::agent::types::{AgentRoleKind, PrivacyPolicy};
use crate::agent::types::{AgentSpec, AgentSpecStoreError};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AgentSpecStore {
    conn: Mutex<Connection>,
}

impl AgentSpecStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open agent_specs db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        store.ensure_default_main_spec()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("failed to open in-memory agent_specs db")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        store.ensure_default_main_spec()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_specs (
                id TEXT PRIMARY KEY,
                spec_json TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_specs_active ON agent_specs(active)",
            [],
        )?;
        Ok(())
    }

    fn lock_conn(&self) -> std::result::Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn
            .lock()
            .map_err(|e| format!("agent_specs mutex poison: {}", e))
    }

    fn parse_dt(s: &str) -> std::result::Result<chrono::DateTime<chrono::Utc>, String> {
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| format!("invalid datetime: {}", e))
    }

    fn row_to_spec(row: &rusqlite::Row<'_>) -> std::result::Result<AgentSpec, rusqlite::Error> {
        let spec_json: String = row.get(1)?;
        let active_int: i32 = row.get(2)?;
        let created_at_str: String = row.get(3)?;
        let updated_at_str: String = row.get(4)?;

        let mut spec: AgentSpec = serde_json::from_str(&spec_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("corrupt spec_json for row: {}", e),
                )),
            )
        })?;
        spec.active = active_int != 0;
        spec.created_at = Self::parse_dt(&created_at_str).unwrap_or_else(|_| chrono::Utc::now());
        spec.updated_at = Self::parse_dt(&updated_at_str).unwrap_or_else(|_| chrono::Utc::now());
        Ok(spec)
    }

    pub fn ensure_default_main_spec(&self) -> Result<(), AgentSpecStoreError> {
        let conn = self.lock_conn().map_err(AgentSpecStoreError::Store)?;
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM agent_specs WHERE id = 'main.default'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(false);

        if !exists {
            let spec = AgentSpec::default_main_spec();
            let now = spec.created_at.to_rfc3339();
            let spec_json = serde_json::to_string(&spec)
                .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
            conn.execute(
                "INSERT INTO agent_specs (id, spec_json, active, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params!["main.default", spec_json, 1, now, now],
            )
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        }
        Ok(())
    }

    pub fn create_spec(&self, spec: &AgentSpec) -> Result<(), AgentSpecStoreError> {
        let conn = self.lock_conn().map_err(AgentSpecStoreError::Store)?;
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM agent_specs WHERE id = ?1",
                params![spec.id],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if exists {
            return Err(AgentSpecStoreError::AlreadyExists(spec.id.clone()));
        }
        let now = chrono::Utc::now().to_rfc3339();
        let spec_json =
            serde_json::to_string(spec).map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        let active = if spec.active { 1 } else { 0 };
        conn.execute(
            "INSERT INTO agent_specs (id, spec_json, active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![spec.id, spec_json, active, now, now],
        )
        .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        Ok(())
    }

    pub fn get_spec(&self, spec_id: &str) -> Result<Option<AgentSpec>, AgentSpecStoreError> {
        let conn = self.lock_conn().map_err(AgentSpecStoreError::Store)?;
        let mut stmt = conn
            .prepare("SELECT id, spec_json, active, created_at, updated_at FROM agent_specs WHERE id = ?1")
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        let result = stmt
            .query_row(params![spec_id], Self::row_to_spec)
            .optional()
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        result.map_or(
            Err(AgentSpecStoreError::NotFound(spec_id.to_string())),
            |spec| Ok(Some(spec)),
        )
    }

    pub fn get_spec_optional(
        &self,
        spec_id: &str,
    ) -> Result<Option<AgentSpec>, AgentSpecStoreError> {
        let conn = self.lock_conn().map_err(AgentSpecStoreError::Store)?;
        let mut stmt = conn
            .prepare("SELECT id, spec_json, active, created_at, updated_at FROM agent_specs WHERE id = ?1")
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        stmt.query_row(params![spec_id], Self::row_to_spec)
            .optional()
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))
    }

    pub fn list_specs(&self) -> Result<Vec<AgentSpec>, AgentSpecStoreError> {
        let conn = self.lock_conn().map_err(AgentSpecStoreError::Store)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, spec_json, active, created_at, updated_at FROM agent_specs ORDER BY created_at DESC",
            )
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        let rows = stmt
            .query_map([], Self::row_to_spec)
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        let mut specs = Vec::new();
        for row in rows {
            specs.push(row.map_err(|e| AgentSpecStoreError::Store(e.to_string()))?);
        }
        Ok(specs)
    }

    pub fn list_active_specs(&self) -> Result<Vec<AgentSpec>, AgentSpecStoreError> {
        let conn = self.lock_conn().map_err(AgentSpecStoreError::Store)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, spec_json, active, created_at, updated_at FROM agent_specs WHERE active = 1 ORDER BY created_at DESC",
            )
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        let rows = stmt
            .query_map([], Self::row_to_spec)
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        let mut specs = Vec::new();
        for row in rows {
            specs.push(row.map_err(|e| AgentSpecStoreError::Store(e.to_string()))?);
        }
        Ok(specs)
    }

    pub fn update_spec(&self, spec: &AgentSpec) -> Result<(), AgentSpecStoreError> {
        let conn = self.lock_conn().map_err(AgentSpecStoreError::Store)?;
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM agent_specs WHERE id = ?1",
                params![spec.id],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if !exists {
            return Err(AgentSpecStoreError::NotFound(spec.id.clone()));
        }
        let now = chrono::Utc::now().to_rfc3339();
        let spec_json =
            serde_json::to_string(spec).map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        let active = if spec.active { 1 } else { 0 };
        conn.execute(
            "UPDATE agent_specs SET spec_json = ?2, active = ?3, updated_at = ?4 WHERE id = ?1",
            params![spec.id, spec_json, active, now],
        )
        .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        Ok(())
    }

    pub fn set_active(&self, spec_id: &str, active: bool) -> Result<(), AgentSpecStoreError> {
        let conn = self.lock_conn().map_err(AgentSpecStoreError::Store)?;
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM agent_specs WHERE id = ?1",
                params![spec_id],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if !exists {
            return Err(AgentSpecStoreError::NotFound(spec_id.to_string()));
        }
        let val = if active { 1 } else { 0 };
        conn.execute(
            "UPDATE agent_specs SET active = ?2, updated_at = ?3 WHERE id = ?1",
            params![spec_id, val, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        Ok(())
    }

    /// Atomically set the default main AgentSpec.  Validates that the requested
    /// spec exists and has the Main role **before** mutating any rows.  Other
    /// active main specs are deactivated inside a single transaction so a
    /// partial state cannot be persisted.
    ///
    /// Returns `NotFound` when the spec id does not exist, `InvalidRole` when
    /// the spec exists but does not have the Main role, or `Store` on DB errors.
    ///
    /// Uses a single lock acquisition: loads and validates the spec via a
    /// direct query on the already-held connection, avoiding a deadlock from
    /// re-entering `get_spec()` which would lock the mutex again.
    pub fn set_default_main_spec(&self, spec_id: &str) -> Result<(), AgentSpecStoreError> {
        let conn = self.lock_conn().map_err(AgentSpecStoreError::Store)?;

        // ── Pre-check: load spec via direct query (no re-lock) ────────
        let mut stmt = conn
            .prepare("SELECT id, spec_json, active, created_at, updated_at FROM agent_specs WHERE id = ?1")
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        let spec = stmt
            .query_row(params![spec_id], Self::row_to_spec)
            .optional()
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?
            .ok_or_else(|| AgentSpecStoreError::NotFound(spec_id.to_string()))?;
        drop(stmt);

        if !matches!(spec.role, crate::agent::types::AgentRoleKind::Main) {
            return Err(AgentSpecStoreError::InvalidRole {
                spec_id: spec_id.to_string(),
                role: spec.role,
            });
        }

        // ── Atomic transaction ──────────────────────────────────────────
        conn.execute("BEGIN IMMEDIATE", [])
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        let result = (|| -> Result<(), AgentSpecStoreError> {
            let now = chrono::Utc::now().to_rfc3339();
            // Deactivate all other main specs
            conn.execute(
                "UPDATE agent_specs SET active = 0, updated_at = ?1
                 WHERE id != ?2 AND active = 1 AND json_extract(spec_json, '$.role') = 'main'",
                params![now, spec_id],
            )
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
            // Activate the requested spec
            conn.execute(
                "UPDATE agent_specs SET active = 1, updated_at = ?1 WHERE id = ?2",
                params![now, spec_id],
            )
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                conn.execute("COMMIT", [])
                    .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                Err(e)
            }
        }
    }

    pub fn get_default_spec(&self) -> Result<Option<AgentSpec>, AgentSpecStoreError> {
        let conn = self.lock_conn().map_err(AgentSpecStoreError::Store)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, spec_json, active, created_at, updated_at FROM agent_specs
                 WHERE active = 1 AND json_extract(spec_json, '$.role') = 'main'
                 ORDER BY id = 'main.default' DESC, created_at ASC LIMIT 1",
            )
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))?;
        stmt.query_row([], Self::row_to_spec)
            .optional()
            .map_err(|e| AgentSpecStoreError::Store(e.to_string()))
    }

    /// Resolve the best AgentSpec for execution following deterministic order:
    /// 1. explicit spec_id if provided and exists
    /// 2. stored default main spec
    pub fn resolve_spec(
        &self,
        explicit_spec_id: Option<&str>,
    ) -> Result<AgentSpec, AgentSpecStoreError> {
        if let Some(sid) = explicit_spec_id {
            return self
                .get_spec(sid)?
                .ok_or_else(|| AgentSpecStoreError::NotFound(sid.to_string()));
        }
        self.get_default_spec()?
            .ok_or_else(|| AgentSpecStoreError::Store("no active main AgentSpec found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_main_spec_is_bootstrapped() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let spec = store.get_default_spec().unwrap();
        assert!(
            spec.is_some(),
            "default main spec should exist after bootstrap"
        );
        let spec = spec.unwrap();
        assert_eq!(spec.id, "main.default");
        assert_eq!(spec.role, AgentRoleKind::Main);
        assert!(spec.can_access_lifemodel);
        assert!(spec.can_access_memory_evidence);
        assert!(spec.active);
        assert_eq!(spec.privacy_policy, PrivacyPolicy::LocalOnly);
    }

    #[test]
    fn test_agentspec_round_trips_through_store() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let spec = AgentSpec::default_main_spec();
        // Update via store
        store.update_spec(&spec).unwrap();

        let fetched = store.get_spec(&spec.id).unwrap().unwrap();
        assert_eq!(fetched.id, "main.default");
        assert_eq!(fetched.role, AgentRoleKind::Main);
        assert_eq!(fetched.name, "OpenLife Main Agent");
        assert!(fetched.can_access_lifemodel);
        assert!(fetched.can_access_memory_evidence);
        assert!(fetched.active);
    }

    #[test]
    fn test_set_default_main_spec_is_atomic_on_error() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let _ = store.set_default_main_spec("nonexistent");
        let default = store.get_default_spec().unwrap().unwrap();
        assert_eq!(default.id, "main.default");
        assert!(default.active);
    }

    // ── Phase 2: corrupted spec_json must not silently degrade to default ──

    #[test]
    fn test_corrupt_spec_json_returns_store_error() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        // Directly insert a row with illegal JSON
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO agent_specs (id, spec_json, active, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    "corrupt.test",
                    "not-valid-json{{{",
                    1,
                    "2026-01-01T00:00:00Z",
                    "2026-01-01T00:00:00Z"
                ],
            )
            .unwrap();
        }

        // get_spec should return Store error, not silently return default
        let result = store.get_spec_optional("corrupt.test");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("corrupt spec_json"),
            "should report corruption, not silently degrade, got: {}",
            err_msg
        );

        // list_specs should also fail (propagates through row_to_spec)
        let list_result = store.list_specs();
        assert!(
            list_result.is_err(),
            "list_specs should fail on corrupt JSON"
        );
    }

    #[test]
    fn test_create_and_get_custom_spec() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let custom = AgentSpec::new(AgentRoleKind::Planner, "Test Planner", "planning tasks")
            .with_id("test.planner".to_string())
            .with_read_only();
        store.create_spec(&custom).unwrap();

        let fetched = store.get_spec_optional("test.planner").unwrap();
        assert!(fetched.is_some());
        let f = fetched.unwrap();
        assert_eq!(f.role, AgentRoleKind::Planner);
        assert!(f.read_only);
    }

    #[test]
    fn test_inactive_specs_not_selected_as_default() {
        let store = AgentSpecStore::new_in_memory().unwrap();

        // Create a second main spec and activate it, deactivate main.default
        let mut alt_spec = AgentSpec::new(AgentRoleKind::Main, "Alt Main", "alternative")
            .with_id("main.alt".to_string())
            .with_lifemodel_access()
            .with_memory_evidence();
        alt_spec.active = true;
        store.create_spec(&alt_spec).unwrap();

        // Deactivate main.default
        store.set_active("main.default", false).unwrap();

        // Default should now be main.alt
        let default = store.get_default_spec().unwrap();
        assert!(default.is_some());
        assert_eq!(default.unwrap().id, "main.alt");

        // main.default should still be gettable
        let main_default = store.get_spec_optional("main.default").unwrap();
        assert!(main_default.is_some());
        assert!(!main_default.unwrap().active);
    }

    #[test]
    fn test_unknown_spec_id_returns_structured_error() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let err = store.get_spec("nonexistent").unwrap_err();
        assert!(matches!(err, AgentSpecStoreError::NotFound(_)));
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn test_create_duplicate_returns_already_exists() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let spec = AgentSpec::new(AgentRoleKind::Planner, "Planner", "test")
            .with_id("dup.test".to_string());
        store.create_spec(&spec).unwrap();
        let err = store.create_spec(&spec).unwrap_err();
        assert!(matches!(err, AgentSpecStoreError::AlreadyExists(_)));
        assert!(err.to_string().contains("dup.test"));
    }

    #[test]
    fn test_update_preserves_id_and_role() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let mut spec = store.get_default_spec().unwrap().unwrap();
        let original_id = spec.id.clone();
        let original_role = spec.role.clone();

        spec.name = "Updated Name".to_string();
        spec.purpose = "Updated purpose".to_string();
        store.update_spec(&spec).unwrap();

        let fetched = store.get_default_spec().unwrap().unwrap();
        assert_eq!(fetched.id, original_id);
        assert_eq!(fetched.role, original_role);
        assert_eq!(fetched.name, "Updated Name");
        assert_eq!(fetched.purpose, "Updated purpose");
    }

    #[test]
    fn test_set_active_deactivates() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        store.set_active("main.default", false).unwrap();
        let spec = store.get_spec_optional("main.default").unwrap().unwrap();
        assert!(!spec.active);
    }

    #[test]
    fn test_resolve_spec_with_explicit_id() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let spec = store.resolve_spec(Some("main.default")).unwrap();
        assert_eq!(spec.id, "main.default");
    }

    #[test]
    fn test_resolve_spec_without_explicit_returns_default() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let spec = store.resolve_spec(None).unwrap();
        assert_eq!(spec.id, "main.default");
    }

    #[test]
    fn test_resolve_spec_with_unknown_explicit_fails() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let err = store.resolve_spec(Some("nonexistent")).unwrap_err();
        assert!(matches!(err, AgentSpecStoreError::NotFound(_)));
    }

    // ── P7 stabilization: atomic set_default_main_spec ──────────────────

    #[test]
    fn test_set_default_to_missing_id_leaves_main_default_active() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let result = store.set_default_main_spec("nonexistent");
        assert!(result.is_err());
        // main.default must still be active
        let default = store.get_default_spec().unwrap().unwrap();
        assert_eq!(default.id, "main.default");
        assert!(default.active);
        // No other main spec should have been activated
        let active_mains: Vec<_> = store
            .list_specs()
            .unwrap()
            .into_iter()
            .filter(|s| s.active && s.role == AgentRoleKind::Main)
            .collect();
        assert_eq!(
            active_mains.len(),
            1,
            "only main.default should remain active"
        );
        assert_eq!(active_mains[0].id, "main.default");
    }

    #[test]
    fn test_set_default_to_non_main_spec_leaves_main_default_active() {
        let store = AgentSpecStore::new_in_memory().unwrap();
        let planner = AgentSpec::new(AgentRoleKind::Planner, "Planner", "test")
            .with_id("planner.test".to_string());
        store.create_spec(&planner).unwrap();

        let result = store.set_default_main_spec("planner.test");
        assert!(result.is_err());
        // main.default must still be active
        let default = store.get_default_spec().unwrap().unwrap();
        assert_eq!(default.id, "main.default");
        assert!(default.active);
        // No other main spec should have been activated
        let active_mains: Vec<_> = store
            .list_specs()
            .unwrap()
            .into_iter()
            .filter(|s| s.active && s.role == AgentRoleKind::Main)
            .collect();
        assert_eq!(
            active_mains.len(),
            1,
            "only main.default should remain active"
        );
    }

    #[test]
    fn test_set_default_to_alternate_main_switches_default() {
        let store = AgentSpecStore::new_in_memory().unwrap();

        // Create an alternate main spec
        let alt = AgentSpec::new(AgentRoleKind::Main, "Alt Main", "alternative")
            .with_id("main.alt".to_string())
            .with_lifemodel_access()
            .with_memory_evidence();
        store.create_spec(&alt).unwrap();

        // Switch default to main.alt
        store.set_default_main_spec("main.alt").unwrap();

        // main.alt should now be the default
        let default = store.get_default_spec().unwrap().unwrap();
        assert_eq!(default.id, "main.alt");
        assert!(default.active);

        // main.default should be deactivated
        let old = store.get_spec_optional("main.default").unwrap().unwrap();
        assert!(!old.active);
    }
}
