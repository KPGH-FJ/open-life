use crate::errors::AppError;
use crate::AppState;
use openlife_core::agent::{AgentPlan, PlanExecutionError, PlanExecutor};
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
) -> Result<AgentPlan, AppError> {
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

    Ok(plan)
}

#[tauri::command]
pub async fn reject_agent_plan(
    plan_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<AgentPlan, AppError> {
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

    Ok(plan)
}

#[tauri::command]
pub async fn execute_agent_plan(
    plan_id: String,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
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
        return Err(AppError::permission(format!(
            "Plan {} requires confirmation before execution (status: {:?})",
            plan_id, plan.status
        )));
    }

    let event_store = state
        .agent_run_event_store
        .as_ref()
        .map(|es| (**es).clone());

    // Build ActionExecutor with read-only policy for plan execution.
    // Writes must go through Proposal; the executor will enforce this.
    let executor_config = openlife_core::agent::ActionExecutorConfig {
        allow_writes: false,
        ..Default::default()
    };

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

    let action_executor = openlife_core::agent::ActionExecutor::new(executor_config);

    let plan_executor = PlanExecutor::new(plan_store_arc, event_store);

    let gate = openlife_core::agent::DefaultPlanReviewGate;
    let execution_result = plan_executor
        .execute_with_review(&plan_id, &run_id, |step, intent| {
            let tool_name =
                intent.map(|i| i.tool_name.clone()).unwrap_or_else(|| "unknown".to_string());

            let request = openlife_core::agent::AgentActionRequest {
                action_type: "builtin_tool".to_string(),
                target: tool_name.clone(),
                input: serde_json::json!({"plan_step": step.index, "description": step.description}),
                source_run_id: Some(run_id.clone()),
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
        }, &gate);

    // Drop guards to release held locks after synchronous execution completes.
    drop(proposal_store_guard);
    drop(agent_run_store_guard);
    drop(memory_store);

    // Always emit plan-execution-done so ChatPage can refresh trace,
    // even when the review gate rejects the plan.
    match execution_result {
        Ok(outcome) => {
            let _ = app_handle.emit(
                "plan-execution-done",
                serde_json::json!({
                    "run_id": run_id,
                    "plan_id": plan_id,
                    "success": outcome.success,
                    "status": if outcome.success { "completed" } else { "failed" }
                }),
            );
            Ok(serde_json::json!({
                "plan_id": outcome.plan_id,
                "success": outcome.success,
                "steps_completed": outcome.steps_completed,
                "steps_failed": outcome.steps_failed,
                "deviations": outcome.deviations,
            }))
        }
        Err(PlanExecutionError::ReviewFailed(msg)) => {
            let _ = app_handle.emit(
                "plan-execution-done",
                serde_json::json!({
                    "run_id": run_id,
                    "plan_id": plan_id,
                    "success": false,
                    "status": "failed_review"
                }),
            );
            Err(AppError::internal(format!("plan failed review: {}", msg)))
        }
        Err(e) => Err(AppError::internal(format!("plan execution error: {}", e))),
    }
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
}
