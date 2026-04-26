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
                started_at TEXT NOT NULL,
                finished_at TEXT
            )",
            [],
        )?;
        // Migration: add generated_proposals_json if table exists without it
        let _ = conn.execute(
            "ALTER TABLE agent_runs ADD COLUMN generated_proposals_json TEXT DEFAULT '[]'",
            [],
        );
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_session ON agent_runs(session_id, started_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_runs_started ON agent_runs(started_at DESC)",
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
                generated_proposals_json, started_at, finished_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                finished_at = ?8
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
                    generated_proposals_json, started_at, finished_at
             FROM agent_runs WHERE id = ?1",
        )?;
        let row = stmt.query_row([run_id], |row| {
            let status_str: String = row.get(3)?;
            let kind_str: String = row.get(4)?;
            let context_summary_json: Option<String> = row.get(6)?;
            let model_route_json: Option<String> = row.get(7)?;
            let error_json: Option<String> = row.get(9)?;
            let generated_proposals_json: String = row.get(10)?;
            let started_at_str: String = row.get(11)?;
            let finished_at_str: Option<String> = row.get(12)?;

            let status = match status_str.as_str() {
                "running" => AgentRunStatus::Running,
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
                _ => AgentTaskKind::Conversation,
            };

            let context_summary = context_summary_json.and_then(|s| serde_json::from_str(&s).ok());
            let model_route = model_route_json.and_then(|s| serde_json::from_str(&s).ok());
            let error = error_json.and_then(|s| serde_json::from_str(&s).ok());
            let generated_proposals: Vec<String> = serde_json::from_str(&generated_proposals_json)
                .unwrap_or_default();

            let started_at = chrono::DateTime::parse_from_rfc3339(&started_at_str)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        11,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
                .with_timezone(&chrono::Utc);
            let finished_at = finished_at_str
                .map(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .flatten()
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
                    generated_proposals_json, started_at, finished_at
             FROM agent_runs
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
                    generated_proposals_json, started_at, finished_at
             FROM agent_runs
             WHERE session_id = ?1
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
        let generated_proposals_json: String = row.get(10)?;
        let started_at_str: String = row.get(11)?;
        let finished_at_str: Option<String> = row.get(12)?;

        let status = match status_str.as_str() {
            "running" => AgentRunStatus::Running,
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
            _ => AgentTaskKind::Conversation,
        };

        let context_summary = context_summary_json.and_then(|s| serde_json::from_str(&s).ok());
        let model_route = model_route_json.and_then(|s| serde_json::from_str(&s).ok());
        let error = error_json.and_then(|s| serde_json::from_str(&s).ok());
        let generated_proposals: Vec<String> = serde_json::from_str(&generated_proposals_json)
            .unwrap_or_default();

        let started_at = chrono::DateTime::parse_from_rfc3339(&started_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    11,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&chrono::Utc);
        let finished_at = finished_at_str
            .map(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .flatten()
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
                    generated_proposals_json, started_at, finished_at
             FROM agent_runs
             WHERE session_id = ?1
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
        };
        let context_summary = ContextSummary {
            life_model_empty: true,
            included_life_model_sections: vec![],
            memory_hit_count: 0,
            used_tools_prompt: false,
            redaction_applied: false,
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
}
