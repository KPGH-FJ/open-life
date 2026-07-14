use super::{
    __cmd__list_mcp_audit_logs, __tauri_command_name_list_mcp_audit_logs, list_mcp_audit_logs,
};
use crate::mcp_audit_read_contract_test_support::{
    assert_d065_composite_read_owners, assert_d065_effects_blocked_independently,
    assert_d065_store_mode, corrupt_ciphertext, sqlite_family_snapshot, CiphertextColumn,
    D065_AUDIT_KEY_REFERENCE_STORE, D065_AUDIT_STORE, D065_UNRELATED_STORE,
};
use crate::persistence_coordinator::{
    PersistenceCoordinator, PersistenceStoreMode, EXPECTED_BOOTSTRAP_STORES,
};
use openlife_core::mcp_audit::{
    AuditKeyConfig, AuditKeyMaterial, KeyMode, McpAuditExportDays, McpAuditStore,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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

fn d065_list_audit_logs_command_with_limit(
    state: Arc<crate::AppState>,
    limit: usize,
) -> Result<serde_json::Value, String> {
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
            body: tauri::ipc::InvokeBody::Json(serde_json::json!({"limit": limit})),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    response
        .deserialize::<serde_json::Value>()
        .map_err(|error| error.to_string())
}

fn d065_list_audit_logs_command(state: Arc<crate::AppState>) -> Result<serde_json::Value, String> {
    d065_list_audit_logs_command_with_limit(state, 50)
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
    let projection = &diagnostics["mcp_audit_read"];
    assert_eq!(
        projection["status"], expected_status,
        "audit-read availability must distinguish unavailable substrate from an unknown read result"
    );
    assert!(
        projection.get("recentAuditCount").is_none(),
        "failed audit projection must not contain a count, zero, or stale data: {diagnostics:#}"
    );
    assert!(
        projection.get("recentPiiCount").is_none(),
        "failed audit projection must not contain a PII count, zero, or stale data: {diagnostics:#}"
    );
    assert!(projection["reasonCode"].is_string());
}

fn assert_d065_failed_list_projection(
    projection: &serde_json::Value,
    expected_status: &str,
    expected_reason: &str,
) {
    assert_eq!(projection["status"], expected_status);
    assert_eq!(projection["reasonCode"], expected_reason);
    assert!(
        projection.get("entries").is_none(),
        "failed list projection must make entries structurally unavailable: {projection:#}"
    );
}

async fn assert_d065_exact_one_row_read_surfaces(
    state: &Arc<crate::AppState>,
    expected_status: &str,
    expected_reason: Option<&str>,
    expected_tool_name: &str,
) {
    let list = d065_list_audit_logs_command(Arc::clone(state))
        .expect("independently trusted D065 audit-list command");
    assert_eq!(list["status"], expected_status);
    match expected_reason {
        Some(reason) => assert_eq!(list["reasonCode"], reason),
        None => assert!(list.get("reasonCode").is_none()),
    }
    let rows = list["entries"]
        .as_array()
        .expect("D065 successful audit-list projection carries entries");
    assert_eq!(rows.len(), 1, "audit list must retain the exact row");
    assert_eq!(
        rows[0]["tool_name"], expected_tool_name,
        "audit list must retain exact row identity"
    );

    let diagnostics = d065_diagnostics(state).await;
    let projection = &diagnostics["mcp_audit_read"];
    assert_eq!(
        projection["status"], expected_status,
        "audit status must be derived only from the two composite audit owners"
    );
    match expected_reason {
        Some(reason) => assert_eq!(projection["reasonCode"], reason),
        None => assert!(projection.get("reasonCode").is_none()),
    }
    assert_eq!(projection["recentAuditCount"], 1);
    assert_eq!(projection["recentPiiCount"], 1);
}

#[tokio::test]
async fn d065_unrelated_read_only_store_does_not_degrade_exact_audit_reads() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    d065_insert_audit_row(&state).await;
    state.persistence_coordinator.register_read_only(
        D065_UNRELATED_STORE,
        "d065_unrelated_read_only",
        "unrelated canonical store is read-only",
    );

    assert_d065_composite_read_owners(
        &state.persistence_coordinator,
        PersistenceStoreMode::ReadWriteCanonical,
        PersistenceStoreMode::ReadWriteCanonical,
    );
    assert_d065_store_mode(
        &state.persistence_coordinator,
        D065_UNRELATED_STORE,
        PersistenceStoreMode::ReadOnlyCanonical,
    );
    assert_d065_effects_blocked_independently(&state.persistence_coordinator);
    assert_d065_exact_one_row_read_surfaces(&state, "available", None, "d065_fixture_tool").await;
}

#[tokio::test]
async fn d065_unrelated_unavailable_store_does_not_hide_exact_audit_reads() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    d065_insert_audit_row(&state).await;
    state.persistence_coordinator.register_unavailable(
        D065_UNRELATED_STORE,
        "d065_unrelated_unavailable",
        "unrelated canonical store is unavailable",
    );

    assert_d065_composite_read_owners(
        &state.persistence_coordinator,
        PersistenceStoreMode::ReadWriteCanonical,
        PersistenceStoreMode::ReadWriteCanonical,
    );
    assert_d065_store_mode(
        &state.persistence_coordinator,
        D065_UNRELATED_STORE,
        PersistenceStoreMode::Unavailable,
    );
    assert_d065_effects_blocked_independently(&state.persistence_coordinator);
    assert_d065_exact_one_row_read_surfaces(&state, "available", None, "d065_fixture_tool").await;
}

#[tokio::test]
async fn d065_read_only_key_reference_with_writable_audit_store_is_degraded_but_readable() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    d065_insert_audit_row(&state).await;
    state.persistence_coordinator.register_read_only(
        D065_AUDIT_KEY_REFERENCE_STORE,
        "d065_key_reference_read_only",
        "key-reference owner is canonical read-only",
    );

    assert_d065_composite_read_owners(
        &state.persistence_coordinator,
        PersistenceStoreMode::ReadOnlyCanonical,
        PersistenceStoreMode::ReadWriteCanonical,
    );
    assert_d065_effects_blocked_independently(&state.persistence_coordinator);
    assert_d065_exact_one_row_read_surfaces(
        &state,
        "degraded",
        Some("key_reference_store_read_only"),
        "d065_fixture_tool",
    )
    .await;
}

#[tokio::test]
async fn d065_read_only_audit_store_with_writable_key_reference_is_degraded_but_readable() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let material = d065_keychain_material(68, [0x68; 32]);
    let writable = McpAuditStore::with_key_materials(&database_path, vec![material.clone()])
        .expect("create writable D065 one-sided read-only fixture");
    writable
        .insert_log(
            "d065_audit_only_read_only_fixture",
            &serde_json::json!({"private": "not-returned"}),
            "fixture-result",
            true,
            true,
        )
        .expect("insert D065 one-sided read-only fixture row");
    drop(writable);
    let read_only =
        McpAuditStore::open_read_only_existing_with_key_materials(&database_path, vec![material])
            .expect("open genuine D065 one-sided canonical read-only audit store");
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
        "the one-sided audit-store fixture must be genuinely non-writable"
    );
    let state = d065_state_with_store(read_only);
    state.persistence_coordinator.register_read_only(
        D065_AUDIT_STORE,
        "d065_audit_store_read_only",
        "audit database owner is canonical read-only",
    );
    let before = sqlite_family_snapshot(&database_path);

    assert_d065_composite_read_owners(
        &state.persistence_coordinator,
        PersistenceStoreMode::ReadWriteCanonical,
        PersistenceStoreMode::ReadOnlyCanonical,
    );
    assert_d065_effects_blocked_independently(&state.persistence_coordinator);
    assert_d065_exact_one_row_read_surfaces(
        &state,
        "degraded",
        Some("audit_store_read_only"),
        "d065_audit_only_read_only_fixture",
    )
    .await;
    assert_eq!(sqlite_family_snapshot(&database_path), before);
}

#[tokio::test]
async fn d065_trusted_empty_and_nonempty_diagnostics_are_available_and_exact() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));

    let empty = d065_diagnostics(&state).await;
    assert_eq!(empty["mcp_audit_read"]["status"], "available");
    assert_eq!(empty["mcp_audit_read"]["recentAuditCount"], 0);
    assert_eq!(empty["mcp_audit_read"]["recentPiiCount"], 0);

    d065_insert_audit_row(&state).await;
    let nonempty = d065_diagnostics(&state).await;
    assert_eq!(nonempty["mcp_audit_read"]["status"], "available");
    assert_eq!(nonempty["mcp_audit_read"]["recentAuditCount"], 1);
    assert_eq!(nonempty["mcp_audit_read"]["recentPiiCount"], 1);
}

#[tokio::test]
async fn d065_trusted_empty_and_nonempty_list_command_remains_exact() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));

    assert_eq!(
        d065_list_audit_logs_command(state.clone())
            .expect("trusted empty audit list")
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .unwrap()
            .len(),
        0
    );
    d065_insert_audit_row(&state).await;
    assert_eq!(
        d065_list_audit_logs_command(state)
            .expect("trusted nonempty audit list")
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn d065_webview_list_limit_rejects_zero_and_over_ceiling_without_reading() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    let gateway = Arc::clone(&state.mcp_audit_read_gateway);

    for limit in [0, 201] {
        let error = d065_list_audit_logs_command_with_limit(Arc::clone(&state), limit)
            .expect_err("invalid list limit must fail before the gateway reads audit rows");
        assert!(
            error.contains("mcp_audit_list_limit_out_of_range"),
            "invalid limit {limit} must preserve the typed reason: {error}"
        );
    }
    assert_eq!(gateway.call_counts().list, 0);
}

#[tokio::test]
async fn d065_blocking_worker_join_error_fails_closed_without_poisoning_runtime_or_store() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    d065_insert_audit_row(&state).await;

    let projection = state
        .mcp_audit_read_gateway
        .worker_panic_projection_for_test(&state)
        .await;
    assert!(matches!(
        projection,
        crate::McpAuditReadProjection::Unknown {
            reason_code: crate::McpAuditReadReasonCode::AuditReadFailed
        }
    ));

    tokio::task::yield_now().await;
    let control = d065_list_audit_logs_command(state)
        .expect("runtime and canonical store remain readable after worker JoinError");
    assert_eq!(control["status"], "available");
    assert_eq!(control["entries"].as_array().map(Vec::len), Some(1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn d065_concurrent_reads_queue_before_spawn_and_keep_async_runtime_live() {
    const READS: usize = 12;
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    d065_insert_audit_row(&state).await;

    let heartbeat_done = Arc::new(AtomicBool::new(false));
    let heartbeat_ticks = Arc::new(AtomicUsize::new(0));
    let heartbeat = {
        let heartbeat_done = Arc::clone(&heartbeat_done);
        let heartbeat_ticks = Arc::clone(&heartbeat_ticks);
        tokio::spawn(async move {
            while !heartbeat_done.load(Ordering::SeqCst) {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                heartbeat_ticks.fetch_add(1, Ordering::SeqCst);
            }
        })
    };

    let mut reads = tokio::task::JoinSet::new();
    for _ in 0..READS {
        let state = Arc::clone(&state);
        reads.spawn(async move {
            state
                .mcp_audit_read_gateway
                .projection_with_operation_for_test(&state, |audit| {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    audit.list_logs(1)
                })
                .await
        });
    }
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(result) = reads.join_next().await {
            assert!(matches!(
                result.expect("bounded audit read task"),
                crate::McpAuditReadProjection::Available { .. }
            ));
        }
    })
    .await
    .expect("bounded audit reads must complete without deadlock");

    heartbeat_done.store(true, Ordering::SeqCst);
    heartbeat.await.expect("async heartbeat task");
    assert!(
        heartbeat_ticks.load(Ordering::SeqCst) >= 5,
        "blocking SQLite/decrypt work must not stall the async runtime"
    );
    let stats = state.mcp_audit_read_gateway.blocking_worker_stats();
    assert_eq!(stats.started, READS);
    assert_eq!(stats.peak, 1, "only one blocking worker may enter at once");
    assert_eq!(stats.active, 0);
    assert_eq!(stats.available_permits, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn d065_cancelled_caller_cannot_release_an_inflight_worker_permit_early() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    d065_insert_audit_row(&state).await;
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();

    let first = {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            state
                .mcp_audit_read_gateway
                .projection_with_operation_for_test(&state, move |audit| {
                    let _ = started_tx.send(());
                    release_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .map_err(|_| anyhow::anyhow!("d065_controlled_worker_release_dropped"))?;
                    audit.list_logs(1)
                })
                .await
        })
    };
    tokio::time::timeout(std::time::Duration::from_secs(2), started_rx)
        .await
        .expect("first blocking worker must start")
        .expect("first blocking worker start signal");
    first.abort();
    assert!(first
        .await
        .expect_err("caller task is cancelled")
        .is_cancelled());

    let occupied = state.mcp_audit_read_gateway.blocking_worker_stats();
    assert_eq!(occupied.active, 1);
    assert_eq!(occupied.started, 1);
    assert_eq!(occupied.available_permits, 0);

    let second = {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            state
                .mcp_audit_read_gateway
                .projection_with_operation_for_test(&state, |audit| {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    audit.list_logs(1)
                })
                .await
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert_eq!(
        state.mcp_audit_read_gateway.blocking_worker_stats().started,
        1,
        "the cancelled caller must not let a second worker enter while its detached worker runs"
    );

    release_tx
        .send(())
        .expect("release detached blocking worker");
    let second_projection = tokio::time::timeout(std::time::Duration::from_secs(2), second)
        .await
        .expect("queued read completes after the detached worker exits")
        .expect("queued read task");
    assert!(matches!(
        second_projection,
        crate::McpAuditReadProjection::Available { .. }
    ));
    let settled = state.mcp_audit_read_gateway.blocking_worker_stats();
    assert_eq!(settled.started, 2);
    assert_eq!(settled.peak, 1);
    assert_eq!(settled.active, 0);
    assert_eq!(settled.available_permits, 1);
}

#[tokio::test]
async fn d065_closed_blocking_worker_gate_projects_unknown_without_starting_a_worker() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    d065_insert_audit_row(&state).await;
    state
        .mcp_audit_read_gateway
        .close_blocking_worker_gate_for_test();

    let projection = state.mcp_audit_read_gateway.diagnostic_counts(&state).await;
    assert!(matches!(
        projection,
        crate::McpAuditReadProjection::Unknown {
            reason_code: crate::McpAuditReadReasonCode::AuditReadFailed
        }
    ));
    let stats = state.mcp_audit_read_gateway.blocking_worker_stats();
    assert_eq!(stats.started, 0);
    assert_eq!(stats.active, 0);
    assert_eq!(stats.available_permits, 1);
}

#[tokio::test]
async fn d065_all_audit_read_paths_share_one_concrete_gateway_instance() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_state(&directory.path().join("mcp_audit.db"));
    let gateway = Arc::clone(&state.mcp_audit_read_gateway);

    let _ = d065_diagnostics(&state).await;
    let _ = d065_list_audit_logs_command(Arc::clone(&state)).expect("trusted list command");
    let _ = gateway
        .export_logs(
            &state,
            McpAuditExportDays::try_from(30).expect("valid D065 export window"),
        )
        .await
        .expect("trusted export gateway path");

    let calls = gateway.call_counts();
    assert_eq!(calls.diagnostics, 1);
    assert_eq!(calls.list, 1);
    assert_eq!(calls.export, 1);
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

    let projection = d065_list_audit_logs_command(state)
        .expect("typed unavailable projection is a successful IPC response");
    assert_d065_failed_list_projection(
        &projection,
        "unavailable",
        "key_reference_store_unavailable",
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

    let projection = d065_list_audit_logs_command(state)
        .expect("typed unavailable projection is a successful IPC response");
    assert_d065_failed_list_projection(&projection, "unavailable", "audit_store_unavailable");
}

async fn d065_assert_corrupt_ciphertext_projects_unknown_without_mutation(
    column: CiphertextColumn,
) {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let state = d065_state(&database_path);
    d065_insert_audit_row(&state).await;
    corrupt_ciphertext(&database_path, column);
    let before = sqlite_family_snapshot(&database_path);

    assert_d065_untrusted(&d065_diagnostics(&state).await, "unknown");
    assert_eq!(
        sqlite_family_snapshot(&database_path),
        before,
        "failed {} diagnostics must not rewrite the canonical SQLite family",
        column.label()
    );
}

#[tokio::test]
async fn d065_corrupt_arguments_ciphertext_projects_unknown_without_mutation() {
    d065_assert_corrupt_ciphertext_projects_unknown_without_mutation(CiphertextColumn::Arguments)
        .await;
}

#[tokio::test]
async fn d065_corrupt_result_ciphertext_projects_unknown_without_mutation() {
    d065_assert_corrupt_ciphertext_projects_unknown_without_mutation(CiphertextColumn::Result)
        .await;
}

async fn d065_assert_corrupt_ciphertext_list_fails_without_mutation(column: CiphertextColumn) {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let state = d065_state(&database_path);
    d065_insert_audit_row(&state).await;
    corrupt_ciphertext(&database_path, column);
    let before = sqlite_family_snapshot(&database_path);

    let projection = d065_list_audit_logs_command(state)
        .expect("typed unknown projection is a successful IPC response");
    assert_d065_failed_list_projection(&projection, "unknown", "audit_read_failed");
    assert_eq!(
        sqlite_family_snapshot(&database_path),
        before,
        "failed {} list reads must not rewrite or delete the canonical SQLite family",
        column.label()
    );
}

#[tokio::test]
async fn d065_corrupt_arguments_ciphertext_list_fails_without_mutation() {
    d065_assert_corrupt_ciphertext_list_fails_without_mutation(CiphertextColumn::Arguments).await;
}

#[tokio::test]
async fn d065_corrupt_result_ciphertext_list_fails_without_mutation() {
    d065_assert_corrupt_ciphertext_list_fails_without_mutation(CiphertextColumn::Result).await;
}

async fn d065_assert_uncovered_persisted_key_epoch_fails_closed_without_mutation(
    persisted_key_epoch: i64,
) {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let state = d065_state(&database_path);
    d065_insert_audit_row(&state).await;

    let valid_control = d065_list_audit_logs_command(Arc::clone(&state))
        .expect("current-key ciphertext must be readable before key_epoch tampering");
    assert_eq!(valid_control["status"], "available");
    assert_eq!(valid_control["entries"].as_array().unwrap().len(), 1);

    let connection = rusqlite::Connection::open(&database_path).unwrap();
    assert_eq!(
        connection
            .execute("UPDATE mcp_log SET key_epoch = ?1", [persisted_key_epoch],)
            .unwrap(),
        1,
        "the counterfactual must mutate exactly one real SQLite row"
    );
    drop(connection);
    let before = sqlite_family_snapshot(&database_path);

    let list = d065_list_audit_logs_command(Arc::clone(&state))
        .expect("an uncovered persisted key epoch must use the typed unknown projection");
    assert_d065_failed_list_projection(&list, "unknown", "audit_read_failed");
    assert_d065_untrusted(&d065_diagnostics(&state).await, "unknown");
    assert!(
        state
            .mcp_audit_read_gateway
            .export_logs(
                &state,
                McpAuditExportDays::try_from(30).expect("valid D065 export window"),
            )
            .await
            .is_err(),
        "export must share the exact persisted-epoch failure semantics"
    );
    assert_eq!(
        sqlite_family_snapshot(&database_path),
        before,
        "list, diagnostics, and export must not rewrite an uncovered persisted epoch"
    );
}

#[tokio::test]
async fn d065_positive_missing_persisted_key_epoch_fails_closed_without_mutation() {
    d065_assert_uncovered_persisted_key_epoch_fails_closed_without_mutation(999_999).await;
}

#[tokio::test]
async fn d065_negative_persisted_key_epoch_fails_closed_without_mutation() {
    d065_assert_uncovered_persisted_key_epoch_fails_closed_without_mutation(-1).await;
}

#[tokio::test]
async fn d065_sqlite_query_failure_projects_unknown_not_zero() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let state = d065_state(&database_path);
    let connection = rusqlite::Connection::open(&database_path).unwrap();
    connection.execute("DROP TABLE mcp_log", []).unwrap();
    drop(connection);
    let before = sqlite_family_snapshot(&database_path);

    assert_d065_untrusted(&d065_diagnostics(&state).await, "unknown");
    assert_eq!(
        sqlite_family_snapshot(&database_path),
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
    let before = sqlite_family_snapshot(&database_path);

    let projection = d065_list_audit_logs_command(state)
        .expect("typed unknown projection is a successful IPC response");
    assert_d065_failed_list_projection(&projection, "unknown", "audit_read_failed");
    assert_eq!(sqlite_family_snapshot(&database_path), before);
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
    let before = sqlite_family_snapshot(&database_path);

    let diagnostics = d065_diagnostics(&state).await;
    assert_eq!(sqlite_family_snapshot(&database_path), before);
    assert_eq!(diagnostics["mcp_audit_read"]["status"], "degraded");
    assert_eq!(
        diagnostics["mcp_audit_read"]["reasonCode"],
        "both_owners_read_only"
    );
    assert_eq!(diagnostics["mcp_audit_read"]["recentAuditCount"], 1);
    assert_eq!(diagnostics["mcp_audit_read"]["recentPiiCount"], 1);
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
    let before = sqlite_family_snapshot(&database_path);

    assert_eq!(
        d065_list_audit_logs_command(state)
            .expect("verified canonical read-only audit list")
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(sqlite_family_snapshot(&database_path), before);
}

#[test]
fn d065_shipped_audit_read_graph_has_one_concrete_gateway_owner() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let state_source = std::fs::read_to_string(manifest.join("src/state.rs")).unwrap();
    let app_state = state_source
        .split_once("pub struct AppState {")
        .expect("AppState declaration")
        .1
        .split_once("\n}")
        .expect("AppState declaration end")
        .0;
    assert_eq!(
        app_state
            .matches("pub(crate) mcp_audit_read_gateway:")
            .count(),
        1,
        "AppState must own exactly one concrete MCP audit read gateway instance"
    );
    assert_eq!(app_state.matches("McpAuditReadGateway").count(), 1);

    let gateway_source =
        std::fs::read_to_string(manifest.join("src/mcp_audit_read_gateway.rs")).unwrap();
    assert!(
        gateway_source.contains("pub(crate) struct McpAuditReadGateway")
            && gateway_source.contains("async fn diagnostic_counts")
            && gateway_source.contains("async fn list_logs")
            && gateway_source.contains("async fn export_logs"),
        "one concrete gateway type must own all three MCP audit read projections"
    );
    assert_eq!(
        gateway_source.matches(".list_logs(").count(),
        1,
        "the gateway must own exactly one raw audit-store list read shared by list and diagnostics"
    );
    assert_eq!(gateway_source.matches(".export_logs(").count(), 1);
    let settings_source =
        std::fs::read_to_string(manifest.join("src/commands/settings.rs")).unwrap();
    assert!(
        settings_source.contains("async fn export_mcp_audit_logs_with_state")
            && !settings_source.contains("pub(crate) async fn export_mcp_audit_logs_with_state"),
        "the test seam must not expose a crate-wide confirmation-bypass helper"
    );
    assert_eq!(settings_source.matches(".export_logs(").count(), 1);
    let mcp_source = std::fs::read_to_string(manifest.join("src/commands/mcp.rs")).unwrap();
    assert_eq!(mcp_source.matches(".list_logs(").count(), 1);
    let diagnostics_source =
        std::fs::read_to_string(manifest.join("src/commands/diagnostics.rs")).unwrap();
    assert_eq!(diagnostics_source.matches(".diagnostic_counts(").count(), 1);

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
            body.contains(".mcp_audit_read_gateway")
                && !body.contains("mcp_audit_store")
                && !body.contains("store.list_logs(")
                && !body.contains("store.export_logs("),
            "{relative} still owns or bypasses the AppState MCP audit read gateway instance"
        );
    }

    for entry in std::fs::read_dir(manifest.join("src/commands")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains("mcp_audit_store.lock().await.list_logs(")
                && !source.contains("mcp_audit_store.lock().await.export_logs(")
                && !source.contains("store.list_logs(")
                && !source.contains("store.export_logs("),
            "{} contains a parallel raw MCP audit read wrapper outside the concrete gateway",
            path.display()
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

    assert!(diagnostics_contract
        .contains("mcp_audit_read: McpAuditReadProjection<McpAuditDiagnosticFacts>"));
    assert!(!diagnostics_contract.contains("mcp_recent_audit_count"));
    assert!(!diagnostics_contract.contains("mcp_recent_pii_count"));
    assert!(!diagnostics_contract.contains("mcp_audit_read_status"));

    let projection_contract = bridge
        .split_once("export type McpAuditReadProjection<T>")
        .expect("shared frontend audit-read projection")
        .1
        .split_once("export interface McpAuditDiagnosticFacts")
        .expect("shared frontend audit-read projection end")
        .0;
    assert!(
        ["available", "degraded", "unavailable", "unknown"]
            .iter()
            .all(|status| projection_contract.contains(status))
            && projection_contract.contains("reasonCode")
            && !projection_contract.contains("entries"),
        "one shared discriminated projection must own status and failure reason without lending entries to failed variants"
    );
}

#[test]
fn d065_sqlite_family_snapshot_tracks_main_wal_shm_and_journal_with_stable_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut path = database_path.as_os_str().to_os_string();
        path.push(suffix);
        std::fs::write(std::path::PathBuf::from(path), format!("fixture:{suffix}"))
            .expect("write SQLite family control");
    }

    let snapshot = sqlite_family_snapshot(&database_path);
    assert_eq!(snapshot.members.len(), 4);
    assert_eq!(snapshot.files.len(), 4);
    assert!(snapshot.files.iter().all(|file| {
        file.exists
            && file.bytes.is_some()
            && file.len.is_some()
            && file.modified.is_some()
            && file.permissions_read_only.is_some()
    }));
    assert!(snapshot.files.iter().any(|file| file
        .path
        .as_os_str()
        .to_string_lossy()
        .ends_with("-journal")));
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
