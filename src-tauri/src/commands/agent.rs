use crate::errors::AppError;
use crate::AppState;
use openlife_core::agent::action_executor::ExecutionBlockReason;
use openlife_core::agent::{
    ActionExecutionStatus, AgentAction, AgentEventActor, AgentRun, AgentRunEvent, AgentRunEventType,
};
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
    // Helper to record ReplayFailed events at early-failure paths.
    // Use None for fields we don't yet have at the point of failure.
    let record_replay_failed = |reason: &str,
                                block_reason: Option<ExecutionBlockReason>,
                                failure_kind: Option<&str>,
                                tool_name: Option<&str>,
                                source: Option<&str>,
                                agent_spec_id: Option<&str>| {
        if let Some(ref event_store) = state.agent_run_event_store {
            let mut payload = serde_json::json!({
                "status": "failed",
                "run_id": run_id,
                "action_id": action_id,
                "replay_of_action_id": action_id,
                "human_message": reason,
            });
            if let Some(br) = block_reason {
                payload["block_reason"] = serde_json::json!(br.to_string());
            }
            if let Some(fk) = failure_kind {
                payload["failure_kind"] = serde_json::json!(fk);
            }
            if let Some(tn) = tool_name {
                payload["tool_name"] = serde_json::json!(tn);
            }
            if let Some(s) = source {
                payload["source"] = serde_json::json!(s);
            }
            if let Some(asid) = agent_spec_id {
                payload["agent_spec_id"] = serde_json::json!(asid);
            }
            let event = AgentRunEvent::new(
                run_id,
                AgentRunEventType::ReplayFailed,
                AgentEventActor::Runtime,
                reason.to_string(),
                payload,
            );
            let _ = event_store.append_event(&event);
        }
    };

    let mut run = if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        store
            .get_run(run_id)
            .map_err(AppError::from)?
            .ok_or_else(|| {
                record_replay_failed(
                    "Run not found",
                    Some(ExecutionBlockReason::ReplaySpecMissing),
                    None,
                    None,
                    None,
                    None,
                );
                AppError::not_found("Run not found")
            })?
    } else {
        record_replay_failed(
            "AgentRun store not available",
            None,
            Some("internal_error"),
            None,
            None,
            None,
        );
        return Err(AppError::internal("AgentRun store not available"));
    };

    let action_idx = run
        .actions
        .iter()
        .position(|a| a.id == action_id)
        .ok_or_else(|| {
            record_replay_failed(
                "Action not found",
                Some(ExecutionBlockReason::ReplaySpecMissing),
                None,
                None,
                None,
                None,
            );
            AppError::not_found("Action not found")
        })?;

    let action = &run.actions[action_idx];

    if action.status != "needs_confirmation" {
        record_replay_failed(
            "Action does not need confirmation",
            Some(ExecutionBlockReason::InvalidArguments),
            None,
            action.tool_scope.as_ref().map(|s| s.tool_name.as_str()),
            action.tool_scope.as_ref().map(|s| s.source.as_str()),
            None,
        );
        return Err(AppError::permission("Action does not need confirmation"));
    }

    let tool_scope = action.tool_scope.as_ref().ok_or_else(|| {
        record_replay_failed(
            "Action has no tool_scope",
            Some(ExecutionBlockReason::InvalidArguments),
            None,
            None,
            None,
            None,
        );
        AppError::not_found("Action has no tool_scope")
    })?;

    // Snapshot tool_scope values before mutable borrow of `run`
    let replay_tool_name = tool_scope.tool_name.clone();
    let replay_source = tool_scope.source.clone();

    let peek_decision = {
        let permission_store = state.tool_permission_store.lock().await;
        match permission_store.peek(
            &tool_scope.tool_name,
            &tool_scope.source,
            &tool_scope.risk_level,
            &tool_scope.action_type,
            &tool_scope.capabilities,
        ) {
            Ok(d) => d,
            Err(e) => {
                record_replay_failed(
                    &format!("Permission store error: {}", e),
                    None,
                    Some("internal_error"),
                    Some(&replay_tool_name),
                    Some(&replay_source),
                    None,
                );
                return Err(AppError::from(e));
            }
        }
    };
    if !peek_decision.allowed {
        record_replay_failed(
            &format!(
                "Action is not authorized yet. Decision: {} ({})",
                peek_decision.decision, peek_decision.reason
            ),
            Some(ExecutionBlockReason::ToolPermissionDenied),
            None,
            Some(&replay_tool_name),
            Some(&replay_source),
            None,
        );
        return Err(AppError::permission(format!(
            "Action is not authorized yet. Decision: {} ({})",
            peek_decision.decision, peek_decision.reason
        )));
    }

    // ── Restore original AgentSpec governance context ─────────────────
    let agent_spec_id = match resolve_replay_agent_spec_id(&run, state).await {
        Ok(id) => id,
        Err(e) => {
            record_replay_failed(
                "Replay failed: missing AgentSpec governance context",
                Some(ExecutionBlockReason::ReplaySpecMissing),
                None,
                Some(&replay_tool_name),
                Some(&replay_source),
                None,
            );
            return Err(e);
        }
    };
    let agent_spec = {
        let spec_store = state.agent_spec_store.lock().await;
        match spec_store.get_spec(&agent_spec_id) {
            Ok(Some(spec)) => spec,
            Ok(None) | Err(_) => {
                record_replay_failed(
                    &format!("Replay failed: AgentSpec '{}' not found", agent_spec_id),
                    Some(ExecutionBlockReason::ReplaySpecMissing),
                    None,
                    Some(&replay_tool_name),
                    Some(&replay_source),
                    Some(&agent_spec_id),
                );
                return Err(AppError::not_found(format!(
                    "AgentSpec '{}' not found",
                    agent_spec_id
                )));
            }
        }
    };

    // ── Record ReplayStarted event ────────────────────────────────────
    if let Some(ref event_store) = state.agent_run_event_store {
        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::ReplayStarted,
            AgentEventActor::Runtime,
            format!("Replay started for action {}", action_id),
            serde_json::json!({
                "status": "started",
                "run_id": run_id,
                "action_id": action_id,
                "replay_of_action_id": action_id,
                "agent_spec_id": agent_spec_id,
                "tool_name": replay_tool_name,
                "source": replay_source,
            }),
        );
        let _ = event_store.append_event(&event);
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
        agent_spec: Some(agent_spec.clone()),
    };

    let request = openlife_core::agent::AgentActionRequest {
        action_type: action.action_type.clone(),
        target: action.target.clone().unwrap_or_default(),
        input: action.input.clone(),
        source_run_id: Some(run_id.to_string()),
        step_index: action_idx as u32,
    };

    let exec_result = match executor.execute(request, &ctx).await {
        Ok(r) => r,
        Err(e) => {
            record_replay_failed(
                &format!("Replay execution failed: {}", e),
                None,
                Some("internal_error"),
                Some(&replay_tool_name),
                Some(&replay_source),
                Some(&agent_spec_id),
            );
            return Err(AppError::from(e));
        }
    };

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

    // ── Record replay outcome event with typed result ─────────────────
    let outcome_status = match exec_result.status {
        ActionExecutionStatus::Succeeded => "completed",
        ActionExecutionStatus::Blocked => "blocked",
        ActionExecutionStatus::NeedsConfirmation => "needs_confirmation",
        ActionExecutionStatus::Failed => "failed",
    };
    let event_type = if exec_result.status == ActionExecutionStatus::Failed {
        AgentRunEventType::ReplayFailed
    } else {
        AgentRunEventType::ReplayCompleted
    };
    let human_msg = format!("Replay {} for action {}", outcome_status, action_id);

    if let Some(ref event_store) = state.agent_run_event_store {
        let event = AgentRunEvent::new(
            run_id,
            event_type,
            AgentEventActor::Runtime,
            human_msg,
            serde_json::json!({
                "status": outcome_status,
                "run_id": run_id,
                "action_id": action_id,
                "replay_of_action_id": action_id,
                "agent_spec_id": agent_spec_id,
                "tool_name": replay_tool_name,
                "source": replay_source,
                "block_reason": exec_result.block_reason.map(|r| r.to_string()),
                "proposal_reason": exec_result.proposal_reason.map(|r| r.to_string()),
                "failure_kind": exec_result.failure_kind.map(|r| r.to_string()),
            }),
        );
        let _ = event_store.append_event(&event);
    }

    Ok(new_action)
}

/// Resolve the AgentSpec id that must govern the replay.
///
/// Resolution order:
/// 1. `run.agent_spec_id` — set by modern execution paths.
/// 2. Plan-bound spec — look up the plan associated with this run.
/// 3. Fail closed — missing governance context in a formal replay path.
async fn resolve_replay_agent_spec_id(
    run: &openlife_core::agent::AgentRun,
    state: &Arc<AppState>,
) -> Result<String, AppError> {
    if let Some(ref sid) = run.agent_spec_id {
        return Ok(sid.clone());
    }

    if let Some(ref plan_store) = state.plan_store {
        let ps = plan_store.lock().await;
        let plans = ps
            .list_plans_by_run(&run.id)
            .map_err(|e| AppError::internal(format!("Failed to query plans: {}", e)))?;
        if let Some(sid) = plans.into_iter().find_map(|p| p.agent_spec_id) {
            return Ok(sid);
        }
    }

    Err(AppError::permission(
        "Cannot replay action: missing AgentSpec governance context. \
         This run was created without an AgentSpec binding. \
         Replay requires the original governance context to be restored.",
    ))
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
