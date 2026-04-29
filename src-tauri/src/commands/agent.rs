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

    // 5. Grant AllowOnce permission
    {
        let permission_store = state.tool_permission_store.lock().await;
        permission_store
            .grant(
                &tool_scope.tool_name,
                &tool_scope.source,
                &tool_scope.risk_level,
                &action.action_type,
                openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                None,
            )
            .map_err(|e| e.to_string())?;
    }

    // 6. Re-execute the tool call
    let (reg, audit) = state.get_mcp_state().await;
    let args = action.input.clone();
    let result = crate::execute_tool_call_internal(
        &tool_scope.tool_name,
        args,
        tool_scope.risk_level.clone(),
        &reg,
        &audit,
        Some("allow_once".into()),
    );

    // 7. Update action and observation
    let (new_action, new_observation) = crate::action_observation_from_tool_result(
        &result,
        action.input.clone(),
        None,
    );

    run.actions[action_idx] = new_action.clone();

    // 8. Update observation
    if let Some(obs_idx) = run
        .observations
        .iter()
        .position(|o| o.action_id.as_deref() == Some(&action_id))
    {
        run.observations[obs_idx] = new_observation;
    } else {
        run.observations.push(new_observation);
    }

    // 9. Update run in store
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.update_run(&run).map_err(|e| e.to_string())?;
    }

    Ok(new_action)
}
