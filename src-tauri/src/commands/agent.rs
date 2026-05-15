use crate::errors::AppError;
use crate::AppState;
use openlife_core::agent::{AgentAction, AgentRun, AgentRunEvent};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_agent_run(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<AgentRun>, AppError> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.get_run(&run_id).map_err(AppError::from)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn list_agent_runs(
    limit: i64,
    offset: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentRun>, AppError> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.list_runs(limit, offset).map_err(AppError::from)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn list_agent_runs_for_session(
    session_id: String,
    limit: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentRun>, AppError> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .list_runs_for_session(&session_id, limit)
            .map_err(AppError::from)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn delete_agent_run(
    run_id: String,
    reason: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .delete_run(&run_id, reason.as_deref())
            .map_err(AppError::from)
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn restore_agent_run(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<AgentRun, AppError> {
    // 1. Restore the run in store
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.restore_run(&run_id).map_err(AppError::from)?;
    } else {
        return Err(AppError::internal("AgentRun store not available"));
    }

    // 2. Retrieve and return the restored run
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .get_run(&run_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Run not found after restore"))
    } else {
        Err(AppError::internal("AgentRun store not available"))
    }
}

/// Replay a single blocked/needs-confirmation action.
///
/// Shared by `replay_agent_action` (the public command) and
/// `continue_agent_plan` (batch continuation).  Returns the replayed
/// `AgentAction` on success.
pub(crate) async fn replay_action_internal(
    run_id: &str,
    action_id: &str,
    state: &Arc<AppState>,
) -> Result<AgentAction, AppError> {
    let mut run = if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .get_run(run_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Run not found"))?
    } else {
        return Err(AppError::internal("AgentRun store not available"));
    };

    let action_idx = run
        .actions
        .iter()
        .position(|a| a.id == action_id)
        .ok_or_else(|| AppError::not_found("Action not found"))?;

    let action = &run.actions[action_idx];

    if action.status != "needs_confirmation" {
        return Err(AppError::permission("Action does not need confirmation"));
    }

    let tool_scope = action
        .tool_scope
        .as_ref()
        .ok_or_else(|| AppError::not_found("Action has no tool_scope"))?;

    let peek_decision = {
        let permission_store = state.tool_permission_store.lock().await;
        permission_store
            .peek(
                &tool_scope.tool_name,
                &tool_scope.source,
                &tool_scope.risk_level,
                &tool_scope.action_type,
                &tool_scope.capabilities,
            )
            .map_err(AppError::from)?
    };
    if !peek_decision.allowed {
        return Err(AppError::permission(format!(
            "Action is not authorized yet. Decision: {} ({})",
            peek_decision.decision, peek_decision.reason
        )));
    }

    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    let calendar_ics_paths = cfg.system.calendar_ics_paths.clone();
    let network_policy = cfg.system.network_policy.clone();
    drop(cfg);
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };

    let executor =
        openlife_core::agent::ActionExecutor::new(openlife_core::agent::ActionExecutorConfig {
            consume_allow_once: false,
            ..Default::default()
        });
    let ctx = openlife_core::agent::ActionContext {
        registry: state.mcp_registry.clone(),
        permission_store: state.tool_permission_store.clone(),
        audit_store: state.mcp_audit_store.clone(),
        privacy_engine: state.privacy_engine.clone(),
        safe_paths,
        life_model: Some(life_model.clone()),
        memory_store: Some(state.memory_store.clone()),
        proposal_store: state.proposal_store.clone(),
        agent_run_store: state.agent_run_store.clone(),
        event_store: state
            .agent_run_event_store
            .as_ref()
            .map(|es| (**es).clone()),
        network_policy: Some(network_policy),
        calendar_ics_paths,
        execution_sandbox: openlife_core::agent::execution_sandbox::ExecutionSandbox::default(),
        agent_spec: None,
    };

    let request = openlife_core::agent::AgentActionRequest {
        action_type: action.action_type.clone(),
        target: action.target.clone().unwrap_or_default(),
        input: action.input.clone(),
        source_run_id: Some(run_id.to_string()),
        step_index: action_idx as u32,
    };

    let exec_result = executor
        .execute(request, &ctx)
        .await
        .map_err(AppError::from)?;

    let mut new_action = exec_result.action;
    let mut new_observation = exec_result.observation;
    new_action.id = action_id.to_string();
    new_observation.action_id = Some(action_id.to_string());

    run.actions[action_idx] = new_action.clone();

    if let Some(obs_idx) = run
        .observations
        .iter()
        .position(|o| o.action_id.as_deref() == Some(action_id))
    {
        run.observations[obs_idx] = new_observation;
    } else {
        run.observations.push(new_observation);
    }

    if let Some(ref proposal_store_arc) = state.proposal_store {
        let proposals = {
            let engine = state.proposal_engine.lock().await;
            engine
                .generate_from_run(&run, "", &life_model)
                .map_err(AppError::from)?
        };
        if !proposals.is_empty() {
            let proposal_store = proposal_store_arc.lock().await;
            for proposal in proposals {
                let proposal_id = proposal.id.clone();
                proposal_store
                    .create_proposal(&proposal)
                    .map_err(AppError::from)?;
                run.add_generated_proposal(&proposal_id);
            }
        }
    }

    let still_pending = run.actions.iter().any(|a| a.status == "needs_confirmation");
    if !still_pending && run.status == openlife_core::agent::AgentRunStatus::WaitingPermission {
        run.status = openlife_core::agent::AgentRunStatus::Completed;
    }

    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.update_run(&run).map_err(AppError::from)?;
    }

    Ok(new_action)
}

#[tauri::command]
pub async fn replay_agent_action(
    run_id: String,
    action_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<AgentAction, AppError> {
    replay_action_internal(&run_id, &action_id, state.inner()).await
}

#[tauri::command]
pub async fn list_agent_run_events(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentRunEvent>, AppError> {
    if let Some(ref es) = state.agent_run_event_store {
        es.list_events_by_run(&run_id).map_err(AppError::from)
    } else {
        Ok(vec![])
    }
}
