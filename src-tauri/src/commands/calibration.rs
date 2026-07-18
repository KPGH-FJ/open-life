use crate::errors::AppError;
use crate::AppState;
use chrono::Datelike;
use openlife_core::agent::{
    AgentProposal, DurableWriteRequest, DurableWriteSource, DurableWriteSubject, ProposalSource,
    ProposalType, ReviewWorkflow, RiskLevel,
};
use openlife_core::evolution::{EvolutionChange, MicroEvolutionEngine};
use std::sync::Arc;
use tauri::State;

/// 评估 calibration change 的风险级别
fn assess_change_risk(change: &EvolutionChange) -> RiskLevel {
    let path = change.dimension.to_lowercase();
    if path.starts_with("identity.") {
        if path.contains("mission") || path.contains("values") || path.contains("philosophy") {
            RiskLevel::High
        } else {
            RiskLevel::Medium
        }
    } else if path.starts_with("goals.") {
        if path.contains("long_term") || path.contains("life_goals") {
            RiskLevel::High
        } else {
            RiskLevel::Medium
        }
    } else if path.starts_with("capabilities.") {
        RiskLevel::Medium
    } else if path.starts_with("state.") {
        RiskLevel::Low
    } else {
        RiskLevel::Medium
    }
}

/// 将 EvolutionChange 转换为 AgentProposal
fn change_to_proposal(
    change: &EvolutionChange,
    source: ProposalSource,
    before_model: &openlife_core::life_model::LifeModel,
) -> Result<AgentProposal, AppError> {
    let risk_level = assess_change_risk(change);
    let proposal_type = if change.dimension.starts_with("goals.") {
        ProposalType::GoalUpdate
    } else if change.dimension.starts_with("state.") {
        ProposalType::StateUpdate
    } else if change.dimension.starts_with("capabilities.") {
        ProposalType::CapabilityUpdate
    } else if change.dimension.starts_with("preferences.") {
        ProposalType::PreferenceUpdate
    } else {
        ProposalType::LifeModelUpdate
    };

    // 提取 before 值
    let before_value = {
        let model_json = serde_json::to_value(before_model).map_err(AppError::from)?;
        let parts: Vec<&str> = change.dimension.split('.').collect();
        let mut current = &model_json;
        for part in parts.iter() {
            current = current
                .get(part)
                .ok_or_else(|| format!("无法提取 before 值：路径 {} 不存在", change.dimension))?;
        }
        // 进一步定位到 target_name
        if !change.target_name.is_empty() {
            current = current.get(&change.target_name).unwrap_or(current);
        }
        current.clone()
    };

    let affected_path = if change.target_name.is_empty() {
        change.dimension.clone()
    } else {
        format!("{}.{}", change.dimension, change.target_name)
    };

    let mut proposal = AgentProposal::new(
        proposal_type,
        &affected_path,
        serde_json::json!({
            "dimension": change.dimension,
            "target_name": change.target_name,
            "new_value": change.new_value,
            "old_value": change.old_value,
            "reason": change.reason,
            "confidence": change.confidence,
        }),
        &format!("Calibration 建议：{}", change.reason),
        change.confidence,
        risk_level,
        source,
    );
    proposal.before = Some(before_value);
    Ok(proposal)
}

#[tauri::command]
pub async fn run_micro_evolution(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    run_micro_evolution_with_state_gated(state.inner()).await
}

#[cfg(test)]
async fn run_micro_evolution_with_state(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    run_micro_evolution_with_state_gated(state).await
}

async fn run_micro_evolution_with_state_gated(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let engine = MicroEvolutionEngine::new(&store);
    let (_, signals) = engine.run_with_signals(&model).map_err(AppError::from)?;
    let signal_summary = signals.summary();
    drop(store);
    drop(manager);
    Err(AppError::permission(format!(
        "run_micro_evolution has been retired as a Calibration legacy direct-write compatibility surface; create reviewable calibration proposals instead. Metadata-safe signal counts: feedback_terms={}, behavior_events={}, inference_items={}.",
        signal_summary.feedback_terms, signal_summary.behavior_events, signal_summary.inference_items
    )))
}

#[tauri::command]
pub async fn generate_calibration_report(
    period_days: u64,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let report = store
        .generate_calibration_report(&model, period_days as i64)
        .map_err(AppError::from)?;
    Ok(serde_json::json!({
        "period_days": report.period_days,
        "feedback_up": report.feedback_up,
        "feedback_down": report.feedback_down,
        "top_liked_patterns": report.top_liked_patterns,
        "top_disliked_patterns": report.top_disliked_patterns,
        "value_changes": report.value_changes,
        "suggested_actions": report.suggested_actions,
        "summary_text": report.summary_text,
    }))
}

#[tauri::command]
pub async fn generate_micro_evolution_changes(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let engine = MicroEvolutionEngine::new(&store);
    let (result, signals) = engine.run_with_signals(&model).map_err(AppError::from)?;
    let signal_summary = signals.summary();
    let mut after_model = model.clone();
    let _ = MicroEvolutionEngine::apply_changes(&mut after_model, &result.changes);

    Ok(serde_json::json!({
        "applied": result.applied,
        "message": result.message,
        "changes": result.changes,
        "before": model.calculate_4d_completion(),
        "after": after_model.calculate_4d_completion(),
        "requires_confirmation": !result.changes.is_empty(),
        "signal_summary": signal_summary,
    }))
}

#[tauri::command]
pub async fn apply_calibration(
    changes: Vec<EvolutionChange>,
    mode: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    apply_calibration_with_state_gated(changes, mode, state.inner()).await
}

#[cfg(test)]
async fn apply_calibration_with_state(
    changes: Vec<EvolutionChange>,
    mode: Option<String>,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    apply_calibration_with_state_gated(changes, mode, state).await
}

async fn apply_calibration_with_state_gated(
    changes: Vec<EvolutionChange>,
    mode: Option<String>,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let mode = mode.as_deref().unwrap_or("proposal");

    if mode != "direct" {
        return calibration_create_proposals_with_state(changes, state).await;
    }

    Err(AppError::permission(format!(
        "apply_calibration(mode=\"direct\") has been retired as a Calibration legacy direct-write compatibility surface; use calibration_create_proposals or apply_calibration(mode=\"proposal\") for {} change(s).",
        changes.len()
    )))
}

#[tauri::command]
pub async fn should_show_calibration(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let now = chrono::Local::now();
    let is_monday = now.weekday() == chrono::Weekday::Mon;
    let is_first_day = now.day() == 1;
    let today = now.format("%Y-%m-%d").to_string();
    let store = state.feedback_store.lock().await;
    let already_weekly = store
        .count_event_today("calibration_prompt_weekly")
        .unwrap_or(1);
    let already_monthly = store
        .count_event_today("calibration_prompt_monthly")
        .unwrap_or(1);
    Ok(serde_json::json!({
        "weekly": is_monday && already_weekly == 0,
        "monthly": is_first_day && already_monthly == 0,
        "today": today,
    }))
}

#[tauri::command]
pub async fn calibration_create_proposals(
    changes: Vec<EvolutionChange>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    calibration_create_proposals_with_state(changes, state.inner()).await
}

async fn calibration_create_proposals_with_state(
    changes: Vec<EvolutionChange>,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let requested_count = changes.len();
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    drop(manager);

    let proposal_store = state
        .proposal_store
        .clone()
        .ok_or_else(|| "Proposal store 不可用".to_string())?;

    // Create the AgentRun only after every non-mutating dependency preflight
    // succeeds, so an unavailable model/proposal store cannot leave a fake
    // Running projection behind.
    let agent_run = openlife_core::agent::AgentRun::new_calibration_run();
    let run_id = agent_run.id.clone();
    crate::terminal_owner_write_gateway::create_agent_run(state, &agent_run)
        .await
        .map_err(|error| AppError::db_with_hint(error, "read_only_degraded"))?;

    let mut created_ids = Vec::new();
    let mut errors = Vec::new();

    for change in &changes {
        match change_to_proposal(change, ProposalSource::CalibrationRun, &model) {
            Ok(mut proposal) => {
                proposal.run_id = Some(run_id.clone());
                proposal.source_detail = Some("evolution".to_string());
                if let Err(e) =
                    crate::life_model_write_gateway::stamp_lifemodel_proposal_base_hash_with_state(
                        state,
                        &mut proposal,
                    )
                    .await
                {
                    errors.push(format!("{}: {}", proposal.affected_path, e));
                    continue;
                }
                let store = proposal_store.lock().await;
                match ReviewWorkflow::new(&store).submit(
                    DurableWriteRequest::from_agent_proposal(
                        DurableWriteSource::Calibration,
                        DurableWriteSubject::from_proposal_type(proposal.proposal_type),
                        proposal.clone(),
                        "Calibration proposal is pending Review Center approval.",
                    )
                    .with_evidence_refs(vec![change.dimension.clone()]),
                ) {
                    Ok(outcome) => created_ids.push(outcome.proposal_id().to_string()),
                    Err(e) => errors.push(format!("{}: {}", proposal.affected_path, e)),
                }
            }
            Err(e) => {
                errors.push(format!("{}: {}", change.dimension, e));
            }
        }
    }

    let result_state = if created_ids.is_empty() && !errors.is_empty() {
        "failed"
    } else if !created_ids.is_empty() {
        if errors.is_empty() {
            "waiting_permission"
        } else {
            "partial_waiting_permission"
        }
    } else {
        "no_op"
    };
    crate::terminal_owner_write_gateway::project_agent_run_from_proposal_staging(
        state,
        &run_id,
        &created_ids,
        crate::terminal_owner_write_gateway::AgentRunProposalStagingReceipt {
            kind: crate::terminal_owner_write_gateway::AgentRunProposalStagingKind::Calibration,
            requested_count,
            failed_count: errors.len(),
        },
    )
    .await
    .map_err(|error| {
        AppError::db(format!(
            "Calibration Proposals were processed, but AgentRun projection is degraded: {error}"
        ))
    })?;
    let warnings = if result_state == "partial_waiting_permission" {
        errors.clone()
    } else {
        Vec::new()
    };

    Ok(serde_json::json!({
        "success": errors.is_empty(),
        "result_state": result_state,
        "requested_count": requested_count,
        "created_count": created_ids.len(),
        "created_ids": created_ids,
        "run_id": run_id,
        "error_count": errors.len(),
        "errors": errors,
        "warnings": warnings,
        "message": format!("已创建 {} 个 Proposal 到 Mailbox", created_ids.len()),
    }))
}

#[tauri::command]
pub async fn mark_calibration_shown(
    period: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let store = state.feedback_store.lock().await;
    let event = format!("calibration_prompt_{}", period);
    store
        .log_event(&event, None, None)
        .map_err(AppError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calibration_test_change() -> EvolutionChange {
        EvolutionChange {
            dimension: "identity.values".into(),
            target_name: "W82_RAW_CALIBRATION_TARGET_SECRET".into(),
            old_value: 5.0,
            new_value: 7.0,
            reason: "W82_RAW_CALIBRATION_REASON_SECRET".into(),
            confidence: 0.8,
            sources: vec![openlife_core::evolution::SignalSource {
                source: "feedback".into(),
                score: 0.8,
                weight: 1.0,
            }],
        }
    }

    fn invalid_calibration_test_change() -> EvolutionChange {
        EvolutionChange {
            dimension: "missing.calibration.dimension".into(),
            target_name: "missing-target".into(),
            old_value: 0.0,
            new_value: 1.0,
            reason: "metadata-safe invalid-path fixture".into(),
            confidence: 0.5,
            sources: Vec::new(),
        }
    }

    async fn stored_calibration_run(
        state: &Arc<AppState>,
        result: &serde_json::Value,
    ) -> openlife_core::agent::AgentRun {
        let run_id = result["run_id"].as_str().expect("Calibration run id");
        state
            .agent_run_store
            .as_ref()
            .expect("AgentRun store")
            .lock()
            .await
            .get_run(run_id)
            .unwrap()
            .expect("Calibration AgentRun")
    }

    async fn seed_calibration_target(state: &Arc<AppState>) {
        let manager = state.life_model_manager.lock().await;
        let mut model = manager.load().unwrap();
        model
            .identity
            .values
            .push(openlife_core::life_model::ValueItem {
                name: "W82_RAW_CALIBRATION_TARGET_SECRET".into(),
                weight: 5,
                description: "W82_RAW_CALIBRATION_DESCRIPTION_SECRET".into(),
            });
        manager.save(&model).unwrap();
    }

    async fn calibration_target_weight(state: &Arc<AppState>) -> u8 {
        let model = state.life_model_manager.lock().await.load().unwrap();
        model
            .identity
            .values
            .iter()
            .find(|value| value.name == "W82_RAW_CALIBRATION_TARGET_SECRET")
            .map(|value| value.weight)
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn calibration_default_mode_creates_proposals_instead_of_direct_apply() {
        let state = crate::test_utils::test_app_state();

        let result = apply_calibration_with_state(vec![calibration_test_change()], None, &state)
            .await
            .unwrap();

        assert_eq!(result["created_count"], 1);
        assert_eq!(result["success"], true);
        assert_eq!(result["result_state"], "waiting_permission");
        let run = stored_calibration_run(&state, &result).await;
        assert_eq!(
            run.status,
            openlife_core::agent::AgentRunStatus::WaitingPermission
        );
        assert!(run.finished_at.is_none());
        assert_eq!(run.generated_proposals.len(), 1);
        let proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].source, ProposalSource::CalibrationRun);

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert!(model.is_effectively_empty());
    }

    #[tokio::test]
    async fn calibration_full_staging_failure_is_failed_not_completed() {
        let state = crate::test_utils::test_app_state();

        let result =
            apply_calibration_with_state(vec![invalid_calibration_test_change()], None, &state)
                .await
                .unwrap();

        assert_eq!(result["success"], false);
        assert_eq!(result["result_state"], "failed");
        assert_eq!(result["requested_count"], 1);
        assert_eq!(result["created_count"], 0);
        assert_eq!(result["error_count"], 1);
        let run = stored_calibration_run(&state, &result).await;
        assert_eq!(run.status, openlife_core::agent::AgentRunStatus::Failed);
        assert!(run.finished_at.is_some());
        assert!(run.generated_proposals.is_empty());
        assert_eq!(
            run.status_updates.last().map(|update| update.step_index),
            Some(1)
        );
        assert_eq!(
            run.status_updates
                .last()
                .and_then(|update| update.tool_call_index),
            Some(1)
        );
    }

    #[tokio::test]
    async fn calibration_partial_staging_waits_for_review_and_reports_warning() {
        let state = crate::test_utils::test_app_state();

        let result = apply_calibration_with_state(
            vec![calibration_test_change(), invalid_calibration_test_change()],
            None,
            &state,
        )
        .await
        .unwrap();

        assert_eq!(result["success"], false);
        assert_eq!(result["result_state"], "partial_waiting_permission");
        assert_eq!(result["requested_count"], 2);
        assert_eq!(result["created_count"], 1);
        assert_eq!(result["error_count"], 1);
        assert_eq!(result["warnings"].as_array().unwrap().len(), 1);
        let run = stored_calibration_run(&state, &result).await;
        assert_eq!(
            run.status,
            openlife_core::agent::AgentRunStatus::WaitingPermission
        );
        assert!(run.finished_at.is_none());
        assert_eq!(run.generated_proposals.len(), 1);
    }

    #[tokio::test]
    async fn calibration_empty_request_is_explicit_completed_no_op() {
        let state = crate::test_utils::test_app_state();

        let result = apply_calibration_with_state(Vec::new(), None, &state)
            .await
            .unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(result["result_state"], "no_op");
        assert_eq!(result["requested_count"], 0);
        assert_eq!(result["created_count"], 0);
        assert_eq!(result["error_count"], 0);
        let run = stored_calibration_run(&state, &result).await;
        assert_eq!(run.status, openlife_core::agent::AgentRunStatus::Completed);
        assert!(run.finished_at.is_some());
        assert!(run.generated_proposals.is_empty());
    }

    #[tokio::test]
    async fn phase4_calibration_created_ids_use_reused_review_workflow_outcome_id() {
        let state = crate::test_utils::test_app_state();

        let first = apply_calibration_with_state(vec![calibration_test_change()], None, &state)
            .await
            .unwrap();
        let reused_id = first["created_ids"][0]
            .as_str()
            .expect("first created id")
            .to_string();

        let second = apply_calibration_with_state(vec![calibration_test_change()], None, &state)
            .await
            .unwrap();
        assert_eq!(second["created_ids"][0].as_str(), Some(reused_id.as_str()));
        let run_id = second["run_id"].as_str().expect("second run id");
        let stored_run = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await
            .get_run(run_id)
            .unwrap()
            .expect("calibration run exists");
        assert_eq!(
            stored_run.generated_proposals,
            vec![reused_id.clone()],
            "Calibration AgentRun must record the authoritative reused proposal id"
        );

        let proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].id, reused_id);
    }

    #[tokio::test]
    async fn w91_apply_calibration_direct_mode_fails_closed_as_retired_surface() {
        let state = crate::test_utils::test_app_state();
        seed_calibration_target(&state).await;

        let err = apply_calibration_with_state(
            vec![calibration_test_change()],
            Some("direct".to_string()),
            &state,
        )
        .await
        .expect_err("calibration legacy direct apply must fail closed by default");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("apply_calibration"));
        assert!(err.message().contains("retired"));
        assert!(err.message().contains("calibration_create_proposals"));

        assert_eq!(calibration_target_weight(&state).await, 5);
        let proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        assert!(proposals.is_empty());
    }

    #[tokio::test]
    async fn w91_apply_calibration_direct_mode_is_retired_and_writes_no_lifemodel() {
        let state = crate::test_utils::test_app_state();
        seed_calibration_target(&state).await;

        let err = apply_calibration_with_state(
            vec![calibration_test_change()],
            Some("direct".to_string()),
            &state,
        )
        .await
        .expect_err("W91 retires Calibration direct mode persistence");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("apply_calibration"));
        assert!(err.message().contains("retired"));
        assert!(err.message().contains("calibration_create_proposals"));

        let response_dump = err.message().to_string();
        for forbidden in [
            "W82_RAW_CALIBRATION_TARGET_SECRET",
            "W82_RAW_CALIBRATION_REASON_SECRET",
            "identity.values.W82_RAW_CALIBRATION_TARGET_SECRET",
        ] {
            assert!(
                !response_dump.contains(forbidden),
                "legacy calibration direct response leaked raw marker {forbidden}"
            );
        }

        assert_eq!(calibration_target_weight(&state).await, 5);
        let proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        assert!(proposals.is_empty());
    }

    #[tokio::test]
    async fn w91_run_micro_evolution_default_fails_closed_after_retirement() {
        let state = crate::test_utils::test_app_state();

        let err = run_micro_evolution_with_state(&state)
            .await
            .expect_err("micro-evolution direct persist must fail closed by default");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("run_micro_evolution"));
        assert!(err.message().contains("retired"));
        assert!(err.message().contains("proposals"));
    }

    #[tokio::test]
    async fn w91_run_micro_evolution_is_retired_metadata_safe_and_writes_no_lifemodel() {
        let state = crate::test_utils::test_app_state();

        {
            let manager = state.life_model_manager.lock().await;
            let mut model = manager.load().unwrap();
            model
                .identity
                .values
                .push(openlife_core::life_model::ValueItem {
                    name: "W82_RAW_EVOLUTION_TARGET_SECRET".into(),
                    weight: 5,
                    description: "W82_RAW_LIFEMODEL_DESCRIPTION_SECRET".into(),
                });
            manager.save(&model).unwrap();
        }
        {
            let store = state.feedback_store.lock().await;
            store
                .save_conversation_inference(
                    Some("w82"),
                    "identity.values",
                    "W82_RAW_EVOLUTION_TARGET_SECRET",
                    0.03,
                    1.0,
                    "W82_RAW_EVOLUTION_REASON_SECRET",
                )
                .unwrap();
        }

        let err = run_micro_evolution_with_state(&state)
            .await
            .expect_err("W91 retires micro-evolution direct persistence");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("run_micro_evolution"));
        assert!(err.message().contains("retired"));
        assert!(err.message().contains("feedback_terms"));

        let response_dump = err.message().to_string();
        for forbidden in [
            "W82_RAW_EVOLUTION_TARGET_SECRET",
            "W82_RAW_EVOLUTION_REASON_SECRET",
            "W82_RAW_LIFEMODEL_DESCRIPTION_SECRET",
            "identity.values:W82_RAW_EVOLUTION_TARGET_SECRET",
        ] {
            assert!(
                !response_dump.contains(forbidden),
                "legacy micro-evolution response leaked raw marker {forbidden}"
            );
        }

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(model.identity.values[0].weight, 5);
    }
}
