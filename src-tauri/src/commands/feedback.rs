use crate::errors::AppError;
use crate::{persist_life_model, AppState};
use openlife_core::feedback::{AnalyticsSummary, FeedbackEntry, FeedbackType};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn save_feedback(
    session_id: String,
    message_index: i64,
    feedback_type: String,
    content_preview: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let ft = match feedback_type.as_str() {
        "up" => FeedbackType::ThumbsUp,
        _ => FeedbackType::ThumbsDown,
    };
    let entry = FeedbackEntry {
        session_id,
        message_index,
        feedback_type: ft,
        content_preview,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let store = state.feedback_store.lock().await;
    store
        .save_feedback(&entry)
        .map_err(AppError::from)
        .map(|_| ())
}

#[tauri::command]
pub async fn get_feedback_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<AnalyticsSummary, AppError> {
    let store = state.feedback_store.lock().await;
    store.summary().map_err(AppError::from)
}

#[tauri::command]
pub async fn apply_feedback_evolution(state: State<'_, Arc<AppState>>) -> Result<String, AppError> {
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let result = store
        .apply_feedback_to_model(&mut model)
        .map_err(AppError::from)?;
    drop(manager);
    let _ = persist_life_model(&state.inner().clone(), model, true)
        .await
        .map_err(AppError::from)?;
    Ok(result)
}

#[tauri::command]
pub async fn generate_evolution_report(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let manager = state.life_model_manager.lock().await;
    let mut model = manager.load().map_err(AppError::from)?;
    let store = state.feedback_store.lock().await;
    let report = store.generate_evolution_report().map_err(AppError::from)?;
    model.evolution_rules = report.suggested_rules.clone();
    drop(manager);
    let _ = persist_life_model(&state.inner().clone(), model, true)
        .await
        .map_err(AppError::from)?;
    Ok(serde_json::json!({
        "summary": report.summary_text,
        "liked_patterns": report.liked_patterns,
        "disliked_patterns": report.disliked_patterns,
        "applied_rules": report.suggested_rules,
    }))
}

#[tauri::command]
pub async fn log_analytics_event(
    event_name: String,
    session_id: Option<String>,
    detail: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let store = state.feedback_store.lock().await;
    store
        .log_event(&event_name, session_id.as_deref(), detail.as_deref())
        .map_err(AppError::from)
        .map(|_| ())
}
