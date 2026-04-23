use crate::{persist_life_model, AppState};
use chrono::Datelike;
use openlife_core::evolution::{EvolutionChange, MicroEvolutionEngine};
use std::sync::Arc;
use tauri::State;

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
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(|e| e.to_string())?;
    let store = state.feedback_store.lock().await;
    let engine = MicroEvolutionEngine::new(&*store);
    let (result, signals) = engine.run_with_signals(&model).map_err(|e| e.to_string())?;
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
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
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
