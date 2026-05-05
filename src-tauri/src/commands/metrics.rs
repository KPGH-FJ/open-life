use crate::errors::AppError;
use openlife_core::agent::{RolloutMetric, RolloutSummary};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_rollout_metrics(
    experiment: String,
    limit: i64,
    offset: i64,
    state: State<'_, Arc<crate::AppState>>,
) -> Result<Vec<RolloutMetric>, AppError> {
    if let Some(ref store_arc) = state.rollout_metrics_store {
        let store = store_arc.lock().await;
        store
            .list_metrics(&experiment, limit, offset)
            .map_err(AppError::from)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn get_rollout_summary(
    experiment: String,
    state: State<'_, Arc<crate::AppState>>,
) -> Result<RolloutSummary, AppError> {
    if let Some(ref store_arc) = state.rollout_metrics_store {
        let store = store_arc.lock().await;
        store.get_summary(&experiment).map_err(AppError::from)
    } else {
        Ok(RolloutSummary {
            total: 0,
            v2_count: 0,
            v1_count: 0,
            success_count: 0,
            v2_avg_duration_ms: None,
            v1_avg_duration_ms: None,
        })
    }
}

#[tauri::command]
pub async fn get_rollout_errors(
    experiment: String,
    limit: i64,
    state: State<'_, Arc<crate::AppState>>,
) -> Result<Vec<RolloutMetric>, AppError> {
    if let Some(ref store_arc) = state.rollout_metrics_store {
        let store = store_arc.lock().await;
        store
            .get_recent_errors(&experiment, limit)
            .map_err(AppError::from)
    } else {
        Ok(vec![])
    }
}
