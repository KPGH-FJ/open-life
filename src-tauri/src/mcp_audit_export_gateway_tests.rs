use super::export_mcp_audit_logs_with_state;
use crate::mcp_audit_read_contract_test_support::{
    corrupt_ciphertext, sqlite_family_snapshot, CiphertextColumn,
};
use crate::persistence_coordinator::{PersistenceCoordinator, EXPECTED_BOOTSTRAP_STORES};
use openlife_core::mcp_audit::{AuditKeyConfig, AuditKeyMaterial, KeyMode, McpAuditStore};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

fn d065_export_state(database_path: &Path) -> Arc<crate::AppState> {
    d065_export_state_with_store(
        crate::main_chat_eval_state::isolated_mcp_audit_store_for_test(database_path.to_path_buf()),
    )
}

fn d065_export_state_with_store(store: McpAuditStore) -> Arc<crate::AppState> {
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

fn d065_export_keychain_material(epoch: u64, key: [u8; 32]) -> AuditKeyMaterial {
    AuditKeyMaterial {
        config: AuditKeyConfig {
            mode: KeyMode::Keychain,
            salt_b64: None,
            env_var: None,
            key_ref: Some(format!("openlife/mcp-audit/export-test/{epoch}")),
            epoch,
            created_at: "2026-07-13T00:00:00Z".into(),
        },
        key,
    }
}

async fn d065_insert_export_row(state: &Arc<crate::AppState>) {
    state
        .mcp_audit_store
        .lock()
        .await
        .insert_log(
            "d065_export_fixture_tool",
            &serde_json::json!({"private": "not-returned"}),
            "fixture-result",
            true,
            true,
        )
        .expect("insert D065 export row");
}

async fn d065_assert_unavailable_export_gate(store_name: &'static str) {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_export_state(&directory.path().join("mcp_audit.db"));
    d065_insert_export_row(&state).await;
    state.persistence_coordinator.register_unavailable(
        store_name,
        "d065_injected_export_gate_failure",
        "composite audit export authority unavailable",
    );

    let error = export_mcp_audit_logs_with_state(30, &state)
        .await
        .expect_err("audit export must fail at composite trust before confirmation")
        .to_string();

    assert!(
        error.contains("persistence_store_unavailable") || error.contains(store_name),
        "export returned the wrong failure authority for {store_name}: {error}"
    );
}

#[tokio::test]
async fn d065_key_reference_unavailable_export_fails_at_composite_trust_gate() {
    d065_assert_unavailable_export_gate("McpAuditKeyReferenceStore").await;
}

#[tokio::test]
async fn d065_audit_store_unavailable_export_fails_at_composite_trust_gate() {
    d065_assert_unavailable_export_gate("McpAuditStore").await;
}

#[tokio::test]
async fn d065_trusted_empty_and_nonempty_export_is_exact_and_uses_concrete_gateway() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_export_state(&directory.path().join("mcp_audit.db"));
    let gateway = Arc::clone(&state.mcp_audit_read_gateway);
    let database_path = directory.path().join("mcp_audit.db");
    let empty_before = sqlite_family_snapshot(&database_path);

    let empty = export_mcp_audit_logs_with_state(30, &state)
        .await
        .expect("trusted empty audit export");
    assert_eq!(empty.days, 30);
    assert_eq!(empty.entry_count, 0);
    assert!(empty.entries.is_empty());
    assert_eq!(sqlite_family_snapshot(&database_path), empty_before);

    d065_insert_export_row(&state).await;
    let nonempty_before = sqlite_family_snapshot(&database_path);
    let nonempty = export_mcp_audit_logs_with_state(30, &state)
        .await
        .expect("trusted nonempty audit export");
    assert_eq!(nonempty.days, 30);
    assert_eq!(nonempty.entry_count, 1);
    assert_eq!(nonempty.entries.len(), 1);
    assert_eq!(nonempty.entries[0].tool_name, "d065_export_fixture_tool");
    assert!(nonempty.entries[0].success);
    assert!(nonempty.entries[0].pii_found);
    assert_eq!(sqlite_family_snapshot(&database_path), nonempty_before);

    let calls = gateway.call_counts();
    assert_eq!(calls.diagnostics, 0);
    assert_eq!(calls.list, 0);
    assert_eq!(calls.export, 2);
}

#[tokio::test]
async fn d065_verified_canonical_read_only_store_retains_exact_export_capability() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let material = d065_export_keychain_material(67, [0x67; 32]);
    let writable = McpAuditStore::with_key_materials(&database_path, vec![material.clone()])
        .expect("create writable D065 export fixture");
    writable
        .insert_log(
            "d065_read_only_export_fixture",
            &serde_json::json!({"private": "not-returned"}),
            "fixture-result",
            true,
            true,
        )
        .unwrap();
    drop(writable);

    let read_only =
        McpAuditStore::open_read_only_existing_with_key_materials(&database_path, vec![material])
            .expect("open a real canonical read-only export fixture");
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
        "the export control must use a genuinely non-writable store handle"
    );
    let state = d065_export_state_with_store(read_only);
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

    let export = export_mcp_audit_logs_with_state(30, &state)
        .await
        .expect("verified canonical read-only audit export");
    assert_eq!(export.entry_count, 1);
    assert_eq!(export.entries.len(), 1);
    assert_eq!(export.entries[0].tool_name, "d065_read_only_export_fixture");
    assert_eq!(sqlite_family_snapshot(&database_path), before);

    let calls = state.mcp_audit_read_gateway.call_counts();
    assert_eq!(calls.diagnostics, 0);
    assert_eq!(calls.list, 0);
    assert_eq!(calls.export, 1);
}

async fn d065_assert_corrupt_ciphertext_export_fails_without_mutation(column: CiphertextColumn) {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let state = d065_export_state(&database_path);
    d065_insert_export_row(&state).await;
    corrupt_ciphertext(&database_path, column);
    let before = sqlite_family_snapshot(&database_path);

    assert!(
        export_mcp_audit_logs_with_state(30, &state).await.is_err(),
        "corrupt {} must fail shipped audit export",
        column.label()
    );
    assert_eq!(
        sqlite_family_snapshot(&database_path),
        before,
        "failed {} export must not rewrite the canonical SQLite family",
        column.label()
    );
}

#[tokio::test]
async fn d065_corrupt_arguments_ciphertext_export_fails_without_mutation() {
    d065_assert_corrupt_ciphertext_export_fails_without_mutation(CiphertextColumn::Arguments).await;
}

#[tokio::test]
async fn d065_corrupt_result_ciphertext_export_fails_without_mutation() {
    d065_assert_corrupt_ciphertext_export_fails_without_mutation(CiphertextColumn::Result).await;
}

#[tokio::test]
async fn d065_sqlite_query_failure_fails_export_without_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let state = d065_export_state(&database_path);
    let connection = rusqlite::Connection::open(&database_path).unwrap();
    connection.execute("DROP TABLE mcp_log", []).unwrap();
    drop(connection);
    let before = sqlite_family_snapshot(&database_path);

    assert!(
        export_mcp_audit_logs_with_state(30, &state).await.is_err(),
        "a real SQLite query failure must fail shipped audit export"
    );
    assert_eq!(sqlite_family_snapshot(&database_path), before);
}
