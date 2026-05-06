use crate::errors::AppError;
use crate::AppState;
use openlife_core::agent::{AgentPlan, PlanExecutionError, PlanExecutor, PlanOperationResult};
use std::sync::Arc;
use tauri::{Emitter, State};

#[tauri::command]
pub async fn get_agent_plan(
    plan_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<AgentPlan>, AppError> {
    if let Some(ref store_arc) = state.plan_store {
        let store = store_arc.lock().unwrap();
        store.get_plan(&plan_id).map_err(AppError::from)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn list_agent_plans_for_run(
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentPlan>, AppError> {
    if let Some(ref store_arc) = state.plan_store {
        let store = store_arc.lock().unwrap();
        store.list_plans_by_run(&run_id).map_err(AppError::from)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn list_agent_plans_for_session(
    session_id: String,
    limit: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentPlan>, AppError> {
    if let Some(ref store_arc) = state.plan_store {
        let store = store_arc.lock().unwrap();
        store
            .list_plans_by_session(&session_id, limit)
            .map_err(AppError::from)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn confirm_agent_plan(
    plan_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanOperationResult, AppError> {
    let plan_store = state
        .plan_store
        .as_ref()
        .ok_or_else(|| AppError::internal("PlanStore not available"))?;

    let mut plan = {
        let store = plan_store.lock().unwrap();
        store
            .get_plan(&plan_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Plan not found"))?
    };

    plan.confirm();

    {
        let store = plan_store.lock().unwrap();
        store.update_plan(&plan).map_err(AppError::from)?;
    }

    Ok(PlanOperationResult {
        plan_id,
        run_id: plan.run_id,
        operation: "confirm".to_string(),
        success: true,
        status: plan.status,
        steps_completed: None,
        steps_failed: None,
        deviations: vec![],
        review_verdict: None,
        message: Some("plan confirmed".to_string()),
    })
}

#[tauri::command]
pub async fn reject_agent_plan(
    plan_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanOperationResult, AppError> {
    let plan_store = state
        .plan_store
        .as_ref()
        .ok_or_else(|| AppError::internal("PlanStore not available"))?;

    let mut plan = {
        let store = plan_store.lock().unwrap();
        store
            .get_plan(&plan_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Plan not found"))?
    };

    plan.reject();

    {
        let store = plan_store.lock().unwrap();
        store.update_plan(&plan).map_err(AppError::from)?;
    }

    Ok(PlanOperationResult {
        plan_id,
        run_id: plan.run_id,
        operation: "reject".to_string(),
        success: true,
        status: plan.status,
        steps_completed: None,
        steps_failed: None,
        deviations: vec![],
        review_verdict: None,
        message: Some("plan rejected".to_string()),
    })
}

#[tauri::command]
pub async fn execute_agent_plan(
    plan_id: String,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanOperationResult, AppError> {
    let plan_store_arc = state
        .plan_store
        .as_ref()
        .ok_or_else(|| AppError::internal("PlanStore not available"))?
        .clone();

    let plan = {
        let store = plan_store_arc.lock().unwrap();
        store
            .get_plan(&plan_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Plan not found"))?
    };

    let run_id = plan.run_id.clone().unwrap_or_else(|| plan.id.clone());

    // Reject unconfirmed high-risk plans early.
    if plan.requires_confirmation
        && !matches!(plan.status, openlife_core::agent::PlanStatus::Confirmed)
    {
        return Ok(PlanOperationResult {
            plan_id,
            run_id: plan.run_id,
            operation: "execute".to_string(),
            success: false,
            status: plan.status,
            steps_completed: None,
            steps_failed: None,
            deviations: vec![],
            review_verdict: None,
            message: Some("plan requires confirmation before execution".to_string()),
        });
    }

    let event_store = state
        .agent_run_event_store
        .as_ref()
        .map(|es| (**es).clone());

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
        event_store: event_store.clone(),
    };

    // Resolve stored AgentSpec: plan-bound spec first, then stored default.
    let agent_spec = state
        .agent_spec_store
        .lock()
        .map_err(|e| AppError::internal(format!("{}", e)))?
        .resolve_spec(plan.agent_spec_id.as_deref())
        .map_err(|e: openlife_core::agent::AgentSpecStoreError| match &e {
            openlife_core::agent::AgentSpecStoreError::NotFound(_) => AppError::not_found(e.to_string()),
            openlife_core::agent::AgentSpecStoreError::InvalidRole { .. } => AppError::permission(e.to_string()),
            _ => AppError::internal(e.to_string()),
        })?; // execute_agent_plan spec resolution

    let result = run_plan_execution(
        &plan_id,
        &run_id,
        plan_store_arc,
        event_store,
        ctx,
        &app_handle,
        &openlife_core::agent::DefaultPlanReviewGate,
        agent_spec,
    )
    .await;

    drop(proposal_store_guard);
    drop(agent_run_store_guard);
    drop(memory_store);

    Ok(result)
}

#[tauri::command]
pub async fn retry_agent_plan(
    plan_id: String,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanOperationResult, AppError> {
    let plan_store_arc = state
        .plan_store
        .as_ref()
        .ok_or_else(|| AppError::internal("PlanStore not available"))?
        .clone();

    let mut plan = {
        let store = plan_store_arc.lock().unwrap();
        store
            .get_plan(&plan_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Plan not found"))?
    };

    // Only Failed and FailedReview plans are retryable.
    match plan.status {
        openlife_core::agent::PlanStatus::Failed
        | openlife_core::agent::PlanStatus::FailedReview => {}
        _ => {
            return Ok(PlanOperationResult {
                plan_id,
                run_id: plan.run_id,
                operation: "retry".to_string(),
                success: false,
                status: plan.status,
                steps_completed: None,
                steps_failed: None,
                deviations: vec![],
                review_verdict: None,
                message: Some(format!("cannot retry plan in status {:?}", plan.status)),
            });
        }
    }

    let run_id = plan.run_id.clone().unwrap_or_else(|| plan.id.clone());

    // Record retry requested — always, even if setup later fails.
    if let Some(ref es) = state.agent_run_event_store {
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            &run_id,
            openlife_core::agent::AgentRunEventType::PlanRetryRequested,
            openlife_core::agent::AgentEventActor::User,
            format!("plan {} retry requested", plan_id),
            serde_json::json!({"plan_id": plan_id}),
        ));
    }

    // Build execution context BEFORE mutating plan status.
    // If context setup fails, the plan stays in its terminal state.
    let event_store = state
        .agent_run_event_store
        .as_ref()
        .map(|es| (**es).clone());

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
        event_store: event_store.clone(),
    };

    // Context setup complete — now atomically reset plan for retry.
    plan.retry();
    {
        let store = plan_store_arc.lock().unwrap();
        store.update_plan(&plan).map_err(AppError::from)?;
    }
    if let Some(ref es) = state.agent_run_event_store {
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            &run_id,
            openlife_core::agent::AgentRunEventType::PlanRetryStarted,
            openlife_core::agent::AgentEventActor::Runtime,
            format!("plan {} retry started", plan_id),
            serde_json::json!({"plan_id": plan_id}),
        ));
    }

    // Resolve stored AgentSpec: plan-bound spec first, then stored default.
    let agent_spec = state
        .agent_spec_store
        .lock()
        .map_err(|e| AppError::internal(format!("{}", e)))?
        .resolve_spec(plan.agent_spec_id.as_deref())
        .map_err(|e: openlife_core::agent::AgentSpecStoreError| match &e {
            openlife_core::agent::AgentSpecStoreError::NotFound(_) => AppError::not_found(e.to_string()),
            openlife_core::agent::AgentSpecStoreError::InvalidRole { .. } => AppError::permission(e.to_string()),
            _ => AppError::internal(e.to_string()),
        })?; // retry spec resolution

    let mut result = run_plan_execution(
        &plan_id,
        &run_id,
        plan_store_arc,
        event_store,
        ctx,
        &app_handle,
        &openlife_core::agent::DefaultPlanReviewGate,
        agent_spec,
    )
    .await;
    result.operation = "retry".to_string();

    drop(proposal_store_guard);
    drop(agent_run_store_guard);
    drop(memory_store);

    Ok(result)
}

#[tauri::command]
pub async fn cancel_agent_plan(
    plan_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanOperationResult, AppError> {
    let plan_store = state
        .plan_store
        .as_ref()
        .ok_or_else(|| AppError::internal("PlanStore not available"))?;

    let mut plan = {
        let store = plan_store.lock().unwrap();
        store
            .get_plan(&plan_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Plan not found"))?
    };

    // Only cancelable in pre-terminal states.
    match plan.status {
        openlife_core::agent::PlanStatus::Published
        | openlife_core::agent::PlanStatus::Confirmed
        | openlife_core::agent::PlanStatus::Executing => {}
        _ => {
            return Ok(PlanOperationResult {
                plan_id,
                run_id: plan.run_id,
                operation: "cancel".to_string(),
                success: false,
                status: plan.status,
                steps_completed: None,
                steps_failed: None,
                deviations: vec![],
                review_verdict: None,
                message: Some(format!("cannot cancel plan in status {:?}", plan.status)),
            });
        }
    }

    plan.cancel();

    // Record cancel events if event store is available.
    if let Some(ref es) = state.agent_run_event_store {
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            plan.run_id.as_deref().unwrap_or(&plan_id),
            openlife_core::agent::AgentRunEventType::PlanCancelRequested,
            openlife_core::agent::AgentEventActor::User,
            format!("plan {} cancellation requested", plan_id),
            serde_json::json!({"plan_id": plan_id}),
        ));
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            plan.run_id.as_deref().unwrap_or(&plan_id),
            openlife_core::agent::AgentRunEventType::PlanCancelled,
            openlife_core::agent::AgentEventActor::Runtime,
            format!("plan {} cancelled", plan_id),
            serde_json::json!({"plan_id": plan_id}),
        ));
    }

    {
        let store = plan_store.lock().unwrap();
        store.update_plan(&plan).map_err(AppError::from)?;
    }

    Ok(PlanOperationResult {
        plan_id,
        run_id: plan.run_id,
        operation: "cancel".to_string(),
        success: true,
        status: plan.status,
        steps_completed: None,
        steps_failed: None,
        deviations: vec![],
        review_verdict: None,
        message: Some("plan cancelled".to_string()),
    })
}

/// Run the plan execution core: execute steps through PlanExecutor,
/// build PlanOperationResult, and emit plan-execution-done.
///
/// Both `execute_agent_plan` and `retry_agent_plan` route through this
/// after their respective pre-checks are complete.
#[allow(clippy::too_many_arguments)]
async fn run_plan_execution(
    plan_id: &str,
    run_id: &str,
    plan_store_arc: Arc<std::sync::Mutex<openlife_core::agent::PlanStore>>,
    event_store: Option<openlife_core::agent::event_store::AgentRunEventStore>,
    ctx: openlife_core::agent::ActionExecutionContext<'_>,
    app_handle: &tauri::AppHandle,
    review_gate: &impl openlife_core::agent::PlanReviewGate,
    agent_spec: openlife_core::agent::AgentSpec,
) -> PlanOperationResult {
    let executor_config = openlife_core::agent::ActionExecutorConfig {
        allow_writes: false,
        ..Default::default()
    };
    let action_executor = openlife_core::agent::ActionExecutor::new(executor_config);
    let plan_executor =
        PlanExecutor::new(plan_store_arc.clone(), event_store).with_agent_spec(agent_spec);

    let execution_result = plan_executor.execute_with_review(
        plan_id,
        run_id,
        |step, intent| {
            let tool_name = intent
                .map(|i| i.tool_name.clone())
                .unwrap_or_else(|| "unknown".to_string());

            let request = openlife_core::agent::AgentActionRequest {
                action_type: "builtin_tool".to_string(),
                target: tool_name.clone(),
                input: serde_json::json!({
                    "plan_step": step.index,
                    "description": step.description,
                    "plan_id": plan_id,
                }),
                source_run_id: Some(run_id.to_string()),
                step_index: step.index,
            };

            match action_executor.execute(request, &ctx) {
                Ok(result) => {
                    let success = matches!(
                        result.status,
                        openlife_core::agent::ActionExecutionStatus::Succeeded
                    );
                    let deviation = if result.action.tool_scope.as_ref().map(|s| &s.tool_name)
                        != intent.map(|i| &i.tool_name)
                    {
                        Some("executed tool scope differs from plan intent".to_string())
                    } else {
                        None
                    };
                    openlife_core::agent::PlanStepExecutionResult {
                        step_index: step.index,
                        tool_name,
                        success,
                        output: Some(result.observation.content),
                        error: if success { None } else { result.stop_reason },
                        duration_ms: 0,
                        deviation,
                    }
                }
                Err(e) => openlife_core::agent::PlanStepExecutionResult {
                    step_index: step.index,
                    tool_name,
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                    duration_ms: 0,
                    deviation: None,
                },
            }
        },
        review_gate,
    );

    let mut result = match execution_result {
        Ok(outcome) => {
            let (status, review_verdict) = if outcome.success {
                (
                    openlife_core::agent::PlanStatus::Completed,
                    Some("approved".to_string()),
                )
            } else {
                (openlife_core::agent::PlanStatus::Failed, None)
            };
            PlanOperationResult {
                plan_id: plan_id.to_string(),
                run_id: Some(run_id.to_string()),
                operation: "execute".to_string(),
                success: outcome.success,
                status,
                steps_completed: Some(outcome.steps_completed),
                steps_failed: Some(outcome.steps_failed),
                deviations: outcome.deviations,
                review_verdict,
                message: if outcome.success {
                    Some("plan executed successfully".to_string())
                } else {
                    Some("plan execution failed".to_string())
                },
            }
        }
        Err(PlanExecutionError::ReviewFailed(msg)) => PlanOperationResult {
            plan_id: plan_id.to_string(),
            run_id: Some(run_id.to_string()),
            operation: "execute".to_string(),
            success: false,
            status: openlife_core::agent::PlanStatus::FailedReview,
            steps_completed: None,
            steps_failed: None,
            deviations: vec![],
            review_verdict: Some("rejected".to_string()),
            message: Some(msg),
        },
        Err(e) => PlanOperationResult {
            plan_id: plan_id.to_string(),
            run_id: Some(run_id.to_string()),
            operation: "execute".to_string(),
            success: false,
            status: openlife_core::agent::PlanStatus::Failed,
            steps_completed: None,
            steps_failed: None,
            deviations: vec![],
            review_verdict: None,
            message: Some(format!("plan execution error: {}", e)),
        },
    };

    // Re-read persisted plan to capture final status (e.g. Cancelled mid-execution).
    if let Ok(guard) = plan_store_arc.lock() {
        if let Ok(Some(persisted)) = guard.get_plan(plan_id) {
            result.status = persisted.status;
            result.success =
                result.success && persisted.status == openlife_core::agent::PlanStatus::Completed;
            if persisted.status == openlife_core::agent::PlanStatus::Cancelled {
                result.message = Some("plan was cancelled".to_string());
            }
        }
    }

    let _ = app_handle.emit(
        "plan-execution-done",
        serde_json::json!({
            "run_id": run_id,
            "plan_id": plan_id,
            "success": result.success,
            "status": result.status.to_string(),
        }),
    );

    result
}

#[tauri::command]
pub async fn continue_agent_plan(
    plan_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanOperationResult, AppError> {
    let plan_store = state
        .plan_store
        .as_ref()
        .ok_or_else(|| AppError::internal("PlanStore not available"))?;

    let plan = {
        let store = plan_store.lock().unwrap();
        store
            .get_plan(&plan_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Plan not found"))?
    };

    let run_id = plan.run_id.clone().unwrap_or_else(|| plan.id.clone());

    // Count blocked actions in the associated run.
    let run_available = if let Some(ref agent_store_arc) = state.agent_run_store {
        agent_store_arc
            .lock()
            .await
            .get_run(&run_id)
            .is_ok_and(|r| r.is_some())
    } else {
        false
    };

    // Record continuation request event.
    if let Some(ref es) = state.agent_run_event_store {
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            &run_id,
            openlife_core::agent::AgentRunEventType::PlanContinuationRequested,
            openlife_core::agent::AgentEventActor::User,
            format!("plan {} continuation requested", plan_id),
            serde_json::json!({"plan_id": plan_id}),
        ));
    }

    struct ReplayCandidate {
        action_id: String,
        step_index: u32,
    }

    // Build replay candidates: needs_confirmation, plan_id match, step in plan.
    let allowed_steps: Vec<u32> = plan.steps.iter().map(|s| s.index).collect();
    let candidates: Vec<ReplayCandidate> = if run_available {
        let agent_store_ref = state.agent_run_store.as_ref().unwrap();
        let agent_store = agent_store_ref.lock().await;
        if let Ok(Some(run)) = agent_store.get_run(&run_id) {
            run.actions
                .iter()
                .filter(|a| a.status == "needs_confirmation")
                .filter(|a| {
                    a.input
                        .get("plan_id")
                        .and_then(|v| v.as_str())
                        .map(|id| id == plan_id)
                        .unwrap_or(false)
                })
                .filter_map(|a| {
                    a.input
                        .get("plan_step")
                        .and_then(|v| v.as_u64())
                        .map(|i| i as u32)
                        .filter(|step| allowed_steps.contains(step))
                        .map(|step_index| ReplayCandidate {
                            action_id: a.id.clone(),
                            step_index,
                        })
                })
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let eligible_count = candidates.len() as u32;
    let mut replayed = 0u32;
    let mut still_blocked = 0u32;

    let state_arc = state.inner().clone();
    for candidate in &candidates {
        // Emit replay-requested with step_index.
        if let Some(ref es) = state.agent_run_event_store {
            let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
                &run_id,
                openlife_core::agent::AgentRunEventType::PlanActionReplayRequested,
                openlife_core::agent::AgentEventActor::User,
                format!("plan {} action replay requested", plan_id),
                serde_json::json!({
                    "plan_id": plan_id,
                    "action_id": candidate.action_id,
                    "step_index": candidate.step_index,
                }),
            ));
        }
        match crate::commands::agent::replay_action_internal(
            &run_id,
            &candidate.action_id,
            &state_arc,
        )
        .await
        {
            Ok(_) => {
                replayed += 1;
                if let Some(ref es) = state.agent_run_event_store {
                    let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
                        &run_id,
                        openlife_core::agent::AgentRunEventType::PlanActionReplayed,
                        openlife_core::agent::AgentEventActor::Runtime,
                        format!("plan {} action {} replayed", plan_id, candidate.action_id),
                        serde_json::json!({
                            "plan_id": plan_id,
                            "action_id": candidate.action_id,
                            "step_index": candidate.step_index,
                        }),
                    ));
                }
            }
            Err(_) => {
                still_blocked += 1;
            }
        }
    }

    let success =
        run_available && eligible_count > 0 && replayed == eligible_count && still_blocked == 0;

    Ok(PlanOperationResult {
        plan_id,
        run_id: Some(run_id),
        operation: "continue".to_string(),
        success,
        status: plan.status,
        steps_completed: None,
        steps_failed: None,
        deviations: vec![],
        review_verdict: None,
        message: if !run_available {
            Some("run not available for this plan".to_string())
        } else if eligible_count == 0 {
            Some("no eligible blocked plan actions found".to_string())
        } else if still_blocked > 0 {
            Some(format!(
                "{} replayed, {} still blocked (approval required)",
                replayed, still_blocked
            ))
        } else {
            Some(format!("{} action(s) replayed successfully", replayed))
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_confirm_agent_plan_changes_status_to_confirmed() {
        let plan_store = Arc::new(std::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        plan_store.lock().unwrap().create_plan(&plan).unwrap();
        let plan = plan_store
            .lock()
            .unwrap()
            .get_plan(&plan.id)
            .unwrap()
            .unwrap();

        // Directly verify PlanStore confirmation logic (core of the Tauri command).
        let mut p = plan.clone();
        p.confirm();
        plan_store.lock().unwrap().update_plan(&p).unwrap();

        let fetched = plan_store
            .lock()
            .unwrap()
            .get_plan(&plan.id)
            .unwrap()
            .unwrap();
        assert_eq!(fetched.status, openlife_core::agent::PlanStatus::Confirmed);
        assert!(fetched.confirmed_at.is_some());
    }

    #[tokio::test]
    async fn test_reject_agent_plan_changes_status_to_rejected() {
        let plan_store = Arc::new(std::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        plan_store.lock().unwrap().create_plan(&plan).unwrap();
        let plan = plan_store
            .lock()
            .unwrap()
            .get_plan(&plan.id)
            .unwrap()
            .unwrap();

        let mut p = plan.clone();
        p.reject();
        plan_store.lock().unwrap().update_plan(&p).unwrap();

        let fetched = plan_store
            .lock()
            .unwrap()
            .get_plan(&plan.id)
            .unwrap()
            .unwrap();
        assert_eq!(fetched.status, openlife_core::agent::PlanStatus::Rejected);
    }

    #[tokio::test]
    async fn test_execute_rejects_unconfirmed_high_risk() {
        let plan_store = Arc::new(std::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let mut plan = AgentPlan::new("write", openlife_core::agent::RiskLevel::High);
        plan.publish(); // Published but NOT confirmed
        plan_store.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(plan_store, None);
        let result = executor.execute(&plan.id, "run-1", |_step, _intent| {
            openlife_core::agent::PlanStepExecutionResult {
                step_index: 0,
                tool_name: "file.write_proposal".to_string(),
                success: true,
                output: None,
                error: None,
                duration_ms: 0,
                deviation: None,
            }
        });
        assert!(matches!(
            result,
            Err(PlanExecutionError::PlanNotConfirmed(_))
        ));
    }

    #[tokio::test]
    async fn test_cancel_confirmed_plan_succeeds() {
        let plan_store = Arc::new(std::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        plan.confirm();
        plan_store.lock().unwrap().create_plan(&plan).unwrap();

        let mut p = plan.clone();
        p.cancel();
        plan_store.lock().unwrap().update_plan(&p).unwrap();

        let fetched = plan_store
            .lock()
            .unwrap()
            .get_plan(&plan.id)
            .unwrap()
            .unwrap();
        assert_eq!(fetched.status, openlife_core::agent::PlanStatus::Cancelled);
    }

    // ── P7 stabilization: plan-bound AgentSpec resolution tests ──────

    #[tokio::test]
    async fn test_plan_bound_deny_spec_blocks_tool_before_execution() {
        use openlife_core::agent::AgentSpecStore;
        use openlife_core::agent::AgentRoleKind;

        let plan_store = Arc::new(std::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let event_store = openlife_core::agent::event_store::AgentRunEventStore::new_in_memory().unwrap();

        let spec_store = AgentSpecStore::new_in_memory().unwrap();
        let deny_spec = openlife_core::agent::AgentSpec::new(
            AgentRoleKind::Main,
            "Deny Spec",
            "deny file.read",
        )
        .with_id("main.deny".to_string())
        .with_denied_tools(vec!["file.read".to_string()]);
        spec_store.create_spec(&deny_spec).unwrap();

        let mut plan = AgentPlan::new("read file", openlife_core::agent::RiskLevel::Low);
        plan.steps = vec![openlife_core::agent::PlanStep {
            index: 0,
            description: "Read a file".into(),
            tool_intent: Some("file.read".into()),
            expected_output: None,
            depends_on: vec![],
        }];
        plan.tool_intents = vec![openlife_core::agent::ToolIntent {
            tool_name: "file.read".into(),
            purpose: "read".into(),
            risk_level: openlife_core::agent::RiskLevel::Low,
            is_write: false,
            parameters_summary: None,
        }];
        plan.publish();
        plan.confirm();
        plan.agent_spec_id = Some("main.deny".to_string());
        plan_store.lock().unwrap().create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(plan_store.clone(), Some(event_store))
            .with_agent_spec(deny_spec);

        let result = executor.execute(&plan.id, "run-1", |_step, _intent| {
            openlife_core::agent::PlanStepExecutionResult {
                step_index: 0,
                tool_name: "file.read".to_string(),
                success: true,
                output: None,
                error: None,
                duration_ms: 0,
                deviation: None,
            }
        });

        let outcome = result.expect("execution should complete even with blocked tool");
        assert!(!outcome.success, "blocked tool should cause plan step failure");
        assert_eq!(outcome.steps_completed, 0, "no steps should have completed");
        assert_eq!(outcome.steps_failed, 1, "one step should have failed");

        // Re-read plan to check status
        let fetched = plan_store.lock().unwrap().get_plan(&plan.id).unwrap().unwrap();
        assert!(matches!(
            fetched.status,
            openlife_core::agent::PlanStatus::Failed
        ));
    }

    #[tokio::test]
    async fn test_execute_plan_without_spec_uses_stored_default() {
        let plan_store = Arc::new(std::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));

        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        plan.confirm();
        plan.agent_spec_id = None; // no plan-bound spec
        plan_store.lock().unwrap().create_plan(&plan).unwrap();

        // When no plan-bound spec, resolve_spec(None) should return default.
        // The resolve_spec method on AgentSpecStore already has tests for this.
        let spec_store = openlife_core::agent::AgentSpecStore::new_in_memory().unwrap();
        let resolved = spec_store.resolve_spec(plan.agent_spec_id.as_deref()).unwrap();
        assert_eq!(resolved.id, "main.default");
        assert_eq!(resolved.role, openlife_core::agent::AgentRoleKind::Main);
    }

    #[test]
    fn test_plan_execution_started_includes_agentspec_id() {
        let plan_store = Arc::new(std::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let event_store = openlife_core::agent::event_store::AgentRunEventStore::new_in_memory().unwrap();

        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        plan.confirm();
        plan_store.lock().unwrap().create_plan(&plan).unwrap();

        let spec = openlife_core::agent::AgentSpec::default_main_spec();

        let executor = PlanExecutor::new(plan_store, Some(event_store.clone()))
            .with_agent_spec(spec);

        let _ = executor.execute(&plan.id, "run-1", |_step, _intent| {
            openlife_core::agent::PlanStepExecutionResult {
                step_index: 0,
                tool_name: "life_model.read".to_string(),
                success: true,
                output: None,
                error: None,
                duration_ms: 0,
                deviation: None,
            }
        });

        let events = event_store.list_events_by_run("run-1").unwrap();
        let started_evt = events
            .iter()
            .find(|e| e.event_type == openlife_core::agent::AgentRunEventType::PlanExecutionStarted)
            .expect("PlanExecutionStarted event should exist");
        let agentspec_id = started_evt.payload.get("agentspec_id")
            .and_then(|v| v.as_str());
        assert_eq!(agentspec_id, Some("main.default"));
    }

    #[test]
    fn test_agent_plan_with_agent_spec_builder() {
        let plan = AgentPlan::new("plan with spec", openlife_core::agent::RiskLevel::Low)
            .with_agent_spec("main.custom");
        assert_eq!(plan.agent_spec_id, Some("main.custom".to_string()));
    }
}
