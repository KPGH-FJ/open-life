use std::sync::Arc;
use tauri::State;

use crate::main_chat_react_execution::execute_main_chat_react_action_with_executor;
use crate::main_chat_react_tool_selection::{
    build_main_chat_react_action_plan, resolve_main_chat_mcp_read_target,
};
use crate::main_chat_runtime_support::{
    append_main_chat_agent_transcript, fail_main_chat_action, transition_main_chat_action,
};
use crate::AppState;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentTaskState {
    pub session: Option<openlife_core::agent::main_chat_agent_v1::AgentTaskSession>,
    pub actions: Vec<openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction>,
    pub transcript: Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>,
    pub pending_approval_count: usize,
    pub active_tool_count: usize,
    pub can_resume: bool,
    pub can_cancel: bool,
    pub can_retry: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub task_session: openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    pub actions: Vec<openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction>,
    pub transcript: Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>,
    pub proposals: Vec<openlife_core::agent::AgentProposal>,
    pub blockers: Vec<String>,
    #[serde(default)]
    pub final_delivery: Option<serde_json::Value>,
    pub continuity_diagnostics: ContinuityDiagnostics,
    pub allowed_controls: Vec<String>,
    pub next_recommended_control: String,
    #[serde(default)]
    pub last_safe_resume_point: Option<String>,
    pub context_digest: String,
    #[serde(default)]
    pub selected_skill_digest: Option<String>,
    pub tool_manifest_digest: String,
}

fn default_true() -> bool {
    true
}

#[tauri::command]
pub(crate) async fn get_main_chat_agent_task_state(
    task_session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatAgentTaskState, String> {
    load_main_chat_agent_task_state(&task_session_id, &state).await
}

#[tauri::command]
pub(crate) async fn list_main_chat_agent_tasks(
    filter: Option<MainChatAgentTaskFilter>,
    limit: Option<usize>,
    offset: Option<usize>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<TaskSummary>, String> {
    list_main_chat_agent_tasks_with_state(filter, limit, offset, &state).await
}

#[tauri::command]
pub(crate) async fn get_main_chat_agent_task_detail(
    task_session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<TaskDetail, String> {
    get_main_chat_agent_task_detail_with_state(&task_session_id, &state).await
}

#[tauri::command]
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
        if !filter.include_terminal && main_chat_task_status_is_terminal(session.status) {
            continue;
        }
        let detail = build_main_chat_agent_task_detail(state, session).await?;
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
    let allowed_controls = allowed_controls_for_task(&session, &actions, &continuity_diagnostics);
    let next_recommended_control =
        next_recommended_control_for_task(&session, &actions, &continuity_diagnostics);
    let last_safe_resume_point = last_safe_resume_point_for_task(&actions, &continuity_diagnostics);
    let final_delivery = final_delivery_from_task(&session, &transcript);

    Ok(TaskDetail {
        task_session: session,
        actions,
        transcript,
        proposals,
        blockers,
        final_delivery,
        continuity_diagnostics,
        allowed_controls,
        next_recommended_control,
        last_safe_resume_point,
        context_digest,
        selected_skill_digest,
        tool_manifest_digest,
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
    let provider_unavailable = provider_unavailable_detected(state).await;
    let selected_skill_context_digest_mismatch =
        selected_skill_context_digest_mismatch_detected(transcript, selected_skill_digest);
    let plan_revision_mismatch = plan_revision_mismatch_detected(state, transcript).await?;
    let terminal_no_resume = main_chat_task_status_is_terminal(session.status);
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
    diagnostics.automatic_replay_allowed =
        continuity_hard_resume_blocker(&diagnostics).is_none()
            && actions.iter().any(|action| {
                matches!(
                    action.status,
                    openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
                        | openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
                ) && openlife_core::agent::main_chat_agent_v1::main_chat_action_type_supports_automatic_retry(
                    &action.action.action_type,
                )
            });
    Ok(diagnostics)
}

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
    TaskSummary {
        task_session_id: detail.task_session.id.clone(),
        conversation_id: detail.task_session.chat_session_id.clone(),
        run_id: run_id_from_detail(detail).unwrap_or_else(|| "unknown".into()),
        title: bounded_text(&detail.task_session.user_goal, 96),
        strategy: detail.task_session.selected_strategy,
        status: detail.task_session.status,
        last_updated_at: detail.task_session.updated_at,
        last_observation_preview: last_observation_preview(
            &detail.transcript,
            &detail.task_session,
        ),
        pending_blocker_count: detail.blockers.len(),
        pending_proposal_count: detail
            .proposals
            .iter()
            .filter(|proposal| proposal.status == openlife_core::agent::ProposalStatus::Pending)
            .count(),
        next_recommended_control: detail.next_recommended_control.clone(),
        stale_state: stale_state_for_detail(detail),
        resume_safety_digest,
    }
}

fn run_id_from_detail(detail: &TaskDetail) -> Option<String> {
    detail
        .transcript
        .iter()
        .rev()
        .find_map(|entry| string_from_metadata(&entry.metadata, &["runId", "run_id"]))
}

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
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    diagnostics: &ContinuityDiagnostics,
) -> Vec<String> {
    let mut controls = vec!["open_trace".to_string(), "refresh_context".to_string()];
    if diagnostics.terminal_no_resume {
        return vec!["open_trace".into()];
    }
    controls.push("cancel".into());
    if continuity_hard_resume_blocker(diagnostics).is_none() {
        if actions.iter().any(|action| {
            action.status
                == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
                && openlife_core::agent::main_chat_agent_v1::main_chat_action_type_supports_automatic_retry(
                    &action.action.action_type,
                )
        }) {
            controls.push("retry".into());
        }
        if matches!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
                | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
                | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
        ) && actions.iter().any(|action| {
            action.status
                == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
        }) {
            controls.push("resume".into());
        }
    }
    controls.sort();
    controls.dedup();
    controls
}

fn next_recommended_control_for_task(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    diagnostics: &ContinuityDiagnostics,
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
    if actions.iter().any(|action| {
        action.status == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
            && openlife_core::agent::main_chat_agent_v1::main_chat_action_type_supports_automatic_retry(
                &action.action.action_type,
            )
    }) {
        return "retry".into();
    }
    if matches!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    ) {
        return "review_permission".into();
    }
    if matches!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
            | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
    ) {
        return "resume".into();
    }
    "open_trace".into()
}

fn last_safe_resume_point_for_task(
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
            matches!(
                action.status,
                openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
                    | openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
                    | openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Planned
            )
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

fn task_blockers_from_evidence(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
) -> Vec<String> {
    let mut blockers = session.pending_blockers.clone();
    if matches!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
            | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
    ) {
        if let Some(summary) = session.final_summary.as_ref() {
            blockers.push(summary.clone());
        }
    }
    for action in actions {
        match action.status {
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission => {
                blockers.push(format!("pending_permission:{}", action.id));
            }
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed => {
                blockers.push(
                    action
                        .error
                        .clone()
                        .unwrap_or_else(|| format!("action_failed:{}", action.id)),
                );
            }
            _ => {}
        }
    }
    blockers.sort();
    blockers.dedup();
    blockers
}

fn final_delivery_from_task(
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
) -> Option<serde_json::Value> {
    let final_entry = transcript.iter().rev().find(|entry| {
        entry.kind
            == openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::FinalResult
    });
    final_entry
        .map(|entry| {
            serde_json::json!({
                "transcriptEntryId": entry.id,
                "summary": entry.summary,
                "metadata": entry.metadata,
            })
        })
        .or_else(|| {
            session.final_summary.as_ref().map(|summary| {
                serde_json::json!({
                    "summary": summary,
                    "source": "task_session_final_summary",
                })
            })
        })
}

fn last_observation_preview(
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
) -> String {
    transcript
        .iter()
        .rev()
        .find(|entry| {
            matches!(
                entry.kind,
                openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Observation
                    | openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Error
                    | openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::FinalResult
                    | openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::PermissionRequest
            )
        })
        .map(|entry| bounded_text(&entry.summary, 180))
        .or_else(|| session.final_summary.as_ref().map(|summary| bounded_text(summary, 180)))
        .unwrap_or_else(|| "No observation recorded yet.".into())
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
    for action in actions.iter().filter(|action| {
        action.status == ExecutionQueueStatus::PendingPermission
            && openlife_core::agent::main_chat_agent_v1::main_chat_action_type_supports_automatic_retry(
                &action.action.action_type,
            )
    }) {
        let Some(scope) = main_chat_pending_action_accepted_tool_permission_scope(state, action).await?
        else {
            continue;
        };
        if !scope.blocked_action_scope_present {
            continue;
        }
        let plan = build_main_chat_react_action_plan(&session.chat_session_id, &session.user_goal)?;
        let registry = state.mcp_registry.lock().await;
        let resolution = resolve_main_chat_mcp_read_target(&registry, &plan);
        if resolution.blocker_reason.is_some() {
            return Ok(true);
        }
        if !scope.matches_current_resolution(&plan, &resolution) {
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
                &["toolName", "tool_name", "target", "resolvedTarget"],
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
    let Some((plan_session_id, revision)) = transcript.iter().rev().find_map(|entry| {
        let plan_session_id = string_from_metadata(&entry.metadata, &["planExecuteSessionId"])?;
        let revision = entry
            .metadata
            .get("revision")
            .and_then(serde_json::Value::as_u64)?;
        Some((plan_session_id, revision))
    }) else {
        return Ok(false);
    };
    let Some(ref plan_store_arc) = state.plan_execute_session_store else {
        return Ok(true);
    };
    let plan_store = plan_store_arc.lock().await;
    let Some(plan_session) = plan_store
        .get_session(&plan_session_id)
        .map_err(|err| format!("load Plan-Execute session for continuity failed: {err}"))?
    else {
        return Ok(true);
    };
    Ok(plan_session.revision != revision)
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

fn digest_label(value: &serde_json::Value) -> String {
    let (bytes, hash) = openlife_core::agent::react_beta::metadata_safe_value_digest(value);
    format!("bytes:{bytes} hash:{hash}")
}

fn bounded_text(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut output = value
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    output.push_str("...");
    output
}

#[tauri::command]
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
                if main_chat_pending_action_permission_ready_for_resume(
                    state,
                    session_ref,
                    action_ref,
                )
                .await?
                {
                    append_main_chat_agent_transcript(
                        state,
                        Some(task_session_id),
                        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Retry,
                        "Task resume is replaying a pending action after accepted ToolPermission.",
                        serde_json::json!({
                            "actionId": action_ref.id,
                            "resumeRequested": true,
                            "automaticResumeReplayStarted": true,
                            "directWritesExecuted": false,
                        }),
                    )
                    .await;
                    replay_main_chat_agent_action(
                        state,
                        task_session_id,
                        &action_ref.id,
                        session_ref,
                        action_ref,
                    )
                    .await?;
                    mark_main_chat_action_resume_replay_metadata(state, &action_ref.id).await?;
                    append_main_chat_agent_transcript(
                        state,
                        Some(task_session_id),
                        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Observation,
                        "Task resume replay completed through the governed executor.",
                        serde_json::json!({
                            "actionId": action_ref.id,
                            "resumeRequested": true,
                            "automaticResumeReplayCompleted": true,
                            "directWritesExecuted": false,
                        }),
                    )
                    .await;
                    return load_main_chat_agent_task_state(task_session_id, state).await;
                }
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

    let store = store_arc.lock().await;
    store
        .resume_session(task_session_id)
        .map_err(|err| format!("resume Main Chat task failed: {err}"))?;
    drop(store);
    append_main_chat_agent_transcript(
        state,
        Some(task_session_id),
        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Retry,
        "Task was resumed from Main Chat.",
        serde_json::json!({
            "resumeRequested": true,
            "resumeReasonCode": resume_decision.reason_code,
            "resumeBlockedByPendingPermission": false,
            "directWritesExecuted": false,
        }),
    )
    .await;
    load_main_chat_agent_task_state(task_session_id, state).await
}

async fn main_chat_pending_action_permission_ready_for_resume(
    state: &Arc<AppState>,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
) -> Result<bool, String> {
    use openlife_core::agent::main_chat_agent_v1::{ExecutionQueueStatus, MainChatAgentStrategy};

    if session.selected_strategy != MainChatAgentStrategy::ReActToolExecution
        || action.status != ExecutionQueueStatus::PendingPermission
        || !openlife_core::agent::main_chat_agent_v1::main_chat_action_type_supports_automatic_retry(
            &action.action.action_type,
        )
    {
        return Ok(false);
    }
    let Some(accepted_scope) =
        main_chat_pending_action_accepted_tool_permission_scope(state, action).await?
    else {
        return Ok(false);
    };

    let plan = build_main_chat_react_action_plan(&session.chat_session_id, &session.user_goal)?;
    if plan.queue_action_type != action.action.action_type {
        return Ok(false);
    }

    let (tool_name, source, risk_level, action_type, capabilities) = {
        let registry = state.mcp_registry.lock().await;
        let resolution = resolve_main_chat_mcp_read_target(&registry, &plan);
        if resolution.blocker_reason.is_some() {
            return Ok(false);
        }
        if !accepted_scope.matches_current_resolution(&plan, &resolution) {
            return Ok(false);
        }
        let Some(manifest) = registry.list_manifests().into_iter().find(|manifest| {
            manifest.name == resolution.target || manifest.id == resolution.target
        }) else {
            return Ok(false);
        };
        (
            manifest.name.clone(),
            openlife_core::agent::action_executor::helpers::canonical_tool_source(&manifest),
            manifest.risk_level.clone(),
            manifest.action_type.clone(),
            manifest.capabilities.clone(),
        )
    };

    let decision = {
        let permission_store = state.tool_permission_store.lock().await;
        permission_store
            .peek(
                &tool_name,
                &source,
                &risk_level,
                &action_type,
                &capabilities,
            )
            .map_err(|err| format!("peek ToolPermission for resume failed: {err}"))?
    };

    Ok(decision.allowed && decision.policy_id.is_some())
}

struct AcceptedToolPermissionScope {
    blocked_action_scope_present: bool,
    action_type: Option<String>,
    requested_target: Option<String>,
    resolved_target: Option<String>,
    input_hash: Option<String>,
    input_length_bytes: Option<u64>,
}

impl AcceptedToolPermissionScope {
    fn from_proposal(proposal: &openlife_core::agent::AgentProposal) -> Self {
        let Some(blocked_action) = proposal
            .after
            .get("blocked_action")
            .or_else(|| proposal.after.get("blockedAction"))
        else {
            return Self {
                blocked_action_scope_present: false,
                action_type: None,
                requested_target: None,
                resolved_target: None,
                input_hash: None,
                input_length_bytes: None,
            };
        };

        Self {
            blocked_action_scope_present: true,
            action_type: blocked_action
                .get("action_type")
                .or_else(|| blocked_action.get("actionType"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            requested_target: blocked_action
                .get("target")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            resolved_target: blocked_action
                .get("resolved_target")
                .or_else(|| blocked_action.get("resolvedTarget"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            input_hash: blocked_action
                .get("input_hash")
                .or_else(|| blocked_action.get("inputHash"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            input_length_bytes: blocked_action
                .get("input_length_bytes")
                .or_else(|| blocked_action.get("inputLengthBytes"))
                .and_then(serde_json::Value::as_u64),
        }
    }

    fn matches_current_resolution(
        &self,
        plan: &crate::main_chat_react_tool_selection::MainChatReactActionPlan,
        resolution: &crate::main_chat_react_tool_selection::MainChatMcpReadResolution,
    ) -> bool {
        if !self.blocked_action_scope_present {
            return true;
        }
        let (
            Some(action_type),
            Some(requested_target),
            Some(resolved_target),
            Some(input_hash),
            Some(input_length_bytes),
        ) = (
            self.action_type.as_deref(),
            self.requested_target.as_deref(),
            self.resolved_target.as_deref(),
            self.input_hash.as_deref(),
            self.input_length_bytes,
        )
        else {
            return false;
        };
        let (current_length_bytes, current_input_hash) =
            openlife_core::agent::react_beta::metadata_safe_value_digest(&resolution.arguments);
        action_type == plan.queue_action_type
            && requested_target == plan.target
            && resolved_target == resolution.target
            && input_hash == current_input_hash
            && input_length_bytes == current_length_bytes as u64
    }
}

async fn main_chat_pending_action_accepted_tool_permission_scope(
    state: &Arc<AppState>,
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
) -> Result<Option<AcceptedToolPermissionScope>, String> {
    let proposal_ids = main_chat_action_proposal_ids(action);
    if proposal_ids.is_empty() {
        return Ok(None);
    }
    let Some(ref proposal_store_arc) = state.proposal_store else {
        return Ok(None);
    };
    let proposal_store = proposal_store_arc.lock().await;
    for proposal_id in proposal_ids {
        let proposal = proposal_store
            .get_proposal(&proposal_id)
            .map_err(|err| format!("load ToolPermission proposal for resume failed: {err}"))?;
        if let Some(proposal) = proposal.filter(|proposal| {
            proposal.proposal_type == openlife_core::agent::ProposalType::ToolPermission
                && proposal.status == openlife_core::agent::ProposalStatus::Accepted
        }) {
            return Ok(Some(AcceptedToolPermissionScope::from_proposal(&proposal)));
        }
    }
    Ok(None)
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

async fn mark_main_chat_action_resume_replay_metadata(
    state: &Arc<AppState>,
    action_id: &str,
) -> Result<(), String> {
    let queue_arc = state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "Main Chat action queue store not available".to_string())?;
    let queue = queue_arc.lock().await;
    let action = queue
        .load(action_id)
        .map_err(|err| format!("load Main Chat action after resume replay failed: {err}"))?
        .ok_or_else(|| format!("Main Chat action not found after resume replay: {action_id}"))?;
    let mut metadata = action
        .observation_metadata
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(object) = metadata.as_object_mut() {
        object.insert(
            "automaticResumeReplayCompleted".into(),
            serde_json::json!(
                action.status
                    == openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
            ),
        );
        object.insert("resumeRequested".into(), serde_json::json!(true));
        object.insert("directWritesExecuted".into(), serde_json::json!(false));
    }
    queue
        .transition(&action.id, action.status, Some(metadata))
        .map_err(|err| format!("mark Main Chat resume replay metadata failed: {err}"))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn cancel_main_chat_agent_task(
    task_session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatAgentTaskState, String> {
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "Main Chat task session store not available".to_string())?;
    let store = store_arc.lock().await;
    store
        .cancel_session(&task_session_id, "Cancelled from Main Chat controls.")
        .map_err(|err| format!("cancel Main Chat task failed: {err}"))?;
    drop(store);
    cancel_main_chat_nonterminal_actions(&state, &task_session_id).await?;
    append_main_chat_agent_transcript(
        &state,
        Some(&task_session_id),
        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Retry,
        "Task and queued actions were cancelled from Main Chat.",
        serde_json::json!({
            "cancelRequested": true,
            "queuedActionsCancelled": true,
            "directWritesExecuted": false,
        }),
    )
    .await;
    load_main_chat_agent_task_state(&task_session_id, &state).await
}

async fn cancel_main_chat_nonterminal_actions(
    state: &Arc<AppState>,
    task_session_id: &str,
) -> Result<(), String> {
    let Some(ref queue_arc) = state.main_chat_action_queue_store else {
        return Ok(());
    };
    let actions = {
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(task_session_id)
            .map_err(|err| format!("list Main Chat actions before cancel failed: {err}"))?
    };
    for action in actions {
        if matches!(
            action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
                | openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Cancelled
        ) {
            continue;
        }
        transition_main_chat_action(
            state,
            &action.id,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Cancelled,
            Some(serde_json::json!({
                "cancelRequested": true,
                "taskSessionId": task_session_id,
                "previousStatus": action.status.as_str(),
                "directWritesExecuted": false,
            })),
        )
        .await?;
    }
    Ok(())
}

#[tauri::command]
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

    transition_main_chat_action(
        &state,
        &action_id,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Retrying,
        Some(serde_json::json!({ "retryRequested": true })),
    )
    .await?;
    if retry_decision.manual_blocker_required {
        let manual_blocker = format!("manual_retry_replay_required:{action_id}");
        transition_main_chat_action(
            &state,
            &action_id,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission,
            Some(serde_json::json!({
                "retryRequested": true,
                "manualReplayRequired": true,
                "manualBlocker": manual_blocker,
                "automaticExecution": false,
            })),
        )
        .await?;
        if let Some(ref store_arc) = state.main_chat_agent_session_store {
            let store = store_arc.lock().await;
            if let Some(current) = session.as_ref() {
                if matches!(
                    current.status,
                    openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
                        | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
                ) {
                    store
                        .resume_session(&task_session_id)
                        .map_err(|err| format!("resume task before retry blocker failed: {err}"))?;
                }
            }
            store
                .set_pending_blockers(&task_session_id, vec![manual_blocker.clone()])
                .map_err(|err| format!("set retry manual blocker failed: {err}"))?;
            store
                .mark_waiting_permission(&task_session_id)
                .map_err(|err| format!("mark retry manual blocker pending failed: {err}"))?;
        }
        append_main_chat_agent_transcript(
            &state,
            Some(&task_session_id),
            openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::PermissionRequest,
            "Action retry requires manual replay because the failed action has no safe replay payload.",
            serde_json::json!({
                "actionId": action_id,
                "retryRequested": true,
                "manualReplayRequired": true,
                "manualBlocker": manual_blocker,
                "automaticExecution": false,
            }),
        )
        .await;
    } else if let (Some(session_ref), Some(action_ref)) = (session.as_ref(), action.as_ref()) {
        if let Err(error) = replay_main_chat_agent_action(
            &state,
            &task_session_id,
            &action_id,
            session_ref,
            action_ref,
        )
        .await
        {
            let manual_blocker = format!("automatic_retry_replay_blocked:{action_id}");
            transition_main_chat_action(
                &state,
                &action_id,
                openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission,
                Some(serde_json::json!({
                    "retryRequested": true,
                    "manualReplayRequired": true,
                    "manualBlocker": manual_blocker,
                    "automaticExecution": false,
                    "automaticReplayBlocked": true,
                    "retryReplayErrorDigest": openlife_core::agent::react_beta::metadata_safe_value_digest(
                        &serde_json::json!({ "error": error.to_string() })
                    ),
                })),
            )
            .await?;
            if let Some(ref store_arc) = state.main_chat_agent_session_store {
                let store = store_arc.lock().await;
                if matches!(
                    session_ref.status,
                    openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
                        | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
                ) {
                    store
                        .resume_session(&task_session_id)
                        .map_err(|err| format!("resume task before retry blocker failed: {err}"))?;
                }
                store
                    .set_pending_blockers(&task_session_id, vec![manual_blocker.clone()])
                    .map_err(|err| format!("set retry automatic blocker failed: {err}"))?;
                store
                    .mark_waiting_permission(&task_session_id)
                    .map_err(|err| format!("mark retry automatic blocker pending failed: {err}"))?;
            }
            append_main_chat_agent_transcript(
                &state,
                Some(&task_session_id),
                openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::PermissionRequest,
                "Automatic action retry replay could not be safely reconstructed and now requires manual review.",
                serde_json::json!({
                    "actionId": action_id,
                    "retryRequested": true,
                    "manualReplayRequired": true,
                    "manualBlocker": manual_blocker,
                    "automaticExecution": false,
                    "automaticReplayBlocked": true,
                }),
            )
            .await;
        }
    }
    append_main_chat_agent_transcript(
        &state,
        Some(&task_session_id),
        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Retry,
        "Action retry was requested from Main Chat.",
        serde_json::json!({
            "actionId": action_id,
            "retryRequested": true,
            "retryReasonCode": retry_decision.reason_code,
            "manualReplayRequired": retry_decision.manual_blocker_required,
            "automaticExecution": false,
        }),
    )
    .await;
    load_main_chat_agent_task_state(&task_session_id, &state).await
}

async fn replay_main_chat_agent_action(
    state: &Arc<AppState>,
    task_session_id: &str,
    action_id: &str,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    action: &openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction,
) -> Result<(), String> {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentIngress, AgentTaskSessionStatus, ExecutionQueueStatus, ExecutionTranscriptEntryKind,
        MainChatAgentStrategy,
    };

    if session.selected_strategy != MainChatAgentStrategy::ReActToolExecution {
        return Err("retry_replay_strategy_not_react".into());
    }
    if !openlife_core::agent::main_chat_agent_v1::main_chat_action_type_supports_automatic_retry(
        &action.action.action_type,
    ) {
        return Err("retry_replay_action_type_not_supported".into());
    }

    let action_plan =
        build_main_chat_react_action_plan(&session.chat_session_id, &session.user_goal)?;
    if action_plan.queue_action_type != action.action.action_type {
        return Err("retry_replay_plan_mismatch".into());
    }

    if let Some(ref store_arc) = state.main_chat_agent_session_store {
        let store = store_arc.lock().await;
        if matches!(
            session.status,
            AgentTaskSessionStatus::WaitingPermission
                | AgentTaskSessionStatus::Blocked
                | AgentTaskSessionStatus::Failed
        ) {
            store
                .resume_session(task_session_id)
                .map_err(|err| format!("resume task before automatic retry failed: {err}"))?;
        }
        store
            .set_pending_blockers(task_session_id, Vec::new())
            .map_err(|err| format!("clear retry blockers failed: {err}"))?;
    }

    transition_main_chat_action(
        state,
        action_id,
        ExecutionQueueStatus::Executing,
        Some(serde_json::json!({
            "retryRequested": true,
            "automaticExecution": true,
            "automaticReplayStarted": true,
            "directWritesExecuted": false,
        })),
    )
    .await?;

    let local_only_required = AgentIngress::default()
        .decide(
            &session.chat_session_id,
            &session.user_goal,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        )
        .privacy_risk
        .local_only_required;

    let observation = match execute_main_chat_react_action_with_executor(
        state,
        &action_plan,
        local_only_required,
    )
    .await
    {
        Ok(observation) => observation,
        Err(error) => {
            let metadata = serde_json::json!({
                "retryRequested": true,
                "automaticExecution": true,
                "automaticReplayFailed": true,
                "retryReplayErrorDigest": openlife_core::agent::react_beta::metadata_safe_value_digest(
                    &serde_json::json!({ "error": error.to_string() })
                ),
                "directWritesExecuted": false,
            });
            fail_main_chat_action(
                state,
                action_id,
                "automatic retry replay failed",
                metadata.clone(),
            )
            .await?;
            if let Some(ref store_arc) = state.main_chat_agent_session_store {
                let store = store_arc.lock().await;
                store
                    .fail_session(task_session_id, "Automatic retry replay failed.")
                    .map_err(|err| format!("mark automatic retry failed failed: {err}"))?;
            }
            append_main_chat_agent_transcript(
                state,
                Some(task_session_id),
                ExecutionTranscriptEntryKind::Error,
                "Automatic retry replay failed through the governed executor.",
                metadata,
            )
            .await;
            return Ok(());
        }
    };

    let mut retry_metadata = observation.metadata.clone();
    if let Some(object) = retry_metadata.as_object_mut() {
        object.insert("retryRequested".into(), serde_json::json!(true));
        object.insert("automaticExecution".into(), serde_json::json!(true));
        object.insert("directWritesExecuted".into(), serde_json::json!(false));
    }

    match observation.executor_status {
        openlife_core::agent::ActionExecutionStatus::Succeeded => {
            if let Some(object) = retry_metadata.as_object_mut() {
                object.insert("automaticReplayCompleted".into(), serde_json::json!(true));
            }
            transition_main_chat_action(
                state,
                action_id,
                ExecutionQueueStatus::Observed,
                Some(retry_metadata.clone()),
            )
            .await?;
            transition_main_chat_action(state, action_id, ExecutionQueueStatus::Completed, None)
                .await?;
            if let Some(ref store_arc) = state.main_chat_agent_session_store {
                let store = store_arc.lock().await;
                store
                    .set_pending_blockers(task_session_id, Vec::new())
                    .map_err(|err| format!("clear retry blockers after replay failed: {err}"))?;
                store
                    .complete_session(task_session_id, "Automatic retry replay completed.")
                    .map_err(|err| format!("complete task after automatic retry failed: {err}"))?;
            }
            append_main_chat_agent_transcript(
                state,
                Some(task_session_id),
                ExecutionTranscriptEntryKind::Observation,
                "Automatic retry replay completed through the governed executor.",
                retry_metadata,
            )
            .await;
        }
        openlife_core::agent::ActionExecutionStatus::NeedsConfirmation => {
            if let Some(object) = retry_metadata.as_object_mut() {
                object.insert(
                    "automaticReplayNeedsPermission".into(),
                    serde_json::json!(true),
                );
            }
            transition_main_chat_action(
                state,
                action_id,
                ExecutionQueueStatus::PendingPermission,
                Some(retry_metadata.clone()),
            )
            .await?;
            let blocker = observation
                .blocker_reason
                .clone()
                .unwrap_or_else(|| "tool_permission_required".into());
            if let Some(ref store_arc) = state.main_chat_agent_session_store {
                let store = store_arc.lock().await;
                store
                    .set_pending_blockers(task_session_id, vec![blocker.clone()])
                    .map_err(|err| format!("set retry permission blocker failed: {err}"))?;
                store
                    .mark_waiting_permission(task_session_id)
                    .map_err(|err| format!("mark retry permission pending failed: {err}"))?;
            }
            append_main_chat_agent_transcript(
                state,
                Some(task_session_id),
                ExecutionTranscriptEntryKind::PermissionRequest,
                "Automatic retry replay needs permission before it can continue.",
                retry_metadata,
            )
            .await;
        }
        openlife_core::agent::ActionExecutionStatus::Blocked
        | openlife_core::agent::ActionExecutionStatus::Failed => {
            if let Some(object) = retry_metadata.as_object_mut() {
                object.insert("automaticReplayFailed".into(), serde_json::json!(true));
            }
            let blocker = observation
                .blocker_reason
                .clone()
                .unwrap_or_else(|| "automatic_retry_replay_failed".into());
            fail_main_chat_action(state, action_id, &blocker, retry_metadata.clone()).await?;
            if let Some(ref store_arc) = state.main_chat_agent_session_store {
                let store = store_arc.lock().await;
                if observation.executor_status
                    == openlife_core::agent::ActionExecutionStatus::Blocked
                {
                    store
                        .block_session(task_session_id, "Automatic retry replay was blocked.")
                        .map_err(|err| format!("mark automatic retry blocked failed: {err}"))?;
                } else {
                    store
                        .fail_session(task_session_id, "Automatic retry replay failed.")
                        .map_err(|err| format!("mark automatic retry failed failed: {err}"))?;
                }
            }
            append_main_chat_agent_transcript(
                state,
                Some(task_session_id),
                ExecutionTranscriptEntryKind::Error,
                "Automatic retry replay did not complete.",
                retry_metadata,
            )
            .await;
        }
    }

    Ok(())
}

async fn load_main_chat_agent_task_state(
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

    Ok(MainChatAgentTaskState {
        session,
        actions,
        transcript,
        pending_approval_count,
        active_tool_count,
        can_resume,
        can_cancel,
        can_retry,
    })
}
