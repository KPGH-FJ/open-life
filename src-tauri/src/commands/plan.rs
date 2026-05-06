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

    let result = run_plan_execution(
        &plan_id,
        &run_id,
        plan_store_arc,
        event_store,
        ctx,
        &app_handle,
        &openlife_core::agent::DefaultPlanReviewGate,
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

    // Record retry events.
    if let Some(ref es) = state.agent_run_event_store {
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            &run_id,
            openlife_core::agent::AgentRunEventType::PlanRetryRequested,
            openlife_core::agent::AgentEventActor::User,
            format!("plan {} retry requested", plan_id),
            serde_json::json!({"plan_id": plan_id}),
        ));
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            &run_id,
            openlife_core::agent::AgentRunEventType::PlanRetryStarted,
            openlife_core::agent::AgentEventActor::Runtime,
            format!("plan {} retry started", plan_id),
            serde_json::json!({"plan_id": plan_id}),
        ));
    }

    // Reset to Confirmed so the execution path accepts it.
    plan.retry();
    {
        let store = plan_store_arc.lock().unwrap();
        store.update_plan(&plan).map_err(AppError::from)?;
    }

    // Build execution context (same as execute_agent_plan).
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

    let mut result = run_plan_execution(
        &plan_id,
        &run_id,
        plan_store_arc,
        event_store,
        ctx,
        &app_handle,
        &openlife_core::agent::DefaultPlanReviewGate,
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
async fn run_plan_execution(
    plan_id: &str,
    run_id: &str,
    plan_store_arc: Arc<std::sync::Mutex<openlife_core::agent::PlanStore>>,
    event_store: Option<openlife_core::agent::event_store::AgentRunEventStore>,
    ctx: openlife_core::agent::ActionExecutionContext<'_>,
    app_handle: &tauri::AppHandle,
    review_gate: &impl openlife_core::agent::PlanReviewGate,
) -> PlanOperationResult {
    let executor_config = openlife_core::agent::ActionExecutorConfig {
        allow_writes: false,
        ..Default::default()
    };
    let action_executor = openlife_core::agent::ActionExecutor::new(executor_config);
    let plan_executor = PlanExecutor::new(plan_store_arc, event_store);

    let execution_result = plan_executor
        .execute_with_review(plan_id, run_id, |step, intent| {
            let tool_name =
                intent.map(|i| i.tool_name.clone()).unwrap_or_else(|| "unknown".to_string());

            let request = openlife_core::agent::AgentActionRequest {
                action_type: "builtin_tool".to_string(),
                target: tool_name.clone(),
                input: serde_json::json!({"plan_step": step.index, "description": step.description}),
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
        }, review_gate);

    let result = match execution_result {
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
    let (blocked_count, run_available) = if let Some(ref agent_store_arc) = state.agent_run_store {
        if let Ok(Some(run)) = {
            let store = agent_store_arc.lock().await;
            store.get_run(&run_id)
        } {
            let count = run
                .actions
                .iter()
                .filter(|a| a.status == "needs_confirmation")
                .count();
            (count, true)
        } else {
            (0, false)
        }
    } else {
        (0, false)
    };

    // Record continuation request event.
    if let Some(ref es) = state.agent_run_event_store {
        let _ = es.append_event(&openlife_core::agent::AgentRunEvent::new(
            &run_id,
            openlife_core::agent::AgentRunEventType::PlanContinuationRequested,
            openlife_core::agent::AgentEventActor::User,
            format!("plan {} continuation requested", plan_id),
            serde_json::json!({"plan_id": plan_id, "blocked_actions": blocked_count}),
        ));
    }

    Ok(PlanOperationResult {
        plan_id,
        run_id: Some(run_id),
        operation: "continue".to_string(),
        success: run_available && blocked_count == 0,
        status: plan.status,
        steps_completed: None,
        steps_failed: None,
        deviations: vec![],
        review_verdict: None,
        message: if !run_available {
            Some("run not available for this plan".to_string())
        } else if blocked_count == 0 {
            Some("no blocked actions to continue".to_string())
        } else {
            Some(format!(
                "{} blocked action(s) found — use replay_agent_action to continue each",
                blocked_count
            ))
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
}
