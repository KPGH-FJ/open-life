use crate::errors::AppError;
use crate::AppState;
use std::sync::Arc;
use tauri::State;

#[cfg(test)]
#[path = "../mcp_audit_read_gateway_tests.rs"]
mod mcp_audit_read_gateway_tests;

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
    use super::{register_mcp_server_with_registry, validate_mcp_command};
    use openlife_core::mcp::McpRegistry;
    use openlife_core::tool_manifest::{ToolManifest, ToolSource};
    use std::sync::Arc;

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

    #[tokio::test]
    async fn registration_probe_never_holds_the_shared_registry_guard() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("tools-list-started");
        let script = r#"
import json, pathlib, sys, time
marker = pathlib.Path(sys.argv[1])
for line in sys.stdin:
    message = json.loads(line)
    method = message.get('method')
    if method == 'initialize':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'protocolVersion':'2024-11-05','capabilities':{}}}), flush=True)
    elif method == 'tools/list':
        marker.write_text('started')
        time.sleep(0.5)
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'tools':[{'name':'probe.read','description':'read probe','parameters':{'type':'object'}}]}}), flush=True)
"#;
        let manifest = ToolManifest {
            id: "mcp:probe:probe.read".into(),
            name: "probe.read".into(),
            description: "Read probe".into(),
            parameters: serde_json::json!({"type": "object"}),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: ToolSource::Mcp {
                server_name: "probe".into(),
            },
            capabilities: vec!["read".into()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            idempotency_contract: openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
            tags: vec!["typed_contract".into()],
        };
        let registry = Arc::new(tokio::sync::Mutex::new(McpRegistry::new()));
        let registration_registry = Arc::clone(&registry);
        let marker_arg = marker.to_string_lossy().into_owned();
        let registration = tokio::spawn(async move {
            register_mcp_server_with_registry(
                &registration_registry,
                "probe".into(),
                "python3".into(),
                vec!["-u".into(), "-c".into(), script.into(), marker_arg],
                Default::default(),
                vec![manifest],
            )
            .await
        });

        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !marker.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("fixture reached external tools/list await");

        let guard = tokio::time::timeout(std::time::Duration::from_millis(100), registry.lock())
            .await
            .expect("registry remains available while external MCP probe is pending");
        assert!(guard.list_servers().is_empty());
        drop(guard);

        registration
            .await
            .expect("registration task joined")
            .expect("prepared registration committed");
        assert_eq!(registry.lock().await.list_servers().len(), 1);
    }
}

#[tauri::command]
pub async fn register_mcp_server(
    name: String,
    command: String,
    args: Vec<String>,
    env: Option<std::collections::HashMap<String, String>>,
    manifests: Option<Vec<openlife_core::tool_manifest::ToolManifest>>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    validate_mcp_command(&command)?;
    register_mcp_server_with_registry(
        &state.mcp_registry,
        name,
        command,
        args,
        env.unwrap_or_default(),
        manifests.unwrap_or_default(),
    )
    .await
}

async fn register_mcp_server_with_registry(
    registry: &Arc<tokio::sync::Mutex<openlife_core::mcp::McpRegistry>>,
    name: String,
    command: String,
    args: Vec<String>,
    env: std::collections::HashMap<String, String>,
    manifests: Vec<openlife_core::tool_manifest::ToolManifest>,
) -> Result<(), AppError> {
    let args_ref = args.iter().map(String::as_str).collect::<Vec<_>>();
    let prepared = openlife_core::mcp::McpRegistry::prepare_registration(
        &name, &command, &args_ref, &env, manifests,
    )
    .await
    .map_err(AppError::from)?;
    // The only registry critical section is the synchronous compare-and-commit.
    // Subprocess spawn, handshake, and tool discovery all happened above.
    registry
        .lock()
        .await
        .commit_prepared_registration(prepared)
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
