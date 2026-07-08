use crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn;
use crate::AppState;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use openlife_core::agent::main_chat_runtime_contract::{
    MainChatAgentProductProposalStatus, MainChatAgentStateSnapshot,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentDurableEvent {
    pub event_id: String,
    pub task_session_id: String,
    pub run_id: String,
    pub sequence: u64,
    pub event_type: String,
    pub object_type: String,
    pub object_id: String,
    pub created_at: DateTime<Utc>,
    pub source: String,
    pub payload_digest: String,
    pub payload: Value,
    pub backfilled: bool,
}

#[derive(Debug, Clone)]
struct MainChatAgentEventDraft {
    task_session_id: String,
    run_id: String,
    event_type: String,
    object_type: String,
    object_id: String,
    created_at: DateTime<Utc>,
    source: String,
    payload: Value,
    backfilled: bool,
}

pub struct MainChatAgentEventStore {
    conn: Mutex<Connection>,
}

impl MainChatAgentEventStore {
    pub(crate) fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = Self {
            conn: Mutex::new(Connection::open(&db_path).with_context(|| {
                format!("failed to open main chat agent event db at {:?}", db_path)
            })?),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub(crate) fn new_in_memory() -> Result<Self> {
        let store = Self {
            conn: Mutex::new(
                Connection::open_in_memory()
                    .context("failed to open in-memory main chat agent event db")?,
            ),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS main_chat_agent_event_sequences (
                task_session_id TEXT PRIMARY KEY,
                last_sequence INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS main_chat_agent_events (
                event_id TEXT PRIMARY KEY,
                task_session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                object_type TEXT NOT NULL,
                object_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                source TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                backfilled INTEGER NOT NULL DEFAULT 0,
                UNIQUE(task_session_id, sequence),
                UNIQUE(task_session_id, event_type, object_id, payload_digest, backfilled)
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_main_chat_agent_events_replay
             ON main_chat_agent_events(task_session_id, sequence)",
            [],
        )?;
        Ok(())
    }

    fn append(&self, draft: MainChatAgentEventDraft) -> Result<MainChatAgentDurableEvent> {
        let payload_json = serde_json::to_string(&draft.payload)?;
        let payload_digest = metadata_safe_digest(&payload_json);
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;

        if let Some(existing) = select_event_by_identity(
            &tx,
            &draft.task_session_id,
            &draft.event_type,
            &draft.object_id,
            &payload_digest,
            draft.backfilled,
        )? {
            tx.commit()?;
            return Ok(existing);
        }
        if draft.backfilled {
            if let Some(existing_live) = select_event_by_identity(
                &tx,
                &draft.task_session_id,
                &draft.event_type,
                &draft.object_id,
                &payload_digest,
                false,
            )? {
                tx.commit()?;
                return Ok(existing_live);
            }
        }

        let last_sequence = tx
            .query_row(
                "SELECT last_sequence FROM main_chat_agent_event_sequences WHERE task_session_id = ?1",
                [&draft.task_session_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        let sequence = (last_sequence + 1) as u64;
        let event_id = stable_event_id(
            &draft.task_session_id,
            sequence,
            &draft.event_type,
            &draft.object_id,
            &payload_digest,
        );
        tx.execute(
            "INSERT INTO main_chat_agent_events (
                event_id, task_session_id, run_id, sequence, event_type, object_type,
                object_id, created_at, source, payload_digest, payload_json, backfilled
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                event_id,
                draft.task_session_id,
                draft.run_id,
                sequence as i64,
                draft.event_type,
                draft.object_type,
                draft.object_id,
                draft.created_at.to_rfc3339(),
                draft.source,
                payload_digest,
                payload_json,
                if draft.backfilled { 1 } else { 0 },
            ],
        )?;
        tx.execute(
            "INSERT INTO main_chat_agent_event_sequences(task_session_id, last_sequence)
             VALUES (?1, ?2)
             ON CONFLICT(task_session_id) DO UPDATE SET last_sequence = excluded.last_sequence",
            params![draft.task_session_id, sequence as i64],
        )?;
        let event = select_event_by_id(&tx, &event_id)?.context("inserted event missing")?;
        tx.commit()?;
        Ok(event)
    }

    pub(crate) fn list(
        &self,
        task_session_id: &str,
        after_sequence: u64,
        limit: u64,
    ) -> Result<Vec<MainChatAgentDurableEvent>> {
        let bounded_limit = limit.clamp(1, 250);
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled
             FROM main_chat_agent_events
             WHERE task_session_id = ?1 AND sequence > ?2
             ORDER BY sequence ASC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![task_session_id, after_sequence as i64, bounded_limit as i64],
            row_to_event,
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub(crate) fn latest_run_id(&self, task_session_id: &str) -> Result<Option<String>> {
        let conn = self.lock_conn()?;
        let run_id = conn
            .query_row(
                "SELECT run_id FROM main_chat_agent_events
                 WHERE task_session_id = ?1
                 ORDER BY sequence DESC
                 LIMIT 1",
                [task_session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(run_id)
    }

    pub(crate) fn latest_sequence(&self, task_session_id: &str) -> Result<u64> {
        let conn = self.lock_conn()?;
        let sequence = conn
            .query_row(
                "SELECT sequence FROM main_chat_agent_events
                 WHERE task_session_id = ?1
                 ORDER BY sequence DESC
                 LIMIT 1",
                [task_session_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        Ok(sequence.max(0) as u64)
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|err| anyhow::anyhow!("mutex poison: {}", err))
    }
}

fn select_event_by_identity(
    conn: &Connection,
    task_session_id: &str,
    event_type: &str,
    object_id: &str,
    payload_digest: &str,
    backfilled: bool,
) -> Result<Option<MainChatAgentDurableEvent>> {
    let event = conn
        .query_row(
            "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled
             FROM main_chat_agent_events
             WHERE task_session_id = ?1 AND event_type = ?2 AND object_id = ?3
                   AND payload_digest = ?4 AND backfilled = ?5
             ORDER BY sequence ASC
             LIMIT 1",
            params![
                task_session_id,
                event_type,
                object_id,
                payload_digest,
                if backfilled { 1 } else { 0 },
            ],
            row_to_event,
        )
        .optional()?;
    Ok(event)
}

fn select_event_by_id(
    conn: &Connection,
    event_id: &str,
) -> Result<Option<MainChatAgentDurableEvent>> {
    let event = conn
        .query_row(
            "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled
             FROM main_chat_agent_events
             WHERE event_id = ?1",
            [event_id],
            row_to_event,
        )
        .optional()?;
    Ok(event)
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<MainChatAgentDurableEvent> {
    let sequence = row.get::<_, i64>(3)?;
    let created_at_raw: String = row.get(7)?;
    let payload_json: String = row.get(10)?;
    let backfilled = row.get::<_, i64>(11)? != 0;
    Ok(MainChatAgentDurableEvent {
        event_id: row.get(0)?,
        task_session_id: row.get(1)?,
        run_id: row.get(2)?,
        sequence: sequence.max(0) as u64,
        event_type: row.get(4)?,
        object_type: row.get(5)?,
        object_id: row.get(6)?,
        created_at: DateTime::parse_from_rfc3339(&created_at_raw)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        source: row.get(8)?,
        payload_digest: row.get(9)?,
        payload: serde_json::from_str(&payload_json).unwrap_or(Value::Null),
        backfilled,
    })
}

pub(crate) fn materialize_main_chat_agent_events_for_snapshot_in_store(
    store: &MainChatAgentEventStore,
    snapshot: &MainChatAgentStateSnapshot,
) -> Result<Vec<MainChatAgentDurableEvent>> {
    materialize_main_chat_agent_events_for_snapshot_in_store_with_backfill(store, snapshot, false)
}

pub(crate) fn materialize_main_chat_agent_backfill_events_for_snapshot_in_store(
    store: &MainChatAgentEventStore,
    snapshot: &MainChatAgentStateSnapshot,
) -> Result<Vec<MainChatAgentDurableEvent>> {
    materialize_main_chat_agent_events_for_snapshot_in_store_with_backfill(store, snapshot, true)
}

fn materialize_main_chat_agent_events_for_snapshot_in_store_with_backfill(
    store: &MainChatAgentEventStore,
    snapshot: &MainChatAgentStateSnapshot,
    backfilled: bool,
) -> Result<Vec<MainChatAgentDurableEvent>> {
    let drafts = event_drafts_from_snapshot(snapshot, backfilled);
    drafts
        .into_iter()
        .map(|draft| store.append(draft))
        .collect::<Result<Vec<_>>>()
}

pub(crate) async fn materialize_main_chat_agent_events_for_snapshot(
    state: &Arc<AppState>,
    snapshot: &MainChatAgentStateSnapshot,
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err("main_chat_agent_event_store_unavailable".into());
    };
    let store = store_arc.lock().await;
    materialize_main_chat_agent_events_for_snapshot_in_store(&store, snapshot)
        .map_err(|err| err.to_string())
}

pub(crate) async fn materialize_optional_main_chat_agent_events(
    state: &Arc<AppState>,
    snapshot: Option<&MainChatAgentStateSnapshot>,
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    if let Some(snapshot) = snapshot {
        materialize_main_chat_agent_events_for_snapshot(state, snapshot).await
    } else {
        Ok(Vec::new())
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn append_main_chat_agent_runtime_event(
    state: &Arc<AppState>,
    task_session_id: impl Into<String>,
    run_id: impl Into<String>,
    event_type: impl Into<String>,
    object_type: impl Into<String>,
    object_id: impl Into<String>,
    source: impl Into<String>,
    payload: Value,
) -> Result<MainChatAgentDurableEvent, String> {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err("main_chat_agent_event_store_unavailable".into());
    };
    let store = store_arc.lock().await;
    store
        .append(MainChatAgentEventDraft {
            task_session_id: task_session_id.into(),
            run_id: run_id.into(),
            event_type: event_type.into(),
            object_type: object_type.into(),
            object_id: object_id.into(),
            created_at: Utc::now(),
            source: source.into(),
            payload,
            backfilled: false,
        })
        .map_err(|err| err.to_string())
}

pub(crate) async fn list_main_chat_agent_events_with_state(
    state: &Arc<AppState>,
    task_session_id: String,
    after_sequence: Option<u64>,
    limit: Option<u64>,
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err("main_chat_agent_event_store_unavailable".into());
    };
    let store = store_arc.lock().await;
    store
        .list(
            &task_session_id,
            after_sequence.unwrap_or(0),
            limit.unwrap_or(100),
        )
        .map_err(|err| err.to_string())
}

pub(crate) async fn get_main_chat_agent_state_snapshot_with_state(
    state: &Arc<AppState>,
    task_session_id: String,
) -> Result<MainChatAgentStateSnapshot, String> {
    let run_id = if let Some(store_arc) = state.main_chat_agent_event_store.as_ref() {
        store_arc
            .lock()
            .await
            .latest_run_id(&task_session_id)
            .map_err(|err| err.to_string())?
    } else {
        None
    };
    let mut snapshot =
        assemble_main_chat_agent_state_for_turn(state, Some(&task_session_id), run_id.as_deref())
            .await
            .ok_or_else(|| "main_chat_agent_snapshot_unavailable".to_string())?;
    if let Some(store_arc) = state.main_chat_agent_event_store.as_ref() {
        let store = store_arc.lock().await;
        materialize_main_chat_agent_backfill_events_for_snapshot_in_store(&store, &snapshot)
            .map_err(|err| err.to_string())?;
        snapshot.sequence = store
            .latest_sequence(&task_session_id)
            .map_err(|err| err.to_string())?;
    }
    Ok(snapshot)
}

#[tauri::command]
pub(crate) async fn list_main_chat_agent_events(
    task_session_id: String,
    after_sequence: Option<u64>,
    limit: Option<u64>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    list_main_chat_agent_events_with_state(state.inner(), task_session_id, after_sequence, limit)
        .await
}

#[tauri::command]
pub(crate) async fn get_main_chat_agent_state_snapshot(
    task_session_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<MainChatAgentStateSnapshot, String> {
    get_main_chat_agent_state_snapshot_with_state(state.inner(), task_session_id).await
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatEventGapRecoveryDecision {
    pub(crate) status: String,
    pub(crate) replay_after_sequence: u64,
    pub(crate) expected_sequence: u64,
    pub(crate) observed_sequence: u64,
    pub(crate) snapshot_required: bool,
}

pub(crate) fn evaluate_main_chat_event_gap_recovery(
    replayed_events: &[MainChatAgentDurableEvent],
    last_applied_sequence: u64,
    observed_sequence: u64,
) -> MainChatEventGapRecoveryDecision {
    let expected_sequence = last_applied_sequence + 1;
    let relevant_events = replayed_events
        .iter()
        .filter(|event| event.sequence > last_applied_sequence)
        .collect::<Vec<_>>();
    let replay_covers_gap = !relevant_events.is_empty()
        && relevant_events
            .first()
            .is_some_and(|event| event.sequence == expected_sequence)
        && relevant_events
            .windows(2)
            .all(|pair| pair[1].sequence == pair[0].sequence + 1)
        && relevant_events
            .last()
            .is_some_and(|event| event.sequence >= observed_sequence);
    MainChatEventGapRecoveryDecision {
        status: if replay_covers_gap {
            "replaying_events".into()
        } else {
            "snapshot_refresh_required".into()
        },
        replay_after_sequence: last_applied_sequence,
        expected_sequence,
        observed_sequence,
        snapshot_required: !replay_covers_gap,
    }
}

fn event_drafts_from_snapshot(
    snapshot: &MainChatAgentStateSnapshot,
    backfilled: bool,
) -> Vec<MainChatAgentEventDraft> {
    let mut drafts = Vec::new();
    let task_id = snapshot.task.task_id.clone();
    let run_id = snapshot.task.run_id.clone();
    drafts.push(draft(
        snapshot,
        "task.created",
        "task",
        &task_id,
        "agent_ingress",
        json!({
            "taskSessionId": task_id,
            "runId": run_id,
            "strategy": snapshot.task.strategy.as_str(),
        }),
    ));
    drafts.push(draft(
        snapshot,
        "route.selected",
        "route",
        snapshot.route.strategy.as_str(),
        "strategy_router",
        json!({
            "taskSessionId": snapshot.task.task_id,
            "runId": snapshot.task.run_id,
            "strategy": snapshot.route.strategy.as_str(),
            "reason": snapshot.route.reason,
        }),
    ));
    for context in &snapshot.context {
        drafts.push(draft(
            snapshot,
            "context.selected",
            "context",
            &context.context_id,
            "context_compiler",
            json!({
                "contextId": context.context_id,
                "sourceKind": context.source_kind,
                "sourceLabel": context.source_label,
                "evidenceId": context.evidence_id,
            }),
        ));
    }
    if let Some(provider) = &snapshot.provider {
        drafts.push(draft(
            snapshot,
            "provider.selected",
            "provider",
            &provider.evidence_id,
            "strategy_router",
            json!({
                "provider": provider.provider,
                "model": provider.model,
                "routeType": provider.route_type,
                "evidenceId": provider.evidence_id,
            }),
        ));
    }
    if let Some(plan) = &snapshot.plan {
        drafts.push(draft(
            snapshot,
            "plan.updated",
            "plan",
            &plan.plan_id,
            "plan_runtime",
            json!({
                "planId": plan.plan_id,
                "status": plan.status,
                "evidenceId": plan.evidence_id,
            }),
        ));
    }
    for action in &snapshot.actions {
        drafts.push(draft(
            snapshot,
            "action.queued",
            "action",
            &action.action_id,
            "action_queue",
            json!({
                "actionId": action.action_id,
                "actionType": action.action_type,
                "target": action.target,
                "status": action.status,
                "policyDecisionId": action.policy_decision_id,
            }),
        ));
        if action.started_at.is_some() || action.status != "queued" {
            drafts.push(draft(
                snapshot,
                "action.started",
                "action",
                &action.action_id,
                "action_executor",
                json!({
                    "actionId": action.action_id,
                    "status": action.status,
                    "startedAt": action.started_at,
                }),
            ));
        }
        match action.status.as_str() {
            "succeeded" => drafts.push(draft(
                snapshot,
                "action.completed",
                "action",
                &action.action_id,
                "action_executor",
                json!({
                    "actionId": action.action_id,
                    "status": action.status,
                    "observationIds": action.observation_ids,
                }),
            )),
            "failed" => drafts.push(draft(
                snapshot,
                "action.failed",
                "action",
                &action.action_id,
                "action_executor",
                json!({
                    "actionId": action.action_id,
                    "status": action.status,
                    "retryable": action.retryable,
                }),
            )),
            _ => drafts.push(draft(
                snapshot,
                "action.updated",
                "action",
                &action.action_id,
                "action_queue",
                json!({
                    "actionId": action.action_id,
                    "status": action.status,
                }),
            )),
        }
    }
    for observation in &snapshot.observations {
        drafts.push(draft(
            snapshot,
            "observation.created",
            "observation",
            &observation.observation_id,
            "action_executor",
            json!({
                "observationId": observation.observation_id,
                "actionId": observation.action_id,
                "sourceKind": observation.source_kind,
                "sourceLabel": observation.source_label,
                "readExecution": observation.read_execution,
            }),
        ));
    }
    for blocker in &snapshot.blockers {
        drafts.push(draft(
            snapshot,
            "blocker.created",
            "blocker",
            &blocker.blocker_id,
            "agent_loop",
            json!({
                "blockerId": blocker.blocker_id,
                "reasonCode": blocker.reason_code,
                "affectedActionId": blocker.affected_action_id,
                "recoverable": blocker.recoverable,
            }),
        ));
    }
    for proposal in &snapshot.proposals {
        drafts.push(draft(
            snapshot,
            "proposal.created",
            "proposal",
            &proposal.proposal_id,
            "proposal_store",
            json!({
                "proposalId": proposal.proposal_id,
                "proposalType": proposal.proposal_type,
                "status": proposal.status.as_str(),
                "evidenceIds": proposal.evidence_ids,
                "actionIds": proposal.action_ids,
            }),
        ));
        let status_event = match proposal.status {
            MainChatAgentProductProposalStatus::Accepted => Some("proposal.accepted"),
            MainChatAgentProductProposalStatus::Rejected => Some("proposal.rejected"),
            MainChatAgentProductProposalStatus::Deferred => Some("proposal.deferred"),
            _ => None,
        };
        if let Some(event_type) = status_event {
            drafts.push(draft(
                snapshot,
                event_type,
                "proposal",
                &proposal.proposal_id,
                "proposal_store",
                json!({
                    "proposalId": proposal.proposal_id,
                    "status": proposal.status.as_str(),
                }),
            ));
        }
        if let Some(record) = &proposal.memory_lifecycle {
            let status = format!("{:?}", record.status).to_ascii_lowercase();
            if status.contains("materialized") {
                drafts.push(draft(
                    snapshot,
                    "memory.materialized",
                    "memory",
                    &record.memory_id,
                    "proposal_store",
                    json!({
                        "memoryId": record.memory_id,
                        "proposalId": record.proposal_id,
                        "materializedViewVersion": record.materialized_view_version,
                    }),
                ));
            }
            if status.contains("rolledback") || status.contains("rolled_back") {
                drafts.push(draft(
                    snapshot,
                    "memory.rolled_back",
                    "memory",
                    &record.memory_id,
                    "proposal_store",
                    json!({
                        "memoryId": record.memory_id,
                        "proposalId": record.proposal_id,
                        "rolledBackByEventId": record.rolled_back_by_event_id,
                    }),
                ));
            }
        }
    }
    if let Some(final_delivery) = &snapshot.final_delivery {
        drafts.push(draft(
            snapshot,
            "final_delivery.created",
            "final_delivery",
            &final_delivery.delivery_id,
            "finalizer",
            json!({
                "deliveryId": final_delivery.delivery_id,
                "taskSessionId": final_delivery.task_id,
                "runId": final_delivery.run_id,
                "status": final_delivery.status.as_str(),
                "completedActionCount": final_delivery.completed_actions.len(),
                "observationCount": final_delivery.observations_used.len(),
                "proposalCount": final_delivery.proposals_created.len(),
                "blockerCount": final_delivery.blockers.len(),
                "pendingUserActionCount": final_delivery.pending_user_actions.len(),
                "directWritesExecuted": false,
            }),
        ));
    }
    for diagnostic in &snapshot.diagnostics {
        drafts.push(draft(
            snapshot,
            "diagnostic.created",
            "diagnostic",
            &diagnostic.gap_id,
            "diagnostic",
            json!({
                "gapId": diagnostic.gap_id,
                "gapCode": diagnostic.gap_code,
                "evidenceId": diagnostic.evidence_id,
            }),
        ));
    }
    drafts.push(draft(
        snapshot,
        "task.updated",
        "task",
        &snapshot.task.task_id,
        "task_control",
        json!({
            "taskSessionId": snapshot.task.task_id,
            "status": snapshot.task.status.as_str(),
            "controls": snapshot.task.controls,
            "actionIds": snapshot.task.action_ids,
            "observationIds": snapshot.task.observation_ids,
            "blockerIds": snapshot.task.blocker_ids,
            "proposalIds": snapshot.task.proposal_ids,
            "finalDeliveryId": snapshot.task.final_delivery_id,
        }),
    ));
    if backfilled {
        for draft in &mut drafts {
            draft.backfilled = true;
            draft.source = "diagnostic".into();
        }
    }
    drafts
}

fn draft(
    snapshot: &MainChatAgentStateSnapshot,
    event_type: &str,
    object_type: &str,
    object_id: &str,
    source: &str,
    payload: Value,
) -> MainChatAgentEventDraft {
    MainChatAgentEventDraft {
        task_session_id: snapshot.task.task_id.clone(),
        run_id: snapshot.task.run_id.clone(),
        event_type: event_type.into(),
        object_type: object_type.into(),
        object_id: bounded_label(object_id, 180),
        created_at: snapshot.emitted_at,
        source: source.into(),
        payload,
        backfilled: false,
    }
}

fn metadata_safe_digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("bytes:{} hash:sha256:{:x}", value.len(), hasher.finalize())
}

fn stable_event_id(
    task_session_id: &str,
    sequence: u64,
    event_type: &str,
    object_id: &str,
    payload_digest: &str,
) -> String {
    let hash = payload_digest
        .rsplit_once("hash:sha256:")
        .map(|(_, hash)| hash)
        .unwrap_or(payload_digest);
    format!(
        "mainchat_event:{}:{}:{}:{}:{}",
        event_id_part(task_session_id),
        sequence,
        event_id_part(event_type),
        event_id_part(object_id),
        event_id_part(hash)
    )
}

fn event_id_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .take(160)
        .collect()
}

fn bounded_label(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for ch in value.chars().take(max_chars) {
        if ch.is_control() {
            output.push(' ');
        } else {
            output.push(ch);
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}
