use super::*;
use serde_json::json;
use std::path::Path;

const RAW_ARGUMENT_SENTINEL: &str = "D068-RAW-MEDICAL-ARGUMENT-7419";
const RAW_RESULT_SENTINEL: &str = "D068-RAW-FINANCIAL-RESULT-2853";

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuditArtifact {
    name: String,
    bytes_sha256: String,
    len: u64,
    readonly: bool,
    modified_nanos: Option<u128>,
}

fn artifact_family(path: &Path) -> Vec<AuditArtifact> {
    let parent = path.parent().expect("D068 database parent");
    let base = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("D068 UTF-8 database name");
    let mut artifacts = std::fs::read_dir(parent)
        .expect("list D068 database family")
        .map(|entry| entry.expect("read D068 family entry").path())
        .filter(|candidate| {
            candidate
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name == base
                        || name == format!("{base}-wal")
                        || name == format!("{base}-shm")
                        || name == format!("{base}-journal")
                })
        })
        .map(|candidate| {
            let metadata =
                std::fs::symlink_metadata(&candidate).expect("read D068 database-family metadata");
            let bytes = std::fs::read(&candidate).expect("read D068 database-family bytes");
            let bytes_sha256 = general_purpose::STANDARD_NO_PAD
                .encode(ring::digest::digest(&SHA256, &bytes).as_ref());
            let modified_nanos = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_nanos());
            AuditArtifact {
                name: candidate
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("D068 artifact name")
                    .to_string(),
                bytes_sha256,
                len: metadata.len(),
                readonly: metadata.permissions().readonly(),
                modified_nanos,
            }
        })
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| left.name.cmp(&right.name));
    artifacts
}

fn material() -> AuditKeyMaterial {
    AuditKeyMaterial {
        config: AuditKeyConfig {
            mode: KeyMode::Keychain,
            salt_b64: None,
            env_var: None,
            key_ref: Some("openlife/mcp-audit/d068/store-fixture/epoch/7".into()),
            epoch: 7,
            created_at: "2026-07-13T12:00:00Z".into(),
        },
        key: [0xD6; 32],
    }
}

fn create_store(path: &Path) -> McpAuditStore {
    McpAuditStore::with_key_materials(path, vec![material()])
        .expect("create real D068 file-backed audit store")
}

fn insert_encrypted_row(
    store: &McpAuditStore,
    tool_name: &str,
    arguments_encrypted: &str,
    result_encrypted: &str,
    payload_version: i64,
) -> i64 {
    let connection = store.conn().expect("open D068 fixture database");
    connection
        .execute(
            "INSERT INTO mcp_log (
                tool_name, arguments_encrypted, result_encrypted, success, pii_found,
                created_at, key_epoch, payload_minimized_version
             ) VALUES (?1, ?2, ?3, 1, 1, ?4, ?5, ?6)",
            params![
                tool_name,
                arguments_encrypted,
                result_encrypted,
                "2026-07-13T12:00:00Z",
                store.key_config().epoch as i64,
                payload_version,
            ],
        )
        .expect("insert D068 encrypted fixture row");
    connection.last_insert_rowid()
}

fn legacy_ciphertexts(store: &McpAuditStore) -> (String, String) {
    (
        store
            .d068_encrypt_legacy_payload_fixture_for_test(
                &json!({"medicalNote": RAW_ARGUMENT_SENTINEL}).to_string(),
            )
            .expect("encrypt source-backed legacy arguments"),
        store
            .d068_encrypt_legacy_payload_fixture_for_test(RAW_RESULT_SENTINEL)
            .expect("encrypt source-backed legacy result"),
    )
}

fn legal_receipts() -> (String, String) {
    (
        audit_arguments_receipt(&json!({"bounded": true}))
            .expect("materialize legal D068 argument receipt"),
        audit_result_receipt("bounded-result"),
    )
}

#[derive(Debug)]
struct ProductReadObservation {
    list: std::result::Result<Vec<McpLogEntry>, String>,
    export: std::result::Result<AuditExport, String>,
}

impl ProductReadObservation {
    fn list_failed_closed(&self) -> bool {
        self.list.is_err()
    }

    fn export_failed_closed(&self) -> bool {
        self.export.is_err()
    }

    fn both_failed_closed(&self) -> bool {
        self.list_failed_closed() && self.export_failed_closed()
    }

    fn successful_product_json(&self) -> String {
        let list = self
            .list
            .as_ref()
            .ok()
            .map(|logs| serde_json::to_value(logs).expect("serialize D068 list observation"));
        let export =
            self.export.as_ref().ok().map(|export| {
                serde_json::to_value(export).expect("serialize D068 export observation")
            });
        serde_json::to_string(&(list, export)).expect("serialize D068 successful product reads")
    }

    fn into_successes(self) -> (Vec<McpLogEntry>, AuditExport) {
        let logs = self.list.expect("D068 list succeeded");
        let export = self.export.expect("D068 export succeeded");
        (logs, export)
    }
}

fn restart_and_observe(path: &Path) -> ProductReadObservation {
    let list = McpAuditStore::with_key_materials(path, vec![material()])
        .map_err(|error| error.to_string())
        .and_then(|store| store.list_logs(100).map_err(|error| error.to_string()));
    // Export is deliberately attempted from a fresh product store. A failed
    // list call must not be allowed to poison shared in-memory state and hide
    // an independently vulnerable export path.
    let export = McpAuditStore::with_key_materials(path, vec![material()])
        .map_err(|error| error.to_string())
        .and_then(|store| store.export_logs(30).map_err(|error| error.to_string()));
    ProductReadObservation { list, export }
}

fn assert_no_raw_payload(value: &impl Serialize) {
    let serialized = serde_json::to_string(value).expect("serialize D068 product observation");
    assert!(!serialized.contains(RAW_ARGUMENT_SENTINEL));
    assert!(!serialized.contains(RAW_RESULT_SENTINEL));
}

fn version_flip_attack(raw_sentinel: &str) -> (ProductReadObservation, bool, bool) {
    let directory = tempfile::tempdir().expect("D068 version-flip temp directory");
    let path = directory.path().join("mcp_audit.db");
    let store = create_store(&path);
    let (arguments, result) = legacy_ciphertexts(&store);
    let row_id = insert_encrypted_row(&store, "d068_version_flip", &arguments, &result, 0);
    store
        .conn()
        .expect("open D068 tamper connection")
        .execute(
            "UPDATE mcp_log SET payload_minimized_version = ?1 WHERE id = ?2",
            params![MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION, row_id],
        )
        .expect("flip only the unauthenticated payload version");
    drop(store);
    let before = artifact_family(&path);
    let observation = restart_and_observe(&path);
    let raw_exposed = observation.successful_product_json().contains(raw_sentinel);
    let after = artifact_family(&path);
    (
        observation,
        raw_exposed,
        before == after && !before.is_empty(),
    )
}

#[test]
fn d068_version_flip_cannot_expose_legacy_argument_plaintext_or_rewrite_database() {
    let (observation, raw_exposed, byte_exact) = version_flip_attack(RAW_ARGUMENT_SENTINEL);
    assert!(
        observation.list_failed_closed() && !raw_exposed && byte_exact,
        "version flip must fail list before product truth with zero rewrite: list_failed={}, raw_exposed={raw_exposed}, byte_exact={byte_exact}",
        observation.list_failed_closed()
    );
}

#[test]
fn d068_version_flip_cannot_expose_legacy_result_plaintext_through_export() {
    let (observation, raw_exposed, byte_exact) = version_flip_attack(RAW_RESULT_SENTINEL);
    assert!(
        observation.export_failed_closed() && !raw_exposed && byte_exact,
        "export must share authenticated format truth and zero-rewrite failure: export_failed={}, raw_exposed={raw_exposed}, byte_exact={byte_exact}",
        observation.export_failed_closed()
    );
}

#[test]
fn d068_legal_current_product_write_remains_exactly_readable_and_minimized() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp_audit.db");
    let store = create_store(&path);
    store
        .insert_log(
            "d068_current_control",
            &json!({"secret": RAW_ARGUMENT_SENTINEL}),
            RAW_RESULT_SENTINEL,
            true,
            true,
        )
        .expect("insert legal current D068 row");
    drop(store);

    let (logs, export) = restart_and_observe(&path).into_successes();
    assert_eq!(logs.len(), 1);
    assert_eq!(export.entry_count, 1);
    assert_eq!(
        logs[0].arguments,
        audit_arguments_receipt(&json!({"secret": RAW_ARGUMENT_SENTINEL})).unwrap()
    );
    assert_eq!(logs[0].result, audit_result_receipt(RAW_RESULT_SENTINEL));
    assert_no_raw_payload(&(logs, export));
}

#[test]
fn d068_valid_current_fixture_uses_the_same_authenticated_envelope_decoder() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp_audit.db");
    let store = create_store(&path);
    let (arguments_receipt, result_receipt) = legal_receipts();
    let arguments = store
        .d068_encrypt_current_payload_fixture_for_test(
            "arguments",
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            &arguments_receipt,
        )
        .unwrap();
    let result = store
        .d068_encrypt_current_payload_fixture_for_test(
            "result",
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            &result_receipt,
        )
        .unwrap();
    insert_encrypted_row(
        &store,
        "d068_current_fixture_control",
        &arguments,
        &result,
        MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
    );
    drop(store);

    let (logs, export) = restart_and_observe(&path).into_successes();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].arguments, arguments_receipt);
    assert_eq!(logs[0].result, result_receipt);
    assert_eq!(export.entry_count, 1);
}

#[test]
fn d068_legal_version_zero_legacy_payload_migrates_transactionally_and_stays_readable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp_audit.db");
    let store = create_store(&path);
    let (arguments, result) = legacy_ciphertexts(&store);
    let row_id = insert_encrypted_row(&store, "d068_legacy_control", &arguments, &result, 0);
    drop(store);

    let (logs, export) = restart_and_observe(&path).into_successes();
    assert_eq!(logs.len(), 1);
    assert_eq!(export.entry_count, 1);
    assert_no_raw_payload(&(logs, export));
    let connection = Connection::open(&path).unwrap();
    let version: i64 = connection
        .query_row(
            "SELECT payload_minimized_version FROM mcp_log WHERE id = ?1",
            [row_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION);
}

fn invalid_current_receipt_variants() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "wrong_kind",
            "arguments",
            json!({"kind":"result","payloadStored":false,"valueType":"object","bytes":1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        ),
        (
            "payload_stored",
            "arguments",
            json!({"kind":"arguments","payloadStored":true,"valueType":"object","bytes":1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        ),
        (
            "invalid_value_type",
            "arguments",
            json!({"kind":"arguments","payloadStored":false,"valueType":"secret_object","bytes":1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        ),
        (
            "negative_bytes",
            "arguments",
            json!({"kind":"arguments","payloadStored":false,"valueType":"object","bytes":-1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        ),
        (
            "bad_digest",
            "arguments",
            json!({"kind":"arguments","payloadStored":false,"valueType":"object","bytes":1,"digest":"sha256:not-a-sha256-digest"}),
        ),
        (
            "unknown_field",
            "arguments",
            json!({"kind":"arguments","payloadStored":false,"valueType":"object","bytes":1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","raw":"must-not-be-accepted"}),
        ),
        (
            "missing_digest",
            "arguments",
            json!({"kind":"arguments","payloadStored":false,"valueType":"object","bytes":1}),
        ),
        (
            "missing_payload_stored",
            "arguments",
            json!({"kind":"arguments","valueType":"object","bytes":1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        ),
        (
            "wrong_result_kind",
            "result",
            json!({"kind":"arguments","payloadStored":false,"valueType":"string","bytes":1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        ),
        (
            "wrong_result_value_type",
            "result",
            json!({"kind":"result","payloadStored":false,"valueType":"object","bytes":1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        ),
        (
            "result_payload_stored",
            "result",
            json!({"kind":"result","payloadStored":true,"valueType":"string","bytes":1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        ),
        (
            "result_negative_bytes",
            "result",
            json!({"kind":"result","payloadStored":false,"valueType":"string","bytes":-1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        ),
        (
            "result_bad_digest",
            "result",
            json!({"kind":"result","payloadStored":false,"valueType":"string","bytes":1,"digest":"sha256:not-a-sha256-digest"}),
        ),
        (
            "result_unknown_field",
            "result",
            json!({"kind":"result","payloadStored":false,"valueType":"string","bytes":1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","raw":"must-not-be-accepted"}),
        ),
        (
            "result_missing_digest",
            "result",
            json!({"kind":"result","payloadStored":false,"valueType":"string","bytes":1}),
        ),
        (
            "result_missing_payload_stored",
            "result",
            json!({"kind":"result","valueType":"string","bytes":1,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}),
        ),
    ]
}

#[test]
fn d068_strict_current_decoder_rejects_every_invalid_receipt_for_list_and_export() {
    let mut accepted = Vec::new();
    let mut rewritten = Vec::new();
    for (label, role, invalid_receipt) in invalid_current_receipt_variants() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit.db");
        let store = create_store(&path);
        let (legal_arguments, legal_result) = legal_receipts();
        let candidate = store
            .d068_encrypt_current_payload_fixture_for_test(
                role,
                MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
                &invalid_receipt.to_string(),
            )
            .unwrap();
        let arguments = if role == "arguments" {
            candidate.clone()
        } else {
            store
                .d068_encrypt_current_payload_fixture_for_test(
                    "arguments",
                    MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
                    &legal_arguments,
                )
                .unwrap()
        };
        let result = if role == "result" {
            candidate
        } else {
            store
                .d068_encrypt_current_payload_fixture_for_test(
                    "result",
                    MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
                    &legal_result,
                )
                .unwrap()
        };
        insert_encrypted_row(
            &store,
            &format!("d068_invalid_{label}"),
            &arguments,
            &result,
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
        );
        drop(store);
        let before = artifact_family(&path);
        let observation = restart_and_observe(&path);
        if !observation.list_failed_closed() {
            accepted.push(format!("{label}:list"));
        }
        if !observation.export_failed_closed() {
            accepted.push(format!("{label}:export"));
        }
        if artifact_family(&path) != before {
            rewritten.push(label);
        }
    }
    assert!(
        accepted.is_empty() && rewritten.is_empty(),
        "invalid current receipts reached product truth or rewrote durable bytes: accepted={accepted:?}, rewritten={rewritten:?}"
    );
}

#[test]
fn d068_corrupt_current_ciphertext_fails_list_and_export_without_rewrite() {
    let mut accepted = Vec::new();
    let mut rewritten = Vec::new();
    for corrupt_role in ["arguments", "result"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit.db");
        let store = create_store(&path);
        let (arguments_receipt, result_receipt) = legal_receipts();
        let mut arguments = store
            .d068_encrypt_current_payload_fixture_for_test(
                "arguments",
                MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
                &arguments_receipt,
            )
            .unwrap();
        let mut result = store
            .d068_encrypt_current_payload_fixture_for_test(
                "result",
                MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
                &result_receipt,
            )
            .unwrap();
        if corrupt_role == "arguments" {
            arguments = "not-valid-aead-ciphertext".to_string();
        } else {
            result = "not-valid-aead-ciphertext".to_string();
        }
        insert_encrypted_row(
            &store,
            &format!("d068_current_corrupt_{corrupt_role}"),
            &arguments,
            &result,
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
        );
        drop(store);
        let before = artifact_family(&path);
        let observation = restart_and_observe(&path);
        if !observation.both_failed_closed() {
            accepted.push(corrupt_role);
        }
        if artifact_family(&path) != before {
            rewritten.push(corrupt_role);
        }
    }
    assert!(
        accepted.is_empty() && rewritten.is_empty(),
        "current ciphertext authentication failures reached product truth or rewrote durable bytes: accepted={accepted:?}, rewritten={rewritten:?}"
    );
}

#[test]
fn d068_envelope_role_version_and_column_swaps_fail_closed_without_rewrite() {
    let mut accepted = Vec::new();
    let mut rewritten = Vec::new();
    for scenario in ["envelope_version", "swap_columns", "column_version"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit.db");
        let store = create_store(&path);
        let (arguments_receipt, result_receipt) = legal_receipts();
        let envelope_version = if scenario == "envelope_version" {
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION + 1
        } else {
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION
        };
        let mut arguments = store
            .d068_encrypt_current_payload_fixture_for_test(
                "arguments",
                envelope_version,
                &arguments_receipt,
            )
            .unwrap();
        let mut result = store
            .d068_encrypt_current_payload_fixture_for_test(
                "result",
                envelope_version,
                &result_receipt,
            )
            .unwrap();
        if scenario == "swap_columns" {
            std::mem::swap(&mut arguments, &mut result);
        }
        let database_version = if scenario == "column_version" {
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION + 1
        } else {
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION
        };
        insert_encrypted_row(
            &store,
            &format!("d068_{scenario}"),
            &arguments,
            &result,
            database_version,
        );
        drop(store);
        let before = artifact_family(&path);
        let observation = restart_and_observe(&path);
        if !observation.list_failed_closed() {
            accepted.push(format!("{scenario}:list"));
        }
        if !observation.export_failed_closed() {
            accepted.push(format!("{scenario}:export"));
        }
        if artifact_family(&path) != before {
            rewritten.push(scenario);
        }
    }
    assert!(
        accepted.is_empty() && rewritten.is_empty(),
        "unbound envelope role/version scenarios were accepted or rewrote durable bytes: accepted={accepted:?}, rewritten={rewritten:?}"
    );
}

#[test]
fn d068_migration_authentication_failure_is_atomic_and_performs_zero_rewrite() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp_audit.db");
    let store = create_store(&path);
    let (arguments, result) = legacy_ciphertexts(&store);
    insert_encrypted_row(&store, "d068_legacy_valid_first", &arguments, &result, 0);
    insert_encrypted_row(
        &store,
        "d068_legacy_corrupt_second",
        "not-valid-aead-ciphertext",
        &result,
        0,
    );
    drop(store);
    let before = artifact_family(&path);

    let observation = restart_and_observe(&path);
    let after = artifact_family(&path);

    assert!(
        observation.both_failed_closed() && after == before,
        "legacy authentication failure must abort list and export with zero rewrite: both_failed={}, byte_exact={}",
        observation.both_failed_closed(),
        after == before
    );
}
