use crate::{persist_life_model, AppState};
use openlife_core::life_model::LifeModel;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_life_model(state: State<'_, Arc<AppState>>) -> Result<LifeModel, String> {
    let manager = state.life_model_manager.lock().await;
    manager.load().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_life_model(
    life_model: LifeModel,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    persist_life_model(&state.inner().clone(), life_model, true)
        .await
        .map(|_| ())
}
