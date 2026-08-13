use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use openlife_core::agent::main_chat_agent_v1::{
    ActionQueueStore, ActionReplayClaim, AgentTaskSession, AgentTaskSessionStatus,
    AgentTaskSessionStore, ExecutionAction, ExecutionPolicyDecision, ExecutionQueueStatus,
    ExecutionTranscriptEntry, ExecutionTranscriptEntryDraft, QueuedExecutionAction,
    VerifiedTerminalOwnerTransitionReceipt,
};
use openlife_core::agent::proposal_store::TerminalOwnerOriginBinding;
use openlife_core::agent::review_workflow::{
    ClaimedReviewAcceptanceSnapshot, MaterializedReviewAcceptanceSnapshot,
};
use openlife_core::agent::{
    AgentAction, AgentLoopPhase, AgentLoopStatusUpdate, AgentObservation, AgentRun, AgentRunError,
    AgentRunReviewRelationProjectionLane, AgentRunReviewRelationProjectionOutcome, AgentRunStatus,
    AgentRunStore, CanonicalWriteAdmission, CanonicalWriteAdmissionRequest, ContextSummary,
    DurableWriteRequest, HSBehaviorCheckSummary, HSSelectionAudit, MemoryLifecycleAcceptanceInput,
    MemoryLifecycleStore, ModelRouteTrace, ProposalStatus, ProposalStore,
    ProposalTerminalRelationKind, ProposalType, ReasoningTrace, ReviewWorkflow,
    TerminalOwnerReviewOriginProof, TerminalOwnerReviewSubmission,
};
use openlife_core::persistence_outbox::CanonicalMutationReceipt;

use crate::main_chat_cancellation::{MainChatCanonicalCommitRejection, MainChatExecutionEpoch};
use crate::main_chat_event_stream::{
    MainChatAgentDurableEvent, MainChatAgentEventStore, TerminalOwnerSealState,
};
use crate::main_chat_replay_contract::DurableMainChatReplayExecutionEnvelope;
use crate::persistence_coordinator::AgentRunCanonicalWriteAdmission;
use crate::AppState;

fn terminal_owner_task_fences() -> &'static Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>> {
    static FENCES: OnceLock<Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    FENCES.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
struct AgentRunLifecycleCommitTestBarrier {
    reached: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}

#[cfg(test)]
fn agent_run_lifecycle_commit_test_barriers(
) -> &'static Mutex<HashMap<String, AgentRunLifecycleCommitTestBarrier>> {
    static BARRIERS: OnceLock<Mutex<HashMap<String, AgentRunLifecycleCommitTestBarrier>>> =
        OnceLock::new();
    BARRIERS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
pub(crate) fn install_agent_run_lifecycle_commit_test_barrier(
    run_id: &str,
) -> (
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (reached_tx, reached_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let replaced = agent_run_lifecycle_commit_test_barriers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            run_id.to_string(),
            AgentRunLifecycleCommitTestBarrier {
                reached: reached_tx,
                release: release_rx,
            },
        );
    assert!(
        replaced.is_none(),
        "AgentRun lifecycle barrier already installed"
    );
    (reached_rx, release_tx)
}

#[cfg(test)]
pub(crate) async fn wait_at_agent_run_lifecycle_commit_test_barrier(run_id: &str) {
    let barrier = agent_run_lifecycle_commit_test_barriers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(run_id);
    if let Some(barrier) = barrier {
        let _ = barrier.reached.send(());
        let _ = barrier.release.await;
    }
}

/// Ensures a pre-commit test failure cannot leave the barrier's `reached`
/// sender parked in the global registry forever. On the normal commit path
/// `wait_at_agent_run_lifecycle_commit_test_barrier` removes the entry first,
/// so this drop becomes a no-op.
#[cfg(test)]
struct AgentRunLifecycleCommitTestBarrierScope(String);

#[cfg(test)]
impl AgentRunLifecycleCommitTestBarrierScope {
    fn enter(run_id: &str) -> Self {
        Self(run_id.to_string())
    }
}

#[cfg(test)]
impl Drop for AgentRunLifecycleCommitTestBarrierScope {
    fn drop(&mut self) {
        agent_run_lifecycle_commit_test_barriers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.0);
    }
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

/// Sole Main Chat product seam for Proposal creation with an explicit typed
/// lifecycle relation to the currently open terminal owner. This is a
/// projection coordinator, not a second runtime: ReviewWorkflow owns the
/// canonical Proposal/relation transaction, ProposalStore owns the outbox,
/// and AgentRunStore owns only the derived lifecycle link.
pub(crate) async fn submit_main_chat_terminal_review_relation(
    state: &Arc<AppState>,
    origin: &TerminalOwnerReviewOriginProof,
    relation_kind: ProposalTerminalRelationKind,
    request: DurableWriteRequest,
    execution_epoch: &MainChatExecutionEpoch,
) -> Result<TerminalOwnerReviewSubmission, String> {
    origin
        .validate()
        .map_err(|error| format!("terminal owner review origin invalid: {error}"))?;
    if relation_kind == ProposalTerminalRelationKind::LegacyUnclassified {
        return Err("main_chat_legacy_review_relation_forbidden".into());
    }

    // Clone connection owners before entering the run/task critical section;
    // no shared Tokio store guard is held across the synchronous cross-store
    // proof and projection protocol below.
    let agent_run_store = clone_agent_run_store(state).await?;
    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "proposal_store_unavailable".to_string())?
        .lock()
        .await
        .clone();

    let causal_lock = state
        .persistence_coordinator
        .agent_run_causal_lock(origin.run_id());
    let _causal_guard = causal_lock.lock().await;
    let _terminal_fence = acquire_open_turn_write_fence(state, origin.task_session_id()).await?;
    {
        let event_store = state
            .main_chat_agent_event_store
            .as_ref()
            .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?
            .lock()
            .await;
        event_store
            .revalidate_open_review_origin(origin)
            .map_err(|error| {
                format!("terminal owner review origin revalidation failed: {error}")
            })?;
    }

    let target = {
        let memory_store = state.memory_store.lock().await;
        memory_store
            .issue_agent_run_terminal_relation_target_intent(&agent_run_store, origin)
            .map_err(|error| format!("issue AgentRun review target failed: {error}"))?
    };
    let submission = ReviewWorkflow::new(&proposal_store)
        .submit_product_with_terminal_owner_relation(
            request,
            origin,
            relation_kind,
            execution_epoch,
            &target,
        )
        .map_err(|error| format!("submit typed Main Chat review relation failed: {error}"))?;

    if !submission.owns_terminal_relation() {
        return Ok(submission);
    }

    let proposal_id = submission.review().proposal_id();
    let projection = proposal_store
        .terminal_relation_projection_proof(proposal_id)
        .map_err(|error| format!("load typed Main Chat review projection failed: {error}"))?
        .ok_or_else(|| "typed_main_chat_review_projection_missing".to_string())?;
    let lane = openlife_core::agent::issue_agent_run_review_relation_projection_lane(
        origin,
        AgentRunReviewRelationProjectionLane::ForegroundOpen,
    )
    .map_err(|error| format!("issue foreground review projection lane failed: {error}"))?;
    let projection_permit = execution_epoch
        .acquire(CanonicalWriteAdmissionRequest::new(
            "agent_run_review_relation_projection",
            format!("proposal:{proposal_id}"),
        ))
        .map_err(|error| format!("review relation projection admission rejected: {error}"))?;
    let projection_result = register_agent_run_store_result(
        state,
        agent_run_store
            .apply_terminal_review_relation_projection(&projection, &lane)
            .map_err(|error| error.to_string()),
    );
    match projection_result {
        Ok(AgentRunReviewRelationProjectionOutcome::Applied) => {
            projection_permit.finish_committed();
        }
        Ok(AgentRunReviewRelationProjectionOutcome::AlreadyApplied) => {
            projection_permit.finish_noop();
        }
        Err(error) => {
            projection_permit.finish_failed();
            let degraded = proposal_store
                .mark_terminal_relation_projection_degraded(&projection, &error)
                .map_err(|degrade_error| {
                    format!(
                        "{error}; mark typed Main Chat review projection degraded failed: {degrade_error}"
                    )
                });
            return match degraded {
                Ok(()) => Err(error),
                Err(combined) => Err(combined),
            };
        }
    }
    proposal_store
        .mark_terminal_relation_projection_applied(&projection)
        .map_err(|error| format!("finalize typed Main Chat review projection failed: {error}"))?;
    Ok(submission)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalOwnerReplayCause {
    AutomaticRetry,
    AcceptedToolPermission,
    AcceptedToolNetworkConsent,
    AcceptedProviderNetworkConsent,
}

impl TerminalOwnerReplayCause {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AutomaticRetry => "automatic_retry",
            Self::AcceptedToolPermission => "accepted_tool_permission",
            Self::AcceptedToolNetworkConsent => "accepted_tool_network_consent",
            Self::AcceptedProviderNetworkConsent => "accepted_provider_network_consent",
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
    retry_proof: Option<openlife_core::agent::tool_gateway::ToolAutomaticRetryProof>,
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
    ) -> Result<openlife_core::agent::tool_gateway::ToolAutomaticRetryProof, String> {
        self.retry_proof
            .ok_or_else(|| "terminal_owner_tool_replay_retry_proof_missing".to_string())
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

pub(crate) struct TerminalOwnerReplayPermissionAuthorities<'a> {
    pub(crate) action_bound:
        Option<&'a openlife_core::tool_permissions::ActionBoundToolPermissionAuthorization>,
    pub(crate) network_consent_proposal_id: Option<&'a str>,
}

pub(crate) async fn issue_terminal_owner_replay_epoch_admission(
    state: &Arc<AppState>,
    session: &AgentTaskSession,
    action: &QueuedExecutionAction,
    envelope: &DurableMainChatReplayExecutionEnvelope,
    cause: TerminalOwnerReplayCause,
    permission_authorities: TerminalOwnerReplayPermissionAuthorities<'_>,
    retry_proof: openlife_core::agent::tool_gateway::ToolAutomaticRetryProof,
) -> Result<TerminalOwnerReplayEpochAdmission, String> {
    let action_bound_permission = permission_authorities.action_bound;
    let network_consent_proposal_id = permission_authorities.network_consent_proposal_id;
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
            if action_bound_permission.is_some() || network_consent_proposal_id.is_some() {
                return Err("terminal_owner_retry_unexpected_permission_authority".into());
            }
            final_owner_head(&final_event)?
        }
        TerminalOwnerReplayCause::AcceptedToolPermission => {
            if network_consent_proposal_id.is_some() {
                return Err("terminal_owner_permission_replay_unexpected_network_authority".into());
            }
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
            let relation_kind = proposal_store
                .terminal_relation_projection_proof(&authorization.proposal_id)
                .map_err(|error| error.to_string())?
                .map(|proof| proof.relation_kind());
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
            if let Some(relation_kind) = relation_kind {
                if relation_kind
                    != openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite
                {
                    return Err("terminal_owner_permission_replay_relation_mismatch".into());
                }
                final_owner_head(&final_event)?
            } else {
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
                    .ok_or_else(|| {
                        "terminal_owner_permission_replay_successor_missing".to_string()
                    })?;
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
        }
        TerminalOwnerReplayCause::AcceptedToolNetworkConsent => {
            let authorization = action_bound_permission.ok_or_else(|| {
                "terminal_owner_tool_network_replay_action_authorization_missing".to_string()
            })?;
            let network_proposal_id = network_consent_proposal_id
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "terminal_owner_tool_network_replay_proposal_missing".to_string())?;
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
                return Err("terminal_owner_tool_network_replay_action_scope_mismatch".into());
            }
            let (
                action_proposal,
                action_origin,
                action_relation,
                action_dispatch,
                network_proposal,
                network_origin,
                network_relation,
                network_dispatch,
            ) = {
                let proposal_store = state
                    .proposal_store
                    .as_ref()
                    .ok_or_else(|| "proposal_store_unavailable".to_string())?
                    .lock()
                    .await;
                let action_proposal = proposal_store
                    .get_proposal(&authorization.proposal_id)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| {
                        "terminal_owner_tool_network_replay_action_proposal_missing".to_string()
                    })?;
                let action_origin = proposal_store
                    .terminal_owner_origin_binding(&authorization.proposal_id)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| {
                        "terminal_owner_tool_network_replay_action_origin_missing".to_string()
                    })?;
                let action_relation = proposal_store
                    .terminal_relation_projection_proof(&authorization.proposal_id)
                    .map_err(|error| error.to_string())?
                    .map(|proof| proof.relation_kind());
                let action_dispatch = proposal_store
                    .dispatch_state(&authorization.proposal_id)
                    .map_err(|error| error.to_string())?;
                let network_proposal = proposal_store
                    .get_proposal(network_proposal_id)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| {
                        "terminal_owner_tool_network_replay_network_proposal_missing".to_string()
                    })?;
                let network_origin = proposal_store
                    .terminal_owner_origin_binding(network_proposal_id)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| {
                        "terminal_owner_tool_network_replay_network_origin_missing".to_string()
                    })?;
                let network_relation = proposal_store
                    .terminal_relation_projection_proof(network_proposal_id)
                    .map_err(|error| error.to_string())?
                    .map(|proof| proof.relation_kind());
                let network_dispatch = proposal_store
                    .dispatch_state(network_proposal_id)
                    .map_err(|error| error.to_string())?;
                (
                    action_proposal,
                    action_origin,
                    action_relation,
                    action_dispatch,
                    network_proposal,
                    network_origin,
                    network_relation,
                    network_dispatch,
                )
            };
            let action_origin_matches = action_origin.task_session_id() == session.id
                && action_origin.run_id() == envelope.run_id
                && action_origin.epoch_generation() < epoch.generation()
                && action_origin.canonical_user_message_ref() == epoch.canonical_user_message_ref()
                && action_origin.canonical_user_message_digest()
                    == epoch.canonical_user_message_digest();
            if action_proposal.status != openlife_core::agent::ProposalStatus::Accepted
                || action_proposal.proposal_type != ProposalType::ToolPermission
                || action_dispatch.as_deref() != Some("confirmed")
                || action_relation
                    != Some(openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite)
                || !action_origin_matches
            {
                return Err("terminal_owner_tool_network_replay_action_origin_mismatch".into());
            }
            let after_string = |key: &str| {
                network_proposal
                    .after
                    .get(key)
                    .and_then(serde_json::Value::as_str)
            };
            let canonical_scope = network_proposal.after.get("canonical_scope");
            let scope_string = |key: &str| {
                canonical_scope
                    .and_then(|scope| scope.get(key))
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| after_string(key))
            };
            let permission_scope = scope_string("tool_name")
                .ok_or_else(|| "terminal_owner_tool_network_replay_scope_missing".to_string())?;
            let blocked_action = network_proposal.after.get("blocked_action");
            let blocked_string = |key: &str| {
                blocked_action
                    .and_then(|blocked| blocked.get(key))
                    .and_then(serde_json::Value::as_str)
            };
            if network_proposal.status != openlife_core::agent::ProposalStatus::Accepted
                || network_proposal.proposal_type != ProposalType::ToolPermission
                || !matches!(
                    network_proposal.source,
                    openlife_core::agent::ProposalSource::ChatConversation
                )
                || network_dispatch.as_deref() != Some("confirmed")
                || network_relation
                    != Some(openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite)
                || after_string("permission_scope_kind") != Some("network_policy")
                || after_string("permission") != Some("allow_once")
                || scope_string("source") != Some("network_policy")
                || scope_string("risk_level") != Some("medium")
                || scope_string("action_type") != Some("network")
                || scope_string("network_capability") != Some(envelope.requested_target.as_str())
                || blocked_string("target") != Some(envelope.requested_target.as_str())
                || blocked_string("resolved_target") != Some(envelope.requested_target.as_str())
                || !network_proposal
                    .affected_path
                    .starts_with("tool_permission.network_policy.")
            {
                return Err("terminal_owner_tool_network_replay_proposal_contract_mismatch".into());
            }
            if network_origin.task_session_id() != session.id
                || network_origin.run_id() != epoch.run_id()
                || network_origin.epoch_id() != epoch.epoch_id()
                || network_origin.epoch_generation() != epoch.generation()
                || network_origin.canonical_user_message_ref() != epoch.canonical_user_message_ref()
                || network_origin.canonical_user_message_digest()
                    != epoch.canonical_user_message_digest()
            {
                return Err("terminal_owner_tool_network_replay_network_origin_mismatch".into());
            }
            let action_permission_available = state
                .tool_permission_store
                .lock()
                .await
                .peek_action_bound(&authorization.proposal_id, &authorization.scope)
                .map_err(|error| error.to_string())?
                .is_some();
            if !action_permission_available {
                return Err("terminal_owner_tool_network_replay_action_grant_missing".into());
            }
            let network_permission_available = state
                .tool_permission_store
                .lock()
                .await
                .reviewed_network_once_available_for_proposal(
                    network_proposal_id,
                    permission_scope,
                    "network_policy",
                    "medium",
                    "network",
                )
                .map_err(|error| error.to_string())?;
            if !network_permission_available {
                return Err("terminal_owner_tool_network_replay_network_grant_missing".into());
            }
            final_owner_head(&final_event)?
        }
        TerminalOwnerReplayCause::AcceptedProviderNetworkConsent => {
            return Err("provider consent requires provider replay admission issuer".into());
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
        TerminalOwnerReplayCause::AcceptedToolNetworkConsent => network_consent_proposal_id
            .expect("tool network replay checked above")
            .to_string(),
        TerminalOwnerReplayCause::AcceptedProviderNetworkConsent => {
            unreachable!("provider consent is rejected by the tool replay admission issuer")
        }
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
        retry_proof: Some(retry_proof),
        authority: TerminalOwnerReplayEpochAuthority::VerifiedByTerminalOwnerWriteGateway,
    })
}

/// Issue one continuation epoch for the exact accepted provider-network
/// Proposal that blocked the prior generation. This is deliberately separate
/// from action replay: it has no ActionQueue identity or ToolGateway retry
/// proof, while retaining the same single terminal-owner epoch CAS.
pub(crate) async fn issue_terminal_owner_provider_consent_replay_admission(
    state: &Arc<AppState>,
    session: &AgentTaskSession,
    proposal_id: &str,
) -> Result<TerminalOwnerReplayEpochAdmission, String> {
    use openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus;
    use openlife_core::agent::{ProposalStatus, ProposalTerminalRelationKind, ProposalType};

    if proposal_id.trim().is_empty() || session.status != AgentTaskSessionStatus::WaitingPermission
    {
        return Err("terminal_owner_provider_replay_not_waiting_permission".into());
    }
    let _fence = acquire_terminal_owner_task_fence(&session.id).await;
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
            .ok_or_else(|| "terminal_owner_provider_replay_epoch_missing".to_string())?;
        if epoch.state() != TerminalOwnerSealState::Sealed {
            return Err("terminal_owner_provider_replay_requires_sealed_epoch".into());
        }
        let final_event = event_store
            .terminal_owner_final_event(&session.id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "terminal_owner_provider_replay_final_event_missing".to_string())?;
        if epoch.final_event_id() != Some(final_event.event_id.as_str()) {
            return Err("terminal_owner_provider_replay_final_identity_mismatch".into());
        }
        (epoch, final_event)
    };
    let (proposal, origin, relation_kind, dispatch_state) = {
        let proposal_store = state
            .proposal_store
            .as_ref()
            .ok_or_else(|| "proposal_store_unavailable".to_string())?
            .lock()
            .await;
        let proposal = proposal_store
            .get_proposal(proposal_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "terminal_owner_provider_replay_proposal_missing".to_string())?;
        let origin = proposal_store
            .terminal_owner_origin_binding(proposal_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "terminal_owner_provider_replay_origin_missing".to_string())?;
        let relation_kind = proposal_store
            .terminal_relation_projection_proof(proposal_id)
            .map_err(|error| error.to_string())?
            .map(|proof| proof.relation_kind());
        let dispatch_state = proposal_store
            .dispatch_state(proposal_id)
            .map_err(|error| error.to_string())?;
        (proposal, origin, relation_kind, dispatch_state)
    };
    let after_string = |key: &str| proposal.after.get(key).and_then(serde_json::Value::as_str);
    let canonical_scope = proposal.after.get("canonical_scope");
    let scope_string = |key: &str| {
        canonical_scope
            .and_then(|scope| scope.get(key))
            .and_then(serde_json::Value::as_str)
            .or_else(|| after_string(key))
    };
    let permission_scope = scope_string("tool_name")
        .ok_or_else(|| "terminal_owner_provider_replay_scope_missing".to_string())?;
    if proposal.status != ProposalStatus::Accepted
        || proposal.proposal_type != ProposalType::ToolPermission
        || !matches!(
            proposal.source,
            openlife_core::agent::ProposalSource::ChatConversation
        )
        || dispatch_state.as_deref() != Some("confirmed")
        || relation_kind != Some(ProposalTerminalRelationKind::ActionResumePrerequisite)
        || after_string("permission_scope_kind") != Some("network_policy")
        || after_string("permission") != Some("allow_once")
        || scope_string("source") != Some("provider")
        || scope_string("risk_level") != Some("high")
        || scope_string("action_type") != Some("network")
        || !proposal
            .affected_path
            .starts_with("tool_permission.provider.")
    {
        return Err("terminal_owner_provider_replay_proposal_contract_mismatch".into());
    }
    if origin.task_session_id() != session.id
        || origin.run_id() != epoch.run_id()
        || origin.epoch_id() != epoch.epoch_id()
        || origin.epoch_generation() != epoch.generation()
        || origin.canonical_user_message_ref() != epoch.canonical_user_message_ref()
        || origin.canonical_user_message_digest() != epoch.canonical_user_message_digest()
    {
        return Err("terminal_owner_provider_replay_origin_mismatch".into());
    }
    let run_store = clone_agent_run_store(state).await?;
    let run = load_live_agent_run(state, &run_store, origin.run_id())?;
    if run.task_id != session.id || run.status != AgentRunStatus::WaitingPermission {
        return Err("terminal_owner_provider_replay_run_not_waiting_permission".into());
    }
    let permission_available = state
        .tool_permission_store
        .lock()
        .await
        .reviewed_network_once_available_for_proposal(
            proposal_id,
            permission_scope,
            "provider",
            "high",
            "network",
        )
        .map_err(|error| error.to_string())?;
    if !permission_available {
        return Err("terminal_owner_provider_replay_grant_missing_or_consumed".into());
    }
    let expected_owner = final_owner_head(&final_event)?;
    let owner_head = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?
        .lock()
        .await
        .canonical_owner_head(&session.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "terminal_owner_provider_replay_task_owner_missing".to_string())?;
    if owner_head.revision() != expected_owner.0 || owner_head.digest() != expected_owner.1 {
        return Err("terminal_owner_provider_replay_task_owner_drift".into());
    }
    Ok(TerminalOwnerReplayEpochAdmission {
        admission_id: format!(
            "terminal-provider-replay-admission:{}",
            uuid::Uuid::new_v4()
        ),
        task_session_id: session.id.clone(),
        run_id: origin.run_id().to_string(),
        action_id: proposal_id.to_string(),
        prior_epoch_id: epoch.epoch_id().to_string(),
        prior_epoch_generation: epoch.generation(),
        prior_final_event_id: final_event.event_id,
        canonical_user_message_ref: epoch.canonical_user_message_ref().to_string(),
        canonical_user_message_digest: epoch.canonical_user_message_digest().to_string(),
        cause: TerminalOwnerReplayCause::AcceptedProviderNetworkConsent,
        cause_ref: proposal_id.to_string(),
        retry_proof: None,
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
    ResumeAfterResolvedBlocker(String),
    Cancel(String),
    RecordActionQueueId(String),
    RecordContextSnapshotRef(String),
    UpdatePlanSummary(Option<String>),
}

pub(crate) enum TaskSessionTransition {
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
        TaskSessionWrite::ResumeAfterResolvedBlocker(blocker) => {
            store.resume_session_after_resolved_blocker(task_session_id, &blocker)
        }
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

#[cfg(test)]
pub(crate) async fn replace_agent_run_for_test(
    state: &Arc<AppState>,
    run: &AgentRun,
) -> Result<(), String> {
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(&run.id);
    let _causal_guard = causal_lock.lock().await;
    let store = clone_agent_run_store(state).await?;
    let canonical_task_id = load_live_agent_run(state, &store, &run.id)?.task_id;
    if canonical_task_id != run.task_id {
        return Err("agent_run_update_canonical_task_identity_mismatch".into());
    }
    let _fence = acquire_open_turn_write_fence(state, &canonical_task_id).await?;
    let admission = state
        .persistence_coordinator
        .admit_agent_run_write()
        .map_err(|error| error.to_string())?;
    commit_agent_run_update(state, &store, run, &admission).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentRunWriteLane {
    Normal,
    StartupReconciliation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentRunMainChatFailureKind {
    Timeout,
    Cancelled,
    Interrupted,
    ProviderError,
    ToolError,
    PolicyBlocker,
    UnknownError,
}

impl AgentRunMainChatFailureKind {
    fn error_phase(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
            Self::ProviderError => "provider_error",
            Self::ToolError => "tool_error",
            Self::PolicyBlocker => "policy_blocker",
            Self::UnknownError => "unknown_error",
        }
    }

    fn recoverable(self) -> bool {
        !matches!(self, Self::Cancelled | Self::Interrupted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentRunReplayProjection {
    WaitingForAnotherAction,
    WaitingForPermission,
    FailedUnresolvedAction,
}

pub(crate) struct MainChatGenerationProjection {
    pub context_summary: ContextSummary,
    pub model_route: ModelRouteTrace,
    pub output_preview: String,
    pub reasoning_strategy: Option<String>,
    pub reasoning_trace: ReasoningTrace,
    pub terminal_owner_generation: u64,
    pub actions: Vec<AgentAction>,
    pub observations: Vec<AgentObservation>,
    pub hs_selection_audit: Option<HSSelectionAudit>,
    pub behavior_checks: Vec<HSBehaviorCheckSummary>,
    pub step_count: u32,
    pub tool_call_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatBlockedDisposition {
    WaitingPermission,
    TerminalFailurePendingDurableReceipt,
}

pub(crate) struct MainChatBlockedProjection {
    pub reasoning_strategy: Option<String>,
    pub reasoning_trace: ReasoningTrace,
    pub actions: Vec<AgentAction>,
    pub observations: Vec<AgentObservation>,
    pub step_count: u32,
    pub tool_call_count: u32,
    pub disposition: MainChatBlockedDisposition,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CanonicalAgentRunReviewCounts {
    confirmed: usize,
    confirmed_action_resume: usize,
    rejected: usize,
    expired: usize,
    waiting: usize,
    claimed: usize,
    unknown: usize,
    failed_before_effect: usize,
    projection_pending: usize,
    terminal_evidence_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl CanonicalAgentRunReviewCounts {
    fn terminal_without_unknown_or_waiting(self) -> bool {
        self.waiting == 0 && self.claimed == 0 && self.unknown == 0
    }

    fn declined(self) -> usize {
        self.rejected.saturating_add(self.expired)
    }

    fn effect_unknown(self) -> usize {
        self.claimed.saturating_add(self.unknown)
    }

    fn observe_evidence_time(&mut self, observed_at: chrono::DateTime<chrono::Utc>) {
        self.terminal_evidence_time = Some(
            self.terminal_evidence_time
                .map_or(observed_at, |current| std::cmp::max(current, observed_at)),
        );
    }
}

fn admit_agent_run_write(
    state: &Arc<AppState>,
    lane: AgentRunWriteLane,
) -> Result<AgentRunCanonicalWriteAdmission, String> {
    match lane {
        AgentRunWriteLane::Normal => state.persistence_coordinator.admit_agent_run_write(),
        AgentRunWriteLane::StartupReconciliation => state
            .persistence_coordinator
            .admit_startup_agent_run_write(),
    }
    .map_err(|error| error.to_string())
}

async fn clone_agent_run_store(state: &Arc<AppState>) -> Result<AgentRunStore, String> {
    Ok(state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?
        .lock()
        .await
        .clone())
}

/// Sole physical AgentRun update seam. Callers retain their distinct
/// persistence-admission and execution-epoch semantics, but every accepted
/// mutation reaches the store and the durable-failure classifier exactly once.
fn write_agent_run_update_once(
    state: &Arc<AppState>,
    store: &AgentRunStore,
    run: &AgentRun,
) -> Result<(), String> {
    register_agent_run_store_result(
        state,
        store.update_run(run).map_err(|error| error.to_string()),
    )
}

async fn commit_agent_run_update(
    state: &Arc<AppState>,
    store: &AgentRunStore,
    run: &AgentRun,
    admission: &AgentRunCanonicalWriteAdmission,
) -> Result<(), String> {
    #[cfg(test)]
    wait_at_agent_run_lifecycle_commit_test_barrier(&run.id).await;
    let permit = state
        .persistence_coordinator
        .acquire_agent_run_commit_permit(admission)
        .await
        .map_err(|error| error.to_string())?;
    let result = write_agent_run_update_once(state, store, run);
    drop(permit);
    result
}

fn main_chat_agent_run_commit_rejection(rejection: MainChatCanonicalCommitRejection) -> String {
    let reason = match rejection {
        MainChatCanonicalCommitRejection::CancelRequested => "cancel_requested",
        MainChatCanonicalCommitRejection::TerminalizationDegraded => "terminalization_degraded",
        MainChatCanonicalCommitRejection::InvalidDomain => "invalid_domain",
        MainChatCanonicalCommitRejection::InvalidObjectReference => "invalid_object_reference",
    };
    format!("main_chat_agent_run_commit_rejected:{reason}")
}

async fn commit_main_chat_agent_run_update(
    state: &Arc<AppState>,
    store: &AgentRunStore,
    run: &AgentRun,
    admission: &AgentRunCanonicalWriteAdmission,
    execution_epoch: &MainChatExecutionEpoch,
) -> Result<(), String> {
    // This barrier represents the last pre-commit scheduling point. The
    // persistence permit may still wait, so acquire it before the non-Send
    // execution-epoch permit. The epoch permit is then created immediately
    // before the synchronous owner transaction: cancel-first rejects, while
    // commit-first becomes an in-flight fact terminalization must observe.
    #[cfg(test)]
    wait_at_agent_run_lifecycle_commit_test_barrier(&run.id).await;
    let persistence_permit = state
        .persistence_coordinator
        .acquire_agent_run_commit_permit(admission)
        .await
        .map_err(|error| error.to_string())?;
    let epoch_permit = execution_epoch
        .begin_canonical_commit("agent_run", &run.id)
        .map_err(main_chat_agent_run_commit_rejection)?;
    let result = write_agent_run_update_once(state, store, run);
    drop(persistence_permit);
    match result {
        Ok(()) => {
            epoch_permit.finish_committed();
            Ok(())
        }
        Err(error) => {
            epoch_permit.finish_failed();
            Err(error)
        }
    }
}

pub(crate) fn register_agent_run_store_result<T>(
    state: &Arc<AppState>,
    result: Result<T, String>,
) -> Result<T, String> {
    if let Err(error) = &result {
        state
            .persistence_coordinator
            .register_runtime_durable_failure("AgentRunStore", error);
    }
    result
}

/// Classifies an AgentRunStore failure that crossed a cloned-store execution
/// boundary (for example ToolGateway/AgentLoop). This is synchronous and must
/// be called before the outer path turns the error into a blocker or warning;
/// non-durable validation/policy failures are ignored by the coordinator's
/// durable-failure classifier.
pub(crate) fn register_agent_run_store_error(state: &Arc<AppState>, error: impl ToString) {
    let _ = register_agent_run_store_result::<()>(state, Err(error.to_string()));
}

async fn project_agent_run_from_typed_delta<F>(
    state: &Arc<AppState>,
    run_id: &str,
    expected_task_id: &str,
    apply: F,
) -> Result<AgentRun, String>
where
    F: FnOnce(&AgentRunStore, &mut AgentRun) -> Result<(), String>,
{
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(run_id);
    let _causal_guard = causal_lock.lock().await;
    // The test barrier scope is deliberately created only after this run's
    // causal lock is owned. A second typed delta that is waiting on (or
    // cancelled before acquiring) the same lock therefore cannot remove the
    // active operation's run-keyed barrier.
    #[cfg(test)]
    let _test_barrier_scope = AgentRunLifecycleCommitTestBarrierScope::enter(run_id);
    let store = clone_agent_run_store(state).await?;
    let mut run = load_live_agent_run(state, &store, run_id)?;
    if run.task_id != expected_task_id {
        return Err("agent_run_typed_delta_task_identity_mismatch".into());
    }
    let _fence = acquire_open_turn_write_fence(state, expected_task_id).await?;
    let before = serde_json::to_vec(&run).map_err(|error| error.to_string())?;
    apply(&store, &mut run)?;
    if serde_json::to_vec(&run).map_err(|error| error.to_string())? == before {
        return Ok(run);
    }
    let admission = admit_agent_run_write(state, AgentRunWriteLane::Normal)?;
    commit_agent_run_update(state, &store, &run, &admission).await?;
    Ok(run)
}

async fn project_main_chat_agent_run_from_typed_delta<F>(
    state: &Arc<AppState>,
    run_id: &str,
    expected_task_id: &str,
    execution_epoch: &MainChatExecutionEpoch,
    apply: F,
) -> Result<AgentRun, String>
where
    F: FnOnce(&AgentRunStore, &mut AgentRun) -> Result<(), String>,
{
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(run_id);
    let _causal_guard = causal_lock.lock().await;
    #[cfg(test)]
    let _test_barrier_scope = AgentRunLifecycleCommitTestBarrierScope::enter(run_id);
    let store = clone_agent_run_store(state).await?;
    let canonical = load_live_agent_run(state, &store, run_id)?;
    if canonical.task_id != expected_task_id {
        return Err("agent_run_typed_delta_task_identity_mismatch".into());
    }
    let _fence = acquire_open_turn_write_fence(state, expected_task_id).await?;

    if matches!(
        canonical.status,
        AgentRunStatus::Cancelled | AgentRunStatus::Completed | AgentRunStatus::Failed
    ) {
        // A terminal owner is immutable. Build the replay candidate away from
        // canonical state, then accept only exact semantic replay after the
        // store's receipt/digest normalization. No commit permit or revision
        // is consumed for that no-op.
        let mut candidate = canonical.clone();
        if apply(&store, &mut candidate).is_err() {
            return Err("main_chat_agent_run_terminal_delta_conflict".into());
        }
        return match store.typed_projection_matches_canonical(&candidate, &canonical) {
            Ok(true) => Ok(canonical),
            Ok(false) | Err(_) => Err("main_chat_agent_run_terminal_delta_conflict".into()),
        };
    }

    if !matches!(
        canonical.status,
        AgentRunStatus::Running | AgentRunStatus::WaitingPermission | AgentRunStatus::RemoteUnknown
    ) {
        return Err("main_chat_agent_run_projection_state_invalid".into());
    }

    let mut candidate = canonical.clone();
    apply(&store, &mut candidate)?;
    if store
        .typed_projection_matches_canonical(&candidate, &canonical)
        .map_err(|error| error.to_string())?
    {
        return Ok(canonical);
    }
    let admission = admit_agent_run_write(state, AgentRunWriteLane::Normal)?;
    commit_main_chat_agent_run_update(state, &store, &candidate, &admission, execution_epoch)
        .await?;
    Ok(candidate)
}

pub(crate) async fn project_main_chat_agent_run_failure(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
    failure: AgentRunMainChatFailureKind,
    safe_reason: &str,
) -> Result<(), String> {
    project_agent_run_from_typed_delta(state, run_id, task_session_id, |_store, run| {
        if run.status == AgentRunStatus::Completed {
            return Ok(());
        }
        if failure == AgentRunMainChatFailureKind::Cancelled {
            if run.status != AgentRunStatus::Cancelled {
                run.cancel();
            }
        } else {
            run.fail(AgentRunError {
                message: safe_reason.to_string(),
                phase: failure.error_phase().to_string(),
                recoverable: failure.recoverable(),
            });
        }
        Ok(())
    })
    .await
    .map(|_| ())
}

fn merge_main_chat_action_observation_delta(
    store: &AgentRunStore,
    run: &mut AgentRun,
    actions: Vec<AgentAction>,
    observations: Vec<AgentObservation>,
) -> Result<(), String> {
    // Persisted identities are already canonical receipts/references, whereas
    // new typed deltas still carry their producer identity. Keep a separate
    // canonical ownership set so same-turn observations can bind to a newly
    // supplied action without writing a pre-minimized receipt back through the
    // untrusted-input minimizer (which would hash it a second time).
    let persisted_action_owners = run
        .actions
        .iter()
        .map(|action| action.id.clone())
        .collect::<HashSet<_>>();
    let mut canonical_action_owners = persisted_action_owners.clone();
    let persisted_action_count = run.actions.len();
    let mut new_action_positions = HashMap::<String, usize>::new();
    for action in actions {
        if action.id.trim().is_empty() {
            return Err("main_chat_agent_run_action_identity_missing".into());
        }
        // A receipt-looking ID is trusted only when it exactly names an owner
        // loaded from this canonical run. Producer IDs still cross the
        // store-scoped identity boundary; arbitrary caller-supplied receipt
        // strings are therefore never accepted as canonical on shape alone.
        let canonical_action_id = if persisted_action_owners.contains(&action.id) {
            action.id.clone()
        } else {
            store.canonical_action_identity(&action.id)
        };
        if let Some(existing) = run.actions[..persisted_action_count]
            .iter()
            .find(|existing| existing.id == canonical_action_id)
        {
            if !store
                .action_delta_matches_canonical(&action, existing)
                .map_err(|error| error.to_string())?
            {
                return Err("main_chat_agent_run_action_identity_conflict".into());
            }
            continue;
        }
        if let Some(existing_position) = new_action_positions.get(&canonical_action_id) {
            if !store.action_deltas_match(&action, &run.actions[*existing_position]) {
                return Err("main_chat_agent_run_action_identity_conflict".into());
            }
            continue;
        }
        if !canonical_action_owners.insert(canonical_action_id.clone()) {
            return Err("main_chat_agent_run_action_identity_conflict".into());
        }
        new_action_positions.insert(canonical_action_id, run.actions.len());
        run.actions.push(action);
    }

    let persisted_observation_owners = run
        .observations
        .iter()
        .map(|observation| observation.id.clone())
        .collect::<HashSet<_>>();
    let mut canonical_observation_owners = persisted_observation_owners.clone();
    let persisted_observation_count = run.observations.len();
    let mut new_observation_positions = HashMap::<String, usize>::new();
    for observation in observations {
        if observation.id.trim().is_empty() {
            return Err("main_chat_agent_run_observation_identity_missing".into());
        }
        let canonical_observation_id = if persisted_observation_owners.contains(&observation.id) {
            observation.id.clone()
        } else {
            store.canonical_observation_identity(&observation.id)
        };
        if let Some(existing) = run.observations[..persisted_observation_count]
            .iter()
            .find(|existing| existing.id == canonical_observation_id)
        {
            if !store.observation_delta_matches_canonical(&observation, existing) {
                return Err("main_chat_agent_run_observation_identity_conflict".into());
            }
            continue;
        }
        if let Some(existing_position) = new_observation_positions.get(&canonical_observation_id) {
            if !store.observation_deltas_match(&observation, &run.observations[*existing_position])
            {
                return Err("main_chat_agent_run_observation_identity_conflict".into());
            }
            continue;
        }
        if !canonical_observation_owners.insert(canonical_observation_id.clone()) {
            return Err("main_chat_agent_run_observation_identity_conflict".into());
        }
        match observation.action_id.as_deref() {
            Some(action_id) => {
                let canonical_owner_id = if persisted_action_owners.contains(action_id) {
                    action_id.to_string()
                } else {
                    store.canonical_action_identity(action_id)
                };
                if !canonical_action_owners.contains(&canonical_owner_id) {
                    return Err("main_chat_agent_run_observation_action_owner_missing".into());
                }
            }
            None if observation.source != "agent_loop"
                || observation.react_trace.is_some()
                || observation
                    .structured_result
                    .as_ref()
                    .and_then(|value| value.get("error"))
                    .and_then(serde_json::Value::as_str)
                    != Some("max_tool_calls exceeded") =>
            {
                return Err("main_chat_agent_run_supplemental_observation_contract_invalid".into());
            }
            _ => {}
        }
        new_observation_positions.insert(canonical_observation_id, run.observations.len());
        run.observations.push(observation);
    }
    Ok(())
}

pub(crate) async fn project_main_chat_generation_result(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
    execution_epoch: &MainChatExecutionEpoch,
    projection: MainChatGenerationProjection,
) -> Result<(), String> {
    project_main_chat_agent_run_from_typed_delta(
        state,
        run_id,
        task_session_id,
        execution_epoch,
        move |store, run| {
            merge_main_chat_action_observation_delta(
                store,
                run,
                projection.actions,
                projection.observations,
            )?;
            run.context_summary = Some(projection.context_summary);
            run.model_route = Some(projection.model_route);
            run.output_preview = Some(projection.output_preview);
            run.reasoning_strategy = projection.reasoning_strategy;
            if projection.terminal_owner_generation == 1 {
                run.reasoning_trace = Some(projection.reasoning_trace);
            } else {
                // AgentRun reasoning_trace is immutable evidence for the first
                // generation. Continuation truth lives in ordered provider and
                // final-delivery events; replacing the original digest would
                // make a same-Run replay look like rewritten history.
                run.status_updates.push(AgentLoopStatusUpdate {
                    phase: AgentLoopPhase::Completed,
                    message: format!(
                        "Provider continuation generation {} completed under OpenLifeTurnRuntime.",
                        projection.terminal_owner_generation
                    ),
                    step_index: projection.step_count,
                    tool_call_index: None,
                    timestamp: chrono::Utc::now(),
                });
            }
            run.hs_selection_audit = projection.hs_selection_audit;
            run.behavior_checks = projection.behavior_checks;
            run.step_count = projection.step_count;
            run.tool_call_count = projection.tool_call_count;
            match run.status {
                AgentRunStatus::Running => {
                    run.status = AgentRunStatus::Completed;
                    run.finished_at = Some(chrono::Utc::now());
                }
                AgentRunStatus::WaitingPermission | AgentRunStatus::RemoteUnknown => {
                    run.finished_at = None;
                }
                _ => {}
            }
            Ok(())
        },
    )
    .await
    .map(|_| ())
}

pub(crate) async fn project_main_chat_kernel_evidence(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
    execution_epoch: &MainChatExecutionEpoch,
    projection: MainChatBlockedProjection,
) -> Result<(), String> {
    let terminal_owner_generation = execution_epoch.terminal_owner_generation()?;
    project_main_chat_agent_run_from_typed_delta(
        state,
        run_id,
        task_session_id,
        execution_epoch,
        move |store, run| {
            merge_main_chat_action_observation_delta(
                store,
                run,
                projection.actions,
                projection.observations,
            )?;
            run.reasoning_strategy = projection.reasoning_strategy;
            if terminal_owner_generation == 1 {
                run.reasoning_trace = Some(projection.reasoning_trace);
            } else {
                // The first generation owns the immutable reasoning-trace
                // digest. Continuations project their ordered tool/blocker
                // evidence without rewriting that historical trace.
                run.status_updates.push(AgentLoopStatusUpdate {
                    phase: if projection.disposition
                        == MainChatBlockedDisposition::WaitingPermission
                    {
                        AgentLoopPhase::WaitingPermission
                    } else {
                        AgentLoopPhase::Failed
                    },
                    message: format!(
                        "Kernel continuation generation {terminal_owner_generation} projected blocker evidence under OpenLifeTurnRuntime."
                    ),
                    step_index: projection.step_count,
                    tool_call_index: Some(projection.tool_call_count),
                    timestamp: chrono::Utc::now(),
                });
            }
            run.tool_call_count = projection.tool_call_count;
            run.step_count = projection.step_count;
            if projection.disposition == MainChatBlockedDisposition::WaitingPermission {
                run.status = AgentRunStatus::WaitingPermission;
                run.finished_at = None;
                run.error = None;
            }
            Ok(())
        },
    )
    .await
    .map(|_| ())
}

pub(crate) async fn begin_main_chat_agent_run_replay(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
) -> Result<(), String> {
    project_agent_run_from_typed_delta(state, run_id, task_session_id, |_store, run| {
        if !matches!(
            run.status,
            AgentRunStatus::Failed | AgentRunStatus::WaitingPermission
        ) {
            return Err(format!("canonical_replay_run_not_resumable:{}", run.status));
        }
        run.status = AgentRunStatus::Running;
        run.finished_at = None;
        run.error = None;
        run.status_updates.push(AgentLoopStatusUpdate {
            phase: AgentLoopPhase::ExecutingTool,
            message: "Governed replay execution started under OpenLifeTurnRuntime.".into(),
            step_index: run.step_count,
            tool_call_index: Some(run.tool_call_count),
            timestamp: chrono::Utc::now(),
        });
        Ok(())
    })
    .await
    .map(|_| ())
}

pub(crate) async fn project_main_chat_agent_run_replay(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
    projection: AgentRunReplayProjection,
) -> Result<(), String> {
    project_agent_run_from_typed_delta(state, run_id, task_session_id, move |_store, run| {
        if run.status != AgentRunStatus::Running {
            return Err(format!(
                "canonical_replay_run_terminal_transition_conflict:{}",
                run.status
            ));
        }
        let (status, phase, message) = match projection {
            AgentRunReplayProjection::WaitingForAnotherAction => (
                AgentRunStatus::WaitingPermission,
                AgentLoopPhase::WaitingPermission,
                "A replay action completed; another action is waiting for permission.",
            ),
            AgentRunReplayProjection::WaitingForPermission => (
                AgentRunStatus::WaitingPermission,
                AgentLoopPhase::WaitingPermission,
                "Governed replay is waiting for permission.",
            ),
            AgentRunReplayProjection::FailedUnresolvedAction => (
                AgentRunStatus::Failed,
                AgentLoopPhase::Failed,
                "A replay action completed; another required action remains unresolved.",
            ),
        };
        run.status = status;
        run.finished_at = matches!(
            status,
            AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Cancelled
        )
        .then(chrono::Utc::now);
        if projection == AgentRunReplayProjection::FailedUnresolvedAction {
            run.error = Some(AgentRunError {
                message: "replay_action_unresolved".into(),
                phase: "tool_error".into(),
                recoverable: true,
            });
        }
        run.status_updates.push(AgentLoopStatusUpdate {
            phase,
            message: message.into(),
            step_index: run.step_count,
            tool_call_index: Some(run.tool_call_count),
            timestamp: chrono::Utc::now(),
        });
        Ok(())
    })
    .await
    .map(|_| ())
}

/// Project a pre-existing AgentRun from the canonical startup task owner. The
/// caller supplies only identity; this gateway derives the transition while
/// holding the task fence and mutates lifecycle fields only.
pub(crate) async fn project_agent_run_from_startup_task_owner(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
) -> Result<(), String> {
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(run_id);
    let _causal_guard = causal_lock.lock().await;
    let _fence = acquire_terminal_owner_task_fence(task_session_id).await;
    let task = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?
        .lock()
        .await
        .load_session(task_session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("canonical_task_session_missing:{task_session_id}"))?;
    let store = clone_agent_run_store(state).await?;
    let mut run = register_agent_run_store_result(
        state,
        store.get_run(run_id).map_err(|error| error.to_string()),
    )?
    .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))?;
    if run.task_id != task_session_id || task.id != task_session_id {
        return Err("startup_agent_run_task_owner_identity_mismatch".into());
    }
    if task.status != AgentTaskSessionStatus::WaitingPermission {
        return Err("startup_agent_run_task_owner_requires_waiting_permission".into());
    }
    run.status = AgentRunStatus::WaitingPermission;
    run.finished_at = None;
    run.error = None;
    let admission = state
        .persistence_coordinator
        .admit_startup_agent_run_write()
        .map_err(|error| error.to_string())?;
    commit_agent_run_update(state, &store, &run, &admission).await
}

/// Project a pre-existing AgentRun from an exact durable startup event. The
/// event is re-read by id under the task fence; caller-shaped event values or
/// arbitrary AgentRun mutations cannot authorize this lane.
pub(crate) fn startup_agent_run_status_from_durable_event(
    evidence: &MainChatAgentDurableEvent,
) -> Result<AgentRunStatus, String> {
    let status = evidence
        .payload
        .get("status")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "startup_agent_run_durable_evidence_status_missing".to_string())?;
    if evidence.event_type != "final_delivery.created" || evidence.object_type != "final_delivery" {
        return match (
            evidence.event_type.as_str(),
            evidence.object_type.as_str(),
            status,
        ) {
            ("local_aborted", "turn", "local_aborted") => Ok(AgentRunStatus::Cancelled),
            ("failed", "turn", "failed") | ("interrupted", "turn", "interrupted") => {
                Ok(AgentRunStatus::Failed)
            }
            _ => Err("startup_agent_run_durable_evidence_transition_invalid".into()),
        };
    }

    let explicit_owner = evidence
        .payload
        .get("runOwnerStatus")
        .and_then(serde_json::Value::as_str)
        .map(|owner| match owner {
            "completed" => Ok(AgentRunStatus::Completed),
            "waiting_permission" => Ok(AgentRunStatus::WaitingPermission),
            "failed" => Ok(AgentRunStatus::Failed),
            "cancelled" => Ok(AgentRunStatus::Cancelled),
            _ => Err("startup_final_delivery_run_owner_status_invalid".to_string()),
        })
        .transpose()?;
    let projected = explicit_owner.unwrap_or(match status {
        "completed" => AgentRunStatus::Completed,
        "completed_with_pending_items" => AgentRunStatus::WaitingPermission,
        "blocked" | "failed" | "interrupted" => AgentRunStatus::Failed,
        "cancelled" => AgentRunStatus::Cancelled,
        _ => AgentRunStatus::RemoteUnknown,
    });
    let compatible = match status {
        "completed" => projected == AgentRunStatus::Completed,
        "completed_with_pending_items" => matches!(
            projected,
            AgentRunStatus::Completed | AgentRunStatus::WaitingPermission
        ),
        "blocked" | "failed" | "interrupted" => projected == AgentRunStatus::Failed,
        "cancelled" => projected == AgentRunStatus::Cancelled,
        _ => false,
    };
    if !compatible {
        return Err("startup_final_delivery_run_owner_status_incompatible".into());
    }
    Ok(projected)
}

pub(crate) async fn project_agent_run_from_startup_durable_event(
    state: &Arc<AppState>,
    evidence: &MainChatAgentDurableEvent,
) -> Result<(), String> {
    let causal_lock = state
        .persistence_coordinator
        .agent_run_causal_lock(&evidence.run_id);
    let _causal_guard = causal_lock.lock().await;
    let _fence = acquire_terminal_owner_task_fence(&evidence.task_session_id).await;
    let (stored_evidence, lifecycle) = {
        let event_store = state
            .main_chat_agent_event_store
            .as_ref()
            .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?
            .lock()
            .await;
        let stored_evidence = event_store
            .event_by_id(&evidence.event_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "startup_agent_run_durable_evidence_missing".to_string())?;
        let lifecycle = event_store
            .turn_lifecycle_snapshot(&evidence.task_session_id)
            .map_err(|error| error.to_string())?;
        (stored_evidence, lifecycle)
    };
    if stored_evidence != *evidence {
        return Err("startup_agent_run_durable_evidence_mismatch".into());
    }
    if lifecycle.bound_run_id.as_deref() != Some(evidence.run_id.as_str())
        || lifecycle
            .lifecycle_event
            .as_ref()
            .map(|event| (event.event_id.as_str(), event.sequence))
            != Some((evidence.event_id.as_str(), evidence.sequence))
    {
        return Err("startup_agent_run_durable_evidence_stale".into());
    }
    let store = clone_agent_run_store(state).await?;
    let mut run = register_agent_run_store_result(
        state,
        store
            .get_run(&evidence.run_id)
            .map_err(|error| error.to_string()),
    )?
    .ok_or_else(|| format!("canonical_agent_run_missing:{}", evidence.run_id))?;
    if run.task_id != evidence.task_session_id {
        return Err("startup_agent_run_durable_evidence_identity_mismatch".into());
    }
    let status = evidence
        .payload
        .get("status")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "startup_agent_run_durable_evidence_status_missing".to_string())?;
    match startup_agent_run_status_from_durable_event(evidence)? {
        AgentRunStatus::Completed => {
            run.status = AgentRunStatus::Completed;
            run.finished_at = Some(evidence.created_at);
            run.error = None;
        }
        AgentRunStatus::WaitingPermission => {
            run.status = AgentRunStatus::WaitingPermission;
            run.finished_at = None;
            run.error = None;
        }
        AgentRunStatus::Cancelled => run.cancel(),
        AgentRunStatus::Failed => {
            run.fail(AgentRunError {
                message: "Recovered terminal lifecycle from an exact durable startup receipt."
                    .into(),
                phase: "startup_durable_event_projection".into(),
                recoverable: status == "blocked" || status == "failed",
            });
        }
        AgentRunStatus::Running | AgentRunStatus::RemoteUnknown => {
            return Err("startup_agent_run_durable_evidence_transition_invalid".into())
        }
    }
    if run.status != AgentRunStatus::WaitingPermission {
        run.finished_at = Some(evidence.created_at);
    }
    let admission = state
        .persistence_coordinator
        .admit_startup_agent_run_write()
        .map_err(|error| error.to_string())?;
    commit_agent_run_update(state, &store, &run, &admission).await
}

pub(crate) async fn update_agent_run_after_review_reconciliation(
    state: &Arc<AppState>,
    proposal_id: &str,
    run_id: &str,
) -> Result<(), String> {
    update_agent_run_after_review_reconciliation_inner(
        state,
        proposal_id,
        run_id,
        AgentRunWriteLane::Normal,
    )
    .await
}

pub(crate) async fn update_agent_run_after_startup_review_reconciliation(
    state: &Arc<AppState>,
    proposal_id: &str,
    run_id: &str,
) -> Result<(), String> {
    update_agent_run_after_review_reconciliation_inner(
        state,
        proposal_id,
        run_id,
        AgentRunWriteLane::StartupReconciliation,
    )
    .await
}

fn load_live_agent_run(
    state: &Arc<AppState>,
    store: &AgentRunStore,
    run_id: &str,
) -> Result<AgentRun, String> {
    let run = register_agent_run_store_result(
        state,
        store.get_run(run_id).map_err(|error| error.to_string()),
    )?
    .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))?;
    if run.deleted_at.is_some() {
        return Err("agent_run_review_projection_owner_inactive".into());
    }
    Ok(run)
}

fn review_owned_error(error: &AgentRunError) -> bool {
    matches!(
        error.phase.as_str(),
        "review_staging"
            | "review_staging_partial"
            | "review_partial_effect"
            | "review_effect_unknown"
            | "review_failed_before_effect"
            | "review_projection_pending"
    )
}

fn clear_review_owned_error(run: &mut AgentRun) {
    if run.error.as_ref().is_some_and(review_owned_error) {
        run.error = None;
    }
}

fn upsert_review_count_receipt(
    store: &AgentRunStore,
    run: &mut AgentRun,
    message: &str,
    primary_count: usize,
    secondary_count: usize,
) {
    let primary_count = u32::try_from(primary_count).unwrap_or(u32::MAX);
    let secondary_count = u32::try_from(secondary_count).unwrap_or(u32::MAX);
    if run.status_updates.last().is_some_and(|update| {
        update.phase == AgentLoopPhase::Failed
            && store.status_update_message_matches(&update.message, message)
            && update.step_index == primary_count
            && update.tool_call_index == Some(secondary_count)
    }) {
        return;
    }
    run.status_updates.push(AgentLoopStatusUpdate {
        phase: AgentLoopPhase::Failed,
        message: message.into(),
        step_index: primary_count,
        tool_call_index: Some(secondary_count),
        timestamp: chrono::Utc::now(),
    });
}

async fn canonical_agent_run_review_counts(
    state: &Arc<AppState>,
    proposal_ids: &[String],
) -> Result<CanonicalAgentRunReviewCounts, String> {
    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "proposal_store_unavailable".to_string())?
        .lock()
        .await;
    let mut counts = CanonicalAgentRunReviewCounts::default();
    for proposal_id in proposal_ids {
        let Some(proposal) = proposal_store
            .get_proposal(proposal_id)
            .map_err(|error| error.to_string())?
        else {
            counts.unknown = counts.unknown.saturating_add(1);
            continue;
        };
        let evidence_time = match proposal.status {
            ProposalStatus::Expired => proposal
                .resolved_at
                .unwrap_or(proposal.expires_at.unwrap_or(proposal.created_at)),
            _ => proposal.resolved_at.unwrap_or(proposal.created_at),
        };
        counts.observe_evidence_time(evidence_time);
        let dispatch_state = proposal_store
            .dispatch_state(proposal_id)
            .map_err(|error| error.to_string())?;
        match dispatch_state.as_deref() {
            Some("claimed") => {
                counts.claimed = counts.claimed.saturating_add(1);
            }
            Some("unknown") | None => {
                counts.unknown = counts.unknown.saturating_add(1);
            }
            Some("failed_before_effect") => {
                counts.failed_before_effect = counts.failed_before_effect.saturating_add(1);
            }
            Some("confirmed_projection_pending") => {
                counts.projection_pending = counts.projection_pending.saturating_add(1);
            }
            Some("confirmed") if proposal.status == ProposalStatus::Accepted => {
                counts.confirmed = counts.confirmed.saturating_add(1);
                if proposal_store
                    .terminal_relation_projection_proof(proposal_id)
                    .map_err(|error| error.to_string())?
                    .is_some_and(|proof| {
                        proof.relation_kind()
                            == ProposalTerminalRelationKind::ActionResumePrerequisite
                    })
                {
                    counts.confirmed_action_resume =
                        counts.confirmed_action_resume.saturating_add(1);
                }
                let resolved_at = proposal.resolved_at.ok_or_else(|| {
                    "agent_run_review_projection_resolution_time_missing".to_string()
                })?;
                counts.observe_evidence_time(resolved_at);
            }
            Some("confirmed") => {
                // A confirmed external effect without an accepted canonical
                // Proposal projection is not a decline and not completion.
                counts.projection_pending = counts.projection_pending.saturating_add(1);
            }
            Some("unclaimed") => match proposal.status {
                ProposalStatus::Rejected => {
                    counts.rejected = counts.rejected.saturating_add(1);
                }
                ProposalStatus::Expired => {
                    counts.expired = counts.expired.saturating_add(1);
                }
                ProposalStatus::Pending
                | ProposalStatus::Postponed
                | ProposalStatus::Edited
                | ProposalStatus::Accepted => {
                    counts.waiting = counts.waiting.saturating_add(1);
                }
            },
            Some(_) => {
                // New or corrupt dispatch states are not safe to reinterpret
                // as either unclaimed or confirmed.
                counts.unknown = counts.unknown.saturating_add(1);
            }
        }
    }
    Ok(counts)
}

async fn unresolved_agent_run_action_count(
    state: &Arc<AppState>,
    task_id: &str,
) -> Result<usize, String> {
    let Some(action_queue_store) = state.main_chat_action_queue_store.as_ref() else {
        return Ok(0);
    };
    Ok(action_queue_store
        .lock()
        .await
        .list_for_session(task_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|action| action.status != ExecutionQueueStatus::Completed)
        .count())
}

async fn canonical_task_waiting_for_action_resume(
    state: &Arc<AppState>,
    task_id: &str,
) -> Result<bool, String> {
    let task = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?
        .lock()
        .await
        .load_session(task_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("canonical_task_session_missing:{task_id}"))?;
    Ok(task.status == AgentTaskSessionStatus::WaitingPermission)
}

fn terminal_review_projection_time(
    lane: AgentRunWriteLane,
    counts: CanonicalAgentRunReviewCounts,
) -> Result<chrono::DateTime<chrono::Utc>, String> {
    match lane {
        AgentRunWriteLane::Normal => Ok(chrono::Utc::now()),
        AgentRunWriteLane::StartupReconciliation => counts
            .terminal_evidence_time
            .ok_or_else(|| "agent_run_startup_review_projection_time_evidence_missing".into()),
    }
}

fn has_exact_review_failure(
    store: &AgentRunStore,
    run: &AgentRun,
    status: AgentRunStatus,
    message: &str,
    phase: &str,
    recoverable: bool,
) -> bool {
    run.status == status
        && run.finished_at.is_some()
        && run.error.as_ref().is_some_and(|error| {
            error.phase == phase
                && error.recoverable == recoverable
                && store.run_error_message_matches(&error.message, message)
        })
}

fn set_review_failure(
    run: &mut AgentRun,
    status: AgentRunStatus,
    finished_at: chrono::DateTime<chrono::Utc>,
    message: &str,
    phase: &str,
    recoverable: bool,
) {
    run.status = status;
    run.finished_at = Some(finished_at);
    run.error = Some(AgentRunError {
        message: message.into(),
        phase: phase.into(),
        recoverable,
    });
}

async fn project_canonical_review_lifecycle(
    state: &Arc<AppState>,
    store: &AgentRunStore,
    run: &mut AgentRun,
    lane: AgentRunWriteLane,
) -> Result<(), String> {
    let counts = canonical_agent_run_review_counts(state, &run.generated_proposals).await?;
    let unresolved_action_count = unresolved_agent_run_action_count(state, &run.task_id).await?;
    let task_waiting_for_action_resume = if counts.confirmed_action_resume > 0 {
        canonical_task_waiting_for_action_resume(state, &run.task_id).await?
    } else {
        false
    };
    let staging_failed = run
        .error
        .as_ref()
        .is_some_and(|error| error.phase == "review_staging_partial");
    // A failure owned by another phase is canonical and must not be erased by
    // review projection. Proposal links still commit from the re-read row.
    if run.status == AgentRunStatus::Failed
        && run
            .error
            .as_ref()
            .is_some_and(|error| !review_owned_error(error))
    {
        return Ok(());
    }

    if counts.effect_unknown() > 0 {
        if !has_exact_review_failure(
            store,
            run,
            AgentRunStatus::RemoteUnknown,
            "review_effect_state_unknown",
            "review_effect_unknown",
            true,
        ) {
            set_review_failure(
                run,
                AgentRunStatus::RemoteUnknown,
                terminal_review_projection_time(lane, counts)?,
                "review_effect_state_unknown",
                "review_effect_unknown",
                true,
            );
        }
        upsert_review_count_receipt(
            store,
            run,
            "review_effect_unknown_count_receipt",
            counts.unknown,
            counts.claimed,
        );
    } else if counts.projection_pending > 0 {
        if !has_exact_review_failure(
            store,
            run,
            AgentRunStatus::Failed,
            "review_projection_pending",
            "review_projection_pending",
            true,
        ) {
            set_review_failure(
                run,
                AgentRunStatus::Failed,
                terminal_review_projection_time(lane, counts)?,
                "review_projection_pending",
                "review_projection_pending",
                true,
            );
        }
        upsert_review_count_receipt(
            store,
            run,
            "review_projection_pending_count_receipt",
            counts.projection_pending,
            counts.confirmed,
        );
    } else if counts.failed_before_effect > 0 && counts.confirmed == 0 {
        if !has_exact_review_failure(
            store,
            run,
            AgentRunStatus::Failed,
            "review_failed_before_effect",
            "review_failed_before_effect",
            true,
        ) {
            set_review_failure(
                run,
                AgentRunStatus::Failed,
                terminal_review_projection_time(lane, counts)?,
                "review_failed_before_effect",
                "review_failed_before_effect",
                true,
            );
        }
        upsert_review_count_receipt(
            store,
            run,
            "review_failed_before_effect_count_receipt",
            counts.failed_before_effect,
            counts.waiting,
        );
    } else if counts.waiting == 0
        && counts.confirmed > 0
        && (counts.declined() > 0 || counts.failed_before_effect > 0 || staging_failed)
    {
        debug_assert!(counts.terminal_without_unknown_or_waiting());
        if !has_exact_review_failure(
            store,
            run,
            AgentRunStatus::Failed,
            "review_partial_effect",
            "review_partial_effect",
            false,
        ) {
            set_review_failure(
                run,
                AgentRunStatus::Failed,
                terminal_review_projection_time(lane, counts)?,
                "review_partial_effect",
                "review_partial_effect",
                false,
            );
        }
        upsert_review_count_receipt(
            store,
            run,
            "review_partial_effect_count_receipt",
            counts.confirmed,
            counts
                .declined()
                .saturating_add(counts.failed_before_effect)
                .saturating_add(usize::from(staging_failed)),
        );
    } else if counts.waiting > 0 {
        run.status = AgentRunStatus::WaitingPermission;
        run.finished_at = None;
        if !staging_failed {
            clear_review_owned_error(run);
        }
    } else if counts.confirmed > 0 {
        debug_assert!(counts.terminal_without_unknown_or_waiting());
        match () {
            _ if task_waiting_for_action_resume => {
                // An accepted ActionResumePrerequisite grants only the exact
                // continuation capability. The explicit replay owner below
                // is responsible for moving this run back to Running; Review
                // projection must not pre-emptively mark it Completed. The
                // canonical TaskSession is used so startup reconciliation can
                // also repair a previously misprojected Completed AgentRun.
                run.status = AgentRunStatus::WaitingPermission;
                run.finished_at = None;
                clear_review_owned_error(run);
            }
            _ if unresolved_action_count > 0 => {
                run.status = AgentRunStatus::WaitingPermission;
                run.finished_at = None;
                clear_review_owned_error(run);
            }
            _ => {
                run.status = AgentRunStatus::Completed;
                run.finished_at = Some(terminal_review_projection_time(lane, counts)?);
                clear_review_owned_error(run);
            }
        }
    } else if counts.declined() > 0 {
        debug_assert!(counts.terminal_without_unknown_or_waiting());
        if staging_failed {
            run.status = AgentRunStatus::Failed;
            if run.finished_at.is_none() {
                run.finished_at = Some(terminal_review_projection_time(lane, counts)?);
            }
        } else {
            run.cancel();
            run.finished_at = Some(terminal_review_projection_time(lane, counts)?);
            clear_review_owned_error(run);
        }
    } else if run.error.as_ref().is_some_and(review_owned_error) {
        run.status = AgentRunStatus::Failed;
        run.finished_at = Some(chrono::Utc::now());
    } else {
        run.status = AgentRunStatus::Completed;
        run.finished_at = Some(chrono::Utc::now());
        run.error = None;
    }
    Ok(())
}

async fn update_agent_run_after_review_reconciliation_inner(
    state: &Arc<AppState>,
    proposal_id: &str,
    run_id: &str,
    lane: AgentRunWriteLane,
) -> Result<(), String> {
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(run_id);
    let _causal_guard = causal_lock.lock().await;
    let store = clone_agent_run_store(state).await?;
    let mut run = load_live_agent_run(state, &store, run_id)?;
    let canonical_task_id = run.task_id.clone();
    let _fence = acquire_terminal_owner_task_fence(&canonical_task_id).await;
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
            let relation = proposal_store
                .terminal_relation_projection_proof(proposal_id)
                .map_err(|error| error.to_string())?;
            if relation.as_ref().is_some_and(|relation| {
                relation.task_session_id() != origin.task_session_id()
                    || relation.run_id() != origin.run_id()
            }) {
                return Err("terminal_owner_agent_run_projection_relation_origin_mismatch".into());
            }
            let relation_kind = relation.map(|relation| relation.relation_kind());
            drop(proposal_store);
            if proposal.status == openlife_core::agent::ProposalStatus::Accepted {
                if dispatch_state.as_deref() != Some("confirmed") {
                    return Err("terminal_owner_agent_run_projection_effect_not_confirmed".into());
                }
                match relation_kind {
                    Some(
                        ProposalTerminalRelationKind::NonBlockingSuccessor
                        | ProposalTerminalRelationKind::ActionResumePrerequisite,
                    ) => {}
                    Some(ProposalTerminalRelationKind::EffectBlockingPrerequisite) | None => {
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
                            return Err(
                                "terminal_owner_agent_run_projection_successor_missing".into()
                            );
                        }
                    }
                    Some(ProposalTerminalRelationKind::LegacyUnclassified) => {
                        return Err("terminal_owner_agent_run_projection_relation_unproven".into());
                    }
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

    if !run.generated_proposals.iter().any(|id| id == proposal_id) {
        return Err("agent_run_review_projection_proposal_not_linked".into());
    }
    let before_projection = serde_json::to_vec(&run).map_err(|error| error.to_string())?;
    project_canonical_review_lifecycle(state, &store, &mut run, lane).await?;
    if serde_json::to_vec(&run).map_err(|error| error.to_string())? == before_projection {
        return Ok(());
    }
    let admission = admit_agent_run_write(state, lane)?;
    commit_agent_run_update(state, &store, &run, &admission).await
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

/// The only Tauri-side entry for creating an AgentRun that has no canonical
/// Conversation input proof. The process-local causal and task fences close
/// duplicate in-flight creates; the generation-bound persistence permit is
/// acquired immediately before the synchronous owner transaction.
pub(crate) async fn create_agent_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(&run.id);
    let _causal_guard = causal_lock.lock().await;
    let _task_fence = acquire_open_turn_write_fence(state, &run.task_id).await?;

    // Clone the synchronous owner behind a short Tokio guard. No owner guard
    // may cross the admission/permit awaits below.
    let store = {
        state
            .agent_run_store
            .as_ref()
            .ok_or_else(|| "agent_run_store_unavailable".to_string())?
            .lock()
            .await
            .clone()
    };
    let admission = state
        .persistence_coordinator
        .admit_agent_run_write()
        .map_err(|error| error.to_string())?;
    #[cfg(test)]
    wait_at_agent_run_lifecycle_commit_test_barrier(&run.id).await;
    let _commit_permit = state
        .persistence_coordinator
        .acquire_agent_run_commit_permit(&admission)
        .await
        .map_err(|error| error.to_string())?;
    register_agent_run_store_result(
        state,
        store.create_run(run).map_err(|error| error.to_string()),
    )
}

/// Main Chat AgentRun creation keeps the canonical Conversation proof seam in
/// Core while sharing the same Tauri persistence admission and lock order as
/// every other AgentRun create. The final synchronous helper owns the existing
/// MemoryStore connection -> AgentRunStore connection order.
pub(crate) async fn create_conversation_bound_agent_run(
    state: &Arc<AppState>,
    run: &AgentRun,
    message_commit: &openlife_core::memory::CanonicalConversationMessageCommit,
) -> Result<(), String> {
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(&run.id);
    let _causal_guard = causal_lock.lock().await;
    let _task_fence = acquire_open_turn_write_fence(state, &run.task_id).await?;

    // Clone both synchronous owners with non-overlapping Tokio guards. The
    // Core proof seam performs no await while holding either SQLite owner.
    let memory_store = { state.memory_store.lock().await.clone() };
    let store = {
        state
            .agent_run_store
            .as_ref()
            .ok_or_else(|| "agent_run_store_unavailable".to_string())?
            .lock()
            .await
            .clone()
    };
    let admission = state
        .persistence_coordinator
        .admit_agent_run_write()
        .map_err(|error| error.to_string())?;
    #[cfg(test)]
    wait_at_agent_run_lifecycle_commit_test_barrier(&run.id).await;
    let _commit_permit = state
        .persistence_coordinator
        .acquire_agent_run_commit_permit(&admission)
        .await
        .map_err(|error| error.to_string())?;
    register_agent_run_store_result(
        state,
        memory_store
            .create_agent_run_from_active_conversation_message(&store, run, message_commit.proof())
            .map_err(|error| error.to_string()),
    )
}

pub(crate) async fn delete_agent_run_with_tombstone(
    state: &Arc<AppState>,
    run_id: &str,
    reason: Option<&str>,
    admission: &AgentRunCanonicalWriteAdmission,
) -> Result<CanonicalMutationReceipt, String> {
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(run_id);
    let _causal_guard = causal_lock.lock().await;
    let store = clone_agent_run_store(state).await?;
    let task_session_id = register_agent_run_store_result(
        state,
        store
            .lifecycle_task_id(run_id)
            .map_err(|error| error.to_string()),
    )?
    .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))?;
    let _fence = acquire_terminal_owner_task_fence(&task_session_id).await;
    reject_agent_run_lifecycle_race_with_sealing(state, &task_session_id).await?;
    let current = register_agent_run_store_result(
        state,
        store.get_run(run_id).map_err(|error| error.to_string()),
    )?
    .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))?;
    if current.task_id != task_session_id {
        return Err("agent_run_lifecycle_task_binding_changed".into());
    }
    #[cfg(test)]
    wait_at_agent_run_lifecycle_commit_test_barrier(run_id).await;
    let commit_permit = state
        .persistence_coordinator
        .acquire_agent_run_commit_permit(admission)
        .await
        .map_err(|error| error.to_string())?;
    let result = register_agent_run_store_result(
        state,
        store
            .delete_run_with_tombstone(run_id, reason)
            .map_err(|error| error.to_string()),
    );
    drop(commit_permit);
    result
}

pub(crate) async fn restore_agent_run_with_receipt(
    state: &Arc<AppState>,
    run_id: &str,
    admission: &AgentRunCanonicalWriteAdmission,
) -> Result<CanonicalMutationReceipt, String> {
    let causal_lock = state.persistence_coordinator.agent_run_causal_lock(run_id);
    let _causal_guard = causal_lock.lock().await;
    let store = clone_agent_run_store(state).await?;
    let memory_store = { state.memory_store.lock().await.clone() };
    let task_session_id = register_agent_run_store_result(
        state,
        store
            .lifecycle_task_id(run_id)
            .map_err(|error| error.to_string()),
    )?
    .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))?;
    let _fence = acquire_terminal_owner_task_fence(&task_session_id).await;
    reject_agent_run_lifecycle_race_with_sealing(state, &task_session_id).await?;
    let current_task_id = register_agent_run_store_result(
        state,
        store
            .lifecycle_task_id(run_id)
            .map_err(|error| error.to_string()),
    )?
    .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))?;
    if current_task_id != task_session_id {
        return Err("agent_run_lifecycle_task_binding_changed".into());
    }
    #[cfg(test)]
    wait_at_agent_run_lifecycle_commit_test_barrier(run_id).await;
    let commit_permit = state
        .persistence_coordinator
        .acquire_agent_run_commit_permit(admission)
        .await
        .map_err(|error| error.to_string())?;
    let result = register_agent_run_store_result(
        state,
        memory_store
            .restore_agent_run_with_parent_conversation_fence(&store, run_id)
            .map_err(|error| error.to_string()),
    );
    drop(commit_permit);
    result
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
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

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
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

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
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

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
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
    workspace_memory_root: Option<String>,
    project_memory_root: Option<String>,
    action_queue_store: Option<Arc<tokio::sync::Mutex<ActionQueueStore>>>,
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
        let (workspace_memory_root, project_memory_root) = {
            let config = state.config.lock().await;
            (
                config.system.workspace_memory_root.clone(),
                config.system.project_memory_root.clone(),
            )
        };
        let mut gateway = Self::new(&event_store, &task_store, &proposal_store, &memory_store)
            .with_memory_scope_roots(workspace_memory_root, project_memory_root);
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
            workspace_memory_root: None,
            project_memory_root: None,
            action_queue_store: None,
        }
    }

    fn with_memory_scope_roots(
        mut self,
        workspace_memory_root: Option<String>,
        project_memory_root: Option<String>,
    ) -> Self {
        self.workspace_memory_root = workspace_memory_root;
        self.project_memory_root = project_memory_root;
        self
    }

    pub(crate) fn with_action_queue_store(
        mut self,
        action_queue_store: Arc<tokio::sync::Mutex<ActionQueueStore>>,
    ) -> Self {
        self.action_queue_store = Some(action_queue_store);
        self
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

    fn conversation_owner_id_for_task(&self, task_session_id: &str) -> anyhow::Result<String> {
        self.task_store
            .chat_session_id_for_task(task_session_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_task_session_missing"))
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
            .terminal_owner_final_event(origin.task_session_id())?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_event_missing"))?;
        if final_event.event_id != final_event_id {
            anyhow::bail!("terminal_owner_final_event_identity_mismatch");
        }
        let existing_receipt = self
            .task_store
            .terminal_owner_transition_receipt_for_claim(proposal_id, claim_id)?;
        let (before_revision, before_digest) = if let Some(receipt) = existing_receipt.as_ref() {
            (
                receipt.before_revision(),
                receipt.before_digest().to_string(),
            )
        } else {
            let event_head = self
                .event_store
                .terminal_owner_successor_head(origin.task_session_id())?;
            let task_head = self
                .task_store
                .canonical_owner_head(origin.task_session_id())?
                .ok_or_else(|| anyhow::anyhow!("terminal_owner_task_head_missing"))?;
            if task_head.revision() != event_head.0 || task_head.digest() != event_head.1.as_str() {
                anyhow::bail!("terminal_owner_task_head_unproven_drift");
            }
            event_head
        };
        let receipt = if let Some(receipt) = existing_receipt {
            receipt
        } else {
            let complete_when_unblocked = self
                .complete_when_unblocked(origin.task_session_id())
                .await?;
            self.task_store.apply_terminal_owner_review_transition(
                proposal_id,
                claim_id,
                origin.task_session_id(),
                before_revision,
                &before_digest,
                complete_when_unblocked,
            )?
        };
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
        let successor = self.event_store.append_terminal_owner_successor(
            origin.task_session_id(),
            origin.run_id(),
            proposal_id,
            "proposal_review_acceptance",
            &receipt,
        )?;
        Ok(transition_from_receipt(receipt, successor))
    }

    pub(crate) async fn apply_blocking_review_rejection(
        &self,
        proposal_id: &str,
    ) -> anyhow::Result<TerminalOwnerReviewTransition> {
        let proposal = self
            .proposal_store
            .get_proposal(proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_rejected_proposal_missing"))?;
        if proposal.status != openlife_core::agent::ProposalStatus::Rejected {
            anyhow::bail!("terminal_owner_review_rejection_not_canonical");
        }
        let projection = self
            .proposal_store
            .terminal_relation_projection_proof(proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("typed_terminal_owner_relation_missing"))?;
        if !matches!(
            projection.relation_kind(),
            openlife_core::agent::ProposalTerminalRelationKind::EffectBlockingPrerequisite
                | openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite
        ) {
            anyhow::bail!("terminal_owner_review_rejection_relation_mismatch");
        }
        let origin = self
            .proposal_store
            .terminal_owner_origin_binding(proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_origin_binding_missing"))?;
        if projection.task_session_id() != origin.task_session_id()
            || projection.run_id() != origin.run_id()
        {
            anyhow::bail!("terminal_owner_review_rejection_origin_mismatch");
        }
        let epoch = self
            .event_store
            .terminal_owner_epoch(origin.task_session_id())?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_epoch_missing"))?;
        if epoch.run_id() != origin.run_id() || epoch.state() != TerminalOwnerSealState::Sealed {
            anyhow::bail!("terminal_owner_review_rejection_requires_sealed_epoch");
        }
        let final_event_id = epoch
            .final_event_id()
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_event_missing"))?;
        let final_event = self
            .event_store
            .terminal_owner_final_event(origin.task_session_id())?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_final_event_missing"))?;
        if final_event.event_id != final_event_id {
            anyhow::bail!("terminal_owner_final_event_identity_mismatch");
        }
        const REJECTION_DECISION_REF: &str = "review-rejected";
        let existing_receipt = self
            .task_store
            .terminal_owner_transition_receipt_for_claim(proposal_id, REJECTION_DECISION_REF)?;
        let receipt = if let Some(receipt) = existing_receipt {
            receipt
        } else {
            let event_head = self
                .event_store
                .terminal_owner_successor_head(origin.task_session_id())?;
            let task_head = self
                .task_store
                .canonical_owner_head(origin.task_session_id())?
                .ok_or_else(|| anyhow::anyhow!("terminal_owner_task_head_missing"))?;
            if task_head.revision() != event_head.0 || task_head.digest() != event_head.1.as_str() {
                anyhow::bail!("terminal_owner_task_head_unproven_drift");
            }
            if let Some(action_queue_store) = self.action_queue_store.as_ref() {
                action_queue_store.lock().await.cancel_session_nonterminal(
                    origin.task_session_id(),
                    Some(serde_json::json!({
                        "proposalRejected": true,
                        "proposalId": proposal_id,
                        "taskSessionId": origin.task_session_id(),
                        "directWritesExecuted": false,
                    })),
                )?;
            }
            self.task_store
                .apply_terminal_owner_review_rejection_transition(
                    proposal_id,
                    origin.task_session_id(),
                    event_head.0,
                    &event_head.1,
                )?
        };
        let successor = self.event_store.append_terminal_owner_successor(
            origin.task_session_id(),
            origin.run_id(),
            proposal_id,
            "proposal_review_rejection",
            &receipt,
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
                let content = proposal
                    .after
                    .get("content")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("terminal_owner_memory_content_missing"))?;
                let conversation_owner_id =
                    self.conversation_owner_id_for_task(origin.task_session_id())?;
                let mut input =
                    MemoryLifecycleAcceptanceInput::from_memory_proposal_with_terminal_origin(
                        &proposal,
                        content.to_string(),
                        origin.task_session_id(),
                        &conversation_owner_id,
                        origin.run_id(),
                        origin.canonical_user_message_ref(),
                        origin.canonical_user_message_digest(),
                    )?;
                openlife_core::agent::bind_memory_fact_scope_owner(
                    &mut input.fact,
                    Some(&conversation_owner_id),
                    self.workspace_memory_root.as_deref(),
                    self.project_memory_root.as_deref(),
                )?;
                self.memory_store.accept_memory_proposal(input)?;
            }
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
            if !self
                .proposal_store
                .project_confirmed_effect(&accepted, &claim_id)?
            {
                anyhow::bail!("terminal_owner_proposal_projection_cas_lost");
            }
        }
        Ok(transition)
    }

    /// Converges a typed Review acceptance whose relation explicitly does not
    /// mutate the originating TaskSession at acceptance time.
    ///
    /// `NonBlockingSuccessor` belongs to an already-independent turn, while
    /// `ActionResumePrerequisite` keeps the task waiting until the user starts
    /// the separately governed replay. Neither relation may be forced through
    /// the legacy proposal-blocker transition merely because it has a terminal
    /// owner origin.
    pub(crate) async fn apply_claimed_review_without_task_transition(
        &self,
        acceptance: ClaimedReviewAcceptanceSnapshot,
    ) -> anyhow::Result<openlife_core::agent::ProposalTerminalRelationKind> {
        acceptance.validate()?;
        let proposal = acceptance.proposal().clone();
        let proposal_id = proposal.id.clone();
        let projection = self
            .proposal_store
            .terminal_relation_projection_proof(&proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("typed_terminal_owner_relation_missing"))?;
        let relation_kind = projection.relation_kind();
        if !matches!(
            relation_kind,
            openlife_core::agent::ProposalTerminalRelationKind::NonBlockingSuccessor
                | openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite
        ) {
            anyhow::bail!("typed_terminal_owner_relation_requires_task_transition");
        }
        let origin = acceptance
            .terminal_owner_origin()
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_origin_binding_missing"))?;
        if projection.task_session_id() != origin.task_session_id()
            || projection.run_id() != origin.run_id()
        {
            anyhow::bail!("typed_terminal_owner_relation_origin_mismatch");
        }
        let claim_id = self
            .proposal_store
            .dispatch_claim_id(&proposal_id)?
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_dispatch_claim_missing"))?;
        let mut dispatch_state = self
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
                let content = proposal
                    .after
                    .get("content")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("terminal_owner_memory_content_missing"))?;
                let conversation_owner_id =
                    self.conversation_owner_id_for_task(origin.task_session_id())?;
                let mut input =
                    MemoryLifecycleAcceptanceInput::from_memory_proposal_with_terminal_origin(
                        &proposal,
                        content.to_string(),
                        origin.task_session_id(),
                        &conversation_owner_id,
                        origin.run_id(),
                        origin.canonical_user_message_ref(),
                        origin.canonical_user_message_digest(),
                    )?;
                openlife_core::agent::bind_memory_fact_scope_owner(
                    &mut input.fact,
                    Some(&conversation_owner_id),
                    self.workspace_memory_root.as_deref(),
                    self.project_memory_root.as_deref(),
                )?;
                self.memory_store.accept_memory_proposal(input)?;
            }
            if dispatch_state == "claimed" {
                if !self
                    .proposal_store
                    .mark_effect_confirmed_projection_pending(&proposal_id, &claim_id)?
                {
                    anyhow::bail!("terminal_owner_proposal_checkpoint_cas_lost");
                }
                dispatch_state = "confirmed_projection_pending".into();
            }
        } else if dispatch_state != "confirmed_projection_pending" {
            anyhow::bail!("terminal_owner_non_memory_effect_not_materialized");
        }
        if dispatch_state == "confirmed_projection_pending" {
            let mut accepted = proposal;
            accepted.accept();
            if !self
                .proposal_store
                .project_confirmed_effect(&accepted, &claim_id)?
            {
                anyhow::bail!("terminal_owner_proposal_projection_cas_lost");
            }
        }
        Ok(relation_kind)
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
            let relation_kind = self
                .proposal_store
                .terminal_relation_projection_proof(&proposal.id)?
                .map(|proof| proof.relation_kind());
            let requires_task_transition = relation_kind.is_none()
                || relation_kind
                    == Some(
                        openlife_core::agent::ProposalTerminalRelationKind::EffectBlockingPrerequisite,
                    );
            if proposal.proposal_type == ProposalType::MemoryWrite
                && self
                    .memory_store
                    .get_record_by_proposal_id(&proposal.id)?
                    .is_none()
            {
                before_memory += 1;
            }
            if requires_task_transition
                && self
                    .task_store
                    .terminal_owner_transition_receipt_for_claim(&proposal.id, claim_id)?
                    .is_none()
            {
                before_task += 1;
            }
        }
        let mut before_successors = 0;
        for (proposal, _, _) in &candidates {
            let relation_kind = self
                .proposal_store
                .terminal_relation_projection_proof(&proposal.id)?
                .map(|proof| proof.relation_kind());
            if relation_kind.is_some()
                && relation_kind
                    != Some(
                        openlife_core::agent::ProposalTerminalRelationKind::EffectBlockingPrerequisite,
                    )
            {
                continue;
            }
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
            let relation_kind = self
                .proposal_store
                .terminal_relation_projection_proof(&proposal.id)?
                .map(|proof| proof.relation_kind());
            if matches!(
                relation_kind,
                Some(openlife_core::agent::ProposalTerminalRelationKind::NonBlockingSuccessor)
                    | Some(
                        openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite
                    )
            ) {
                self.apply_claimed_review_without_task_transition(acceptance)
                    .await?;
            } else {
                self.apply_claimed_review_acceptance(acceptance).await?;
            }
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
