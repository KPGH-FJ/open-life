use super::export_mcp_audit_logs_with_state;
use crate::persistence_coordinator::{PersistenceCoordinator, EXPECTED_BOOTSTRAP_STORES};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

fn d065_export_state(database_path: &Path) -> Arc<crate::AppState> {
    let base = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let mut state = (*base).clone();
    let persistence = Arc::new(PersistenceCoordinator::for_release_bootstrap());
    for store in EXPECTED_BOOTSTRAP_STORES {
        persistence.register_read_write(*store);
    }
    persistence.seal();
    state.persistence_coordinator = persistence;
    state.mcp_audit_store = Arc::new(Mutex::new(
        crate::main_chat_eval_state::isolated_mcp_audit_store_for_test(database_path.to_path_buf()),
    ));
    Arc::new(state)
}

fn d065_export_artifact_snapshot(database_path: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let path = PathBuf::from(format!("{}{suffix}", database_path.display()));
            let bytes = std::fs::read(&path).ok();
            (path, bytes)
        })
        .collect()
}

#[tokio::test]
async fn d065_key_reference_unavailable_export_fails_at_composite_trust_gate() {
    let directory = tempfile::tempdir().unwrap();
    let state = d065_export_state(&directory.path().join("mcp_audit.db"));
    state.persistence_coordinator.register_unavailable(
        "McpAuditKeyReferenceStore",
        "d065_injected_key_reference_failure",
        "key reference unavailable",
    );

    let error = export_mcp_audit_logs_with_state(30, &state)
        .await
        .expect_err("audit export must fail before confirmation when key trust is unavailable");
    let error = error.to_string();

    assert!(
        error.contains("persistence_store_unavailable")
            || error.contains("McpAuditKeyReferenceStore"),
        "export returned the wrong failure authority instead of the composite read gate: {error}"
    );
}

#[tokio::test]
async fn d065_sqlite_query_failure_fails_export_without_mutation() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let state = d065_export_state(&database_path);
    let connection = rusqlite::Connection::open(&database_path).unwrap();
    connection.execute("DROP TABLE mcp_log", []).unwrap();
    drop(connection);
    let before = d065_export_artifact_snapshot(&database_path);

    assert!(
        export_mcp_audit_logs_with_state(30, &state).await.is_err(),
        "a real SQLite query failure must fail shipped audit export"
    );
    assert_eq!(d065_export_artifact_snapshot(&database_path), before);
}
