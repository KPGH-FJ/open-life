use crate::AppState;
use openlife_core::life_model::LifeModel;
use openlife_core::versioning::LifeModelVersion;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn create_snapshot(
    tag: String,
    note: String,
    state: State<'_, Arc<AppState>>,
) -> Result<LifeModelVersion, String> {
    let manager = state.life_model_manager.lock().await;
    let model = manager.load().map_err(|e| e.to_string())?;
    let vm = state.version_manager.lock().await;
    vm.snapshot(&model, &tag, &note).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_snapshots(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<LifeModelVersion>, String> {
    let vm = state.version_manager.lock().await;
    vm.list_versions().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_snapshot(
    version: String,
    state: State<'_, Arc<AppState>>,
) -> Result<LifeModel, String> {
    {
        let manager = state.life_model_manager.lock().await;
        if let Ok(current_model) = manager.load() {
            let vm = state.version_manager.lock().await;
            let _ = vm.snapshot(
                &current_model,
                "auto:pre-restore",
                &format!("回滚到 {} 之前自动备份", version),
            );
        }
    }
    let vm = state.version_manager.lock().await;
    let model = vm.restore(&version).map_err(|e| e.to_string())?;
    let manager = state.life_model_manager.lock().await;
    manager.save(&model).map_err(|e| e.to_string())?;
    Ok(model)
}

#[tauri::command]
pub async fn diff_snapshots(
    v1: String,
    v2: String,
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let vm = state.version_manager.lock().await;
    vm.diff(&v1, &v2).map_err(|e| e.to_string())
}
