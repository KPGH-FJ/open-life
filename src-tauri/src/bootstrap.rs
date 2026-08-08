//! Application bootstrap: store initialization and AppState assembly.
//! Extracted from lib.rs to keep the main entry point focused on Tauri lifecycle.

use crate::a2a_sidecar;
use crate::main_chat_event_stream::{MainChatAgentEventStore, MainChatEventDigestKey};
use crate::persistence_coordinator::PersistenceCoordinator;
#[cfg(test)]
use crate::secret_store::MCP_AUDIT_KEY_REF_PREFIX;
use crate::secret_store::{
    hydrate_config_secrets_read_only, inspect_and_hydrate_integrity_key,
    inspect_existing_mcp_audit_keys, selected_keyring_service_classification,
    IntegrityKeyHydration, McpAuditKeyHydrationInspection, ProviderCredentialHydrationStatus,
    SecretReader, StartupKeyringSecretStore, ACTION_QUEUE_AUTHORITY_KEY_REF,
    AGENT_RUN_RECEIPT_KEY_REF, MAIN_CHAT_EVENT_INTEGRITY_KEY_REF, TASK_STORE_AUTHORITY_KEY_REF,
};
use crate::state::{AppState, CredentialBootstrapSnapshot, CredentialBootstrapStatus};
use crate::storage::{load_mcp_audit_keyring_from_path, privacy_policy_path, McpAuditKeyringLoad};
use openlife_core::agent::{
    main_chat_agent_v1::{ActionQueueAuthorityKey, ActionQueueStore, AgentTaskSessionStore},
    reconcile_collaboration_guidance_authority, AgentProposal, AgentRunReceiptKey,
    CollaborationGuidanceCutoverStatus, DurableWriteRequest, DurableWriteSource,
    DurableWriteSubject, HSAssetAuthorityRegistry, MemoryLifecycleStore, PlanExecuteSessionStore,
    ProposalSource, ProposalStore, ProposalType, ReviewWorkflow, RiskLevel,
};
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

fn protected_paths_are_absent(data_dir: &Path, relative_paths: &[&str]) -> std::io::Result<bool> {
    for relative_path in relative_paths {
        match std::fs::symlink_metadata(data_dir.join(relative_path)) {
            Ok(_) => return Ok(false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

fn inspect_fixed_credential<R: SecretReader + ?Sized>(
    data_dir: &Path,
    secret_ref: &'static str,
    protected_paths: &[&str],
    store: &R,
) -> (CredentialBootstrapStatus, Option<[u8; 32]>) {
    match inspect_and_hydrate_integrity_key(secret_ref, store) {
        IntegrityKeyHydration::Available(key) => (CredentialBootstrapStatus::Available, Some(key)),
        IntegrityKeyHydration::Missing => {
            match protected_paths_are_absent(data_dir, protected_paths) {
                Ok(true) => (CredentialBootstrapStatus::InitializationRequired, None),
                Ok(false) => (CredentialBootstrapStatus::MissingExistingData, None),
                Err(_) => (CredentialBootstrapStatus::Unknown, None),
            }
        }
        IntegrityKeyHydration::Invalid => (CredentialBootstrapStatus::Invalid, None),
        IntegrityKeyHydration::Unavailable => (CredentialBootstrapStatus::Unavailable, None),
    }
}

const STARTUP_PROPOSAL_RECONCILIATION_BATCH: i64 = 200;
const STARTUP_PROPOSAL_RECONCILIATION_SYNC_PASSES: usize = 5;
const STARTUP_TERMINAL_OWNER_RECONCILIATION_BATCH: usize = 64;
const STARTUP_TERMINAL_OWNER_RECONCILIATION_PASSES: usize = 8;
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
            crate::terminal_owner_write_gateway::register_agent_run_store_result(
                state,
                store
                    .get_run_for_task_id(&task.id)
                    .map_err(|error| error.to_string()),
            )
            .map_err(|error| format!("load exact startup AgentRun failed: {error}"))?
            .ok_or_else(|| format!("startup_main_chat_agent_run_missing:{}", task.id))?
        };
        if run.task_id != task.id {
            return Err("startup_main_chat_agent_run_task_identity_mismatch".into());
        }
        if let Some(proposal_id) =
            startup_rejected_blocking_review_proposal(state, &run, &task).await?
        {
            crate::terminal_owner_write_gateway::TerminalOwnerWriteGateway::from_state(state)
                .await?
                .apply_blocking_review_rejection(&proposal_id)
                .await
                .map_err(|error| {
                    format!("apply startup blocking review rejection failed: {error}")
                })?;
            crate::terminal_owner_write_gateway::update_agent_run_after_startup_review_reconciliation(
                state,
                &proposal_id,
                &run.id,
            )
            .await
            .map_err(|error| {
                format!("project startup rejected-review AgentRun failed: {error}")
            })?;
            verify_startup_terminal_owner_successor_projection(state, &run.id, &task.id).await?;
            reconciled = reconciled.saturating_add(1);
            continue;
        }
        if crate::main_chat_turn_runtime::reconcile_orphaned_openlife_replay_epoch_after_restart(
            state, &task.id, &run.id,
        )
        .await?
        {
            verify_startup_terminal_projection(state, &run.id, &task.id).await?;
            reconciled = reconciled.saturating_add(1);
            continue;
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
        if let Some(proposal_id) = startup_terminal_owner_successor_proposal(
            state,
            &run,
            &task,
            lifecycle.lifecycle_event.as_ref(),
        )
        .await?
        {
            crate::terminal_owner_write_gateway::update_agent_run_after_startup_review_reconciliation(
                state,
                &proposal_id,
                &run.id,
            )
            .await
            .map_err(|error| {
                format!("project startup terminal-owner AgentRun successor failed: {error}")
            })?;
            verify_startup_terminal_owner_successor_projection(state, &run.id, &task.id).await?;
            reconciled = reconciled.saturating_add(1);
            continue;
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
                    finalize_main_chat_task_failure_after_durable_receipt_at_startup_reconciliation(
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
                crate::main_chat_runtime_support::finalize_main_chat_task_failure_at_startup_reconciliation(
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
                crate::main_chat_runtime_support::finalize_main_chat_task_failure_at_startup_reconciliation(
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
                crate::main_chat_runtime_support::finalize_main_chat_task_failure_at_startup_reconciliation(
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
                crate::terminal_owner_write_gateway::project_agent_run_from_startup_task_owner(
                    state,
                    &run.id,
                    &run.task_id,
                )
                .await
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
                crate::main_chat_runtime_support::finalize_main_chat_task_failure_at_startup_reconciliation(
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

async fn startup_rejected_blocking_review_proposal(
    state: &Arc<AppState>,
    run: &openlife_core::agent::AgentRun,
    task: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
) -> Result<Option<String>, String> {
    use openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus;
    use openlife_core::agent::{ProposalStatus, ProposalTerminalRelationKind};

    if task.status != AgentTaskSessionStatus::WaitingPermission {
        return Ok(None);
    }
    let proposal_ids = task
        .pending_blockers
        .iter()
        .filter_map(|blocker| blocker.strip_prefix("proposal:"))
        .filter(|proposal_id| !proposal_id.trim().is_empty())
        .collect::<Vec<_>>();
    if proposal_ids.is_empty() {
        return Ok(None);
    }

    let store_arc = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "startup_rejected_review_proposal_store_unavailable".to_string())?;
    let store = store_arc.lock().await;
    for proposal_id in proposal_ids {
        let proposal = store
            .get_proposal(proposal_id)
            .map_err(|error| format!("load startup rejected review failed: {error}"))?
            .ok_or_else(|| format!("startup_rejected_review_proposal_missing:{proposal_id}"))?;
        if proposal.status != ProposalStatus::Rejected {
            continue;
        }
        let origin = store
            .terminal_owner_origin_binding(proposal_id)
            .map_err(|error| format!("load startup rejected review origin failed: {error}"))?
            .ok_or_else(|| "startup_rejected_review_origin_missing".to_string())?;
        let relation = store
            .terminal_relation_projection_proof(proposal_id)
            .map_err(|error| format!("load startup rejected review relation failed: {error}"))?
            .ok_or_else(|| "startup_rejected_review_relation_missing".to_string())?;
        if origin.task_session_id() != task.id
            || origin.run_id() != run.id
            || relation.task_session_id() != task.id
            || relation.run_id() != run.id
        {
            return Err("startup_rejected_review_identity_mismatch".into());
        }
        if matches!(
            relation.relation_kind(),
            ProposalTerminalRelationKind::EffectBlockingPrerequisite
                | ProposalTerminalRelationKind::ActionResumePrerequisite
        ) {
            return Ok(Some(proposal_id.to_string()));
        }
        return Err("startup_rejected_review_relation_not_blocking".into());
    }
    Ok(None)
}

async fn startup_terminal_owner_successor_proposal(
    state: &Arc<AppState>,
    run: &openlife_core::agent::AgentRun,
    task: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    lifecycle_event: Option<&crate::main_chat_event_stream::MainChatAgentDurableEvent>,
) -> Result<Option<String>, String> {
    let Some(final_event) = lifecycle_event.filter(|event| {
        event.event_type == "final_delivery.created"
            && event
                .payload
                .get("status")
                .and_then(serde_json::Value::as_str)
                == Some("completed_with_pending_items")
    }) else {
        return Ok(None);
    };
    let successor = {
        let store_arc = state
            .main_chat_agent_event_store
            .as_ref()
            .ok_or_else(|| "startup_orphan_event_store_unavailable".to_string())?;
        let store = store_arc.lock().await;
        let post_final = store
            .list(&task.id, final_event.sequence, 2)
            .map_err(|error| format!("load startup terminal-owner successor failed: {error}"))?;
        match post_final.as_slice() {
            [] => return Ok(None),
            [successor] => successor.clone(),
            _ => return Err("startup_terminal_owner_successor_window_invalid".into()),
        }
    };
    if successor.event_type != "terminal_owner.successor_confirmed"
        || successor.object_type != "terminal_owner_successor"
        || successor.source != "terminal_owner_write_gateway.review_successor"
        || successor.task_session_id != task.id
        || successor.run_id != run.id
        || successor
            .payload
            .get("finalEventId")
            .and_then(serde_json::Value::as_str)
            != Some(final_event.event_id.as_str())
        || successor
            .payload
            .get("ownerKind")
            .and_then(serde_json::Value::as_str)
            != Some("agent_task_session")
        || successor
            .payload
            .get("ownerId")
            .and_then(serde_json::Value::as_str)
            != Some(task.id.as_str())
        || !matches!(
            successor
                .payload
                .get("causeKind")
                .and_then(serde_json::Value::as_str),
            Some("proposal_review_acceptance" | "proposal_review_rejection")
        )
    {
        return Err("startup_terminal_owner_successor_identity_invalid".into());
    }
    let proposal_id = successor
        .payload
        .get("causeRef")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "startup_terminal_owner_successor_proposal_missing".to_string())?;
    let receipt_ref = successor
        .payload
        .get("localTransitionReceiptRef")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "startup_terminal_owner_successor_receipt_ref_missing".to_string())?;
    let receipt_digest = successor
        .payload
        .get("localTransitionReceiptDigest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "startup_terminal_owner_successor_receipt_digest_missing".to_string())?;
    let before_revision = successor
        .payload
        .get("beforeOwnerRevision")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "startup_terminal_owner_successor_before_revision_missing".to_string())?;
    let after_revision = successor
        .payload
        .get("afterOwnerRevision")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "startup_terminal_owner_successor_after_revision_missing".to_string())?;
    let before_digest = successor
        .payload
        .get("beforeOwnerDigest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "startup_terminal_owner_successor_before_digest_missing".to_string())?;
    let after_digest = successor
        .payload
        .get("afterOwnerDigest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "startup_terminal_owner_successor_after_digest_missing".to_string())?;
    let task_store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "startup_orphan_task_store_unavailable".to_string())?;
    let task_store = task_store_arc.lock().await;
    let receipt = task_store
        .verified_terminal_owner_transition_receipt(receipt_ref)
        .map_err(|error| format!("verify startup terminal-owner receipt failed: {error}"))?
        .ok_or_else(|| "startup_terminal_owner_successor_receipt_missing".to_string())?;
    let owner_head = task_store
        .canonical_owner_head(&task.id)
        .map_err(|error| format!("load startup terminal-owner task head failed: {error}"))?
        .ok_or_else(|| "startup_terminal_owner_task_head_missing".to_string())?;
    if receipt.receipt_digest() != receipt_digest
        || receipt.proposal_id() != proposal_id
        || receipt.owner_kind() != "agent_task_session"
        || receipt.owner_id() != task.id
        || receipt.before_revision() != before_revision
        || receipt.after_revision() != after_revision
        || receipt.before_digest() != before_digest
        || receipt.after_digest() != after_digest
        || owner_head.revision() != after_revision
        || owner_head.digest() != after_digest
    {
        return Err("startup_terminal_owner_successor_receipt_mismatch".into());
    }
    Ok(Some(proposal_id.to_string()))
}

async fn verify_startup_terminal_owner_successor_projection(
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
        crate::terminal_owner_write_gateway::register_agent_run_store_result(
            state,
            store.get_run(run_id).map_err(|error| error.to_string()),
        )
        .map_err(|error| format!("reload startup successor AgentRun failed: {error}"))?
        .ok_or_else(|| format!("startup_successor_agent_run_missing:{run_id}"))?
    };
    let task = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "startup_orphan_task_store_unavailable".to_string())?;
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .map_err(|error| format!("reload startup successor task failed: {error}"))?
            .ok_or_else(|| format!("startup_successor_task_missing:{task_session_id}"))?
    };
    use openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus;
    use openlife_core::agent::AgentRunStatus;
    let projection_matches = matches!(
        (run.status, task.status),
        (AgentRunStatus::Completed, AgentTaskSessionStatus::Completed)
            | (AgentRunStatus::Cancelled, AgentTaskSessionStatus::Cancelled)
            | (
                AgentRunStatus::WaitingPermission,
                AgentTaskSessionStatus::WaitingPermission
            )
            | (AgentRunStatus::Failed, AgentTaskSessionStatus::Failed)
            | (AgentRunStatus::Failed, AgentTaskSessionStatus::Blocked)
    );
    if !projection_matches || run.task_id != task_session_id {
        return Err(format!(
            "startup_terminal_owner_successor_projection_inconsistent:{run_id}:{task_session_id}"
        ));
    }
    Ok(())
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
            Some("interrupted") => (AgentRunStatus::Failed, AgentTaskSessionStatus::Failed),
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
        || error_digest_hex.is_none()
        || error_digest_hex
            .is_some_and(|hex| hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
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
        crate::terminal_owner_write_gateway::register_agent_run_store_result(
            state,
            store.get_run(run_id).map_err(|error| error.to_string()),
        )
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
    if !matches!(
        status,
        "completed"
            | "completed_with_pending_items"
            | "blocked"
            | "failed"
            | "interrupted"
            | "cancelled"
    ) {
        return Err("startup_final_delivery_status_invalid".into());
    }
    crate::terminal_owner_write_gateway::project_agent_run_from_startup_durable_event(state, event)
        .await
        .map_err(|error| format!("project startup final AgentRun failed: {error}"))?;
    let expected_task_status = match status {
        "completed" => openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed,
        "completed_with_pending_items" => {
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        }
        "blocked" => openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked,
        "failed" | "interrupted" => {
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
        }
        "cancelled" => openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled,
        _ => unreachable!("validated final delivery status"),
    };
    if task.status == expected_task_status {
        // Startup reconciliation may need to repair only AgentRun. Rewriting
        // an already-canonical TaskSession would advance its owner revision
        // past the sealed final-delivery head and invalidate a legitimate
        // ActionResumePrerequisite replay admission.
        return Ok(());
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
        "failed" | "interrupted" => store.fail_session(
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
            crate::life_model_write_gateway::reconcile_startup_lifemodel_file_mutations_with_state(
                state,
            )
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

/// Finish terminal-owner successors from durable ReviewWorkflow claims before
/// the generic Proposal/AgentRun projection pass. This may complete a local
/// Memory effect that was already claimed before a crash, but it never retries
/// an external effect whose remote outcome is unknown.
pub(crate) async fn reconcile_startup_terminal_owner_successors(
    state: &Arc<AppState>,
) -> Result<usize, String> {
    if !state
        .persistence_coordinator
        .startup_reconciliation_mutations_safe()
    {
        return Err("startup_terminal_owner_reconciliation_mutations_unavailable".into());
    }
    let gateway =
        crate::terminal_owner_write_gateway::TerminalOwnerWriteGateway::from_state(state).await?;
    let mut reconciled = 0usize;
    for _ in 0..STARTUP_TERMINAL_OWNER_RECONCILIATION_PASSES {
        let report = gateway
            .reconcile_pending_terminal_owner_successors(
                STARTUP_TERMINAL_OWNER_RECONCILIATION_BATCH,
            )
            .await
            .map_err(|error| format!("startup terminal-owner reconciliation failed: {error}"))?;
        reconciled = reconciled.saturating_add(report.successors_confirmed);
        if report.proposals_projected < STARTUP_TERMINAL_OWNER_RECONCILIATION_BATCH {
            return Ok(reconciled);
        }
    }
    Err("startup_terminal_owner_reconciliation_bound_exceeded".into())
}

/// Reconcile a bounded amount of already-confirmed Proposal truth before the
/// product window becomes interactive. `true` means a durable indexed backlog
/// remains and must be drained by the async continuation; it never means the
/// effects should be replayed.
pub(crate) async fn reconcile_startup_proposal_projections(
    state: &Arc<AppState>,
) -> Result<bool, String> {
    for _ in 0..STARTUP_PROPOSAL_RECONCILIATION_SYNC_PASSES {
        let report =
            crate::commands::proposal::reconcile_startup_durable_proposal_projections_with_state(
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
                .inspect(|_| {
                    persistence.register_ephemeral_development(
                        name,
                        "dev_ephemeral_store_fallback",
                        &e,
                    );
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
    match selected_keyring_service_classification() {
        Ok(classification) => {
            log::info!("[startup] OS credential service class: {classification}")
        }
        Err(error) => log::error!("[startup] OS credential service selection blocked: {error}"),
    }
    bootstrap_with_secret_store(data_dir, &StartupKeyringSecretStore::default())
}

#[cfg(test)]
pub(crate) fn bootstrap_with_secret_store_for_test(
    data_dir: PathBuf,
    secret_store: &dyn crate::secret_store::SecretStore,
) -> BootstrapResult {
    struct RecordedTestSecretReader<'a>(&'a dyn crate::secret_store::SecretStore);

    impl SecretReader for RecordedTestSecretReader<'_> {
        fn read_secret(&self, secret_ref: &str) -> anyhow::Result<Option<String>> {
            crate::secret_store::SecretStore::get(self.0, secret_ref)
        }
    }

    // The long-lived release-bootstrap fixtures exercise durable-store recovery,
    // not first-run credential recovery. Seed their fixture explicitly before
    // entering the product path, so evidence recorders observe every fixture
    // write instead of receiving synthetic, unreported reads. NKR first-run tests
    // call `bootstrap_with_secret_store` directly and observe the real product path.
    use base64::Engine as _;
    let fixed_material = base64::engine::general_purpose::STANDARD.encode([0x54_u8; 32]);
    for secret_ref in [
        AGENT_RUN_RECEIPT_KEY_REF,
        MAIN_CHAT_EVENT_INTEGRITY_KEY_REF,
        ACTION_QUEUE_AUTHORITY_KEY_REF,
        TASK_STORE_AUTHORITY_KEY_REF,
    ] {
        if crate::secret_store::SecretStore::get(secret_store, secret_ref)
            .expect("inspect initialized test credential")
            .is_none()
        {
            secret_store
                .set(secret_ref, &fixed_material)
                .expect("seed initialized test credential");
        }
    }
    let keyring_path = data_dir.join("mcp_audit_keys.json");
    let database_path = data_dir.join("mcp_audit.db");
    if !keyring_path.exists() && !database_path.exists() {
        let epoch = 1_700_000_000_u64;
        let config = openlife_core::mcp_audit::AuditKeyConfig {
            mode: openlife_core::mcp_audit::KeyMode::Keychain,
            salt_b64: None,
            env_var: None,
            key_ref: Some(format!("{MCP_AUDIT_KEY_REF_PREFIX}{epoch}")),
            epoch,
            created_at: "2026-07-29T00:00:00Z".into(),
        };
        crate::storage::save_mcp_audit_keyring_to_path(
            &keyring_path,
            std::slice::from_ref(&config),
        )
        .expect("seed initialized test MCP keyring");
        secret_store
            .set(
                config.key_ref.as_deref().expect("test MCP key reference"),
                &base64::engine::general_purpose::STANDARD.encode([0x4d_u8; 32]),
            )
            .expect("seed initialized test MCP credential");
    }
    bootstrap_with_secret_store(data_dir, &RecordedTestSecretReader(secret_store))
}

fn bootstrap_with_secret_store(
    data_dir: PathBuf,
    secret_store: &dyn SecretReader,
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
    let secret_hydration = hydrate_config_secrets_read_only(&mut config, secret_store);
    let provider_credential_hydration_status = secret_hydration.provider_credential_status;
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

    let (agent_run_credential_status, agent_run_receipt_key_material) = inspect_fixed_credential(
        &data_dir,
        AGENT_RUN_RECEIPT_KEY_REF,
        &[
            "agent_runs.db",
            "life_events.db",
            "main_chat_agent_sessions.db",
        ],
        secret_store,
    );
    let (main_chat_event_credential_status, main_chat_event_integrity_key) =
        inspect_fixed_credential(
            &data_dir,
            MAIN_CHAT_EVENT_INTEGRITY_KEY_REF,
            &["main_chat_agent_events.db"],
            secret_store,
        );
    let (action_queue_credential_status, action_queue_authority_key) = inspect_fixed_credential(
        &data_dir,
        ACTION_QUEUE_AUTHORITY_KEY_REF,
        &["main_chat_action_queue.db"],
        secret_store,
    );
    let (task_store_credential_status, task_store_authority_key_material) =
        inspect_fixed_credential(
            &data_dir,
            TASK_STORE_AUTHORITY_KEY_REF,
            &["tasks.db"],
            secret_store,
        );

    // Apply system configuration
    openlife_core::ollama::set_ollama_cache_ttl_seconds(config.system.ollama_cache_ttl_seconds);

    let life_model_manager = LifeModelManager::new(data_dir.join("life-model").join("current"));
    // Bootstrap must remain read-only for an absent legacy model. Calling
    // `load()` here used to manufacture a default-filled YAML document on the
    // first launch, which then looked like user-authored migration input to the
    // v2 product path. Canonical creation is owned by an explicitly reviewed
    // v2 proposal, never by application startup.
    match life_model_manager.load_existing() {
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
    let governed_data_import_journal =
        match openlife_core::persistence_outbox::GovernedDataImportJournal::new(
            life_model_manager.mutation_journal_path(),
        ) {
            Ok(journal) => {
                let journal = Arc::new(journal);
                match journal.recovery_requirement() {
                    Ok(Some(receipt)) => {
                        persistence.register_read_write("GovernedDataImportJournal");
                        persistence.degrade_globally(
                        openlife_core::persistence_outbox::GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON,
                    );
                        startup_warnings.borrow_mut().push(format!(
                        "governed data import recovery required before effects may resume: operation={} stage={}",
                        receipt.operation_id,
                        receipt.stage.as_str(),
                    ));
                    }
                    Ok(None) => persistence.register_read_write("GovernedDataImportJournal"),
                    Err(error) => {
                        persistence.register_unavailable(
                            "GovernedDataImportJournal",
                            "data_import_journal_read_failed",
                            &error.to_string(),
                        );
                        persistence.degrade_globally("data_import_journal_unavailable");
                        startup_warnings.borrow_mut().push(format!(
                        "governed data-import journal could not be inspected; effects remain fail-closed: {error}"
                    ));
                    }
                }
                Some(journal)
            }
            Err(error) => {
                persistence.register_unavailable(
                    "GovernedDataImportJournal",
                    "data_import_journal_open_failed",
                    &error.to_string(),
                );
                persistence.degrade_globally("data_import_journal_unavailable");
                startup_warnings.borrow_mut().push(format!(
                "governed data-import journal could not be opened; effects remain fail-closed: {error}"
            ));
                None
            }
        };

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

    let agent_run_receipt_key = match agent_run_receipt_key_material {
        Some(key) => match AgentRunReceiptKey::from_bytes(key) {
            Ok(key) => Some(key),
            Err(error) => {
                startup_warnings.borrow_mut().push(format!(
                    "AgentRun receipt key is invalid; AgentRun persistence is disabled: {error}"
                ));
                None
            }
        },
        None => {
            startup_warnings.borrow_mut().push(format!(
                "AgentRun receipt key is unavailable; AgentRun persistence is disabled: {}",
                agent_run_credential_status.as_str()
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
                    let Some(hs_authority_model) =
                        life_model_manager.load_existing().map_err(|error| {
                            format!(
                    "LifeModel could not be loaded for HS asset authority reconciliation: {error}"
                )
                        })?
                    else {
                        // A fresh profile has no legacy authority to reconcile.
                        // In particular, do not manufacture an empty YAML
                        // compatibility owner during bootstrap.
                        return Ok(());
                    };
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
                            log::info!(
                                "collaboration guidance remains LifeModel YAML-owned until a real product runtime receipt is observed; LM-C promotion is fail-closed"
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

    let life_model_learning_db_path = data_dir.join("life_model_learning.db");
    let life_model_learning_store = init_store(
        || {
            openlife_core::agent::LifeModelLearningStore::new(&life_model_learning_db_path)
                .map_err(|error| error.to_string())
        },
        || {
            openlife_core::agent::LifeModelLearningStore::open_read_only_existing(
                &life_model_learning_db_path,
            )
            .map_err(|error| error.to_string())
        },
        || {
            openlife_core::agent::LifeModelLearningStore::new_in_memory()
                .map_err(|error| error.to_string())
        },
        "LifeModelLearningStore",
        &startup_warnings,
        &persistence,
    );
    let life_model_learning_store = optional_store(
        life_model_learning_store,
        "LifeModelLearningStore",
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

    if action_queue_authority_key.is_none() {
        startup_warnings.borrow_mut().push(format!(
            "ActionQueue authority key is unavailable; automatic replay authority is disabled: {}",
            action_queue_credential_status.as_str()
        ));
    }
    let task_store_db_path = data_dir.join("tasks.db");
    let task_store_authority_key = match task_store_authority_key_material {
        Some(key) => openlife_core::tasks::TaskStoreAuthorityKey::from_key_material(&key)
            .map(Some)
            .unwrap_or_else(|error| {
                startup_warnings.borrow_mut().push(format!(
                    "TaskStore authority key is invalid; scheduled execution is disabled: {error}"
                ));
                None
            }),
        None => {
            startup_warnings.borrow_mut().push(format!(
                "TaskStore authority key is unavailable; scheduled execution is disabled: {}",
                task_store_credential_status.as_str()
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

    if main_chat_event_integrity_key.is_none() {
        startup_warnings.borrow_mut().push(format!(
            "Main Chat event integrity key is unavailable; durable event truth is unavailable: {}",
            main_chat_event_credential_status.as_str()
        ));
    }
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
    let mcp_audit_db_path = data_dir.join("mcp_audit.db");
    let (audit_key_hydration, mcp_audit_credential_status, mcp_warning) =
        match load_mcp_audit_keyring_from_path(&audit_keyring_path) {
            McpAuditKeyringLoad::Absent => {
                match McpAuditStore::inspect_existing_database(&mcp_audit_db_path) {
                    Ok(inspection) if inspection.is_empty_or_absent() => (
                        None,
                        CredentialBootstrapStatus::InitializationRequired,
                        None,
                    ),
                    Ok(inspection) => (
                        None,
                        CredentialBootstrapStatus::MissingExistingData,
                        Some(format!(
                            "MCP audit keyring is missing while the canonical audit database contains {} rows",
                            inspection.row_count
                        )),
                    ),
                    Err(error) => (
                        None,
                        CredentialBootstrapStatus::Unknown,
                        Some(format!(
                            "missing MCP audit keyring beside an untrusted audit database: {error}"
                        )),
                    ),
                }
            }
            McpAuditKeyringLoad::Present(configs) => {
                match inspect_existing_mcp_audit_keys(configs, secret_store) {
                    McpAuditKeyHydrationInspection::Available(hydration) => {
                        match McpAuditStore::preflight_existing_database_key_materials(
                            &mcp_audit_db_path,
                            &hydration.materials,
                        ) {
                            Ok(_) => {
                                let latest_is_keychain = hydration.configs.last().is_some_and(
                                    |config| {
                                        config.mode
                                            == openlife_core::mcp_audit::KeyMode::Keychain
                                    },
                                );
                                let has_keychain_epoch = hydration.configs.iter().any(|config| {
                                    config.mode == openlife_core::mcp_audit::KeyMode::Keychain
                                });
                                if latest_is_keychain {
                                    (
                                        Some(hydration),
                                        CredentialBootstrapStatus::Available,
                                        None,
                                    )
                                } else if !has_keychain_epoch {
                                    (
                                        Some(hydration),
                                        CredentialBootstrapStatus::InitializationRequired,
                                        None,
                                    )
                                } else {
                                    (
                                        Some(hydration),
                                        CredentialBootstrapStatus::Invalid,
                                        Some(
                                            "MCP audit keyring has a legacy epoch after a Keychain write epoch; initialization is blocked"
                                                .into(),
                                        ),
                                    )
                                }
                            }
                            Err(error)
                                if openlife_core::mcp_audit::is_payload_integrity_failure(
                                    &error,
                                ) =>
                            {
                                (
                                    Some(hydration),
                                    CredentialBootstrapStatus::Invalid,
                                    Some(format!(
                                        "audit payload integrity is invalid; MCP audit effects remain disabled: {error}"
                                    )),
                                )
                            }
                            Err(error) => (
                                Some(hydration),
                                CredentialBootstrapStatus::Unknown,
                                Some(format!(
                                    "MCP audit database preflight is unavailable; effects remain disabled: {error}"
                                )),
                            ),
                        }
                    }
                    McpAuditKeyHydrationInspection::MissingExistingData => (
                        None,
                        CredentialBootstrapStatus::MissingExistingData,
                        Some("MCP audit keychain reference has no credential".into()),
                    ),
                    McpAuditKeyHydrationInspection::Invalid => (
                        None,
                        CredentialBootstrapStatus::Invalid,
                        Some("MCP audit key material is invalid".into()),
                    ),
                    McpAuditKeyHydrationInspection::Unavailable => (
                        None,
                        CredentialBootstrapStatus::Unavailable,
                        Some("MCP audit key material is unavailable".into()),
                    ),
                }
            }
            McpAuditKeyringLoad::PresentInvalid { error } => (
                None,
                CredentialBootstrapStatus::Invalid,
                Some(format!("MCP audit keyring is present but invalid: {error}")),
            ),
            McpAuditKeyringLoad::Unreadable { error } => (
                None,
                CredentialBootstrapStatus::Unavailable,
                Some(format!("MCP audit keyring is unreadable: {error}")),
            ),
        };
    let mcp_reference_available = audit_key_hydration.is_some();
    if mcp_reference_available {
        persistence.register_read_write("McpAuditKeyReferenceStore");
    } else {
        persistence.register_unavailable(
            "McpAuditKeyReferenceStore",
            "mcp_audit_key_hydration_failed",
            mcp_audit_credential_status.as_str(),
        );
    }
    if mcp_audit_credential_status != CredentialBootstrapStatus::Available {
        let warning = mcp_warning.unwrap_or_else(|| {
            "MCP audit credential initialization is required; audit effects remain disabled".into()
        });
        startup_warnings.borrow_mut().push(warning);
    }
    let audit_materials = audit_key_hydration
        .map(|hydration| hydration.materials)
        .unwrap_or_default();
    let mcp_audit_store = if mcp_audit_credential_status == CredentialBootstrapStatus::Available {
        let store = init_store(
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
        required_store_or_unavailable(store, "McpAuditStore", &startup_warnings, || {
            Ok(McpAuditStore::unavailable_sentinel(
                "canonical and read-only audit store open failed",
            ))
        })
    } else {
        persistence.register_unavailable(
            "McpAuditStore",
            "mcp_audit_credential_unavailable",
            mcp_audit_credential_status.as_str(),
        );
        McpAuditStore::unavailable_sentinel(
            "credential bootstrap did not prove an available MCP audit write epoch",
        )
    };

    let hot_cache: SharedHotCache = {
        let initial_cache = match life_model_manager.load_existing() {
            Ok(Some(model)) => HotMemoryCache::from_life_model(&model),
            Ok(None) | Err(_) => HotMemoryCache::default(),
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
                if persistence.bootstrap_mutations_safe() {
                    let daily_task_cutover_result = match life_model_manager.load_existing() {
                        Ok(Some(model)) => crate::state_projection::reconcile_and_import_legacy_yaml_daily_tasks(
                                &store,
                                &model,
                                chrono::Utc::now(),
                            )
                            .map(|_| ()),
                        Ok(None) => Ok(()),
                        Err(error) => Err(format!(
                            "LifeModel could not be loaded for legacy daily-task StateStore cutover: {error}"
                        )),
                    };
                    if let Err(error) = daily_task_cutover_result {
                        // Shipped product reads require the import receipt and
                        // fail closed. Never merge a partial StateStore view
                        // with the legacy YAML source after a blocked cutover.
                        startup_warnings.borrow_mut().push(format!(
                            "legacy daily-task StateStore cutover remains blocked: {error}"
                        ));
                    }
                    let history_cutover_result = memory_store
                        .list_legacy_state_history_migration_source()
                        .map_err(|error| {
                            format!(
                                "MemoryStore state history could not be loaded for StateStore cutover: {error}"
                            )
                        })
                        .and_then(|snapshot| {
                            crate::state_projection::reconcile_legacy_memory_state_history_shadow(
                                &store,
                                &snapshot,
                                chrono::Utc::now(),
                            )?;
                            store
                                .import_legacy_state_history_shadow(chrono::Utc::now())
                                .map_err(|error| {
                                    format!(
                                        "legacy state-history canonical import failed: {error}"
                                    )
                                })
                        });
                    if let Err(error) = history_cutover_result {
                        // Product reads fail closed on the absent import
                        // receipt. MemoryStore remains migration evidence, not
                        // a hidden product fallback.
                        startup_warnings.borrow_mut().push(format!(
                            "legacy state-history StateStore cutover remains blocked: {error}"
                        ));
                    }
                } else {
                    startup_warnings.borrow_mut().push(
                        "legacy daily-task and state-history StateStore shadow reconciliation skipped because canonical bootstrap mutations are unsafe"
                            .into(),
                    );
                }
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

    let provider_credential_status = match provider_credential_hydration_status {
        ProviderCredentialHydrationStatus::NotReferenced
        | ProviderCredentialHydrationStatus::Missing => {
            CredentialBootstrapStatus::MissingExistingData
        }
        ProviderCredentialHydrationStatus::Available => CredentialBootstrapStatus::Available,
        ProviderCredentialHydrationStatus::Invalid => CredentialBootstrapStatus::Invalid,
        ProviderCredentialHydrationStatus::Unavailable => CredentialBootstrapStatus::Unavailable,
    };
    let credential_bootstrap_snapshot = CredentialBootstrapSnapshot::from_statuses([
        agent_run_credential_status,
        main_chat_event_credential_status,
        action_queue_credential_status,
        task_store_credential_status,
        mcp_audit_credential_status,
    ])
    .with_provider_status(provider_credential_status);
    let app_state = Arc::new(AppState {
        persistence_coordinator: Arc::clone(&persistence),
        governed_data_import_journal,
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
        life_model_learning_store: life_model_learning_store
            .map(|store| Arc::new(Mutex::new(store))),
        plan_execute_session_store: plan_execute_session_store
            .map(|store| Arc::new(Mutex::new(store))),
        main_chat_agent_session_store: main_chat_agent_session_store
            .map(|store| Arc::new(Mutex::new(store))),
        main_chat_action_queue_store: main_chat_action_queue_store
            .map(|store| Arc::new(Mutex::new(store))),
        main_chat_agent_event_store: main_chat_agent_event_store
            .map(|store| Arc::new(Mutex::new(store))),
        main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
        patch_store: patch_store.map(|store| Arc::new(Mutex::new(store))),
        rollout_metrics_store,
        tool_permission_store: Arc::new(Mutex::new(tool_permission_store)),
        skill_registry: Arc::new(Mutex::new(skill_registry)),
        plugin_registry: Arc::new(Mutex::new(plugin_registry)),
        hot_cache,
        startup_warnings: startup_warnings.into_inner(),
        credential_bootstrap_snapshot,
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
    use crate::secret_store::SecretStore;
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

    struct TestSecretStore {
        values: std::sync::Mutex<std::collections::HashMap<String, String>>,
        set_count: std::sync::atomic::AtomicUsize,
        delete_count: std::sync::atomic::AtomicUsize,
    }

    impl Default for TestSecretStore {
        fn default() -> Self {
            use base64::Engine as _;

            let encoded = base64::engine::general_purpose::STANDARD.encode([0x41; 32]);
            Self {
                values: std::sync::Mutex::new(std::collections::HashMap::from([
                    (AGENT_RUN_RECEIPT_KEY_REF.into(), encoded.clone()),
                    (MAIN_CHAT_EVENT_INTEGRITY_KEY_REF.into(), encoded.clone()),
                    (ACTION_QUEUE_AUTHORITY_KEY_REF.into(), encoded.clone()),
                    (TASK_STORE_AUTHORITY_KEY_REF.into(), encoded),
                ])),
                set_count: std::sync::atomic::AtomicUsize::new(0),
                delete_count: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl TestSecretStore {
        fn empty() -> Self {
            Self {
                values: std::sync::Mutex::new(std::collections::HashMap::new()),
                set_count: std::sync::atomic::AtomicUsize::new(0),
                delete_count: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn mcp_secret_snapshot(&self) -> Vec<(String, String)> {
            let mut values = self
                .values
                .lock()
                .unwrap()
                .iter()
                .filter(|(secret_ref, _)| {
                    secret_ref.starts_with(crate::secret_store::MCP_AUDIT_KEY_REF_PREFIX)
                })
                .map(|(secret_ref, value)| (secret_ref.clone(), value.clone()))
                .collect::<Vec<_>>();
            values.sort_by(|left, right| left.0.cmp(&right.0));
            values
        }

        fn mcp_secret_refs(&self) -> Vec<String> {
            let mut refs = self
                .values
                .lock()
                .unwrap()
                .keys()
                .filter(|secret_ref| {
                    secret_ref.starts_with(crate::secret_store::MCP_AUDIT_KEY_REF_PREFIX)
                })
                .cloned()
                .collect::<Vec<_>>();
            refs.sort();
            refs
        }

        fn preload_mcp_key(&self, epoch: u64, key: [u8; 32]) {
            use base64::Engine as _;

            self.set(
                &format!("{}{}", crate::secret_store::MCP_AUDIT_KEY_REF_PREFIX, epoch),
                &base64::engine::general_purpose::STANDARD.encode(key),
            )
            .unwrap();
        }

        fn operation_counts(&self) -> (usize, usize) {
            (
                self.set_count.load(std::sync::atomic::Ordering::SeqCst),
                self.delete_count.load(std::sync::atomic::Ordering::SeqCst),
            )
        }

        fn reset_operation_counts(&self) {
            self.set_count.store(0, std::sync::atomic::Ordering::SeqCst);
            self.delete_count
                .store(0, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn d068_database_family_bytes(
        path: &std::path::Path,
    ) -> Vec<(std::path::PathBuf, Option<Vec<u8>>)> {
        [
            path.to_path_buf(),
            std::path::PathBuf::from(format!("{}-wal", path.display())),
            std::path::PathBuf::from(format!("{}-shm", path.display())),
            std::path::PathBuf::from(format!("{}-journal", path.display())),
        ]
        .into_iter()
        .map(|candidate| {
            let bytes = match std::fs::read(&candidate) {
                Ok(bytes) => Some(bytes),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => panic!("read {}: {error}", candidate.display()),
            };
            (candidate, bytes)
        })
        .collect()
    }

    impl SecretStore for TestSecretStore {
        fn get(&self, secret_ref: &str) -> anyhow::Result<Option<String>> {
            Ok(self.values.lock().unwrap().get(secret_ref).cloned())
        }

        fn set(&self, secret_ref: &str, value: &str) -> anyhow::Result<()> {
            self.set_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.values
                .lock()
                .unwrap()
                .insert(secret_ref.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, secret_ref: &str) -> anyhow::Result<()> {
            self.delete_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.values.lock().unwrap().remove(secret_ref);
            Ok(())
        }
    }

    struct UnavailableSecretReader;

    impl SecretReader for UnavailableSecretReader {
        fn read_secret(&self, _secret_ref: &str) -> anyhow::Result<Option<String>> {
            anyhow::bail!("credential backend unavailable")
        }
    }

    #[test]
    fn nkr_s1_credential_fresh_bootstrap_is_zero_write_and_reports_five_initialization_slots() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::empty();

        let result = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);

        assert_eq!(secrets.operation_counts(), (0, 0));
        assert!(!directory.path().join("mcp_audit_keys.json").exists());
        assert!(!directory.path().join("mcp_audit.db").exists());
        assert!(
            !directory
                .path()
                .join("life-model/current/life_model.yaml")
                .exists(),
            "fresh bootstrap must not manufacture a legacy LifeModel YAML"
        );
        assert_eq!(result.state.credential_bootstrap_snapshot.purposes.len(), 6);
        assert!(result.state.credential_bootstrap_snapshot.purposes[..5]
            .iter()
            .all(|purpose| purpose.status == CredentialBootstrapStatus::InitializationRequired));
        assert_eq!(
            result.state.credential_bootstrap_snapshot.purposes[5].purpose,
            "provider_api_key"
        );
        assert_eq!(
            result.state.credential_bootstrap_snapshot.purposes[5].status,
            CredentialBootstrapStatus::MissingExistingData
        );
        assert_eq!(result.state.credential_bootstrap_snapshot.digest.len(), 64);
    }

    #[tokio::test]
    async fn nkr_s1_credential_life_state_projection_publishes_the_bootstrap_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::empty();
        let result = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);

        let projection =
            crate::life_state_projection::get_life_state_projection_with_state(&result.state)
                .await
                .unwrap();

        assert_eq!(
            projection.credential_bootstrap,
            result.state.credential_bootstrap_snapshot
        );
        assert!(projection
            .source_refs
            .contains(&"bootstrap:credential_snapshot".to_string()));
    }

    #[test]
    fn nkr_s1_credential_missing_key_beside_existing_data_is_never_initialization_required() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("agent_runs.db"), b"existing").unwrap();
        let secrets = TestSecretStore::empty();

        let result = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);

        assert_eq!(secrets.operation_counts(), (0, 0));
        assert_eq!(
            result.state.credential_bootstrap_snapshot.purposes[0].status,
            CredentialBootstrapStatus::MissingExistingData
        );
    }

    #[test]
    fn nkr_s1_credential_invalid_and_unavailable_material_remain_fail_closed() {
        let invalid_directory = tempfile::tempdir().unwrap();
        let invalid = TestSecretStore::empty();
        invalid
            .set(AGENT_RUN_RECEIPT_KEY_REF, "not-base64")
            .unwrap();
        invalid.reset_operation_counts();
        let invalid_result =
            bootstrap_with_secret_store(invalid_directory.path().to_path_buf(), &invalid);
        assert_eq!(invalid.operation_counts(), (0, 0));
        assert_eq!(
            invalid_result.state.credential_bootstrap_snapshot.purposes[0].status,
            CredentialBootstrapStatus::Invalid
        );

        let unavailable_directory = tempfile::tempdir().unwrap();
        let unavailable_result = bootstrap_with_secret_store(
            unavailable_directory.path().to_path_buf(),
            &UnavailableSecretReader,
        );
        assert!(unavailable_result
            .state
            .credential_bootstrap_snapshot
            .purposes[..4]
            .iter()
            .all(|purpose| purpose.status == CredentialBootstrapStatus::Unavailable));
    }

    fn seed_governed_import_journal(
        data_dir: &Path,
    ) -> (
        openlife_core::persistence_outbox::GovernedDataImportJournal,
        String,
        String,
    ) {
        let manager = LifeModelManager::new(data_dir.join("life-model").join("current"));
        let journal = openlife_core::persistence_outbox::GovernedDataImportJournal::new(
            manager.mutation_journal_path(),
        )
        .unwrap();
        let operation_id = uuid::Uuid::new_v4().hyphenated().to_string();
        let owner = "LifeModelFileStore".to_string();
        journal
            .prepare(
                openlife_core::persistence_outbox::GovernedDataImportPrepare {
                    operation_id: operation_id.clone(),
                    payload_digest: openlife_core::persistence_outbox::metadata_digest(
                        "bootstrap data import payload",
                    ),
                    request_digest: openlife_core::persistence_outbox::metadata_digest(
                        "bootstrap governed request",
                    ),
                    owners: vec![
                        openlife_core::persistence_outbox::GovernedDataImportOwnerPlan {
                            owner: owner.clone(),
                            import_target: "life_model".into(),
                            before_digest: openlife_core::persistence_outbox::metadata_digest(
                                "bootstrap before",
                            ),
                            target_digest: openlife_core::persistence_outbox::metadata_digest(
                                "bootstrap target",
                            ),
                            item_count: 1,
                        },
                    ],
                },
            )
            .unwrap();
        (journal, operation_id, owner)
    }

    #[test]
    fn startup_fails_closed_when_governed_data_import_requires_recovery() {
        let directory = tempfile::tempdir().unwrap();
        let (_journal, operation_id, _owner) = seed_governed_import_journal(directory.path());

        let result = bootstrap_with_secret_store_for_test(
            directory.path().to_path_buf(),
            &TestSecretStore::default(),
        );
        let health = result.state.persistence_coordinator.snapshot();
        assert!(result.state.governed_data_import_journal.is_some());
        assert!(health.global_reason_codes.iter().any(|reason| {
            reason
                == openlife_core::persistence_outbox::GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON
        }));
        assert!(!health.canonical_writes_allowed);
        assert!(!health.provider_dispatch_allowed);
        assert!(!health.tool_dispatch_allowed);
        assert!(result.state.startup_warnings.iter().any(|warning| {
            warning.contains(&operation_id) && warning.contains("stage=prepared")
        }));
    }

    #[test]
    fn startup_accepts_terminal_governed_data_import_receipt() {
        let directory = tempfile::tempdir().unwrap();
        let (journal, operation_id, owner) = seed_governed_import_journal(directory.path());
        journal
            .transition(
                &operation_id,
                openlife_core::persistence_outbox::GovernedDataImportStage::Compensated,
                &[openlife_core::persistence_outbox::GovernedDataImportOwnerUpdate {
                    owner,
                    status:
                        openlife_core::persistence_outbox::GovernedDataImportOwnerStatus::Compensated,
                }],
                Some(&openlife_core::persistence_outbox::metadata_digest(
                    "no owner effect was committed",
                )),
            )
            .unwrap();
        drop(journal);

        let result = bootstrap_with_secret_store_for_test(
            directory.path().to_path_buf(),
            &TestSecretStore::default(),
        );
        let health = result.state.persistence_coordinator.snapshot();
        assert!(result.state.governed_data_import_journal.is_some());
        assert!(!health.global_reason_codes.iter().any(|reason| {
            reason
                == openlife_core::persistence_outbox::GOVERNED_DATA_IMPORT_RECOVERY_REQUIRED_REASON
        }));
        assert!(!result
            .state
            .startup_warnings
            .iter()
            .any(|warning| warning.contains("governed data import recovery required")));
    }

    #[test]
    fn governed_import_journal_open_failure_has_no_runtime_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let manager = LifeModelManager::new(directory.path().join("life-model").join("current"));
        std::fs::create_dir_all(manager.mutation_journal_path()).unwrap();

        let result = bootstrap_with_secret_store_for_test(
            directory.path().to_path_buf(),
            &TestSecretStore::default(),
        );
        let health = result.state.persistence_coordinator.snapshot();

        assert!(result.state.governed_data_import_journal.is_none());
        assert!(health
            .global_reason_codes
            .iter()
            .any(|reason| reason == "data_import_journal_unavailable"));
        assert!(!health.canonical_writes_allowed);
        assert!(!health.provider_dispatch_allowed);
        assert!(!health.tool_dispatch_allowed);
    }

    fn d057_key_config(epoch: u64) -> openlife_core::mcp_audit::AuditKeyConfig {
        openlife_core::mcp_audit::AuditKeyConfig {
            mode: openlife_core::mcp_audit::KeyMode::Keychain,
            salt_b64: None,
            env_var: None,
            key_ref: Some(format!(
                "{}{}",
                crate::secret_store::MCP_AUDIT_KEY_REF_PREFIX,
                epoch
            )),
            epoch,
            created_at: "2026-07-16T00:00:00Z".into(),
        }
    }

    fn bootstrap_with_initialized_test_credentials(
        data_dir: PathBuf,
        secrets: &TestSecretStore,
    ) -> BootstrapResult {
        const EPOCH: u64 = 1_700_000_001;
        let key = [0x52; 32];
        let config = d057_key_config(EPOCH);
        if SecretStore::get(
            secrets,
            config.key_ref.as_deref().expect("test MCP key reference"),
        )
        .unwrap()
        .is_none()
        {
            secrets.preload_mcp_key(EPOCH, key);
        }
        crate::storage::save_mcp_audit_keyring_to_path(
            &data_dir.join("mcp_audit_keys.json"),
            &[config],
        )
        .unwrap();
        secrets.reset_operation_counts();
        bootstrap_with_secret_store(data_dir, secrets)
    }

    fn bootstrap_with_default_initialized_test_credentials(data_dir: PathBuf) -> BootstrapResult {
        bootstrap_with_initialized_test_credentials(data_dir, &TestSecretStore::default())
    }

    fn d057_seed_nonempty_audit_store(
        data_dir: &std::path::Path,
        secrets: &TestSecretStore,
        epoch: u64,
        key: [u8; 32],
    ) {
        let config = d057_key_config(epoch);
        secrets.preload_mcp_key(epoch, key);
        crate::storage::save_mcp_audit_keyring_to_path(
            &data_dir.join("mcp_audit_keys.json"),
            std::slice::from_ref(&config),
        )
        .unwrap();
        let store = McpAuditStore::with_key_materials(
            data_dir.join("mcp_audit.db"),
            vec![openlife_core::mcp_audit::AuditKeyMaterial { config, key }],
        )
        .unwrap();
        store
            .insert_log(
                "d057.seed",
                &serde_json::json!({"payload": "minimized"}),
                "seeded",
                true,
                false,
            )
            .unwrap();
    }

    #[tokio::test]
    async fn d057_malformed_keyring_never_creates_secret_or_overwrites_authority() {
        let directory = tempfile::tempdir().unwrap();
        let keyring_path = directory.path().join("mcp_audit_keys.json");
        let malformed = b"{ malformed audit key authority\n";
        std::fs::write(&keyring_path, malformed).unwrap();
        let secrets = TestSecretStore::default();

        let result = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);

        assert_eq!(std::fs::read(&keyring_path).unwrap(), malformed);
        assert!(secrets.mcp_secret_refs().is_empty());
        assert!(!directory.path().join("mcp_audit.db").exists());
        assert!(result
            .state
            .mcp_audit_store
            .lock()
            .await
            .list_logs(1)
            .unwrap_err()
            .to_string()
            .contains("unavailable"));
        assert!(result
            .state
            .startup_warnings
            .iter()
            .any(|warning| { warning.contains("MCP audit keyring is present but invalid") }));
    }

    #[tokio::test]
    async fn d057_missing_keyring_beside_nonempty_database_fails_without_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        d057_seed_nonempty_audit_store(directory.path(), &secrets, 17, [0x71; 32]);
        let keyring_path = directory.path().join("mcp_audit_keys.json");
        std::fs::remove_file(&keyring_path).unwrap();
        let db_path = directory.path().join("mcp_audit.db");
        let before_db = std::fs::read(&db_path).unwrap();
        let before_refs = secrets.mcp_secret_refs();

        let result = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);

        assert!(!keyring_path.exists());
        assert_eq!(std::fs::read(&db_path).unwrap(), before_db);
        assert_eq!(secrets.mcp_secret_refs(), before_refs);
        assert!(result
            .state
            .mcp_audit_store
            .lock()
            .await
            .list_logs(1)
            .unwrap_err()
            .to_string()
            .contains("unavailable"));
        assert!(result.state.startup_warnings.iter().any(|warning| {
            warning.contains("keyring is missing") && warning.contains("contains 1 rows")
        }));
    }

    #[tokio::test]
    async fn d068_version_flip_keeps_key_authority_canonical_and_audit_unavailable() {
        const PRIVATE_ARGUMENT: &str = "D068-BOOTSTRAP-RAW-HEALTH-ARGUMENT";
        const PRIVATE_RESULT: &str = "D068-BOOTSTRAP-RAW-FINANCE-RESULT";
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let epoch = 68;
        let key = [0x68; 32];
        let config = d057_key_config(epoch);
        secrets.preload_mcp_key(epoch, key);
        let keyring_path = directory.path().join("mcp_audit_keys.json");
        crate::storage::save_mcp_audit_keyring_to_path(
            &keyring_path,
            std::slice::from_ref(&config),
        )
        .unwrap();
        let database_path = directory.path().join("mcp_audit.db");
        let store = McpAuditStore::with_key_materials(
            &database_path,
            vec![openlife_core::mcp_audit::AuditKeyMaterial { config, key }],
        )
        .unwrap();
        let row_id = store
            .d068_insert_legacy_payload_fixture_for_test(
                "d068-bootstrap-version-flip",
                &serde_json::json!({"health": PRIVATE_ARGUMENT}),
                PRIVATE_RESULT,
            )
            .unwrap();
        store
            .d068_flip_payload_version_to_current_for_test(row_id)
            .unwrap();
        drop(store);
        let keyring_before = std::fs::read(&keyring_path).unwrap();
        let secrets_before = secrets.mcp_secret_snapshot();
        let database_before = d068_database_family_bytes(&database_path);

        let result = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
        let persistence = result.state.persistence_coordinator.snapshot();
        let mode = |store: &str| {
            persistence
                .stores
                .iter()
                .find(|health| health.store == store)
                .map(|health| health.mode)
        };
        let audit_error = result
            .state
            .mcp_audit_store
            .lock()
            .await
            .list_logs(10)
            .unwrap_err();

        assert_eq!(
            mode("McpAuditKeyReferenceStore"),
            Some(crate::persistence_coordinator::PersistenceStoreMode::ReadWriteCanonical)
        );
        assert_eq!(
            mode("McpAuditStore"),
            Some(crate::persistence_coordinator::PersistenceStoreMode::Unavailable)
        );
        assert!(!persistence.provider_dispatch_allowed);
        assert!(!persistence.tool_dispatch_allowed);
        assert!(audit_error.to_string().contains("unavailable"));
        assert_eq!(std::fs::read(&keyring_path).unwrap(), keyring_before);
        assert_eq!(secrets.mcp_secret_snapshot(), secrets_before);
        assert_eq!(d068_database_family_bytes(&database_path), database_before);
        let warnings = result.state.startup_warnings.join("\n");
        assert!(warnings.contains("audit payload integrity is invalid"));
        assert!(!warnings.contains(PRIVATE_ARGUMENT));
        assert!(!warnings.contains(PRIVATE_RESULT));
    }

    #[tokio::test]
    async fn d057_wrong_audit_key_remains_unavailable_after_payload_attribution_split() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let epoch = 69;
        let correct_key = [0x69; 32];
        let wrong_key = [0x96; 32];
        let config = d057_key_config(epoch);
        let keyring_path = directory.path().join("mcp_audit_keys.json");
        crate::storage::save_mcp_audit_keyring_to_path(
            &keyring_path,
            std::slice::from_ref(&config),
        )
        .unwrap();
        let database_path = directory.path().join("mcp_audit.db");
        let store = McpAuditStore::with_key_materials(
            &database_path,
            vec![openlife_core::mcp_audit::AuditKeyMaterial {
                config,
                key: correct_key,
            }],
        )
        .unwrap();
        store
            .insert_log(
                "d057-wrong-key",
                &serde_json::json!({"bounded": true}),
                "bounded",
                true,
                false,
            )
            .unwrap();
        drop(store);
        secrets.preload_mcp_key(epoch, wrong_key);
        let keyring_before = std::fs::read(&keyring_path).unwrap();
        let secrets_before = secrets.mcp_secret_snapshot();
        let database_before = d068_database_family_bytes(&database_path);

        let result = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
        let persistence = result.state.persistence_coordinator.snapshot();
        let mode = |store: &str| {
            persistence
                .stores
                .iter()
                .find(|health| health.store == store)
                .map(|health| health.mode)
        };

        assert_eq!(
            mode("McpAuditKeyReferenceStore"),
            Some(crate::persistence_coordinator::PersistenceStoreMode::ReadWriteCanonical)
        );
        assert_eq!(
            mode("McpAuditStore"),
            Some(crate::persistence_coordinator::PersistenceStoreMode::Unavailable)
        );
        assert!(!persistence.provider_dispatch_allowed);
        assert!(!persistence.tool_dispatch_allowed);
        assert_eq!(std::fs::read(&keyring_path).unwrap(), keyring_before);
        assert_eq!(secrets.mcp_secret_snapshot(), secrets_before);
        assert_eq!(d068_database_family_bytes(&database_path), database_before);
        assert!(result
            .state
            .startup_warnings
            .iter()
            .any(|warning| warning.contains("MCP audit database preflight is unavailable")));
        assert_eq!(
            result.state.credential_bootstrap_snapshot.purposes[4].status,
            CredentialBootstrapStatus::Unknown
        );
    }

    #[tokio::test]
    async fn nkr_s1_credential_true_first_boot_defers_mcp_reference_creation_until_explicit_recovery(
    ) {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();

        let first = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);
        assert!(first
            .state
            .mcp_audit_store
            .lock()
            .await
            .list_logs(1)
            .unwrap_err()
            .to_string()
            .contains("unavailable"));
        assert!(secrets.mcp_secret_refs().is_empty());
        let keyring_path = directory.path().join("mcp_audit_keys.json");
        assert!(!keyring_path.exists());
        assert!(!directory.path().join("mcp_audit.db").exists());
        assert_eq!(
            first.state.credential_bootstrap_snapshot.purposes[4].status,
            CredentialBootstrapStatus::InitializationRequired
        );
        drop(first);

        let restarted = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);

        assert!(secrets.mcp_secret_refs().is_empty());
        assert!(!keyring_path.exists());
        assert_eq!(
            restarted.state.credential_bootstrap_snapshot.purposes[4].status,
            CredentialBootstrapStatus::InitializationRequired
        );
    }

    #[tokio::test]
    async fn nkr_s1_credential_existing_empty_mcp_database_remains_initialization_required_without_mutation(
    ) {
        let directory = tempfile::tempdir().unwrap();
        let bootstrap_key = d057_key_config(29);
        let bootstrap_material = openlife_core::mcp_audit::AuditKeyMaterial {
            config: bootstrap_key,
            key: [0x29; 32],
        };
        let empty_store = McpAuditStore::with_key_materials(
            directory.path().join("mcp_audit.db"),
            vec![bootstrap_material],
        )
        .unwrap();
        drop(empty_store);
        let before_db = std::fs::read(directory.path().join("mcp_audit.db")).unwrap();
        let secrets = TestSecretStore::default();

        let result = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);

        assert!(secrets.mcp_secret_refs().is_empty());
        assert!(!directory.path().join("mcp_audit_keys.json").exists());
        assert!(result
            .state
            .mcp_audit_store
            .lock()
            .await
            .list_logs(1)
            .unwrap_err()
            .to_string()
            .contains("unavailable"));
        assert_eq!(
            std::fs::read(directory.path().join("mcp_audit.db")).unwrap(),
            before_db
        );
        assert_eq!(
            result.state.credential_bootstrap_snapshot.purposes[4].status,
            CredentialBootstrapStatus::InitializationRequired
        );
    }

    #[test]
    fn nkr_s1_credential_mcp_keychain_epoch_followed_by_legacy_epoch_is_not_initializable() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let keychain = d057_key_config(41);
        secrets.preload_mcp_key(41, [0x41; 32]);
        let legacy = openlife_core::mcp_audit::AuditKeyConfig {
            mode: openlife_core::mcp_audit::KeyMode::Derived,
            epoch: 42,
            created_at: "2026-07-29T00:00:01Z".into(),
            ..Default::default()
        };
        crate::storage::save_mcp_audit_keyring_to_path(
            &directory.path().join("mcp_audit_keys.json"),
            &[keychain, legacy],
        )
        .unwrap();
        secrets.reset_operation_counts();

        let result = bootstrap_with_secret_store(directory.path().to_path_buf(), &secrets);

        assert_eq!(secrets.operation_counts(), (0, 0));
        assert_eq!(
            result.state.credential_bootstrap_snapshot.purposes[4].status,
            CredentialBootstrapStatus::Invalid
        );
        assert!(result
            .state
            .startup_warnings
            .iter()
            .any(|warning| warning.contains("legacy epoch after a Keychain write epoch")));
    }

    #[test]
    fn mcp_keyring_failure_injection_remains_available_for_explicit_recovery_tests() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit_keys.json");
        let config = d057_key_config(31);

        crate::storage::fail_next_mcp_audit_keyring_save_for_test(path.clone());
        assert!(crate::storage::save_mcp_audit_keyring_to_path(
            &path,
            std::slice::from_ref(&config)
        )
        .is_err());
        assert!(!path.exists());

        crate::storage::fail_next_mcp_audit_keyring_save_after_write_for_test(path.clone());
        assert!(crate::storage::save_mcp_audit_keyring_to_path(&path, &[config]).is_err());
        assert!(path.exists());
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
            bootstrap_with_default_initialized_test_credentials(temp_dir.path().to_path_buf());

        assert!(
            result.state.startup_warnings.is_empty(),
            "an initialized fresh profile must not enter global safe mode for informational authority state: {:?}",
            result.state.startup_warnings
        );

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

    #[tokio::test]
    async fn startup_releases_external_claim_when_artifact_intent_was_never_prepared() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let result =
            bootstrap_with_default_initialized_test_credentials(temp_dir.path().to_path_buf());
        let target_path = temp_dir.path().join("never-written.txt");
        let proposal = openlife_core::agent::AgentProposal::new(
            openlife_core::agent::ProposalType::ExternalWriteAction,
            "filesystem.startup-orphan",
            serde_json::json!({
                "path": target_path,
                "content": "must not be written by startup recovery",
            }),
            "Crash after claim and before ArtifactMaterializer prepared intent.",
            1.0,
            openlife_core::agent::RiskLevel::High,
            openlife_core::agent::ProposalSource::ChatConversation,
        );
        let proposal_id = proposal.id.clone();
        let claim_id = {
            let store = result
                .state
                .proposal_store
                .as_ref()
                .expect("proposal store")
                .lock()
                .await;
            store.create_proposal(&proposal).expect("create proposal");
            store
                .claim_dispatch(&proposal_id)
                .expect("claim proposal")
                .expect("one startup fixture claim")
        };

        assert!(!reconcile_startup_proposal_projections(&result.state)
            .await
            .expect("startup reconciliation proves the effect was not attempted"));
        let store = result
            .state
            .proposal_store
            .as_ref()
            .expect("proposal store")
            .lock()
            .await;
        assert_eq!(
            store
                .dispatch_state(&proposal_id)
                .expect("read recovered dispatch state")
                .as_deref(),
            Some("failed_before_effect")
        );
        assert_eq!(
            store
                .dispatch_error_code(&proposal_id)
                .expect("read recovery reason")
                .as_deref(),
            Some("startup_artifact_claim_without_prepared_intent")
        );
        assert!(store
            .artifact_effect(&proposal_id)
            .expect("read artifact intent")
            .is_none());
        assert_eq!(
            store
                .dispatch_claim_id(&proposal_id)
                .expect("read preserved first claim")
                .as_deref(),
            Some(claim_id.as_str())
        );
        assert!(store
            .claim_dispatch(&proposal_id)
            .expect("a proven before-effect failure is retryable")
            .is_some());
        assert!(!target_path.exists());
    }

    #[test]
    fn bootstrap_imports_verified_legacy_yaml_daily_tasks_into_statestore_owner() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let manager = LifeModelManager::new(temp_dir.path().join("life-model").join("current"));
        let mut model = openlife_core::life_model::LifeModel::default_model();
        model
            .goals
            .daily
            .push(openlife_core::life_model::DailyGoal {
                name: "保留的旧 YAML 任务".into(),
                done: false,
                time_block: Some(openlife_core::life_model::TimeBlock {
                    start: "09:00".into(),
                    end: "10:00".into(),
                }),
                due_at: None,
                operation_id: None,
                operation_digest: None,
            });
        manager.save(&model).unwrap();

        let result =
            bootstrap_with_default_initialized_test_credentials(temp_dir.path().to_path_buf());
        let store = result.state.state_store.as_ref().expect("StateStore");
        let receipt = store
            .legacy_daily_task_shadow_receipt(false)
            .unwrap()
            .expect("legacy shadow receipt");
        let import_receipt = store
            .legacy_daily_task_import_receipt(false)
            .unwrap()
            .expect("legacy import receipt");

        assert_eq!(receipt.item_count, 1);
        assert!(receipt.deterministic);
        assert!(receipt.parity);
        assert!(receipt.rollback_rehearsed);
        assert_eq!(receipt.candidate_digest, receipt.repeated_read_digest);
        assert_eq!(receipt.candidate_digest, receipt.restored_digest);
        assert_eq!(import_receipt.item_count, 1);
        assert_eq!(
            import_receipt.candidate_digest,
            import_receipt.canonical_digest
        );
        assert_eq!(store.list_legacy_daily_task_shadow().unwrap().len(), 1);
        let tasks = store.get_product_daily_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "保留的旧 YAML 任务");
        assert_eq!(tasks[0].time_block_start.as_deref(), Some("09:00"));
        assert_eq!(tasks[0].time_block_end.as_deref(), Some("10:00"));
        assert_eq!(
            tasks[0].source_kind,
            openlife_core::state_store::StateSourceKind::LegacyLifeModelMigration
        );
        let persisted = result
            .state
            .life_model_manager
            .blocking_lock()
            .load()
            .unwrap();
        assert_eq!(persisted.goals.daily.len(), 1);
        assert_eq!(persisted.goals.daily[0].name, "保留的旧 YAML 任务");
        let encoded_receipt = serde_json::to_string(&receipt).unwrap();
        let encoded_import_receipt = serde_json::to_string(&import_receipt).unwrap();
        assert!(!encoded_receipt.contains("保留的旧 YAML 任务"));
        assert!(!encoded_import_receipt.contains("保留的旧 YAML 任务"));
    }

    #[test]
    fn bootstrap_imports_verified_legacy_state_history_and_preserves_migration_source() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let operation_id = uuid::Uuid::new_v4().hyphenated().to_string();
        let operation_digest =
            openlife_core::persistence_outbox::metadata_digest("bootstrap legacy state history");
        let memory_path = temp_dir.path().join("memory.db");
        {
            let memory = openlife_core::memory::MemoryStore::new(&memory_path).unwrap();
            drop(memory);
            let conn = rusqlite::Connection::open(&memory_path).unwrap();
            conn.execute(
                "INSERT INTO state_history (
                    dimension_name, value, unit, recorded_at, note,
                    operation_id, operation_digest
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    "专注度",
                    8.0,
                    "分",
                    chrono::Utc::now().to_rfc3339(),
                    "路演前",
                    operation_id,
                    operation_digest,
                ],
            )
            .unwrap();
        }

        let result =
            bootstrap_with_default_initialized_test_credentials(temp_dir.path().to_path_buf());
        let store = result.state.state_store.as_ref().expect("StateStore");
        let receipt = store
            .legacy_state_history_shadow_receipt(false)
            .unwrap()
            .expect("legacy state-history shadow receipt");
        let import_receipt = store
            .legacy_state_history_import_receipt(false)
            .unwrap()
            .expect("legacy state-history import receipt");
        let shadow = store.list_legacy_state_history_shadow().unwrap();

        assert_eq!(receipt.item_count, 1);
        assert!(receipt.deterministic);
        assert!(receipt.parity);
        assert!(receipt.rollback_rehearsed);
        assert_eq!(receipt.candidate_digest, receipt.repeated_read_digest);
        assert_eq!(receipt.candidate_digest, receipt.restored_digest);
        assert_eq!(shadow.len(), 1);
        assert_eq!(shadow[0].dimension_name, "专注度");
        assert_eq!(shadow[0].value, 8.0);
        assert_eq!(shadow[0].unit, "分");
        assert_eq!(shadow[0].note.as_deref(), Some("路演前"));
        assert_eq!(
            shadow[0].legacy_operation_id.as_deref(),
            Some(operation_id.as_str())
        );
        assert_eq!(import_receipt.item_count, 1);
        assert_eq!(
            import_receipt.candidate_digest,
            import_receipt.canonical_digest
        );
        let canonical_history = store.get_product_state_history("专注度", 10).unwrap();
        assert_eq!(canonical_history.len(), 1);
        assert_eq!(canonical_history[0].value, 8.0);
        assert_eq!(canonical_history[0].note.as_deref(), Some("路演前"));
        let legacy_history = result
            .state
            .memory_store
            .blocking_lock()
            .get_state_history("专注度", 10)
            .unwrap();
        assert_eq!(legacy_history.len(), 1);
        assert_eq!(legacy_history[0].note.as_deref(), Some("路演前"));
        let encoded_receipt = serde_json::to_string(&(receipt, import_receipt)).unwrap();
        for body in ["专注度", "分", "路演前"] {
            assert!(!encoded_receipt.contains(body));
        }
    }

    #[test]
    fn bootstrap_injects_stable_agent_run_key_and_rejects_key_mismatch() {
        use base64::Engine as _;

        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let first =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
        assert!(first.state.agent_run_store.is_some());
        let first_key = secrets
            .get(AGENT_RUN_RECEIPT_KEY_REF)
            .unwrap()
            .expect("bootstrap creates purpose-isolated AgentRun key");
        drop(first);

        let restarted =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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
        let mismatched =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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
        let first =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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

        let restarted =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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
    async fn restart_never_completes_open_provider_consent_epoch_from_empty_action_queue() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let preparation =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
        {
            let manager = preparation.state.life_model_manager.lock().await;
            let heuristic_store = preparation.state.heuristic_store.lock().await;
            let registry = openlife_core::agent::HSAssetAuthorityRegistry::new(
                manager.hs_asset_authority_registry_path(),
            )
            .expect("restart test HS authority registry");
            let revision = registry
                .authority(openlife_core::agent::HSAssetCategory::CollaborationGuidance)
                .expect("restart test HS authority")
                .revision;
            let scenario = registry
                .record_product_scenario(
                    openlife_core::agent::HSAssetCategory::CollaborationGuidance,
                    revision,
                    "test-fixture:provider-consent-restart",
                    openlife_core::agent::HSAssetOwner::AcceptedHsStore,
                    &[openlife_core::agent::BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING.into()],
                    openlife_core::agent::digest_string("provider-consent-restart-runtime-audit"),
                )
                .expect("restart test product receipt shape");
            let model = manager.load().expect("restart test LifeModel");
            let report = openlife_core::agent::complete_collaboration_guidance_cutover(
                &registry,
                &model,
                &heuristic_store,
                &scenario,
            )
            .expect("restart test HS cutover fixture");
            manager
                .save_hs_compatibility_view(&report.projection.yaml)
                .expect("restart test HS compatibility view");
        }
        drop(preparation);

        let first =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
        assert!(first.state.startup_warnings.is_empty());
        first.state.persistence_coordinator.seal();

        let mut config = first.state.config.lock().await.clone();
        config.llm.provider = "openai".into();
        config.llm.openai_base = "http://127.0.0.1:9/v1".into();
        config.llm.openai_key = "sk-provider-restart-test".into();
        config.llm.chat_model = "gpt-provider-restart-test".into();
        config.prefer_local_model = false;
        config.system.network_policy.enabled = true;
        config.system.network_policy.default_decision = "ask".into();
        first.state.replace_provider_runtime_config(config).await;

        let operation_id = uuid::Uuid::new_v4().hyphenated().to_string();
        let initial = crate::main_chat_turn_runtime::OpenLifeTurnRuntime::new(&first.state)
            .run_buffered(crate::main_chat_turn_runtime::OpenLifeTurnInput {
                operation_id: operation_id.clone(),
                session_id: "provider-consent-restart-chat".into(),
                messages: vec![openlife_core::llm::ChatMessage {
                    role: "user".into(),
                    content: "Draft a concise roadshow opening.".into(),
                }],
                selected_skill_id: None,
                stream_mode: crate::main_chat_turn_runtime::MainChatTurnStreamMode::Buffered,
            })
            .await
            .expect("initial provider turn stages consent");
        let proposal_id = initial.terminal.proposals[0]
            .strip_prefix("proposal:")
            .unwrap_or(&initial.terminal.proposals[0])
            .to_string();
        crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &first.state)
            .await
            .expect("accept exact provider consent");

        let waiting_session = first
            .state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await
            .load_session(&operation_id)
            .expect("load waiting task")
            .expect("waiting task exists");
        let replay_admission = crate::terminal_owner_write_gateway::issue_terminal_owner_provider_consent_replay_admission(
            &first.state,
            &waiting_session,
            &proposal_id,
        )
        .await
        .expect("issue exact provider replay admission");
        let replay_epoch = first
            .state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .open_terminal_owner_replay_epoch_from_admission(&replay_admission)
            .expect("open provider replay epoch");
        let replay_run_id = replay_epoch.run_id().to_string();
        assert_eq!(replay_epoch.generation(), 2);
        let replay_start = first
            .state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .latest_terminal_owner_replay_start(&operation_id, &replay_run_id)
            .expect("load replay start")
            .expect("replay start exists");
        assert_eq!(
            replay_start.payload["replayCause"],
            "accepted_provider_network_consent"
        );
        crate::terminal_owner_write_gateway::write_task_session(
            &first.state,
            &operation_id,
            crate::terminal_owner_write_gateway::TaskSessionWrite::ResumeAfterResolvedBlocker(
                format!("proposal:{proposal_id}"),
            ),
        )
        .await
        .expect("simulate continuation task activation");
        crate::terminal_owner_write_gateway::begin_main_chat_agent_run_replay(
            &first.state,
            &replay_run_id,
            &operation_id,
        )
        .await
        .expect("simulate continuation run activation");
        assert!(first
            .state
            .main_chat_action_queue_store
            .as_ref()
            .expect("action queue")
            .lock()
            .await
            .list_for_session(&operation_id)
            .expect("list actions")
            .is_empty());
        drop(first);

        let restarted =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
        assert!(
            crate::main_chat_turn_runtime::reconcile_orphaned_openlife_replay_epoch_after_restart(
                &restarted.state,
                &operation_id,
                &replay_run_id,
            )
            .await
            .expect("reconcile orphaned provider continuation")
        );
        restarted.state.persistence_coordinator.seal();

        let recovered_task = restarted
            .state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task store")
            .lock()
            .await
            .load_session(&operation_id)
            .expect("load recovered task")
            .expect("recovered task exists");
        assert_eq!(
            recovered_task.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
        );
        let final_event = restarted
            .state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .terminal_owner_final_event(&operation_id)
            .expect("load final")
            .expect("final exists");
        assert_eq!(final_event.payload["status"], "failed");
        assert_eq!(final_event.payload["toolInvoked"], false);
        assert_eq!(final_event.payload["modelInvoked"], false);
        assert_eq!(
            final_event.payload["providerInvocationStatus"],
            "not_attempted"
        );
        assert_eq!(final_event.payload["requiresProvider"], true);
        assert_eq!(final_event.payload["requiresToolLoop"], false);
        let events = restarted
            .state
            .main_chat_agent_event_store
            .as_ref()
            .expect("event store")
            .lock()
            .await
            .list(&operation_id, 0, 250)
            .expect("list recovered events");
        let restart_failure = events
            .iter()
            .find(|event| event.source == "bootstrap.orphan_open_replay_epoch")
            .expect("startup failure receipt exists");
        assert_eq!(
            restart_failure.payload["providerAttemptState"],
            "not_attempted"
        );
        assert_eq!(
            restart_failure.payload["remoteProviderState"],
            "not_attempted"
        );
    }

    #[tokio::test]
    async fn startup_pending_final_agent_run_repair_preserves_task_owner_head() {
        use openlife_core::agent::main_chat_agent_v1::{
            AgentTaskSessionDraft, AgentTaskSessionStatus, MainChatAgentStrategy,
        };

        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let result =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
        let operation_id = uuid::Uuid::new_v4().to_string();
        let chat_session_id = format!("restart-pending-final:{operation_id}");
        let user_goal = "Resume one accepted provider consent without replaying prior tools.";
        result
            .state
            .memory_store
            .lock()
            .await
            .create_chat_session(&chat_session_id, "Pending restart fixture")
            .unwrap();
        let task = result
            .state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_session_with_id(
                operation_id.clone(),
                AgentTaskSessionDraft {
                    chat_session_id: chat_session_id.clone(),
                    user_goal: user_goal.into(),
                    selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                    current_plan_summary: None,
                    context_snapshot_refs: Vec::new(),
                },
            )
            .unwrap();
        let canonical_message = result
            .state
            .memory_store
            .lock()
            .await
            .save_message_idempotent_with_proof(
                &chat_session_id,
                &openlife_core::llm::ChatMessage {
                    role: "user".into(),
                    content: user_goal.into(),
                },
                &operation_id,
            )
            .unwrap();
        let admission = {
            let store = result
                .state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            store
                .bind_session_canonical_user_message(
                    &task.id,
                    &canonical_message.receipt().canonical_ref,
                    user_goal,
                )
                .unwrap();
            store
                .issue_terminal_owner_epoch_admission(&task.id, &operation_id, canonical_message)
                .unwrap()
        };
        let mut run = openlife_core::agent::AgentRun::new_chat_run(&chat_session_id, user_goal);
        run.id = operation_id.clone();
        run.task_id = task.id.clone();
        result
            .state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        let epoch = result
            .state
            .main_chat_agent_event_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .open_terminal_owner_epoch_from_admission(admission)
            .unwrap();
        let task_before_projection = {
            let store = result
                .state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            store
                .set_pending_blockers(&task.id, vec!["proposal:provider-consent".into()])
                .unwrap();
            store.mark_waiting_permission(&task.id).unwrap()
        };
        assert_eq!(
            task_before_projection.status,
            AgentTaskSessionStatus::WaitingPermission
        );
        let owner_before_projection = result
            .state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_owner_head(&task.id)
            .unwrap()
            .unwrap();
        let final_event = {
            let event_store = result
                .state
                .main_chat_agent_event_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            event_store
                .begin_terminal_owner_seal(&task.id, &run.id, epoch.generation())
                .unwrap();
            event_store
                .append_terminal_final_and_seal(
                    crate::main_chat_event_stream::MainChatTerminalFinalizationInput {
                        task_session_id: task.id.clone(),
                        run_id: run.id.clone(),
                        epoch_generation: epoch.generation(),
                        delivery_id: format!("pending-final:{}", run.id),
                        expected_task_owner_revision: owner_before_projection.revision(),
                        expected_task_owner_digest: owner_before_projection.digest().to_string(),
                        status: "completed_with_pending_items".into(),
                    },
                )
                .unwrap()
        };
        {
            let store = result.state.agent_run_store.as_ref().unwrap().lock().await;
            let mut pre_fix_projection = store.get_run(&run.id).unwrap().unwrap();
            pre_fix_projection.status = openlife_core::agent::AgentRunStatus::Completed;
            pre_fix_projection.finished_at = Some(chrono::Utc::now());
            store.update_run(&pre_fix_projection).unwrap();
        }
        let run_before_repair = result
            .state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run.id)
            .unwrap()
            .unwrap();

        project_startup_final_delivery_receipt(
            &result.state,
            &run_before_repair,
            &task_before_projection,
            &final_event,
        )
        .await
        .expect("repair only the stale AgentRun projection");

        let repaired_run = result
            .state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            repaired_run.status,
            openlife_core::agent::AgentRunStatus::WaitingPermission
        );
        assert!(repaired_run.finished_at.is_none());
        let owner_after_projection = result
            .state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_owner_head(&task.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            owner_after_projection.revision(),
            owner_before_projection.revision()
        );
        assert_eq!(
            owner_after_projection.digest(),
            owner_before_projection.digest()
        );
    }

    #[tokio::test]
    async fn restart_projects_interrupted_final_delivery_to_failed_run_and_task_at_event_time() {
        use openlife_core::agent::main_chat_agent_v1::{
            AgentTaskSessionDraft, MainChatAgentStrategy,
        };

        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let first =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
        let operation_id = uuid::Uuid::new_v4().to_string();
        let chat_session_id = format!("restart-interrupted-final:{operation_id}");
        let user_goal = "Recover an interrupted final delivery without inventing cancellation.";
        first
            .state
            .memory_store
            .lock()
            .await
            .create_chat_session(&chat_session_id, "Interrupted restart fixture")
            .unwrap();
        let task = first
            .state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_session_with_id(
                operation_id.clone(),
                AgentTaskSessionDraft {
                    chat_session_id: chat_session_id.clone(),
                    user_goal: user_goal.into(),
                    selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                    current_plan_summary: None,
                    context_snapshot_refs: Vec::new(),
                },
            )
            .unwrap();
        let canonical_message = first
            .state
            .memory_store
            .lock()
            .await
            .save_message_idempotent_with_proof(
                &chat_session_id,
                &openlife_core::llm::ChatMessage {
                    role: "user".into(),
                    content: user_goal.into(),
                },
                &operation_id,
            )
            .unwrap();
        let admission = {
            let store = first
                .state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            store
                .bind_session_canonical_user_message(
                    &task.id,
                    &canonical_message.receipt().canonical_ref,
                    user_goal,
                )
                .unwrap();
            store
                .issue_terminal_owner_epoch_admission(&task.id, &operation_id, canonical_message)
                .unwrap()
        };
        let mut run = openlife_core::agent::AgentRun::new_chat_run(&chat_session_id, user_goal);
        run.id = operation_id.clone();
        run.task_id = task.id.clone();
        first
            .state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        let head = first
            .state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_owner_head(&task.id)
            .unwrap()
            .unwrap();
        let interrupted_event = {
            let event_store = first
                .state
                .main_chat_agent_event_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            let epoch = event_store
                .open_terminal_owner_epoch_from_admission(admission)
                .unwrap();
            event_store
                .begin_terminal_owner_seal(&task.id, &run.id, epoch.generation())
                .unwrap();
            event_store
                .append_terminal_final_and_seal(
                    crate::main_chat_event_stream::MainChatTerminalFinalizationInput {
                        task_session_id: task.id.clone(),
                        run_id: run.id.clone(),
                        epoch_generation: epoch.generation(),
                        delivery_id: format!("interrupted-final:{}", run.id),
                        expected_task_owner_revision: head.revision(),
                        expected_task_owner_digest: head.digest().to_string(),
                        status: "interrupted".into(),
                    },
                )
                .unwrap()
        };
        drop(first);

        let restarted =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
        assert_eq!(
            reconcile_startup_orphaned_main_chat_runs(&restarted.state)
                .await
                .expect("restart projects exact interrupted final delivery"),
            1
        );
        let recovered_run = restarted
            .state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            recovered_run.status,
            openlife_core::agent::AgentRunStatus::Failed
        );
        assert_eq!(
            recovered_run.finished_at,
            Some(interrupted_event.created_at)
        );
        let recovered_task = restarted
            .state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .load_session(&task.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            recovered_task.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
        );
    }

    #[tokio::test]
    async fn restart_repairs_both_directions_of_partial_terminal_projection() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = TestSecretStore::default();
        let first =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
        // Seed the durable terminal through the same release-like admission
        // used by normal product effects. The restart below is the only phase
        // that may use startup reconciliation while the coordinator is still
        // Initializing.
        first.state.persistence_coordinator.seal();
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

        let restarted =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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
        let first =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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

        let restarted =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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
        let first =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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

        let restarted =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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
        let first =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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

        let restarted =
            bootstrap_with_initialized_test_credentials(directory.path().to_path_buf(), &secrets);
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
