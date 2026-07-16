use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use openlife_core::agent::main_chat_agent_v1::{
    ActionQueueStore, ActionReplayClaim, AgentTaskSession, AgentTaskSessionStore, ExecutionAction,
    ExecutionPolicyDecision, ExecutionQueueStatus, ExecutionTranscriptEntry,
    ExecutionTranscriptEntryDraft, QueuedExecutionAction, VerifiedTerminalOwnerTransitionReceipt,
};
use openlife_core::agent::proposal_store::TerminalOwnerOriginBinding;
use openlife_core::agent::review_workflow::{
    ClaimedReviewAcceptanceSnapshot, MaterializedReviewAcceptanceSnapshot,
};
use openlife_core::agent::{
    AgentRun, MemoryLifecycleAcceptanceInput, MemoryLifecycleStore, ProposalStore, ProposalType,
    ReviewWorkflow,
};
use openlife_core::persistence_outbox::CanonicalMutationReceipt;

use crate::main_chat_event_stream::{
    MainChatAgentDurableEvent, MainChatAgentEventStore, TerminalOwnerSealState,
};
use crate::main_chat_replay_contract::DurableMainChatReplayExecutionEnvelope;
use crate::AppState;

fn terminal_owner_task_fences() -> &'static Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>> {
    static FENCES: OnceLock<Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    FENCES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn terminal_owner_task_fence(task_session_id: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut fences = terminal_owner_task_fences()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    fences.retain(|_, fence| fence.strong_count() > 0);
    if let Some(fence) = fences.get(task_session_id).and_then(Weak::upgrade) {
        return fence;
    }
    let fence = Arc::new(tokio::sync::Mutex::new(()));
    fences.insert(task_session_id.to_string(), Arc::downgrade(&fence));
    fence
}

pub(crate) async fn acquire_terminal_owner_task_fence(
    task_session_id: &str,
) -> tokio::sync::OwnedMutexGuard<()> {
    terminal_owner_task_fence(task_session_id)
        .lock_owned()
        .await
}

async fn acquire_open_turn_write_fence(
    state: &Arc<AppState>,
    task_session_id: &str,
) -> Result<tokio::sync::OwnedMutexGuard<()>, String> {
    let fence = acquire_terminal_owner_task_fence(task_session_id).await;
    if let Some(event_store) = state.main_chat_agent_event_store.as_ref() {
        let epoch = event_store
            .lock()
            .await
            .terminal_owner_epoch(task_session_id)
            .map_err(|error| format!("load terminal owner epoch before write failed: {error}"))?;
        if epoch.is_some_and(|epoch| epoch.state() != TerminalOwnerSealState::Open) {
            return Err("terminal_owner_write_rejected_after_sealing".into());
        }
    }
    Ok(fence)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalOwnerReplayCause {
    AutomaticRetry,
    AcceptedToolPermission,
}

impl TerminalOwnerReplayCause {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AutomaticRetry => "automatic_retry",
            Self::AcceptedToolPermission => "accepted_tool_permission",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalOwnerReplayEpochAuthority {
    VerifiedByTerminalOwnerWriteGateway,
}

/// One-shot, non-Serde authority for opening the next execution generation of
/// an already SEALED task. A replay is therefore never a collection of writes
/// smuggled through the old terminal boundary: it becomes a new OPEN epoch
/// under the same TurnRuntime and must produce its own final before returning.
pub(crate) struct TerminalOwnerReplayEpochAdmission {
    admission_id: String,
    task_session_id: String,
    run_id: String,
    action_id: String,
    prior_epoch_id: String,
    prior_epoch_generation: u64,
    prior_final_event_id: String,
    canonical_user_message_ref: String,
    canonical_user_message_digest: String,
    cause: TerminalOwnerReplayCause,
    cause_ref: String,
    retry_proof: openlife_core::agent::tool_gateway::ToolAutomaticRetryProof,
    authority: TerminalOwnerReplayEpochAuthority,
}

impl TerminalOwnerReplayEpochAdmission {
    pub(crate) fn admission_id(&self) -> &str {
        &self.admission_id
    }

    pub(crate) fn task_session_id(&self) -> &str {
        &self.task_session_id
    }

    pub(crate) fn run_id(&self) -> &str {
        &self.run_id
    }

    pub(crate) fn action_id(&self) -> &str {
        &self.action_id
    }

    pub(crate) fn prior_epoch_id(&self) -> &str {
        &self.prior_epoch_id
    }

    pub(crate) fn prior_epoch_generation(&self) -> u64 {
        self.prior_epoch_generation
    }

    pub(crate) fn prior_final_event_id(&self) -> &str {
        &self.prior_final_event_id
    }

    pub(crate) fn canonical_user_message_ref(&self) -> &str {
        &self.canonical_user_message_ref
    }

    pub(crate) fn canonical_user_message_digest(&self) -> &str {
        &self.canonical_user_message_digest
    }

    pub(crate) fn cause(&self) -> TerminalOwnerReplayCause {
        self.cause
    }

    pub(crate) fn cause_ref(&self) -> &str {
        &self.cause_ref
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.authority != TerminalOwnerReplayEpochAuthority::VerifiedByTerminalOwnerWriteGateway
            || self.admission_id.trim().is_empty()
            || self.task_session_id.trim().is_empty()
            || self.run_id.trim().is_empty()
            || self.action_id.trim().is_empty()
            || self.prior_epoch_id.trim().is_empty()
            || self.prior_epoch_generation == 0
            || self.prior_final_event_id.trim().is_empty()
            || self.canonical_user_message_ref.trim().is_empty()
            || self.canonical_user_message_digest.trim().is_empty()
            || self.cause_ref.trim().is_empty()
        {
            return Err("terminal_owner_replay_epoch_admission_invalid".into());
        }
        Ok(())
    }

    pub(crate) fn into_retry_proof(
        self,
    ) -> openlife_core::agent::tool_gateway::ToolAutomaticRetryProof {
        self.retry_proof
    }
}

fn final_owner_head(final_event: &MainChatAgentDurableEvent) -> Result<(u64, String), String> {
    let revision = final_event
        .payload
        .get("taskOwnerRevision")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "terminal_owner_replay_final_revision_missing".to_string())?;
    let digest = final_event
        .payload
        .get("taskOwnerDigest")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "terminal_owner_replay_final_digest_missing".to_string())?;
    Ok((revision, digest.to_string()))
}

fn successor_owner_head(successor: &MainChatAgentDurableEvent) -> Result<(u64, String), String> {
    let revision = successor
        .payload
        .get("afterOwnerRevision")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "terminal_owner_replay_successor_revision_missing".to_string())?;
    let digest = successor
        .payload
        .get("afterOwnerDigest")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "terminal_owner_replay_successor_digest_missing".to_string())?;
    Ok((revision, digest.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn issue_terminal_owner_replay_epoch_admission(
    state: &Arc<AppState>,
    session: &AgentTaskSession,
    action: &QueuedExecutionAction,
    envelope: &DurableMainChatReplayExecutionEnvelope,
    cause: TerminalOwnerReplayCause,
    action_bound_permission: Option<
        &openlife_core::tool_permissions::ActionBoundToolPermissionAuthorization,
    >,
    retry_proof: openlife_core::agent::tool_gateway::ToolAutomaticRetryProof,
) -> Result<TerminalOwnerReplayEpochAdmission, String> {
    let _fence = acquire_terminal_owner_task_fence(&session.id).await;
    let authority = action
        .replay_authority
        .as_ref()
        .ok_or_else(|| "terminal_owner_replay_canonical_authority_missing".to_string())?;
    if action.session_id != session.id
        || envelope.task_session_id != session.id
        || envelope.queue_action_id != action.id
        || envelope.run_id != authority.run_id()
        || authority.task_session_id() != session.id
        || authority.action_id() != action.id
        || authority.queue_action_type() != envelope.queue_action_type
        || authority.executor_action_id() != envelope.executor_action_id
        || authority.executor_action_type() != envelope.executor_action_type
        || authority.requested_target() != envelope.requested_target
        || authority.resolved_target() != envelope.resolved_target
        || authority.manifest_id() != envelope.manifest_id
        || authority.manifest_name() != envelope.manifest_name
        || authority.manifest_source() != envelope.manifest_source
        || authority.manifest_contract_digest() != envelope.manifest_contract_digest
        || authority.input_hash() != envelope.input_hash
        || authority.input_length_bytes() != envelope.input_length_bytes
    {
        return Err("terminal_owner_replay_execution_authority_mismatch".into());
    }

    let (epoch, final_event) = {
        let event_store = state
            .main_chat_agent_event_store
            .as_ref()
            .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?
            .lock()
            .await;
        let epoch = event_store
            .terminal_owner_epoch(&session.id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "terminal_owner_replay_epoch_missing".to_string())?;
        if epoch.state() != TerminalOwnerSealState::Sealed || epoch.run_id() != envelope.run_id {
            return Err("terminal_owner_replay_requires_sealed_epoch".into());
        }
        let final_event = event_store
            .terminal_owner_final_event(&session.id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "terminal_owner_replay_final_event_missing".to_string())?;
        if epoch.final_event_id() != Some(final_event.event_id.as_str()) {
            return Err("terminal_owner_replay_final_event_identity_mismatch".into());
        }
        (epoch, final_event)
    };

    let expected_owner = match cause {
        TerminalOwnerReplayCause::AutomaticRetry => {
            if action_bound_permission.is_some() {
                return Err("terminal_owner_retry_unexpected_permission_authority".into());
            }
            final_owner_head(&final_event)?
        }
        TerminalOwnerReplayCause::AcceptedToolPermission => {
            let authorization = action_bound_permission.ok_or_else(|| {
                "terminal_owner_permission_replay_authorization_missing".to_string()
            })?;
            if authorization.proposal_id.trim().is_empty()
                || authorization.scope.binding_digest() != authorization.scope_digest
                || authorization.scope.queue_action_type != envelope.queue_action_type
                || authorization.scope.requested_target != envelope.requested_target
                || authorization.scope.resolved_target != envelope.resolved_target
                || authorization.scope.tool_name != envelope.manifest_name
                || authorization.scope.source != envelope.manifest_source
                || authorization.scope.input_hash != envelope.input_hash
                || authorization.scope.input_length_bytes != envelope.input_length_bytes
            {
                return Err("terminal_owner_permission_replay_scope_mismatch".into());
            }
            let proposal_store = state
                .proposal_store
                .as_ref()
                .ok_or_else(|| "proposal_store_unavailable".to_string())?
                .lock()
                .await;
            let proposal = proposal_store
                .get_proposal(&authorization.proposal_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "terminal_owner_permission_replay_proposal_missing".to_string())?;
            let origin = proposal_store
                .terminal_owner_origin_binding(&authorization.proposal_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "terminal_owner_permission_replay_origin_missing".to_string())?;
            let dispatch_state = proposal_store
                .dispatch_state(&authorization.proposal_id)
                .map_err(|error| error.to_string())?;
            if proposal.status != openlife_core::agent::ProposalStatus::Accepted
                || proposal.proposal_type != ProposalType::ToolPermission
                || dispatch_state.as_deref() != Some("confirmed")
                || origin.task_session_id() != session.id
                || origin.run_id() != envelope.run_id
                || origin.epoch_id() != epoch.epoch_id()
                || origin.epoch_generation() != epoch.generation()
                || origin.canonical_user_message_ref() != epoch.canonical_user_message_ref()
                || origin.canonical_user_message_digest() != epoch.canonical_user_message_digest()
            {
                return Err("terminal_owner_permission_replay_origin_mismatch".into());
            }
            drop(proposal_store);
            let successor = state
                .main_chat_agent_event_store
                .as_ref()
                .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?
                .lock()
                .await
                .get_immutable_event(
                    &session.id,
                    "terminal_owner.successor_confirmed",
                    &format!("successor:{}", authorization.proposal_id),
                )
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "terminal_owner_permission_replay_successor_missing".to_string())?;
            if successor
                .payload
                .get("causeRef")
                .and_then(serde_json::Value::as_str)
                != Some(authorization.proposal_id.as_str())
                || successor
                    .payload
                    .get("finalEventId")
                    .and_then(serde_json::Value::as_str)
                    != Some(final_event.event_id.as_str())
            {
                return Err("terminal_owner_permission_replay_successor_mismatch".into());
            }
            successor_owner_head(&successor)?
        }
    };
    let owner_head = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?
        .lock()
        .await
        .canonical_owner_head(&session.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "terminal_owner_replay_task_owner_missing".to_string())?;
    if owner_head.revision() != expected_owner.0 || owner_head.digest() != expected_owner.1 {
        return Err("terminal_owner_replay_task_owner_drift".into());
    }

    let cause_ref = match cause {
        TerminalOwnerReplayCause::AutomaticRetry => action.id.clone(),
        TerminalOwnerReplayCause::AcceptedToolPermission => action_bound_permission
            .expect("permission replay checked above")
            .proposal_id
            .clone(),
    };
    Ok(TerminalOwnerReplayEpochAdmission {
        admission_id: format!("terminal-replay-admission:{}", uuid::Uuid::new_v4()),
        task_session_id: session.id.clone(),
        run_id: envelope.run_id.clone(),
        action_id: action.id.clone(),
        prior_epoch_id: epoch.epoch_id().to_string(),
        prior_epoch_generation: epoch.generation(),
        prior_final_event_id: final_event.event_id,
        canonical_user_message_ref: epoch.canonical_user_message_ref().to_string(),
        canonical_user_message_digest: epoch.canonical_user_message_digest().to_string(),
        cause,
        cause_ref,
        retry_proof,
        authority: TerminalOwnerReplayEpochAuthority::VerifiedByTerminalOwnerWriteGateway,
    })
}

pub(crate) enum TaskSessionWrite {
    SetPendingBlockers(Vec<String>),
    SetPendingBlockersAndTransition {
        blockers: Vec<String>,
        transition: TaskSessionTransition,
    },
    Complete(String),
    Fail(String),
    Resume,
    Cancel(String),
    RecordActionQueueId(String),
    RecordContextSnapshotRef(String),
    UpdatePlanSummary(Option<String>),
}

pub(crate) enum TaskSessionTransition {
    Complete(String),
    Block(String),
    Fail(String),
    WaitingPermission,
}

fn apply_task_session_write(
    store: &AgentTaskSessionStore,
    task_session_id: &str,
    write: TaskSessionWrite,
) -> Result<AgentTaskSession, String> {
    match write {
        TaskSessionWrite::SetPendingBlockers(blockers) => {
            store.set_pending_blockers(task_session_id, blockers)
        }
        TaskSessionWrite::SetPendingBlockersAndTransition {
            blockers,
            transition,
        } => match store.set_pending_blockers(task_session_id, blockers) {
            Ok(_) => match transition {
                TaskSessionTransition::Complete(summary) => {
                    store.complete_session(task_session_id, &summary)
                }
                TaskSessionTransition::Block(summary) => {
                    store.block_session(task_session_id, &summary)
                }
                TaskSessionTransition::Fail(summary) => {
                    store.fail_session(task_session_id, &summary)
                }
                TaskSessionTransition::WaitingPermission => {
                    store.mark_waiting_permission(task_session_id)
                }
            },
            Err(error) => Err(error),
        },
        TaskSessionWrite::Complete(summary) => store.complete_session(task_session_id, &summary),
        TaskSessionWrite::Fail(summary) => store.fail_session(task_session_id, &summary),
        TaskSessionWrite::Resume => store.resume_session(task_session_id),
        TaskSessionWrite::Cancel(summary) => store.cancel_session(task_session_id, &summary),
        TaskSessionWrite::RecordActionQueueId(action_id) => {
            store.record_action_queue_id(task_session_id, &action_id)
        }
        TaskSessionWrite::RecordContextSnapshotRef(context_snapshot_ref) => {
            store.record_context_snapshot_ref(task_session_id, &context_snapshot_ref)
        }
        TaskSessionWrite::UpdatePlanSummary(summary) => {
            store.update_plan_summary(task_session_id, summary)
        }
    }
    .map_err(|error| error.to_string())
}

pub(crate) async fn write_task_session(
    state: &Arc<AppState>,
    task_session_id: &str,
    write: TaskSessionWrite,
) -> Result<AgentTaskSession, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    let store = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?
        .lock()
        .await;
    apply_task_session_write(&store, task_session_id, write)
}

pub(crate) async fn write_task_session_with_commit_admission(
    state: &Arc<AppState>,
    task_session_id: &str,
    write: TaskSessionWrite,
    execution_epoch: &crate::main_chat_cancellation::MainChatExecutionEpoch,
) -> Result<AgentTaskSession, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    let store = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?
        .lock()
        .await;
    let commit_permit = execution_epoch
        .begin_canonical_commit("task_session", task_session_id)
        .map_err(|rejection| {
            format!("task session write rejected after cancellation: {rejection:?}")
        })?;
    match apply_task_session_write(&store, task_session_id, write) {
        Ok(session) => {
            commit_permit.finish_committed();
            Ok(session)
        }
        Err(error) => {
            commit_permit.finish_failed();
            Err(error)
        }
    }
}

pub(crate) async fn append_task_transcript(
    state: &Arc<AppState>,
    draft: ExecutionTranscriptEntryDraft,
) -> Result<ExecutionTranscriptEntry, String> {
    let _fence = acquire_open_turn_write_fence(state, &draft.session_id).await?;
    state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?
        .lock()
        .await
        .append_transcript_entry(draft)
        .map_err(|error| error.to_string())
}

pub(crate) async fn update_agent_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let _fence = acquire_open_turn_write_fence(state, &run.task_id).await?;
    state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?
        .lock()
        .await
        .update_run(run)
        .map_err(|error| error.to_string())
}

pub(crate) async fn update_agent_run_after_review_reconciliation(
    state: &Arc<AppState>,
    proposal_id: &str,
    run: &AgentRun,
) -> Result<(), String> {
    let _fence = acquire_terminal_owner_task_fence(&run.task_id).await;
    let epoch = match state.main_chat_agent_event_store.as_ref() {
        Some(store) => store
            .lock()
            .await
            .terminal_owner_epoch(&run.task_id)
            .map_err(|error| error.to_string())?,
        None => None,
    };
    if let Some(epoch) = epoch {
        if epoch.state() == TerminalOwnerSealState::Sealing {
            return Err("terminal_owner_agent_run_projection_rejected_while_sealing".into());
        }
        if epoch.state() == TerminalOwnerSealState::Sealed {
            let proposal_store = state
                .proposal_store
                .as_ref()
                .ok_or_else(|| "proposal_store_unavailable".to_string())?
                .lock()
                .await;
            let origin = proposal_store
                .terminal_owner_origin_binding(proposal_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "terminal_owner_agent_run_projection_origin_missing".to_string())?;
            if origin.task_session_id() != run.task_id || origin.run_id() != run.id {
                return Err("terminal_owner_agent_run_projection_origin_mismatch".into());
            }
            let proposal = proposal_store
                .get_proposal(proposal_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    "terminal_owner_agent_run_projection_proposal_missing".to_string()
                })?;
            let dispatch_state = proposal_store
                .dispatch_state(proposal_id)
                .map_err(|error| error.to_string())?;
            drop(proposal_store);
            if proposal.status == openlife_core::agent::ProposalStatus::Accepted {
                if dispatch_state.as_deref() != Some("confirmed") {
                    return Err("terminal_owner_agent_run_projection_effect_not_confirmed".into());
                }
                let successor = state
                    .main_chat_agent_event_store
                    .as_ref()
                    .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?
                    .lock()
                    .await
                    .get_immutable_event(
                        &run.task_id,
                        "terminal_owner.successor_confirmed",
                        &format!("successor:{proposal_id}"),
                    )
                    .map_err(|error| error.to_string())?;
                if successor.is_none() {
                    return Err("terminal_owner_agent_run_projection_successor_missing".into());
                }
            } else if !matches!(
                proposal.status,
                openlife_core::agent::ProposalStatus::Pending
                    | openlife_core::agent::ProposalStatus::Postponed
                    | openlife_core::agent::ProposalStatus::Edited
                    | openlife_core::agent::ProposalStatus::Rejected
                    | openlife_core::agent::ProposalStatus::Expired
            ) {
                return Err("terminal_owner_agent_run_projection_status_unproven".into());
            }
        }
    }
    state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?
        .lock()
        .await
        .update_run(run)
        .map_err(|error| error.to_string())
}

async fn agent_run_task_id(state: &Arc<AppState>, run_id: &str) -> Result<String, String> {
    state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?
        .lock()
        .await
        .get_run(run_id)
        .map_err(|error| error.to_string())?
        .map(|run| run.task_id)
        .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))
}

async fn reject_agent_run_lifecycle_race_with_sealing(
    state: &Arc<AppState>,
    task_session_id: &str,
) -> Result<(), String> {
    let Some(event_store) = state.main_chat_agent_event_store.as_ref() else {
        return Ok(());
    };
    let epoch = event_store
        .lock()
        .await
        .terminal_owner_epoch(task_session_id)
        .map_err(|error| error.to_string())?;
    if epoch.is_some_and(|epoch| epoch.state() == TerminalOwnerSealState::Sealing) {
        return Err("agent_run_lifecycle_mutation_rejected_while_terminal_owner_sealing".into());
    }
    Ok(())
}

pub(crate) async fn delete_agent_run_with_tombstone(
    state: &Arc<AppState>,
    run_id: &str,
    reason: Option<&str>,
) -> Result<CanonicalMutationReceipt, String> {
    let task_session_id = agent_run_task_id(state, run_id).await?;
    let _fence = acquire_terminal_owner_task_fence(&task_session_id).await;
    reject_agent_run_lifecycle_race_with_sealing(state, &task_session_id).await?;
    let store = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?
        .lock()
        .await;
    let current = store
        .get_run(run_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))?;
    if current.task_id != task_session_id {
        return Err("agent_run_lifecycle_task_binding_changed".into());
    }
    store
        .delete_run_with_tombstone(run_id, reason)
        .map_err(|error| error.to_string())
}

pub(crate) async fn restore_agent_run_with_receipt(
    state: &Arc<AppState>,
    run_id: &str,
) -> Result<CanonicalMutationReceipt, String> {
    let task_session_id = agent_run_task_id(state, run_id).await?;
    let _fence = acquire_terminal_owner_task_fence(&task_session_id).await;
    reject_agent_run_lifecycle_race_with_sealing(state, &task_session_id).await?;
    let store = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?
        .lock()
        .await;
    let current = store
        .get_run(run_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))?;
    if current.task_id != task_session_id {
        return Err("agent_run_lifecycle_task_binding_changed".into());
    }
    store
        .restore_run_with_receipt(run_id)
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn append_runtime_event(
    state: &Arc<AppState>,
    task_session_id: &str,
    run_id: &str,
    event_type: &str,
    object_type: &str,
    object_id: impl Into<String>,
    source: impl Into<String>,
    payload: serde_json::Value,
) -> Result<MainChatAgentDurableEvent, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
        state,
        task_session_id,
        run_id,
        event_type,
        object_type,
        object_id,
        source,
        payload,
    )
    .await
}

pub(crate) async fn enqueue_action(
    state: &Arc<AppState>,
    task_session_id: &str,
    action: ExecutionAction,
    policy: ExecutionPolicyDecision,
) -> Result<QueuedExecutionAction, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?
        .lock()
        .await
        .enqueue(task_session_id, action, policy)
        .map_err(|error| error.to_string())
}

pub(crate) async fn claim_action_replay(
    state: &Arc<AppState>,
    task_session_id: &str,
    action_id: &str,
    expected_status: ExecutionQueueStatus,
    expected_revision: u64,
    owner_execution_id: &str,
    retry_proof: openlife_core::agent::tool_gateway::ToolAutomaticRetryProof,
) -> Result<ActionReplayClaim, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?
        .lock()
        .await
        .claim_replay_with_automatic_retry_proof(
            action_id,
            expected_status,
            expected_revision,
            owner_execution_id,
            retry_proof,
        )
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn transition_claimed_action_replay(
    state: &Arc<AppState>,
    task_session_id: &str,
    action_id: &str,
    claim_id: &str,
    expected_status: ExecutionQueueStatus,
    expected_revision: u64,
    status: ExecutionQueueStatus,
    metadata: Option<serde_json::Value>,
) -> Result<QueuedExecutionAction, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?
        .lock()
        .await
        .transition_claimed_replay(
            action_id,
            claim_id,
            expected_status,
            expected_revision,
            status,
            metadata,
        )
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn fail_and_release_action_replay_before_dispatch(
    state: &Arc<AppState>,
    task_session_id: &str,
    action_id: &str,
    claim_id: &str,
    expected_status: ExecutionQueueStatus,
    expected_revision: u64,
    safe_error: &str,
    metadata: Option<serde_json::Value>,
) -> Result<QueuedExecutionAction, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?
        .lock()
        .await
        .fail_and_release_replay_claim_before_dispatch(
            action_id,
            claim_id,
            expected_status,
            expected_revision,
            safe_error,
            metadata,
        )
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn fail_claimed_action_replay(
    state: &Arc<AppState>,
    task_session_id: &str,
    action_id: &str,
    claim_id: &str,
    expected_status: ExecutionQueueStatus,
    expected_revision: u64,
    safe_error: &str,
    metadata: Option<serde_json::Value>,
) -> Result<QueuedExecutionAction, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?
        .lock()
        .await
        .fail_claimed_replay(
            action_id,
            claim_id,
            expected_status,
            expected_revision,
            safe_error,
            metadata,
        )
        .map_err(|error| error.to_string())
}

pub(crate) async fn release_pending_action_replay_claim(
    state: &Arc<AppState>,
    task_session_id: &str,
    action_id: &str,
    claim_id: &str,
    expected_revision: u64,
) -> Result<QueuedExecutionAction, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?
        .lock()
        .await
        .release_pending_permission_replay_claim_without_dispatch(
            action_id,
            claim_id,
            expected_revision,
        )
        .map_err(|error| error.to_string())
}

pub(crate) async fn fence_action_replay_dispatch(
    state: &Arc<AppState>,
    task_session_id: &str,
    action_id: &str,
    claim_id: &str,
    expected_owner_generation: u64,
    expected_revision: u64,
) -> Result<QueuedExecutionAction, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?
        .lock()
        .await
        .fence_replay_dispatch_commit(
            action_id,
            claim_id,
            expected_owner_generation,
            expected_revision,
        )
        .map_err(|error| error.to_string())
}

pub(crate) async fn record_action_replay_dispatch_started(
    state: &Arc<AppState>,
    task_session_id: &str,
    action_id: &str,
    claim_id: &str,
    expected_revision: u64,
) -> Result<QueuedExecutionAction, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?
        .lock()
        .await
        .record_replay_dispatch_started(action_id, claim_id, expected_revision)
        .map_err(|error| error.to_string())
}

pub(crate) async fn complete_claimed_action_replay_with_commit_admission(
    state: &Arc<AppState>,
    task_session_id: &str,
    action_id: &str,
    claim_id: &str,
    expected_revision: u64,
    metadata: Option<serde_json::Value>,
    execution_epoch: &crate::main_chat_cancellation::MainChatExecutionEpoch,
) -> Result<QueuedExecutionAction, String> {
    let _fence = acquire_open_turn_write_fence(state, task_session_id).await?;
    let queue = state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "main_chat_action_queue_store_unavailable".to_string())?
        .lock()
        .await;
    let commit_permit = execution_epoch
        .begin_canonical_commit("action_queue", action_id)
        .map_err(|rejection| {
            format!("complete replay rejected after cancellation: {rejection:?}")
        })?;
    match queue.complete_claimed_replay(action_id, claim_id, expected_revision, metadata) {
        Ok(action) => {
            commit_permit.finish_committed();
            Ok(action)
        }
        Err(error) => {
            commit_permit.finish_failed();
            Err(error.to_string())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalOwnerCrashPoint {
    AfterClaimPersistedBeforeEffect,
    AfterMemoryCommittedBeforeTaskOwner,
    AfterTaskOwnerReceiptBeforeProposalCheckpoint,
    AfterProposalCheckpointBeforeSuccessor,
    AfterSuccessorBeforeProposalProjection,
}

impl TerminalOwnerCrashPoint {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AfterClaimPersistedBeforeEffect => "after_claim_persisted_before_effect",
            Self::AfterMemoryCommittedBeforeTaskOwner => "after_memory_committed_before_task_owner",
            Self::AfterTaskOwnerReceiptBeforeProposalCheckpoint => {
                "after_task_owner_receipt_before_proposal_checkpoint"
            }
            Self::AfterProposalCheckpointBeforeSuccessor => {
                "after_proposal_checkpoint_before_successor"
            }
            Self::AfterSuccessorBeforeProposalProjection => {
                "after_successor_before_proposal_projection"
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TerminalOwnerExecutionCapture {
    counts: Arc<Mutex<HashMap<(String, &'static str), usize>>>,
}

impl TerminalOwnerExecutionCapture {
    fn record(&self, proposal_id: &str, stage: &'static str) {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *counts.entry((proposal_id.to_string(), stage)).or_default() += 1;
    }

    #[cfg(test)]
    fn count(&self, proposal_id: &str, stage: &'static str) -> usize {
        self.counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(proposal_id.to_string(), stage))
            .copied()
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn memory_effect_invocations(&self, proposal_id: &str) -> usize {
        self.count(proposal_id, "memory_effect")
    }

    #[cfg(test)]
    pub(crate) fn task_owner_transition_invocations(&self, proposal_id: &str) -> usize {
        self.count(proposal_id, "task_owner_transition")
    }

    #[cfg(test)]
    pub(crate) fn successor_confirmation_invocations(&self, proposal_id: &str) -> usize {
        self.count(proposal_id, "successor_confirmation")
    }

    #[cfg(test)]
    pub(crate) fn proposal_projection_invocations(&self, proposal_id: &str) -> usize {
        self.count(proposal_id, "proposal_projection")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalOwnerReviewTransition {
    pub(crate) before_owner_revision: u64,
    pub(crate) after_owner_revision: u64,
    pub(crate) before_owner_digest: String,
    pub(crate) after_owner_digest: String,
    pub(crate) local_transition_receipt_ref: String,
    pub(crate) local_transition_receipt_digest: String,
    pub(crate) successor_event_id: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TerminalOwnerReconciliationReport {
    pub(crate) canonical_effects_executed: usize,
    pub(crate) task_owner_transitions_executed: usize,
    pub(crate) successors_confirmed: usize,
    pub(crate) proposals_projected: usize,
    pub(crate) unknown_external_effects_retried: usize,
}

pub(crate) struct TerminalOwnerWriteGateway {
    event_store: MainChatAgentEventStore,
    task_store: AgentTaskSessionStore,
    proposal_store: ProposalStore,
    memory_store: MemoryLifecycleStore,
    action_queue_store: Option<Arc<tokio::sync::Mutex<ActionQueueStore>>>,
    execution_capture: TerminalOwnerExecutionCapture,
    crash_points: Mutex<HashMap<String, TerminalOwnerCrashPoint>>,
}

impl TerminalOwnerWriteGateway {
    pub(crate) async fn from_state(state: &Arc<AppState>) -> Result<Self, String> {
        let event_store = state
            .main_chat_agent_event_store
            .as_ref()
            .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?
            .lock()
            .await
            .clone();
        let task_store = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?
            .lock()
            .await
            .clone();
        let proposal_store = state
            .proposal_store
            .as_ref()
            .ok_or_else(|| "proposal_store_unavailable".to_string())?
            .lock()
            .await
            .clone();
        let memory_store = state
            .memory_lifecycle_store
            .as_ref()
            .ok_or_else(|| "memory_lifecycle_store_unavailable".to_string())?
            .lock()
            .await
            .clone();
        let mut gateway = Self::new(&event_store, &task_store, &proposal_store, &memory_store);
        if let Some(action_queue_store) = state.main_chat_action_queue_store.as_ref() {
            gateway = gateway.with_action_queue_store(action_queue_store.clone());
        }
        Ok(gateway)
    }

    pub(crate) fn new(
        event_store: &MainChatAgentEventStore,
        task_store: &AgentTaskSessionStore,
        proposal_store: &ProposalStore,
        memory_store: &MemoryLifecycleStore,
    ) -> Self {
        Self {
            event_store: event_store.clone(),
            task_store: task_store.clone(),
            proposal_store: proposal_store.clone(),
            memory_store: memory_store.clone(),
            action_queue_store: None,
            execution_capture: TerminalOwnerExecutionCapture::default(),
            crash_points: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn with_action_queue_store(
        mut self,
        action_queue_store: Arc<tokio::sync::Mutex<ActionQueueStore>>,
    ) -> Self {
        self.action_queue_store = Some(action_queue_store);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_execution_capture_for_test(
        mut self,
        capture: TerminalOwnerExecutionCapture,
    ) -> Self {
        self.execution_capture = capture;
        self
    }

    #[cfg(test)]
    pub(crate) fn install_crash_point_for_test(
        &self,
        proposal_id: &str,
        crash_point: TerminalOwnerCrashPoint,
    ) -> anyhow::Result<()> {
        self.crash_points
            .lock()
            .map_err(|error| anyhow::anyhow!("terminal crash point mutex: {error}"))?
            .insert(proposal_id.to_string(), crash_point);
        Ok(())
    }

    fn crash_if_selected(
        &self,
        proposal_id: &str,
        crash_point: TerminalOwnerCrashPoint,
    ) -> anyhow::Result<()> {
        let selected = self
            .crash_points
            .lock()
            .map_err(|error| anyhow::anyhow!("terminal crash point mutex: {error}"))?
            .get(proposal_id)
            .copied();
        if selected == Some(crash_point) {
            anyhow::bail!("injected_terminal_owner_crash:{}", crash_point.as_str());
        }
        Ok(())
    }

    async fn complete_when_unblocked(&self, task_session_id: &str) -> anyhow::Result<bool> {
        let Some(action_queue_store) = self.action_queue_store.as_ref() else {
            return Ok(true);
        };
        let actions = action_queue_store
            .lock()
            .await
            .list_for_session(task_session_id)?;
        Ok(!actions
            .iter()
            .any(|action| action.status == ExecutionQueueStatus::PendingPermission))
    }

    async fn apply_terminal_owner_successor(
        &self,
        proposal_id: &str,
        claim_id: &str,
        origin: &TerminalOwnerOriginBinding,
    ) -> anyhow::Result<TerminalOwnerReviewTransition> {
        let epoch = self
            .event_store
            .terminal_owner_epoch(origin.task_session_id())?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_epoch_missing"))?;
        if epoch.run_id() != origin.run_id() {
            anyhow::bail!("terminal_owner_epoch_run_mismatch");
        }
        match epoch.state() {
            TerminalOwnerSealState::Open => {
                anyhow::bail!("terminal_owner_origin_turn_open");
            }
            TerminalOwnerSealState::Sealing => {
                anyhow::bail!("terminal_owner_origin_turn_sealing");
            }
            TerminalOwnerSealState::Sealed => {}
        }

        let final_event_id = epoch
            .final_event_id()
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_event_missing"))?;
        let final_event = self
            .event_store
            .get_immutable_event(
                origin.task_session_id(),
                "final_delivery.created",
                &format!("delivery:{}:{}", origin.task_session_id(), origin.run_id()),
            )?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_event_missing"))?;
        if final_event.event_id != final_event_id {
            anyhow::bail!("terminal_owner_final_event_identity_mismatch");
        }
        let before_revision = final_event
            .payload
            .get("taskOwnerRevision")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_revision_missing"))?;
        let before_digest = final_event
            .payload
            .get("taskOwnerDigest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_digest_missing"))?;
        let existing_receipt = self
            .task_store
            .terminal_owner_transition_receipt_for_claim(proposal_id, claim_id)?;
        let receipt = if let Some(receipt) = existing_receipt {
            receipt
        } else {
            self.execution_capture
                .record(proposal_id, "task_owner_transition");
            let complete_when_unblocked = self
                .complete_when_unblocked(origin.task_session_id())
                .await?;
            self.task_store.apply_terminal_owner_review_transition(
                proposal_id,
                claim_id,
                origin.task_session_id(),
                before_revision,
                before_digest,
                complete_when_unblocked,
            )?
        };
        self.crash_if_selected(
            proposal_id,
            TerminalOwnerCrashPoint::AfterTaskOwnerReceiptBeforeProposalCheckpoint,
        )?;

        let dispatch_state = self
            .proposal_store
            .dispatch_state(proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_dispatch_state_missing"))?;
        if dispatch_state == "claimed"
            && !self
                .proposal_store
                .mark_effect_confirmed_projection_pending(proposal_id, claim_id)?
        {
            anyhow::bail!("terminal_owner_proposal_checkpoint_cas_lost");
        }
        self.crash_if_selected(
            proposal_id,
            TerminalOwnerCrashPoint::AfterProposalCheckpointBeforeSuccessor,
        )?;

        let successor_existed = self
            .event_store
            .get_immutable_event(
                origin.task_session_id(),
                "terminal_owner.successor_confirmed",
                &format!("successor:{proposal_id}"),
            )?
            .is_some();
        let successor = self.event_store.append_terminal_owner_successor(
            origin.task_session_id(),
            origin.run_id(),
            proposal_id,
            &receipt,
        )?;
        if !successor_existed {
            self.execution_capture
                .record(proposal_id, "successor_confirmation");
        }
        self.crash_if_selected(
            proposal_id,
            TerminalOwnerCrashPoint::AfterSuccessorBeforeProposalProjection,
        )?;
        Ok(transition_from_receipt(receipt, successor))
    }

    pub(crate) async fn apply_claimed_review_acceptance(
        &self,
        acceptance: ClaimedReviewAcceptanceSnapshot,
    ) -> anyhow::Result<TerminalOwnerReviewTransition> {
        acceptance.validate()?;
        let proposal = acceptance.proposal().clone();
        let proposal_id = proposal.id.clone();
        let claim_id = self
            .proposal_store
            .dispatch_claim_id(&proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_dispatch_claim_missing"))?;
        self.crash_if_selected(
            &proposal_id,
            TerminalOwnerCrashPoint::AfterClaimPersistedBeforeEffect,
        )?;
        let origin = acceptance
            .terminal_owner_origin()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_origin_binding_missing"))?;
        let dispatch_state = self
            .proposal_store
            .dispatch_state(&proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_dispatch_state_missing"))?;
        if proposal.proposal_type == ProposalType::ExternalWriteAction
            && dispatch_state == "claimed"
        {
            anyhow::bail!("terminal_owner_external_effect_requires_artifact_materializer");
        }
        if proposal.proposal_type == ProposalType::MemoryWrite {
            if self
                .memory_store
                .get_record_by_proposal_id(&proposal_id)?
                .is_none()
            {
                self.execution_capture.record(&proposal_id, "memory_effect");
                let content = proposal
                    .after
                    .get("content")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("terminal_owner_memory_content_missing"))?;
                let input =
                    MemoryLifecycleAcceptanceInput::from_memory_proposal_with_terminal_origin(
                        &proposal,
                        content.to_string(),
                        origin.task_session_id(),
                        origin.run_id(),
                        origin.canonical_user_message_ref(),
                        origin.canonical_user_message_digest(),
                    )?;
                self.memory_store.accept_memory_proposal(input)?;
            }
            self.crash_if_selected(
                &proposal_id,
                TerminalOwnerCrashPoint::AfterMemoryCommittedBeforeTaskOwner,
            )?;
        } else if dispatch_state != "confirmed_projection_pending" {
            anyhow::bail!("terminal_owner_non_memory_effect_not_materialized");
        }
        let transition = self
            .apply_terminal_owner_successor(&proposal_id, &claim_id, &origin)
            .await?;

        if self.proposal_store.dispatch_state(&proposal_id)?.as_deref()
            == Some("confirmed_projection_pending")
        {
            let mut accepted = proposal;
            accepted.accept();
            self.execution_capture
                .record(&proposal_id, "proposal_projection");
            if !self
                .proposal_store
                .project_confirmed_effect(&accepted, &claim_id)?
            {
                anyhow::bail!("terminal_owner_proposal_projection_cas_lost");
            }
        }
        Ok(transition)
    }

    pub(crate) async fn apply_materialized_review_successor(
        &self,
        acceptance: MaterializedReviewAcceptanceSnapshot,
    ) -> anyhow::Result<TerminalOwnerReviewTransition> {
        acceptance.validate()?;
        let proposal = acceptance.proposal();
        if proposal.status != openlife_core::agent::ProposalStatus::Accepted {
            anyhow::bail!("terminal_owner_materialized_proposal_not_accepted");
        }
        let proposal_id = proposal.id.as_str();
        if self.proposal_store.dispatch_state(proposal_id)?.as_deref() != Some("confirmed") {
            anyhow::bail!("terminal_owner_materialized_effect_not_confirmed");
        }
        let claim_id = self
            .proposal_store
            .dispatch_claim_id(proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_dispatch_claim_missing"))?;
        let origin = self
            .proposal_store
            .terminal_owner_origin_binding(proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_origin_binding_missing"))?;
        self.apply_terminal_owner_successor(proposal_id, &claim_id, &origin)
            .await
    }

    pub(crate) async fn reconcile_pending_terminal_owner_successors(
        &self,
        limit: usize,
    ) -> anyhow::Result<TerminalOwnerReconciliationReport> {
        let candidates = self
            .proposal_store
            .list_terminal_owner_reconciliation_candidates(limit)?;
        let mut before_memory = 0;
        let mut before_task = 0;
        for (proposal, claim_id, _) in &candidates {
            if proposal.proposal_type == ProposalType::MemoryWrite
                && self
                    .memory_store
                    .get_record_by_proposal_id(&proposal.id)?
                    .is_none()
            {
                before_memory += 1;
            }
            if self
                .task_store
                .terminal_owner_transition_receipt_for_claim(&proposal.id, claim_id)?
                .is_none()
            {
                before_task += 1;
            }
        }
        let mut before_successors = 0;
        for (proposal, _, _) in &candidates {
            let origin = self
                .proposal_store
                .terminal_owner_origin_binding(&proposal.id)?
                .ok_or_else(|| anyhow::anyhow!("terminal_owner_origin_binding_missing"))?;
            if self
                .event_store
                .get_immutable_event(
                    origin.task_session_id(),
                    "terminal_owner.successor_confirmed",
                    &format!("successor:{}", proposal.id),
                )?
                .is_none()
            {
                before_successors += 1;
            }
        }
        for (proposal, claim_id, _) in &candidates {
            let acceptance = ReviewWorkflow::new(&self.proposal_store)
                .claimed_acceptance_snapshot(&proposal.id, claim_id)?;
            self.apply_claimed_review_acceptance(acceptance).await?;
        }
        Ok(TerminalOwnerReconciliationReport {
            canonical_effects_executed: before_memory,
            task_owner_transitions_executed: before_task,
            successors_confirmed: before_successors,
            proposals_projected: candidates.len(),
            unknown_external_effects_retried: 0,
        })
    }
}

fn transition_from_receipt(
    receipt: VerifiedTerminalOwnerTransitionReceipt,
    successor: MainChatAgentDurableEvent,
) -> TerminalOwnerReviewTransition {
    TerminalOwnerReviewTransition {
        before_owner_revision: receipt.before_revision(),
        after_owner_revision: receipt.after_revision(),
        before_owner_digest: receipt.before_digest().to_string(),
        after_owner_digest: receipt.after_digest().to_string(),
        local_transition_receipt_ref: receipt.receipt_ref().to_string(),
        local_transition_receipt_digest: receipt.receipt_digest().to_string(),
        successor_event_id: successor.event_id,
    }
}
