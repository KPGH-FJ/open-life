use crate::{persist_life_model, AppState};
use chrono::Datelike;
use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
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
) -> Result<AgentProposal, String> {
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
        let model_json = serde_json::to_value(before_model).map_err(|e| e.to_string())?;
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
) -> Result<serde_json::Value, String> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(|e| e.to_string())?;
    let store = state.feedback_store.lock().await;
    let engine = MicroEvolutionEngine::new(&*store);
    let (result, signals) = engine.run_with_signals(&model).map_err(|e| e.to_string())?;
    let signal_summary = signals.summary();
    let mut snapshot_version = None;
    if result.applied {
        let mut new_model = model.clone();
        MicroEvolutionEngine::apply_changes(&mut new_model, &result.changes)
            .map_err(|e| e.to_string())?;
        drop(manager);
        let new_model = persist_life_model(&state.inner().clone(), new_model, false).await?;
        // auto snapshot after evolution
        let vm = state.version_manager.lock().await;
        if let Ok(snap) = vm.snapshot(&new_model, "auto:evolution", &result.message) {
            snapshot_version = Some(snap.version);
        }
    }
    Ok(serde_json::json!({
        "changes": result.changes,
        "applied": result.applied,
        "message": result.message,
        "snapshot_version": snapshot_version,
        "signal_summary": signal_summary,
    }))
}

#[tauri::command]
pub async fn generate_calibration_report(
    period_days: u64,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(|e| e.to_string())?;
    let store = state.feedback_store.lock().await;
    let report = store
        .generate_calibration_report(&model, period_days as i64)
        .map_err(|e| e.to_string())?;
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
) -> Result<serde_json::Value, String> {
    let mut agent_run = openlife_core::agent::AgentRun::new_calibration_run();
    
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(|e| e.to_string())?;
    let store = state.feedback_store.lock().await;
    let engine = MicroEvolutionEngine::new(&*store);
    let (result, signals) = engine.run_with_signals(&model).map_err(|e| e.to_string())?;
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
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let mode = mode.as_deref().unwrap_or("direct");

    if mode == "proposal" {
        // 创建 Proposal 而不是直接应用
        return calibration_create_proposals(changes, state).await;
    }

    // direct 模式：直接应用变更
    let mut agent_run = openlife_core::agent::AgentRun::new_calibration_run();
    
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(|e| e.to_string())?;
    MicroEvolutionEngine::apply_changes(&mut model, &changes).map_err(|e| e.to_string())?;
    drop(manager);
    let model = persist_life_model(&state.inner().clone(), model, false).await?;
    let vm = state.version_manager.lock().await;
    let snap = vm
        .snapshot(&model, "auto:calibration", "用户确认并应用校准确认变更")
        .map_err(|e| e.to_string())?;
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
    
    Ok(serde_json::json!({
        "success": true,
        "snapshot_version": snap.version,
        "applied_count": changes.len(),
        "message": format!("已应用 {} 项校准变更，并创建快照 {}", changes.len(), snap.version),
    }))
}

#[tauri::command]
pub async fn should_show_calibration(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
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
) -> Result<serde_json::Value, String> {
    // Create AgentRun for this calibration
    let mut agent_run = openlife_core::agent::AgentRun::new_calibration_run();
    let run_id = agent_run.id.clone();
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(|e| e.to_string())?;
    drop(manager);

    let store = state
        .proposal_store
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
) -> Result<(), String> {
    let store = state.feedback_store.lock().await;
    let event = format!("calibration_prompt_{}", period);
    store
        .log_event(&event, None, None)
        .map_err(|e| e.to_string())?;
    Ok(())
}
