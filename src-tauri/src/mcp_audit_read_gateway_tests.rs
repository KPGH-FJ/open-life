use super::{
    __cmd__list_mcp_audit_logs, __tauri_command_name_list_mcp_audit_logs, list_mcp_audit_logs,
};
use crate::persistence_coordinator::{PersistenceCoordinator, EXPECTED_BOOTSTRAP_STORES};
use openlife_core::mcp_audit::McpAuditStore;
use std::sync::Arc;
use tokio::sync::Mutex;

fn d065_state(path: &std::path::Path) -> Arc<crate::AppState> {
    let base = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let mut state = (*base).clone();
    let persistence = Arc::new(PersistenceCoordinator::for_release_bootstrap());
    for store in EXPECTED_BOOTSTRAP_STORES {
        persistence.register_read_write(*store);
    }
    persistence.seal();
    state.persistence_coordinator = persistence;
    state.mcp_audit_store = Arc::new(Mutex::new(
        crate::main_chat_eval_state::isolated_mcp_audit_store_for_test(path.to_path_buf()),
    ));
    Arc::new(state)
}

async fn d065_insert_audit_row(state: &Arc<crate::AppState>) {
    state
        .mcp_audit_store
        .lock()
        .await
        .insert_log(
            "d065_fixture_tool",
            &serde_json::json!({"private": "not-returned"}),
            "fixture-result",
            true,
            true,
        )
        .expect("insert D065 audit row");
}

fn d065_command_context() -> tauri::Context<tauri::test::MockRuntime> {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    let origin = tauri::utils::acl::ExecutionContext::Remote {
        url: "http://tauri.localhost"
            .parse()
            .expect("valid mock IPC origin"),
    };
    context
        .runtime_authority_mut()
        .__allow_command("list_mcp_audit_logs".into(), origin);
    context
}

fn d065_list_audit_logs_command(state: Arc<crate::AppState>) -> Result<serde_json::Value, String> {
    let app = tauri::test::mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![list_mcp_audit_logs])
        .build(d065_command_context())
        .expect("build D065 mock app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build D065 mock webview");
    let response = tauri::test::get_ipc_response(
        &webview,
        tauri::webview::InvokeRequest {
            cmd: "list_mcp_audit_logs".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(serde_json::json!({"limit": 50})),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    response
        .deserialize::<serde_json::Value>()
        .map_err(|error| error.to_string())
}

async fn d065_diagnostics(state: &Arc<crate::AppState>) -> serde_json::Value {
    serde_json::to_value(
        crate::commands::diagnostics::get_system_diagnostics_with_state(state)
            .await
            .expect("collect D065 diagnostics"),
    )
    .expect("serialize D065 diagnostics")
}

fn assert_d065_unknown(diagnostics: &serde_json::Value) {
    assert_eq!(
        diagnostics["mcp_audit_read_status"], "unavailable",
        "audit-read availability must be explicit"
    );
    assert!(
        diagnostics["mcp_recent_audit_count"].is_null(),
        "unknown audit count must not be projected as zero or stale data: {diagnostics:#}"
    );
    assert!(
        diagnostics["mcp_recent_pii_count"].is_null(),
        "unknown PII count must not be projected as zero or stale data: {diagnostics:#}"
    );
}

#[tokio::test]
async fn d065_trusted_empty_and_nonempty_audit_reads_remain_exact() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));

    let empty = d065_diagnostics(&state).await;
    assert_eq!(empty["mcp_recent_audit_count"], 0);
    assert_eq!(empty["mcp_recent_pii_count"], 0);
    assert_eq!(
        d065_list_audit_logs_command(state.clone())
            .expect("trusted empty audit list")
            .as_array()
            .unwrap()
            .len(),
        0
    );

    d065_insert_audit_row(&state).await;
    let nonempty = d065_diagnostics(&state).await;
    assert_eq!(nonempty["mcp_recent_audit_count"], 1);
    assert_eq!(nonempty["mcp_recent_pii_count"], 1);
    assert_eq!(
        d065_list_audit_logs_command(state)
            .expect("trusted nonempty audit list")
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn d065_key_reference_unavailable_projects_unknown_not_data_or_zero() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    d065_insert_audit_row(&state).await;
    state.persistence_coordinator.register_unavailable(
        "McpAuditKeyReferenceStore",
        "d065_injected_key_reference_failure",
        "key reference unavailable",
    );

    assert_d065_unknown(&d065_diagnostics(&state).await);
}

#[tokio::test]
async fn d065_key_reference_unavailable_list_command_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    d065_insert_audit_row(&state).await;
    state.persistence_coordinator.register_unavailable(
        "McpAuditKeyReferenceStore",
        "d065_injected_key_reference_failure",
        "key reference unavailable",
    );

    assert!(
        d065_list_audit_logs_command(state).is_err(),
        "the shipped list command must not bypass composite key-reference trust"
    );
}

#[tokio::test]
async fn d065_audit_store_unavailable_projects_unknown_not_healthy_zero() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    state.persistence_coordinator.register_unavailable(
        "McpAuditStore",
        "d065_injected_audit_store_failure",
        "audit store unavailable",
    );

    assert_d065_unknown(&d065_diagnostics(&state).await);
}

#[tokio::test]
async fn d065_corrupt_ciphertext_is_unknown_and_list_fails_without_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let state = d065_state(&database_path);
    d065_insert_audit_row(&state).await;
    let connection = rusqlite::Connection::open(&database_path).unwrap();
    connection
        .execute(
            "UPDATE mcp_log SET arguments_encrypted = 'invalid-ciphertext' WHERE id = 1",
            [],
        )
        .unwrap();
    drop(connection);
    let before = std::fs::read(&database_path).unwrap();

    assert_d065_unknown(&d065_diagnostics(&state).await);
    assert!(
        d065_list_audit_logs_command(state).is_err(),
        "corrupt ciphertext must not be returned as a successful placeholder row"
    );
    assert_eq!(
        std::fs::read(&database_path).unwrap(),
        before,
        "failed reads must not rewrite or delete canonical audit evidence"
    );
}

#[tokio::test]
async fn d065_verified_canonical_read_only_mode_retains_read_capability() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    d065_insert_audit_row(&state).await;
    state.persistence_coordinator.register_read_only(
        "McpAuditKeyReferenceStore",
        "d065_verified_read_only",
        "fixture read-only authority",
    );
    state.persistence_coordinator.register_read_only(
        "McpAuditStore",
        "d065_verified_read_only",
        "fixture read-only database",
    );

    let diagnostics = d065_diagnostics(&state).await;
    assert_eq!(diagnostics["mcp_recent_audit_count"], 1);
    assert_eq!(
        d065_list_audit_logs_command(state)
            .expect("verified canonical read-only audit list")
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn d065_shipped_audit_read_consumers_do_not_own_raw_store_interpretation() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in ["src/commands/diagnostics.rs", "src/commands/mcp.rs"] {
        let source = std::fs::read_to_string(manifest.join(relative)).unwrap();
        assert!(
            !source.contains("mcp_audit_store.lock()")
                && !source.contains("mcp_audit_store\n")
                && !source.contains("audit.list_logs("),
            "{relative} still interprets the raw audit store instead of one trusted-read gateway"
        );
    }
}

#[test]
fn d065_product_contract_can_distinguish_unknown_from_zero() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let bridge = std::fs::read_to_string(repo.join("frontend/src/tauri.ts")).unwrap();
    let privacy =
        std::fs::read_to_string(repo.join("frontend/src/pages/settings/tabs/PrivacyTab.tsx"))
            .unwrap();

    assert!(
        bridge.contains("mcp_recent_audit_count: number | null")
            && bridge.contains("mcp_recent_pii_count: number | null"),
        "the frontend contract must preserve nullable unknown counts"
    );
    assert!(
        privacy.contains("未知") || privacy.contains("不可用"),
        "the audit UI must render unknown explicitly instead of formatting it as zero"
    );
}

#[test]
fn d065_test_fixture_uses_real_file_backed_audit_store() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let store: McpAuditStore =
        crate::main_chat_eval_state::isolated_mcp_audit_store_for_test(database_path.clone());
    store
        .insert_log(
            "file_backed_control",
            &serde_json::json!({}),
            "control",
            true,
            false,
        )
        .unwrap();
    assert!(database_path.is_file());
    assert_eq!(store.list_logs(10).unwrap().len(), 1);
}
