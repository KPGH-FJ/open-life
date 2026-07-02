use crate::commands::settings::{
    require_danger_action_confirmation, DangerActionConfirmationEvidence,
};
use crate::errors::AppError;
use crate::main_chat_runtime_facts::{
    provider_transmission_history_from_runs, ProviderTransmissionHistoryItem,
};
use crate::AppState;
use openlife_core::agent::{AgentAction, AgentRun};
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
pub async fn list_provider_transmission_history(
    limit: Option<i64>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ProviderTransmissionHistoryItem>, AppError> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let runs = store.list_runs(limit, 0).map_err(AppError::from)?;
        Ok(provider_transmission_history_from_runs(&runs))
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
    confirmation_evidence: Option<DangerActionConfirmationEvidence>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let evidence = confirmation_evidence.as_ref().ok_or_else(|| {
        AppError::permission("delete_agent_run requires confirmed preflight evidence")
    })?;
    match evidence.action_type.as_str() {
        "agent_run_delete" => {
            require_danger_action_confirmation(
                "agent_run_delete",
                std::slice::from_ref(&run_id),
                Some(1),
                Some(evidence),
                state.inner(),
            )
            .await?;
        }
        "agent_run_bulk_delete" => {
            if !evidence
                .target_ids
                .iter()
                .any(|target_id| target_id == &run_id)
            {
                return Err(AppError::permission(
                    "delete_agent_run target is outside confirmed bulk preflight scope",
                ));
            }
            require_danger_action_confirmation(
                "agent_run_bulk_delete",
                &evidence.target_ids,
                Some(evidence.target_ids.len()),
                Some(evidence),
                state.inner(),
            )
            .await?;
        }
        _ => {
            return Err(AppError::permission(
                "delete_agent_run requires agent_run_delete preflight evidence",
            ));
        }
    }
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

#[tauri::command]
pub async fn replay_agent_action(
    run_id: String,
    action_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<AgentAction, AppError> {
    // 1. Retrieve the run
    let mut run = if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .get_run(&run_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Run not found"))?
    } else {
        return Err(AppError::internal("AgentRun store not available"));
    };

    // 2. Find the action
    let action_idx = run
        .actions
        .iter()
        .position(|a| a.id == action_id)
        .ok_or_else(|| AppError::not_found("Action not found"))?;

    let action = &run.actions[action_idx];

    // 3. Check status
    if action.status != "needs_confirmation" {
        return Err(AppError::permission("Action does not need confirmation"));
    }

    // 4. Get tool scope
    let tool_scope = action
        .tool_scope
        .as_ref()
        .ok_or_else(|| AppError::not_found("Action has no tool_scope"))?;

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
            .map_err(AppError::from)?
    };
    if !peek_decision.allowed {
        return Err(AppError::permission(format!(
            "Action is not authorized yet. Please accept the ToolPermission proposal in Mailbox first. Decision: {} ({})",
            peek_decision.decision, peek_decision.reason
        )));
    }

    // 5. Re-execute the tool call via ActionExecutor.
    let (reg, audit) = state.get_mcp_state().await;
    let permission_store = state.tool_permission_store.lock().await;
    let privacy_engine = state.privacy_engine.lock().await;
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    let calendar_ics_paths = cfg.system.calendar_ics_paths.clone();
    let network_policy = cfg.system.network_policy.clone();
    drop(cfg);
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let memory_store = state.memory_store.lock().await;
    let proposal_store_opt = state.proposal_store.clone();
    let proposal_store_guard = if let Some(ref store) = proposal_store_opt {
        Some(store.lock().await)
    } else {
        None
    };
    let agent_run_store_opt = state.agent_run_store.clone();
    let agent_run_store_guard = if let Some(ref store) = agent_run_store_opt {
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
        calendar_ics_paths: &calendar_ics_paths,
        life_model: Some(&life_model),
        memory_store: Some(&memory_store),
        proposal_store: proposal_store_guard.as_deref(),
        agent_run_store: agent_run_store_guard.as_deref(),
        network_policy: Some(&network_policy),
        hs_runtime_packet: None,
        web_search_fixture_output: None,
    };

    let request = openlife_core::agent::AgentActionRequest {
        action_type: action.action_type.clone(),
        target: tool_scope.tool_name.clone(),
        input: serde_json::json!({ "arguments": args }),
        source_run_id: Some(run_id.clone()),
        step_index: action_idx as u32,
    };

    let exec_result = executor.execute(request, &ctx).map_err(AppError::from)?;
    drop(proposal_store_guard);
    drop(agent_run_store_guard);

    // 6. Update action and observation, preserving the original action id.
    let mut new_action = exec_result.action;
    let mut new_observation = exec_result.observation;
    new_action.id = action_id.clone();
    new_observation.action_id = Some(action_id.clone());
    if let Some(trace) = new_action.react_trace.as_mut() {
        trace.action_id = action_id.clone();
    }
    if let Some(trace) = new_observation.react_trace.as_mut() {
        trace.action_id = action_id.clone();
        trace.observation_id = Some(new_observation.id.clone());
    }

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

    // 8. Update run status if no more pending actions
    let still_pending = run.actions.iter().any(|a| a.status == "needs_confirmation");
    if !still_pending && run.status == openlife_core::agent::AgentRunStatus::WaitingPermission {
        run.status = openlife_core::agent::AgentRunStatus::Completed;
    }

    // 9. Update run in store
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store.update_run(&run).map_err(AppError::from)?;
    }

    Ok(new_action)
}
