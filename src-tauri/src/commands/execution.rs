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

#[tauri::command]
pub async fn list_plugins(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::plugins::PluginRecord>, AppError> {
    let registry = state.plugin_registry.lock().await;
    Ok(registry.list())
}

#[tauri::command]
pub async fn reload_plugins(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::plugins::PluginRecord>, AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let records = {
        let mut registry = state.plugin_registry.lock().await;
        registry.reload().map_err(AppError::from)?
    };

    // Plugin tools require a configured executor/provider; do not register them to McpRegistry.
    // They remain visible in PluginRegistry for manifest inspection only.
    {
        let mut mcp = state.mcp_registry.lock().await;
        mcp.remove_builtins_by_source(|source| {
            matches!(
                source,
                openlife_core::tool_manifest::ToolSource::Plugin { .. }
            )
        });
    }

    // Remove any pre-remediation plugin entries. PluginRegistry remains the manifest authority,
    // while the product SkillRegistry contains only executable skills.
    {
        let mut skill_reg = state.skill_registry.lock().await;
        skill_reg.remove_by_source_prefix("plugin:");
    }

    Ok(records)
}

#[tauri::command]
pub async fn enable_plugin(
    plugin_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    {
        let mut registry = state.plugin_registry.lock().await;
        registry.enable(&plugin_id, true).map_err(AppError::from)?;
    }

    // Enabling a plugin makes its manifest available for inspection only. It must not create a
    // selectable skill entry until an executable ToolGateway contract exists.
    let mut skill_reg = state.skill_registry.lock().await;
    skill_reg.remove_by_source_prefix(&format!("plugin:{}:", plugin_id));

    Ok(())
}

#[tauri::command]
pub async fn disable_plugin(
    plugin_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    {
        let mut registry = state.plugin_registry.lock().await;
        registry.enable(&plugin_id, false).map_err(AppError::from)?;
    }

    {
        let mut mcp = state.mcp_registry.lock().await;
        mcp.remove_builtins_by_source(|source| {
            matches!(source, openlife_core::tool_manifest::ToolSource::Plugin { plugin_id: ref pid } if pid == &plugin_id)
        });
    }
    {
        let mut skill_reg = state.skill_registry.lock().await;
        skill_reg.remove_by_source_prefix(&format!("plugin:{}:", plugin_id));
    }

    Ok(())
}
