use crate::errors::AppError;
use crate::AppState;
use openlife_core::tool_permissions::{
    ToolPermissionDecision, ToolPermissionPolicy, ToolPermissionRecord,
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn list_tool_permissions(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ToolPermissionRecord>, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("ToolPermissionStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let store = state.tool_permission_store.lock().await;
    store.list().map_err(AppError::from)
}

#[tauri::command]
pub async fn grant_tool_permission(
    tool_name: String,
    source: String,
    risk_level: String,
    action_type: String,
    policy: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ToolPermissionRecord, AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let policy = policy
        .parse::<ToolPermissionPolicy>()
        .map_err(AppError::from)?;
    let store = state.tool_permission_store.lock().await;
    store
        .grant(&tool_name, &source, &risk_level, &action_type, policy, None)
        .map_err(|error| {
            state
                .persistence_coordinator
                .register_runtime_durable_failure("ToolPermissionStore", error.to_string());
            AppError::from(error)
        })
}

#[tauri::command]
pub async fn revoke_tool_permission(
    permission_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let store = state.tool_permission_store.lock().await;
    store.revoke(&permission_id).map_err(|error| {
        state
            .persistence_coordinator
            .register_runtime_durable_failure("ToolPermissionStore", error.to_string());
        AppError::from(error)
    })
}

#[tauri::command]
pub async fn check_tool_permission(
    tool_name: String,
    source: String,
    risk_level: String,
    action_type: String,
    capabilities: Vec<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<ToolPermissionDecision, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("ToolPermissionStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let store = state.tool_permission_store.lock().await;
    store
        .check(
            &tool_name,
            &source,
            &risk_level,
            &action_type,
            &capabilities,
        )
        .map_err(AppError::from)
}
