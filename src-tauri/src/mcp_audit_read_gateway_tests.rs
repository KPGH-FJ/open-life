use super::{
    __cmd__list_mcp_audit_logs, __tauri_command_name_list_mcp_audit_logs, list_mcp_audit_logs,
};
use crate::persistence_coordinator::{PersistenceCoordinator, EXPECTED_BOOTSTRAP_STORES};
use openlife_core::mcp_audit::{AuditKeyConfig, AuditKeyMaterial, KeyMode, McpAuditStore};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

fn d065_state(path: &std::path::Path) -> Arc<crate::AppState> {
    d065_state_with_store(
        crate::main_chat_eval_state::isolated_mcp_audit_store_for_test(path.to_path_buf()),
    )
}

fn d065_state_with_store(store: McpAuditStore) -> Arc<crate::AppState> {
    let base = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let mut state = (*base).clone();
    let persistence = Arc::new(PersistenceCoordinator::for_release_bootstrap());
    for store in EXPECTED_BOOTSTRAP_STORES {
        persistence.register_read_write(*store);
    }
    persistence.seal();
    state.persistence_coordinator = persistence;
    state.mcp_audit_store = Arc::new(Mutex::new(store));
    Arc::new(state)
}

fn d065_keychain_material(epoch: u64, key: [u8; 32]) -> AuditKeyMaterial {
    AuditKeyMaterial {
        config: AuditKeyConfig {
            mode: KeyMode::Keychain,
            salt_b64: None,
            env_var: None,
            key_ref: Some(format!("openlife/mcp-audit/test/{epoch}")),
            epoch,
            created_at: "2026-07-13T00:00:00Z".into(),
        },
        key,
    }
}

fn d065_artifact_snapshot(database_path: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let path = PathBuf::from(format!("{}{suffix}", database_path.display()));
            let bytes = std::fs::read(&path).ok();
            (path, bytes)
        })
        .collect()
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

fn assert_d065_untrusted(diagnostics: &serde_json::Value, expected_status: &str) {
    assert_eq!(
        diagnostics["mcp_audit_read_status"], expected_status,
        "audit-read availability must distinguish unavailable substrate from an unknown read result"
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
async fn d065_trusted_empty_and_nonempty_diagnostics_are_available_and_exact() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));

    let empty = d065_diagnostics(&state).await;
    assert_eq!(empty["mcp_audit_read_status"], "available");
    assert_eq!(empty["mcp_recent_audit_count"], 0);
    assert_eq!(empty["mcp_recent_pii_count"], 0);

    d065_insert_audit_row(&state).await;
    let nonempty = d065_diagnostics(&state).await;
    assert_eq!(nonempty["mcp_audit_read_status"], "available");
    assert_eq!(nonempty["mcp_recent_audit_count"], 1);
    assert_eq!(nonempty["mcp_recent_pii_count"], 1);
}

#[tokio::test]
async fn d065_trusted_empty_and_nonempty_list_command_remains_exact() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));

    assert_eq!(
        d065_list_audit_logs_command(state.clone())
            .expect("trusted empty audit list")
            .as_array()
            .unwrap()
            .len(),
        0
    );
    d065_insert_audit_row(&state).await;
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

    assert_d065_untrusted(&d065_diagnostics(&state).await, "unavailable");
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

    assert_d065_untrusted(&d065_diagnostics(&state).await, "unavailable");
}

#[tokio::test]
async fn d065_audit_store_unavailable_fails_list_command() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    state.persistence_coordinator.register_unavailable(
        "McpAuditStore",
        "d065_injected_audit_store_failure",
        "audit store unavailable",
    );

    assert!(
        d065_list_audit_logs_command(state).is_err(),
        "an unavailable audit store must fail the shipped list command"
    );
}

#[tokio::test]
async fn d065_corrupt_ciphertext_projects_unknown_without_mutation() {
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
    let before = d065_artifact_snapshot(&database_path);

    assert_d065_untrusted(&d065_diagnostics(&state).await, "unknown");
    assert_eq!(
        d065_artifact_snapshot(&database_path),
        before,
        "failed diagnostics must not rewrite the canonical SQLite family"
    );
}

#[tokio::test]
async fn d065_corrupt_ciphertext_list_fails_without_mutation() {
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
    let before = d065_artifact_snapshot(&database_path);

    assert!(
        d065_list_audit_logs_command(state).is_err(),
        "corrupt ciphertext must not be returned as a successful placeholder row"
    );
    assert_eq!(
        d065_artifact_snapshot(&database_path),
        before,
        "failed list reads must not rewrite or delete the canonical SQLite family"
    );
}

#[tokio::test]
async fn d065_sqlite_query_failure_projects_unknown_not_zero() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let state = d065_state(&database_path);
    let connection = rusqlite::Connection::open(&database_path).unwrap();
    connection.execute("DROP TABLE mcp_log", []).unwrap();
    drop(connection);
    let before = d065_artifact_snapshot(&database_path);

    assert_d065_untrusted(&d065_diagnostics(&state).await, "unknown");
    assert_eq!(
        d065_artifact_snapshot(&database_path),
        before,
        "a failed SQLite diagnostics query must not repair or replace canonical evidence"
    );
}

#[tokio::test]
async fn d065_sqlite_query_failure_fails_list_without_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let state = d065_state(&database_path);
    let connection = rusqlite::Connection::open(&database_path).unwrap();
    connection.execute("DROP TABLE mcp_log", []).unwrap();
    drop(connection);
    let before = d065_artifact_snapshot(&database_path);

    assert!(
        d065_list_audit_logs_command(state).is_err(),
        "a real SQLite query failure must fail the shipped list command"
    );
    assert_eq!(d065_artifact_snapshot(&database_path), before);
}

#[tokio::test]
async fn d065_verified_canonical_read_only_mode_projects_degraded_counts() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let material = d065_keychain_material(65, [0x65; 32]);
    let writable = McpAuditStore::with_key_materials(&database_path, vec![material.clone()])
        .expect("create writable D065 fixture");
    writable
        .insert_log(
            "d065_read_only_fixture",
            &serde_json::json!({"private": "not-returned"}),
            "fixture-result",
            true,
            true,
        )
        .unwrap();
    drop(writable);
    let read_only =
        McpAuditStore::open_read_only_existing_with_key_materials(&database_path, vec![material])
            .expect("open a real canonical read-only D065 fixture");
    assert!(
        read_only
            .insert_log(
                "must_not_write",
                &serde_json::json!({}),
                "must-not-write",
                true,
                false,
            )
            .is_err(),
        "the read-only control must use a genuinely non-writable store handle"
    );
    let state = d065_state_with_store(read_only);
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
    assert_eq!(diagnostics["mcp_audit_read_status"], "degraded");
    assert_eq!(diagnostics["mcp_recent_audit_count"], 1);
}

#[tokio::test]
async fn d065_verified_canonical_read_only_store_retains_list_capability() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let material = d065_keychain_material(66, [0x66; 32]);
    let writable = McpAuditStore::with_key_materials(&database_path, vec![material.clone()])
        .expect("create writable D065 fixture");
    writable
        .insert_log(
            "d065_read_only_list_fixture",
            &serde_json::json!({"private": "not-returned"}),
            "fixture-result",
            true,
            false,
        )
        .unwrap();
    drop(writable);
    let read_only =
        McpAuditStore::open_read_only_existing_with_key_materials(&database_path, vec![material])
            .expect("open a real canonical read-only D065 fixture");
    assert!(
        read_only
            .insert_log(
                "must_not_write",
                &serde_json::json!({}),
                "must-not-write",
                true,
                false,
            )
            .is_err(),
        "the read-only control must use a genuinely non-writable store handle"
    );
    let state = d065_state_with_store(read_only);
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
fn d065_shipped_audit_read_consumers_use_one_gateway_without_raw_store_reads() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases = [
        (
            "src/commands/diagnostics.rs",
            "pub(crate) async fn get_system_diagnostics_with_state",
            "pub async fn check_ollama_status",
        ),
        (
            "src/commands/mcp.rs",
            "pub async fn list_mcp_audit_logs",
            "pub async fn list_tool_manifests",
        ),
        (
            "src/commands/settings.rs",
            "pub async fn export_mcp_audit_logs",
            "pub async fn cleanup_mcp_audit_logs",
        ),
    ];
    for (relative, start, next) in cases {
        let source = std::fs::read_to_string(manifest.join(relative)).unwrap();
        let body = source
            .split_once(start)
            .unwrap_or_else(|| panic!("missing shipped audit-read function in {relative}"))
            .1
            .split_once(next)
            .unwrap_or_else(|| {
                panic!("missing end marker for shipped audit-read function in {relative}")
            })
            .0;
        assert!(
            body.contains("mcp_audit_read_gateway")
                && !body.contains("mcp_audit_store")
                && !body.contains(".list_logs(")
                && !body.contains(".export_logs("),
            "{relative} still owns or bypasses the single typed MCP audit read gateway"
        );
    }
}

#[test]
fn d065_product_contract_can_distinguish_unknown_from_zero() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let bridge = std::fs::read_to_string(repo.join("frontend/src/tauri.ts")).unwrap();
    let diagnostics_contract = bridge
        .split_once("export interface SystemDiagnostics")
        .expect("SystemDiagnostics frontend contract")
        .1
        .split_once("\n}")
        .expect("SystemDiagnostics frontend contract end")
        .0;

    assert!(
        diagnostics_contract.contains("mcp_recent_audit_count: number | null")
            && diagnostics_contract.contains("mcp_recent_pii_count: number | null")
            && diagnostics_contract.contains("mcp_audit_read_status")
            && ["available", "unavailable", "degraded", "unknown"]
                .iter()
                .all(|status| diagnostics_contract.contains(status)),
        "the frontend diagnostics contract must preserve all four typed audit-read states and nullable unknown counts"
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
