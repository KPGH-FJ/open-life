use crate::agent::types::{AgentRunEvent, AgentRunEventType};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Append-only event store for AgentRunEvent records.
/// Colocated with AgentRunStore; uses a separate `agent_run_events` table.
/// Uses Arc<Mutex<Connection>> internally for cheap Clone.
#[derive(Clone)]
pub struct AgentRunEventStore {
    conn: Arc<Mutex<Connection>>,
}

impl AgentRunEventStore {
    /// Open an event store at the given path. Creates the database and table
    /// if they do not exist.
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open event store at {:?}", db_path))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_tables()?;
        Ok(store)
    }

    /// Open an in-memory event store (for tests).
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory event store")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
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
            "CREATE TABLE IF NOT EXISTS agent_run_events (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                parent_event_id TEXT,
                event_type TEXT NOT NULL,
                phase TEXT,
                actor TEXT NOT NULL,
                summary TEXT NOT NULL,
                payload_json TEXT NOT NULL DEFAULT '{}',
                redaction_json TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_run_events_run_id ON agent_run_events(run_id, created_at ASC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_run_events_type ON agent_run_events(event_type)",
            [],
        )?;
        Ok(())
    }

    /// Append a single event. Returns the event id.
    pub fn append_event(&self, event: &AgentRunEvent) -> Result<String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO agent_run_events (
                id, run_id, parent_event_id, event_type, phase, actor,
                summary, payload_json, redaction_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                event.id,
                event.run_id,
                event.parent_event_id,
                event.event_type.to_string(),
                event.phase,
                event.actor.to_string(),
                event.summary,
                serde_json::to_string(&event.payload).unwrap_or_default(),
                event
                    .redaction
                    .as_ref()
                    .map(|r| serde_json::to_string(r).unwrap_or_default()),
                event.created_at.to_rfc3339(),
            ],
        )?;
        Ok(event.id.clone())
    }

    /// List events for a run, ordered by creation time ascending.
    pub fn list_events_by_run(&self, run_id: &str) -> Result<Vec<AgentRunEvent>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, parent_event_id, event_type, phase, actor,
                    summary, payload_json, redaction_json, created_at
             FROM agent_run_events
             WHERE run_id = ?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([run_id], |row| {
            let event_type_str: String = row.get(3)?;
            let actor_str: String = row.get(5)?;
            let created_at_str: String = row.get(9)?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
                .with_timezone(&chrono::Utc);

            Ok(AgentRunEvent {
                id: row.get(0)?,
                run_id: row.get(1)?,
                parent_event_id: row.get(2)?,
                event_type: parse_event_type(&event_type_str),
                phase: row.get(4)?,
                actor: parse_event_actor(&actor_str),
                summary: row.get(6)?,
                payload: row
                    .get::<_, String>(7)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or(serde_json::json!({})),
                redaction: row
                    .get::<_, Option<String>>(8)
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str(&s).ok()),
                created_at,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    /// Count events for a run.
    pub fn count_events_by_run(&self, run_id: &str) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let count: usize = conn.query_row(
            "SELECT COUNT(*) FROM agent_run_events WHERE run_id = ?1",
            [run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Count events by type.
    pub fn count_events_by_type(&self, event_type: AgentRunEventType) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let event_type = event_type.to_string();
        let count: usize = conn.query_row(
            "SELECT COUNT(*) FROM agent_run_events WHERE event_type = ?1",
            [event_type],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

fn parse_event_type(s: &str) -> AgentRunEventType {
    match s {
        "run.created" => AgentRunEventType::RunCreated,
        "context.assembled" => AgentRunEventType::ContextAssembled,
        "agent_spec.selected" => AgentRunEventType::AgentSpecSelected,
        "prompt_stack.assembled" => AgentRunEventType::PromptStackAssembled,
        "context_governance.applied" => AgentRunEventType::ContextGovernanceApplied,
        "model.route_selected" => AgentRunEventType::ModelRouteSelected,
        "model.call_started" => AgentRunEventType::ModelCallStarted,
        "model.call_completed" => AgentRunEventType::ModelCallCompleted,
        "model.call_failed" => AgentRunEventType::ModelCallFailed,
        "tool.call_started" => AgentRunEventType::ToolCallStarted,
        "tool.call_blocked" => AgentRunEventType::ToolCallBlocked,
        "tool.call_completed" => AgentRunEventType::ToolCallCompleted,
        "tool.call_failed" => AgentRunEventType::ToolCallFailed,
        "observation.created" => AgentRunEventType::ObservationCreated,
        "proposal.created" => AgentRunEventType::ProposalCreated,
        "fallback.started" => AgentRunEventType::FallbackStarted,
        "fallback.completed" => AgentRunEventType::FallbackCompleted,
        "fallback.failed" => AgentRunEventType::FallbackFailed,
        "json_repair.started" => AgentRunEventType::JsonRepairStarted,
        "json_repair.completed" => AgentRunEventType::JsonRepairCompleted,
        "plan.created" => AgentRunEventType::PlanCreated,
        "plan.confirmation_requested" => AgentRunEventType::PlanConfirmationRequested,
        "plan.confirmation_resolved" => AgentRunEventType::PlanConfirmationResolved,
        "plan.execution_started" => AgentRunEventType::PlanExecutionStarted,
        "plan.step_started" => AgentRunEventType::PlanStepStarted,
        "plan.step_completed" => AgentRunEventType::PlanStepCompleted,
        "plan.step_failed" => AgentRunEventType::PlanStepFailed,
        "plan.deviation_recorded" => AgentRunEventType::PlanDeviationRecorded,
        "plan.execution_completed" => AgentRunEventType::PlanExecutionCompleted,
        "plan.execution_failed" => AgentRunEventType::PlanExecutionFailed,
        "plan.cancel_requested" => AgentRunEventType::PlanCancelRequested,
        "plan.cancelled" => AgentRunEventType::PlanCancelled,
        "plan.retry_requested" => AgentRunEventType::PlanRetryRequested,
        "plan.retry_started" => AgentRunEventType::PlanRetryStarted,
        "plan.continuation_requested" => AgentRunEventType::PlanContinuationRequested,
        "plan.action_replayed" => AgentRunEventType::PlanActionReplayed,
        "plan.action_replay_requested" => AgentRunEventType::PlanActionReplayRequested,
        "replay.started" => AgentRunEventType::ReplayStarted,
        "replay.completed" => AgentRunEventType::ReplayCompleted,
        "replay.failed" => AgentRunEventType::ReplayFailed,
        "compaction.created" => AgentRunEventType::CompactionCreated,
        "run.completed" => AgentRunEventType::RunCompleted,
        "run.failed" => AgentRunEventType::RunFailed,
        "model.failed" => AgentRunEventType::ModelFailed,
        unknown => AgentRunEventType::Unknown(unknown.to_string()),
    }
}

fn parse_event_actor(s: &str) -> crate::agent::types::AgentEventActor {
    if s == "user" {
        crate::agent::types::AgentEventActor::User
    } else if s == "agent" {
        crate::agent::types::AgentEventActor::Agent
    } else if s == "runtime" {
        crate::agent::types::AgentEventActor::Runtime
    } else if s == "system" {
        crate::agent::types::AgentEventActor::System
    } else if let Some(name) = s.strip_prefix("sub_agent:") {
        crate::agent::types::AgentEventActor::SubAgent(name.to_string())
    } else if let Some(name) = s.strip_prefix("tool:") {
        crate::agent::types::AgentEventActor::Tool(name.to_string())
    } else {
        crate::agent::types::AgentEventActor::System
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{AgentEventActor, AgentRunEventType};

    #[test]
    fn test_append_and_list_events() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "test-run-001";

        let e1 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "Run created by chat",
            serde_json::json!({"task_kind": "conversation"}),
        );
        let e2 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ModelCallStarted,
            AgentEventActor::Agent,
            "Calling deepseek-chat",
            serde_json::json!({"provider": "openrouter", "model": "deepseek-chat"}),
        );
        let e3 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ModelCallCompleted,
            AgentEventActor::Agent,
            "Model call completed",
            serde_json::json!({"latency_ms": 1234}),
        );
        let e4 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::RunCompleted,
            AgentEventActor::Runtime,
            "Run completed successfully",
            serde_json::json!({"stop_reason": "no_tools"}),
        );

        let id1 = store.append_event(&e1).unwrap();
        let id2 = store.append_event(&e2).unwrap();
        let id3 = store.append_event(&e3).unwrap();
        let id4 = store.append_event(&e4).unwrap();

        assert_eq!(id1, e1.id);
        assert_eq!(id2, e2.id);
        assert_eq!(id3, e3.id);
        assert_eq!(id4, e4.id);

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].event_type, AgentRunEventType::RunCreated);
        assert_eq!(events[1].event_type, AgentRunEventType::ModelCallStarted);
        assert_eq!(events[2].event_type, AgentRunEventType::ModelCallCompleted);
        assert_eq!(events[3].event_type, AgentRunEventType::RunCompleted);

        assert_eq!(store.count_events_by_run(run_id).unwrap(), 4);
    }

    #[test]
    fn test_events_are_appended_in_order() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "test-run-002";

        for i in 0..10 {
            let e = AgentRunEvent::new(
                run_id,
                AgentRunEventType::ModelCallStarted,
                AgentEventActor::Agent,
                format!("call {}", i),
                serde_json::json!({"index": i}),
            );
            store.append_event(&e).unwrap();
        }

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 10);
        for (idx, event) in events.iter().enumerate() {
            let payload: i32 = event.payload.get("index").unwrap().as_i64().unwrap() as i32;
            assert_eq!(payload, idx as i32);
        }
    }

    #[test]
    fn test_different_runs_isolated() {
        let store = AgentRunEventStore::new_in_memory().unwrap();

        let e_a = AgentRunEvent::new(
            "run-a",
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "A",
            serde_json::json!({}),
        );
        let e_b = AgentRunEvent::new(
            "run-b",
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "B",
            serde_json::json!({}),
        );

        store.append_event(&e_a).unwrap();
        store.append_event(&e_b).unwrap();

        assert_eq!(store.list_events_by_run("run-a").unwrap().len(), 1);
        assert_eq!(store.list_events_by_run("run-b").unwrap().len(), 1);
        assert_eq!(
            store.list_events_by_run("run-nonexistent").unwrap().len(),
            0
        );
    }

    #[test]
    fn test_event_with_parent_linkage() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "test-run-parent";

        let model_failed = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ModelCallFailed,
            AgentEventActor::Agent,
            "Model returned malformed JSON",
            serde_json::json!({"error": "parse error"}),
        );
        store.append_event(&model_failed).unwrap();

        let repair_started = AgentRunEvent::new(
            run_id,
            AgentRunEventType::JsonRepairStarted,
            AgentEventActor::Runtime,
            "Attempting JSON self-repair",
            serde_json::json!({}),
        )
        .with_parent(&model_failed.id);

        store.append_event(&repair_started).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[1].parent_event_id.as_deref(),
            Some(model_failed.id.as_str())
        );
    }

    #[test]
    fn test_events_payload_preserves_data() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "test-run-payload";

        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallCompleted,
            AgentEventActor::Tool("file.read".to_string()),
            "Read file successfully",
            serde_json::json!({
                "tool": "file.read",
                "path": "/tmp/test.txt",
                "size_bytes": 42,
                "lines": 3
            }),
        )
        .with_phase("execution");

        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        let stored = &events[0];
        assert_eq!(stored.actor, AgentEventActor::Tool("file.read".to_string()));
        assert_eq!(stored.phase.as_deref(), Some("execution"));
        assert_eq!(
            stored.payload.get("path").unwrap().as_str().unwrap(),
            "/tmp/test.txt"
        );
        assert_eq!(
            stored.payload.get("size_bytes").unwrap().as_i64().unwrap(),
            42
        );
    }

    #[test]
    fn test_fallback_failed_event_round_trip() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "test-fallback-failed";
        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::FallbackFailed,
            AgentEventActor::Runtime,
            "Governed compatibility fallback failed",
            crate::agent::trace_payloads::build_fallback_failed_payload(
                "main.default",
                "local_only",
                "model unavailable",
                "retry unavailable",
            ),
        );

        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AgentRunEventType::FallbackFailed);
        assert_eq!(events[0].payload["status"].as_str(), Some("failed"));
        assert_eq!(
            events[0].payload["generation_path"].as_str(),
            Some("generate_governed")
        );
    }

    #[test]
    fn test_events_with_redaction() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "test-run-redacted";

        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ModelCallStarted,
            AgentEventActor::System,
            "Cloud model call started",
            serde_json::json!({"model": "deepseek-chat"}),
        )
        .with_redaction(
            "life_model fields redacted",
            vec!["life_model.identity".to_string()],
        );

        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        let redaction = events[0].redaction.as_ref().unwrap();
        assert!(redaction.redacted);
        assert_eq!(redaction.reason, "life_model fields redacted");
        assert_eq!(redaction.fields_removed, vec!["life_model.identity"]);
    }

    /// Verify that cloned stores share the same underlying connection.
    #[test]
    fn test_cloned_store_shares_connection() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let clone = store.clone();
        let run_id = "test-clone-shared";

        // Write via original
        let e1 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::RunCreated,
            AgentEventActor::Runtime,
            "original write",
            serde_json::json!({}),
        );
        store.append_event(&e1).unwrap();

        // Write via clone
        let e2 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::RunCompleted,
            AgentEventActor::Runtime,
            "clone write",
            serde_json::json!({}),
        );
        clone.append_event(&e2).unwrap();

        // Both visible from either handle
        let from_original = store.list_events_by_run(run_id).unwrap();
        let from_clone = clone.list_events_by_run(run_id).unwrap();

        assert_eq!(from_original.len(), 2);
        assert_eq!(from_clone.len(), 2);
        assert_eq!(from_original[0].summary, "original write");
        assert_eq!(from_original[1].summary, "clone write");
    }

    /// Unknown/future event types must NOT be silently mapped to RunCreated.
    /// They must round-trip as Unknown(String) through the store.
    #[test]
    fn test_unknown_event_type_round_trip() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "test-unknown-events";

        // Simulate a future event type that this build doesn't know about
        let future_event = AgentRunEvent {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run_id.to_string(),
            parent_event_id: None,
            event_type: AgentRunEventType::Unknown("future.event.type".into()),
            phase: None,
            actor: AgentEventActor::System,
            summary: "Future event".into(),
            payload: serde_json::json!({}),
            redaction: None,
            created_at: chrono::Utc::now(),
        };
        store.append_event(&future_event).unwrap();

        // Read back — must be Unknown, not RunCreated
        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        assert!(
            matches!(events[0].event_type, AgentRunEventType::Unknown(ref s) if s == "future.event.type"),
            "expected Unknown(\"future.event.type\"), got {:?}",
            events[0].event_type
        );
    }

    /// Multiple unknown event types must coexist without collision.
    #[test]
    fn test_multiple_unknown_events() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "test-multi-unknown";

        for (i, event_type_str) in ["v2.new_event", "v3.enriched", "ext.audit"]
            .iter()
            .enumerate()
        {
            let event = AgentRunEvent {
                id: format!("evt-{}", i),
                run_id: run_id.to_string(),
                parent_event_id: None,
                event_type: AgentRunEventType::Unknown(event_type_str.to_string()),
                phase: None,
                actor: AgentEventActor::Runtime,
                summary: format!("Event {}", i),
                payload: serde_json::json!({"index": i}),
                redaction: None,
                created_at: chrono::Utc::now(),
            };
            store.append_event(&event).unwrap();
        }

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 3);
        // All should be Unknown, none should be RunCreated
        for evt in &events {
            assert!(
                matches!(evt.event_type, AgentRunEventType::Unknown(_)),
                "all events should be Unknown, got {:?}",
                evt.event_type
            );
        }
    }

    /// P4-1: Plan execution event types round-trip through event store.
    #[test]
    fn test_plan_execution_events_round_trip() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "p4-plan-exec-events";

        let event_types = [
            AgentRunEventType::PlanExecutionStarted,
            AgentRunEventType::PlanStepStarted,
            AgentRunEventType::PlanStepCompleted,
            AgentRunEventType::PlanStepFailed,
            AgentRunEventType::PlanDeviationRecorded,
            AgentRunEventType::PlanExecutionCompleted,
            AgentRunEventType::PlanExecutionFailed,
        ];

        for (i, event_type) in event_types.iter().enumerate() {
            let event = AgentRunEvent {
                id: format!("p4-ev-{}", i),
                run_id: run_id.to_string(),
                parent_event_id: None,
                event_type: event_type.clone(),
                phase: Some("execution".to_string()),
                actor: AgentEventActor::Runtime,
                summary: format!("plan execution event {}", event_type),
                payload: serde_json::json!({"step": i}),
                redaction: None,
                created_at: chrono::Utc::now(),
            };
            store.append_event(&event).unwrap();
        }

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 7);
        assert_eq!(
            events[0].event_type,
            AgentRunEventType::PlanExecutionStarted
        );
        assert_eq!(events[1].event_type, AgentRunEventType::PlanStepStarted);
        assert_eq!(events[2].event_type, AgentRunEventType::PlanStepCompleted);
        assert_eq!(events[3].event_type, AgentRunEventType::PlanStepFailed);
        assert_eq!(
            events[4].event_type,
            AgentRunEventType::PlanDeviationRecorded
        );
        assert_eq!(
            events[5].event_type,
            AgentRunEventType::PlanExecutionCompleted
        );
        assert_eq!(events[6].event_type, AgentRunEventType::PlanExecutionFailed);
        assert!(events
            .iter()
            .all(|e| e.phase == Some("execution".to_string())));
    }

    /// P4-1: Execution outcome types serialize and deserialize correctly.
    #[test]
    fn test_plan_execution_outcome_serialization() {
        use crate::agent::types::{
            PlanExecutionMode, PlanExecutionOutcome, PlanStepExecutionResult,
        };

        let mode = PlanExecutionMode::Sequential;
        let mode_json = serde_json::to_string(&mode).unwrap();
        assert_eq!(mode_json, r#""sequential""#);
        let mode_parsed: PlanExecutionMode = serde_json::from_str(&mode_json).unwrap();
        assert_eq!(mode_parsed, PlanExecutionMode::Sequential);

        let step_result = PlanStepExecutionResult {
            step_index: 0,
            tool_name: "life_model.read".to_string(),
            success: true,
            output: Some("read ok".to_string()),
            error: None,
            duration_ms: 42,
            deviation: None,
        };
        let json = serde_json::to_string_pretty(&step_result).unwrap();
        let parsed: PlanStepExecutionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.step_index, 0);
        assert_eq!(parsed.tool_name, "life_model.read");
        assert!(parsed.success);
        assert_eq!(parsed.duration_ms, 42);

        let outcome = PlanExecutionOutcome {
            plan_id: "plan-1".to_string(),
            success: true,
            steps_completed: 3,
            steps_failed: 0,
            deviations: vec![],
            review_required: false,
        };
        let json = serde_json::to_string_pretty(&outcome).unwrap();
        let parsed: PlanExecutionOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.plan_id, "plan-1");
        assert!(parsed.success);
        assert_eq!(parsed.steps_completed, 3);

        let failed_outcome = PlanExecutionOutcome {
            plan_id: "plan-2".to_string(),
            success: false,
            steps_completed: 1,
            steps_failed: 1,
            deviations: vec!["step 1 used different tool".to_string()],
            review_required: true,
        };
        let json = serde_json::to_string_pretty(&failed_outcome).unwrap();
        let parsed2: PlanExecutionOutcome = serde_json::from_str(&json).unwrap();
        assert!(!parsed2.success);
        assert!(parsed2.review_required);
        assert_eq!(parsed2.deviations.len(), 1);
    }

    /// P4-1: Unknown event type still round-trips preserving forward-compatibility.
    #[test]
    fn test_unknown_event_with_new_plan_events_does_not_collide() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "p4-unknown-coexist";

        // Store a known new event
        let known = AgentRunEvent {
            id: "known".to_string(),
            run_id: run_id.to_string(),
            parent_event_id: None,
            event_type: AgentRunEventType::PlanExecutionStarted,
            phase: None,
            actor: AgentEventActor::Runtime,
            summary: "known plan event".to_string(),
            payload: serde_json::json!({}),
            redaction: None,
            created_at: chrono::Utc::now(),
        };
        store.append_event(&known).unwrap();

        // Store a future unknown event
        let future = AgentRunEvent {
            id: "future".to_string(),
            run_id: run_id.to_string(),
            parent_event_id: None,
            event_type: AgentRunEventType::Unknown("plan.parallel_started".to_string()),
            phase: None,
            actor: AgentEventActor::Runtime,
            summary: "future parallel plan".to_string(),
            payload: serde_json::json!({}),
            redaction: None,
            created_at: chrono::Utc::now(),
        };
        store.append_event(&future).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0].event_type,
            AgentRunEventType::PlanExecutionStarted
        );
        assert!(
            matches!(events[1].event_type, AgentRunEventType::Unknown(ref s) if s == "plan.parallel_started")
        );
    }

    // ── P7 Stabilize: governance event payload safety ────────────────────

    /// P7-3: Governance events (AgentSpecSelected, PromptStackAssembled,
    /// ContextGovernanceApplied) must NOT contain raw prompt content,
    /// raw memory snippets, or full LifeModel data.
    #[test]
    fn test_runtime_governance_event_excludes_raw_prompt() {
        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "p7-governance-events";

        // Simulate the three governance events written in the Chat path.
        let spec_selected = AgentRunEvent::new(
            run_id,
            AgentRunEventType::AgentSpecSelected,
            AgentEventActor::Runtime,
            "AgentSpec main.default selected",
            serde_json::json!({
                "agent_spec_id": "main.default",
                "role": "main",
                "privacy_policy": "local_only",
            }),
        );
        let prompt_stack_assembled = AgentRunEvent::new(
            run_id,
            AgentRunEventType::PromptStackAssembled,
            AgentEventActor::Runtime,
            "PromptStack assembled with 2 blocks from AgentSpec main.default",
            serde_json::json!({
                "agent_spec_id": "main.default",
                "prompt_blocks": [
                    {"id": "base_system", "version": "1.0.0"},
                    {"id": "privacy_rule", "version": "1.0.0"},
                ],
            }),
        );
        let context_governance = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ContextGovernanceApplied,
            AgentEventActor::Runtime,
            "Context governance applied by AgentSpec main.default",
            serde_json::json!({
                "agent_spec_id": "main.default",
                "context_included": ["session_summary", "lifemodel_summary"],
                "context_excluded": ["memory"],
                "privacy_policy": "local_only",
            }),
        );

        store.append_event(&spec_selected).unwrap();
        store.append_event(&prompt_stack_assembled).unwrap();
        store.append_event(&context_governance).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 3);

        // All three events must round-trip with correct types
        assert_eq!(events[0].event_type, AgentRunEventType::AgentSpecSelected);
        assert_eq!(
            events[1].event_type,
            AgentRunEventType::PromptStackAssembled
        );
        assert_eq!(
            events[2].event_type,
            AgentRunEventType::ContextGovernanceApplied
        );

        // Payload must NOT contain raw prompt content, raw memory, or full LifeModel.
        for event in &events {
            let payload_str = serde_json::to_string(&event.payload).unwrap();
            assert!(
                !payload_str.contains("raw memory hit"),
                "governance event must not expose raw memory content"
            );
            assert!(
                !payload_str.contains("prompt raw content"),
                "governance event must not expose raw prompt content"
            );
            assert!(
                !payload_str.contains("life_model.identity.name"),
                "governance event must not expose full LifeModel data"
            );
            // prompt_blocks payload uses only id/version, no raw content
            assert!(
                !payload_str.contains("You are OpenLife"),
                "PromptStack event must not contain raw system prompt text"
            );
        }

        // context_excluded must show "memory" as a category label, not raw memory text
        let governance_payload = &events[2].payload;
        let excluded: Vec<String> = governance_payload
            .get("context_excluded")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        assert!(excluded.contains(&"memory".to_string()));
    }

    /// Trace Contract: Verify that tool.call_blocked events with typed governance
    /// payloads satisfy the mandatory field contract (status, tool_name, source,
    /// and either block_reason or proposal_reason).
    #[test]
    fn test_tool_call_blocked_typed_payload_contract() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-blocked-contract";

        // -- Contract case 1: AgentSpec denial --
        let payload1 = trace_payloads::build_tool_call_blocked_payload(
            "blocked",
            "web.search",
            "builtin",
            Some("main.default"),
            Some("agent_spec_denied"),
            None::<&str>,
            None::<&str>,
            None,
        );
        let event1 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallBlocked,
            AgentEventActor::Tool("web.search".into()),
            "web.search blocked by AgentSpec",
            payload1,
        );
        store.append_event(&event1).unwrap();

        // -- Contract case 2: NetworkPolicy ask --
        let payload2 = trace_payloads::build_tool_call_blocked_payload(
            "needs_confirmation",
            "web.search",
            "builtin",
            Some("main.default"),
            None::<&str>,
            Some("network_policy_ask"),
            None::<&str>,
            Some(serde_json::json!({"proposal_id": "proposal-net-1"})),
        );
        let event2 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallBlocked,
            AgentEventActor::Tool("web.search".into()),
            "web.search needs network confirmation",
            payload2,
        );
        store.append_event(&event2).unwrap();

        // -- Contract case 3: MCP target block --
        let payload3 = trace_payloads::build_tool_call_blocked_payload(
            "blocked",
            "mcp.call_tool",
            "builtin",
            Some("main.default"),
            Some("tool_permission_denied"),
            None::<&str>,
            None::<&str>,
            Some(serde_json::json!({
                "target_tool_name": "remote_search",
                "target_source": "mcp:my-server",
                "wrapper_tool_name": "mcp.call_tool",
            })),
        );
        let event3 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallBlocked,
            AgentEventActor::Tool("mcp.call_tool".into()),
            "mcp.call_tool target denied",
            payload3,
        );
        store.append_event(&event3).unwrap();

        // -- Contract case 4: budget exceeded (agent_spec_id = None) --
        let payload4 = trace_payloads::build_tool_call_blocked_payload(
            "blocked",
            "file.read",
            "runtime",
            None::<&str>,
            Some("invalid_arguments"),
            None::<&str>,
            None::<&str>,
            Some(serde_json::json!({
                "max_tool_calls": 6,
                "current_count": 7,
            })),
        );
        let event4 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallBlocked,
            AgentEventActor::Runtime,
            "Budget exceeded",
            payload4,
        );
        store.append_event(&event4).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 4);

        for event in &events {
            assert_eq!(event.event_type, AgentRunEventType::ToolCallBlocked);

            let payload = &event.payload;

            // Mandatory fields per typed governance contract
            contract_helpers::assert_has_string(payload, "status");
            contract_helpers::assert_has_string(payload, "tool_name");
            contract_helpers::assert_has_string(payload, "source");

            // agent_spec_id must exist but accepts string | null
            contract_helpers::assert_has_optional_string_or_null(payload, "agent_spec_id");

            // At least one typed reason must be present
            contract_helpers::assert_has_typed_reason(
                payload,
                &["block_reason", "proposal_reason"],
            );
        }
    }

    /// Trace Contract: Verify that all ReplayFailed events carry at least
    /// one valid typed reason (block_reason or failure_kind).
    #[test]
    fn test_replay_failed_events_have_typed_reason() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-replay-fail-contract";

        // Case 1: Run not found -> block_reason = replay_spec_missing
        let payload1 = trace_payloads::build_replay_failed_payload(
            run_id,
            "action-1",
            "action-1",
            "Run not found",
            Some("replay_spec_missing"),
            None::<&str>,
            None,
        );
        let r1 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ReplayFailed,
            AgentEventActor::Runtime,
            "Run not found",
            payload1,
        );
        store.append_event(&r1).unwrap();

        // Case 2: Store not available -> failure_kind = internal_error
        let payload2 = trace_payloads::build_replay_failed_payload(
            run_id,
            "action-2",
            "action-2",
            "AgentRun store not available",
            None::<&str>,
            Some("internal_error"),
            None,
        );
        let r2 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ReplayFailed,
            AgentEventActor::Runtime,
            "AgentRun store not available",
            payload2,
        );
        store.append_event(&r2).unwrap();

        // Case 3: Action not found -> block_reason = replay_spec_missing
        let payload3 = trace_payloads::build_replay_failed_payload(
            run_id,
            "action-3",
            "action-3",
            "Action not found",
            Some("replay_spec_missing"),
            None::<&str>,
            None,
        );
        let r3 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ReplayFailed,
            AgentEventActor::Runtime,
            "Action not found",
            payload3,
        );
        store.append_event(&r3).unwrap();

        // Case 4: Executor internal error -> failure_kind = internal_error
        let payload4 = trace_payloads::build_replay_failed_payload(
            run_id,
            "action-4",
            "action-4",
            "Replay execution failed: some error",
            None::<&str>,
            Some("internal_error"),
            None,
        );
        let r4 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ReplayFailed,
            AgentEventActor::Runtime,
            "Replay execution failed",
            payload4,
        );
        store.append_event(&r4).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 4);

        for event in &events {
            assert_eq!(event.event_type, AgentRunEventType::ReplayFailed);
            let payload = &event.payload;

            contract_helpers::assert_has_string(payload, "status");
            assert_eq!(
                payload["status"].as_str(),
                Some("failed"),
                "replay.failed status must be 'failed'"
            );
            contract_helpers::assert_has_string(payload, "run_id");
            contract_helpers::assert_has_string(payload, "action_id");
            contract_helpers::assert_has_string(payload, "replay_of_action_id");

            // At least one typed reason must be present
            contract_helpers::assert_has_typed_reason(payload, &["block_reason", "failure_kind"]);
        }
    }

    // ── Trace Contract: agent_spec.selected payload ──────────────────
    //
    //  Uses the production payload builder so that a change to the
    //  builder automatically flows into the contract test.  The builder
    //  is the single source-of-truth shared by:
    //    - src-tauri/src/streaming.rs
    //    - src-tauri/src/commands/execution.rs
    //    - openlife-core/src/agent/agent_loop/orchestrator.rs
    //
    //  If this test fails, production emits changed WITHOUT the builder.
    //
    /// Verify that agent_spec.selected payloads (from the production
    /// builder) carry agent_spec_id, role, privacy_policy in
    /// snake_case — matching the frontend explainability contract.
    #[test]
    fn test_agent_spec_selected_payload_contract() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-agent-spec-selected";

        let payload =
            trace_payloads::build_agent_spec_selected_payload("main.default", "main", "local_only");
        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::AgentSpecSelected,
            AgentEventActor::Runtime,
            "AgentSpec main.default selected",
            payload,
        );
        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AgentRunEventType::AgentSpecSelected);

        let payload = &events[0].payload;
        contract_helpers::assert_has_string(payload, "agent_spec_id");
        contract_helpers::assert_has_string(payload, "role");
        contract_helpers::assert_has_string(payload, "privacy_policy");

        assert_eq!(payload["agent_spec_id"].as_str(), Some("main.default"));
        assert_eq!(payload["role"].as_str(), Some("main"));
        assert_eq!(payload["privacy_policy"].as_str(), Some("local_only"));
    }

    // ── Trace Contract: prompt_stack.assembled payload ──────────────
    //
    //  Uses the production payload builder.  Contract: prompt_blocks
    //  array items carry metadata only.  No prompt_stack_id field
    //  (Scheme B).

    /// Verify that prompt_stack.assembled payloads (from the production
    /// builder) carry agent_spec_id and prompt_blocks metadata.  PromptStack
    /// Scheme B is enforced — no prompt_stack_id and no raw prompt content.
    #[test]
    fn test_prompt_stack_assembled_payload_contract() {
        use crate::agent::prompt_stack::{PromptBlock, PromptPrivacyLevel, PromptStack};
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-prompt-stack";

        let stack = PromptStack::new()
            .with_block(
                PromptBlock::base_identity()
                    .with_token_budget(256)
                    .with_applies_to(vec!["Main".into()]),
            )
            .with_block(
                PromptBlock::planning()
                    .with_privacy(PromptPrivacyLevel::StrictlyLocal)
                    .with_cloud_allowed(false),
            );
        let block_trace = stack.block_trace();
        let payload =
            trace_payloads::build_prompt_stack_assembled_payload("main.default", &block_trace);
        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::PromptStackAssembled,
            AgentEventActor::Runtime,
            "PromptStack assembled with 2 blocks",
            payload,
        );
        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].event_type,
            AgentRunEventType::PromptStackAssembled
        );

        let payload = &events[0].payload;
        contract_helpers::assert_has_string(payload, "agent_spec_id");
        contract_helpers::assert_has_array(payload, "prompt_blocks");
        contract_helpers::assert_array_items_have_field(payload, "prompt_blocks", "id");
        contract_helpers::assert_array_items_have_field(payload, "prompt_blocks", "version");
        contract_helpers::assert_array_items_have_field(payload, "prompt_blocks", "purpose");
        contract_helpers::assert_array_items_have_field(payload, "prompt_blocks", "privacy_level");
        contract_helpers::assert_array_items_have_field(payload, "prompt_blocks", "cloud_allowed");
        contract_helpers::assert_array_items_have_field(payload, "prompt_blocks", "token_budget");
        contract_helpers::assert_array_items_have_field(payload, "prompt_blocks", "applies_to");
        contract_helpers::assert_array_items_have_field(
            payload,
            "prompt_blocks",
            "estimated_tokens",
        );

        let payload_text = payload.to_string();
        assert!(payload_text.contains("base_identity"));
        assert!(payload_text.contains("\"version\":\"1.0.0\""));
        assert!(payload_text.contains("\"privacy_level\":\"strictly_local\""));
        assert!(!payload_text.contains("你是 OpenLife"));
        assert!(!payload_text.contains("raw_prompt"));
        assert!(!payload_text.contains("raw_lifemodel"));
        assert!(!payload_text.contains("raw_memory"));
        assert!(!payload_text.contains("RAW_USER_SENTINEL"));

        // Scheme B: no prompt_stack_id / promptStackId field
        contract_helpers::assert_field_absent(payload, "prompt_stack_id");
        contract_helpers::assert_field_absent(payload, "promptStackId");
    }

    // ── Trace Contract: context_governance.applied payload ──────────
    //
    //  Uses the production payload builder for both StreamingExecution
    //  and Orchestrator paths.  streaming.rs uses StreamingExecution,
    //  orchestrator.rs uses Orchestrator.

    /// Verify that context_governance.applied payloads (from the
    /// production builder) carry the correct fields for both the
    /// streaming/execution path and the orchestrator path.
    #[test]
    fn test_context_governance_applied_payload_contract() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;
        use crate::agent::trace_payloads::ContextGovernanceEmitter;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-context-gov";

        // Case 1: streaming/execution path — uses `privacy_policy`
        let payload1 = trace_payloads::build_context_governance_applied_payload(
            "main.default",
            vec!["lifemodel_summary".into(), "goals".into()],
            vec!["raw_health_data".into()],
            "local_only",
            ContextGovernanceEmitter::StreamingExecution,
        );
        let e1 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ContextGovernanceApplied,
            AgentEventActor::Runtime,
            "Context governance applied (streaming path)",
            payload1,
        );
        store.append_event(&e1).unwrap();

        // Case 2: orchestrator path — uses `agent_spec_privacy_policy`
        let payload2 = trace_payloads::build_context_governance_applied_payload(
            "main.strict",
            vec!["lifemodel_summary".into()],
            vec!["memory".into()],
            "local_only",
            ContextGovernanceEmitter::Orchestrator,
        );
        let e2 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ContextGovernanceApplied,
            AgentEventActor::Runtime,
            "Context governance applied (orchestrator path)",
            payload2,
        );
        store.append_event(&e2).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 2);

        for event in &events {
            assert_eq!(
                event.event_type,
                AgentRunEventType::ContextGovernanceApplied
            );
            let payload = &event.payload;
            contract_helpers::assert_has_string(payload, "agent_spec_id");
            contract_helpers::assert_has_array_allow_empty(payload, "context_included");
            contract_helpers::assert_has_array_allow_empty(payload, "context_excluded");

            let has_privacy = payload
                .get("privacy_policy")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
                || payload
                    .get("agent_spec_privacy_policy")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.is_empty());
            assert!(
                has_privacy,
                "context_governance.applied must have privacy_policy or agent_spec_privacy_policy"
            );
        }
    }

    // ── Trace Contract: generic failure event surface ────────────────
    //
    //  Uses the production payload builders.

    /// Verify that generic failure event payloads produced by the
    /// production builders survive event store round-trip.
    #[test]
    fn test_generic_failure_events_round_trip() {
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-generic-failures";

        let e1 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ModelFailed,
            AgentEventActor::Runtime,
            "Governance failure",
            trace_payloads::build_model_failed_payload("main.default", "some governance error"),
        );
        store.append_event(&e1).unwrap();

        let e2 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ModelCallFailed,
            AgentEventActor::Agent,
            "Calling deepseek-chat failed",
            trace_payloads::build_model_call_failed_payload(
                "openrouter",
                "deepseek-chat",
                "timeout",
            ),
        );
        store.append_event(&e2).unwrap();

        let e3 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallFailed,
            AgentEventActor::Tool("web.search".into()),
            "Tool 'web.search' failed",
            trace_payloads::build_tool_call_failed_payload("web.search", "network error"),
        );
        store.append_event(&e3).unwrap();

        let e4 = AgentRunEvent::new(
            run_id,
            AgentRunEventType::RunFailed,
            AgentEventActor::Runtime,
            "Run failed due to error",
            trace_payloads::build_run_failed_payload("budget exceeded"),
        );
        store.append_event(&e4).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 4);

        assert_eq!(events[0].event_type, AgentRunEventType::ModelFailed);
        assert_eq!(events[1].event_type, AgentRunEventType::ModelCallFailed);
        assert_eq!(events[2].event_type, AgentRunEventType::ToolCallFailed);
        assert_eq!(events[3].event_type, AgentRunEventType::RunFailed);

        for event in &events {
            let payload_str = serde_json::to_string(&event.payload).unwrap();
            assert!(
                !payload_str.is_empty() && payload_str != "{}",
                "failure event {} must have non-empty payload",
                event.event_type
            );
        }
    }

    // ── Trace Contract: typed governance malformed + enum prevention ─

    /// Verify that tool.call_blocked payloads with invalid enum
    /// variants (like "not_a_real_enum_variant") are rejected by the
    /// typed-reason validation — same semantics as frontend
    /// typedContract.ts malformedKnownTyped warning.
    #[test]
    fn test_tool_call_blocked_rejects_invalid_enum_reason() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-blocked-invalid-enum";

        // Build with a valid status/tool_name/source but invalid block_reason
        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallBlocked,
            AgentEventActor::Tool("web.search".into()),
            "Blocked with invalid reason",
            trace_payloads::build_tool_call_blocked_payload(
                "blocked",
                "web.search",
                "builtin",
                Some("main.default"),
                Some("not_a_real_enum_variant"),
                None::<&str>,
                None::<&str>,
                None,
            ),
        );
        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        let payload = &events[0].payload;

        // Structural fields present
        assert_eq!(payload["status"].as_str(), Some("blocked"));
        assert_eq!(payload["tool_name"].as_str(), Some("web.search"));
        assert_eq!(payload["source"].as_str(), Some("builtin"));

        // Typed reason must NOT be valid — the string exists but is not a known enum
        contract_helpers::assert_no_typed_reason(payload, &["block_reason", "proposal_reason"]);
    }

    /// Verify that replay.failed payloads with invalid enum variants
    /// are rejected.
    #[test]
    fn test_replay_failed_rejects_invalid_enum_reason() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-replay-invalid-enum";

        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ReplayFailed,
            AgentEventActor::Runtime,
            "Replay failed — invalid reason",
            trace_payloads::build_replay_failed_payload(
                run_id,
                "a1",
                "orig-1",
                "Replay failed — invalid reason",
                Some("not_a_real_enum_variant"),
                None::<&str>,
                None,
            ),
        );
        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AgentRunEventType::ReplayFailed);

        let payload = &events[0].payload;
        assert_eq!(payload["status"].as_str(), Some("failed"));

        // Must NOT be valid — unknown variant
        contract_helpers::assert_no_typed_reason(payload, &["block_reason", "failure_kind"]);
    }

    /// Verify that tool.call_blocked with valid block_reason passes
    /// the typed-reason helper (regression test — valid enum must work).
    #[test]
    fn test_tool_call_blocked_with_valid_block_reason_passes() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-blocked-valid";

        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallBlocked,
            AgentEventActor::Tool("web.search".into()),
            "Blocked by AgentSpec",
            trace_payloads::build_tool_call_blocked_payload(
                "blocked",
                "web.search",
                "builtin",
                Some("main.default"),
                Some("agent_spec_denied"),
                None::<&str>,
                None::<&str>,
                None,
            ),
        );
        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        let payload = &events[0].payload;

        contract_helpers::assert_has_typed_reason(payload, &["block_reason", "proposal_reason"]);
    }

    /// Verify that replay.failed with valid block_reason passes the
    /// typed-reason helper.
    #[test]
    fn test_replay_failed_with_valid_reason_passes() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-replay-valid";

        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ReplayFailed,
            AgentEventActor::Runtime,
            "Run not found",
            trace_payloads::build_replay_failed_payload(
                run_id,
                "a1",
                "orig-1",
                "Run not found",
                Some("replay_spec_missing"),
                None::<&str>,
                None,
            ),
        );
        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        let payload = &events[0].payload;

        contract_helpers::assert_has_typed_reason(payload, &["block_reason", "failure_kind"]);
    }

    /// Verify that a proposal_reason using an invalid enum variant is
    /// rejected.
    #[test]
    fn test_tool_call_blocked_rejects_null_reasons() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-blocked-malformed";

        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallBlocked,
            AgentEventActor::Tool("web.search".into()),
            "Blocked for unknown reason",
            trace_payloads::build_tool_call_blocked_payload(
                "blocked",
                "web.search",
                "builtin",
                Some("main.default"),
                None::<&str>,
                None::<&str>,
                None::<&str>,
                None,
            ),
        );
        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        let payload = &events[0].payload;

        assert_eq!(payload["status"].as_str(), Some("blocked"));
        assert_eq!(payload["tool_name"].as_str(), Some("web.search"));
        assert_eq!(payload["source"].as_str(), Some("builtin"));

        contract_helpers::assert_no_typed_reason(payload, &["block_reason", "proposal_reason"]);
    }

    /// Verify that replay.failed events with null reasons fail the
    /// typed-reason validation.
    #[test]
    fn test_replay_failed_rejects_null_reasons() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-replay-null";

        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ReplayFailed,
            AgentEventActor::Runtime,
            "Replay failed — no typed reason",
            trace_payloads::build_replay_failed_payload(
                run_id,
                "a1",
                "orig-1",
                "Replay failed — no typed reason",
                None::<&str>,
                None::<&str>,
                None,
            ),
        );
        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AgentRunEventType::ReplayFailed);

        let payload = &events[0].payload;
        assert_eq!(payload["status"].as_str(), Some("failed"));

        contract_helpers::assert_no_typed_reason(payload, &["block_reason", "failure_kind"]);
    }

    /// Verify tool.call_blocked with agent_spec_id=None passes the
    /// typed-reason helper — agents like runtime budget exceeded have
    /// no AgentSpec in scope.
    #[test]
    fn test_tool_call_blocked_none_agent_spec_id_passes() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-blocked-no-spec";

        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallBlocked,
            AgentEventActor::Runtime,
            "Budget exceeded",
            trace_payloads::build_tool_call_blocked_payload(
                "blocked",
                "file.read",
                "runtime",
                None::<&str>,
                Some("invalid_arguments"),
                None::<&str>,
                None::<&str>,
                None,
            ),
        );
        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        let payload = &events[0].payload;

        assert_eq!(payload["agent_spec_id"], serde_json::Value::Null);
        contract_helpers::assert_has_typed_reason(payload, &["block_reason", "proposal_reason"]);
    }

    /// Verify tool.call_blocked with agent_spec_id=Some passes the
    /// typed-reason helper — real AgentSpec deny path.
    #[test]
    fn test_tool_call_blocked_some_agent_spec_id_passes() {
        use crate::agent::tests::contract_helpers;
        use crate::agent::trace_payloads;

        let store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "tc-blocked-with-spec";

        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ToolCallBlocked,
            AgentEventActor::Tool("web.search".into()),
            "Blocked by AgentSpec",
            trace_payloads::build_tool_call_blocked_payload(
                "blocked",
                "web.search",
                "builtin",
                Some("custom.spec"),
                Some("agent_spec_denied"),
                None::<&str>,
                None::<&str>,
                None,
            ),
        );
        store.append_event(&event).unwrap();

        let events = store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        let payload = &events[0].payload;

        assert_eq!(payload["agent_spec_id"].as_str(), Some("custom.spec"));
        contract_helpers::assert_has_typed_reason(payload, &["block_reason", "proposal_reason"]);
    }
}
