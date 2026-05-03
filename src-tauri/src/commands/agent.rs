use crate::AppState;
use openlife_core::agent::{AgentAction, AgentRun};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_agent_run(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<AgentRun>, String> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.get_run(&run_id).map_err(|e| e.to_string())
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn list_agent_runs(
    limit: i64,
    offset: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentRun>, String> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.list_runs(limit, offset).map_err(|e| e.to_string())
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn list_agent_runs_for_session(
    session_id: String,
    limit: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentRun>, String> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .list_runs_for_session(&session_id, limit)
            .map_err(|e| e.to_string())
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn delete_agent_run(
    run_id: String,
    reason: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .delete_run(&run_id, reason.as_deref())
            .map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn restore_agent_run(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<AgentRun, String> {
    // 1. Restore the run in store
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.restore_run(&run_id).map_err(|e| e.to_string())?;
    } else {
        return Err("AgentRun store not available".to_string());
    }

    // 2. Retrieve and return the restored run
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .get_run(&run_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Run not found after restore".to_string())
    } else {
        Err("AgentRun store not available".to_string())
    }
}

#[tauri::command]
pub async fn replay_agent_action(
    run_id: String,
    action_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<AgentAction, String> {
    // 1. Retrieve the run
    let mut run = if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .get_run(&run_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Run not found".to_string())?
    } else {
        return Err("AgentRun store not available".to_string());
    };

    // 2. Find the action
    let action_idx = run
        .actions
        .iter()
        .position(|a| a.id == action_id)
        .ok_or_else(|| "Action not found".to_string())?;

    let action = &run.actions[action_idx];

    // 3. Check status
    if action.status != "needs_confirmation" {
        return Err("Action does not need confirmation".to_string());
    }

    // 4. Get tool scope
    let tool_scope = action
        .tool_scope
        .as_ref()
        .ok_or_else(|| "Action has no tool_scope".to_string())?;

    // Pre-check with peek() - does NOT consume AllowOnce policies
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
            .map_err(|e| e.to_string())?
    };
    if !peek_decision.allowed {
        return Err(format!(
            "Action is not authorized yet. Please accept the ToolPermission proposal in Review Center first. Decision: {} ({})",
            peek_decision.decision, peek_decision.reason
        ));
    }

    // 5. Re-execute the tool call via ActionExecutor.
    let (reg, audit) = state.get_mcp_state().await;
    let permission_store = state.tool_permission_store.lock().await;
    let privacy_engine = state.privacy_engine.lock().await;
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    drop(cfg);
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    let memory_store = state.memory_store.lock().await;
    let proposal_store_guard = if let Some(ref store) = state.proposal_store {
        Some(store.lock().await)
    } else {
        None
    };
    let agent_run_store_guard = if let Some(ref store) = state.agent_run_store {
        Some(store.lock().await)
    } else {
        None
    };
    let args = action
        .input
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| action.input.clone());

    let executor =
        openlife_core::agent::ActionExecutor::new(openlife_core::agent::ActionExecutorConfig {
            consume_allow_once: false,
            ..Default::default()
        });
    let ctx = openlife_core::agent::ActionExecutionContext {
        registry: &reg,
        permission_store: &permission_store,
        audit_store: &audit,
        privacy_engine: &privacy_engine,
        safe_paths: &safe_paths,
        life_model: Some(&life_model),
        memory_store: Some(&memory_store),
        proposal_store: proposal_store_guard.as_deref(),
        agent_run_store: agent_run_store_guard.as_deref(),
    };

    let request = openlife_core::agent::AgentActionRequest {
        action_type: action.action_type.clone(),
        target: tool_scope.tool_name.clone(),
        input: serde_json::json!({ "arguments": args }),
        source_run_id: Some(run_id.clone()),
        step_index: action_idx as u32,
    };

    let exec_result = executor.execute(request, &ctx).map_err(|e| e.to_string())?;
    drop(proposal_store_guard);
    drop(agent_run_store_guard);

    // 6. Update action and observation, preserving the original action id.
    let mut new_action = exec_result.action;
    let mut new_observation = exec_result.observation;
    new_action.id = action_id.clone();
    new_observation.action_id = Some(action_id.clone());

    run.actions[action_idx] = new_action.clone();

    // 7. Update observation
    if let Some(obs_idx) = run
        .observations
        .iter()
        .position(|o| o.action_id.as_deref() == Some(&action_id))
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
                .map_err(|e| e.to_string())?
        };
        if !proposals.is_empty() {
            let proposal_store = proposal_store_arc.lock().await;
            for proposal in proposals {
                let proposal_id = proposal.id.clone();
                proposal_store
                    .create_proposal(&proposal)
                    .map_err(|e| e.to_string())?;
                run.add_generated_proposal(&proposal_id);
            }
        }
    }

    // 8. Update run in store
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.update_run(&run).map_err(|e| e.to_string())?;
    }

    Ok(new_action)
}
