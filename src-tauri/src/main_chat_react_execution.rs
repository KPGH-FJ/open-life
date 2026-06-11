use std::sync::Arc;

use crate::main_chat_react_runtime::{blocked_main_chat_observation, MainChatObservation};
use crate::main_chat_react_tool_selection::{
    resolve_main_chat_mcp_read_target, MainChatReactActionPlan,
};
use crate::{preview_text, AppState};

pub(crate) async fn execute_main_chat_react_action_with_executor(
    state: &Arc<AppState>,
    plan: &MainChatReactActionPlan,
    local_only_required: bool,
) -> Result<MainChatObservation, String> {
    if local_only_required && plan.requires_network {
        return Err(
            "local-only policy blocks governed network reads for this Main Chat request".into(),
        );
    }

    let (safe_paths, calendar_ics_paths, network_policy) = {
        let cfg = state.config.lock().await;
        let mut safe_paths = cfg.system.safe_paths.clone();
        if let Ok(workspace) = std::env::current_dir().and_then(|dir| dir.canonicalize()) {
            let workspace = workspace.to_string_lossy().to_string();
            if !safe_paths.iter().any(|path| path == &workspace) {
                safe_paths.push(workspace);
            }
        }
        (
            safe_paths,
            cfg.system.calendar_ics_paths.clone(),
            cfg.system.network_policy.clone(),
        )
    };
    let web_search_fixture_output = state.web_search_fixture_output.lock().await.clone();
    let local_permission_store = if plan.uses_ephemeral_file_permission {
        let store = openlife_core::tool_permissions::ToolPermissionStore::new_in_memory()
            .map_err(|err| format!("create ephemeral tool permission store failed: {err}"))?;
        store
            .grant(
                "file.read",
                "builtin",
                "low",
                "read",
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .map_err(|err| format!("grant ephemeral file.read permission failed: {err}"))?;
        Some(store)
    } else {
        None
    };
    let permission_store_guard = if local_permission_store.is_none() {
        Some(state.tool_permission_store.lock().await)
    } else {
        None
    };
    let permission_store_ref = match (&local_permission_store, &permission_store_guard) {
        (Some(store), _) => store,
        (None, Some(store)) => &**store,
        _ => return Err("tool permission store unavailable".into()),
    };

    let registry = state.mcp_registry.lock().await;
    let audit_store = state.mcp_audit_store.lock().await;
    let privacy_engine = state.privacy_engine.lock().await;
    let memory_store = state.memory_store.lock().await;
    let agent_run_store_guard = if let Some(ref store_arc) = state.agent_run_store {
        Some(store_arc.lock().await)
    } else {
        None
    };
    let mcp_read_resolution = resolve_main_chat_mcp_read_target(&registry, plan);
    if let Some(ref blocker_reason) = mcp_read_resolution.blocker_reason {
        return Ok(blocked_main_chat_observation(
            plan,
            blocker_reason,
            serde_json::json!({
                "mcpReadTargetResolved": mcp_read_resolution.resolved,
                "mcpReadTarget": mcp_read_resolution.target,
            }),
        ));
    }

    let mut action_ctx = openlife_core::agent::ActionExecutionContext::new(
        &registry,
        permission_store_ref,
        &audit_store,
        &privacy_engine,
        &safe_paths,
    )
    .with_memory_store(&memory_store)
    .with_network_policy(&network_policy)
    .with_calendar_ics_paths(&calendar_ics_paths);
    if let Some(ref agent_run_store) = agent_run_store_guard {
        action_ctx = action_ctx.with_agent_run_store(agent_run_store);
    }
    if let Some(ref fixture_output) = web_search_fixture_output {
        action_ctx = action_ctx.with_web_search_fixture_output(fixture_output);
    }

    let request_input = if plan.executor_action_type == "mcp_tool" {
        serde_json::json!({ "arguments": mcp_read_resolution.arguments.clone() })
    } else {
        mcp_read_resolution.arguments.clone()
    };
    let request = openlife_core::agent::AgentActionRequest {
        action_type: plan.executor_action_type.clone(),
        target: mcp_read_resolution.target.clone(),
        input: request_input,
        source_run_id: None,
        step_index: 0,
    };
    let result =
        openlife_core::agent::ActionExecutor::new(openlife_core::agent::ActionExecutorConfig {
            allow_writes: false,
            ..Default::default()
        })
        .execute(request, &action_ctx)
        .map_err(|err| format!("ActionExecutor failed: {err}"))?;

    let executor_status = result.status.clone();
    let status_label = match executor_status {
        openlife_core::agent::ActionExecutionStatus::Succeeded => "succeeded",
        openlife_core::agent::ActionExecutionStatus::Failed => "failed",
        openlife_core::agent::ActionExecutionStatus::Blocked => "blocked",
        openlife_core::agent::ActionExecutionStatus::NeedsConfirmation => "needs_confirmation",
    };
    let blocker_reason = result
        .stop_reason
        .clone()
        .or_else(|| result.action.error.clone())
        .or_else(|| {
            result
                .observation
                .structured_result
                .as_ref()
                .and_then(|value| value.get("permission_decision"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
    let output_preview = preview_text(&result.observation.content, 500);
    let proposal_id = result
        .action
        .react_trace
        .as_ref()
        .and_then(|trace| trace.proposal_id.clone())
        .or_else(|| {
            result
                .observation
                .react_trace
                .as_ref()
                .and_then(|trace| trace.proposal_id.clone())
        })
        .or_else(|| {
            result
                .observation
                .structured_result
                .as_ref()
                .and_then(|structured| {
                    structured
                        .get("proposalId")
                        .or_else(|| structured.get("proposal_id"))
                })
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    let final_answer = match executor_status {
        openlife_core::agent::ActionExecutionStatus::Succeeded => format!(
            "I completed the read-only action through the governed ActionExecutor path. Observation:\n\n{}",
            preview_text(&result.observation.content, 700)
        ),
        openlife_core::agent::ActionExecutionStatus::NeedsConfirmation => {
            "That read action needs explicit permission before it can continue. I queued it as a blocker and did not execute any write.".into()
        }
        openlife_core::agent::ActionExecutionStatus::Blocked => format!(
            "That read action is blocked by governance: {}",
            blocker_reason
                .clone()
                .unwrap_or_else(|| "policy_blocked".into())
        ),
        openlife_core::agent::ActionExecutionStatus::Failed => format!(
            "I could not complete that governed read action. Blocker: {}",
            blocker_reason
                .clone()
                .unwrap_or_else(|| "action_failed".into())
        ),
    };
    let mut metadata = serde_json::json!({
        "actionType": plan.queue_action_type.clone(),
        "executorActionType": plan.executor_action_type.clone(),
        "target": mcp_read_resolution.target.clone(),
        "requestedTarget": plan.target.clone(),
        "argumentsDigest": openlife_core::agent::react_beta::metadata_safe_value_digest(&mcp_read_resolution.arguments),
        "actionExecutorBacked": true,
        "mcpReadTargetResolved": mcp_read_resolution.resolved,
        "executorStatus": status_label,
        "actionId": result.action.id,
        "observationId": result.observation.id,
        "stopReason": result.stop_reason,
        "structuredResult": result.observation.structured_result,
        "retryReplayable": matches!(executor_status, openlife_core::agent::ActionExecutionStatus::Failed)
            && openlife_core::agent::main_chat_agent_v1::main_chat_action_type_supports_automatic_retry(&plan.queue_action_type),
        "directWritesExecuted": false,
    });
    if let Some(ref proposal_id) = proposal_id {
        if let Some(object) = metadata.as_object_mut() {
            object.insert("proposalId".into(), serde_json::json!(proposal_id));
        }
    }

    Ok(MainChatObservation {
        summary: format!(
            "Governed ReAct action {} finished with {status_label}.",
            plan.target
        ),
        output_preview,
        final_answer,
        metadata,
        executor_status,
        blocker_reason,
    })
}
