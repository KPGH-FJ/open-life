use std::sync::Arc;
#[cfg(test)]
use tauri::State;

use crate::main_chat_replay_contract::DurableMainChatReplayExecutionEnvelope;
use crate::main_chat_runtime_support::{
    append_main_chat_agent_transcript, finalize_main_chat_task_failure_after_durable_receipt,
    MainChatTaskFailureKind,
};
use crate::product_agent_dto::{
    product_transcript_summary, project_execution_transcript, ProductExecutionTranscriptEntry,
};
use crate::AppState;

#[cfg(test)]
#[derive(Clone)]
struct MainChatReplayCommitBarrier {
    task_session_id: String,
    reached: Arc<tokio::sync::Barrier>,
    // A one-way permit deliberately replaces a two-party Barrier here. The
    // TurnRuntime is allowed to drop the replay future as soon as cancellation
    // wins its select; a two-party release barrier would then strand the test
    // driver even though production cancellation completed correctly.
    release: Arc<tokio::sync::Semaphore>,
}

#[cfg(test)]
fn main_chat_replay_commit_barrier_slot(
) -> &'static std::sync::Mutex<Option<MainChatReplayCommitBarrier>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<MainChatReplayCommitBarrier>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

#[cfg(test)]
pub(crate) struct MainChatReplayCommitBarrierGuard {
    task_session_id: String,
}

#[cfg(test)]
impl Drop for MainChatReplayCommitBarrierGuard {
    fn drop(&mut self) {
        let mut slot = main_chat_replay_commit_barrier_slot()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot
            .as_ref()
            .is_some_and(|barrier| barrier.task_session_id == self.task_session_id)
        {
            *slot = None;
        }
    }
}

#[cfg(test)]
pub(crate) fn install_main_chat_replay_commit_barrier_for_test(
    task_session_id: &str,
) -> (
    MainChatReplayCommitBarrierGuard,
    Arc<tokio::sync::Barrier>,
    Arc<tokio::sync::Semaphore>,
) {
    let reached = Arc::new(tokio::sync::Barrier::new(2));
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let barrier = MainChatReplayCommitBarrier {
        task_session_id: task_session_id.to_string(),
        reached: Arc::clone(&reached),
        release: Arc::clone(&release),
    };
    *main_chat_replay_commit_barrier_slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(barrier);
    (
        MainChatReplayCommitBarrierGuard {
            task_session_id: task_session_id.to_string(),
        },
        reached,
        release,
    )
}

#[cfg(test)]
pub(crate) async fn pause_main_chat_replay_before_commit_for_test(task_session_id: &str) {
    let barrier = main_chat_replay_commit_barrier_slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .filter(|barrier| barrier.task_session_id == task_session_id)
        .cloned();
    if let Some(barrier) = barrier {
        barrier.reached.wait().await;
        let permit = barrier
            .release
            .acquire()
            .await
            .expect("replay commit test barrier must remain open");
        permit.forget();
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentTaskState {
    pub session: Option<ProductTaskSession>,
    pub actions: Vec<ProductQueuedExecutionAction>,
    pub transcript: Vec<ProductExecutionTranscriptEntry>,
    pub pending_approval_count: usize,
    pub active_tool_count: usize,
    pub can_resume: bool,
    pub can_cancel: bool,
    pub can_retry: bool,
    pub cancellation_pending: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductTaskSession {
    pub id: String,
    pub chat_session_id: String,
    pub selected_strategy: openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy,
    pub status: openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus,
    pub action_queue_ids: Vec<String>,
    pub pending_blockers: Vec<String>,
    pub context_snapshot_count: usize,
    pub has_plan_summary: bool,
    pub has_final_summary: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl ProductTaskSession {
    fn from_internal(session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession) -> Self {
        let mut pending_blockers = session
            .pending_blockers
            .iter()
            .filter_map(|blocker| product_task_blocker(blocker))
            .collect::<Vec<_>>();
        if pending_blockers.len() < session.pending_blockers.len() {
            pending_blockers.push("unknown_blocker".into());
        }
        pending_blockers.sort();
        pending_blockers.dedup();
        Self {
            id: product_task_ref_or_unknown(session.id.clone()),
            chat_session_id: product_task_ref_or_unknown(session.chat_session_id.clone()),
            selected_strategy: session.selected_strategy,
            status: session.status,
            action_queue_ids: session
                .action_queue_ids
                .iter()
                .cloned()
                .map(product_task_ref_or_unknown)
                .collect(),
            pending_blockers,
            context_snapshot_count: session.context_snapshot_refs.len(),
            has_plan_summary: session.current_plan_summary.is_some(),
            has_final_summary: session.final_summary.is_some(),
            created_at: session.created_at,
            updated_at: session.updated_at,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductExecutionPolicyDecision {
    pub level: openlife_core::agent::main_chat_agent_v1::MainChatPolicyLevel,
    pub reason_code: String,
    pub execution_allowed: bool,
    pub requires_confirmation: bool,
    pub requires_proposal: bool,
    pub requires_blocker: bool,
    pub silent_write_allowed: bool,
}

impl From<&openlife_core::agent::main_chat_agent_v1::ExecutionPolicyDecision>
    for ProductExecutionPolicyDecision
{
    fn from(policy: &openlife_core::agent::main_chat_agent_v1::ExecutionPolicyDecision) -> Self {
        Self {
            level: policy.level,
            reason_code: product_task_policy_reason(policy),
            execution_allowed: policy.execution_allowed,
            requires_confirmation: policy.requires_confirmation,
            requires_proposal: policy.requires_proposal,
            requires_blocker: policy.requires_blocker,
            silent_write_allowed: policy.silent_write_allowed,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductQueuedExecutionAction {
    pub id: String,
    pub session_id: String,
    pub action_type: String,
    pub policy: ProductExecutionPolicyDecision,
    pub status: openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus,
    pub attempts: u32,
    pub revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<&'static str>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl ProductQueuedExecutionAction {
    fn from_internal(
        action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
    ) -> Self {
        let failure_code = action
            .policy
            .requires_blocker
            .then_some("action_blocked")
            .or_else(|| action.error.as_ref().map(|_| "action_failed"));
        Self {
            id: product_task_ref_or_unknown(action.id.clone()),
            session_id: product_task_ref_or_unknown(action.session_id.clone()),
            action_type: product_task_action_type(&action.action.action_type),
            policy: (&action.policy).into(),
            status: action.status,
            attempts: action.attempts,
            revision: action.revision,
            failure_code,
            created_at: action.created_at,
            updated_at: action.updated_at,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductTaskProposal {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_ref: Option<String>,
    pub proposal_type: openlife_core::agent::ProposalType,
    pub source: openlife_core::agent::ProposalSource,
    pub risk_level: openlife_core::agent::RiskLevel,
    pub status: openlife_core::agent::ProposalStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ProductTaskProposal {
    fn from_internal(proposal: &openlife_core::agent::AgentProposal) -> Self {
        Self {
            id: product_task_ref_or_unknown(proposal.id.clone()),
            run_ref: proposal.run_id.clone().map(product_task_ref_or_unknown),
            proposal_type: proposal.proposal_type,
            source: proposal.source,
            risk_level: proposal.risk_level,
            status: proposal.status,
            created_at: proposal.created_at,
            resolved_at: proposal.resolved_at,
            expires_at: proposal.expires_at,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg(test)]
pub struct MainChatAgentTaskFilter {
    #[serde(default)]
    pub statuses: Vec<openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default = "default_true")]
    pub include_terminal: bool,
    #[serde(default = "default_true")]
    pub include_stale: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg(test)]
pub struct TaskSummary {
    pub task_session_id: String,
    pub conversation_id: String,
    pub run_id: String,
    pub title: String,
    pub strategy: openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy,
    pub status: openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus,
    pub last_updated_at: chrono::DateTime<chrono::Utc>,
    pub last_observation_preview: String,
    pub pending_blocker_count: usize,
    pub pending_proposal_count: usize,
    pub next_recommended_control: String,
    pub stale_state: String,
    pub resume_safety_digest: String,
    pub lifecycle_state: String,
    #[serde(default)]
    pub last_safe_event: Option<String>,
    pub action_count: usize,
    pub observation_count: usize,
    #[serde(default)]
    pub allowed_controls: Vec<String>,
    pub redaction_state: String,
    #[serde(default)]
    pub route_evidence: Option<ProductRouteEvidence>,
    pub evidence_view: ProductRunEvidenceView,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuityDiagnostics {
    pub stale_context: bool,
    pub missing_action_evidence: bool,
    pub permission_scope_mismatch: bool,
    pub terminal_no_resume: bool,
    pub provider_unavailable: bool,
    pub tool_unavailable: bool,
    pub requires_user_decision: bool,
    #[serde(default)]
    pub selected_skill_context_digest_mismatch: bool,
    #[serde(default)]
    pub plan_revision_mismatch: bool,
    #[serde(default)]
    pub reason_codes: Vec<String>,
    #[serde(default)]
    pub automatic_replay_allowed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    pub task_session: ProductTaskSession,
    pub actions: Vec<ProductQueuedExecutionAction>,
    pub transcript: Vec<ProductExecutionTranscriptEntry>,
    pub proposals: Vec<ProductTaskProposal>,
    pub blockers: Vec<String>,
    #[serde(default)]
    pub final_delivery: Option<serde_json::Value>,
    pub continuity_diagnostics: ContinuityDiagnostics,
    pub allowed_controls: Vec<String>,
    pub next_recommended_control: String,
    #[serde(default)]
    pub last_safe_resume_point: Option<String>,
    #[serde(default)]
    pub retry_target_action_id: Option<String>,
    pub context_digest: String,
    #[serde(default)]
    pub selected_skill_digest: Option<String>,
    pub tool_manifest_digest: String,
    pub evidence_view: ProductRunEvidenceView,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEvidenceTimelineEvent {
    pub id: String,
    pub kind: String,
    pub summary: String,
    #[serde(default)]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub failure_kind: Option<String>,
    #[serde(default)]
    pub normalized_lifecycle_state: Option<String>,
    #[serde(default)]
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DurableTurnLifecycleReceiptView {
    pub event_id: String,
    pub run_id: String,
    pub sequence: u64,
    pub event_type: String,
    pub source_ref: String,
    pub lifecycle_state: String,
    #[serde(default)]
    pub failure_kind: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub payload_digest: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductRouteIdentity {
    pub provider: String,
    pub model_ref: String,
    pub route_type: String,
    pub privacy_level: String,
    pub reason_ref: String,
    pub provider_health_is_estimated: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductProviderReadiness {
    pub configured: bool,
    pub credential_present: bool,
    pub validated: bool,
    pub validation_status: String,
    pub preferred: String,
    pub actually_used: Option<String>,
    pub stale: bool,
    pub failed: bool,
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductFallbackEvidence {
    pub from_route: Option<ProductRouteIdentity>,
    pub to_route: Option<ProductRouteIdentity>,
    pub reason_ref: String,
    pub blocker_codes: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductRouteSourceRef {
    pub source: String,
    pub ref_id: Option<String>,
    pub status: Option<String>,
    pub route_type: Option<String>,
}

/// Product-only route evidence. The core runtime fact owns richer diagnostic
/// values, including arbitrary JSON source records; this DTO deliberately
/// emits only typed codes, bounded references, booleans, and timestamps.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductRouteEvidence {
    pub evidence_id: String,
    pub generated_at: String,
    pub conversation_id: Option<String>,
    pub run_id: Option<String>,
    pub task_session_id: Option<String>,
    pub answer_scope: String,
    pub planned_route: Option<ProductRouteIdentity>,
    pub actual_route: Option<ProductRouteIdentity>,
    pub last_completed_route: Option<ProductRouteIdentity>,
    pub provider_readiness: ProductProviderReadiness,
    pub fallback: Option<ProductFallbackEvidence>,
    pub external_transmission: String,
    pub source_refs: Vec<ProductRouteSourceRef>,
    pub truth_confidence: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductRunEvidenceView {
    #[serde(default)]
    pub run_id: Option<String>,
    pub task_session_id: String,
    pub title: String,
    pub lifecycle_state: String,
    pub projection_state: String,
    pub identity_state: String,
    pub snapshot_state: String,
    #[serde(default)]
    pub durable_sequence_before: Option<u64>,
    #[serde(default)]
    pub durable_sequence_after: Option<u64>,
    #[serde(default)]
    pub durable_lifecycle_receipt: Option<DurableTurnLifecycleReceiptView>,
    #[serde(default)]
    pub route_evidence: Option<ProductRouteEvidence>,
    pub event_timeline: Vec<RunEvidenceTimelineEvent>,
    pub action_count: usize,
    pub observation_count: usize,
    pub blockers: Vec<String>,
    pub proposals: Vec<String>,
    pub plan_refs: Vec<String>,
    pub allowed_controls: Vec<String>,
    pub next_recommended_control: String,
    pub redaction_state: String,
}

#[cfg(test)]
fn default_true() -> bool {
    true
}

#[tauri::command]
#[cfg(test)]
pub(crate) async fn get_main_chat_agent_task_state(
    task_session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatAgentTaskState, String> {
    load_main_chat_agent_task_state(&task_session_id, &state).await
}

#[tauri::command]
#[cfg(test)]
pub(crate) async fn list_main_chat_agent_tasks(
    filter: Option<MainChatAgentTaskFilter>,
    limit: Option<usize>,
    offset: Option<usize>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<TaskSummary>, String> {
    list_main_chat_agent_tasks_with_state(filter, limit, offset, &state).await
}

#[tauri::command]
#[cfg(test)]
pub(crate) async fn get_main_chat_agent_task_detail(
    task_session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<TaskDetail, String> {
    get_main_chat_agent_task_detail_with_state(&task_session_id, &state).await
}

#[tauri::command]
#[cfg(test)]
pub(crate) async fn refresh_main_chat_agent_task_context(
    task_session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<TaskDetail, String> {
    append_main_chat_agent_transcript(
        &state,
        Some(&task_session_id),
        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Observation,
        "Task continuity context refresh recomputed resume diagnostics without replaying actions.",
        serde_json::json!({
            "taskContinuityRefresh": true,
            "automaticReplayStarted": false,
            "directWritesExecuted": false,
        }),
    )
    .await;
    get_main_chat_agent_task_detail_with_state(&task_session_id, &state).await
}

#[cfg(test)]
pub(crate) async fn list_main_chat_agent_tasks_with_state(
    filter: Option<MainChatAgentTaskFilter>,
    limit: Option<usize>,
    offset: Option<usize>,
    state: &Arc<AppState>,
) -> Result<Vec<TaskSummary>, String> {
    let filter = filter.unwrap_or(MainChatAgentTaskFilter {
        statuses: Vec::new(),
        conversation_id: None,
        include_terminal: true,
        include_stale: true,
    });
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0);
    let sessions = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "Main Chat task session store not available".to_string())?;
        let store = store_arc.lock().await;
        store
            .list_sessions(None, 200, 0)
            .map_err(|err| format!("list Main Chat tasks failed: {err}"))?
    };

    let mut summaries = Vec::new();
    for session in sessions {
        if !filter.statuses.is_empty() && !filter.statuses.contains(&session.status) {
            continue;
        }
        if let Some(conversation_id) = filter.conversation_id.as_deref() {
            if session.chat_session_id != conversation_id {
                continue;
            }
        }
        let detail = build_main_chat_agent_task_detail(state, session).await?;
        if !filter.include_terminal
            && run_evidence_lifecycle_is_terminal(&detail.evidence_view.lifecycle_state)
        {
            continue;
        }
        if !filter.include_stale && detail.continuity_diagnostics.stale_context {
            continue;
        }
        summaries.push(task_summary_from_detail(&detail));
    }

    Ok(summaries.into_iter().skip(offset).take(limit).collect())
}

pub(crate) async fn get_main_chat_agent_task_detail_with_state(
    task_session_id: &str,
    state: &Arc<AppState>,
) -> Result<TaskDetail, String> {
    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "Main Chat task session store not available".to_string())?;
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .map_err(|err| format!("load Main Chat task failed: {err}"))?
            .ok_or_else(|| format!("Main Chat task session not found: {task_session_id}"))?
    };
    build_main_chat_agent_task_detail(state, session).await
}

async fn build_main_chat_agent_task_detail(
    state: &Arc<AppState>,
    session: openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
) -> Result<TaskDetail, String> {
    // Cancellation events are canonical. Heal any cross-store projection that
    // previously degraded before assembling a product read model.
    reconcile_main_chat_cancellation_projections(state, Some(&session.id)).await?;
    let durable_lifecycle_before = load_durable_turn_lifecycle(state, &session.id).await;
    let session = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "Main Chat task session store not available".to_string())?;
        let store = store_arc.lock().await;
        store
            .load_session(&session.id)
            .map_err(|err| format!("reload Main Chat task projection failed: {err}"))?
            .ok_or_else(|| format!("Main Chat task session disappeared: {}", session.id))?
    };
    let transcript = {
        let store_arc = state
            .main_chat_agent_session_store
            .as_ref()
            .ok_or_else(|| "Main Chat task session store not available".to_string())?;
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(&session.id)
            .map_err(|err| format!("load Main Chat transcript failed: {err}"))?
    };
    let actions = if let Some(ref queue_arc) = state.main_chat_action_queue_store {
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(&session.id)
            .map_err(|err| format!("load Main Chat actions failed: {err}"))?
    } else {
        Vec::new()
    };
    let proposals = load_main_chat_task_linked_proposals(state, &actions, &transcript).await?;
    let blockers = task_blockers_from_evidence(&session, &actions);
    let agent_run_load = load_main_chat_task_agent_run(state, &session).await;
    let agent_run = agent_run_load.agent_run();
    let context_digest = main_chat_context_digest(&session, &transcript);
    let tool_manifest_digest = main_chat_tool_manifest_digest(state).await;
    let selected_skill_digest = main_chat_selected_skill_digest(state, &transcript).await?;
    let continuity_diagnostics = continuity_diagnostics_for_task(
        state,
        &session,
        &actions,
        &transcript,
        &context_digest,
        selected_skill_digest.as_deref(),
    )
    .await?;
    let projected_retry_target_action_id =
        retry_target_action_id_for_task(state, &session, &actions, &continuity_diagnostics).await;
    let provider_consent_ready_for_resume =
        main_chat_provider_consent_ready_for_resume(state, &session)
            .await?
            .is_some();
    let mut tool_permission_ready_for_resume = false;
    for action in actions.iter().filter(|action| {
        action.status
            == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
    }) {
        let action_permission =
            main_chat_pending_action_permission_ready_for_resume(state, &session, action).await?;
        if action_permission.is_none() {
            continue;
        }
        let has_network_consent = main_chat_action_network_consent_proposal_id(action).is_some();
        if !has_network_consent
            || main_chat_tool_network_consent_ready_for_resume(state, &session, action)
                .await?
                .is_some()
        {
            tool_permission_ready_for_resume = true;
            break;
        }
    }
    let projected_allowed_controls = allowed_controls_for_task(
        &session,
        &continuity_diagnostics,
        projected_retry_target_action_id.as_deref(),
        provider_consent_ready_for_resume,
        tool_permission_ready_for_resume,
    );
    let projected_next_recommended_control = next_recommended_control_for_task(
        &session,
        &continuity_diagnostics,
        projected_retry_target_action_id.as_deref(),
        &projected_allowed_controls,
    );
    let last_safe_resume_point =
        last_safe_resume_point_for_task(&session, &actions, &continuity_diagnostics)
            .map(product_task_ref_or_unknown);
    let final_delivery = final_delivery_from_task(&session, &transcript, &proposals, &blockers);
    let projected_lifecycle = normalized_run_lifecycle_state(&session, &transcript, agent_run);
    let durable_lifecycle_after = load_durable_turn_lifecycle(state, &session.id).await;
    let reconciliation = reconcile_run_evidence(
        &session,
        &projected_lifecycle,
        &agent_run_load,
        durable_lifecycle_before,
        durable_lifecycle_after,
    );
    let allowed_controls = match reconciliation.control_policy {
        RunEvidenceControlPolicy::Projected => projected_allowed_controls,
        RunEvidenceControlPolicy::Active => vec!["cancel".into(), "open_trace".into()],
        RunEvidenceControlPolicy::TraceOnly => vec!["open_trace".into()],
    };
    // A projected retry target is not an authorization surface by itself. If
    // durable lifecycle reconciliation fails closed, do not leak a target that
    // the command layer could otherwise treat as replayable.
    let retry_target_action_id = allowed_controls
        .iter()
        .any(|control| control == "retry")
        .then_some(projected_retry_target_action_id)
        .flatten()
        .map(product_task_ref_or_unknown);
    let next_recommended_control = if reconciliation.requires_reconciliation {
        "wait_for_projection_reconciliation".to_string()
    } else if reconciliation.control_policy == RunEvidenceControlPolicy::TraceOnly {
        "open_trace".to_string()
    } else {
        projected_next_recommended_control
    };
    let evidence_view = build_run_evidence_view(
        &session,
        &actions,
        &transcript,
        &proposals,
        &blockers,
        final_delivery.as_ref(),
        &allowed_controls,
        &next_recommended_control,
        agent_run,
        &reconciliation,
    );
    let product_task_session = ProductTaskSession::from_internal(&session);
    let product_actions = actions
        .iter()
        .map(ProductQueuedExecutionAction::from_internal)
        .collect();
    let product_proposals = proposals
        .iter()
        .map(ProductTaskProposal::from_internal)
        .collect();
    let product_blockers = blockers
        .iter()
        .map(|blocker| product_task_blocker(blocker).unwrap_or_else(|| "unknown_blocker".into()))
        .collect();

    Ok(TaskDetail {
        task_session: product_task_session,
        actions: product_actions,
        transcript: project_execution_transcript(transcript),
        proposals: product_proposals,
        blockers: product_blockers,
        final_delivery,
        continuity_diagnostics,
        allowed_controls,
        next_recommended_control,
        last_safe_resume_point,
        retry_target_action_id,
        context_digest,
        selected_skill_digest,
        tool_manifest_digest,
        evidence_view,
    })
}

async fn load_main_chat_task_linked_proposals(
    state: &Arc<AppState>,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
) -> Result<Vec<openlife_core::agent::AgentProposal>, String> {
    let mut proposal_ids = Vec::new();
    for action in actions {
        proposal_ids.extend(main_chat_action_proposal_ids(action));
    }
    for entry in transcript {
        collect_main_chat_proposal_ids(&entry.metadata, &mut proposal_ids);
    }
    proposal_ids.sort();
    proposal_ids.dedup();
    let Some(ref proposal_store_arc) = state.proposal_store else {
        return Ok(Vec::new());
    };
    let proposal_store = proposal_store_arc.lock().await;
    let mut proposals = Vec::new();
    for proposal_id in proposal_ids {
        if let Some(proposal) = proposal_store
            .get_proposal(&proposal_id)
            .map_err(|err| format!("load linked proposal failed: {err}"))?
        {
            proposals.push(proposal);
        }
    }
    Ok(proposals)
}

enum CanonicalAgentRunLoad {
    Available(Box<openlife_core::agent::AgentRun>),
    Missing,
    Degraded,
}

impl CanonicalAgentRunLoad {
    fn agent_run(&self) -> Option<&openlife_core::agent::AgentRun> {
        match self {
            Self::Available(run) => Some(run.as_ref()),
            Self::Missing | Self::Degraded => None,
        }
    }
}

async fn load_main_chat_task_agent_run(
    state: &Arc<AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
) -> CanonicalAgentRunLoad {
    let Some(ref run_store_arc) = state.agent_run_store else {
        return CanonicalAgentRunLoad::Degraded;
    };
    let run_store = run_store_arc.lock().await;
    match crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        run_store
            .get_run_for_task_id(&session.id)
            .map_err(|error| error.to_string()),
    ) {
        Ok(Some(run)) if run.task_id == session.id => {
            CanonicalAgentRunLoad::Available(Box::new(run))
        }
        Ok(Some(_)) => CanonicalAgentRunLoad::Degraded,
        Ok(None) => CanonicalAgentRunLoad::Missing,
        Err(_) => CanonicalAgentRunLoad::Degraded,
    }
}

enum DurableTurnLifecycleLoad {
    Available {
        sequence: u64,
        bound_run_id: Option<String>,
        receipt: Option<Box<DurableTurnLifecycleReceiptView>>,
    },
    Unavailable,
    Degraded,
}

async fn load_durable_turn_lifecycle(
    state: &Arc<AppState>,
    task_session_id: &str,
) -> DurableTurnLifecycleLoad {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return DurableTurnLifecycleLoad::Unavailable;
    };
    let store = store_arc.lock().await;
    let snapshot = match store.turn_lifecycle_snapshot(task_session_id) {
        Ok(snapshot) => snapshot,
        Err(_) => return DurableTurnLifecycleLoad::Degraded,
    };
    let receipt = snapshot.lifecycle_event.map(|event| {
        let terminal = durable_terminal_product_projection(&event);
        Box::new(DurableTurnLifecycleReceiptView {
            event_id: product_task_ref_or_unknown(event.event_id),
            run_id: product_task_ref_or_unknown(event.run_id),
            sequence: event.sequence,
            event_type: product_durable_event_type(&event.event_type),
            source_ref: product_durable_event_source(&event.source),
            lifecycle_state: terminal.lifecycle_state,
            failure_kind: terminal.failure_kind,
            created_at: event.created_at,
            payload_digest: product_task_digest(&event.payload_digest),
        })
    });
    DurableTurnLifecycleLoad::Available {
        sequence: snapshot.latest_sequence,
        bound_run_id: snapshot.bound_run_id,
        receipt,
    }
}

struct DurableTerminalProductProjection {
    lifecycle_state: String,
    failure_kind: Option<String>,
}

fn durable_terminal_product_projection(
    event: &crate::main_chat_event_stream::MainChatAgentDurableEvent,
) -> DurableTerminalProductProjection {
    let kind = event
        .payload
        .get("kind")
        .and_then(serde_json::Value::as_str);
    let projection =
        |lifecycle_state: &str, failure_kind: Option<&str>| DurableTerminalProductProjection {
            lifecycle_state: lifecycle_state.into(),
            failure_kind: failure_kind.map(str::to_string),
        };
    match event.event_type.as_str() {
        "cancel_requested" => projection("cancelling", None),
        "local_aborted" => match kind {
            None | Some("cancelled") => projection("cancelled", Some("cancelled")),
            _ => projection("unknown", Some("unknown_error")),
        },
        "interrupted" | "turn.interrupted" => match kind {
            None | Some("interrupted") => projection("interrupted", Some("interrupted")),
            _ => projection("unknown", Some("unknown_error")),
        },
        "failed" => match kind {
            Some("timeout") => projection("timed_out", Some("timeout")),
            Some("provider_error") => projection("failed", Some("provider_error")),
            Some("tool_error") => projection("failed", Some("tool_error")),
            Some("policy_blocker") => projection("blocked", Some("policy_blocker")),
            Some("unknown_error") | None => projection("failed", Some("unknown_error")),
            _ => projection("unknown", Some("unknown_error")),
        },
        "final_delivery.created" => match event
            .payload
            .get("status")
            .and_then(serde_json::Value::as_str)
        {
            Some("completed" | "delivered") => projection("completed", None),
            Some("completed_with_pending_items") => {
                projection("completed_with_pending_items", None)
            }
            Some("blocked") => projection("blocked", Some("policy_blocker")),
            Some("failed") => projection("failed", Some("unknown_error")),
            Some("cancelled") => projection("cancelled", Some("cancelled")),
            _ => projection("unknown", None),
        },
        _ => projection("unknown", None),
    }
}

#[cfg(test)]
pub(crate) fn durable_terminal_projection_for_test(
    event_type: &str,
    payload: serde_json::Value,
) -> (String, Option<String>) {
    let event = crate::main_chat_event_stream::MainChatAgentDurableEvent {
        event_id: "mainchat_event:v2:sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        task_session_id: uuid::Uuid::nil().to_string(),
        run_id: uuid::Uuid::nil().to_string(),
        sequence: 1,
        event_type: event_type.into(),
        object_type: "turn".into(),
        object_id: "terminal-projection-test".into(),
        created_at: chrono::Utc::now(),
        source: "test".into(),
        payload_digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        payload,
        backfilled: false,
    };
    let projection = durable_terminal_product_projection(&event);
    (projection.lifecycle_state, projection.failure_kind)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunEvidenceControlPolicy {
    Projected,
    Active,
    TraceOnly,
}

struct RunEvidenceReconciliation {
    lifecycle_state: String,
    projection_state: String,
    identity_state: String,
    snapshot_state: String,
    durable_sequence_before: Option<u64>,
    durable_sequence_after: Option<u64>,
    durable_lifecycle_receipt: Option<DurableTurnLifecycleReceiptView>,
    control_policy: RunEvidenceControlPolicy,
    requires_reconciliation: bool,
}

fn reconcile_run_evidence(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    projected_lifecycle: &str,
    agent_run_load: &CanonicalAgentRunLoad,
    durable_before: DurableTurnLifecycleLoad,
    durable_after: DurableTurnLifecycleLoad,
) -> RunEvidenceReconciliation {
    let (durable_sequence_before, before_available) = match durable_before {
        DurableTurnLifecycleLoad::Available { sequence, .. } => (Some(sequence), true),
        DurableTurnLifecycleLoad::Unavailable => (None, false),
        DurableTurnLifecycleLoad::Degraded => (None, false),
    };
    let (durable_sequence_after, durable_bound_run_id, durable_lifecycle_receipt, after_available) =
        match durable_after {
            DurableTurnLifecycleLoad::Available {
                sequence,
                bound_run_id,
                receipt,
            } => (
                Some(sequence),
                bound_run_id,
                receipt.map(|receipt| *receipt),
                true,
            ),
            DurableTurnLifecycleLoad::Unavailable => (None, None, None, false),
            DurableTurnLifecycleLoad::Degraded => (None, None, None, false),
        };
    let snapshot_state = match (durable_sequence_before, durable_sequence_after) {
        (Some(before), Some(after)) if before == after => "stable",
        (Some(_), Some(_)) => "changed",
        _ if !before_available || !after_available => "degraded",
        _ => "degraded",
    }
    .to_string();

    let identity_state = match agent_run_load {
        CanonicalAgentRunLoad::Available(run) if run.task_id != session.id => "conflict",
        CanonicalAgentRunLoad::Available(run) => {
            let durable_run_id = durable_lifecycle_receipt
                .as_ref()
                .map(|receipt| receipt.run_id.as_str())
                .or(durable_bound_run_id.as_deref());
            match durable_run_id {
                Some(durable_run_id) if durable_run_id != run.id => "conflict",
                _ => "consistent",
            }
        }
        CanonicalAgentRunLoad::Missing => "missing",
        CanonicalAgentRunLoad::Degraded => "degraded",
    }
    .to_string();

    if identity_state != "consistent" {
        return RunEvidenceReconciliation {
            lifecycle_state: "unknown".into(),
            projection_state: "degraded".into(),
            identity_state,
            snapshot_state,
            durable_sequence_before,
            durable_sequence_after,
            durable_lifecycle_receipt,
            control_policy: RunEvidenceControlPolicy::TraceOnly,
            requires_reconciliation: true,
        };
    }

    if snapshot_state != "stable" {
        return RunEvidenceReconciliation {
            lifecycle_state: "unknown".into(),
            projection_state: if snapshot_state == "changed" {
                "pending".into()
            } else {
                "degraded".into()
            },
            identity_state,
            snapshot_state,
            durable_sequence_before,
            durable_sequence_after,
            durable_lifecycle_receipt,
            control_policy: RunEvidenceControlPolicy::TraceOnly,
            requires_reconciliation: true,
        };
    }

    if let Some(receipt) = durable_lifecycle_receipt.as_ref() {
        let agrees = lifecycle_projection_agrees(&receipt.lifecycle_state, projected_lifecycle);
        let uncertain_terminal = matches!(
            receipt.lifecycle_state.as_str(),
            "unknown" | "interrupted" | "partial_or_unknown"
        );
        return RunEvidenceReconciliation {
            lifecycle_state: receipt.lifecycle_state.clone(),
            projection_state: if agrees { "consistent" } else { "pending" }.into(),
            identity_state,
            snapshot_state,
            durable_sequence_before,
            durable_sequence_after,
            durable_lifecycle_receipt,
            control_policy: if agrees && !uncertain_terminal {
                RunEvidenceControlPolicy::Projected
            } else {
                RunEvidenceControlPolicy::TraceOnly
            },
            requires_reconciliation: !agrees || uncertain_terminal,
        };
    }

    let agent_run = agent_run_load.agent_run();
    if projected_terminal_requires_durable_receipt(session, agent_run, projected_lifecycle) {
        return RunEvidenceReconciliation {
            lifecycle_state: "unknown".into(),
            projection_state: "pending".into(),
            identity_state,
            snapshot_state,
            durable_sequence_before,
            durable_sequence_after,
            durable_lifecycle_receipt: None,
            control_policy: RunEvidenceControlPolicy::TraceOnly,
            requires_reconciliation: true,
        };
    }

    if projected_lifecycle == "running" {
        return RunEvidenceReconciliation {
            lifecycle_state: "running".into(),
            projection_state: "active".into(),
            identity_state,
            snapshot_state,
            durable_sequence_before,
            durable_sequence_after,
            durable_lifecycle_receipt: None,
            control_policy: RunEvidenceControlPolicy::Active,
            requires_reconciliation: false,
        };
    }

    RunEvidenceReconciliation {
        lifecycle_state: projected_lifecycle.to_string(),
        projection_state: "projected".into(),
        identity_state,
        snapshot_state,
        durable_sequence_before,
        durable_sequence_after,
        durable_lifecycle_receipt: None,
        control_policy: RunEvidenceControlPolicy::Projected,
        requires_reconciliation: false,
    }
}

fn projected_terminal_requires_durable_receipt(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    agent_run: Option<&openlife_core::agent::AgentRun>,
    projected_lifecycle: &str,
) -> bool {
    matches!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
            | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
            | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled
    ) || agent_run.is_some_and(|run| {
        matches!(
            run.status,
            openlife_core::agent::AgentRunStatus::Completed
                | openlife_core::agent::AgentRunStatus::Failed
                | openlife_core::agent::AgentRunStatus::Cancelled
        )
    }) || matches!(
        projected_lifecycle,
        "completed"
            | "completed_with_pending_items"
            | "failed"
            | "timed_out"
            | "cancelled"
            | "interrupted"
            | "partial_or_unknown"
    )
}

fn lifecycle_projection_agrees(canonical: &str, projected: &str) -> bool {
    canonical == projected
        || matches!(
            (canonical, projected),
            ("completed_with_pending_items", "completed")
                | ("completed_with_pending_items", "blocked")
                | ("blocked", "blocked")
                | ("cancelled", "cancelled")
                | ("timed_out", "failed")
                | ("interrupted", "failed")
                | ("failed", "failed")
        )
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
fn build_run_evidence_view(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    proposals: &[openlife_core::agent::AgentProposal],
    blockers: &[String],
    final_delivery: Option<&serde_json::Value>,
    allowed_controls: &[String],
    next_recommended_control: &str,
    agent_run: Option<&openlife_core::agent::AgentRun>,
    reconciliation: &RunEvidenceReconciliation,
) -> ProductRunEvidenceView {
    let run_id = agent_run.map(|run| product_task_ref_or_unknown(run.id.clone()));
    let route_evidence = route_evidence_from_transcript(transcript)
        .or_else(|| agent_run.and_then(route_evidence_from_agent_run));
    let mut event_timeline = run_evidence_timeline(transcript, blockers, agent_run);
    let action_count = actions
        .len()
        .max(agent_run.map(|run| run.actions.len()).unwrap_or(0));
    let transcript_observation_count = transcript
        .iter()
        .filter(|entry| {
            matches!(
                entry.kind,
                openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Observation
                    | openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Error
                    | openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::FinalResult
                    | openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::PermissionRequest
                    | openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::ProposalRequest
            )
        })
        .count();
    let observation_count = transcript_observation_count.max(
        agent_run
            .map(|run| {
                run.observations.len()
                    + usize::from(run.output_preview.is_some())
                    + usize::from(run.error.is_some())
            })
            .unwrap_or(0),
    );
    let proposal_ids = run_evidence_proposal_ids(proposals, transcript, agent_run);
    let plan_refs = run_evidence_plan_refs(session, transcript, final_delivery);
    if let Some(receipt) = reconciliation.durable_lifecycle_receipt.as_ref() {
        event_timeline.push(RunEvidenceTimelineEvent {
            id: receipt.event_id.clone(),
            kind: receipt.event_type.clone(),
            summary: "durable_lifecycle_state_recorded".into(),
            created_at: Some(receipt.created_at),
            failure_kind: receipt.failure_kind.clone(),
            normalized_lifecycle_state: Some(receipt.lifecycle_state.clone()),
            source_ref: Some(if receipt.source_ref == "unknown" {
                format!("turn-event:sequence:{}", receipt.sequence)
            } else {
                receipt.source_ref.clone()
            }),
        });
    }

    ProductRunEvidenceView {
        run_id,
        task_session_id: product_task_ref_or_unknown(session.id.clone()),
        title: "main_chat_task".into(),
        lifecycle_state: reconciliation.lifecycle_state.clone(),
        projection_state: reconciliation.projection_state.clone(),
        identity_state: reconciliation.identity_state.clone(),
        snapshot_state: reconciliation.snapshot_state.clone(),
        durable_sequence_before: reconciliation.durable_sequence_before,
        durable_sequence_after: reconciliation.durable_sequence_after,
        durable_lifecycle_receipt: reconciliation.durable_lifecycle_receipt.clone(),
        route_evidence,
        event_timeline,
        action_count,
        observation_count,
        blockers: blockers.to_vec(),
        proposals: proposal_ids,
        plan_refs,
        allowed_controls: allowed_controls.to_vec(),
        next_recommended_control: next_recommended_control.to_string(),
        redaction_state: "metadata_only".into(),
    }
}

fn normalized_run_lifecycle_state(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    agent_run: Option<&openlife_core::agent::AgentRun>,
) -> String {
    let latest_failure_kind = transcript
        .iter()
        .rev()
        .find_map(|entry| string_from_metadata(&entry.metadata, &["failureKind", "failure_kind"]));
    let latest_lifecycle = transcript.iter().rev().find_map(|entry| {
        string_from_metadata(
            &entry.metadata,
            &["normalizedLifecycleState", "normalized_lifecycle_state"],
        )
    });
    let stored_failed = session.status
        == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
        || agent_run
            .map(|run| run.status == openlife_core::agent::AgentRunStatus::Failed)
            .unwrap_or(false);
    if stored_failed && latest_failure_kind.as_deref() == Some("timeout") {
        return "timed_out".into();
    }
    if let Some(lifecycle) = latest_lifecycle {
        if lifecycle == "timed_out" {
            return "failed".into();
        }
        return lifecycle;
    }

    match session.status {
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Running => "running",
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked => "blocked",
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed => "completed",
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed => "failed",
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled => "cancelled",
    }
    .into()
}

fn route_evidence_from_transcript(
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
) -> Option<ProductRouteEvidence> {
    transcript
        .iter()
        .rev()
        .find_map(|entry| {
            entry
                .metadata
                .get("routeEvidence")
                .or_else(|| entry.metadata.get("runtimeRouteEvidence"))
                .cloned()
        })
        .and_then(|value| {
            serde_json::from_value::<crate::main_chat_runtime_facts::RuntimeRouteEvidence>(value)
                .ok()
        })
        .map(|evidence| product_route_evidence(&evidence))
}

fn route_evidence_from_agent_run(
    run: &openlife_core::agent::AgentRun,
) -> Option<ProductRouteEvidence> {
    run.reasoning_trace
        .as_ref()
        .and_then(|trace| trace.generation_result.as_ref())
        .and_then(|value| value.get("runtimeRouteEvidence").cloned())
        .and_then(|value| {
            serde_json::from_value::<crate::main_chat_runtime_facts::RuntimeRouteEvidence>(value)
                .ok()
        })
        .map(|evidence| product_route_evidence(&evidence))
}

#[cfg(test)]
pub(crate) fn serialized_route_evidence_from_agent_run_for_test(
    run: &openlife_core::agent::AgentRun,
) -> Option<serde_json::Value> {
    route_evidence_from_agent_run(run).and_then(|evidence| serde_json::to_value(evidence).ok())
}

fn product_route_evidence(
    evidence: &crate::main_chat_runtime_facts::RuntimeRouteEvidence,
) -> ProductRouteEvidence {
    ProductRouteEvidence {
        evidence_id: product_route_evidence_id(&evidence.evidence_id),
        generated_at: product_task_timestamp(&evidence.generated_at),
        conversation_id: evidence
            .conversation_id
            .as_ref()
            .map(|value| product_task_ref_or_unknown(value.clone())),
        run_id: evidence
            .run_id
            .as_ref()
            .map(|value| product_task_ref_or_unknown(value.clone())),
        task_session_id: evidence
            .task_session_id
            .as_ref()
            .map(|value| product_task_ref_or_unknown(value.clone())),
        answer_scope: product_route_enum(
            &evidence.answer_scope,
            &[
                "current_turn",
                "last_completed_turn",
                "settings_readiness",
                "planned_next_turn",
                "unknown",
            ],
        ),
        planned_route: evidence.planned_route.as_ref().map(product_route_identity),
        actual_route: evidence.actual_route.as_ref().map(product_route_identity),
        last_completed_route: evidence
            .last_completed_route
            .as_ref()
            .map(product_route_identity),
        provider_readiness: ProductProviderReadiness {
            configured: evidence.provider_readiness.configured,
            credential_present: evidence.provider_readiness.credential_present,
            validated: evidence.provider_readiness.validated,
            validation_status: product_provider_validation_status(
                &evidence.provider_readiness.validation_status,
            ),
            preferred: product_route_provider(&evidence.provider_readiness.preferred),
            actually_used: evidence
                .provider_readiness
                .actually_used
                .as_ref()
                .map(|value| product_route_provider(value)),
            stale: evidence.provider_readiness.stale,
            failed: evidence.provider_readiness.failed,
            last_checked_at: evidence
                .provider_readiness
                .last_checked_at
                .as_ref()
                .map(|value| product_task_timestamp(value)),
        },
        fallback: evidence
            .fallback
            .as_ref()
            .map(|fallback| ProductFallbackEvidence {
                from_route: fallback.from_route.as_ref().map(product_route_identity),
                to_route: fallback.to_route.as_ref().map(product_route_identity),
                reason_ref: product_route_reason_ref(&fallback.reason),
                blocker_codes: fallback
                    .blocker_codes
                    .iter()
                    .map(|code| product_task_blocker(code).unwrap_or_else(|| "unknown".into()))
                    .collect(),
            }),
        external_transmission: product_route_enum(
            &evidence.external_transmission,
            &["not_sent", "sent", "unknown", "not_instrumented", "blocked"],
        ),
        source_refs: evidence
            .source_refs
            .iter()
            .filter_map(product_route_source_ref)
            .collect(),
        truth_confidence: product_route_enum(
            &evidence.truth_confidence,
            &["verified", "inferred", "unknown"],
        ),
    }
}

fn product_route_identity(
    route: &crate::main_chat_runtime_facts::RouteIdentity,
) -> ProductRouteIdentity {
    ProductRouteIdentity {
        provider: product_route_provider(&route.provider),
        model_ref: product_route_model_ref(&route.model),
        route_type: product_route_type(&route.route_type),
        privacy_level: product_route_privacy_level(&route.privacy_level),
        reason_ref: product_route_reason_ref(&route.reason),
        provider_health_is_estimated: route.provider_health_is_estimated,
    }
}

fn product_route_source_ref(value: &serde_json::Value) -> Option<ProductRouteSourceRef> {
    let object = value.as_object()?;
    let source = object
        .get("source")
        .and_then(serde_json::Value::as_str)
        .map(product_route_source)
        .unwrap_or_else(|| "unknown".into());
    let ref_id = ["refId", "ref_id", "runId", "run_id"]
        .iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(|value| product_task_ref_or_unknown(value.to_string()));
    let status = object
        .get("status")
        .and_then(serde_json::Value::as_str)
        .map(product_route_source_status);
    let route_type = object
        .get("routeType")
        .or_else(|| object.get("route_type"))
        .and_then(serde_json::Value::as_str)
        .map(product_route_type);
    Some(ProductRouteSourceRef {
        source,
        ref_id,
        status,
        route_type,
    })
}

fn product_route_source(value: &str) -> String {
    match value {
        "provider_validation"
        | "provider_preflight"
        | "config"
        | "current_turn_route"
        | "agent_run"
        | "provider_event"
        | "provider_adapter"
        | "unknown" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_route_enum(value: &str, allowed: &[&str]) -> String {
    if allowed.contains(&value) {
        value.to_string()
    } else {
        "unknown".into()
    }
}

fn product_provider_validation_status(value: &str) -> String {
    match value {
        "validated"
        | "stale"
        | "failed"
        | "consent_required"
        | "blocked"
        | "runtime_generation_incoherent"
        | "not_configured"
        | "not_checked"
        | "not_attempted"
        | "unknown" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_route_provider(value: &str) -> String {
    match value {
        "ollama" | "openai" | "openrouter" | "deepseek" | "runtime_fact" | "direct"
        | "scripted" | "local" | "unknown" => value.into(),
        value if !value.trim().is_empty() => {
            let (_, digest) = openlife_core::agent::metadata_safe::metadata_safe_text_digest(value);
            format!("provider:{digest}")
        }
        _ => "unknown".into(),
    }
}

fn product_route_model_ref(value: &str) -> String {
    if value.trim().is_empty() || product_task_text_is_internal_authority(value) {
        return "unknown".into();
    }
    let (_, digest) = openlife_core::agent::metadata_safe::metadata_safe_text_digest(value);
    format!("model:{digest}")
}

fn product_route_type(value: &str) -> String {
    match value {
        "local" | "cloud" | "agent_runtime" | "scripted" | "unknown" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_route_privacy_level(value: &str) -> String {
    match value {
        "none" | "light" | "summary" | "filtered" | "strict" | "local_only" | "internal"
        | "unknown" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_route_reason_ref(value: &str) -> String {
    if value.trim().is_empty() || product_task_text_is_internal_authority(value) {
        return "unknown".into();
    }
    let (_, digest) = openlife_core::agent::metadata_safe::metadata_safe_text_digest(value);
    format!("reason:{digest}")
}

fn product_route_source_status(value: &str) -> String {
    match value {
        "ready" | "available" | "unavailable" | "validated" | "failed" | "blocked"
        | "completed" | "not_attempted" | "unknown" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_route_evidence_id(value: &str) -> String {
    if value.trim().is_empty() {
        return "unknown".into();
    }
    let digest = openlife_core::agent::metadata_safe::metadata_safe_value_digest(
        &serde_json::json!({ "routeEvidenceId": value }),
    )
    .1;
    format!("route_evidence:{digest}")
}

fn run_evidence_timeline(
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    blockers: &[String],
    agent_run: Option<&openlife_core::agent::AgentRun>,
) -> Vec<RunEvidenceTimelineEvent> {
    let mut events = if transcript.is_empty() {
        agent_run_fallback_timeline(agent_run)
    } else {
        transcript
            .iter()
            .map(|entry| RunEvidenceTimelineEvent {
                id: product_task_ref_or_unknown(entry.id.clone()),
                kind: entry.kind.as_str().into(),
                summary: product_transcript_summary(entry.kind).into(),
                created_at: Some(entry.created_at),
                failure_kind: string_from_metadata(
                    &entry.metadata,
                    &["failureKind", "failure_kind"],
                )
                .and_then(|value| product_task_blocker(&value)),
                normalized_lifecycle_state: string_from_metadata(
                    &entry.metadata,
                    &["normalizedLifecycleState", "normalized_lifecycle_state"],
                )
                .and_then(|value| product_task_lifecycle_state(&value)),
                source_ref: string_from_metadata(&entry.metadata, &["sourceRef", "source_ref"])
                    .and_then(product_task_ref),
            })
            .collect::<Vec<_>>()
    };

    if let Some((run, error)) =
        agent_run.and_then(|run| run.error.as_ref().map(|error| (run, error)))
    {
        let failure_kind = product_task_blocker(&error.phase).unwrap_or_else(|| "unknown".into());
        if !events
            .iter()
            .any(|event| event.failure_kind.as_deref() == Some(failure_kind.as_str()))
        {
            events.push(RunEvidenceTimelineEvent {
                id: "run-error".into(),
                kind: "error".into(),
                summary: "error_state_recorded".into(),
                created_at: run.finished_at,
                failure_kind: Some(failure_kind),
                normalized_lifecycle_state: Some(if error.phase == "timeout" {
                    "timed_out".into()
                } else {
                    "failed".into()
                }),
                source_ref: Some("agent_run_error".into()),
            });
        }
    }

    for (index, blocker) in blockers.iter().enumerate() {
        let already_visible = events
            .iter()
            .any(|event| event.source_ref.as_deref() == Some(blocker.as_str()));
        if already_visible {
            continue;
        }
        events.push(RunEvidenceTimelineEvent {
            id: format!("blocker:{index}"),
            kind: "blocker".into(),
            summary: "blocker_state_recorded".into(),
            created_at: None,
            failure_kind: Some("policy_blocker".into()),
            normalized_lifecycle_state: Some("blocked".into()),
            source_ref: Some("task_session_blocker".into()),
        });
    }
    events
}

fn agent_run_fallback_timeline(
    agent_run: Option<&openlife_core::agent::AgentRun>,
) -> Vec<RunEvidenceTimelineEvent> {
    let Some(run) = agent_run else {
        return Vec::new();
    };
    let mut events = Vec::new();
    for (index, update) in run.status_updates.iter().enumerate() {
        events.push(RunEvidenceTimelineEvent {
            id: format!("run-status:{index}"),
            kind: product_agent_run_phase(&update.phase),
            summary: "run_status_update_recorded".into(),
            created_at: Some(update.timestamp),
            failure_kind: None,
            normalized_lifecycle_state: None,
            source_ref: Some("agent_run_status_update".into()),
        });
    }
    for action in &run.actions {
        events.push(RunEvidenceTimelineEvent {
            id: format!(
                "run-action:{}",
                product_task_ref_or_unknown(action.id.clone())
            ),
            kind: "action".into(),
            summary: "action_state_recorded".into(),
            created_at: Some(action.started_at.unwrap_or(action.timestamp)),
            failure_kind: None,
            normalized_lifecycle_state: None,
            source_ref: Some("agent_run_action".into()),
        });
    }
    for observation in &run.observations {
        events.push(RunEvidenceTimelineEvent {
            id: format!(
                "run-observation:{}",
                product_task_ref_or_unknown(observation.id.clone())
            ),
            kind: "observation".into(),
            summary: "observation_state_recorded".into(),
            created_at: Some(observation.timestamp),
            failure_kind: None,
            normalized_lifecycle_state: None,
            source_ref: Some("agent_run_observation".into()),
        });
    }
    if let Some(error) = run.error.as_ref() {
        events.push(RunEvidenceTimelineEvent {
            id: "run-error".into(),
            kind: "error".into(),
            summary: "error_state_recorded".into(),
            created_at: run.finished_at,
            failure_kind: product_task_blocker(&error.phase).or_else(|| Some("unknown".into())),
            normalized_lifecycle_state: Some(if error.phase == "timeout" {
                "timed_out".into()
            } else {
                "failed".into()
            }),
            source_ref: Some("agent_run_error".into()),
        });
    } else if run.output_preview.is_some() {
        events.push(RunEvidenceTimelineEvent {
            id: "run-final".into(),
            kind: "final_result".into(),
            summary: "final_result_state_recorded".into(),
            created_at: run.finished_at,
            failure_kind: None,
            normalized_lifecycle_state: Some("completed".into()),
            source_ref: Some("agent_run_output_preview".into()),
        });
    }
    events.sort_by_key(|event| event.created_at);
    events
}

fn run_evidence_proposal_ids(
    proposals: &[openlife_core::agent::AgentProposal],
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    agent_run: Option<&openlife_core::agent::AgentRun>,
) -> Vec<String> {
    let mut ids = proposals
        .iter()
        .map(|proposal| proposal.id.clone())
        .collect::<Vec<_>>();
    for entry in transcript {
        collect_main_chat_proposal_ids(&entry.metadata, &mut ids);
    }
    if let Some(run) = agent_run {
        ids.extend(run.generated_proposals.clone());
    }
    let mut ids = ids
        .into_iter()
        .map(product_task_ref_or_unknown)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn run_evidence_plan_refs(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    final_delivery: Option<&serde_json::Value>,
) -> Vec<String> {
    let mut refs = session.context_snapshot_refs.clone();
    if session.current_plan_summary.is_some() {
        refs.push(format!("task_plan_summary:{}", session.id));
    }
    for entry in transcript {
        for key in [
            "canonicalTaskId",
            "planId",
            "contextSnapshotRef",
            "lastSafeResumePoint",
        ] {
            if let Some(value) = string_from_metadata(&entry.metadata, &[key]) {
                refs.push(value);
            }
        }
    }
    if let Some(final_delivery) = final_delivery {
        if let Some(value) = string_from_metadata(final_delivery, &["canonicalTaskId", "planId"]) {
            refs.push(value);
        }
    }
    let mut refs = refs
        .into_iter()
        .map(product_task_ref_or_unknown)
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

async fn continuity_diagnostics_for_task(
    state: &Arc<AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    context_digest: &str,
    selected_skill_digest: Option<&str>,
) -> Result<ContinuityDiagnostics, String> {
    let action_ids = actions
        .iter()
        .map(|action| action.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let missing_action_evidence = session
        .action_queue_ids
        .iter()
        .any(|id| !action_ids.contains(id.as_str()));
    let stale_context = stale_context_detected(session, transcript, context_digest);
    let permission_scope_mismatch =
        permission_scope_mismatch_detected(state, session, actions).await?;
    let tool_unavailable = tool_unavailable_detected(state, actions).await?;
    let provider_unavailable = provider_unavailable_detected(state).await
        && !has_provider_independent_replay_action(session, actions);
    let selected_skill_context_digest_mismatch =
        selected_skill_context_digest_mismatch_detected(transcript, selected_skill_digest);
    let plan_revision_mismatch = plan_revision_mismatch_detected(state, transcript).await?;
    let terminal_no_resume = main_chat_task_status_is_terminal(session.status)
        || (session.status
            == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
            && !has_retryable_failed_action(session, actions));
    let requires_user_decision = matches!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
            | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
    ) || !session.pending_blockers.is_empty()
        || actions.iter().any(|action| {
            action.status
                == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
        });
    let mut diagnostics = ContinuityDiagnostics {
        stale_context,
        missing_action_evidence,
        permission_scope_mismatch,
        terminal_no_resume,
        provider_unavailable,
        tool_unavailable,
        requires_user_decision,
        selected_skill_context_digest_mismatch,
        plan_revision_mismatch,
        reason_codes: Vec::new(),
        automatic_replay_allowed: false,
    };
    for (enabled, code) in [
        (diagnostics.stale_context, "stale_context"),
        (
            diagnostics.missing_action_evidence,
            "missing_action_evidence",
        ),
        (
            diagnostics.permission_scope_mismatch,
            "permission_scope_mismatch",
        ),
        (diagnostics.terminal_no_resume, "terminal_no_resume"),
        (diagnostics.provider_unavailable, "provider_unavailable"),
        (diagnostics.tool_unavailable, "tool_unavailable"),
        (
            diagnostics.selected_skill_context_digest_mismatch,
            "selected_skill_context_digest_mismatch",
        ),
        (diagnostics.plan_revision_mismatch, "plan_revision_mismatch"),
        (diagnostics.requires_user_decision, "requires_user_decision"),
    ] {
        if enabled {
            diagnostics.reason_codes.push(code.into());
        }
    }
    diagnostics.automatic_replay_allowed = continuity_hard_resume_blocker(&diagnostics).is_none()
        && actions.iter().any(|action| {
            openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_action_retry(
                Some(session),
                Some(action),
            )
            .allowed
                && openlife_core::agent::main_chat_agent_v1::typed_tool_receipt_allows_automatic_retry(
                    action,
                )
        });
    Ok(diagnostics)
}

#[cfg(test)]
fn task_summary_from_detail(detail: &TaskDetail) -> TaskSummary {
    let resume_safety_digest = digest_label(&serde_json::json!({
        "taskSessionId": detail.task_session.id,
        "status": detail.task_session.status,
        "contextDigest": detail.context_digest,
        "selectedSkillDigest": detail.selected_skill_digest,
        "toolManifestDigest": detail.tool_manifest_digest,
        "diagnostics": detail.continuity_diagnostics.reason_codes,
        "allowedControls": detail.allowed_controls,
    }));
    let evidence_view = detail.evidence_view.clone();
    TaskSummary {
        task_session_id: product_task_ref_or_unknown(detail.task_session.id.clone()),
        conversation_id: product_task_ref_or_unknown(detail.task_session.chat_session_id.clone()),
        run_id: evidence_view
            .run_id
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        title: evidence_view.title.clone(),
        strategy: detail.task_session.selected_strategy,
        status: detail.task_session.status,
        last_updated_at: detail.task_session.updated_at,
        last_observation_preview: last_observation_preview(&detail.evidence_view),
        pending_blocker_count: detail.blockers.len(),
        pending_proposal_count: detail
            .proposals
            .iter()
            .filter(|proposal| proposal.status == openlife_core::agent::ProposalStatus::Pending)
            .count(),
        next_recommended_control: detail.next_recommended_control.clone(),
        stale_state: stale_state_for_detail(detail),
        resume_safety_digest,
        lifecycle_state: evidence_view.lifecycle_state.clone(),
        last_safe_event: evidence_view
            .event_timeline
            .last()
            .map(|event| event.summary.clone()),
        action_count: evidence_view.action_count,
        observation_count: evidence_view.observation_count,
        allowed_controls: evidence_view.allowed_controls.clone(),
        redaction_state: evidence_view.redaction_state.clone(),
        route_evidence: evidence_view.route_evidence.clone(),
        evidence_view,
    }
}

#[cfg(test)]
fn stale_state_for_detail(detail: &TaskDetail) -> String {
    if detail.continuity_diagnostics.terminal_no_resume {
        "terminal".into()
    } else if detail.continuity_diagnostics.stale_context {
        "stale".into()
    } else {
        "fresh".into()
    }
}

fn allowed_controls_for_task(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    diagnostics: &ContinuityDiagnostics,
    retry_target_action_id: Option<&str>,
    provider_consent_ready_for_resume: bool,
    tool_permission_ready_for_resume: bool,
) -> Vec<String> {
    let mut controls = vec!["open_trace".to_string(), "refresh_context".to_string()];
    if diagnostics.terminal_no_resume {
        return vec!["open_trace".into()];
    }
    if session.status == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed {
        if continuity_hard_resume_blocker(diagnostics).is_none() && retry_target_action_id.is_some()
        {
            controls.push("retry".into());
        }
        controls.sort();
        controls.dedup();
        return controls;
    }
    controls.push("cancel".into());
    if continuity_hard_resume_blocker(diagnostics).is_none() {
        if retry_target_action_id.is_some() {
            controls.push("retry".into());
        }
        if matches!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
                | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
                | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
        ) && tool_permission_ready_for_resume
            || provider_consent_ready_for_resume
        {
            controls.push("resume".into());
        }
    }
    controls.sort();
    controls.dedup();
    controls
}

fn next_recommended_control_for_task(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    diagnostics: &ContinuityDiagnostics,
    retry_target_action_id: Option<&str>,
    allowed_controls: &[String],
) -> String {
    if diagnostics.terminal_no_resume {
        return "open_trace".into();
    }
    if diagnostics.stale_context
        || diagnostics.permission_scope_mismatch
        || diagnostics.selected_skill_context_digest_mismatch
        || diagnostics.plan_revision_mismatch
    {
        return "refresh_context".into();
    }
    if retry_target_action_id.is_some() && allowed_controls.iter().any(|control| control == "retry")
    {
        return "retry".into();
    }
    if matches!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    ) {
        return if allowed_controls.iter().any(|control| control == "resume") {
            "resume".into()
        } else {
            "review_permission".into()
        };
    }
    if matches!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
            | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
    ) && allowed_controls.iter().any(|control| control == "resume")
    {
        return "resume".into();
    }
    if allowed_controls
        .iter()
        .any(|control| control == "refresh_context")
    {
        return "refresh_context".into();
    }
    "open_trace".into()
}

async fn retry_target_action_id_for_task(
    state: &Arc<AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    diagnostics: &ContinuityDiagnostics,
) -> Option<String> {
    if continuity_hard_resume_blocker(diagnostics).is_some() {
        return None;
    }
    for action in actions.iter().rev() {
        if openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_action_retry(
            Some(session),
            Some(action),
        )
        .allowed
            && openlife_core::agent::main_chat_agent_v1::typed_tool_receipt_allows_automatic_retry(
                action,
            )
            && crate::main_chat_turn_runtime::validate_openlife_replay_readiness(
                state, session, action,
            )
            .await
            .is_ok()
        {
            return Some(action.id.clone());
        }
    }
    None
}

fn has_retryable_failed_action(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
) -> bool {
    actions.iter().any(|action| {
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_action_retry(
            Some(session),
            Some(action),
        )
        .allowed
            && openlife_core::agent::main_chat_agent_v1::typed_tool_receipt_allows_automatic_retry(
                action,
            )
            && crate::main_chat_turn_runtime::canonical_openlife_replay_envelope(session, action)
                .is_some()
    })
}

fn has_provider_independent_replay_action(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
) -> bool {
    actions.iter().any(|action| {
        matches!(
            action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
                | openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
        ) && openlife_core::agent::main_chat_agent_v1::action_replay_effect_is_safe_to_claim(action)
            && openlife_core::agent::main_chat_agent_v1::typed_tool_receipt_allows_automatic_retry(
                action,
            )
            && crate::main_chat_turn_runtime::canonical_openlife_replay_envelope(session, action)
                .is_some()
    })
}

fn last_safe_resume_point_for_task(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    diagnostics: &ContinuityDiagnostics,
) -> Option<String> {
    if continuity_hard_resume_blocker(diagnostics).is_some() {
        return None;
    }
    actions
        .iter()
        .rev()
        .find(|action| {
            action.status
                == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Planned
                || (matches!(
                    action.status,
                    openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
                        | openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
                ) && openlife_core::agent::main_chat_agent_v1::typed_tool_receipt_allows_automatic_retry(action)
                    && crate::main_chat_turn_runtime::canonical_openlife_replay_envelope(
                        session, action,
                    )
                    .is_some())
        })
        .map(|action| action.id.clone())
}

fn continuity_hard_resume_blocker(diagnostics: &ContinuityDiagnostics) -> Option<&'static str> {
    if diagnostics.terminal_no_resume {
        Some("terminal_no_resume")
    } else if diagnostics.stale_context {
        Some("stale_context")
    } else if diagnostics.missing_action_evidence {
        Some("missing_action_evidence")
    } else if diagnostics.permission_scope_mismatch {
        Some("permission_scope_mismatch")
    } else if diagnostics.provider_unavailable {
        Some("provider_unavailable")
    } else if diagnostics.tool_unavailable {
        Some("tool_unavailable")
    } else if diagnostics.selected_skill_context_digest_mismatch {
        Some("selected_skill_context_digest_mismatch")
    } else if diagnostics.plan_revision_mismatch {
        Some("plan_revision_mismatch")
    } else {
        None
    }
}

fn main_chat_task_status_is_terminal(
    status: openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus,
) -> bool {
    matches!(
        status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
            | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled
    )
}

#[cfg(test)]
fn run_evidence_lifecycle_is_terminal(lifecycle_state: &str) -> bool {
    matches!(
        lifecycle_state,
        "completed"
            | "completed_with_pending_items"
            | "failed"
            | "timed_out"
            | "cancelled"
            | "interrupted"
            | "partial_or_unknown"
    )
}

fn task_blockers_from_evidence(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
) -> Vec<String> {
    // Stored summaries and adapter errors are bodies, not reason-code
    // authority. The product view can expose only typed blocker codes or
    // action/proposal references; an untyped value remains visibly unknown.
    let mut blockers = session
        .pending_blockers
        .iter()
        .map(|blocker| product_task_blocker(blocker).unwrap_or_else(|| "unknown".into()))
        .collect::<Vec<_>>();
    for action in actions {
        match action.status {
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission => {
                blockers.push(
                    product_task_ref(action.id.clone())
                        .map(|action_id| format!("pending_permission:{action_id}"))
                        .unwrap_or_else(|| "unknown".into()),
                );
            }
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed => {
                blockers.push(
                    product_task_ref(action.id.clone())
                        .map(|action_id| format!("action_failed:{action_id}"))
                        .unwrap_or_else(|| "unknown".into()),
                );
            }
            _ => {}
        }
    }
    if blockers.is_empty()
        && matches!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
                | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
        )
    {
        blockers.push("unknown".into());
    }
    blockers.sort();
    blockers.dedup();
    blockers
}

fn final_delivery_from_task(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    proposals: &[openlife_core::agent::AgentProposal],
    blockers: &[String],
) -> Option<serde_json::Value> {
    let final_entry = transcript.iter().rev().find(|entry| {
        entry.kind
            == openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::FinalResult
    });
    final_entry.and_then(|entry| {
        let delivery_override = transcript.iter().rev().find(|candidate| {
            string_from_metadata(
                &candidate.metadata,
                &["finalDeliveryStatus", "final_delivery_status"],
            )
            .is_some()
        });
        let status =
            final_delivery_status_from_task(session, transcript, entry, proposals, blockers)?;
        Some(serde_json::json!({
            "transcriptEntryId": product_task_ref_or_unknown(entry.id.clone()),
            "summary": product_transcript_summary(entry.kind),
            "status": status,
            "deliveryStatusEvidenceId": delivery_override
                .map(|evidence| product_task_ref_or_unknown(evidence.id.clone())),
        }))
    })
}

#[cfg(test)]
pub(crate) fn final_delivery_from_task_for_test(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
) -> Option<serde_json::Value> {
    final_delivery_from_task(session, transcript, &[], &[])
}

fn final_delivery_status_from_task(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    final_entry: &openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry,
    proposals: &[openlife_core::agent::AgentProposal],
    blockers: &[String],
) -> Option<&'static str> {
    if let Some(status) = transcript.iter().rev().find_map(|entry| {
        string_from_metadata(
            &entry.metadata,
            &["finalDeliveryStatus", "final_delivery_status"],
        )
    }) {
        return match status.as_str() {
            "completed" | "delivered" => Some("completed"),
            "completed_with_pending_items" => Some("completed_with_pending_items"),
            "blocked" => Some("blocked"),
            "failed" => Some("failed"),
            "cancelled" => Some("cancelled"),
            _ => None,
        };
    }
    if let Some(status) = string_from_metadata(&final_entry.metadata, &["status", "deliveryStatus"])
    {
        return match status.as_str() {
            "completed" | "delivered" => Some("completed"),
            "completed_with_pending_items" => Some("completed_with_pending_items"),
            "blocked" => Some("blocked"),
            "failed" => Some("failed"),
            "cancelled" => Some("cancelled"),
            _ => None,
        };
    }

    match session.status {
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
            if !proposals.is_empty() || !blockers.is_empty() =>
        {
            Some("completed_with_pending_items")
        }
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed => {
            Some("completed")
        }
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
        | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission => {
            Some("blocked")
        }
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed => Some("failed"),
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled => {
            Some("cancelled")
        }
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Running => None,
    }
}

#[cfg(test)]
fn last_observation_preview(evidence_view: &ProductRunEvidenceView) -> String {
    evidence_view
        .event_timeline
        .iter()
        .rev()
        .find(|entry| {
            matches!(
                entry.kind.as_str(),
                "observation" | "error" | "final_result" | "permission_request"
            )
        })
        .map(|entry| entry.summary.clone())
        .unwrap_or_else(|| "Observation state unknown.".into())
}

fn stale_context_detected(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    context_digest: &str,
) -> bool {
    for entry in transcript {
        if let Some(stored) = string_from_metadata(
            &entry.metadata,
            &["continuityContextDigest", "contextDigest"],
        ) {
            if stored != context_digest {
                return true;
            }
        }
        if let Some(context_ref) = string_from_metadata(&entry.metadata, &["contextSnapshotRef"]) {
            if !session.context_snapshot_refs.is_empty()
                && !session
                    .context_snapshot_refs
                    .iter()
                    .any(|current| current == &context_ref)
            {
                return true;
            }
        }
    }
    false
}

async fn permission_scope_mismatch_detected(
    state: &Arc<AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
) -> Result<bool, String> {
    use openlife_core::agent::main_chat_agent_v1::{ExecutionQueueStatus, MainChatAgentStrategy};
    if session.selected_strategy != MainChatAgentStrategy::ReActToolExecution {
        return Ok(false);
    }
    let canonical_run_id = if let Some(run_store_arc) = state.agent_run_store.as_ref() {
        let run_store = run_store_arc.lock().await;
        crate::terminal_owner_write_gateway::register_agent_run_store_result(
            state,
            run_store
                .get_run_for_task_id(&session.id)
                .map_err(|error| error.to_string()),
        )
        .map_err(|error| {
            format!("load canonical AgentRun for permission diagnostics failed: {error}")
        })?
        .map(|run| run.id)
    } else {
        None
    };
    for action in actions
        .iter()
        .filter(|action| action.status == ExecutionQueueStatus::PendingPermission)
    {
        let Some(envelope) =
            crate::main_chat_turn_runtime::canonical_openlife_replay_envelope(session, action)
        else {
            if main_chat_action_has_accepted_tool_permission_proposal(state, action).await? {
                return Ok(true);
            }
            continue;
        };
        if canonical_run_id.as_deref() != Some(envelope.run_id.as_str()) {
            return Ok(true);
        }
        match main_chat_pending_action_accepted_tool_permission_scope(
            state, session, action, &envelope,
        )
        .await?
        {
            AcceptedToolPermissionScopeLookup::NotAccepted => continue,
            AcceptedToolPermissionScopeLookup::AcceptedInvalid => return Ok(true),
            AcceptedToolPermissionScopeLookup::AcceptedValid(_) => {}
        };
        if main_chat_pending_action_permission_ready_for_resume(state, session, action)
            .await?
            .is_none()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn tool_unavailable_detected(
    state: &Arc<AppState>,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
) -> Result<bool, String> {
    let registry = state.mcp_registry.lock().await;
    let manifests = registry.list_manifests();
    for action in actions
        .iter()
        .filter(|action| action.action.action_type.contains("mcp"))
    {
        let target = action.observation_metadata.as_ref().and_then(|metadata| {
            string_from_metadata(
                metadata,
                &[
                    "manifestId",
                    "manifestName",
                    "resolvedTarget",
                    "tool_name",
                    "toolName",
                    "target",
                ],
            )
        });
        if let Some(target) = target {
            let available = manifests
                .iter()
                .any(|manifest| manifest.name == target || manifest.id == target);
            if !available {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

async fn provider_unavailable_detected(state: &Arc<AppState>) -> bool {
    let scheduler = state.scheduler.lock().await;
    let no_remote = scheduler.effective_api_key().trim().is_empty()
        || scheduler.provider.trim().eq_ignore_ascii_case("none");
    let no_local = scheduler.local_model.trim().is_empty();
    no_remote && no_local
}

async fn main_chat_selected_skill_digest(
    state: &Arc<AppState>,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
) -> Result<Option<String>, String> {
    let Some(skill_id) = transcript.iter().rev().find_map(|entry| {
        string_from_metadata(&entry.metadata, &["selectedSkillId", "selected_skill_id"])
    }) else {
        return Ok(None);
    };
    let registry = state.skill_registry.lock().await;
    let Some(manifest) = registry.get(&skill_id) else {
        return Ok(Some("missing".into()));
    };
    Ok(Some(digest_label(
        &serde_json::to_value(manifest)
            .map_err(|err| format!("serialize skill manifest for digest failed: {err}"))?,
    )))
}

fn selected_skill_context_digest_mismatch_detected(
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    current_digest: Option<&str>,
) -> bool {
    let stored = transcript.iter().rev().find_map(|entry| {
        string_from_metadata(
            &entry.metadata,
            &[
                "selectedSkillDigest",
                "selected_skill_digest",
                "selectedSkillInstructionDigest",
            ],
        )
    });
    match (stored, current_digest) {
        (Some(stored), Some(current)) => stored != current,
        (Some(_), None) => true,
        _ => false,
    }
}

async fn plan_revision_mismatch_detected(
    state: &Arc<AppState>,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
) -> Result<bool, String> {
    let Some((canonical_task_id, revision)) = transcript.iter().rev().find_map(|entry| {
        let task_id = string_from_metadata(&entry.metadata, &["canonicalTaskId"])?;
        let revision = entry
            .metadata
            .get("revision")
            .and_then(serde_json::Value::as_u64)?;
        Some((task_id, revision))
    }) else {
        return Ok(false);
    };
    let Some(ref store_arc) = state.canonical_task_runtime_store else {
        return Ok(true);
    };
    let store = store_arc.lock().await;
    let Some(snapshot) = store
        .list_task_snapshots(500)
        .map_err(|err| format!("load canonical Plan item for continuity failed: {err}"))?
        .into_iter()
        .find(|snapshot| snapshot.task.id == canonical_task_id)
    else {
        return Ok(true);
    };
    Ok(snapshot
        .runs
        .last()
        .is_none_or(|run| run.plan_revision != revision))
}

fn main_chat_context_digest(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
) -> String {
    let transcript_context_refs = transcript
        .iter()
        .filter_map(|entry| string_from_metadata(&entry.metadata, &["contextSnapshotRef"]))
        .collect::<Vec<_>>();
    digest_label(&serde_json::json!({
        "contextSnapshotRefs": session.context_snapshot_refs,
        "transcriptContextRefs": transcript_context_refs,
    }))
}

async fn main_chat_tool_manifest_digest(state: &Arc<AppState>) -> String {
    let registry = state.mcp_registry.lock().await;
    let mut manifests = registry
        .list_manifests()
        .into_iter()
        .map(|manifest| {
            serde_json::json!({
                "id": manifest.id,
                "name": manifest.name,
                "source": manifest.source,
                "riskLevel": manifest.risk_level,
                "actionType": manifest.action_type,
                "capabilities": manifest.capabilities,
            })
        })
        .collect::<Vec<_>>();
    manifests.sort_by_key(|a| a.to_string());
    digest_label(&serde_json::Value::Array(manifests))
}

fn string_from_metadata(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let object = value.as_object()?;
    for key in keys {
        if let Some(value) = object.get(*key).and_then(serde_json::Value::as_str) {
            return Some(value.to_string());
        }
    }
    None
}

fn product_task_text_is_internal_authority(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    normalized.contains("hmac-sha256:")
        || normalized.contains("canonicalstoreidentity")
        || normalized.contains("bindingreceipt")
        || normalized.contains("bodyreceipt")
        || normalized.contains("authoritytag")
}

fn product_task_policy_reason(
    policy: &openlife_core::agent::main_chat_agent_v1::ExecutionPolicyDecision,
) -> String {
    use openlife_core::agent::main_chat_agent_v1::MainChatPolicyLevel;

    match (policy.level, policy.reason_code.as_str()) {
        (MainChatPolicyLevel::L0PureAnswer, "pure_answer_allowed")
        | (MainChatPolicyLevel::L1ReadOnlyAuto, "read_only_action_allowed")
        | (MainChatPolicyLevel::L1GovernedProposalCreate, "governed_proposal_create_allowed")
        | (
            MainChatPolicyLevel::L1GovernedProposalCreate,
            "governed_learning_candidate_capture_allowed",
        )
        | (MainChatPolicyLevel::L2ProposalFirst, "write_like_action_requires_proposal")
        | (MainChatPolicyLevel::L3ConfirmedLocalWrite, "confirmed_local_write_required")
        | (MainChatPolicyLevel::L4ExternalWrite, "external_write_requires_confirmation")
        | (MainChatPolicyLevel::L4ExternalWrite, "unselected_skill_not_injected")
        | (MainChatPolicyLevel::L5DangerousHardBlock, "dangerous_action_hard_block") => {
            policy.reason_code.clone()
        }
        _ => "unknown_policy_reason".into(),
    }
}

fn product_task_action_type(value: &str) -> String {
    match value {
        "direct.answer"
        | "memory.search"
        | "session.search"
        | "file.read"
        | "file.patch"
        | "web.search"
        | "web.fetch"
        | "mcp.read_only"
        | "proposal.create"
        | "memory.write"
        | "life_model.update"
        | "file.write.approved"
        | "calendar.real_write"
        | "email.send"
        | "shell.destructive"
        | "task.plan_item.create"
        | "memory.governance.plan"
        | "lifemodel.learning_candidate.capture" => value.into(),
        _ => "unknown_action_type".into(),
    }
}

fn product_durable_event_type(value: &str) -> String {
    match value {
        "turn_started"
        | "provider.started"
        | "provider.completed"
        | "provider.failed"
        | "provider.remote_unknown"
        | "tool.dispatch_prepared"
        | "tool.started"
        | "tool.completed"
        | "tool.not_dispatched"
        | "tool.dispatch_ambiguous"
        | "tool.remote_unknown"
        | "observation.created"
        | "proposal.created"
        | "proposal.accepted"
        | "plan.created"
        | "plan.reviewed"
        | "task.updated"
        | "cancel_requested"
        | "local_aborted"
        | "interrupted"
        | "failed"
        | "final"
        | "final_answer"
        | "diagnostic.created" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_durable_event_source(value: &str) -> String {
    match value {
        "provider_adapter"
        | "tool_gateway"
        | "action_executor"
        | "proposal_store"
        | "plan_runtime"
        | "task_control"
        | "diagnostic"
        | "turn_runtime"
        | "openlife_turn_runtime"
        | "openlife_turn_runtime.tool_dispatch_prepared"
        | "openlife_turn_runtime.kernel_error_pre_commit"
        | "bootstrap.prepared_tool_reconciliation"
        | "bootstrap.started_tool_reconciliation" => value.into(),
        _ => "unknown".into(),
    }
}

fn product_agent_run_phase(value: &openlife_core::agent::types::AgentLoopPhase) -> String {
    value.to_string()
}

fn product_task_lifecycle_state(value: &str) -> Option<String> {
    matches!(
        value,
        "not_started"
            | "running"
            | "waiting_permission"
            | "waiting_for_user"
            | "blocked"
            | "completed"
            | "failed"
            | "cancelled"
            | "interrupted"
            | "timed_out"
            | "remote_unknown"
            | "unknown"
    )
    .then(|| value.to_string())
}

fn product_task_ref(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 192
        || product_task_text_is_internal_authority(trimmed)
        || trimmed.chars().any(char::is_whitespace)
        || !trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._:/+-".contains(character))
    {
        return None;
    }
    if uuid::Uuid::parse_str(trimmed).is_ok()
        || product_task_stable_ref(trimmed)
        || trimmed
            .strip_prefix("plan-session-")
            .is_some_and(|value| uuid::Uuid::parse_str(value).is_ok())
        || trimmed
            .strip_prefix("plan-")
            .is_some_and(|value| uuid::Uuid::parse_str(value).is_ok())
        || trimmed
            .strip_prefix("plan:plan-session-")
            .is_some_and(|value| uuid::Uuid::parse_str(value).is_ok())
        || trimmed
            .strip_prefix("task_plan_summary:")
            .is_some_and(|value| uuid::Uuid::parse_str(value).is_ok())
        || trimmed
            .strip_prefix("turn-event:sequence:")
            .is_some_and(|value| {
                !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
            })
        || trimmed
            .strip_prefix("conversation://")
            .is_some_and(product_task_conversation_ref)
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn product_task_stable_ref(value: &str) -> bool {
    if let Some(digest) = value.strip_prefix("mainchat_event:v2:sha256:") {
        return product_task_is_lower_hex(digest, 64);
    }
    let Some((prefix, digest)) = value.rsplit_once('_') else {
        return false;
    };
    prefix.starts_with("mainchat_")
        && prefix.len() <= 64
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && product_task_is_lower_hex(digest, 8)
}

fn product_task_conversation_ref(value: &str) -> bool {
    let (session_id, suffix_valid) =
        if let Some((session_id, message_id)) = value.split_once("/message/") {
            (
                session_id,
                !message_id.is_empty() && message_id.bytes().all(|byte| byte.is_ascii_digit()),
            )
        } else if let Some(session_id) = value.strip_suffix("/current-user-message") {
            (session_id, true)
        } else {
            (value, false)
        };
    suffix_valid
        && !session_id.is_empty()
        && session_id.len() <= 96
        && session_id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || "._:-".contains(character)
        })
}

fn product_task_is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn product_task_ref_or_unknown(value: String) -> String {
    product_task_ref(value).unwrap_or_else(|| "unknown".into())
}

fn product_task_timestamp(value: &str) -> String {
    if !product_task_text_is_internal_authority(value)
        && value.len() <= 64
        && chrono::DateTime::parse_from_rfc3339(value).is_ok()
    {
        value.to_string()
    } else {
        "unknown".into()
    }
}

fn product_task_digest(value: &str) -> String {
    if let Some(digest) = value.strip_prefix("sha256:") {
        if product_task_is_lower_hex(digest, 64) {
            return value.to_string();
        }
    }
    if let Some(rest) = value.strip_prefix("bytes:") {
        if let Some((bytes, digest)) = rest.split_once(" hash:sha256:") {
            if !bytes.is_empty()
                && bytes.bytes().all(|byte| byte.is_ascii_digit())
                && product_task_is_lower_hex(digest, 64)
            {
                return value.to_string();
            }
        }
    }
    "unknown".into()
}

fn product_task_blocker(value: &str) -> Option<String> {
    if matches!(
        value,
        "tool_permission_required"
            | "proposal_review_required"
            | "two_actions_failed"
            | "dangerous_action_hard_block"
            | "external_write_requires_confirmation"
            | "policy_confirmation_required"
            | "provider_network_consent_required"
            | "provider_unavailable"
            | "provider_failed"
            | "provider_error"
            | "tool_unavailable"
            | "tool_failed"
            | "tool_error"
            | "runtime_interrupted"
            | "interrupted"
            | "timeout"
            | "cancelled"
            | "unknown_error"
            | "policy_blocker"
            | "web_network_policy_blocked"
            | "memory_lifecycle_reader_unavailable"
            | "stale_context"
            | "missing_action_evidence"
            | "permission_scope_mismatch"
            | "terminal_no_resume"
            | "selected_skill_context_digest_mismatch"
            | "plan_revision_mismatch"
            | "requires_user_decision"
            | "unknown"
    ) {
        return Some(value.to_string());
    }
    if value == "proposal:pending" {
        return Some(value.into());
    }
    if let Some(proposal_id) = value.strip_prefix("proposal:") {
        return product_task_ref(proposal_id.to_string())
            .map(|proposal_id| format!("proposal:{proposal_id}"));
    }
    for prefix in ["pending_permission:", "action_failed:"] {
        if let Some(action_id) = value.strip_prefix(prefix) {
            return product_task_ref(action_id.to_string())
                .map(|action_id| format!("{prefix}{action_id}"));
        }
    }
    if let Some(action) = value.strip_prefix("action:") {
        let (action_id, status) = action.rsplit_once(':')?;
        if !matches!(
            status,
            "planned"
                | "pending_permission"
                | "executing"
                | "observed"
                | "failed"
                | "retrying"
                | "cancelled"
                | "completed"
        ) {
            return None;
        }
        return product_task_ref(action_id.to_string())
            .map(|action_id| format!("action:{action_id}:{status}"));
    }
    None
}

fn digest_label(value: &serde_json::Value) -> String {
    let (bytes, hash) = openlife_core::agent::metadata_safe::metadata_safe_value_digest(value);
    format!("bytes:{bytes} hash:{hash}")
}

#[tauri::command]
#[cfg(test)]
pub(crate) async fn resume_main_chat_agent_task(
    task_session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatAgentTaskState, String> {
    resume_main_chat_agent_task_with_state(&task_session_id, &state).await
}

pub(crate) async fn resume_main_chat_agent_task_with_state(
    task_session_id: &str,
    state: &Arc<AppState>,
) -> Result<MainChatAgentTaskState, String> {
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "Main Chat task session store not available".to_string())?;
    let session = {
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .map_err(|err| format!("load Main Chat task before resume failed: {err}"))?
    };
    let actions = if let Some(ref queue_arc) = state.main_chat_action_queue_store {
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .map_err(|err| format!("load Main Chat actions before resume failed: {err}"))?
    } else {
        Vec::new()
    };
    if session.is_some() {
        let detail = get_main_chat_agent_task_detail_with_state(task_session_id, state).await?;
        if let Some(reason_code) = continuity_hard_resume_blocker(&detail.continuity_diagnostics) {
            append_main_chat_agent_transcript(
                state,
                Some(task_session_id),
                openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Error,
                "Task resume was blocked by continuity diagnostics before any replay.",
                serde_json::json!({
                    "resumeRequested": true,
                    "resumeBlockedByContinuityDiagnostics": true,
                    "resumeReasonCode": reason_code,
                    "continuityDiagnostics": detail.continuity_diagnostics.reason_codes,
                    "automaticReplayStarted": false,
                    "directWritesExecuted": false,
                }),
            )
            .await;
            if reason_code == "permission_scope_mismatch" {
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::PermissionRequest,
                    "Task resume was requested but pending permission blockers remain.",
                    serde_json::json!({
                        "resumeRequested": true,
                        "resumeBlockedByPendingPermission": true,
                        "resumeBlockedByContinuityDiagnostics": true,
                        "resumeReasonCode": reason_code,
                        "continuityDiagnostics": detail.continuity_diagnostics.reason_codes,
                        "automaticReplayStarted": false,
                        "directWritesExecuted": false,
                    }),
                )
                .await;
                return load_main_chat_agent_task_state(task_session_id, state).await;
            }
            return Err(format!("resume Main Chat task rejected: {reason_code}"));
        }
    }
    let resume_decision = openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_task_resume(
        session.as_ref(),
        &actions,
    );
    if !resume_decision.allowed {
        return Err(format!(
            "resume Main Chat task rejected: {}",
            resume_decision.reason_code
        ));
    }
    if resume_decision.remain_waiting_permission {
        if let Some(session_ref) = session.as_ref() {
            if let Some(action_ref) = actions.iter().find(|action| {
                action.status
                    == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
            }) {
                if let Some(action_bound_permission) =
                    main_chat_pending_action_permission_ready_for_resume(
                        state,
                        session_ref,
                        action_ref,
                    )
                    .await?
                {
                    if main_chat_action_network_consent_proposal_id(action_ref).is_some() {
                        if let Some(network_consent_proposal_id) =
                            main_chat_tool_network_consent_ready_for_resume(
                                state,
                                session_ref,
                                action_ref,
                            )
                            .await?
                        {
                            let runtime =
                                crate::main_chat_turn_runtime::OpenLifeTurnRuntime::new(state);
                            Box::pin(runtime.run_replay(
                                crate::main_chat_turn_runtime::OpenLifeReplayInput::resume_after_network_consent(
                                    task_session_id,
                                    &action_ref.id,
                                    action_bound_permission,
                                    network_consent_proposal_id,
                                ),
                            ))
                            .await?;
                            return load_main_chat_agent_task_state(task_session_id, state).await;
                        }
                    } else {
                        let runtime =
                            crate::main_chat_turn_runtime::OpenLifeTurnRuntime::new(state);
                        Box::pin(runtime.run_replay(
                            crate::main_chat_turn_runtime::OpenLifeReplayInput::resume_after_permission(
                                task_session_id,
                                &action_ref.id,
                                action_bound_permission,
                            ),
                        ))
                        .await?;
                        return load_main_chat_agent_task_state(task_session_id, state).await;
                    }
                }
            }
            if let Some(proposal_id) =
                main_chat_provider_consent_ready_for_resume(state, session_ref).await?
            {
                let runtime = crate::main_chat_turn_runtime::OpenLifeTurnRuntime::new(state);
                Box::pin(
                    runtime
                        .run_provider_network_consent_continuation(task_session_id, &proposal_id),
                )
                .await?;
                return load_main_chat_agent_task_state(task_session_id, state).await;
            }
        }
        let store = store_arc.lock().await;
        store
            .mark_waiting_permission(task_session_id)
            .map_err(|err| format!("preserve Main Chat permission blocker failed: {err}"))?;
        drop(store);
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::PermissionRequest,
            "Task resume was requested but pending permission blockers remain.",
            serde_json::json!({
                "resumeRequested": true,
                "resumeBlockedByPendingPermission": true,
                "resumeReasonCode": resume_decision.reason_code,
                "pendingPermissionCount": resume_decision.pending_permission_count,
                "pendingBlockerCount": resume_decision.pending_blocker_count,
                "directWritesExecuted": false,
            }),
        )
        .await;
        return load_main_chat_agent_task_state(task_session_id, state).await;
    }

    append_main_chat_agent_transcript(
        state,
        Some(task_session_id),
        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Error,
        "Task resume was rejected because no executable checkpoint or governed replay action exists.",
        serde_json::json!({
            "resumeRequested": true,
            "resumeReasonCode": "resume_checkpoint_execution_unavailable",
            "resumeBlockedByPendingPermission": false,
            "automaticReplayStarted": false,
            "directWritesExecuted": false,
        }),
    )
    .await;
    Err("resume Main Chat task rejected: resume_checkpoint_execution_unavailable".into())
}

fn main_chat_action_network_consent_proposal_id(
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
) -> Option<String> {
    action
        .observation_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("networkConsentProposalId"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

async fn main_chat_tool_network_consent_ready_for_resume(
    state: &Arc<AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
) -> Result<Option<String>, String> {
    use openlife_core::agent::{ProposalSource, ProposalStatus, ProposalType};

    let Some(proposal_id) = main_chat_action_network_consent_proposal_id(action) else {
        return Ok(None);
    };
    let Some(envelope) =
        crate::main_chat_turn_runtime::canonical_openlife_replay_envelope(session, action)
    else {
        return Ok(None);
    };
    let (proposal, origin, relation_kind, dispatch_state) = {
        let proposal_store = state
            .proposal_store
            .as_ref()
            .ok_or_else(|| "proposal_store_unavailable".to_string())?
            .lock()
            .await;
        let Some(proposal) = proposal_store
            .get_proposal(&proposal_id)
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        let Some(origin) = proposal_store
            .terminal_owner_origin_binding(&proposal_id)
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        let relation_kind = proposal_store
            .terminal_relation_projection_proof(&proposal_id)
            .map_err(|error| error.to_string())?
            .map(|proof| proof.relation_kind());
        let dispatch_state = proposal_store
            .dispatch_state(&proposal_id)
            .map_err(|error| error.to_string())?;
        (proposal, origin, relation_kind, dispatch_state)
    };
    let canonical_scope = proposal.after.get("canonical_scope");
    let scope_string = |key: &str| {
        canonical_scope
            .and_then(|scope| scope.get(key))
            .and_then(serde_json::Value::as_str)
            .or_else(|| proposal.after.get(key).and_then(serde_json::Value::as_str))
    };
    let blocked_action = proposal.after.get("blocked_action");
    let blocked_string = |key: &str| {
        blocked_action
            .and_then(|blocked| blocked.get(key))
            .and_then(serde_json::Value::as_str)
    };
    let Some(permission_scope) = scope_string("tool_name") else {
        return Ok(None);
    };
    if proposal.status != ProposalStatus::Accepted
        || proposal.proposal_type != ProposalType::ToolPermission
        || proposal.source != ProposalSource::ChatConversation
        || dispatch_state.as_deref() != Some("confirmed")
        || relation_kind
            != Some(openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite)
        || proposal
            .after
            .get("permission_scope_kind")
            .and_then(serde_json::Value::as_str)
            != Some("network_policy")
        || proposal
            .after
            .get("permission")
            .and_then(serde_json::Value::as_str)
            != Some("allow_once")
        || scope_string("source") != Some("network_policy")
        || scope_string("risk_level") != Some("medium")
        || scope_string("action_type") != Some("network")
        || scope_string("network_capability") != Some(envelope.requested_target.as_str())
        || blocked_string("target") != Some(envelope.requested_target.as_str())
        || origin.task_session_id() != session.id
        || origin.run_id() != envelope.run_id
    {
        return Ok(None);
    }
    let epoch_matches = {
        let event_store = state
            .main_chat_agent_event_store
            .as_ref()
            .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?
            .lock()
            .await;
        event_store
            .terminal_owner_epoch(&session.id)
            .map_err(|error| error.to_string())?
            .is_some_and(|epoch| {
                epoch.state() == crate::main_chat_event_stream::TerminalOwnerSealState::Sealed
                    && epoch.epoch_id() == origin.epoch_id()
                    && epoch.generation() == origin.epoch_generation()
                    && epoch.run_id() == origin.run_id()
            })
    };
    if !epoch_matches {
        return Ok(None);
    }
    let available = state
        .tool_permission_store
        .lock()
        .await
        .reviewed_network_once_available_for_proposal(
            &proposal_id,
            permission_scope,
            "network_policy",
            "medium",
            "network",
        )
        .map_err(|error| error.to_string())?;
    Ok(available.then_some(proposal_id))
}

async fn main_chat_provider_consent_ready_for_resume(
    state: &Arc<AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
) -> Result<Option<String>, String> {
    use openlife_core::agent::{ProposalSource, ProposalType};

    if session.status
        != openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    {
        return Ok(None);
    }
    let proposals = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "proposal_store_unavailable".to_string())?
        .lock()
        .await
        .list_accepted_action_resume_prerequisites_for_task(&session.id, 16)
        .map_err(|error| error.to_string())?;
    let mut ready = Vec::new();
    for proposal in proposals {
        let canonical_scope = proposal.after.get("canonical_scope");
        let field = |key: &str| {
            canonical_scope
                .and_then(|scope| scope.get(key))
                .and_then(serde_json::Value::as_str)
                .or_else(|| proposal.after.get(key).and_then(serde_json::Value::as_str))
        };
        if proposal.proposal_type != ProposalType::ToolPermission
            || proposal.source != ProposalSource::ChatConversation
            || proposal
                .after
                .get("permission_scope_kind")
                .and_then(serde_json::Value::as_str)
                != Some("network_policy")
            || proposal
                .after
                .get("permission")
                .and_then(serde_json::Value::as_str)
                != Some("allow_once")
            || field("source") != Some("provider")
            || field("risk_level") != Some("high")
            || field("action_type") != Some("network")
        {
            continue;
        }
        let Some(permission_scope) = field("tool_name") else {
            continue;
        };
        let available = state
            .tool_permission_store
            .lock()
            .await
            .reviewed_network_once_available_for_proposal(
                &proposal.id,
                permission_scope,
                "provider",
                "high",
                "network",
            )
            .map_err(|error| error.to_string())?;
        if available {
            ready.push(proposal.id);
        }
    }
    match ready.as_slice() {
        [] => Ok(None),
        [proposal_id] => Ok(Some(proposal_id.clone())),
        _ => Err("provider_consent_continuation_ambiguous_accepted_proposals".into()),
    }
}

fn current_manifest_scope_for_replay_envelope(
    registry: &openlife_core::mcp::McpRegistry,
    envelope: &DurableMainChatReplayExecutionEnvelope,
) -> Option<(String, String, String, String)> {
    // The durable replay envelope is the reviewed action authority. Replanning
    // from the whole user turn is both weaker and incorrect for multi-tool
    // turns because a single-action planner may select a different sibling
    // action. Registry drift is still checked against the exact frozen manifest
    // identity before any replay can be admitted.
    let manifests = registry
        .list_manifests()
        .into_iter()
        .filter(|manifest| {
            manifest.id == envelope.manifest_id
                && manifest.name == envelope.manifest_name
                && manifest.name == envelope.resolved_target
                && manifest.source.to_string() == envelope.manifest_source
                && manifest.execution_contract_digest() == envelope.manifest_contract_digest
        })
        .collect::<Vec<_>>();
    let [manifest] = manifests.as_slice() else {
        return None;
    };
    Some((
        manifest.name.clone(),
        openlife_core::agent::action_executor::helpers::canonical_tool_source(manifest),
        manifest.risk_level.clone(),
        manifest.action_type.clone(),
    ))
}

async fn main_chat_pending_action_permission_ready_for_resume(
    state: &Arc<AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
) -> Result<Option<openlife_core::tool_permissions::ActionBoundToolPermissionAuthorization>, String>
{
    use openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus;

    if !session.selected_strategy.supports_governed_read_replay()
        || action.status != ExecutionQueueStatus::PendingPermission
        || !openlife_core::agent::main_chat_agent_v1::typed_tool_receipt_allows_automatic_retry(
            action,
        )
    {
        return Ok(None);
    }
    let Some(envelope) =
        crate::main_chat_turn_runtime::canonical_openlife_replay_envelope(session, action)
    else {
        return Ok(None);
    };
    let accepted_scope = match main_chat_pending_action_accepted_tool_permission_scope(
        state, session, action, &envelope,
    )
    .await?
    {
        AcceptedToolPermissionScopeLookup::AcceptedValid(scope) => scope,
        AcceptedToolPermissionScopeLookup::NotAccepted
        | AcceptedToolPermissionScopeLookup::AcceptedInvalid => return Ok(None),
    };

    let (tool_name, source, risk_level, action_type) = {
        let registry = state.mcp_registry.lock().await;
        let Some(scope) = current_manifest_scope_for_replay_envelope(&registry, &envelope) else {
            return Ok(None);
        };
        scope
    };
    let scope = &accepted_scope.scope;
    if scope.tool_name != tool_name
        || scope.source != source
        || scope.risk_level != risk_level
        || scope.manifest_action_type != action_type
    {
        return Ok(None);
    }
    let authorization = {
        let permission_store = state.tool_permission_store.lock().await;
        permission_store
            .peek_action_bound(&accepted_scope.proposal_id, scope)
            .map_err(|err| format!("peek action-bound ToolPermission for resume failed: {err}"))?
    };
    Ok(authorization)
}

#[cfg(test)]
pub(crate) async fn main_chat_pending_action_permission_diagnostic_for_test(
    state: &Arc<AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
) -> Result<&'static str, String> {
    use openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus;

    if !session.selected_strategy.supports_governed_read_replay() {
        return Ok("strategy_not_react");
    }
    if action.status != ExecutionQueueStatus::PendingPermission {
        return Ok("action_not_pending_permission");
    }
    if !openlife_core::agent::main_chat_agent_v1::typed_tool_receipt_allows_automatic_retry(action)
    {
        return Ok("typed_receipt_not_retryable");
    }
    let Some(envelope) =
        crate::main_chat_turn_runtime::canonical_openlife_replay_envelope(session, action)
    else {
        return Ok("canonical_replay_envelope_missing");
    };
    let accepted_scope = match main_chat_pending_action_accepted_tool_permission_scope(
        state, session, action, &envelope,
    )
    .await?
    {
        AcceptedToolPermissionScopeLookup::NotAccepted => return Ok("proposal_not_accepted"),
        AcceptedToolPermissionScopeLookup::AcceptedInvalid => {
            let proposal_ids = main_chat_action_proposal_ids(action);
            if proposal_ids.len() != 1 {
                return Ok("accepted_proposal_identity_not_unique");
            }
            let proposal_store = state
                .proposal_store
                .as_ref()
                .ok_or_else(|| "proposal_store_unavailable".to_string())?
                .lock()
                .await;
            let proposal = proposal_store
                .get_proposal(&proposal_ids[0])
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "accepted diagnostic proposal missing".to_string())?;
            let Some(origin) = proposal_store
                .terminal_owner_origin_binding(&proposal_ids[0])
                .map_err(|error| error.to_string())?
            else {
                return Ok("accepted_proposal_origin_missing");
            };
            return Ok(AcceptedToolPermissionScope::diagnose_invalid_for_test(
                &proposal, &origin, session, action, &envelope,
            ));
        }
        AcceptedToolPermissionScopeLookup::AcceptedValid(scope) => scope,
    };
    let (tool_name, source, risk_level, action_type) = {
        let registry = state.mcp_registry.lock().await;
        let Some(scope) = current_manifest_scope_for_replay_envelope(&registry, &envelope) else {
            return Ok("current_manifest_identity_mismatch");
        };
        scope
    };
    let scope = &accepted_scope.scope;
    if scope.tool_name != tool_name {
        return Ok("scope_tool_name_mismatch");
    }
    if scope.source != source {
        return Ok("scope_source_mismatch");
    }
    if scope.risk_level != risk_level {
        return Ok("scope_risk_level_mismatch");
    }
    if scope.manifest_action_type != action_type {
        return Ok("scope_manifest_action_type_mismatch");
    }
    let authorization = state
        .tool_permission_store
        .lock()
        .await
        .peek_action_bound(&accepted_scope.proposal_id, scope)
        .map_err(|error| format!("peek diagnostic action-bound ToolPermission failed: {error}"))?;
    Ok(if authorization.is_some() {
        "ready"
    } else {
        "action_bound_grant_missing_or_consumed"
    })
}

struct AcceptedToolPermissionScope {
    proposal_id: String,
    scope: openlife_core::tool_permissions::ActionBoundToolPermissionScope,
}

enum AcceptedToolPermissionScopeLookup {
    NotAccepted,
    AcceptedInvalid,
    AcceptedValid(Box<AcceptedToolPermissionScope>),
}

impl AcceptedToolPermissionScope {
    fn from_proposal(
        proposal: &openlife_core::agent::AgentProposal,
        origin: &openlife_core::agent::TerminalOwnerOriginBinding,
        session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
        action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
        envelope: &DurableMainChatReplayExecutionEnvelope,
    ) -> Option<Self> {
        if proposal
            .after
            .get("permission_scope_kind")
            .or_else(|| proposal.after.get("permissionScopeKind"))
            .and_then(serde_json::Value::as_str)
            != Some("action_bound")
            || proposal
                .after
                .get("permission")
                .or_else(|| proposal.after.get("policy"))
                .and_then(serde_json::Value::as_str)
                != Some("allow_once")
            || !matches!(
                proposal.source,
                openlife_core::agent::ProposalSource::ChatConversation
            )
            || origin.task_session_id() != session.id
            || origin.run_id() != envelope.run_id
        {
            return None;
        }
        let scope =
            openlife_core::tool_permissions::ActionBoundToolPermissionScope::from_proposal_after(
                &proposal.after,
            )
            .ok()?;
        if scope.queue_action_type != envelope.queue_action_type
            || scope.requested_target != envelope.requested_target
            || scope.resolved_target != envelope.resolved_target
            || scope.tool_name != envelope.manifest_name
            || scope.source != envelope.manifest_source
            || scope.input_hash != envelope.input_hash
            || scope.input_length_bytes != envelope.input_length_bytes
            || !pending_action_identity_matches_envelope(&proposal.after, session, action, envelope)
        {
            return None;
        }
        Some(Self {
            proposal_id: proposal.id.clone(),
            scope,
        })
    }

    #[cfg(test)]
    fn diagnose_invalid_for_test(
        proposal: &openlife_core::agent::AgentProposal,
        origin: &openlife_core::agent::TerminalOwnerOriginBinding,
        session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
        action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
        envelope: &DurableMainChatReplayExecutionEnvelope,
    ) -> &'static str {
        if proposal
            .after
            .get("permission_scope_kind")
            .or_else(|| proposal.after.get("permissionScopeKind"))
            .and_then(serde_json::Value::as_str)
            != Some("action_bound")
        {
            return "accepted_scope_kind_invalid";
        }
        if proposal
            .after
            .get("permission")
            .or_else(|| proposal.after.get("policy"))
            .and_then(serde_json::Value::as_str)
            != Some("allow_once")
        {
            return "accepted_permission_policy_invalid";
        }
        if !matches!(
            proposal.source,
            openlife_core::agent::ProposalSource::ChatConversation
        ) {
            return "accepted_proposal_source_invalid";
        }
        if origin.task_session_id() != session.id {
            return "accepted_origin_task_mismatch";
        }
        if origin.run_id() != envelope.run_id {
            return "accepted_origin_run_mismatch";
        }
        let Ok(scope) =
            openlife_core::tool_permissions::ActionBoundToolPermissionScope::from_proposal_after(
                &proposal.after,
            )
        else {
            return "accepted_scope_parse_failed";
        };
        if scope.queue_action_type != envelope.queue_action_type {
            return "accepted_queue_action_type_mismatch";
        }
        if scope.requested_target != envelope.requested_target {
            return "accepted_requested_target_mismatch";
        }
        if scope.resolved_target != envelope.resolved_target {
            return "accepted_resolved_target_mismatch";
        }
        if scope.tool_name != envelope.manifest_name {
            return "accepted_manifest_name_mismatch";
        }
        if scope.source != envelope.manifest_source {
            return "accepted_manifest_source_mismatch";
        }
        if scope.input_hash != envelope.input_hash {
            return "accepted_input_hash_mismatch";
        }
        if scope.input_length_bytes != envelope.input_length_bytes {
            return "accepted_input_length_mismatch";
        }
        if !pending_action_identity_matches_envelope(&proposal.after, session, action, envelope) {
            return "accepted_pending_action_identity_mismatch";
        }
        "accepted_proposal_binding_invalid_unknown"
    }
}

fn pending_action_identity_matches_envelope(
    after: &serde_json::Value,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
    envelope: &DurableMainChatReplayExecutionEnvelope,
) -> bool {
    let Some(identity) = after
        .get("pending_action_identity")
        .or_else(|| after.get("pendingActionIdentity"))
    else {
        return false;
    };
    let string = |key: &str| identity.get(key).and_then(serde_json::Value::as_str);
    string("taskSessionId") == Some(session.id.as_str())
        && string("runId") == Some(envelope.run_id.as_str())
        && string("queueActionId") == Some(action.id.as_str())
        && string("executorActionId") == Some(envelope.executor_action_id.as_str())
        && string("queueActionType") == Some(envelope.queue_action_type.as_str())
        && string("executorActionType") == Some(envelope.executor_action_type.as_str())
        && string("requestedTarget") == Some(envelope.requested_target.as_str())
        && string("resolvedTarget") == Some(envelope.resolved_target.as_str())
        && string("manifestId") == Some(envelope.manifest_id.as_str())
        && string("manifestName") == Some(envelope.manifest_name.as_str())
        && string("manifestSource") == Some(envelope.manifest_source.as_str())
        && string("manifestContractDigest") == Some(envelope.manifest_contract_digest.as_str())
        && string("inputHash") == Some(envelope.input_hash.as_str())
        && identity
            .get("inputLengthBytes")
            .and_then(serde_json::Value::as_u64)
            == Some(envelope.input_length_bytes)
}

async fn main_chat_pending_action_accepted_tool_permission_scope(
    state: &Arc<AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
    envelope: &DurableMainChatReplayExecutionEnvelope,
) -> Result<AcceptedToolPermissionScopeLookup, String> {
    let proposal_ids = main_chat_action_proposal_ids(action);
    if proposal_ids.len() != 1 {
        return if main_chat_action_has_accepted_tool_permission_proposal(state, action).await? {
            Ok(AcceptedToolPermissionScopeLookup::AcceptedInvalid)
        } else {
            Ok(AcceptedToolPermissionScopeLookup::NotAccepted)
        };
    }
    let proposal_id = &proposal_ids[0];
    let Some(ref proposal_store_arc) = state.proposal_store else {
        return Ok(AcceptedToolPermissionScopeLookup::NotAccepted);
    };
    let proposal_store = proposal_store_arc.lock().await;
    let proposal = proposal_store
        .get_proposal(proposal_id)
        .map_err(|err| format!("load ToolPermission proposal for resume failed: {err}"))?;
    let Some(proposal) = proposal else {
        return Ok(AcceptedToolPermissionScopeLookup::NotAccepted);
    };
    if proposal.status != openlife_core::agent::ProposalStatus::Accepted {
        return Ok(AcceptedToolPermissionScopeLookup::NotAccepted);
    }
    if proposal.proposal_type != openlife_core::agent::ProposalType::ToolPermission {
        return Ok(AcceptedToolPermissionScopeLookup::AcceptedInvalid);
    }
    let Some(origin) = proposal_store
        .terminal_owner_origin_binding(proposal_id)
        .map_err(|err| format!("load ToolPermission terminal origin for resume failed: {err}"))?
    else {
        return Ok(AcceptedToolPermissionScopeLookup::AcceptedInvalid);
    };
    Ok(
        AcceptedToolPermissionScope::from_proposal(&proposal, &origin, session, action, envelope)
            .map(Box::new)
            .map(AcceptedToolPermissionScopeLookup::AcceptedValid)
            .unwrap_or(AcceptedToolPermissionScopeLookup::AcceptedInvalid),
    )
}

async fn main_chat_action_has_accepted_tool_permission_proposal(
    state: &Arc<AppState>,
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
) -> Result<bool, String> {
    let proposal_ids = main_chat_action_proposal_ids(action);
    if proposal_ids.is_empty() {
        return Ok(false);
    }
    let Some(proposal_store_arc) = state.proposal_store.as_ref() else {
        return Ok(false);
    };
    let proposal_store = proposal_store_arc.lock().await;
    for proposal_id in proposal_ids {
        let proposal = proposal_store
            .get_proposal(&proposal_id)
            .map_err(|err| format!("load ToolPermission proposal for diagnostics failed: {err}"))?;
        if proposal.is_some_and(|proposal| {
            proposal.status == openlife_core::agent::ProposalStatus::Accepted
        }) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn main_chat_action_proposal_ids(
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(metadata) = action.observation_metadata.as_ref() {
        collect_main_chat_proposal_ids(metadata, &mut ids);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn collect_main_chat_proposal_ids(value: &serde_json::Value, ids: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if matches!(key.as_str(), "proposalId" | "proposal_id") {
                    if let Some(id) = value.as_str() {
                        ids.push(id.to_string());
                    }
                }
                collect_main_chat_proposal_ids(value, ids);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_main_chat_proposal_ids(value, ids);
            }
        }
        _ => {}
    }
}

/// A sealed task that is waiting on an effect-blocking Review decision no
/// longer has an open turn write lane. Cancelling that product task is the
/// semantic equivalent of rejecting the still-unclaimed prerequisite: use the
/// existing ReviewWorkflow terminal-successor path so the Proposal, task, run,
/// and queued effects converge together instead of attempting a forbidden
/// post-seal lifecycle write.
#[cfg(test)]
async fn sealed_blocking_review_proposals_for_task(
    state: &Arc<AppState>,
    task_session_id: &str,
) -> Result<Vec<String>, String> {
    let epoch = match state.main_chat_agent_event_store.as_ref() {
        Some(store) => store
            .lock()
            .await
            .terminal_owner_epoch(task_session_id)
            .map_err(|error| format!("load terminal owner before cancel failed: {error}"))?,
        None => None,
    };
    let Some(epoch) = epoch else {
        return Ok(Vec::new());
    };
    if epoch.state() != crate::main_chat_event_stream::TerminalOwnerSealState::Sealed {
        return Ok(Vec::new());
    }

    let run = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?
        .lock()
        .await
        .get_run_for_task_id(task_session_id)
        .map_err(|error| format!("load sealed AgentRun before cancel failed: {error}"))?
        .ok_or_else(|| format!("canonical_agent_run_missing_for_task:{task_session_id}"))?;
    if run.id != epoch.run_id() {
        return Err("sealed_cancel_terminal_owner_run_mismatch".into());
    }

    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "proposal_store_unavailable".to_string())?
        .lock()
        .await;
    let mut proposal_ids = Vec::new();
    for proposal_id in &run.generated_proposals {
        let Some(proposal) = proposal_store
            .get_proposal(proposal_id)
            .map_err(|error| format!("load sealed proposal before cancel failed: {error}"))?
        else {
            return Err(format!(
                "sealed_cancel_linked_proposal_missing:{proposal_id}"
            ));
        };
        if !matches!(
            proposal.status,
            openlife_core::agent::ProposalStatus::Pending
                | openlife_core::agent::ProposalStatus::Postponed
                | openlife_core::agent::ProposalStatus::Edited
        ) {
            continue;
        }
        let origin = proposal_store
            .terminal_owner_origin_binding(proposal_id)
            .map_err(|error| format!("load sealed proposal origin before cancel failed: {error}"))?
            .ok_or_else(|| format!("sealed_cancel_proposal_origin_missing:{proposal_id}"))?;
        let relation = proposal_store
            .terminal_relation_projection_proof(proposal_id)
            .map_err(|error| {
                format!("load sealed proposal relation before cancel failed: {error}")
            })?
            .ok_or_else(|| format!("sealed_cancel_proposal_relation_missing:{proposal_id}"))?;
        if origin.task_session_id() != task_session_id
            || origin.run_id() != run.id
            || relation.task_session_id() != task_session_id
            || relation.run_id() != run.id
        {
            return Err(format!(
                "sealed_cancel_proposal_owner_mismatch:{proposal_id}"
            ));
        }
        if matches!(
            relation.relation_kind(),
            openlife_core::agent::ProposalTerminalRelationKind::EffectBlockingPrerequisite
                | openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite
        ) {
            proposal_ids.push(proposal_id.clone());
        }
    }
    proposal_ids.sort();
    proposal_ids.dedup();
    Ok(proposal_ids)
}

#[tauri::command]
#[cfg(test)]
pub(crate) async fn cancel_main_chat_agent_task(
    task_session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatAgentTaskState, String> {
    cancel_main_chat_agent_task_with_state(&task_session_id, state.inner()).await
}

#[cfg(test)]
pub(crate) async fn cancel_main_chat_agent_task_with_state(
    task_session_id: &str,
    state: &Arc<AppState>,
) -> Result<MainChatAgentTaskState, String> {
    let mut current = load_main_chat_agent_task_state(task_session_id, state).await?;
    if current.session.is_none() {
        return Err(format!("Main Chat task not found: {task_session_id}"));
    }
    let sealed_blocking_proposals =
        sealed_blocking_review_proposals_for_task(state, task_session_id).await?;
    if !sealed_blocking_proposals.is_empty() {
        for proposal_id in sealed_blocking_proposals {
            crate::commands::proposal::reject_proposal_with_state(proposal_id, state).await?;
        }
        return load_main_chat_agent_task_state(task_session_id, state).await;
    }
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?;
    let cancellation_registry = {
        state
            .main_chat_runtime_state
            .lock()
            .await
            .cancellation_registry
            .clone()
    };
    // Cancellation intent must be visible to the execution owner before the
    // task-to-AgentRun join is queried. A task is created immediately before
    // its exact AgentRun, so requiring the run first would lose a legitimate
    // user cancellation in that narrow startup window.
    let cancel_request = cancellation_registry.request_cancel(task_session_id);
    let canonical_run_id = {
        let store = store_arc.lock().await;
        crate::terminal_owner_write_gateway::register_agent_run_store_result(
            state,
            store
                .get_run_for_task_id(task_session_id)
                .map_err(|error| error.to_string()),
        )
        .map_err(|error| format!("load canonical AgentRun before cancel failed: {error}"))?
        .map(|run| run.id)
    };
    let Some(canonical_run_id) = canonical_run_id else {
        // This is an accepted pending cancellation, not a terminal result. The
        // TurnRuntime will consume the exact tombstone/active token, create the
        // canonical AgentRun, and only then persist cancellation lifecycle
        // facts. No event or transcript is fabricated without run ownership.
        current.cancellation_pending = true;
        current.can_cancel = false;
        return Ok(current);
    };
    let execution_epoch_snapshot =
        if let Some(execution_epoch) = cancel_request.execution_epoch.as_ref() {
            Some(execution_epoch.wait_for_inflight_commits().await)
        } else {
            None
        };
    let cancel_outcome = cancel_request.outcome;
    if !cancel_outcome.active_turn_found {
        crate::main_chat_turn_runtime::OpenLifeTurnRuntime::new(state)
            .finalize_inactive_cancellation(task_session_id, &canonical_run_id)
            .await?;
    }
    let settled_canonical_effect_state = execution_epoch_snapshot
        .as_ref()
        .map(|snapshot| {
            snapshot
                .cancellation_terminal_disposition()
                .canonical_commit_state()
        })
        .unwrap_or("none");
    let direct_writes_executed = match settled_canonical_effect_state {
        "committed" => Some(true),
        "none" => Some(false),
        "unknown" => None,
        _ => None,
    };
    append_main_chat_agent_transcript(
        state,
        Some(task_session_id),
        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Observation,
        "User requested local cancellation of the active Main Chat turn.",
        serde_json::json!({
            "cancelRequested": true,
            "activeTurnFound": cancel_outcome.active_turn_found,
            "providerAttemptCount": cancel_outcome.provider_attempt_count,
            "providerTerminalCount": cancel_outcome.provider_terminal_count,
            "providerInflightUnknownCount": cancel_outcome.provider_inflight_unknown_count,
            "providerAttemptStateValid": cancel_outcome.provider_attempt_state_valid,
            "terminalDispositionPending": cancel_outcome.active_turn_found,
            "settledCanonicalCommitState": settled_canonical_effect_state,
            "canonicalEffectState": settled_canonical_effect_state,
            "canonicalEffectsCommitted": settled_canonical_effect_state == "committed",
            "settledCanonicalCommitFactCount": execution_epoch_snapshot
                .as_ref()
                .map(|snapshot| snapshot.commit_facts.len())
                .unwrap_or(0),
            "remoteProviderState": if !cancel_outcome.provider_attempt_state_valid
                || cancel_outcome.provider_inflight_unknown_count > 0
            {
                "unknown"
            } else if cancel_outcome.provider_attempt_count > 0 {
                "terminal"
            } else {
                "not_attempted"
            },
            "directWritesExecuted": direct_writes_executed,
        }),
    )
    .await;
    load_main_chat_agent_task_state(task_session_id, state).await
}

async fn cancel_main_chat_nonterminal_actions(
    state: &Arc<AppState>,
    task_session_id: &str,
) -> Result<(), String> {
    let Some(ref queue_arc) = state.main_chat_action_queue_store else {
        return Ok(());
    };
    let metadata = Some(serde_json::json!({
        "cancelRequested": true,
        "taskSessionId": task_session_id,
        "directWritesExecuted": false,
    }));
    let queue = queue_arc.lock().await;
    queue
        .cancel_session_nonterminal(task_session_id, metadata)
        .map_err(|err| format!("cancel Main Chat actions atomically failed: {err}"))?;
    Ok(())
}

pub(crate) async fn reconcile_main_chat_cancellation_projections(
    state: &Arc<AppState>,
    task_session_id: Option<&str>,
) -> Result<usize, String> {
    use std::collections::{BTreeMap, BTreeSet};

    let deliveries = {
        let store_arc = state
            .main_chat_agent_event_store
            .as_ref()
            .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?;
        let store = store_arc.lock().await;
        store
            .list_cancellation_projection_deliveries(task_session_id, 250)
            .map_err(|error| format!("list cancellation projections failed: {error}"))?
    };
    let mut grouped = BTreeMap::<
        String,
        Vec<crate::main_chat_event_stream::MainChatCancellationProjectionDelivery>,
    >::new();
    for delivery in deliveries {
        grouped
            .entry(delivery.cancellation_id.clone())
            .or_default()
            .push(delivery);
    }

    let mut applied = 0usize;
    for (cancellation_id, deliveries) in grouped {
        let targets = deliveries
            .iter()
            .map(|delivery| delivery.projection_target.as_str())
            .collect::<BTreeSet<_>>();
        let first = deliveries
            .first()
            .ok_or_else(|| "cancellation_projection_group_empty".to_string())?;
        if deliveries.iter().any(|delivery| {
            delivery.task_session_id != first.task_session_id
                || delivery.run_id != first.run_id
                || delivery.terminal_event_id != first.terminal_event_id
        }) {
            return Err("cancellation_projection_identity_conflict".into());
        }
        let terminal = {
            let store_arc = state
                .main_chat_agent_event_store
                .as_ref()
                .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?;
            let store = store_arc.lock().await;
            store
                .event_by_id(&first.terminal_event_id)
                .map_err(|error| format!("load cancellation terminal failed: {error}"))?
                .ok_or_else(|| "cancellation_terminal_event_missing".to_string())?
        };
        if terminal.task_session_id != first.task_session_id
            || terminal.run_id != first.run_id
            || terminal
                .payload
                .get("cancellationId")
                .and_then(serde_json::Value::as_str)
                != Some(cancellation_id.as_str())
        {
            return Err("cancellation_projection_terminal_identity_mismatch".into());
        }

        if targets.contains("agent_run") || targets.contains("task_session") {
            let failure_kind = match terminal.event_type.as_str() {
                "local_aborted" => MainChatTaskFailureKind::Cancelled,
                "interrupted" => MainChatTaskFailureKind::Interrupted,
                _ => return Err("cancellation_projection_terminal_type_invalid".into()),
            };
            let projection = finalize_main_chat_task_failure_after_durable_receipt(
                state,
                failure_kind,
                "Reconcile cancellation from its durable terminal receipt.",
                "main_chat_task_controls.cancellation_projection_reconcile",
                terminal.clone(),
            )
            .await;
            for target in ["agent_run", "task_session"] {
                if targets.contains(target) {
                    mark_cancellation_projection_delivery(
                        state,
                        &cancellation_id,
                        target,
                        projection.as_ref().map(|_| ()).map_err(String::as_str),
                    )
                    .await?;
                    if projection.is_ok() {
                        applied += 1;
                    }
                }
            }
        }

        if targets.contains("action_queue") {
            let projection =
                cancel_main_chat_nonterminal_actions(state, &first.task_session_id).await;
            mark_cancellation_projection_delivery(
                state,
                &cancellation_id,
                "action_queue",
                projection.as_ref().map(|_| ()).map_err(String::as_str),
            )
            .await?;
            if projection.is_ok() {
                applied += 1;
            }
        }
    }
    Ok(applied)
}

async fn mark_cancellation_projection_delivery(
    state: &Arc<AppState>,
    cancellation_id: &str,
    projection_target: &str,
    result: Result<(), &str>,
) -> Result<(), String> {
    let store_arc = state
        .main_chat_agent_event_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?;
    let store = store_arc.lock().await;
    match result {
        Ok(()) => store
            .mark_cancellation_projection_applied(cancellation_id, projection_target)
            .map_err(|error| format!("mark cancellation projection applied failed: {error}")),
        Err(error) => {
            let error_digest = openlife_core::agent::metadata_safe::metadata_safe_value_digest(
                &serde_json::json!({ "projectionError": error }),
            )
            .1;
            store
                .mark_cancellation_projection_degraded(
                    cancellation_id,
                    projection_target,
                    &error_digest,
                )
                .map_err(|error| format!("mark cancellation projection degraded failed: {error}"))
        }
    }
}

#[tauri::command]
#[cfg(test)]
pub(crate) async fn retry_main_chat_agent_action(
    task_session_id: String,
    action_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatAgentTaskState, String> {
    let session = if let Some(ref store_arc) = state.main_chat_agent_session_store {
        let store = store_arc.lock().await;
        store
            .load_session(&task_session_id)
            .map_err(|err| format!("load Main Chat task failed: {err}"))?
    } else {
        None
    };
    let action = if let Some(ref queue_arc) = state.main_chat_action_queue_store {
        let queue = queue_arc.lock().await;
        queue
            .load(&action_id)
            .map_err(|err| format!("load Main Chat action failed: {err}"))?
    } else {
        None
    };
    let projected_retry_target =
        get_main_chat_agent_task_detail_with_state(&task_session_id, state.inner())
            .await?
            .retry_target_action_id;
    if projected_retry_target.as_deref() != Some(action_id.as_str()) {
        return Err(
            "retry Main Chat action rejected: action_not_current_backend_retry_target".into(),
        );
    }
    let retry_decision = openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_action_retry(
        session.as_ref(),
        action.as_ref(),
    );
    if !retry_decision.allowed {
        return Err(format!(
            "retry Main Chat action rejected: {}",
            retry_decision.reason_code
        ));
    }

    session
        .as_ref()
        .ok_or_else(|| "retry Main Chat action rejected: task_session_missing".to_string())?;
    action
        .as_ref()
        .ok_or_else(|| "retry Main Chat action rejected: action_missing".to_string())?;
    if retry_decision.manual_blocker_required {
        return Err("retry Main Chat action rejected: typed_retry_receipt_required".into());
    }
    crate::main_chat_turn_runtime::OpenLifeTurnRuntime::new(&state)
        .run_replay(crate::main_chat_turn_runtime::OpenLifeReplayInput::retry(
            &task_session_id,
            &action_id,
        ))
        .await?;
    load_main_chat_agent_task_state(&task_session_id, &state).await
}

pub(crate) async fn load_main_chat_agent_task_state(
    task_session_id: &str,
    state: &Arc<AppState>,
) -> Result<MainChatAgentTaskState, String> {
    let session = if let Some(ref store_arc) = state.main_chat_agent_session_store {
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .map_err(|err| format!("load Main Chat task failed: {err}"))?
    } else {
        None
    };
    let transcript = if let Some(ref store_arc) = state.main_chat_agent_session_store {
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .map_err(|err| format!("load Main Chat transcript failed: {err}"))?
    } else {
        Vec::new()
    };
    let actions = if let Some(ref queue_arc) = state.main_chat_action_queue_store {
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .map_err(|err| format!("load Main Chat actions failed: {err}"))?
    } else {
        Vec::new()
    };
    let pending_approval_count = session
        .as_ref()
        .map(|session| session.pending_blockers.len())
        .unwrap_or(0)
        + actions
            .iter()
            .filter(|action| {
                matches!(
                    action.status,
                    openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
                )
            })
            .count();
    let active_tool_count = actions
        .iter()
        .filter(|action| {
            matches!(
                action.status,
                openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Executing
                    | openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Retrying
            )
        })
        .count();
    let diagnostic_allowed_controls = if let Some(session_ref) = session.as_ref() {
        build_main_chat_agent_task_detail(state, session_ref.clone())
            .await
            .map(|detail| detail.allowed_controls)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let can_resume = diagnostic_allowed_controls
        .iter()
        .any(|control| control == "resume");
    let can_cancel = diagnostic_allowed_controls
        .iter()
        .any(|control| control == "cancel");
    let can_retry = diagnostic_allowed_controls
        .iter()
        .any(|control| control == "retry");
    let cancellation_requested = {
        let registry = state
            .main_chat_runtime_state
            .lock()
            .await
            .cancellation_registry
            .clone();
        registry.is_cancellation_requested(task_session_id)
    };
    let cancellation_pending = cancellation_requested
        && session.as_ref().is_some_and(|session| {
            matches!(
                session.status,
                openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Running
                    | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
                    | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
            )
        });

    Ok(MainChatAgentTaskState {
        session: session.as_ref().map(ProductTaskSession::from_internal),
        actions: actions
            .iter()
            .map(ProductQueuedExecutionAction::from_internal)
            .collect(),
        transcript: project_execution_transcript(transcript),
        pending_approval_count,
        active_tool_count,
        can_resume,
        can_cancel,
        can_retry,
        cancellation_pending,
    })
}

#[cfg(test)]
mod product_task_dto_tests {
    use super::*;
    use serde::Serialize;
    use std::collections::BTreeSet;

    fn serialized_fields(value: impl Serialize) -> BTreeSet<String> {
        serde_json::to_value(value)
            .expect("serialize product task DTO")
            .as_object()
            .expect("product task DTO must be an object")
            .keys()
            .cloned()
            .collect()
    }

    fn typescript_interface_fields(name: &str) -> BTreeSet<String> {
        let typescript = include_str!("../../frontend/src/tauri.ts");
        let marker = format!("export interface {name} {{");
        typescript
            .split(&marker)
            .nth(1)
            .and_then(|tail| tail.split("\n}").next())
            .unwrap_or_else(|| panic!("TypeScript interface {name} is missing"))
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .filter_map(|line| line.split_once(':').map(|(field, _)| field))
            .map(|field| field.trim().trim_end_matches('?').to_string())
            .collect()
    }

    #[test]
    fn product_task_state_excludes_session_bodies_from_ipc() {
        const PLAN_SECRET: &str = "D010_PRIVATE_PLAN_BODY_14CF";
        const BLOCKER_SECRET: &str = "D010_PRIVATE_BLOCKER_BODY_298A";
        const FINAL_SECRET: &str = "D010_PRIVATE_FINAL_BODY_7E31";
        let internal = openlife_core::agent::main_chat_agent_v1::AgentTaskSession {
            id: uuid::Uuid::new_v4().to_string(),
            chat_session_id: "private-conversation-session".into(),
            user_goal: "private user goal body".into(),
            selected_strategy:
                openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer,
            status: openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked,
            current_plan_summary: Some(PLAN_SECRET.into()),
            action_queue_ids: Vec::new(),
            pending_blockers: vec![BLOCKER_SECRET.into()],
            context_snapshot_refs: vec!["private-context-ref".into()],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            final_summary: Some(FINAL_SECRET.into()),
        };
        let state = MainChatAgentTaskState {
            session: Some(ProductTaskSession::from_internal(&internal)),
            actions: Vec::new(),
            transcript: Vec::new(),
            pending_approval_count: 1,
            active_tool_count: 0,
            can_resume: false,
            can_cancel: false,
            can_retry: false,
            cancellation_pending: false,
        };

        let encoded = serde_json::to_string(&state).expect("serialize task state");
        for secret in [PLAN_SECRET, BLOCKER_SECRET, FINAL_SECRET] {
            assert!(
                !encoded.contains(secret),
                "raw task-session body escaped product IPC: {secret}"
            );
        }
    }

    #[test]
    fn product_task_action_and_proposal_exclude_bodies_and_match_typescript() {
        const ACTION_SECRET: &str = "D010_PRIVATE_ACTION_DESCRIPTION_B18D";
        const METADATA_SECRET: &str = "D010_PRIVATE_OBSERVATION_METADATA_3F4A";
        const ERROR_SECRET: &str = "D010_PRIVATE_ACTION_ERROR_9C27";
        const PROPOSAL_SECRET: &str = "D010_PRIVATE_PROPOSAL_PAYLOAD_6D52";
        let session_id = uuid::Uuid::new_v4().to_string();
        let queue =
            openlife_core::agent::main_chat_agent_v1::ActionQueueStore::new_in_memory().unwrap();
        let action = openlife_core::agent::main_chat_agent_v1::ExecutionAction::new(
            "memory.search",
            ACTION_SECRET,
        );
        let policy = openlife_core::agent::main_chat_agent_v1::ExecutionPolicy.classify(&action);
        let mut queued = queue.enqueue(&session_id, action, policy).unwrap();
        queued.observation_metadata = Some(serde_json::json!({"body": METADATA_SECRET}));
        queued.error = Some(ERROR_SECRET.into());
        let product_action = ProductQueuedExecutionAction::from_internal(&queued);

        let mut proposal = openlife_core::agent::AgentProposal::new(
            openlife_core::agent::ProposalType::GoalUpdate,
            PROPOSAL_SECRET,
            serde_json::json!({"body": PROPOSAL_SECRET}),
            PROPOSAL_SECRET,
            0.8,
            openlife_core::agent::RiskLevel::Medium,
            openlife_core::agent::ProposalSource::Manual,
        );
        proposal.run_id = Some(uuid::Uuid::new_v4().to_string());
        proposal.before = Some(serde_json::json!({"body": PROPOSAL_SECRET}));
        proposal.source_detail = Some(PROPOSAL_SECRET.into());
        proposal.resolved_at = Some(chrono::Utc::now());
        proposal.expires_at = Some(chrono::Utc::now());
        let product_proposal = ProductTaskProposal::from_internal(&proposal);

        let encoded = serde_json::to_string(&(&product_action, &product_proposal)).unwrap();
        for secret in [
            ACTION_SECRET,
            METADATA_SECRET,
            ERROR_SECRET,
            PROPOSAL_SECRET,
        ] {
            assert!(
                !encoded.contains(secret),
                "task action/proposal body escaped product IPC: {secret}"
            );
        }
        let action_json = serde_json::to_value(&product_action).unwrap();
        for forbidden in ["description", "observationMetadata", "error", "replayClaim"] {
            assert!(
                action_json.get(forbidden).is_none(),
                "internal task-action key escaped product IPC: {forbidden}"
            );
        }
        let proposal_json = serde_json::to_value(&product_proposal).unwrap();
        for forbidden in ["before", "after", "reason", "affectedPath", "sourceDetail"] {
            assert!(
                proposal_json.get(forbidden).is_none(),
                "internal task-proposal key escaped product IPC: {forbidden}"
            );
        }

        let product_session = ProductTaskSession::from_internal(
            &openlife_core::agent::main_chat_agent_v1::AgentTaskSession {
                id: session_id.clone(),
                chat_session_id: uuid::Uuid::new_v4().to_string(),
                user_goal: ACTION_SECRET.into(),
                selected_strategy:
                    openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer,
                status: openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Running,
                current_plan_summary: Some(ACTION_SECRET.into()),
                action_queue_ids: vec![queued.id.clone()],
                pending_blockers: vec!["network_policy_blocked".into()],
                context_snapshot_refs: vec![METADATA_SECRET.into()],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                final_summary: Some(ACTION_SECRET.into()),
            },
        );

        for (typescript_name, rust_fields) in [
            ("ProductTaskSession", serialized_fields(product_session)),
            (
                "ProductExecutionPolicyDecision",
                serialized_fields(product_action.policy.clone()),
            ),
            (
                "ProductQueuedExecutionAction",
                serialized_fields(product_action),
            ),
            ("ProductTaskProposal", serialized_fields(product_proposal)),
        ] {
            assert_eq!(
                typescript_interface_fields(typescript_name),
                rust_fields,
                "TypeScript {typescript_name} drifted from Rust product task DTO"
            );
        }
        assert!(product_task_blocker("web_network_policy_blocked").is_some());
        assert!(product_task_blocker("d010secret123").is_none());
        assert_eq!(
            product_task_action_type("d010secret123"),
            "unknown_action_type"
        );
        assert_eq!(
            product_task_action_type("lifemodel.learning_candidate.capture"),
            "lifemodel.learning_candidate.capture"
        );
        let learning_policy = openlife_core::agent::main_chat_agent_v1::ExecutionPolicy.classify(
            &openlife_core::agent::main_chat_agent_v1::ExecutionAction::new(
                "lifemodel.learning_candidate.capture",
                "Stage a bounded learning candidate.",
            ),
        );
        assert_eq!(
            product_task_policy_reason(&learning_policy),
            "governed_learning_candidate_capture_allowed"
        );
        assert!(product_route_provider("d010secret123").starts_with("provider:sha256:"));
        assert!(product_route_model_ref("d010secret123").starts_with("model:sha256:"));
        assert!(product_route_reason_ref("d010secret123").starts_with("reason:sha256:"));
        let mut hostile_policy = queued.policy;
        hostile_policy.reason_code = "d010secret123".into();
        assert_eq!(
            ProductExecutionPolicyDecision::from(&hostile_policy).reason_code,
            "unknown_policy_reason"
        );
    }
}
