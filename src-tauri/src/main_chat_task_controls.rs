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

#[derive(serde::Serialize)]
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

#[tauri::command]
pub(crate) async fn get_main_chat_agent_task_state(
    task_session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatAgentTaskState, String> {
    load_main_chat_agent_task_state(&task_session_id, &state).await
}

#[tauri::command]
pub(crate) async fn resume_main_chat_agent_task(
    task_session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatAgentTaskState, String> {
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "Main Chat task session store not available".to_string())?;
    let session = {
        let store = store_arc.lock().await;
        store
            .load_session(&task_session_id)
            .map_err(|err| format!("load Main Chat task before resume failed: {err}"))?
    };
    let actions = if let Some(ref queue_arc) = state.main_chat_action_queue_store {
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(&task_session_id)
            .map_err(|err| format!("load Main Chat actions before resume failed: {err}"))?
    } else {
        Vec::new()
    };
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
                    &state,
                    session_ref,
                    action_ref,
                )
                .await?
                {
                    append_main_chat_agent_transcript(
                        &state,
                        Some(&task_session_id),
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
                        &state,
                        &task_session_id,
                        &action_ref.id,
                        session_ref,
                        action_ref,
                    )
                    .await?;
                    mark_main_chat_action_resume_replay_metadata(&state, &action_ref.id).await?;
                    append_main_chat_agent_transcript(
                        &state,
                        Some(&task_session_id),
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
                    return load_main_chat_agent_task_state(&task_session_id, &state).await;
                }
            }
        }
        let store = store_arc.lock().await;
        store
            .mark_waiting_permission(&task_session_id)
            .map_err(|err| format!("preserve Main Chat permission blocker failed: {err}"))?;
        drop(store);
        append_main_chat_agent_transcript(
            &state,
            Some(&task_session_id),
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
        return load_main_chat_agent_task_state(&task_session_id, &state).await;
    }

    let store = store_arc.lock().await;
    store
        .resume_session(&task_session_id)
        .map_err(|err| format!("resume Main Chat task failed: {err}"))?;
    drop(store);
    append_main_chat_agent_transcript(
        &state,
        Some(&task_session_id),
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
    load_main_chat_agent_task_state(&task_session_id, &state).await
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
    let can_resume = session.as_ref().is_some_and(|session| {
        matches!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
                | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Blocked
                | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
        )
    });
    let can_cancel = session.as_ref().is_some_and(|session| {
        !matches!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled
                | openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
        )
    });
    let can_retry = actions.iter().any(|action| {
        matches!(
            action.status,
            openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
        )
    });

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
