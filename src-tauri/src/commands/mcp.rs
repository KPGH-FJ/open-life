use crate::errors::AppError;
use crate::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn list_mcp_servers(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::mcp::McpServerInfo>, AppError> {
    let reg = state.mcp_registry.lock().await;
    Ok(reg.list_servers())
}

/// Default allowlist for MCP server commands.
/// Only these base commands are permitted to prevent arbitrary execution.
const MCP_COMMAND_ALLOWLIST: &[&str] = &["npx", "node", "python", "python3", "uv", "uvx"];

fn validate_mcp_command(command: &str) -> Result<(), AppError> {
    let has_forbidden_char = command.chars().any(|c| {
        c.is_whitespace()
            || matches!(
                c,
                '/' | '\\' | ';' | '&' | '|' | '$' | '`' | '>' | '<' | '\n' | '\r'
            )
    });
    if has_forbidden_char || !MCP_COMMAND_ALLOWLIST.contains(&command) {
        return Err(AppError::permission(format!(
            "MCP command '{}' is not in the allowlist. Allowed commands: {}",
            command,
            MCP_COMMAND_ALLOWLIST.join(", ")
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_mcp_command;

    #[test]
    fn validate_mcp_command_allows_exact_bare_commands() {
        assert!(validate_mcp_command("npx").is_ok());
        assert!(validate_mcp_command("node").is_ok());
        assert!(validate_mcp_command("python").is_ok());
        assert!(validate_mcp_command("python3").is_ok());
        assert!(validate_mcp_command("uv").is_ok());
        assert!(validate_mcp_command("uvx").is_ok());
    }

    #[test]
    fn validate_mcp_command_rejects_paths_and_shell_syntax() {
        for command in [
            "/tmp/npx",
            "./npx",
            "bin\\npx",
            "npx --foo",
            "npx;rm -rf",
            "npx && echo hi",
            "npx|cat",
            "npx$IFS",
            "npx`whoami`",
            "npx>out",
            "npx\nnode",
        ] {
            assert!(
                validate_mcp_command(command).is_err(),
                "command should be rejected: {command}"
            );
        }
    }
}

#[tauri::command]
pub async fn register_mcp_server(
    name: String,
    command: String,
    args: Vec<String>,
    env: Option<std::collections::HashMap<String, String>>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    validate_mcp_command(&command)?;
    let mut registry = state.mcp_registry.lock().await;
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let env_map = env.unwrap_or_default();
    registry
        .register_with_env(&name, &command, &args_ref, &env_map)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn unregister_mcp_server(
    name: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let mut registry = state.mcp_registry.lock().await;
    registry.unregister(&name).map_err(AppError::from)
}

#[tauri::command]
pub async fn list_mcp_tools(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::mcp::Tool>, AppError> {
    let registry = state.mcp_registry.lock().await;
    Ok(registry.list_all_tools().to_vec())
}

#[tauri::command]
pub async fn list_mcp_templates() -> Result<serde_json::Value, AppError> {
    let content = include_str!("../../resources/mcp_templates.json");
    serde_json::from_str(content).map_err(AppError::from)
}

#[tauri::command]
pub async fn recommend_mcp_manifests(
    top_k: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::tool_manifest::ToolManifest>, AppError> {
    let model = state
        .life_model_manager
        .lock()
        .await
        .load()
        .map_err(AppError::from)?;
    let gaps = model.goal_capability_gap_analysis();
    let registry = state.mcp_registry.lock().await;
    Ok(registry.recommend_manifests(&gaps, top_k))
}

#[tauri::command]
pub async fn list_mcp_audit_logs(
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::mcp_audit::McpLogEntry>, AppError> {
    let store = state.mcp_audit_store.lock().await;
    store.list_logs(limit).map_err(AppError::from)
}

#[tauri::command]
pub async fn list_tool_manifests(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::tool_manifest::ToolManifest>, AppError> {
    let registry = state.mcp_registry.lock().await;
    Ok(registry.list_manifests())
}

#[tauri::command]
pub async fn clear_mcp_audit_logs(
    days: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<usize, AppError> {
    let store = state.mcp_audit_store.lock().await;
    store.clear_old_logs(days).map_err(AppError::from)
}
