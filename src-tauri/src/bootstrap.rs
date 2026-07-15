//! Application bootstrap: store initialization and AppState assembly.
//! Extracted from lib.rs to keep the main entry point focused on Tauri lifecycle.

use crate::a2a_sidecar;
use crate::main_chat_event_stream::{MainChatAgentEventStore, MainChatEventDigestKey};
use crate::persistence_coordinator::PersistenceCoordinator;
use crate::secret_store::{
    hydrate_config_secrets, hydrate_or_create_canonical_store_integrity_key,
    hydrate_or_create_integrity_key, hydrate_or_create_mcp_audit_keys, SecretStore,
    StartupKeyringSecretStore, ACTION_QUEUE_AUTHORITY_KEY_REF, AGENT_RUN_RECEIPT_KEY_REF,
    MAIN_CHAT_EVENT_INTEGRITY_KEY_REF, TASK_STORE_AUTHORITY_KEY_REF,
};
use crate::state::AppState;
use crate::storage::{
    load_mcp_audit_keyring_from_path, privacy_policy_path, save_mcp_audit_keyring_to_path,
};
use openlife_core::agent::{
    main_chat_agent_v1::{ActionQueueAuthorityKey, ActionQueueStore, AgentTaskSessionStore},
    reconcile_collaboration_guidance_authority, AgentProposal, AgentRunReceiptKey,
    CollaborationGuidanceCutoverStatus, DurableWriteRequest, DurableWriteSource,
    DurableWriteSubject, HSAssetAuthorityRegistry, MemoryLifecycleStore, PlanExecuteSessionStore,
    ProposalSource, ProposalStore, ProposalType, ReviewWorkflow, RiskLevel,
};
use openlife_core::builder::BuilderSessionStore;
use openlife_core::config::AppConfig;
use openlife_core::feedback::FeedbackStore;
use openlife_core::life_model::LifeModelManager;
use openlife_core::mcp::McpRegistry;
use openlife_core::mcp_audit::McpAuditStore;
use openlife_core::memory::MemoryStore;
use openlife_core::memory_cache::{HotMemoryCache, SharedHotCache};
use openlife_core::privacy::PrivacyEngine;
use openlife_core::scheduler::InferenceScheduler;
use openlife_core::vectors::VectorStore;
use openlife_core::versioning::VersionManager;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Result of the bootstrap process: assembled application state and startup warnings.
pub struct BootstrapResult {
    pub state: Arc<AppState>,
}

const STARTUP_PROPOSAL_RECONCILIATION_BATCH: i64 = 200;
const STARTUP_PROPOSAL_RECONCILIATION_SYNC_PASSES: usize = 5;
const STARTUP_CANONICAL_OUTBOX_BATCH: usize = 500;
const STARTUP_CANONICAL_OUTBOX_PASSES: usize = 20;
const STARTUP_MAIN_CHAT_TASK_SCAN_BATCH: usize = 200;
const STARTUP_MAIN_CHAT_TASK_SCAN_PASSES: usize = 50;
const STARTUP_PREPARED_TOOL_RECONCILIATION_BATCH: usize = 200;
const STARTUP_PREPARED_TOOL_RECONCILIATION_PASSES: usize = 50;
const STARTUP_TOOL_QUEUE_RECONCILIATION_OUTBOX_BATCH: usize = 200;
const STARTUP_TOOL_QUEUE_RECONCILIATION_OUTBOX_PASSES: usize = 50;

pub(crate) fn apply_tool_queue_reconciliation_projection(
    queue: &ActionQueueStore,
    projection: &crate::main_chat_event_stream::MainChatToolQueueReconciliationProjection,
) -> Result<(), String> {
    let disposition = match projection.disposition {
        crate::main_chat_event_stream::MainChatToolQueueReconciliationDisposition::EffectNotAttempted => openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolReconciliationDisposition::EffectNotAttempted,
        crate::main_chat_event_stream::MainChatToolQueueReconciliationDisposition::DispatchedUnknown => openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolReconciliationDisposition::DispatchedUnknown,
    };
    queue
        .apply_prepared_tool_reconciliation_after_restart(
            openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolReconciliationInput {
                outbox_id: &projection.outbox_id,
                prepared_event_id: &projection.prepared_event_id,
                prepared_payload_digest: &projection.prepared_payload_digest,
                resolution_event_id: &projection.resolution_event_id,
                resolution_payload_digest: &projection.resolution_payload_digest,
                resolution: projection.resolution,
                task_session_id: &projection.task_session_id,
                run_id: &projection.run_id,
                receipt_id: &projection.receipt_id,
                action_id: &projection.replay_action_id,
                replay_claim_id: &projection.replay_claim_id,
                replay_claim_owner_generation: projection.replay_claim_owner_generation,
                manifest_id: &projection.manifest_id,
                tool_name: &projection.tool_name,
                manifest_contract_digest: &projection.manifest_contract_digest,
                input_hash: &projection.input_hash,
                input_length_bytes: projection.input_length_bytes,
                request_digest: &projection.request_digest,
                action_effect: projection.action_effect,
                idempotency_contract: projection.idempotency_contract,
                process_risk: projection.process_risk,
                effect_may_survive_local_process: projection.effect_may_survive_local_process,
                replay_authority_binding: &projection.replay_authority_binding,
                disposition,
                event_store_attestation: &projection.event_store_attestation,
            },
        )
        .map(|_| ())
        .map_err(|error| format!("apply startup tool queue reconciliation failed: {error}"))
}

/// A process-local TurnRuntime cannot survive a process restart. Any canonical
/// Main Chat AgentRun still marked Running is therefore an orphan unless the
/// task already carries the explicit non-terminal WaitingPermission state.
/// Reconciliation runs before product effects are enabled: terminal receipts
/// remain canonical, projections follow them, and any inability to persist the
/// receipt irreversibly degrades the process-wide effects gate.
pub(crate) async fn reconcile_startup_orphaned_main_chat_runs(
    state: &Arc<AppState>,
) -> Result<usize, String> {
    let result = reconcile_startup_orphaned_main_chat_runs_inner(state).await;
    if result.is_err() {
        state
            .persistence_coordinator
            .degrade_globally("startup_orphan_main_chat_terminalization_failed");
    }
    result
}

async fn reconcile_startup_orphaned_main_chat_runs_inner(
    state: &Arc<AppState>,
) -> Result<usize, String> {
    if !state
        .persistence_coordinator
        .startup_reconciliation_mutations_safe()
    {
        return Err("startup_orphan_reconciliation_mutations_unavailable".into());
    }
    let mut prepared_tool_reconciled = 0usize;
    let mut prepared_tool_remote_unknown = 0usize;
    let mut prepared_tool_effect_unknown = 0usize;
    let mut prepared_tool_local_aborted = 0usize;
    let mut prepared_tool_has_more = false;
    for _ in 0..STARTUP_PREPARED_TOOL_RECONCILIATION_PASSES {
        let report = {
            let store_arc = state
                .main_chat_agent_event_store
                .as_ref()
                .ok_or_else(|| "startup_orphan_event_store_unavailable".to_string())?;
            let store = store_arc.lock().await;
            store
                .reconcile_orphaned_tool_attempts_after_restart(
                    STARTUP_PREPARED_TOOL_RECONCILIATION_BATCH,
                )
                .map_err(|error| {
                    format!("reconcile startup prepared-only tool attempts failed: {error}")
                })?
        };
        prepared_tool_reconciled = prepared_tool_reconciled.saturating_add(report.examined);
        prepared_tool_remote_unknown =
            prepared_tool_remote_unknown.saturating_add(report.remote_unknown);
        prepared_tool_effect_unknown =
            prepared_tool_effect_unknown.saturating_add(report.effect_unknown);
        prepared_tool_local_aborted =
            prepared_tool_local_aborted.saturating_add(report.local_aborted);
        prepared_tool_has_more = report.has_more;
        if !report.has_more {
            break;
        }
    }
    if prepared_tool_has_more {
        return Err("startup_prepared_tool_reconciliation_bound_exceeded".into());
    }
    if prepared_tool_reconciled > 0 {
        log::warn!(
            "[startup] reconciled {} unfinished tool attempts: remote_unknown={}, effect_unknown={}, local_aborted={}",
            prepared_tool_reconciled,
            prepared_tool_remote_unknown,
            prepared_tool_effect_unknown,
            prepared_tool_local_aborted,
        );
    }

    // EventStore reconciliation owns the prepared/ambiguous facts. Deliver its
    // durable ActionQueue outbox before generic claim recovery; reversing this
    // order could release a claim whose adapter outcome is actually unknown.
    let mut tool_queue_projection_has_more = false;
    let mut tool_queue_projection_count = 0usize;
    for _ in 0..STARTUP_TOOL_QUEUE_RECONCILIATION_OUTBOX_PASSES {
        let batch = {
            let event_store_arc = state
                .main_chat_agent_event_store
                .as_ref()
                .ok_or_else(|| "startup_orphan_event_store_unavailable".to_string())?;
            let event_store = event_store_arc.lock().await;
            event_store
                .pending_tool_queue_reconciliation_projections(
                    STARTUP_TOOL_QUEUE_RECONCILIATION_OUTBOX_BATCH,
                )
                .map_err(|error| {
                    format!("load startup tool queue reconciliation outbox failed: {error}")
                })?
        };
        tool_queue_projection_has_more = batch.has_more;
        if batch.items.is_empty() {
            break;
        }
        for projection in batch.items {
            {
                let queue_arc = state
                    .main_chat_action_queue_store
                    .as_ref()
                    .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?;
                let queue = queue_arc.lock().await;
                apply_tool_queue_reconciliation_projection(&queue, &projection)?;
            }
            {
                let event_store_arc = state
                    .main_chat_agent_event_store
                    .as_ref()
                    .ok_or_else(|| "startup_orphan_event_store_unavailable".to_string())?;
                let event_store = event_store_arc.lock().await;
                event_store
                    .mark_tool_queue_reconciliation_projection_applied(&projection)
                    .map_err(|error| {
                        format!("ack startup tool queue reconciliation failed: {error}")
                    })?;
            }
            tool_queue_projection_count = tool_queue_projection_count.saturating_add(1);
        }
        if !tool_queue_projection_has_more {
            break;
        }
    }
    if tool_queue_projection_has_more {
        return Err("startup_tool_queue_reconciliation_bound_exceeded".into());
    }

    let replay_recovery = {
        let queue_arc = state
            .main_chat_action_queue_store
            .as_ref()
            .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?;
        let queue = queue_arc.lock().await;
        queue
            .recover_replay_claims_after_process_restart()
            .map_err(|error| format!("Main Chat replay claim recovery failed: {error}"))?
    };
    if tool_queue_projection_count > 0
        || replay_recovery.released_before_dispatch > 0
        || replay_recovery.preserved_dispatched_unknown > 0
    {
        log::warn!(
            "[startup] tool queue projections={}, released_before_dispatch={}, preserved_dispatched_unknown={}",
            tool_queue_projection_count,
            replay_recovery.released_before_dispatch,
            replay_recovery.preserved_dispatched_unknown,
        );
    }

    // Freeze the complete bounded Main Chat owner set before any projection
    // writes change `updated_at`; OFFSET pagination is therefore stable within
    // this snapshot, and the explicit overflow probe degrades rather than
    // silently truncating more than 10,000 tasks.
    let candidate_tasks = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "startup_orphan_task_store_unavailable".to_string())?;
        let store = store_arc.lock().await;
        let mut candidates = Vec::new();
        for page in 0..STARTUP_MAIN_CHAT_TASK_SCAN_PASSES {
            let rows = store
                .list_sessions(
                    None,
                    STARTUP_MAIN_CHAT_TASK_SCAN_BATCH,
                    page * STARTUP_MAIN_CHAT_TASK_SCAN_BATCH,
                )
                .map_err(|error| format!("scan startup Main Chat tasks failed: {error}"))?;
            let row_count = rows.len();
            candidates.extend(rows);
            if row_count < STARTUP_MAIN_CHAT_TASK_SCAN_BATCH {
                break;
            }
        }
        if candidates.len()
            == STARTUP_MAIN_CHAT_TASK_SCAN_BATCH * STARTUP_MAIN_CHAT_TASK_SCAN_PASSES
            && !store
                .list_sessions(None, 1, candidates.len())
                .map_err(|error| format!("probe startup Main Chat task overflow failed: {error}"))?
                .is_empty()
        {
            return Err("startup_main_chat_task_scan_bound_exceeded".into());
        }
        candidates
    };

    let mut reconciled = 0usize;
    for task in candidate_tasks {
        // Exact AgentRun.task_id ownership only. Conversation/session ids are
        // intentionally not considered because they are not canonical task
        // identities and may bind multiple runs.
        let run = {
            let store_arc = state
                .agent_run_store
                .as_ref()
                .ok_or_else(|| "startup_orphan_agent_run_store_unavailable".to_string())?;
            let store = store_arc.lock().await;
            store
                .get_run_for_task_id(&task.id)
                .map_err(|error| format!("load exact startup AgentRun failed: {error}"))?
                .ok_or_else(|| format!("startup_main_chat_agent_run_missing:{}", task.id))?
        };
        if run.task_id != task.id {
            return Err("startup_main_chat_agent_run_task_identity_mismatch".into());
        }
        let lifecycle = {
            let store_arc = state
                .main_chat_agent_event_store
                .as_ref()
                .ok_or_else(|| "startup_orphan_event_store_unavailable".to_string())?;
            let store = store_arc.lock().await;
            store
                .turn_lifecycle_snapshot(&run.task_id)
                .map_err(|error| format!("load startup orphan lifecycle failed: {error}"))?
        };
        if lifecycle
            .bound_run_id
            .as_deref()
            .is_some_and(|bound| bound != run.id)
            || lifecycle
                .lifecycle_event
                .as_ref()
                .is_some_and(|event| event.run_id != run.id || event.task_session_id != run.task_id)
        {
            return Err(format!(
                "startup_orphan_run_task_event_identity_mismatch:{}:{}",
                run.id, run.task_id
            ));
        }
        if lifecycle.lifecycle_event.as_ref().is_some_and(|event| {
            startup_lifecycle_projection_matches(&run, &task, event).unwrap_or(false)
        }) {
            continue;
        }
        let pre_dispatch_failure_marker = if lifecycle.lifecycle_event.is_none() {
            startup_pre_dispatch_failure_marker_exists(state, &run.id, &run.task_id).await?
        } else {
            false
        };

        let verify_terminal = match lifecycle.lifecycle_event {
            Some(event) if event.event_type == "final_delivery.created" => {
                project_startup_final_delivery_receipt(state, &run, &task, &event).await?;
                true
            }
            Some(event)
                if matches!(
                    event.event_type.as_str(),
                    "failed" | "local_aborted" | "interrupted"
                ) =>
            {
                let failure_kind = startup_failure_kind_from_terminal(&event)?;
                crate::main_chat_runtime_support::
                    finalize_main_chat_task_failure_after_durable_receipt(
                        state,
                        failure_kind,
                        "Recovered a terminal Main Chat receipt left unprojected by the previous process.",
                        "bootstrap.orphan_running_projection_recovery",
                        event,
                    )
                    .await?;
                true
            }
            Some(event) if event.event_type == "cancel_requested" => {
                crate::main_chat_runtime_support::finalize_main_chat_task_failure(
                    state,
                    Some(&run.id),
                    Some(&run.task_id),
                    crate::main_chat_runtime_support::MainChatTaskFailureKind::Interrupted,
                    "The previous process ended after local cancellation was requested; remote execution state is unknown.",
                    "bootstrap.orphan_running_after_cancel_request",
                )
                .await?;
                true
            }
            Some(event) if event.event_type == "turn.interrupted" => {
                crate::main_chat_runtime_support::finalize_main_chat_task_failure(
                    state,
                    Some(&run.id),
                    Some(&run.task_id),
                    crate::main_chat_runtime_support::MainChatTaskFailureKind::Interrupted,
                    "Recovered a legacy interrupted receipt left unprojected by the previous process.",
                    "bootstrap.orphan_running_legacy_interrupted",
                )
                .await?;
                true
            }
            Some(event) => {
                return Err(format!(
                    "startup_orphan_unsupported_lifecycle:{}:{}",
                    run.id, event.event_type
                ));
            }
            None if pre_dispatch_failure_marker => {
                crate::main_chat_runtime_support::finalize_main_chat_task_failure(
                    state,
                    Some(&run.id),
                    Some(&run.task_id),
                    crate::main_chat_runtime_support::MainChatTaskFailureKind::UnknownError,
                    "Recovered a typed pre-dispatch persistence failure marker from the previous process.",
                    "bootstrap.pre_dispatch_event_store_failure_recovery",
                )
                .await?;
                true
            }
            None
                if run.status == openlife_core::agent::AgentRunStatus::Running
                    && task.status
                    == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission =>
            {
                let store_arc = state
                    .agent_run_store
                    .as_ref()
                    .ok_or_else(|| "startup_orphan_agent_run_store_unavailable".to_string())?;
                let store = store_arc.lock().await;
                let mut waiting = run.clone();
                waiting.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
                waiting.finished_at = None;
                store
                    .update_run(&waiting)
                    .map_err(|error| format!("project startup waiting run failed: {error}"))?;
                false
            }
            None
                if run.status == openlife_core::agent::AgentRunStatus::Running
                    && matches!(
                    task.status,
                    openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
                        | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled
                ) =>
            {
                return Err(format!(
                    "startup_orphan_terminal_task_missing_durable_receipt:{}:{}",
                    run.id,
                    task.status.as_str()
                ));
            }
            None if run.status == openlife_core::agent::AgentRunStatus::Running => {
                crate::main_chat_runtime_support::finalize_main_chat_task_failure(
                    state,
                    Some(&run.id),
                    Some(&run.task_id),
                    crate::main_chat_runtime_support::MainChatTaskFailureKind::UnknownError,
                    "The previous OpenLife process ended before this Main Chat execution produced a durable terminal receipt.",
                    "bootstrap.orphan_running_process_restart",
                )
                .await?;
                true
            }
            None
                if task.status
                    == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Running =>
            {
                return Err(format!(
                    "startup_orphan_task_running_without_durable_terminal:{}:{}",
                    run.id, run.task_id
                ));
            }
            None
                if run.status == openlife_core::agent::AgentRunStatus::WaitingPermission
                    && task.status
                        == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission =>
            {
                continue;
            }
            None => {
                return Err(format!(
                    "startup_projection_without_durable_terminal_inconsistent:{}:{}:run={}:task={}",
                    run.id,
                    run.task_id,
                    run.status,
                    task.status.as_str()
                ));
            }
        };
        if verify_terminal {
            verify_startup_terminal_projection(state, &run.id, &run.task_id).await?;
        }
        reconciled = reconciled.saturating_add(1);
    }
    Ok(reconciled)
}

fn startup_failure_kind_from_terminal(
    event: &crate::main_chat_event_stream::MainChatAgentDurableEvent,
) -> Result<crate::main_chat_runtime_support::MainChatTaskFailureKind, String> {
    use crate::main_chat_runtime_support::MainChatTaskFailureKind;
    match event.event_type.as_str() {
        "local_aborted" => Ok(MainChatTaskFailureKind::Cancelled),
        "interrupted" => Ok(MainChatTaskFailureKind::Interrupted),
        "failed" => match event
            .payload
            .get("kind")
            .and_then(serde_json::Value::as_str)
        {
            Some("timeout") => Ok(MainChatTaskFailureKind::Timeout),
            Some("provider_error") => Ok(MainChatTaskFailureKind::ProviderError),
            Some("tool_error") => Ok(MainChatTaskFailureKind::ToolError),
            Some("policy_blocker") => Ok(MainChatTaskFailureKind::PolicyBlocker),
            Some("unknown_error") => Ok(MainChatTaskFailureKind::UnknownError),
            _ => Err("startup_orphan_failed_terminal_kind_invalid".into()),
        },
        _ => Err("startup_orphan_terminal_type_invalid".into()),
    }
}

fn startup_lifecycle_projection_matches(
    run: &openlife_core::agent::AgentRun,
    task: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    event: &crate::main_chat_event_stream::MainChatAgentDurableEvent,
) -> Result<bool, String> {
    use openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus;
    use openlife_core::agent::AgentRunStatus;
    let (expected_run, expected_task) = match event.event_type.as_str() {
        "local_aborted" => (AgentRunStatus::Cancelled, AgentTaskSessionStatus::Cancelled),
        "interrupted" => (AgentRunStatus::Failed, AgentTaskSessionStatus::Failed),
        "failed" => {
            let kind = startup_failure_kind_from_terminal(event)?;
            let task_status = if kind
                == crate::main_chat_runtime_support::MainChatTaskFailureKind::PolicyBlocker
            {
                AgentTaskSessionStatus::Blocked
            } else {
                AgentTaskSessionStatus::Failed
            };
            (AgentRunStatus::Failed, task_status)
        }
        "final_delivery.created" => match event
            .payload
            .get("status")
            .and_then(serde_json::Value::as_str)
        {
            Some("completed") => (AgentRunStatus::Completed, AgentTaskSessionStatus::Completed),
            Some("completed_with_pending_items") => (
                AgentRunStatus::WaitingPermission,
                AgentTaskSessionStatus::WaitingPermission,
            ),
            Some("blocked") => (AgentRunStatus::Failed, AgentTaskSessionStatus::Blocked),
            Some("failed") => (AgentRunStatus::Failed, AgentTaskSessionStatus::Failed),
            Some("cancelled") => (AgentRunStatus::Cancelled, AgentTaskSessionStatus::Cancelled),
            _ => return Err("startup_final_delivery_status_invalid".into()),
        },
        "cancel_requested" | "turn.interrupted" => return Ok(false),
        _ => return Err("startup_orphan_terminal_type_invalid".into()),
    };
    Ok(run.status == expected_run && task.status == expected_task)
}

async fn startup_pre_dispatch_failure_marker_exists(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
) -> Result<bool, String> {
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "startup_orphan_task_store_unavailable".to_string())?;
    let store = store_arc.lock().await;
    let marker = store
        .load_pre_dispatch_persistence_failure(task_session_id)
        .map_err(|error| format!("load typed startup persistence marker failed: {error}"))?;
    let Some(marker) = marker else {
        return Ok(false);
    };
    let error_digest_hex = marker.error_digest.strip_prefix("sha256:");
    if marker.task_session_id != task_session_id
        || marker.run_id != run_id
        || marker.failure_kind
            != openlife_core::agent::main_chat_agent_v1::PRE_DISPATCH_PERSISTENCE_FAILURE_KIND
        || error_digest_hex
            .is_none_or(|hex| hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err("startup_pre_dispatch_persistence_marker_identity_invalid".into());
    }
    Ok(true)
}

async fn verify_startup_terminal_projection(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
) -> Result<(), String> {
    let run = {
        let store_arc = state
            .agent_run_store
            .as_ref()
            .ok_or_else(|| "startup_orphan_agent_run_store_unavailable".to_string())?;
        let store = store_arc.lock().await;
        store
            .get_run(run_id)
            .map_err(|error| format!("reload startup recovered run failed: {error}"))?
            .ok_or_else(|| format!("startup_recovered_run_missing:{run_id}"))?
    };
    if run.task_id != task_session_id {
        return Err("startup_recovered_run_task_identity_mismatch".into());
    }
    let task = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "startup_orphan_task_store_unavailable".to_string())?;
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .map_err(|error| format!("reload startup recovered task failed: {error}"))?
            .ok_or_else(|| format!("startup_recovered_task_missing:{task_session_id}"))?
    };
    let event = {
        let store_arc = state
            .main_chat_agent_event_store
            .as_ref()
            .ok_or_else(|| "startup_orphan_event_store_unavailable".to_string())?;
        let store = store_arc.lock().await;
        store
            .latest_turn_lifecycle_event(task_session_id)
            .map_err(|error| format!("reload startup recovered lifecycle failed: {error}"))?
            .ok_or_else(|| "startup_recovered_terminal_missing".to_string())?
    };
    if event.run_id != run_id || event.task_session_id != task_session_id {
        return Err("startup_recovered_terminal_identity_mismatch".into());
    }
    if !startup_lifecycle_projection_matches(&run, &task, &event)? {
        return Err(format!(
            "startup_terminal_projection_still_inconsistent:{run_id}:{task_session_id}"
        ));
    }
    Ok(())
}

async fn project_startup_final_delivery_receipt(
    state: &Arc<AppState>,
    run: &openlife_core::agent::AgentRun,
    task: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    event: &crate::main_chat_event_stream::MainChatAgentDurableEvent,
) -> Result<(), String> {
    if event.object_type != "final_delivery"
        || event
            .payload
            .get("runId")
            .and_then(serde_json::Value::as_str)
            != Some(run.id.as_str())
        || event
            .payload
            .get("taskSessionId")
            .and_then(serde_json::Value::as_str)
            != Some(task.id.as_str())
    {
        return Err("startup_final_delivery_identity_mismatch".into());
    }
    let status = event
        .payload
        .get("status")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "startup_final_delivery_status_missing".to_string())?;
    {
        let store_arc = state
            .agent_run_store
            .as_ref()
            .ok_or_else(|| "startup_orphan_agent_run_store_unavailable".to_string())?;
        let store = store_arc.lock().await;
        let mut projected = run.clone();
        match status {
            "completed" => {
                projected.status = openlife_core::agent::AgentRunStatus::Completed;
                projected.finished_at = Some(event.created_at);
            }
            "completed_with_pending_items" => {
                projected.status = openlife_core::agent::AgentRunStatus::WaitingPermission;
                projected.finished_at = None;
            }
            "blocked" | "failed" => projected.fail(openlife_core::agent::AgentRunError {
                message: "Recovered terminal status from a durable final-delivery receipt.".into(),
                phase: "startup_projection_recovery".into(),
                recoverable: status == "blocked",
            }),
            "cancelled" => projected.cancel(),
            _ => return Err("startup_final_delivery_status_invalid".into()),
        }
        store
            .update_run(&projected)
            .map_err(|error| format!("project startup final AgentRun failed: {error}"))?;
    }
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "startup_orphan_task_store_unavailable".to_string())?;
    let store = store_arc.lock().await;
    match status {
        "completed" => {
            store.complete_session(&task.id, "Recovered from durable final-delivery receipt.")
        }
        "completed_with_pending_items" => store.mark_waiting_permission(&task.id),
        "blocked" => store.block_session(
            &task.id,
            "Recovered blocked state from durable final-delivery receipt.",
        ),
        "failed" => store.fail_session(
            &task.id,
            "Recovered failed state from durable final-delivery receipt.",
        ),
        "cancelled" => store.cancel_session(
            &task.id,
            "Recovered cancelled state from durable final-delivery receipt.",
        ),
        _ => unreachable!("validated final delivery status"),
    }
    .map(|_| ())
    .map_err(|error| format!("project startup final task failed: {error}"))
}

/// Drain canonical-owner outboxes before the product becomes interactive.
/// A deletion projection that cannot be reconciled fails closed at startup so
/// stale content is never presented as if deletion had completed.
pub(crate) async fn reconcile_startup_canonical_outboxes(
    state: &Arc<AppState>,
) -> Result<(), String> {
    let mut lifemodel_drained = false;
    for _ in 0..STARTUP_CANONICAL_OUTBOX_PASSES {
        let report =
            crate::life_model_write_gateway::reconcile_lifemodel_file_mutations_with_state(state)
                .await?;
        if report.degraded > 0 {
            return Err(format!(
                "LifeModel file projection reconciliation degraded: {} delivery attempts",
                report.degraded
            ));
        }
        if !report.backlog_may_remain {
            lifemodel_drained = true;
            break;
        }
    }
    if !lifemodel_drained {
        return Err("LifeModel file projection backlog exceeded startup bound".into());
    }
    for _ in 0..STARTUP_CANONICAL_OUTBOX_PASSES {
        let report = crate::memory_gateway::reconcile_blocking_canonical_outboxes_with_state(
            state,
            STARTUP_CANONICAL_OUTBOX_BATCH,
        )
        .await?;
        if report.blocking_degraded > 0 {
            return Err(format!(
                "canonical deletion/restore projection reconciliation degraded: {} delivery attempts",
                report.blocking_degraded
            ));
        }
        if !report.blocking_backlog_may_remain {
            return Ok(());
        }
    }
    Err("canonical projection reconciliation backlog exceeded startup bound".into())
}

/// Reconcile a bounded amount of already-confirmed Proposal truth before the
/// product window becomes interactive. `true` means a durable indexed backlog
/// remains and must be drained by the async continuation; it never means the
/// effects should be replayed.
pub(crate) async fn reconcile_startup_proposal_projections(
    state: &Arc<AppState>,
) -> Result<bool, String> {
    for _ in 0..STARTUP_PROPOSAL_RECONCILIATION_SYNC_PASSES {
        let report = crate::commands::proposal::reconcile_durable_proposal_projections_with_state(
            state,
            STARTUP_PROPOSAL_RECONCILIATION_BATCH,
        )
        .await?;
        let backlog = report.artifact_backlog_may_remain
            || report.projection_backlog_may_remain
            || report.agent_run_backlog_may_remain;
        if !backlog {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) async fn drain_startup_proposal_projection_backlog(state: Arc<AppState>) {
    const MAX_BACKGROUND_PASSES: usize = 100;
    for _ in 0..MAX_BACKGROUND_PASSES {
        match crate::commands::proposal::reconcile_durable_proposal_projections_with_state(
            &state,
            STARTUP_PROPOSAL_RECONCILIATION_BATCH,
        )
        .await
        {
            Ok(report)
                if !report.artifact_backlog_may_remain
                    && !report.projection_backlog_may_remain
                    && !report.agent_run_backlog_may_remain =>
            {
                return;
            }
            Ok(_) => tokio::task::yield_now().await,
            Err(error) => {
                log::warn!(
                    "[startup] Proposal projection reconciliation remains degraded: {}",
                    error
                );
                return;
            }
        }
    }
    log::warn!(
        "[startup] Proposal projection reconciliation backlog remains after bounded background passes"
    );
}

fn recovery_db_path(file_name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("openlife-recovery")
        .join(std::process::id().to_string());
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!(
            "failed to create OpenLife recovery database directory {}: {}",
            dir.display(),
            e
        );
    }
    dir.join(file_name)
}

fn ephemeral_store_fallback_allowed() -> bool {
    cfg!(feature = "dev-extensions")
}

/// Helper to initialize a store with file-based fallback to in-memory.
fn init_store<T, F, G>(
    file_init: F,
    read_only_init: impl FnOnce() -> Result<T, String>,
    ephemeral_init: G,
    name: &str,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
    persistence: &PersistenceCoordinator,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
    G: FnOnce() -> Result<T, String>,
{
    let warning_count_before = startup_warnings.borrow().len();
    match file_init() {
        Ok(store) => {
            if startup_warnings.borrow().len() > warning_count_before {
                persistence.register_ephemeral_development(
                    name,
                    "dev_ephemeral_store_fallback",
                    "primary durable store failed; specialized development fallback was used",
                );
            } else {
                persistence.register_read_write(name);
            }
            Ok(store)
        }
        Err(e) => {
            startup_warnings
                .borrow_mut()
                .push(format!("{} file init failed: {}", name, e));
            if !ephemeral_store_fallback_allowed() {
                return match read_only_init() {
                    Ok(store) => {
                        persistence.register_read_only(name, "durable_store_write_open_failed", &e);
                        startup_warnings.borrow_mut().push(format!(
                            "{name} entered explicit read-only canonical recovery; all provider, tool, and canonical-write effects are disabled"
                        ));
                        Ok(store)
                    }
                    Err(read_only_error) => {
                        persistence.register_unavailable(
                            name,
                            "durable_store_unavailable",
                            &format!("primary={e}; read_only={read_only_error}"),
                        );
                        Err(format!(
                            "{name} canonical store is unavailable: primary={e}; read_only={read_only_error}"
                        ))
                    }
                };
            }
            ephemeral_init()
                .map(|store| {
                    persistence.register_ephemeral_development(
                        name,
                        "dev_ephemeral_store_fallback",
                        &e,
                    );
                    store
                })
                .map_err(|e| {
                    let msg = format!(
                        "CRITICAL: {} in-memory fallback also failed: {}. \
                     System resources may be exhausted.",
                        name, e
                    );
                    log::warn!("[startup] {}", msg);
                    msg
                })
        }
    }
}

fn optional_store<T>(
    result: Result<T, String>,
    name: &str,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Option<T> {
    match result {
        Ok(store) => Some(store),
        Err(error) => {
            log::warn!("[startup] {name} unavailable: {error}");
            startup_warnings.borrow_mut().push(format!(
                "{name} canonical state is unavailable/unknown; the product remains degraded and all effects are disabled"
            ));
            None
        }
    }
}

fn required_store_or_unavailable<T>(
    result: Result<T, String>,
    name: &str,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
    unavailable_sentinel: impl FnOnce() -> Result<T, String>,
) -> T {
    match result {
        Ok(store) => store,
        Err(error) => {
            log::error!("[startup] {name} unavailable: {error}");
            startup_warnings.borrow_mut().push(format!(
                "{name} canonical state is unavailable/unknown; a schema-less query-only sentinel is active and all effects are disabled"
            ));
            unavailable_sentinel().unwrap_or_else(|sentinel_error| {
                panic!(
                    "{name} unavailable sentinel allocation failed after canonical open failure: {sentinel_error}"
                )
            })
        }
    }
}

fn init_memory_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<MemoryStore, String> {
    match MemoryStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "memory.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("memory.db");
            startup_warnings.borrow_mut().push(format!(
                "memory.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match MemoryStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 memory.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    MemoryStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 memory store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_feedback_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<FeedbackStore, String> {
    match FeedbackStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "feedback.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("feedback.db");
            startup_warnings.borrow_mut().push(format!(
                "feedback.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match FeedbackStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 feedback.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    FeedbackStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 feedback store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_vector_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<VectorStore, String> {
    match VectorStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "vectors.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("vectors.db");
            startup_warnings.borrow_mut().push(format!(
                "vectors.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match VectorStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 vectors.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    VectorStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 vector store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_agent_run_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
    receipt_key: &AgentRunReceiptKey,
) -> Result<openlife_core::agent::AgentRunStore, String> {
    match openlife_core::agent::AgentRunStore::new_with_receipt_key(db_path, receipt_key.clone()) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "agent_runs.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("agent_runs.db");
            startup_warnings.borrow_mut().push(format!(
                "agent_runs.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match openlife_core::agent::AgentRunStore::new_with_receipt_key(
                &fallback,
                receipt_key.clone(),
            ) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 agent_runs.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    openlife_core::agent::AgentRunStore::new_in_memory_with_receipt_key(
                        receipt_key.clone(),
                    )
                    .map_err(|memory_err| {
                        format!(
                            "所有 agent run store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_evidence_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<openlife_core::agent::EvidenceStore, String> {
    match openlife_core::agent::EvidenceStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "evidence.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("evidence.db");
            startup_warnings.borrow_mut().push(format!(
                "evidence.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match openlife_core::agent::EvidenceStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 evidence.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    openlife_core::agent::EvidenceStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 evidence store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_life_event_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
    receipt_key: &AgentRunReceiptKey,
) -> Result<openlife_core::agent::LifeEventStore, String> {
    match openlife_core::agent::LifeEventStore::new_with_receipt_key(db_path, receipt_key.clone()) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "life_events.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("life_events.db");
            startup_warnings.borrow_mut().push(format!(
                "life_events.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match openlife_core::agent::LifeEventStore::new_with_receipt_key(
                &fallback,
                receipt_key.clone(),
            ) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 life_events.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    openlife_core::agent::LifeEventStore::new_in_memory_with_receipt_key(
                        receipt_key.clone(),
                    )
                    .map_err(|memory_err| {
                            format!(
                                "所有 life event store 初始化失败: primary={}, fallback={}, in_memory={}",
                                primary_err, fallback_err, memory_err
                            )
                        })
                }
            }
        }
    }
}

fn init_heuristic_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<openlife_core::agent::HeuristicStore, String> {
    match openlife_core::agent::HeuristicStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "heuristics.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("heuristics.db");
            startup_warnings.borrow_mut().push(format!(
                "heuristics.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match openlife_core::agent::HeuristicStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 heuristics.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    openlife_core::agent::HeuristicStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 heuristic store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_proposal_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<ProposalStore, String> {
    match ProposalStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "proposals.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("proposals.db");
            startup_warnings.borrow_mut().push(format!(
                "proposals.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match ProposalStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 proposals.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    ProposalStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 proposal store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_memory_lifecycle_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<MemoryLifecycleStore, String> {
    match MemoryLifecycleStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "memory_lifecycle.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("memory_lifecycle.db");
            startup_warnings.borrow_mut().push(format!(
                "memory_lifecycle.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match MemoryLifecycleStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 memory_lifecycle.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    MemoryLifecycleStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 memory lifecycle store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_plan_execute_session_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
) -> Result<PlanExecuteSessionStore, String> {
    match PlanExecuteSessionStore::new(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "plan_execute_sessions.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("plan_execute_sessions.db");
            startup_warnings.borrow_mut().push(format!(
                "plan_execute_sessions.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match PlanExecuteSessionStore::new(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 plan_execute_sessions.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    PlanExecuteSessionStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 Plan-Execute session store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_main_chat_agent_session_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
    receipt_key: &AgentRunReceiptKey,
) -> Result<AgentTaskSessionStore, String> {
    match AgentTaskSessionStore::new_with_receipt_key(db_path, receipt_key.clone()) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "main_chat_agent_sessions.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("main_chat_agent_sessions.db");
            startup_warnings.borrow_mut().push(format!(
                "main_chat_agent_sessions.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match AgentTaskSessionStore::new_with_receipt_key(&fallback, receipt_key.clone()) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 main_chat_agent_sessions.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    AgentTaskSessionStore::new_in_memory_with_receipt_key(receipt_key.clone())
                        .map_err(|memory_err| {
                        format!(
                            "所有 Main Chat Agent session store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                        })
                }
            }
        }
    }
}

fn init_main_chat_action_queue_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
    authority_key_material: &[u8; 32],
) -> Result<ActionQueueStore, String> {
    let open = |path: &Path| {
        let key = ActionQueueAuthorityKey::from_key_material(authority_key_material)
            .map_err(|error| error.to_string())?;
        ActionQueueStore::new_with_authority_key(path, key).map_err(|error| error.to_string())
    };
    match open(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "main_chat_action_queue.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("main_chat_action_queue.db");
            startup_warnings.borrow_mut().push(format!(
                "main_chat_action_queue.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match open(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 main_chat_action_queue.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    ActionQueueStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 Main Chat action queue store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn init_main_chat_agent_event_store(
    db_path: &Path,
    startup_warnings: &std::cell::RefCell<Vec<String>>,
    digest_key_material: &[u8; 32],
) -> Result<MainChatAgentEventStore, String> {
    let open = |path: &Path| {
        let key = MainChatEventDigestKey::from_key_material(digest_key_material)
            .map_err(|error| error.to_string())?;
        MainChatAgentEventStore::new_with_digest_key(path, key).map_err(|error| error.to_string())
    };
    match open(db_path) {
        Ok(store) => Ok(store),
        Err(primary_err) => {
            if !ephemeral_store_fallback_allowed() {
                return Err(format!(
                    "main_chat_agent_events.db durable initialization failed: {primary_err}"
                ));
            }
            let fallback = recovery_db_path("main_chat_agent_events.db");
            startup_warnings.borrow_mut().push(format!(
                "main_chat_agent_events.db 初始化失败，正在使用临时数据库：{}",
                primary_err
            ));
            match open(&fallback) {
                Ok(store) => Ok(store),
                Err(fallback_err) => {
                    startup_warnings.borrow_mut().push(format!(
                        "临时 main_chat_agent_events.db 初始化也失败，已降级为内存数据库：{}",
                        fallback_err
                    ));
                    MainChatAgentEventStore::new_in_memory().map_err(|memory_err| {
                        format!(
                            "所有 Main Chat agent event store 初始化失败: primary={}, fallback={}, in_memory={}",
                            primary_err, fallback_err, memory_err
                        )
                    })
                }
            }
        }
    }
}

fn build_legacy_scheduled_task_review_proposal(
    candidate: &openlife_core::tasks::LegacyScheduledTaskReviewCandidate,
) -> Result<(AgentProposal, String), String> {
    let identity_digest =
        openlife_core::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
            "schema": "openlife.legacyScheduledReviewIdentity.v1",
            "sourceDigest": candidate.source_digest.clone(),
            "sourceOrdinal": candidate.source_ordinal,
            "itemDigest": candidate.item_digest.clone(),
        }))
        .1;
    let identity_suffix = identity_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| "legacy scheduled review identity digest is invalid".to_string())?;
    let proposal_id = format!("legacy-scheduled-review-{identity_suffix}");
    let source_detail = format!(
        "legacy_scheduled_review:{}:{}:{}",
        candidate.source_digest, candidate.source_ordinal, candidate.item_digest
    );
    let source_run_id_digest = candidate
        .source_run_id
        .as_deref()
        .map(|value| openlife_core::agent::metadata_safe::metadata_safe_text_digest(value).1);
    let source_proposal_id_digest = candidate
        .source_proposal_id
        .as_deref()
        .map(|value| openlife_core::agent::metadata_safe::metadata_safe_text_digest(value).1);
    let after = serde_json::json!({
        "title": candidate.title.clone(),
        "description": candidate.description.clone(),
        "due_date": candidate.due_at.clone(),
        "scheduled_at": candidate.due_at.clone(),
        "priority": candidate.priority.clone(),
        "tool": candidate.action_type.clone(),
        "legacy_migration": {
            "source_digest": candidate.source_digest.clone(),
            "source_ordinal": candidate.source_ordinal,
            "item_digest": candidate.item_digest.clone(),
            "effect_state": "review_required",
            "source_run_id_digest": source_run_id_digest,
            "source_proposal_id_digest": source_proposal_id_digest,
        }
    });
    let mut proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            &format!("tasks.legacy_review.{}", &identity_suffix[..16]),
            after,
            "A provably not-yet-due legacy scheduled task requires fresh Review Center approval before it can enter the canonical TaskStore.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
    proposal.id = proposal_id.clone();
    proposal.source_detail = Some(source_detail);
    proposal.created_at = chrono::DateTime::parse_from_rfc3339(&candidate.review_created_at)
        .map_err(|_| "legacy scheduled review creation snapshot is invalid".to_string())?
        .with_timezone(&chrono::Utc);
    proposal.expires_at = Some(
        chrono::DateTime::parse_from_rfc3339(&candidate.due_at)
            .map_err(|_| "legacy scheduled review expiry snapshot is invalid".to_string())?
            .with_timezone(&chrono::Utc),
    );
    if proposal
        .expires_at
        .is_some_and(|expiry| expiry <= proposal.created_at)
    {
        return Err("legacy scheduled review snapshot is not future-bounded".into());
    }
    Ok((proposal, proposal_id))
}

fn stage_legacy_scheduled_task_review_proposals(
    task_store: &openlife_core::tasks::TaskStore,
    proposal_store: &ProposalStore,
    evidence_directory: &Path,
) -> Result<usize, String> {
    let candidates = task_store
        .pending_legacy_review_candidates(evidence_directory)
        .map_err(|error| error.to_string())?;
    let mut staged = 0;
    for candidate in candidates {
        let (proposal, proposal_id) = build_legacy_scheduled_task_review_proposal(&candidate)?;
        if let Some(existing) = proposal_store
            .get_proposal(&proposal_id)
            .map_err(|error| error.to_string())?
        {
            if existing.source_detail != proposal.source_detail
                || existing.run_id != proposal.run_id
                || existing.proposal_type != proposal.proposal_type
                || existing.source != proposal.source
                || existing.affected_path != proposal.affected_path
                || existing.base_hash != proposal.base_hash
                || existing.before != proposal.before
                || existing.after != proposal.after
                || existing.reason != proposal.reason
                || existing.confidence.to_bits() != proposal.confidence.to_bits()
                || existing.risk_level != proposal.risk_level
                || existing.created_at != proposal.created_at
                || existing.expires_at != proposal.expires_at
                || existing.resolved_at.is_some()
                || !matches!(
                    existing.status,
                    openlife_core::agent::ProposalStatus::Pending
                        | openlife_core::agent::ProposalStatus::Postponed
                        | openlife_core::agent::ProposalStatus::Edited
                )
                || existing.is_expired()
            {
                return Err(
                    "legacy scheduled review proposal id resolves to a non-exact snapshot".into(),
                );
            }
            if !task_store
                .mark_legacy_review_proposal_staged(&candidate, &proposal_id)
                .map_err(|error| error.to_string())?
            {
                return Err(
                    "legacy scheduled review migration journal rejected the exact proposal".into(),
                );
            }
            staged += 1;
            continue;
        }
        let outcome = ReviewWorkflow::new(proposal_store)
            .submit(
                DurableWriteRequest::from_agent_proposal(
                    DurableWriteSource::ManualOverride,
                    DurableWriteSubject::Calendar,
                    proposal,
                    "A legacy future scheduled task is pending fresh Review Center approval; it has not been scheduled or executed.",
                )
                .with_existing_proposal_id(Some(proposal_id.clone()))
                .with_idempotency_key(format!(
                    "legacy_scheduled_review:{}:{}",
                    candidate.source_digest, candidate.source_ordinal
                )),
            )
            .map_err(|error| error.to_string())?;
        if outcome.proposal_id() != proposal_id {
            return Err("legacy scheduled review did not preserve its deterministic id".into());
        }
        if !task_store
            .mark_legacy_review_proposal_staged(&candidate, &proposal_id)
            .map_err(|error| error.to_string())?
        {
            return Err("legacy scheduled review migration journal rejected the proposal".into());
        }
        staged += 1;
    }
    Ok(staged)
}

/// Bootstrap the entire application: config, stores, routers, engines, AppState.
/// Returns assembled AppState along with startup warnings.
pub fn bootstrap(data_dir: PathBuf) -> BootstrapResult {
    bootstrap_with_secret_store(data_dir, &StartupKeyringSecretStore::default())
}

#[cfg(test)]
pub(crate) fn bootstrap_with_secret_store_for_test(
    data_dir: PathBuf,
    secret_store: &dyn SecretStore,
) -> BootstrapResult {
    bootstrap_with_secret_store(data_dir, secret_store)
}

fn bootstrap_with_secret_store(
    data_dir: PathBuf,
    secret_store: &dyn SecretStore,
) -> BootstrapResult {
    let startup_warnings = std::cell::RefCell::new(Vec::new());
    let persistence = Arc::new(PersistenceCoordinator::for_release_bootstrap());

    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        persistence.degrade_globally("application_data_directory_unavailable");
        startup_warnings.borrow_mut().push(format!(
            "应用数据目录创建失败：{} ({})",
            data_dir.display(),
            e
        ));
    }

    let config_path = data_dir.join("config.yaml");
    let (mut config, config_warning) = AppConfig::load_or_default_with_warning(&config_path);
    if let Some(warning) = config_warning {
        persistence.register_unavailable("ConfigStore", "config_load_failed", &warning);
        startup_warnings.borrow_mut().push(warning);
    } else {
        persistence.register_read_write("ConfigStore");
    }
    let secret_hydration = hydrate_config_secrets(&mut config, secret_store);
    for capability in &secret_hydration.fail_closed_capabilities {
        let owner = match capability.as_str() {
            "provider_credential" => "ProviderCredentialStore",
            "search_provider_credential" => "SearchProviderCredentialStore",
            _ => "CredentialStore",
        };
        persistence.register_unavailable(
            owner,
            "secret_hydration_failed_closed",
            &format!("{capability} is disabled because OS credential hydration did not complete"),
        );
    }
    startup_warnings
        .borrow_mut()
        .extend(secret_hydration.warnings);
    if secret_hydration.rewrite_config_without_plaintext {
        if let Err(error) = config.save(&config_path) {
            persistence.register_unavailable(
                "ConfigStore",
                "config_secret_rewrite_failed",
                &error.to_string(),
            );
            startup_warnings.borrow_mut().push(format!(
                "credential migration succeeded but plaintext config rewrite failed: {error}"
            ));
        }
    }

    // Apply system configuration
    openlife_core::ollama::set_ollama_cache_ttl_seconds(config.system.ollama_cache_ttl_seconds);

    // Initialize web search provider configuration
    openlife_core::agent::action_executor::helpers::set_search_config(
        &config.system.search_provider,
        &config.system.search_provider_key,
        &config.system.searxng_url,
    );

    let life_model_manager = LifeModelManager::new(data_dir.join("life-model").join("current"));
    match life_model_manager.load() {
        Ok(_) => persistence.register_read_write("LifeModelFileStore"),
        Err(error) => persistence.register_unavailable(
            "LifeModelFileStore",
            "lifemodel_load_failed",
            &error.to_string(),
        ),
    }
    match openlife_core::persistence_outbox::FileMutationJournal::new(
        life_model_manager.mutation_journal_path(),
    ) {
        Ok(_) => persistence.register_read_write("LifeModelFileJournal"),
        Err(error) => persistence.register_unavailable(
            "LifeModelFileJournal",
            "lifemodel_journal_open_failed",
            &error.to_string(),
        ),
    }

    let db_path = data_dir.join("memory.db");
    let memory_store = init_store(
        || init_memory_store(&db_path, &startup_warnings),
        || MemoryStore::open_read_only_existing(&db_path).map_err(|e| e.to_string()),
        || MemoryStore::new_in_memory().map_err(|e| e.to_string()),
        "MemoryStore",
        &startup_warnings,
        &persistence,
    );
    let memory_store =
        required_store_or_unavailable(memory_store, "MemoryStore", &startup_warnings, || {
            MemoryStore::unavailable_sentinel().map_err(|error| error.to_string())
        });

    let feedback_db_path = data_dir.join("feedback.db");
    let feedback_store = init_store(
        || init_feedback_store(&feedback_db_path, &startup_warnings),
        || FeedbackStore::open_read_only_existing(&feedback_db_path).map_err(|e| e.to_string()),
        || FeedbackStore::new_in_memory().map_err(|e| e.to_string()),
        "FeedbackStore",
        &startup_warnings,
        &persistence,
    );
    let feedback_store =
        required_store_or_unavailable(feedback_store, "FeedbackStore", &startup_warnings, || {
            FeedbackStore::unavailable_sentinel().map_err(|error| error.to_string())
        });

    let vector_db_path = data_dir.join("vectors.db");
    let vector_store = init_store(
        || init_vector_store(&vector_db_path, &startup_warnings),
        || VectorStore::open_read_only_existing(&vector_db_path).map_err(|e| e.to_string()),
        || VectorStore::new_in_memory().map_err(|e| e.to_string()),
        "VectorStore",
        &startup_warnings,
        &persistence,
    );
    let vector_store =
        required_store_or_unavailable(vector_store, "VectorStore", &startup_warnings, || {
            VectorStore::unavailable_sentinel().map_err(|error| error.to_string())
        });

    let agent_run_receipt_key =
        match hydrate_or_create_integrity_key(AGENT_RUN_RECEIPT_KEY_REF, secret_store) {
            Ok(key) => match AgentRunReceiptKey::from_bytes(key) {
                Ok(key) => Some(key),
                Err(error) => {
                    startup_warnings.borrow_mut().push(format!(
                    "AgentRun receipt key is invalid; AgentRun persistence is disabled: {error}"
                ));
                    None
                }
            },
            Err(error) => {
                startup_warnings.borrow_mut().push(format!(
                "AgentRun receipt key is unavailable; AgentRun persistence is disabled: {error}"
            ));
                None
            }
        };
    let agent_runs_db_path = data_dir.join("agent_runs.db");
    let agent_run_store = init_store(
        || {
            let key = agent_run_receipt_key
                .as_ref()
                .ok_or_else(|| "agent_run_receipt_key_unavailable".to_string())?;
            init_agent_run_store(&agent_runs_db_path, &startup_warnings, key)
        },
        || {
            let key = agent_run_receipt_key
                .as_ref()
                .ok_or_else(|| "agent_run_receipt_key_unavailable".to_string())?;
            openlife_core::agent::AgentRunStore::open_read_only_existing_with_receipt_key(
                &agent_runs_db_path,
                key.clone(),
            )
            .map_err(|e| e.to_string())
        },
        || {
            let key = agent_run_receipt_key
                .as_ref()
                .ok_or_else(|| "agent_run_receipt_key_unavailable".to_string())?;
            openlife_core::agent::AgentRunStore::new_in_memory_with_receipt_key(key.clone())
                .map_err(|e| e.to_string())
        },
        "AgentRunStore",
        &startup_warnings,
        &persistence,
    );
    let agent_run_store = optional_store(agent_run_store, "AgentRunStore", &startup_warnings)
        .and_then(|store| match store.bind_canonical_memory_store(&memory_store) {
            Ok(()) => Some(store),
            Err(error) => {
                persistence.register_unavailable(
                    "AgentRunStore",
                    "canonical_memory_store_binding_failed",
                    &error.to_string(),
                );
                startup_warnings.borrow_mut().push(format!(
                    "AgentRunStore cannot bind the canonical MemoryStore and is unavailable: {error}"
                ));
                None
            }
        });

    let evidence_db_path = data_dir.join("evidence.db");
    let evidence_store = init_store(
        || init_evidence_store(&evidence_db_path, &startup_warnings),
        || {
            openlife_core::agent::EvidenceStore::open_read_only_existing(&evidence_db_path)
                .map_err(|e| e.to_string())
        },
        || openlife_core::agent::EvidenceStore::new_in_memory().map_err(|e| e.to_string()),
        "EvidenceStore",
        &startup_warnings,
        &persistence,
    );
    let evidence_store =
        required_store_or_unavailable(evidence_store, "EvidenceStore", &startup_warnings, || {
            openlife_core::agent::EvidenceStore::unavailable_sentinel()
                .map_err(|error| error.to_string())
        });

    let life_events_db_path = data_dir.join("life_events.db");
    let life_event_store = init_store(
        || {
            let key = agent_run_receipt_key
                .as_ref()
                .ok_or_else(|| "life_event_receipt_key_unavailable".to_string())?;
            init_life_event_store(&life_events_db_path, &startup_warnings, key)
        },
        || {
            let key = agent_run_receipt_key
                .as_ref()
                .ok_or_else(|| "life_event_receipt_key_unavailable".to_string())?;
            openlife_core::agent::LifeEventStore::open_read_only_existing_with_receipt_key(
                &life_events_db_path,
                key.clone(),
            )
            .map_err(|e| e.to_string())
        },
        || {
            let key = agent_run_receipt_key
                .as_ref()
                .ok_or_else(|| "life_event_receipt_key_unavailable".to_string())?;
            openlife_core::agent::LifeEventStore::new_in_memory_with_receipt_key(key.clone())
                .map_err(|e| e.to_string())
        },
        "LifeEventStore",
        &startup_warnings,
        &persistence,
    );
    let life_event_store = optional_store(life_event_store, "LifeEventStore", &startup_warnings)
        .and_then(|store| {
            let binding = store
                .bind_canonical_memory_store(&memory_store)
                .and_then(|()| {
                    if let Some(agent_run_store) = agent_run_store.as_ref() {
                        store.bind_canonical_agent_run_store(agent_run_store)?;
                    }
                    Ok(())
                });
            match binding {
                Ok(()) => Some(store),
                Err(error) => {
                    persistence.register_unavailable(
                        "LifeEventStore",
                        "canonical_source_owner_binding_failed",
                        &error.to_string(),
                    );
                    startup_warnings.borrow_mut().push(format!(
                        "LifeEventStore canonical source owner binding failed: {error}"
                    ));
                    None
                }
            }
        });

    let heuristics_db_path = data_dir.join("heuristics.db");
    let heuristic_store = init_store(
        || init_heuristic_store(&heuristics_db_path, &startup_warnings),
        || {
            openlife_core::agent::HeuristicStore::open_read_only_existing(&heuristics_db_path)
                .map_err(|e| e.to_string())
        },
        || openlife_core::agent::HeuristicStore::new_in_memory().map_err(|e| e.to_string()),
        "HeuristicStore",
        &startup_warnings,
        &persistence,
    );
    let heuristic_store =
        required_store_or_unavailable(heuristic_store, "HeuristicStore", &startup_warnings, || {
            openlife_core::agent::HeuristicStore::unavailable_sentinel()
                .map_err(|error| error.to_string())
        });
    if let Err(e) = heuristic_store.seed_mvp_heuristics() {
        startup_warnings
            .borrow_mut()
            .push(format!("initial heuristics seed failed: {}", e));
    }
    let policy_store = openlife_core::agent::PolicyStore::mvp_builtin();
    match HSAssetAuthorityRegistry::new(life_model_manager.hs_asset_authority_registry_path()) {
        Ok(hs_authority_registry) => {
            persistence.register_read_write("HSAssetAuthorityRegistry");
            let hs_reconciliation = if persistence.bootstrap_mutations_safe() {
                (|| -> Result<(), String> {
                    let hs_authority_model = life_model_manager.load().map_err(|error| {
                        format!(
                    "LifeModel could not be loaded for HS asset authority reconciliation: {error}"
                )
                    })?;
                    let hs_cutover = reconcile_collaboration_guidance_authority(
                        &hs_authority_registry,
                        &hs_authority_model,
                        &heuristic_store,
                    )
                    .map_err(|error| {
                        format!("collaboration guidance authority reconciliation failed: {error}")
                    })?;
                    match hs_cutover.status {
                        CollaborationGuidanceCutoverStatus::Promoted
                        | CollaborationGuidanceCutoverStatus::AlreadyPromoted => life_model_manager
                            .save_hs_compatibility_view(&hs_cutover.projection.yaml)
                            .map_err(|error| {
                                format!(
                            "derived collaboration guidance YAML projection failed: {error}"
                        )
                            })?,
                        CollaborationGuidanceCutoverStatus::ShadowEvidencePending => {
                            startup_warnings.borrow_mut().push(
                        "collaboration guidance remains LifeModel YAML-owned until a real product runtime receipt is observed; LM-C promotion is fail-closed"
                            .into(),
                    );
                        }
                    }
                    Ok(())
                })()
            } else {
                startup_warnings.borrow_mut().push(
                    "HS asset authority reconciliation skipped because another canonical store is degraded"
                        .into(),
                );
                Ok(())
            };
            if let Err(error) = hs_reconciliation {
                persistence.register_unavailable(
                    "HSAssetAuthorityRegistry",
                    "hs_asset_authority_reconciliation_failed",
                    &error,
                );
                startup_warnings.borrow_mut().push(format!(
                    "HS asset authority is unavailable; product entered read-only degraded mode: {error}"
                ));
            }
        }
        Err(error) => {
            persistence.register_unavailable(
                "HSAssetAuthorityRegistry",
                "hs_asset_authority_registry_open_failed",
                &error.to_string(),
            );
            startup_warnings.borrow_mut().push(format!(
                "HS asset authority registry is unavailable; product entered read-only degraded mode: {error}"
            ));
        }
    }

    let proposals_db_path = data_dir.join("proposals.db");
    let proposal_store = init_store(
        || init_proposal_store(&proposals_db_path, &startup_warnings),
        || ProposalStore::open_read_only_existing(&proposals_db_path).map_err(|e| e.to_string()),
        || ProposalStore::new_in_memory().map_err(|e| e.to_string()),
        "ProposalStore",
        &startup_warnings,
        &persistence,
    );
    let proposal_store = optional_store(proposal_store, "ProposalStore", &startup_warnings);

    let memory_lifecycle_db_path = data_dir.join("memory_lifecycle.db");
    let memory_lifecycle_store = init_store(
        || init_memory_lifecycle_store(&memory_lifecycle_db_path, &startup_warnings),
        || {
            MemoryLifecycleStore::open_read_only_existing(&memory_lifecycle_db_path)
                .map_err(|e| e.to_string())
        },
        || MemoryLifecycleStore::new_in_memory().map_err(|e| e.to_string()),
        "MemoryLifecycleStore",
        &startup_warnings,
        &persistence,
    );
    let memory_lifecycle_store = optional_store(
        memory_lifecycle_store,
        "MemoryLifecycleStore",
        &startup_warnings,
    );

    let plan_execute_sessions_db_path = data_dir.join("plan_execute_sessions.db");
    let plan_execute_session_store = init_store(
        || init_plan_execute_session_store(&plan_execute_sessions_db_path, &startup_warnings),
        || {
            PlanExecuteSessionStore::open_read_only_existing(&plan_execute_sessions_db_path)
                .map_err(|e| e.to_string())
        },
        || PlanExecuteSessionStore::new_in_memory().map_err(|e| e.to_string()),
        "PlanExecuteSessionStore",
        &startup_warnings,
        &persistence,
    );
    let plan_execute_session_store = optional_store(
        plan_execute_session_store,
        "PlanExecuteSessionStore",
        &startup_warnings,
    );

    let main_chat_agent_sessions_db_path = data_dir.join("main_chat_agent_sessions.db");
    let main_chat_agent_session_store = init_store(
        || {
            let key = agent_run_receipt_key
                .as_ref()
                .ok_or_else(|| "main_chat_agent_session_receipt_key_unavailable".to_string())?;
            init_main_chat_agent_session_store(
                &main_chat_agent_sessions_db_path,
                &startup_warnings,
                key,
            )
        },
        || Err("read-only session-store reopen is not implemented".into()),
        || {
            let key = agent_run_receipt_key
                .as_ref()
                .ok_or_else(|| "main_chat_agent_session_receipt_key_unavailable".to_string())?;
            AgentTaskSessionStore::new_in_memory_with_receipt_key(key.clone())
                .map_err(|e| e.to_string())
        },
        "MainChatAgentSessionStore",
        &startup_warnings,
        &persistence,
    );
    let main_chat_agent_session_store = optional_store(
        main_chat_agent_session_store,
        "MainChatAgentSessionStore",
        &startup_warnings,
    )
    .and_then(
        |store| match store.bind_canonical_memory_store(&memory_store) {
            Ok(()) => Some(store),
            Err(error) => {
                persistence.register_unavailable(
                    "MainChatAgentSessionStore",
                    "canonical_memory_store_binding_failed",
                    &error.to_string(),
                );
                startup_warnings.borrow_mut().push(format!(
                    "MainChatAgentSessionStore cannot bind MemoryStore: {error}"
                ));
                None
            }
        },
    );

    let action_queue_authority_key = match hydrate_or_create_integrity_key(
        ACTION_QUEUE_AUTHORITY_KEY_REF,
        secret_store,
    ) {
        Ok(key) => Some(key),
        Err(error) => {
            startup_warnings.borrow_mut().push(format!(
                    "ActionQueue authority key is unavailable; automatic replay authority is disabled: {error}"
                ));
            None
        }
    };
    let task_store_db_path = data_dir.join("tasks.db");
    let task_store_authority_key = match hydrate_or_create_canonical_store_integrity_key(
        TASK_STORE_AUTHORITY_KEY_REF,
        &task_store_db_path,
        secret_store,
    ) {
        Ok(key) => openlife_core::tasks::TaskStoreAuthorityKey::from_key_material(&key)
            .map(Some)
            .unwrap_or_else(|error| {
                startup_warnings.borrow_mut().push(format!(
                    "TaskStore authority key is invalid; scheduled execution is disabled: {error}"
                ));
                None
            }),
        Err(error) => {
            startup_warnings.borrow_mut().push(format!(
                "TaskStore authority key is unavailable; scheduled execution is disabled: {error}"
            ));
            None
        }
    };
    let main_chat_action_queue_db_path = data_dir.join("main_chat_action_queue.db");
    let main_chat_action_queue_store = init_store(
        || {
            let key = action_queue_authority_key
                .as_ref()
                .ok_or_else(|| "action_queue_authority_key_unavailable".to_string())?;
            init_main_chat_action_queue_store(
                &main_chat_action_queue_db_path,
                &startup_warnings,
                key,
            )
        },
        || Err("read-only action-queue reopen is not implemented".into()),
        || ActionQueueStore::new_in_memory().map_err(|e| e.to_string()),
        "MainChatActionQueueStore",
        &startup_warnings,
        &persistence,
    );
    let main_chat_action_queue_store = optional_store(
        main_chat_action_queue_store,
        "MainChatActionQueueStore",
        &startup_warnings,
    );

    let main_chat_event_integrity_key = match hydrate_or_create_integrity_key(
        MAIN_CHAT_EVENT_INTEGRITY_KEY_REF,
        secret_store,
    ) {
        Ok(key) => Some(key),
        Err(error) => {
            startup_warnings.borrow_mut().push(format!(
                    "Main Chat event integrity key is unavailable; durable event truth is unavailable: {error}"
                ));
            None
        }
    };
    let main_chat_agent_events_db_path = data_dir.join("main_chat_agent_events.db");
    let main_chat_agent_event_store = init_store(
        || {
            let key = main_chat_event_integrity_key
                .as_ref()
                .ok_or_else(|| "main_chat_event_integrity_key_unavailable".to_string())?;
            init_main_chat_agent_event_store(
                &main_chat_agent_events_db_path,
                &startup_warnings,
                key,
            )
        },
        || Err("read-only event-store reopen is not implemented".into()),
        || MainChatAgentEventStore::new_in_memory().map_err(|e| e.to_string()),
        "MainChatAgentEventStore",
        &startup_warnings,
        &persistence,
    );
    let main_chat_agent_event_store = optional_store(
        main_chat_agent_event_store,
        "MainChatAgentEventStore",
        &startup_warnings,
    );
    if let (Some(queue), Some(event_store)) = (
        main_chat_action_queue_store.as_ref(),
        main_chat_agent_event_store.as_ref(),
    ) {
        let install_result = event_store
            .reconciliation_attestation_public_key()
            .and_then(|public_key| {
                queue.install_event_store_reconciliation_public_key(&public_key)
            });
        if let Err(error) = install_result {
            persistence.register_unavailable(
                "MainChatToolReconciliationBridge",
                "event_store_attestation_key_binding_failed",
                &error.to_string(),
            );
            startup_warnings.borrow_mut().push(format!(
                "Main Chat tool reconciliation attestation is unavailable: {error}"
            ));
        }
    }

    let patches_db_path = data_dir.join("patches.db");
    let patch_store = init_store(
        || {
            openlife_core::life_model::patch_store::PatchStore::new(&patches_db_path)
                .map_err(|e| e.to_string())
        },
        || {
            openlife_core::life_model::patch_store::PatchStore::open_read_only_existing(
                &patches_db_path,
            )
            .map_err(|e| e.to_string())
        },
        || {
            openlife_core::life_model::patch_store::PatchStore::new_in_memory()
                .map_err(|e| e.to_string())
        },
        "PatchStore",
        &startup_warnings,
        &persistence,
    );
    let patch_store = optional_store(patch_store, "PatchStore", &startup_warnings);

    let scheduler = InferenceScheduler::new(
        config.local_model.clone(),
        config.prefer_local_model,
        config.llm.provider.clone(),
        config.llm.openai_base.clone(),
        config.llm.openai_key.clone(),
        config.llm.chat_model.clone(),
        config.llm.embedding_model.clone(),
        config.llm.embedding_enabled,
    )
    .with_provider_credential_version(config.llm.credential_version);
    let privacy_policy_path = privacy_policy_path();
    let privacy_policy = match std::fs::read_to_string(&privacy_policy_path) {
        Ok(text) => match openlife_core::privacy::PrivacyPolicy::from_yaml(&text) {
            Ok(policy) => {
                persistence.register_read_write("PrivacyPolicyStore");
                policy
            }
            Err(error) => {
                persistence.register_unavailable(
                    "PrivacyPolicyStore",
                    "privacy_policy_parse_failed",
                    &error.to_string(),
                );
                openlife_core::privacy::PrivacyPolicy::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            persistence.register_read_write("PrivacyPolicyStore");
            openlife_core::privacy::PrivacyPolicy::default()
        }
        Err(error) => {
            persistence.register_unavailable(
                "PrivacyPolicyStore",
                "privacy_policy_read_failed",
                &error.to_string(),
            );
            openlife_core::privacy::PrivacyPolicy::default()
        }
    };
    let privacy_engine = PrivacyEngine::with_policy(privacy_policy);
    let version_manager = VersionManager::new(data_dir.join("life-model").join("versions"));
    let audit_keyring_path = data_dir.join("mcp_audit_keys.json");
    let audit_key_hydration = hydrate_or_create_mcp_audit_keys(
        load_mcp_audit_keyring_from_path(&audit_keyring_path),
        secret_store,
    );
    let audit_key_hydration = match audit_key_hydration {
        Ok(hydration) => {
            persistence.register_read_write("McpAuditKeyReferenceStore");
            Some(hydration)
        }
        Err(error) => {
            persistence.register_unavailable(
                "McpAuditKeyReferenceStore",
                "mcp_audit_key_hydration_failed",
                &error.to_string(),
            );
            startup_warnings.borrow_mut().push(format!(
                "MCP audit key material is unavailable; audit reads are unknown and all effects are disabled: {error}"
            ));
            None
        }
    };
    if audit_key_hydration
        .as_ref()
        .is_some_and(|hydration| hydration.config_changed)
    {
        if let Err(error) = save_mcp_audit_keyring_to_path(
            &audit_keyring_path,
            &audit_key_hydration.as_ref().expect("checked Some").configs,
        ) {
            persistence.register_unavailable(
                "McpAuditKeyReferenceStore",
                "mcp_audit_key_reference_persistence_failed",
                &error.to_string(),
            );
            startup_warnings.borrow_mut().push(format!(
                "MCP audit key reference persistence failed; effects disabled: {error}"
            ));
        }
    }
    let mcp_audit_db_path = data_dir.join("mcp_audit.db");
    let audit_materials = audit_key_hydration
        .map(|hydration| hydration.materials)
        .unwrap_or_default();
    let mcp_audit_store = init_store(
        || {
            McpAuditStore::with_key_materials(&mcp_audit_db_path, audit_materials.clone())
                .map_err(|error| error.to_string())
        },
        || {
            McpAuditStore::open_read_only_existing_with_key_materials(
                &mcp_audit_db_path,
                audit_materials.clone(),
            )
            .map_err(|error| error.to_string())
        },
        || {
            McpAuditStore::with_key_materials(
                recovery_db_path("mcp_audit.db"),
                audit_materials.clone(),
            )
            .map_err(|error| error.to_string())
        },
        "McpAuditStore",
        &startup_warnings,
        &persistence,
    );
    let mcp_audit_store =
        required_store_or_unavailable(mcp_audit_store, "McpAuditStore", &startup_warnings, || {
            Ok(McpAuditStore::unavailable_sentinel(
                "canonical and read-only audit store open failed",
            ))
        });

    let hot_cache: SharedHotCache = {
        let initial_cache = match life_model_manager.load() {
            Ok(model) => HotMemoryCache::from_life_model(&model),
            Err(_) => HotMemoryCache::default(),
        };
        Arc::new(tokio::sync::RwLock::new(initial_cache))
    };

    #[cfg(feature = "dev-extensions")]
    let mcp_registry = {
        let mut registry = McpRegistry::new();
        registry.register_dev_a2a_tool();
        registry
    };
    #[cfg(not(feature = "dev-extensions"))]
    let mcp_registry = McpRegistry::new_release_product();
    let tool_permission_store = init_store(
        || {
            openlife_core::tool_permissions::ToolPermissionStore::new(
                data_dir.join("tool_permissions.db"),
            )
            .map_err(|e| e.to_string())
        },
        || {
            openlife_core::tool_permissions::ToolPermissionStore::open_read_only_existing(
                data_dir.join("tool_permissions.db"),
            )
            .map_err(|e| e.to_string())
        },
        || {
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory()
                .map_err(|e| e.to_string())
        },
        "ToolPermissionStore",
        &startup_warnings,
        &persistence,
    );
    let tool_permission_store = required_store_or_unavailable(
        tool_permission_store,
        "ToolPermissionStore",
        &startup_warnings,
        || {
            openlife_core::tool_permissions::ToolPermissionStore::unavailable_sentinel()
                .map_err(|error| error.to_string())
        },
    );
    let legacy_scheduled_task_report = std::cell::RefCell::new(None);
    let legacy_scheduled_task_path = data_dir.join("scheduled_tasks.json");
    let scheduled_task_store = init_store(
        || {
            let authority_key = task_store_authority_key
                .as_ref()
                .ok_or_else(|| "task_store_authority_key_unavailable".to_string())?;
            let store = openlife_core::tasks::TaskStore::new_with_authority_key(
                &task_store_db_path,
                authority_key,
            )
            .map_err(|e| e.to_string())?;
            let report = store
                .migrate_legacy_json_if_present(&legacy_scheduled_task_path)
                .map_err(|e| e.to_string())?;
            *legacy_scheduled_task_report.borrow_mut() = Some(report);
            Ok(store)
        },
        || {
            let authority_key = task_store_authority_key
                .as_ref()
                .ok_or_else(|| "task_store_authority_key_unavailable".to_string())?;
            openlife_core::tasks::TaskStore::open_read_only_existing_with_authority_key(
                &task_store_db_path,
                authority_key,
            )
            .map_err(|e| e.to_string())
        },
        || openlife_core::tasks::TaskStore::new_in_memory().map_err(|e| e.to_string()),
        "TaskStore",
        &startup_warnings,
        &persistence,
    );
    if legacy_scheduled_task_path.exists() {
        persistence.register_unavailable(
            "LegacyScheduledTaskOwner",
            "legacy_scheduled_task_quarantine_incomplete",
            "scheduled_tasks.json remains active because its metadata quarantine or atomic evidence retirement did not complete",
        );
        startup_warnings.borrow_mut().push(
            "Legacy scheduled-task state is unresolved/unknown; all effects remain disabled until scheduled_tasks.json is quarantined."
                .into(),
        );
    } else if let Some(report) = legacy_scheduled_task_report.into_inner() {
        if report.quarantined_count > 0 {
            log::warn!(
                "[startup] quarantined legacy scheduled-task source digest={} items={} unknown={}",
                report.source_digest.as_deref().unwrap_or("unknown"),
                report.item_count,
                report.quarantined_count,
            );
            startup_warnings.borrow_mut().push(format!(
                "Quarantined {} legacy scheduled-task record(s) as unknown; no legacy task was auto-executed.",
                report.quarantined_count
            ));
        }
        if report.historical_count > 0 {
            startup_warnings.borrow_mut().push(format!(
                "Imported {} legacy scheduled-task terminal label(s) as metadata-only history; they are not canonical completion receipts.",
                report.historical_count
            ));
        }
        if report.review_required_count > 0 {
            startup_warnings.borrow_mut().push(format!(
                "Identified {} future legacy scheduled task(s) requiring fresh Review Center review; none is executable before approval.",
                report.review_required_count
            ));
        }
    }
    let scheduled_task_store =
        required_store_or_unavailable(scheduled_task_store, "TaskStore", &startup_warnings, || {
            openlife_core::tasks::TaskStore::unavailable_sentinel()
                .map_err(|error| error.to_string())
        });
    let pending_reviewed_cloud_tasks = scheduled_task_store
        .list_tasks(Some("pending"))
        .unwrap_or_default()
        .into_iter()
        .filter(|task| {
            task.provider_grant.data_route == openlife_core::llm::ProviderDataRoute::PolicyAllowed
        })
        .collect::<Vec<_>>();
    for task in pending_reviewed_cloud_tasks {
        let restored = proposal_store
            .as_ref()
            .ok_or_else(|| "ProposalStore is unavailable".to_string())
            .and_then(|store| {
                let proof = ReviewWorkflow::new(store)
                    .materialized_acceptance_snapshot(&task.id)
                    .map_err(|error| error.to_string())?;
                scheduled_task_store
                    .restore_reviewed_cloud_authority(&proof)
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = restored {
            match scheduled_task_store.quarantine_unproven_reviewed_cloud_task(&task.id) {
                Ok(true) => log::warn!(
                    "[startup] scheduled cloud task {} lacks canonical ReviewWorkflow authority and now requires fresh review: {}",
                    task.id,
                    error
                ),
                Ok(false) | Err(_) => {
                    persistence.register_unavailable(
                        "ScheduledCloudAuthority",
                        "scheduled_cloud_authority_quarantine_failed",
                        &error,
                    );
                    startup_warnings.borrow_mut().push(format!(
                        "Scheduled cloud task {} could not prove ReviewWorkflow authority or enter fresh-review quarantine; all effects remain disabled: {}",
                        task.id, error
                    ));
                }
            }
        }
    }
    match proposal_store.as_ref() {
        Some(store) => match stage_legacy_scheduled_task_review_proposals(
            &scheduled_task_store,
            store,
            &data_dir,
        ) {
            Ok(staged) if staged > 0 => log::warn!(
                "[startup] staged {} legacy future scheduled task(s) for fresh review",
                staged
            ),
            Ok(_) => {}
            Err(error) => {
                persistence.register_unavailable(
                    "LegacyScheduledTaskReviewMigration",
                    "legacy_scheduled_review_staging_failed",
                    &error,
                );
                startup_warnings.borrow_mut().push(format!(
                    "Legacy future scheduled-task review staging failed; all effects remain disabled: {error}"
                ));
            }
        },
        None => match scheduled_task_store.pending_legacy_review_candidates(&data_dir) {
            Ok(candidates) if candidates.is_empty() => {}
            Ok(_) | Err(_) => {
                persistence.register_unavailable(
                    "LegacyScheduledTaskReviewMigration",
                    "proposal_store_unavailable_for_legacy_review",
                    "future legacy scheduled tasks require ReviewWorkflow but ProposalStore is unavailable",
                );
                startup_warnings.borrow_mut().push(
                    "Legacy future scheduled tasks await ReviewWorkflow, but ProposalStore is unavailable; all effects remain disabled."
                        .into(),
                );
            }
        },
    }

    let builder_session_store = BuilderSessionStore::new(data_dir.join("builder_sessions.json"));
    match builder_session_store.list_unfinished_sessions() {
        Ok(_) => persistence.register_read_write("BuilderSessionStore"),
        Err(error) => persistence.register_unavailable(
            "BuilderSessionStore",
            "builder_session_store_read_failed",
            &error.to_string(),
        ),
    }

    let mut plugin_registry = openlife_core::plugins::PluginRegistry::new(data_dir.join("plugins"));
    match plugin_registry.reload() {
        Ok(_) => persistence.register_read_write("PluginRegistry"),
        Err(e) => {
            persistence.register_unavailable(
                "PluginRegistry",
                "plugin_registry_reload_failed",
                &e.to_string(),
            );
            startup_warnings
                .borrow_mut()
                .push(format!("plugins manifest reload failed: {}", e));
        }
    }
    // Plugin manifests remain inspectable through PluginRegistry, but their skills are not
    // product-selectable until ToolGateway owns a reviewed plugin executor contract.
    let skill_registry = openlife_core::skills::SkillRegistry::built_in();

    let rollout_metrics_store = {
        let store_path = data_dir.join("rollout_metrics.db");
        match openlife_core::agent::RolloutMetricsStore::new(&store_path) {
            Ok(store) => Some(Arc::new(Mutex::new(store))),
            Err(e) => {
                startup_warnings
                    .borrow_mut()
                    .push(format!("rollout_metrics.db 初始化失败: {}", e));
                None
            }
        }
    };

    let resource_runtime = {
        let store_path = data_dir.join("resources.db");
        let runtime = openlife_core::resource::ResourceStore::new(&store_path).and_then(|store| {
            let parser =
                openlife_core::resource_gateway::ResourceParserProcess::for_current_executable()?;
            Ok(crate::resource_commands::ResourceRuntime::new(
                openlife_core::resource_gateway::ResourceGateway::new(store, parser),
            ))
        });
        match runtime {
            Ok(runtime) => {
                persistence.register_read_write("ResourceStore");
                Some(Arc::new(runtime))
            }
            Err(error) => {
                persistence.register_unavailable(
                    "ResourceStore",
                    "resource_runtime_initialization_failed",
                    &error.to_string(),
                );
                startup_warnings
                    .borrow_mut()
                    .push(format!("resources.db 初始化失败: {error}"));
                None
            }
        }
    };

    let state_store = {
        let store_path = data_dir.join("state.db");
        match openlife_core::state_store::StateStore::new(&store_path) {
            Ok(store) => {
                persistence.register_read_write("StateStore");
                Some(Arc::new(store))
            }
            Err(error) => {
                persistence.register_unavailable(
                    "StateStore",
                    "state_store_initialization_failed",
                    &error.to_string(),
                );
                startup_warnings.borrow_mut().push(format!(
                    "state.db 初始化失败；transient-state 写入已禁用且不会降级到临时存储：{error}"
                ));
                None
            }
        }
    };

    let app_state = Arc::new(AppState {
        persistence_coordinator: Arc::clone(&persistence),
        config: Arc::new(Mutex::new(config)),
        life_model_manager: Arc::new(Mutex::new(life_model_manager)),
        life_model_write_coordinator: Arc::new(Mutex::new(())),
        memory_store: Arc::new(Mutex::new(memory_store)),
        mcp_registry: Arc::new(Mutex::new(mcp_registry)),
        scheduler: Arc::new(Mutex::new(scheduler)),
        privacy_engine: Arc::new(Mutex::new(privacy_engine)),
        version_manager: Arc::new(Mutex::new(version_manager)),
        feedback_store: Arc::new(Mutex::new(feedback_store)),
        vector_store: Arc::new(Mutex::new(vector_store)),
        vector_persistence_mode: crate::state::VectorPersistenceMode::Enabled,
        builder_session_store: Arc::new(Mutex::new(builder_session_store)),
        a2a_sidecar: Arc::new(Mutex::new(a2a_sidecar::A2ASidecar::new(
            crate::a2a_server::configured_a2a_port(),
        ))),
        last_snapshot_date: Arc::new(Mutex::new(None)),
        mcp_audit_store: Arc::new(Mutex::new(mcp_audit_store)),
        agent_run_store: agent_run_store.map(|store| Arc::new(Mutex::new(store))),
        evidence_store: Arc::new(Mutex::new(evidence_store)),
        life_event_store: life_event_store.map(|store| Arc::new(Mutex::new(store))),
        heuristic_store: Arc::new(Mutex::new(heuristic_store)),
        policy_store: Arc::new(policy_store),
        proposal_store: proposal_store.map(|store| Arc::new(Mutex::new(store))),
        memory_lifecycle_store: memory_lifecycle_store.map(|store| Arc::new(Mutex::new(store))),
        plan_execute_session_store: plan_execute_session_store
            .map(|store| Arc::new(Mutex::new(store))),
        main_chat_agent_session_store: main_chat_agent_session_store
            .map(|store| Arc::new(Mutex::new(store))),
        main_chat_action_queue_store: main_chat_action_queue_store
            .map(|store| Arc::new(Mutex::new(store))),
        main_chat_agent_event_store: main_chat_agent_event_store
            .map(|store| Arc::new(Mutex::new(store))),
        main_chat_selected_skill_ids: Arc::new(Mutex::new(std::collections::HashMap::new())),
        main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
        patch_store: patch_store.map(|store| Arc::new(Mutex::new(store))),
        rollout_metrics_store,
        tool_permission_store: Arc::new(Mutex::new(tool_permission_store)),
        skill_registry: Arc::new(Mutex::new(skill_registry)),
        plugin_registry: Arc::new(Mutex::new(plugin_registry)),
        hot_cache,
        startup_warnings: startup_warnings.into_inner(),
        provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
        scheduled_task_store: Arc::new(scheduled_task_store),
        runtime_clock_source: Arc::new(tokio::sync::Mutex::new(
            crate::main_chat_runtime_facts::MainChatRuntimeClockSource::default(),
        )),
        web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
        resource_runtime,
        state_store,
        shutdown_notify: Arc::new(tokio::sync::Notify::new()),
    });

    BootstrapResult { state: app_state }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{
        EvidenceQuery, HeuristicQuery, BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
    };

    #[test]
    fn future_legacy_schedule_is_staged_once_through_review_workflow() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        std::fs::write(
            &legacy_path,
            serde_json::to_vec(&serde_json::json!([{
                "id": "legacy-future-review",
                "title": "Review this future task",
                "prompt": "prepare a bounded future review",
                "scheduled_at": (chrono::Utc::now() + chrono::Duration::days(10)).to_rfc3339(),
                "status": "pending",
                "created_at": chrono::Utc::now().to_rfc3339(),
                "action_type": "scheduled_task"
            }]))
            .unwrap(),
        )
        .unwrap();
        let task_store =
            openlife_core::tasks::TaskStore::new(directory.path().join("tasks.db")).unwrap();
        let proposal_store = ProposalStore::new_in_memory().unwrap();
        let report = task_store
            .migrate_legacy_json_if_present(&legacy_path)
            .unwrap();
        assert_eq!(report.review_required_count, 1);

        assert_eq!(
            stage_legacy_scheduled_task_review_proposals(
                &task_store,
                &proposal_store,
                directory.path(),
            )
            .unwrap(),
            1
        );
        assert_eq!(proposal_store.pending_count().unwrap(), 1);
        assert!(task_store.list_tasks(None).unwrap().is_empty());
        assert!(task_store
            .pending_legacy_review_candidates(directory.path())
            .unwrap()
            .is_empty());
        assert_eq!(
            stage_legacy_scheduled_task_review_proposals(
                &task_store,
                &proposal_store,
                directory.path(),
            )
            .unwrap(),
            0
        );
        assert_eq!(proposal_store.pending_count().unwrap(), 1);
    }

    #[test]
    fn legacy_review_restart_after_proposal_commit_before_task_journal_mark_is_exact() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        let task_db = directory.path().join("tasks.db");
        let proposal_db = directory.path().join("proposals.db");
        std::fs::write(
            &legacy_path,
            serde_json::to_vec(&serde_json::json!([{
                "id": "legacy-crash-window",
                "title": "Recover exact staged review",
                "prompt": "proposal commit must survive before migration journal mark",
                "scheduled_at": (chrono::Utc::now() + chrono::Duration::days(10)).to_rfc3339(),
                "status": "pending",
                "created_at": chrono::Utc::now().to_rfc3339(),
                "action_type": "scheduled_task"
            }]))
            .unwrap(),
        )
        .unwrap();

        let committed_proposal = {
            let task_store = openlife_core::tasks::TaskStore::new(&task_db).unwrap();
            task_store
                .migrate_legacy_json_if_present(&legacy_path)
                .unwrap();
            let candidate = task_store
                .pending_legacy_review_candidates(directory.path())
                .unwrap()
                .remove(0);
            let (proposal, proposal_id) =
                build_legacy_scheduled_task_review_proposal(&candidate).unwrap();
            let proposal_store = ProposalStore::new(&proposal_db).unwrap();
            let outcome = ReviewWorkflow::new(&proposal_store)
                .submit(
                    DurableWriteRequest::from_agent_proposal(
                        DurableWriteSource::ManualOverride,
                        DurableWriteSubject::Calendar,
                        proposal,
                        "A legacy future scheduled task is pending fresh Review Center approval; it has not been scheduled or executed.",
                    )
                    .with_existing_proposal_id(Some(proposal_id.clone()))
                    .with_idempotency_key(format!(
                        "legacy_scheduled_review:{}:{}",
                        candidate.source_digest, candidate.source_ordinal
                    )),
                )
                .unwrap();
            assert_eq!(outcome.proposal_id(), proposal_id);
            assert_eq!(
                task_store
                    .pending_legacy_review_candidates(directory.path())
                    .unwrap()
                    .len(),
                1,
                "simulate a crash before TaskStore marks the Proposal id"
            );
            outcome.proposal
        };

        let task_store = openlife_core::tasks::TaskStore::new(&task_db).unwrap();
        let proposal_store = ProposalStore::new(&proposal_db).unwrap();
        assert_eq!(
            stage_legacy_scheduled_task_review_proposals(
                &task_store,
                &proposal_store,
                directory.path(),
            )
            .unwrap(),
            1
        );
        let recovered = proposal_store
            .get_proposal(&committed_proposal.id)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.created_at, committed_proposal.created_at);
        assert_eq!(recovered.expires_at, committed_proposal.expires_at);
        assert!(task_store
            .pending_legacy_review_candidates(directory.path())
            .unwrap()
            .is_empty());
        assert_eq!(proposal_store.pending_count().unwrap(), 1);
    }

    #[test]
    fn identical_legacy_items_keep_distinct_ordinal_bound_review_identities() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        let item = serde_json::json!({
            "id": "legacy-duplicate-content",
            "title": "Review both retained rows",
            "prompt": "identical content still has two source ordinals",
            "scheduled_at": (chrono::Utc::now() + chrono::Duration::days(10)).to_rfc3339(),
            "status": "pending",
            "created_at": chrono::Utc::now().to_rfc3339(),
            "action_type": "scheduled_task"
        });
        std::fs::write(
            &legacy_path,
            serde_json::to_vec(&serde_json::json!([item.clone(), item])).unwrap(),
        )
        .unwrap();
        let task_store =
            openlife_core::tasks::TaskStore::new(directory.path().join("tasks.db")).unwrap();
        let proposal_store = ProposalStore::new_in_memory().unwrap();
        let report = task_store
            .migrate_legacy_json_if_present(&legacy_path)
            .unwrap();
        assert_eq!(report.review_required_count, 2);

        assert_eq!(
            stage_legacy_scheduled_task_review_proposals(
                &task_store,
                &proposal_store,
                directory.path(),
            )
            .unwrap(),
            2
        );
        assert_eq!(proposal_store.pending_count().unwrap(), 2);
        assert!(task_store
            .pending_legacy_review_candidates(directory.path())
            .unwrap()
            .is_empty());
        assert!(task_store.list_tasks(None).unwrap().is_empty());
    }

    #[test]
    fn legacy_review_staging_rejects_a_same_id_non_exact_proposal_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("scheduled_tasks.json");
        std::fs::write(
            &legacy_path,
            serde_json::to_vec(&serde_json::json!([{
                "id": "legacy-future-tamper",
                "title": "Review this future task",
                "prompt": "prepare the exact future review",
                "scheduled_at": (chrono::Utc::now() + chrono::Duration::days(10)).to_rfc3339(),
                "status": "pending",
                "created_at": chrono::Utc::now().to_rfc3339(),
                "action_type": "scheduled_task"
            }]))
            .unwrap(),
        )
        .unwrap();
        let task_store =
            openlife_core::tasks::TaskStore::new(directory.path().join("tasks.db")).unwrap();
        task_store
            .migrate_legacy_json_if_present(&legacy_path)
            .unwrap();
        let candidate = task_store
            .pending_legacy_review_candidates(directory.path())
            .unwrap()
            .remove(0);
        let identity_digest =
            openlife_core::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "schema": "openlife.legacyScheduledReviewIdentity.v1",
                "sourceDigest": candidate.source_digest.clone(),
                "sourceOrdinal": candidate.source_ordinal,
                "itemDigest": candidate.item_digest.clone(),
            }))
            .1;
        let identity_suffix = identity_digest.strip_prefix("sha256:").unwrap();
        let proposal_id = format!("legacy-scheduled-review-{identity_suffix}");
        let source_detail = format!(
            "legacy_scheduled_review:{}:{}:{}",
            candidate.source_digest, candidate.source_ordinal, candidate.item_digest
        );
        let proposal_store = ProposalStore::new_in_memory().unwrap();
        let mut rogue = AgentProposal::new(
            ProposalType::ScheduledTask,
            &format!("tasks.legacy_review.{}", &identity_suffix[..16]),
            serde_json::json!({
                "title": candidate.title,
                "description": "different payload under the same deterministic id",
                "due_date": candidate.due_at.clone(),
                "scheduled_at": candidate.due_at,
                "priority": candidate.priority,
                "tool": candidate.action_type,
            }),
            "A provably not-yet-due legacy scheduled task requires fresh Review Center approval before it can enter the canonical TaskStore.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        rogue.id = proposal_id;
        rogue.source_detail = Some(source_detail);
        proposal_store.create_proposal(&rogue).unwrap();

        let error = stage_legacy_scheduled_task_review_proposals(
            &task_store,
            &proposal_store,
            directory.path(),
        )
        .unwrap_err();
        assert!(error.contains("non-exact snapshot"));
        assert_eq!(
            task_store
                .pending_legacy_review_candidates(directory.path())
                .unwrap()
                .len(),
            1
        );
        assert!(task_store.list_tasks(None).unwrap().is_empty());
    }

    #[derive(Default)]
    struct TestSecretStore {
        values: std::sync::Mutex<std::collections::HashMap<String, String>>,
    }

    impl SecretStore for TestSecretStore {
        fn get(&self, secret_ref: &str) -> anyhow::Result<Option<String>> {
            Ok(self.values.lock().unwrap().get(secret_ref).cloned())
        }

        fn set(&self, secret_ref: &str, value: &str) -> anyhow::Result<()> {
            self.values
                .lock()
                .unwrap()
                .insert(secret_ref.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, secret_ref: &str) -> anyhow::Result<()> {
            self.values.lock().unwrap().remove(secret_ref);
            Ok(())
        }
    }

    async fn seed_failed_main_chat_owner_fixture(
        state: &Arc<AppState>,
        chat_session_id: &str,
    ) -> (
        openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
        String,
    ) {
        use openlife_core::agent::main_chat_agent_v1::{
            AgentTaskSessionDraft, MainChatAgentStrategy,
        };
        let session = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("task store")
                .lock()
                .await;
            let session = store
                .create_session(AgentTaskSessionDraft {
                    chat_session_id: chat_session_id.into(),
                    user_goal: "Exercise restart terminalization recovery.".into(),
                    selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                    current_plan_summary: None,
                    context_snapshot_refs: vec![],
                })
                .expect("create restart task");
            store
                .fail_session(&session.id, "Fixture awaits replay.")
                .expect("mark restart task failed")
        };
        let mut run = openlife_core::agent::AgentRun::new_chat_run(
            &session.chat_session_id,
            &session.user_goal,
        );
        run.task_id = session.id.clone();
        run.status = openlife_core::agent::AgentRunStatus::Failed;
        run.finished_at = Some(chrono::Utc::now());
        let run_id = run.id.clone();
        state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await
            .create_run(&run)
            .expect("create restart AgentRun");
        (session, run_id)
    }

    #[tokio::test]
    async fn bootstrap_initializes_hs_stores_and_seeds_mvp_heuristics() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let result =
            bootstrap_with_secret_store(temp_dir.path().to_path_buf(), &TestSecretStore::default());

        result
            .state
            .evidence_store
            .lock()
            .await
            .query(EvidenceQuery::default())
            .unwrap();

        let heuristic_store = result.state.heuristic_store.lock().await;
        let heuristics = heuristic_store
            .query(HeuristicQuery {
                domain: Some("planning".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(heuristics
            .iter()
            .any(|heuristic| heuristic.id == BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING));
        assert!(result
            .state
            .policy_store
            .is_hard_policy_id(openlife_core::agent::BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY));
        assert!(result
            .state
            .persistence_coordinator
            .startup_reconciliation_mutations_safe());
        assert_eq!(
            reconcile_startup_orphaned_main_chat_runs(&result.state)
                .await
                .expect("empty orphan reconciliation must succeed"),
            0
        );
        assert!(!reconcile_startup_proposal_projections(&result.state)
            .await
            .expect("empty startup Proposal reconciliation must succeed"));
        reconcile_startup_canonical_outboxes(&result.state)
            .await
            .expect("empty canonical outbox reconciliation must succeed");
        result.state.persistence_coordinator.seal();

        let initial_health = result.state.persistence_coordinator.snapshot();
        assert_eq!(
            initial_health.mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadWrite
        );
        let run_count_before = result
            .state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .run_count()
            .unwrap();
        result.state.persistence_coordinator.register_unavailable(
            "MemoryStore",
            "injected_runtime_disk_failure",
            "injected read-only disk",
        );
        let send_error = crate::main_chat_send::send_message_with_state(
            "persistence-fault-turn".into(),
            vec![openlife_core::llm::ChatMessage {
                role: "user".into(),
                content: "This must never reach a provider or tool.".into(),
            }],
            None,
            &result.state,
        )
        .await
        .expect_err("runtime persistence degradation must stop the turn before dispatch");
        assert!(send_error.contains(crate::persistence_coordinator::PERSISTENCE_EFFECTS_BLOCKED));
        assert!(crate::memory_gateway::create_chat_session_with_state(
            "must-not-write",
            "must-not-write",
            &result.state,
        )
        .await
        .is_err());
        let run_count_after = result
            .state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .run_count()
            .unwrap();
        assert_eq!(run_count_after, run_count_before);
        let projection =
            crate::life_state_projection::get_life_state_projection_with_state(&result.state)
                .await
                .expect("degraded read model remains observable");
        assert_eq!(
            projection.persistence.mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        );
        assert!(projection.safe_mode.active);
        assert_eq!(projection.readiness.database_status, "degraded");
    }

    #[test]
    fn bootstrap_injects_stable_agent_run_key_and_rejects_key_mismatch() {
        use base64::Engine as _;

        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let first = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        assert!(first.state.agent_run_store.is_some());
        let first_key = secrets
            .get(AGENT_RUN_RECEIPT_KEY_REF)
            .unwrap()
            .expect("bootstrap creates purpose-isolated AgentRun key");
        drop(first);

        let restarted = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        assert!(restarted.state.agent_run_store.is_some());
        assert_eq!(
            secrets.get(AGENT_RUN_RECEIPT_KEY_REF).unwrap().as_deref(),
            Some(first_key.as_str())
        );
        drop(restarted);

        secrets
            .set(
                AGENT_RUN_RECEIPT_KEY_REF,
                &base64::engine::general_purpose::STANDARD.encode([0x5A_u8; 32]),
            )
            .unwrap();
        let mismatched = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        assert!(
            mismatched.state.agent_run_store.is_none(),
            "a different key must not reinterpret existing AgentRun receipts"
        );
        assert!(mismatched
            .state
            .startup_warnings
            .iter()
            .any(|warning| warning.contains("agent_run_receipt_key_mismatch")));
    }

    #[tokio::test]
    async fn restart_reconciles_orphaned_running_replay_to_exact_durable_failed_terminal() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let first = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        let (session, run_id) =
            seed_failed_main_chat_owner_fixture(&first.state, "restart-orphan-owner").await;
        {
            let store = first
                .state
                .main_chat_agent_session_store
                .as_ref()
                .expect("task store")
                .lock()
                .await;
            store
                .resume_session(&session.id)
                .expect("simulate a replay active when the process stopped");
        }
        {
            let store = first
                .state
                .agent_run_store
                .as_ref()
                .expect("run store")
                .lock()
                .await;
            let mut run = store
                .get_run(&run_id)
                .expect("load replay run")
                .expect("replay run exists");
            run.status = openlife_core::agent::AgentRunStatus::Running;
            run.finished_at = None;
            store
                .update_run(&run)
                .expect("persist an orphaned running replay without a terminal receipt");
        }
        drop(first);

        let restarted = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        let before = restarted
            .state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await
            .get_run(&run_id)
            .expect("load orphan before recovery")
            .expect("orphan run exists");
        assert_eq!(before.status, openlife_core::agent::AgentRunStatus::Running);
        assert_eq!(
            reconcile_startup_orphaned_main_chat_runs(&restarted.state)
                .await
                .expect("startup orphan recovery succeeds"),
            1
        );
        restarted.state.persistence_coordinator.seal();

        let events = restarted
            .state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .list(&session.id, 0, 100)
            .expect("list restart recovery events");
        let terminal = events
            .iter()
            .find(|event| event.event_type == "failed")
            .expect("restart recovery persists failed first");
        assert_eq!(terminal.run_id, run_id);
        assert_eq!(terminal.task_session_id, session.id);
        assert_eq!(terminal.source, "bootstrap.orphan_running_process_restart");
        assert!(events.iter().all(|event| !matches!(
            event.event_type.as_str(),
            "cancel_requested" | "local_aborted" | "interrupted"
        )));
        let recovered_run = restarted
            .state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await
            .get_run(&run_id)
            .expect("load recovered run")
            .expect("recovered run exists");
        assert_eq!(
            recovered_run.status,
            openlife_core::agent::AgentRunStatus::Failed
        );
        let recovered_task = restarted
            .state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await
            .load_session(&session.id)
            .expect("load recovered task")
            .expect("recovered task exists");
        assert_eq!(
            recovered_task.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
        );
        restarted
            .state
            .persistence_coordinator
            .require_effects_allowed()
            .expect("effects become available only after orphan terminal is durable");
    }

    #[tokio::test]
    async fn restart_repairs_both_directions_of_partial_terminal_projection() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let first = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        let (run_terminal_task_running, run_terminal_id) =
            seed_failed_main_chat_owner_fixture(&first.state, "restart-run-terminal").await;
        let (run_running_task_terminal, task_terminal_run_id) =
            seed_failed_main_chat_owner_fixture(&first.state, "restart-task-terminal").await;

        for (task_id, run_id) in [
            (&run_terminal_task_running.id, &run_terminal_id),
            (&run_running_task_terminal.id, &task_terminal_run_id),
        ] {
            crate::main_chat_runtime_support::finalize_main_chat_task_failure(
                &first.state,
                Some(run_id),
                Some(task_id),
                crate::main_chat_runtime_support::MainChatTaskFailureKind::UnknownError,
                "Seed an exact durable terminal before injecting a projection fault.",
                "bootstrap.test.partial_projection_seed",
            )
            .await
            .expect("seed durable failed terminal");
        }
        first
            .state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await
            .resume_session(&run_terminal_task_running.id)
            .expect("inject task projection left Running");
        {
            let store = first
                .state
                .agent_run_store
                .as_ref()
                .expect("run store")
                .lock()
                .await;
            let mut run = store
                .get_run(&task_terminal_run_id)
                .expect("load run projection")
                .expect("run projection exists");
            run.status = openlife_core::agent::AgentRunStatus::Running;
            run.finished_at = None;
            store
                .update_run(&run)
                .expect("inject run projection left Running");
        }
        drop(first);

        let restarted = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        assert_eq!(
            reconcile_startup_orphaned_main_chat_runs(&restarted.state)
                .await
                .expect("repair both partial terminal projections"),
            2
        );
        restarted.state.persistence_coordinator.seal();
        for (task_id, run_id) in [
            (&run_terminal_task_running.id, &run_terminal_id),
            (&run_running_task_terminal.id, &task_terminal_run_id),
        ] {
            let run = restarted
                .state
                .agent_run_store
                .as_ref()
                .expect("run store")
                .lock()
                .await
                .get_run(run_id)
                .expect("load repaired run")
                .expect("repaired run exists");
            assert_eq!(run.status, openlife_core::agent::AgentRunStatus::Failed);
            let task = restarted
                .state
                .main_chat_agent_session_store
                .as_ref()
                .expect("task store")
                .lock()
                .await
                .load_session(task_id)
                .expect("load repaired task")
                .expect("repaired task exists");
            assert_eq!(
                task.status,
                openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
            );
            let events = restarted
                .state
                .main_chat_agent_event_store
                .as_ref()
                .expect("event store")
                .lock()
                .await
                .list(task_id, 0, 100)
                .expect("list exact terminal events");
            assert_eq!(
                events
                    .iter()
                    .filter(|event| {
                        event.event_type == "failed" && event.run_id.as_str() == run_id.as_str()
                    })
                    .count(),
                1,
                "projection recovery must reuse the exact durable terminal"
            );
        }
    }

    #[tokio::test]
    async fn restart_materializes_pre_dispatch_failure_marker_before_enabling_effects() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let first = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        let (session, run_id) =
            seed_failed_main_chat_owner_fixture(&first.state, "restart-predispatch-marker").await;
        first
            .state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await
            .resume_session(&session.id)
            .expect("make task active before typed marker");
        {
            let store = first
                .state
                .agent_run_store
                .as_ref()
                .expect("run store")
                .lock()
                .await;
            let mut run = store
                .get_run(&run_id)
                .expect("load marker run")
                .expect("marker run exists");
            run.status = openlife_core::agent::AgentRunStatus::Running;
            run.finished_at = None;
            store.update_run(&run).expect("make marker run active");
        }
        let error_digest = openlife_core::agent::metadata_safe::metadata_safe_text_digest(
            "injected event store failure body must be digested",
        )
        .1;
        first
            .state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await
            .record_pre_dispatch_persistence_failure(&session.id, &run_id, &error_digest)
            .expect("commit typed marker + task failure atomically");
        let before_restart_run = first
            .state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await
            .get_run(&run_id)
            .expect("load crash-point run")
            .expect("crash-point run exists");
        assert_eq!(
            before_restart_run.status,
            openlife_core::agent::AgentRunStatus::Running,
            "crash point is after typed task transaction but before AgentRun projection"
        );
        assert!(first
            .state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .list(&session.id, 0, 100)
            .expect("list first-process events")
            .is_empty());
        drop(first);

        let restarted = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        assert!(restarted
            .state
            .persistence_coordinator
            .require_effects_allowed()
            .is_err());
        assert_eq!(
            reconcile_startup_orphaned_main_chat_runs(&restarted.state)
                .await
                .expect("materialize pre-dispatch marker"),
            1
        );
        restarted.state.persistence_coordinator.seal();
        let events = restarted
            .state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .list(&session.id, 0, 100)
            .expect("list recovered marker events");
        let terminal = events
            .iter()
            .find(|event| event.event_type == "failed")
            .expect("marker becomes a durable failed receipt");
        assert_eq!(terminal.run_id, run_id);
        assert_eq!(
            terminal.source,
            "bootstrap.pre_dispatch_event_store_failure_recovery"
        );
        restarted
            .state
            .persistence_coordinator
            .require_effects_allowed()
            .expect("effects enable only after marker becomes durable");
    }

    #[tokio::test]
    async fn restart_degrades_inconsistent_terminal_projections_without_receipt_or_marker() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let first = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        let (session, run_id) =
            seed_failed_main_chat_owner_fixture(&first.state, "restart-no-terminal-truth").await;
        {
            let store = first
                .state
                .agent_run_store
                .as_ref()
                .expect("run store")
                .lock()
                .await;
            let mut run = store
                .get_run(&run_id)
                .expect("load run")
                .expect("run exists");
            run.status = openlife_core::agent::AgentRunStatus::Completed;
            run.finished_at = Some(chrono::Utc::now());
            store.update_run(&run).expect("inject terminal mismatch");
        }
        drop(first);

        let restarted = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        let error = reconcile_startup_orphaned_main_chat_runs(&restarted.state)
            .await
            .expect_err("no receipt exists to choose between contradictory projections");
        assert!(error.contains("startup_projection_without_durable_terminal_inconsistent"));
        restarted.state.persistence_coordinator.seal();
        assert!(restarted
            .state
            .persistence_coordinator
            .require_effects_allowed()
            .is_err());
        assert!(restarted
            .state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .list(&session.id, 0, 100)
            .expect("list absent terminal events")
            .is_empty());
    }

    #[tokio::test]
    async fn restart_with_unwritable_terminal_store_blocks_all_effects_without_false_projection() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let first = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        let (session, run_id) =
            seed_failed_main_chat_owner_fixture(&first.state, "restart-orphan-unwritable").await;
        {
            let task_store = first
                .state
                .main_chat_agent_session_store
                .as_ref()
                .expect("task store")
                .lock()
                .await;
            task_store
                .resume_session(&session.id)
                .expect("leave task Running across restart");
        }
        {
            let run_store = first
                .state
                .agent_run_store
                .as_ref()
                .expect("run store")
                .lock()
                .await;
            let mut run = run_store
                .get_run(&run_id)
                .expect("load run")
                .expect("run exists");
            run.status = openlife_core::agent::AgentRunStatus::Running;
            run.finished_at = None;
            run_store.update_run(&run).expect("leave run Running");
        }
        first
            .state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .install_failed_insert_failure_for_test()
            .expect("persist failed-event fault across restart");
        drop(first);

        let restarted = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        let error = reconcile_startup_orphaned_main_chat_runs(&restarted.state)
            .await
            .expect_err("startup must fail closed while terminal receipt is unwritable");
        assert!(error.contains("persist failure terminal receipt"));
        restarted.state.persistence_coordinator.seal();
        let health = restarted.state.persistence_coordinator.snapshot();
        assert!(matches!(
            health.mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadOnlyDegraded
                | crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        ));
        assert!(!health.provider_dispatch_allowed);
        assert!(!health.tool_dispatch_allowed);
        assert!(!health.canonical_writes_allowed);
        let events = restarted
            .state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .list(&session.id, 0, 100)
            .expect("list unwritable recovery events");
        assert!(events.iter().all(|event| !matches!(
            event.event_type.as_str(),
            "failed"
                | "cancel_requested"
                | "local_aborted"
                | "interrupted"
                | "final_delivery.created"
        )));
        let run = restarted
            .state
            .agent_run_store
            .as_ref()
            .expect("run store")
            .lock()
            .await
            .get_run(&run_id)
            .expect("load unprojected run")
            .expect("run exists");
        assert_eq!(run.status, openlife_core::agent::AgentRunStatus::Running);
        let task = restarted
            .state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await
            .load_session(&session.id)
            .expect("load unprojected task")
            .expect("task exists");
        assert_eq!(
            task.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Running
        );
    }

    #[tokio::test]
    async fn irrecoverable_release_store_failure_starts_explicit_unavailable_degraded_state() {
        let directory = tempfile::tempdir().unwrap();
        let blocked_data_root = directory.path().join("not-a-directory");
        std::fs::write(&blocked_data_root, b"blocks child stores").unwrap();

        let result = bootstrap_with_secret_store(blocked_data_root, &TestSecretStore::default());
        result.state.persistence_coordinator.seal();
        let health = result.state.persistence_coordinator.snapshot();
        assert!(health.sealed);
        assert_eq!(
            health.mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        );
        assert!(!health.canonical_writes_allowed);
        assert!(!health.provider_dispatch_allowed);
        assert!(!health.tool_dispatch_allowed);
        assert!(result
            .state
            .memory_store
            .lock()
            .await
            .list_chat_sessions(1)
            .is_err());
        assert!(
            crate::memory_gateway::count_memory_chunks_with_state(&result.state)
                .await
                .is_err()
        );

        let projection =
            crate::life_state_projection::get_life_state_projection_with_state(&result.state)
                .await
                .expect("unavailable canonical stores must still produce a degraded read model");
        assert_eq!(projection.readiness.database_status, "degraded");
        assert!(projection.safe_mode.active);
        assert_eq!(projection.persistence.mode, health.mode);
    }

    #[cfg(not(feature = "dev-extensions"))]
    #[test]
    fn release_store_initialization_never_calls_ephemeral_fallback() {
        assert!(!ephemeral_store_fallback_allowed());
        let fallback_called = std::cell::Cell::new(false);
        let warnings = std::cell::RefCell::new(Vec::new());
        let persistence = PersistenceCoordinator::new();
        let result: Result<(), String> = init_store(
            || Err("primary durable database failed".into()),
            || Ok(()),
            || {
                fallback_called.set(true);
                Ok(())
            },
            "CanonicalStore",
            &warnings,
            &persistence,
        );

        assert!(result.is_ok());
        assert!(!fallback_called.get());
        persistence.seal();
        assert_eq!(
            persistence.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadOnlyDegraded
        );
        assert!(persistence.require_effects_allowed().is_err());
    }

    #[cfg(not(feature = "dev-extensions"))]
    #[test]
    fn writable_memory_open_failure_uses_the_real_read_only_canonical_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("memory.db");
        {
            let store = MemoryStore::new(&path).unwrap();
            store
                .save_message(
                    "degraded-read-session",
                    &openlife_core::llm::ChatMessage {
                        role: "user".into(),
                        content: "READ_ONLY_CANONICAL_SENTINEL".into(),
                    },
                )
                .unwrap();
        }
        let fallback_called = std::cell::Cell::new(false);
        let warnings = std::cell::RefCell::new(Vec::new());
        let persistence = PersistenceCoordinator::new();

        let store = init_store(
            || Err("simulated writable open failure".into()),
            || MemoryStore::open_read_only_existing(&path).map_err(|error| error.to_string()),
            || {
                fallback_called.set(true);
                MemoryStore::new_in_memory().map_err(|error| error.to_string())
            },
            "MemoryStore",
            &warnings,
            &persistence,
        )
        .expect("current canonical Memory database must remain readable");

        assert_eq!(
            store
                .load_recent_messages("degraded-read-session", 10)
                .unwrap()[0]
                .content,
            "READ_ONLY_CANONICAL_SENTINEL"
        );
        assert!(!fallback_called.get());
        persistence.seal();
        let health = persistence.snapshot();
        assert_eq!(
            health.mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadOnlyDegraded
        );
        assert!(!health.canonical_writes_allowed);
        assert!(!health.provider_dispatch_allowed);
        assert!(!health.tool_dispatch_allowed);
    }

    #[cfg(not(feature = "dev-extensions"))]
    #[test]
    fn canonical_memory_store_failure_is_returned_in_release_mode() {
        let directory = tempfile::TempDir::new().unwrap();
        let warnings = std::cell::RefCell::new(Vec::new());
        let error = match init_memory_store(directory.path(), &warnings) {
            Ok(_) => panic!("directory path must not initialize as a durable SQLite file"),
            Err(error) => error,
        };

        assert!(error.contains("memory.db durable initialization failed"));
        assert!(!error.contains("in_memory"));
    }
}
