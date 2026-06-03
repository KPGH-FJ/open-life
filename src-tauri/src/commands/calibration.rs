use crate::errors::AppError;
use crate::legacy_write_convergence::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::{persist_life_model, AppState};
use chrono::Datelike;
use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use openlife_core::evolution::{EvolutionChange, MicroEvolutionEngine};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalibrationLegacyDirectApplyDevMigrationOverride {
    pub allow_calibration_legacy_direct_apply: bool,
    pub purpose: String,
}

impl CalibrationLegacyDirectApplyDevMigrationOverride {
    #[cfg(test)]
    fn allow_for_dev_migration() -> Self {
        Self {
            allow_calibration_legacy_direct_apply: true,
            purpose: "dev_migration".into(),
        }
    }

    fn is_valid_dev_migration_override(&self) -> bool {
        self.allow_calibration_legacy_direct_apply
            && matches!(
                self.purpose.as_str(),
                "dev_migration" | "migration" | "legacy_migration"
            )
    }
}

fn require_calibration_legacy_direct_apply_override(
    dev_migration_override: Option<&CalibrationLegacyDirectApplyDevMigrationOverride>,
    command_name: &str,
) -> Result<(), AppError> {
    if dev_migration_override.is_some_and(
        CalibrationLegacyDirectApplyDevMigrationOverride::is_valid_dev_migration_override,
    ) {
        Ok(())
    } else {
        Err(AppError::permission(format!(
            "{command_name} is a W82 Calibration legacy direct apply path and requires an explicit dev/migration override; use calibration_create_proposals or apply_calibration(mode=\"proposal\") for normal product flow."
        )))
    }
}

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
    dev_migration_override: Option<CalibrationLegacyDirectApplyDevMigrationOverride>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    run_micro_evolution_with_state_gated(state.inner(), dev_migration_override).await
}

#[cfg(test)]
async fn run_micro_evolution_with_state(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    run_micro_evolution_with_state_gated(state, None).await
}

#[cfg(test)]
async fn run_micro_evolution_with_state_for_dev_migration(
    state: &Arc<AppState>,
    dev_migration_override: CalibrationLegacyDirectApplyDevMigrationOverride,
) -> Result<serde_json::Value, AppError> {
    run_micro_evolution_with_state_gated(state, Some(dev_migration_override)).await
}

async fn run_micro_evolution_with_state_gated(
    state: &Arc<AppState>,
    dev_migration_override: Option<CalibrationLegacyDirectApplyDevMigrationOverride>,
) -> Result<serde_json::Value, AppError> {
    require_calibration_legacy_direct_apply_override(
        dev_migration_override.as_ref(),
        "run_micro_evolution",
    )?;
    run_micro_evolution_direct_apply_after_gate(state).await
}

async fn run_micro_evolution_direct_apply_after_gate(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let engine = MicroEvolutionEngine::new(&store);
    let (result, signals) = engine.run_with_signals(&model).map_err(AppError::from)?;
    let signal_summary = signals.summary();
    let mut snapshot_version = None;
    if result.applied {
        let mut new_model = model.clone();
        MicroEvolutionEngine::apply_changes(&mut new_model, &result.changes)
            .map_err(AppError::from)?;
        drop(manager);
        let new_model = persist_life_model(
            state,
            new_model,
            false,
            LifeModelMaterializerCallerContext::new(
                "calibration_micro_evolution_legacy_direct_apply",
                LifeModelMaterializerCallerKind::LegacyDevMigrationOverride,
                LifeModelMaterializerCallerPurpose::DevMigrationOverrideGuardedLegacyBlocker,
            ),
        )
        .await?;
        // auto snapshot after evolution
        let vm = state.version_manager.lock().await;
        if let Ok(snap) = vm.snapshot(&new_model, "auto:evolution", &result.message) {
            snapshot_version = Some(snap.version);
        }
    }
    let message = if result.applied {
        format!(
            "Legacy micro-evolution direct apply completed for {} change(s)",
            result.changes.len()
        )
    } else {
        "Legacy micro-evolution direct apply completed with no durable changes".into()
    };
    Ok(serde_json::json!({
        "success": true,
        "legacy": true,
        "warning": "legacy direct apply path bypasses Review Center; use calibration_create_proposals for product flow",
        "applied": result.applied,
        "change_count": result.changes.len(),
        "message": message,
        "snapshot_version": snapshot_version,
        "signal_counts": {
            "feedback_terms": signal_summary.feedback_terms,
            "behavior_events": signal_summary.behavior_events,
            "inference_items": signal_summary.inference_items,
        },
        "metadata_safe": true,
    }))
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
    let mut agent_run = openlife_core::agent::AgentRun::new_calibration_run();

    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let engine = MicroEvolutionEngine::new(&store);
    let (result, signals) = engine.run_with_signals(&model).map_err(AppError::from)?;
    let signal_summary = signals.summary();
    let mut after_model = model.clone();
    let _ = MicroEvolutionEngine::apply_changes(&mut after_model, &result.changes);

    // Complete AgentRun
    agent_run.output_preview = Some(result.message.clone());
    agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    agent_run.finished_at = Some(chrono::Utc::now());
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

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
    dev_migration_override: Option<CalibrationLegacyDirectApplyDevMigrationOverride>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    apply_calibration_with_state_gated(changes, mode, state.inner(), dev_migration_override).await
}

#[cfg(test)]
async fn apply_calibration_with_state(
    changes: Vec<EvolutionChange>,
    mode: Option<String>,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    apply_calibration_with_state_gated(changes, mode, state, None).await
}

#[cfg(test)]
async fn apply_calibration_with_state_for_dev_migration(
    changes: Vec<EvolutionChange>,
    mode: Option<String>,
    state: &Arc<AppState>,
    dev_migration_override: CalibrationLegacyDirectApplyDevMigrationOverride,
) -> Result<serde_json::Value, AppError> {
    apply_calibration_with_state_gated(changes, mode, state, Some(dev_migration_override)).await
}

async fn apply_calibration_with_state_gated(
    changes: Vec<EvolutionChange>,
    mode: Option<String>,
    state: &Arc<AppState>,
    dev_migration_override: Option<CalibrationLegacyDirectApplyDevMigrationOverride>,
) -> Result<serde_json::Value, AppError> {
    let mode = mode.as_deref().unwrap_or("proposal");

    if mode != "direct" {
        return calibration_create_proposals_with_state(changes, state).await;
    }

    require_calibration_legacy_direct_apply_override(
        dev_migration_override.as_ref(),
        "apply_calibration(mode=\"direct\")",
    )?;
    apply_calibration_direct_apply_after_gate(changes, state).await
}

async fn apply_calibration_direct_apply_after_gate(
    changes: Vec<EvolutionChange>,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let mut agent_run = openlife_core::agent::AgentRun::new_calibration_run();

    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(AppError::from)?;
    MicroEvolutionEngine::apply_changes(&mut model, &changes).map_err(AppError::from)?;
    drop(manager);
    let model = persist_life_model(
        state,
        model,
        false,
        LifeModelMaterializerCallerContext::new(
            "calibration_direct_apply_legacy_direct_apply",
            LifeModelMaterializerCallerKind::LegacyDevMigrationOverride,
            LifeModelMaterializerCallerPurpose::DevMigrationOverrideGuardedLegacyBlocker,
        ),
    )
    .await?;
    let vm = state.version_manager.lock().await;
    let snap = vm
        .snapshot(&model, "auto:calibration", "用户确认并应用校准确认变更")
        .map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let _ = store.log_event(
        "calibration_applied",
        None,
        Some(&format!("applied_changes={}", changes.len())),
    );

    // Complete AgentRun
    agent_run.output_preview = Some(format!("Applied {} calibration changes", changes.len()));
    agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    agent_run.finished_at = Some(chrono::Utc::now());
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    let legacy_warning =
        "legacy direct apply path bypasses Review Center; use calibration_create_proposals for product flow";
    Ok(serde_json::json!({
        "success": true,
        "legacy": true,
        "warning": legacy_warning,
        "snapshot_version": snap.version,
        "applied_count": changes.len(),
        "message": format!("已应用 {} 项校准变更，并创建快照 {}", changes.len(), snap.version),
        "metadata_safe": true,
    }))
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
    // Create AgentRun for this calibration
    let mut agent_run = openlife_core::agent::AgentRun::new_calibration_run();
    let run_id = agent_run.id.clone();
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(AppError::from)?;
    drop(manager);

    let proposal_store_opt = state.proposal_store.clone();
    let store = proposal_store_opt
        .as_ref()
        .ok_or_else(|| "Proposal store 不可用".to_string())?;
    let store = store.lock().await;

    let mut created_ids = Vec::new();
    let mut errors = Vec::new();

    for change in &changes {
        match change_to_proposal(change, ProposalSource::CalibrationRun, &model) {
            Ok(mut proposal) => {
                proposal.run_id = Some(run_id.clone());
                proposal.source_detail = Some("evolution".to_string());
                let id = proposal.id.clone();
                if let Err(e) = store.create_proposal(&proposal) {
                    errors.push(format!("{}: {}", proposal.affected_path, e));
                } else {
                    created_ids.push(id);
                }
            }
            Err(e) => {
                errors.push(format!("{}: {}", change.dimension, e));
            }
        }
    }

    // Update AgentRun with generated proposal IDs and mark as completed
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        for pid in &created_ids {
            let _ = store.add_generated_proposal(&run_id, pid);
        }
        agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
        agent_run.finished_at = Some(chrono::Utc::now());
        let _ = store.update_run(&agent_run);
    }

    Ok(serde_json::json!({
        "success": true,
        "created_count": created_ids.len(),
        "created_ids": created_ids,
        "run_id": run_id,
        "error_count": errors.len(),
        "errors": errors,
        "message": format!("已创建 {} 个 Proposal 到 Review Center", created_ids.len()),
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
    async fn w82_apply_calibration_direct_mode_default_fails_closed_without_dev_migration_override()
    {
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
        assert!(err.message().contains("Calibration"));
        assert!(err.message().contains("dev/migration"));

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
    async fn w82_apply_calibration_direct_mode_only_updates_model_with_dev_migration_override() {
        let state = crate::test_utils::test_app_state();
        seed_calibration_target(&state).await;

        let result = apply_calibration_with_state_for_dev_migration(
            vec![calibration_test_change()],
            Some("direct".to_string()),
            &state,
            CalibrationLegacyDirectApplyDevMigrationOverride::allow_for_dev_migration(),
        )
        .await
        .unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(result["legacy"], true);
        assert_eq!(result["applied_count"], 1);
        assert_eq!(result["metadata_safe"], true);
        assert!(result["warning"]
            .as_str()
            .is_some_and(|warning| warning.contains("Review Center")));
        assert!(result.get("model").is_none());
        assert!(result.get("changes").is_none());
        assert!(result.get("raw_change").is_none());

        let response_dump = result.to_string();
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

        assert_eq!(calibration_target_weight(&state).await, 7);
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
    async fn w82_run_micro_evolution_default_fails_closed_without_dev_migration_override() {
        let state = crate::test_utils::test_app_state();

        let err = run_micro_evolution_with_state(&state)
            .await
            .expect_err("micro-evolution direct persist must fail closed by default");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("run_micro_evolution"));
        assert!(err.message().contains("Calibration"));
        assert!(err.message().contains("dev/migration"));
    }

    #[tokio::test]
    async fn w82_run_micro_evolution_dev_migration_response_is_metadata_safe() {
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

        let result = run_micro_evolution_with_state_for_dev_migration(
            &state,
            CalibrationLegacyDirectApplyDevMigrationOverride::allow_for_dev_migration(),
        )
        .await
        .unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(result["legacy"], true);
        assert_eq!(result["metadata_safe"], true);
        assert_eq!(result["applied"], true);
        assert_eq!(result["change_count"], 1);
        assert!(result.get("changes").is_none());
        assert!(result.get("raw_evolution_payload").is_none());
        assert!(result.get("model").is_none());

        let response_dump = result.to_string();
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
