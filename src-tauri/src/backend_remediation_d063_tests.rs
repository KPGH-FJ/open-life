use openlife_core::mcp_audit::McpAuditStore;
use rusqlite::params;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

const MCP_AUDIT_RETENTION_MAX_DAYS: i64 = 3_650;

fn function_body<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("missing function marker {marker}"));
    let relative_open = source[start..]
        .find('{')
        .unwrap_or_else(|| panic!("missing function body for {marker}"));
    let body_start = start + relative_open;
    let mut depth = 0usize;
    for (offset, character) in source[body_start..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..body_start + offset + 1];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated function body for {marker}");
}

fn audit_store(path: &Path) -> McpAuditStore {
    McpAuditStore::new(path)
}

fn insert_at(store: &McpAuditStore, path: &Path, tool_name: &str, created_at: &str) {
    store
        .insert_log(
            tool_name,
            &serde_json::json!({"fixture": tool_name}),
            "fixture-result",
            true,
            false,
        )
        .expect("insert D063 audit fixture");
    rusqlite::Connection::open(path)
        .expect("open D063 fixture database")
        .execute(
            "UPDATE mcp_log SET created_at = ?1 WHERE tool_name = ?2",
            params![created_at, tool_name],
        )
        .expect("set deterministic D063 fixture timestamp");
}

fn row_truth(store: &McpAuditStore) -> Vec<(String, String)> {
    let mut rows = store
        .list_logs(100)
        .expect("read D063 audit fixture")
        .into_iter()
        .map(|row| (row.tool_name, row.created_at))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn assert_invalid_retention_is_non_mutating(retention_days: i64) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp-audit.sqlite");
    let store = audit_store(&path);
    insert_at(&store, &path, "d063-old-row", "2020-01-01T00:00:00+00:00");
    insert_at(
        &store,
        &path,
        "d063-recent-row",
        "2090-01-01T00:00:00+00:00",
    );
    let before = row_truth(&store);

    let result = catch_unwind(AssertUnwindSafe(|| store.cleanup(retention_days)));

    assert!(
        result.is_ok(),
        "invalid retention {retention_days} must return a typed error, not panic"
    );
    assert!(
        result.unwrap().is_err(),
        "invalid retention {retention_days} must fail before SQL"
    );
    assert_eq!(row_truth(&store), before, "invalid retention mutated rows");
}

#[test]
fn d063_shipped_backend_exposes_one_governed_cleanup_command() {
    let lib = include_str!("lib.rs");
    let legacy = include_str!("commands/mcp.rs");
    let governed = include_str!("commands/settings.rs");

    assert!(!legacy.contains("pub async fn clear_mcp_audit_logs("));
    assert!(!lib.contains("clear_mcp_audit_logs,"));
    assert_eq!(
        governed
            .matches("pub async fn cleanup_mcp_audit_logs(")
            .count(),
        1
    );
    assert_eq!(
        lib.matches("cleanup_mcp_audit_logs,").count(),
        2,
        "the one governed command must have one import and one shipped handler registration"
    );
}

#[test]
fn d063_frontend_has_no_page_local_cleanup_authority() {
    let bridge = include_str!("../../frontend/src/tauri.ts");
    let page = include_str!("../../frontend/src/pages/McpPage.tsx");

    assert!(!bridge.contains("export async function clearMcpAuditLogs("));
    assert!(!bridge.contains("\"clear_mcp_audit_logs\""));
    assert!(!page.contains("clearMcpAuditLogs"));
    assert!(!page.contains("confirmAuditCleanup"));
    assert!(!page.contains("确认清理 MCP 审计日志"));
    assert!(page.contains("to=\"/settings\""));
    assert!(page.contains("在隐私设置中管理审计保留"));
}

#[test]
fn d063_core_has_one_typed_cleanup_mutation_entry() {
    let source = include_str!("../../openlife-core/src/mcp_audit.rs");

    assert_eq!(source.matches("pub fn cleanup(").count(), 1);
    assert!(!source.contains("pub fn clear_old_logs("));
    assert!(
        source.contains("pub fn cleanup(&self, retention: McpAuditRetentionDays) -> Result<usize>")
    );
    assert_eq!(
        source
            .matches("DELETE FROM mcp_log WHERE created_at < ?1")
            .count(),
        1
    );
}

#[test]
fn d063_governed_command_validates_before_confirmation_and_sql() {
    let source = include_str!("commands/settings.rs");
    let body = function_body(source, "pub async fn cleanup_mcp_audit_logs(");
    let validation = body
        .find("McpAuditRetentionDays::try_from(retention_days)")
        .expect("retention must be typed at the shipped boundary");
    let persistence = body
        .find("require_effects_allowed()")
        .expect("cleanup must retain the persistence effects gate");
    let confirmation = body
        .find("require_danger_action_confirmation(")
        .expect("cleanup must retain Rust-owned native confirmation");
    let mutation = body
        .find("store.cleanup(retention)")
        .expect("cleanup must call the one typed domain mutation");

    assert!(validation < persistence);
    assert!(persistence < confirmation);
    assert!(confirmation < mutation);
}

#[test]
fn d063_degraded_effects_gate_precedes_confirmation_and_mutation() {
    let source = include_str!("commands/settings.rs");
    let body = function_body(source, "pub async fn cleanup_mcp_audit_logs(");
    let persistence = body
        .find("require_effects_allowed()")
        .expect("cleanup must retain the persistence effects gate");
    let confirmation = body
        .find("require_danger_action_confirmation(")
        .expect("cleanup must retain Rust-owned native confirmation");
    let mutation = body
        .find("store.cleanup(")
        .expect("cleanup must retain a governed domain mutation");

    assert!(persistence < confirmation);
    assert!(confirmation < mutation);
}

#[test]
fn d063_negative_retention_is_rejected_without_mutation() {
    assert_invalid_retention_is_non_mutating(-1);
}

#[test]
fn d063_zero_retention_is_rejected_without_mutation() {
    assert_invalid_retention_is_non_mutating(0);
}

#[test]
fn d063_above_maximum_retention_is_rejected_without_mutation() {
    assert_invalid_retention_is_non_mutating(MCP_AUDIT_RETENTION_MAX_DAYS + 1);
}

#[test]
fn d063_overflow_retention_is_rejected_without_panic_or_mutation() {
    assert_invalid_retention_is_non_mutating(i64::MAX);
}

#[test]
fn d063_bounded_cleanup_deletes_old_only_and_preserves_recent() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp-audit.sqlite");
    let store = audit_store(&path);
    insert_at(&store, &path, "d063-old-row", "2020-01-01T00:00:00+00:00");
    insert_at(
        &store,
        &path,
        "d063-recent-row",
        "2090-01-01T00:00:00+00:00",
    );

    assert_eq!(store.cleanup(30).expect("bounded D063 cleanup"), 1);
    assert_eq!(
        row_truth(&store),
        vec![(
            "d063-recent-row".to_string(),
            "2090-01-01T00:00:00+00:00".to_string()
        )]
    );
}
