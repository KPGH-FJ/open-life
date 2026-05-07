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

        let event_types = vec![
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
}
