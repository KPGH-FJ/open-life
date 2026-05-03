use crate::agent::types::{AgentRun, AgentRunStatus, AgentTaskKind};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AgentRunStore {
    conn: Mutex<Connection>,
}

impl AgentRunStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open agent_runs db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("failed to open in-memory agent_runs db")?;
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
            "CREATE TABLE IF NOT EXISTS agent_runs (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                session_id TEXT,
                status TEXT NOT NULL,
                kind TEXT NOT NULL,
                user_input TEXT,
                context_summary_json TEXT,
                model_route_json TEXT,
                output_preview TEXT,
                error_json TEXT,
                generated_proposals_json TEXT DEFAULT '[]',
                actions_json TEXT DEFAULT '[]',
                observations_json TEXT DEFAULT '[]',
                reasoning_strategy TEXT,
                reasoning_trace_json TEXT,
                deleted_at TEXT,
                delete_reason TEXT,
                started_at TEXT NOT NULL,
                finished_at TEXT
            )",
            [],
        )?;
        // Migration: add columns with idempotent helper
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "generated_proposals_json",
            "TEXT DEFAULT '[]'",
        )?;
        Self::add_column_if_missing(&conn, "agent_runs", "deleted_at", "TEXT")?;
        Self::add_column_if_missing(&conn, "agent_runs", "delete_reason", "TEXT")?;
        Self::add_column_if_missing(&conn, "agent_runs", "actions_json", "TEXT DEFAULT '[]'")?;
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "observations_json",
            "TEXT DEFAULT '[]'",
        )?;
        Self::add_column_if_missing(&conn, "agent_runs", "reasoning_strategy", "TEXT")?;
        Self::add_column_if_missing(&conn, "agent_runs", "reasoning_trace_json", "TEXT")?;
        // Phase 0 migration: status_updates, step_count, tool_call_count
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "status_updates_json",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "step_count",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        Self::add_column_if_missing(
            &conn,
            "agent_runs",
            "tool_call_count",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_session ON agent_runs(session_id, started_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_started ON agent_runs(started_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_deleted_at ON agent_runs(deleted_at)",
            [],
        )?;
        Ok(())
    }

    fn add_column_if_missing(
        conn: &Connection,
        table: &str,
        column: &str,
        definition: &str,
    ) -> Result<()> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for col in columns {
            if col? == column {
                return Ok(());
            }
        }
        conn.execute(
            &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition),
            [],
        )?;
        Ok(())
    }

    pub fn create_run(&self, run: &AgentRun) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO agent_runs (
                id, task_id, session_id, status, kind, user_input,
                context_summary_json, model_route_json, output_preview, error_json,
                generated_proposals_json, actions_json, observations_json,
                reasoning_strategy, reasoning_trace_json,
                status_updates_json, step_count, tool_call_count,
                deleted_at, delete_reason, started_at, finished_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            params![
                run.id,
                run.task_id,
                run.session_id,
                run.status.to_string(),
                run.kind.to_string(),
                run.user_input,
                run.context_summary
                    .as_ref()
                    .map(|c| serde_json::to_string(c).unwrap_or_default()),
                run.model_route
                    .as_ref()
                    .map(|m| serde_json::to_string(m).unwrap_or_default()),
                run.output_preview,
                run.error
                    .as_ref()
                    .map(|e| serde_json::to_string(e).unwrap_or_default()),
                serde_json::to_string(&run.generated_proposals).unwrap_or_default(),
                serde_json::to_string(&run.actions).unwrap_or_default(),
                serde_json::to_string(&run.observations).unwrap_or_default(),
                run.reasoning_strategy,
                run.reasoning_trace
                    .as_ref()
                    .map(|t| serde_json::to_string(t).unwrap_or_default()),
                serde_json::to_string(&run.status_updates).unwrap_or_default(),
                run.step_count,
                run.tool_call_count,
                run.deleted_at.map(|t| t.to_rfc3339()),
                run.delete_reason,
                run.started_at.to_rfc3339(),
                run.finished_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn update_run(&self, run: &AgentRun) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "UPDATE agent_runs SET
                status = ?2,
                context_summary_json = ?3,
                model_route_json = ?4,
                output_preview = ?5,
                error_json = ?6,
                generated_proposals_json = ?7,
                actions_json = ?8,
                observations_json = ?9,
                reasoning_strategy = ?10,
                reasoning_trace_json = ?11,
                status_updates_json = ?12,
                step_count = ?13,
                tool_call_count = ?14,
                deleted_at = ?15,
                delete_reason = ?16,
                finished_at = ?17
            WHERE id = ?1",
            params![
                run.id,
                run.status.to_string(),
                run.context_summary
                    .as_ref()
                    .map(|c| serde_json::to_string(c).unwrap_or_default()),
                run.model_route
                    .as_ref()
                    .map(|m| serde_json::to_string(m).unwrap_or_default()),
                run.output_preview,
                run.error
                    .as_ref()
                    .map(|e| serde_json::to_string(e).unwrap_or_default()),
                serde_json::to_string(&run.generated_proposals).unwrap_or_default(),
                serde_json::to_string(&run.actions).unwrap_or_default(),
                serde_json::to_string(&run.observations).unwrap_or_default(),
                run.reasoning_strategy,
                run.reasoning_trace
                    .as_ref()
                    .map(|t| serde_json::to_string(t).unwrap_or_default()),
                serde_json::to_string(&run.status_updates).unwrap_or_default(),
                run.step_count,
                run.tool_call_count,
                run.deleted_at.map(|t| t.to_rfc3339()),
                run.delete_reason,
                run.finished_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn get_run(&self, run_id: &str) -> Result<Option<AgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, session_id, status, kind, user_input,
                    context_summary_json, model_route_json, output_preview, error_json,
                    generated_proposals_json, actions_json, observations_json,
                    reasoning_strategy, reasoning_trace_json,
                    status_updates_json, step_count, tool_call_count,
                    deleted_at, delete_reason, started_at, finished_at
             FROM agent_runs WHERE id = ?1",
        )?;
        let row = stmt.query_row([run_id], |row| {
            let status_str: String = row.get(3)?;
            let kind_str: String = row.get(4)?;
            let context_summary_json: Option<String> = row.get(6)?;
            let model_route_json: Option<String> = row.get(7)?;
            let error_json: Option<String> = row.get(9)?;
            let generated_proposals_json: Option<String> = row.get(10)?;
            let actions_json: Option<String> = row.get(11)?;
            let observations_json: Option<String> = row.get(12)?;
            let reasoning_strategy: Option<String> = row.get(13)?;
            let reasoning_trace_json: Option<String> = row.get(14)?;
            let status_updates_json: Option<String> = row.get(15)?;
            let step_count: u32 = row.get(16)?;
            let tool_call_count: u32 = row.get(17)?;
            let deleted_at_str: Option<String> = row.get(18)?;
            let delete_reason: Option<String> = row.get(19)?;
            let started_at_str: String = row.get(20)?;
            let finished_at_str: Option<String> = row.get(21)?;

            let status = match status_str.as_str() {
                "running" => AgentRunStatus::Running,
                "waiting_permission" => AgentRunStatus::WaitingPermission,
                "completed" => AgentRunStatus::Completed,
                "failed" => AgentRunStatus::Failed,
                "cancelled" => AgentRunStatus::Cancelled,
                _ => AgentRunStatus::Running,
            };

            let kind = match kind_str.as_str() {
                "conversation" => AgentTaskKind::Conversation,
                "builder" => AgentTaskKind::Builder,
                "calibration" => AgentTaskKind::Calibration,
                "evolution" => AgentTaskKind::Evolution,
                "tool_execution" => AgentTaskKind::ToolExecution,
                "proactive" => AgentTaskKind::Proactive,
                "planning" => AgentTaskKind::Planning,
                "review" => AgentTaskKind::Review,
                "writing" => AgentTaskKind::Writing,
                "memory_governance" => AgentTaskKind::MemoryGovernance,
                _ => AgentTaskKind::Conversation,
            };

            let context_summary = context_summary_json.and_then(|s| serde_json::from_str(&s).ok());
            let model_route = model_route_json.and_then(|s| serde_json::from_str(&s).ok());
            let error = error_json.and_then(|s| serde_json::from_str(&s).ok());
            let generated_proposals: Vec<String> = generated_proposals_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            let actions: Vec<crate::agent::types::AgentAction> = actions_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            let observations: Vec<crate::agent::types::AgentObservation> = observations_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            let reasoning_trace = reasoning_trace_json.and_then(|s| serde_json::from_str(&s).ok());
            let deleted_at = deleted_at_str
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            let started_at = chrono::DateTime::parse_from_rfc3339(&started_at_str)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        17,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
                .with_timezone(&chrono::Utc);
            let finished_at = finished_at_str
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            Ok(AgentRun {
                id: row.get(0)?,
                task_id: row.get(1)?,
                session_id: row.get(2)?,
                status,
                kind,
                user_input: row.get(5)?,
                context_summary,
                model_route,
                output_preview: row.get(8)?,
                error,
                generated_proposals,
                actions,
                observations,
                reasoning_strategy,
                reasoning_trace,
                warnings: Vec::new(),
                status_updates: status_updates_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default(),
                step_count,
                tool_call_count,
                deleted_at,
                delete_reason,
                started_at,
                finished_at,
            })
        });
        match row {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_runs(&self, limit: i64, offset: i64) -> Result<Vec<AgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, session_id, status, kind, user_input,
                    context_summary_json, model_route_json, output_preview, error_json,
                    generated_proposals_json, actions_json, observations_json,
                    reasoning_strategy, reasoning_trace_json,
                    status_updates_json, step_count, tool_call_count,
                    deleted_at, delete_reason, started_at, finished_at
             FROM agent_runs
             WHERE deleted_at IS NULL
             ORDER BY started_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let runs = stmt.query_map([limit, offset], Self::row_to_run)?;
        runs.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn list_runs_for_session(&self, session_id: &str, limit: i64) -> Result<Vec<AgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, session_id, status, kind, user_input,
                    context_summary_json, model_route_json, output_preview, error_json,
                    generated_proposals_json, actions_json, observations_json,
                    reasoning_strategy, reasoning_trace_json,
                    status_updates_json, step_count, tool_call_count,
                    deleted_at, delete_reason, started_at, finished_at
             FROM agent_runs
             WHERE session_id = ?1 AND deleted_at IS NULL
             ORDER BY started_at DESC
             LIMIT ?2",
        )?;
        let runs = stmt.query_map(rusqlite::params![session_id, limit], Self::row_to_run)?;
        runs.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRun> {
        let status_str: String = row.get(3)?;
        let kind_str: String = row.get(4)?;
        let context_summary_json: Option<String> = row.get(6)?;
        let model_route_json: Option<String> = row.get(7)?;
        let error_json: Option<String> = row.get(9)?;
        let generated_proposals_json: Option<String> = row.get(10)?;
        let actions_json: Option<String> = row.get(11)?;
        let observations_json: Option<String> = row.get(12)?;
        let reasoning_strategy: Option<String> = row.get(13)?;
        let reasoning_trace_json: Option<String> = row.get(14)?;
        let status_updates_json: Option<String> = row.get(15)?;
        let step_count: u32 = row.get(16)?;
        let tool_call_count: u32 = row.get(17)?;
        let deleted_at_str: Option<String> = row.get(18)?;
        let delete_reason: Option<String> = row.get(19)?;
        let started_at_str: String = row.get(20)?;
        let finished_at_str: Option<String> = row.get(21)?;

        let status = match status_str.as_str() {
            "running" => AgentRunStatus::Running,
            "waiting_permission" => AgentRunStatus::WaitingPermission,
            "completed" => AgentRunStatus::Completed,
            "failed" => AgentRunStatus::Failed,
            "cancelled" => AgentRunStatus::Cancelled,
            _ => AgentRunStatus::Running,
        };

        let kind = match kind_str.as_str() {
            "conversation" => AgentTaskKind::Conversation,
            "builder" => AgentTaskKind::Builder,
            "calibration" => AgentTaskKind::Calibration,
            "evolution" => AgentTaskKind::Evolution,
            "tool_execution" => AgentTaskKind::ToolExecution,
            "proactive" => AgentTaskKind::Proactive,
            "planning" => AgentTaskKind::Planning,
            "review" => AgentTaskKind::Review,
            "writing" => AgentTaskKind::Writing,
            "memory_governance" => AgentTaskKind::MemoryGovernance,
            _ => AgentTaskKind::Conversation,
        };

        let context_summary = context_summary_json.and_then(|s| serde_json::from_str(&s).ok());
        let model_route = model_route_json.and_then(|s| serde_json::from_str(&s).ok());
        let error = error_json.and_then(|s| serde_json::from_str(&s).ok());
        let generated_proposals: Vec<String> = generated_proposals_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let actions: Vec<crate::agent::types::AgentAction> = actions_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let observations: Vec<crate::agent::types::AgentObservation> = observations_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let reasoning_trace = reasoning_trace_json.and_then(|s| serde_json::from_str(&s).ok());
        let deleted_at = deleted_at_str
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let started_at = chrono::DateTime::parse_from_rfc3339(&started_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    17,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&chrono::Utc);
        let finished_at = finished_at_str
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        Ok(AgentRun {
            id: row.get(0)?,
            task_id: row.get(1)?,
            session_id: row.get(2)?,
            status,
            kind,
            user_input: row.get(5)?,
            context_summary,
            model_route,
            output_preview: row.get(8)?,
            error,
            generated_proposals,
            actions,
            observations,
            reasoning_strategy,
            reasoning_trace,
            warnings: Vec::new(),
            status_updates: status_updates_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default(),
            step_count,
            tool_call_count,
            deleted_at,
            delete_reason,
            started_at,
            finished_at,
        })
    }

    pub fn run_count(&self) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM agent_runs", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn last_run_for_session(&self, session_id: &str) -> Result<Option<AgentRun>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, session_id, status, kind, user_input,
                    context_summary_json, model_route_json, output_preview, error_json,
                    generated_proposals_json, actions_json, observations_json,
                    reasoning_strategy, reasoning_trace_json,
                    status_updates_json, step_count, tool_call_count,
                    deleted_at, delete_reason, started_at, finished_at
             FROM agent_runs
             WHERE session_id = ?1 AND deleted_at IS NULL
             ORDER BY started_at DESC
             LIMIT 1",
        )?;
        let row = stmt.query_row([session_id], Self::row_to_run);
        match row {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn add_generated_proposal(&self, run_id: &str, proposal_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        // Fetch current proposals
        let current: String = conn.query_row(
            "SELECT generated_proposals_json FROM agent_runs WHERE id = ?1",
            [run_id],
            |row| row.get(0),
        )?;
        let mut proposals: Vec<String> = serde_json::from_str(&current).unwrap_or_default();
        proposals.push(proposal_id.to_string());
        let updated = serde_json::to_string(&proposals).unwrap_or_default();
        conn.execute(
            "UPDATE agent_runs SET generated_proposals_json = ?2 WHERE id = ?1",
            rusqlite::params![run_id, updated],
        )?;
        Ok(())
    }

    pub fn delete_run(&self, run_id: &str, reason: Option<&str>) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let deleted_at = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE agent_runs SET deleted_at = ?2, delete_reason = ?3 WHERE id = ?1",
            rusqlite::params![run_id, deleted_at, reason],
        )?;
        Ok(())
    }

    pub fn restore_run(&self, run_id: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "UPDATE agent_runs SET deleted_at = NULL, delete_reason = NULL WHERE id = ?1",
            [run_id],
        )?;
        Ok(())
    }

    pub fn cleanup_old_deleted_runs(&self, days: i64) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let rows_affected = conn.execute(
            "DELETE FROM agent_runs WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
            [cutoff],
        )?;
        Ok(rows_affected)
    }

    pub fn add_action(
        &self,
        run_id: &str,
        action: &crate::agent::types::AgentAction,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let current: String = conn.query_row(
            "SELECT actions_json FROM agent_runs WHERE id = ?1",
            [run_id],
            |row| row.get(0),
        )?;
        let mut actions: Vec<crate::agent::types::AgentAction> =
            serde_json::from_str(&current).unwrap_or_default();
        actions.push(action.clone());
        let updated = serde_json::to_string(&actions).unwrap_or_default();
        conn.execute(
            "UPDATE agent_runs SET actions_json = ?2 WHERE id = ?1",
            rusqlite::params![run_id, updated],
        )?;
        Ok(())
    }

    pub fn add_observation(
        &self,
        run_id: &str,
        observation: &crate::agent::types::AgentObservation,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let current: String = conn.query_row(
            "SELECT observations_json FROM agent_runs WHERE id = ?1",
            [run_id],
            |row| row.get(0),
        )?;
        let mut observations: Vec<crate::agent::types::AgentObservation> =
            serde_json::from_str(&current).unwrap_or_default();
        observations.push(observation.clone());
        let updated = serde_json::to_string(&observations).unwrap_or_default();
        conn.execute(
            "UPDATE agent_runs SET observations_json = ?2 WHERE id = ?1",
            rusqlite::params![run_id, updated],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{AgentRunError, ContextSummary, ModelRouteTrace};

    fn create_test_run() -> AgentRun {
        AgentRun::new_chat_run("test-session", "Hello world")
    }

    #[test]
    fn test_create_and_get_run() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run = create_test_run();
        store.create_run(&run).unwrap();

        let fetched = store.get_run(&run.id).unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, run.id);
        assert_eq!(fetched.session_id, Some("test-session".to_string()));
        assert_eq!(fetched.user_input, Some("Hello world".to_string()));
        assert_eq!(fetched.status, AgentRunStatus::Running);
        assert_eq!(fetched.kind, AgentTaskKind::Conversation);
    }

    #[test]
    fn test_update_run() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        store.create_run(&run).unwrap();

        let model_route = ModelRouteTrace {
            provider: "openrouter".to_string(),
            model: "deepseek-chat".to_string(),
            route_type: "cloud".to_string(),
            prefer_local: false,
            local_model: "llama3.2".to_string(),
            reason: "no_ollama".to_string(),
            privacy_level: crate::agent::types::RedactionLevel::None,
            latency_ms: None,
            retry_count: 0,
            fallback_reason: Some("no_ollama".to_string()),
            provider_health_is_estimated: Some(true),
        };
        let context_summary = ContextSummary {
            life_model_empty: true,
            included_life_model_sections: vec![],
            memory_hit_count: 0,
            memory_sources: vec![],
            used_tools_prompt: false,
            redaction_applied: false,
            redaction_level: crate::agent::types::RedactionLevel::None,
        };
        run.complete("Hello! How can I help?", model_route, context_summary);
        store.update_run(&run).unwrap();

        let fetched = store.get_run(&run.id).unwrap().unwrap();
        assert_eq!(fetched.status, AgentRunStatus::Completed);
        assert_eq!(
            fetched.output_preview,
            Some("Hello! How can I help?".to_string())
        );
        assert!(fetched.model_route.is_some());
        assert!(fetched.context_summary.is_some());
        assert!(fetched.finished_at.is_some());
    }

    #[test]
    fn test_list_runs() {
        let store = AgentRunStore::new_in_memory().unwrap();
        for i in 0..5 {
            let run = AgentRun::new_chat_run("session-1", &format!("msg {}", i));
            store.create_run(&run).unwrap();
        }

        let runs = store.list_runs(10, 0).unwrap();
        assert_eq!(runs.len(), 5);

        let session_runs = store.list_runs_for_session("session-1", 10).unwrap();
        assert_eq!(session_runs.len(), 5);
    }

    #[test]
    fn test_run_count() {
        let store = AgentRunStore::new_in_memory().unwrap();
        assert_eq!(store.run_count().unwrap(), 0);

        let run = create_test_run();
        store.create_run(&run).unwrap();
        assert_eq!(store.run_count().unwrap(), 1);
    }

    #[test]
    fn test_last_run_for_session() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run1 = AgentRun::new_chat_run("session-1", "first");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let run2 = AgentRun::new_chat_run("session-1", "second");
        store.create_run(&run1).unwrap();
        store.create_run(&run2).unwrap();

        let last = store.last_run_for_session("session-1").unwrap();
        assert!(last.is_some());
        assert_eq!(last.unwrap().user_input, Some("second".to_string()));
    }

    #[test]
    fn test_fail_run() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let mut run = create_test_run();
        store.create_run(&run).unwrap();

        let error = AgentRunError {
            message: "model timeout".to_string(),
            phase: "model".to_string(),
            recoverable: true,
        };
        run.fail(error);
        store.update_run(&run).unwrap();

        let fetched = store.get_run(&run.id).unwrap().unwrap();
        assert_eq!(fetched.status, AgentRunStatus::Failed);
        assert!(fetched.error.is_some());
    }

    #[test]
    fn test_restore_run() {
        let store = AgentRunStore::new_in_memory().unwrap();
        let run = create_test_run();
        store.create_run(&run).unwrap();

        // Soft delete
        store.delete_run(&run.id, Some("test deletion")).unwrap();
        let fetched = store.get_run(&run.id).unwrap().unwrap();
        assert!(fetched.deleted_at.is_some());
        assert_eq!(fetched.delete_reason, Some("test deletion".to_string()));

        // Restore
        store.restore_run(&run.id).unwrap();
        let restored = store.get_run(&run.id).unwrap().unwrap();
        assert!(restored.deleted_at.is_none());
        assert!(restored.delete_reason.is_none());
    }
}
