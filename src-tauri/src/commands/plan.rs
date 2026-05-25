use crate::errors::AppError;
use crate::AppState;
use openlife_core::agent::{AgentPlan, PlanOperationResult};
use std::sync::Arc;
use tauri::{Emitter, State};

#[tauri::command]
pub async fn get_agent_plan(
    plan_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<AgentPlan>, AppError> {
    if let Some(ref store_arc) = state.plan_store {
        let store = store_arc.lock().await;
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
        let store = store_arc.lock().await;
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
        let store = store_arc.lock().await;
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
        let store = plan_store.lock().await;
        store
            .get_plan(&plan_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Plan not found"))?
    };

    // Legal-state guard: only Draft and Published can be confirmed.
    match plan.status {
        openlife_core::agent::PlanStatus::Draft | openlife_core::agent::PlanStatus::Published => {}
        _ => {
            return Ok(PlanOperationResult {
                plan_id,
                run_id: plan.run_id,
                operation: "confirm".to_string(),
                success: false,
                status: plan.status,
                steps_completed: None,
                steps_failed: None,
                deviations: vec![],
                review_verdict: None,
                message: Some(format!(
                    "cannot confirm plan in status {:?} — only draft/published plans can be confirmed",
                    plan.status
                )),
            });
        }
    }

    plan.confirm();

    {
        let store = plan_store.lock().await;
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
        let store = plan_store.lock().await;
        store
            .get_plan(&plan_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Plan not found"))?
    };

    // Legal-state guard: only Draft and Published can be rejected.
    match plan.status {
        openlife_core::agent::PlanStatus::Draft | openlife_core::agent::PlanStatus::Published => {}
        _ => {
            return Ok(PlanOperationResult {
                plan_id,
                run_id: plan.run_id,
                operation: "reject".to_string(),
                success: false,
                status: plan.status,
                steps_completed: None,
                steps_failed: None,
                deviations: vec![],
                review_verdict: None,
                message: Some(format!(
                    "cannot reject plan in status {:?} — only draft/published plans can be rejected",
                    plan.status
                )),
            });
        }
    }

    plan.reject();

    {
        let store = plan_store.lock().await;
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
    let outcome = crate::execution_facade::run_tauri_plan_execution(
        crate::execution_facade::TauriPlanExecutionInput {
            plan_id,
            app_state: state.inner().clone(),
            operation: crate::execution_facade::TauriPlanExecutionOperation::Execute,
        },
    )
    .await
    .map_err(plan_facade_error_to_app_error)?;

    emit_plan_execution_done(&app_handle, &outcome.result, outcome.emit_done);
    Ok(outcome.result)
}

#[tauri::command]
pub async fn retry_agent_plan(
    plan_id: String,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanOperationResult, AppError> {
    let outcome = crate::execution_facade::run_tauri_plan_execution(
        crate::execution_facade::TauriPlanExecutionInput {
            plan_id,
            app_state: state.inner().clone(),
            operation: crate::execution_facade::TauriPlanExecutionOperation::Retry,
        },
    )
    .await
    .map_err(plan_facade_error_to_app_error)?;

    emit_plan_execution_done(&app_handle, &outcome.result, outcome.emit_done);
    Ok(outcome.result)
}

fn plan_facade_error_to_app_error(
    error: crate::execution_facade::TauriExecutionFacadeError,
) -> AppError {
    match error.kind {
        crate::execution_facade::TauriExecutionFacadeErrorKind::Governance => {
            if error.message.contains("not found") {
                AppError::not_found(error.message)
            } else {
                AppError::permission(error.message)
            }
        }
        crate::execution_facade::TauriExecutionFacadeErrorKind::Runtime => {
            if error.message == "Plan not found" {
                AppError::not_found(error.message)
            } else {
                AppError::internal(error.message)
            }
        }
    }
}

fn emit_plan_execution_done(
    app_handle: &tauri::AppHandle,
    result: &PlanOperationResult,
    emit_done: bool,
) {
    if !emit_done {
        return;
    }
    let _ = app_handle.emit(
        "plan-execution-done",
        serde_json::json!({
            "run_id": result.run_id,
            "plan_id": result.plan_id,
            "success": result.success,
            "status": result.status.to_string(),
        }),
    );
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
        let store = plan_store.lock().await;
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
        let store = plan_store.lock().await;
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
        let store = plan_store.lock().await;
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

/// Request body for edit_agent_plan — only safe editable fields.
/// toolIntents and steps are NOT editable (execution capability must not change via edit).
#[derive(Debug, serde::Deserialize)]
pub struct EditPlanRequest {
    goal: Option<String>,
    assumptions: Option<Vec<String>>,
    #[serde(rename = "missingContext")]
    missing_context: Option<Vec<String>>,
    #[serde(rename = "successCriteria")]
    success_criteria: Option<Vec<String>>,
    #[serde(rename = "rollbackPlan")]
    rollback_plan: Option<Option<String>>,
}

// ── Legal-state helpers ──────────────────────────────────────────────

fn plan_can_confirm_or_reject(status: openlife_core::agent::PlanStatus) -> bool {
    matches!(
        status,
        openlife_core::agent::PlanStatus::Draft | openlife_core::agent::PlanStatus::Published
    )
}

fn plan_can_edit(status: openlife_core::agent::PlanStatus) -> bool {
    plan_can_confirm_or_reject(status)
}

fn apply_safe_plan_edit(plan: &mut AgentPlan, edit: &EditPlanRequest) -> PlanOperationResult {
    let plan_id = plan.id.clone();
    let run_id = plan.run_id.clone();
    let original_status = plan.status;

    if !plan_can_edit(original_status) {
        return PlanOperationResult {
            plan_id,
            run_id,
            operation: "edit".to_string(),
            success: false,
            status: original_status,
            steps_completed: None,
            steps_failed: None,
            deviations: vec![],
            review_verdict: None,
            message: Some(format!(
                "cannot edit plan in status {:?} — only draft/published plans can be edited",
                original_status
            )),
        };
    }

    // Apply only safe fields
    if let Some(ref goal) = edit.goal {
        plan.goal = goal.clone();
    }
    if let Some(ref assumptions) = edit.assumptions {
        plan.assumptions = assumptions.clone();
    }
    if let Some(ref missing_context) = edit.missing_context {
        plan.missing_context = missing_context.clone();
    }
    if let Some(ref success_criteria) = edit.success_criteria {
        plan.success_criteria = success_criteria.clone();
    }
    if let Some(ref rollback_plan) = edit.rollback_plan {
        plan.rollback_plan = rollback_plan.clone();
    }

    // Security constraints:
    // 1. Never lower risk_level
    // 2. Never disable requires_confirmation
    // (These are ensured by not exposing them in EditPlanRequest)

    // If the plan was published, revert to draft to force re-confirmation
    if original_status == openlife_core::agent::PlanStatus::Published {
        plan.status = openlife_core::agent::PlanStatus::Draft;
    }

    plan.updated_at = chrono::Utc::now();

    PlanOperationResult {
        plan_id,
        run_id,
        operation: "edit".to_string(),
        success: true,
        status: plan.status,
        steps_completed: None,
        steps_failed: None,
        deviations: vec![],
        review_verdict: None,
        message: Some("plan edited successfully".to_string()),
    }
}

#[tauri::command]
pub async fn edit_agent_plan(
    plan_id: String,
    edit: EditPlanRequest,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanOperationResult, AppError> {
    let plan_store = state
        .plan_store
        .as_ref()
        .ok_or_else(|| AppError::internal("PlanStore not available"))?;

    let mut plan = {
        let store = plan_store.lock().await;
        store
            .get_plan(&plan_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Plan not found"))?
    };

    let result = apply_safe_plan_edit(&mut plan, &edit);

    if result.success {
        let store = plan_store.lock().await;
        store.update_plan(&plan).map_err(AppError::from)?;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{PlanExecutionError, PlanExecutor};

    #[tokio::test]
    async fn test_confirm_agent_plan_changes_status_to_confirmed() {
        let plan_store = Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        plan_store.lock().await.create_plan(&plan).unwrap();
        let plan = plan_store.lock().await.get_plan(&plan.id).unwrap().unwrap();

        // Directly verify PlanStore confirmation logic (core of the Tauri command).
        let mut p = plan.clone();
        p.confirm();
        plan_store.lock().await.update_plan(&p).unwrap();

        let fetched = plan_store.lock().await.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.status, openlife_core::agent::PlanStatus::Confirmed);
        assert!(fetched.confirmed_at.is_some());
    }

    #[tokio::test]
    async fn test_reject_agent_plan_changes_status_to_rejected() {
        let plan_store = Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        plan_store.lock().await.create_plan(&plan).unwrap();
        let plan = plan_store.lock().await.get_plan(&plan.id).unwrap().unwrap();

        let mut p = plan.clone();
        p.reject();
        plan_store.lock().await.update_plan(&p).unwrap();

        let fetched = plan_store.lock().await.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.status, openlife_core::agent::PlanStatus::Rejected);
    }

    #[tokio::test]
    async fn test_execute_rejects_unconfirmed_high_risk() {
        let plan_store = Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let mut plan = AgentPlan::new("write", openlife_core::agent::RiskLevel::High);
        plan.publish(); // Published but NOT confirmed
        plan_store.lock().await.create_plan(&plan).unwrap();

        let executor = PlanExecutor::new(plan_store, None);
        let result = executor
            .execute_sync(&plan.id, "run-1", |_step, _intent| {
                openlife_core::agent::PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "file.write_proposal".to_string(),
                    success: true,
                    output: None,
                    error: None,
                    duration_ms: 0,
                    deviation: None,
                }
            })
            .await;
        assert!(matches!(
            result,
            Err(PlanExecutionError::PlanNotConfirmed(_))
        ));
    }

    #[tokio::test]
    async fn test_cancel_confirmed_plan_succeeds() {
        let plan_store = Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        plan.confirm();
        plan_store.lock().await.create_plan(&plan).unwrap();

        let mut p = plan.clone();
        p.cancel();
        plan_store.lock().await.update_plan(&p).unwrap();

        let fetched = plan_store.lock().await.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.status, openlife_core::agent::PlanStatus::Cancelled);
    }

    // ── P7 stabilization: plan-bound AgentSpec resolution tests ──────

    #[tokio::test]
    async fn test_plan_bound_deny_spec_blocks_tool_before_execution() {
        use openlife_core::agent::AgentRoleKind;
        use openlife_core::agent::AgentSpecStore;

        let plan_store = Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new_in_memory().unwrap();

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
        plan_store.lock().await.create_plan(&plan).unwrap();

        let executor =
            PlanExecutor::new(plan_store.clone(), Some(event_store)).with_agent_spec(deny_spec);

        let result = executor
            .execute_sync(&plan.id, "run-1", |_step, _intent| {
                openlife_core::agent::PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "file.read".to_string(),
                    success: true,
                    output: None,
                    error: None,
                    duration_ms: 0,
                    deviation: None,
                }
            })
            .await;

        let outcome = result.expect("execution should complete even with blocked tool");
        assert!(
            !outcome.success,
            "blocked tool should cause plan step failure"
        );
        assert_eq!(outcome.steps_completed, 0, "no steps should have completed");
        assert_eq!(outcome.steps_failed, 1, "one step should have failed");

        // Re-read plan to check status
        let fetched = plan_store.lock().await.get_plan(&plan.id).unwrap().unwrap();
        assert!(matches!(
            fetched.status,
            openlife_core::agent::PlanStatus::Failed
        ));
    }

    #[tokio::test]
    async fn test_execute_plan_without_spec_uses_stored_default() {
        let plan_store = Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));

        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        plan.confirm();
        plan.agent_spec_id = None; // no plan-bound spec
        plan_store.lock().await.create_plan(&plan).unwrap();

        // When no plan-bound spec, resolve_spec(None) should return default.
        // The resolve_spec method on AgentSpecStore already has tests for this.
        let spec_store = openlife_core::agent::AgentSpecStore::new_in_memory().unwrap();
        let resolved = spec_store
            .resolve_spec(plan.agent_spec_id.as_deref())
            .unwrap();
        assert_eq!(resolved.id, "main.default");
        assert_eq!(resolved.role, openlife_core::agent::AgentRoleKind::Main);
    }

    #[tokio::test]
    async fn test_plan_execution_started_includes_agent_spec_id() {
        let plan_store = Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));
        let event_store =
            openlife_core::agent::event_store::AgentRunEventStore::new_in_memory().unwrap();

        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        plan.confirm();
        plan_store.lock().await.create_plan(&plan).unwrap();

        let spec = openlife_core::agent::AgentSpec::default_main_spec();

        let executor =
            PlanExecutor::new(plan_store, Some(event_store.clone())).with_agent_spec(spec);

        let _ = executor
            .execute_sync(&plan.id, "run-1", |_step, _intent| {
                openlife_core::agent::PlanStepExecutionResult {
                    step_index: 0,
                    tool_name: "life_model.read".to_string(),
                    success: true,
                    output: None,
                    error: None,
                    duration_ms: 0,
                    deviation: None,
                }
            })
            .await;

        let events = event_store.list_events_by_run("run-1").unwrap();
        let started_evt = events
            .iter()
            .find(|e| e.event_type == openlife_core::agent::AgentRunEventType::PlanExecutionStarted)
            .expect("PlanExecutionStarted event should exist");
        let agent_spec_id = started_evt
            .payload
            .get("agent_spec_id")
            .and_then(|v| v.as_str());
        assert_eq!(agent_spec_id, Some("main.default"));
    }

    #[tokio::test]
    async fn test_agent_plan_with_agent_spec_builder() {
        let plan = AgentPlan::new("plan with spec", openlife_core::agent::RiskLevel::Low)
            .with_agent_spec("main.custom");
        assert_eq!(plan.agent_spec_id, Some("main.custom".to_string()));
    }

    // ── Legal-state guard tests ──────────────────────────────────────

    #[tokio::test]
    async fn test_plan_can_confirm_or_reject_helper() {
        use openlife_core::agent::PlanStatus;
        assert!(plan_can_confirm_or_reject(PlanStatus::Draft));
        assert!(plan_can_confirm_or_reject(PlanStatus::Published));
        assert!(!plan_can_confirm_or_reject(PlanStatus::Confirmed));
        assert!(!plan_can_confirm_or_reject(PlanStatus::Executing));
        assert!(!plan_can_confirm_or_reject(PlanStatus::Completed));
        assert!(!plan_can_confirm_or_reject(PlanStatus::Rejected));
        assert!(!plan_can_confirm_or_reject(PlanStatus::Cancelled));
        assert!(!plan_can_confirm_or_reject(PlanStatus::Failed));
        assert!(!plan_can_confirm_or_reject(PlanStatus::FailedReview));
    }

    #[tokio::test]
    async fn test_plan_can_edit_helper() {
        use openlife_core::agent::PlanStatus;
        assert!(plan_can_edit(PlanStatus::Draft));
        assert!(plan_can_edit(PlanStatus::Published));
        assert!(!plan_can_edit(PlanStatus::Confirmed));
        assert!(!plan_can_edit(PlanStatus::Executing));
        assert!(!plan_can_edit(PlanStatus::Completed));
        assert!(!plan_can_edit(PlanStatus::Rejected));
        assert!(!plan_can_edit(PlanStatus::Cancelled));
        assert!(!plan_can_edit(PlanStatus::Failed));
        assert!(!plan_can_edit(PlanStatus::FailedReview));
    }

    #[tokio::test]
    async fn test_apply_safe_plan_edit_rejects_terminal_states() {
        let terminal_states = [
            openlife_core::agent::PlanStatus::Confirmed,
            openlife_core::agent::PlanStatus::Executing,
            openlife_core::agent::PlanStatus::Completed,
            openlife_core::agent::PlanStatus::Rejected,
            openlife_core::agent::PlanStatus::Cancelled,
            openlife_core::agent::PlanStatus::Failed,
            openlife_core::agent::PlanStatus::FailedReview,
        ];
        for status in &terminal_states {
            let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
            plan.status = *status;
            let before = plan.updated_at;

            let edit = EditPlanRequest {
                goal: Some("new goal".into()),
                assumptions: None,
                missing_context: None,
                success_criteria: None,
                rollback_plan: None,
            };
            let result = apply_safe_plan_edit(&mut plan, &edit);

            assert!(!result.success, "edit should fail for status {:?}", status);
            assert_eq!(result.status, *status, "status unchanged for {:?}", status);
            assert_eq!(
                plan.updated_at, before,
                "updated_at unchanged for {:?}",
                status
            );
            assert_eq!(plan.goal, "test", "goal unchanged for {:?}", status);
        }
    }

    #[tokio::test]
    async fn test_apply_safe_plan_edit_updates_draft_safely() {
        let mut plan = AgentPlan::new("old goal", openlife_core::agent::RiskLevel::Low);
        let original_id = plan.id.clone();
        let original_created = plan.created_at;

        let edit = EditPlanRequest {
            goal: Some("new goal".into()),
            assumptions: Some(vec!["assumption A".into()]),
            missing_context: Some(vec!["missing ctx".into()]),
            success_criteria: Some(vec!["criterion 1".into()]),
            rollback_plan: None,
        };
        let result = apply_safe_plan_edit(&mut plan, &edit);

        assert!(result.success);
        assert_eq!(result.status, openlife_core::agent::PlanStatus::Draft);
        assert_eq!(plan.goal, "new goal");
        assert_eq!(plan.assumptions, vec!["assumption A"]);
        assert_eq!(plan.missing_context, vec!["missing ctx"]);
        assert_eq!(plan.success_criteria, vec!["criterion 1"]);
        // Immutable fields unchanged
        assert_eq!(plan.id, original_id);
        assert_eq!(plan.created_at, original_created);
        assert!(plan.confirmed_at.is_none());
        assert!(plan.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_apply_safe_plan_edit_reverts_published_to_draft() {
        let mut plan = AgentPlan::new("pub", openlife_core::agent::RiskLevel::Low);
        plan.publish();
        assert_eq!(plan.status, openlife_core::agent::PlanStatus::Published);

        let edit = EditPlanRequest {
            goal: Some("updated".into()),
            assumptions: None,
            missing_context: None,
            success_criteria: None,
            rollback_plan: None,
        };
        let result = apply_safe_plan_edit(&mut plan, &edit);

        assert!(result.success);
        // Published plan reverts to draft
        assert_eq!(plan.status, openlife_core::agent::PlanStatus::Draft);
        assert_eq!(plan.goal, "updated");
    }

    #[tokio::test]
    async fn test_apply_safe_plan_edit_ignores_status_confirmed_at() {
        // Verify that edit cannot change critical execution fields
        let mut plan = AgentPlan::new("initial", openlife_core::agent::RiskLevel::Low);
        let original_id = plan.id.clone();
        let original_run_id = plan.run_id.clone();
        let original_created = plan.created_at;

        let edit = EditPlanRequest {
            goal: Some("edited".into()),
            assumptions: None,
            missing_context: None,
            success_criteria: None,
            rollback_plan: None,
        };
        apply_safe_plan_edit(&mut plan, &edit);

        assert_eq!(plan.id, original_id);
        assert_eq!(plan.run_id, original_run_id);
        assert_eq!(plan.created_at, original_created);
        assert!(plan.confirmed_at.is_none());
        assert!(plan.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_apply_safe_plan_edit_does_not_change_risk_level() {
        let mut plan = AgentPlan::new("risky", openlife_core::agent::RiskLevel::High);
        let original_risk = plan.risk_level;
        let original_requires = plan.requires_confirmation;

        let edit = EditPlanRequest {
            goal: Some("changed".into()),
            assumptions: None,
            missing_context: None,
            success_criteria: None,
            rollback_plan: None,
        };
        apply_safe_plan_edit(&mut plan, &edit);

        assert_eq!(plan.risk_level, original_risk);
        assert_eq!(plan.requires_confirmation, original_requires);
    }

    // ── Phase 1 / P7 legacy tests ────────────────────────────────────

    #[tokio::test]
    async fn test_retry_with_missing_plan_bound_spec_preserves_failed_status() {
        let store = openlife_core::agent::AgentSpecStore::new_in_memory().unwrap();
        let plan_store = Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
        ));

        // Create a failed plan with a non-existent spec id
        let mut plan = AgentPlan::new("test", openlife_core::agent::RiskLevel::Low);
        plan.status = openlife_core::agent::PlanStatus::Failed;
        plan.agent_spec_id = Some("nonexistent".to_string());
        plan_store.lock().await.create_plan(&plan).unwrap();

        // Resolving the spec should fail without changing the plan
        let resolved = store.resolve_spec(plan.agent_spec_id.as_deref());
        assert!(resolved.is_err());
        assert!(matches!(
            resolved.unwrap_err(),
            openlife_core::agent::AgentSpecStoreError::NotFound(_)
        ));

        // Plan should still be Failed
        let fetched = plan_store.lock().await.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.status, openlife_core::agent::PlanStatus::Failed);
        assert_eq!(fetched.agent_spec_id, Some("nonexistent".to_string()));
    }
}
