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
    material_at_epoch(7)
}

fn material_at_epoch(epoch: u64) -> AuditKeyMaterial {
    AuditKeyMaterial {
        config: AuditKeyConfig {
            mode: KeyMode::Keychain,
            salt_b64: None,
            env_var: None,
            key_ref: Some(format!(
                "openlife/mcp-audit/d068/store-fixture/epoch/{epoch}"
            )),
            epoch,
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
                McpAuditStore::sqlite_key_epoch(store.key_config().epoch).unwrap(),
                payload_version,
            ],
        )
        .expect("insert D068 encrypted fixture row");
    connection.last_insert_rowid()
}

#[allow(clippy::too_many_arguments)]
fn insert_current_fixture(
    store: &McpAuditStore,
    tool_name: &str,
    arguments_role: &str,
    arguments_format_version: i64,
    arguments_receipt_json: &str,
    result_role: &str,
    result_format_version: i64,
    result_receipt_json: &str,
    database_version: i64,
) -> i64 {
    store
        .d068_insert_current_payload_fixture_for_test(
            tool_name,
            arguments_role,
            arguments_format_version,
            arguments_receipt_json,
            result_role,
            result_format_version,
            result_receipt_json,
            database_version,
        )
        .expect("insert storage-bound D068 current fixture")
}

fn current_ciphertexts(store: &McpAuditStore, row_id: i64) -> (String, String) {
    store
        .conn()
        .expect("open D068 fixture database")
        .query_row(
            "SELECT arguments_encrypted, result_encrypted FROM mcp_log WHERE id = ?1",
            [row_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read D068 current fixture ciphertexts")
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

fn flip_authenticated_ciphertext(ciphertext: &str) -> String {
    const AES_GCM_NONCE_BYTES: usize = 12;
    const AES_GCM_TAG_BYTES: usize = 16;

    let original = general_purpose::STANDARD
        .decode(ciphertext)
        .expect("decode source-backed AEAD fixture");
    assert!(
        original.len() > AES_GCM_NONCE_BYTES + AES_GCM_TAG_BYTES,
        "source-backed AEAD fixture must contain nonce, ciphertext, and authentication tag"
    );
    let mut corrupted = original.clone();
    let flip_index = corrupted.len() - 1;
    assert!(flip_index >= AES_GCM_NONCE_BYTES);
    corrupted[flip_index] ^= 0x01;

    let encoded = general_purpose::STANDARD.encode(&corrupted);
    let decoded = general_purpose::STANDARD
        .decode(&encoded)
        .expect("bit-flipped AEAD fixture remains valid base64");
    assert_eq!(decoded.len(), original.len());
    assert_ne!(decoded, original);
    encoded
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
fn d068_noncanonical_record_identity_replay_fails_live_and_restart_without_rewrite() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp_audit.db");
    let store = create_store(&path);
    store
        .insert_log(
            "d068_noncanonical_replay",
            &json!({"bounded": true}),
            "bounded-result",
            true,
            false,
        )
        .unwrap();
    let connection = store.conn().unwrap();
    connection
        .execute(
            "INSERT INTO mcp_log (
                audit_record_id, tool_name, arguments_encrypted, result_encrypted,
                success, pii_found, created_at, key_epoch, payload_minimized_version
             )
             SELECT replace(audit_record_id, '-', ''), tool_name,
                    arguments_encrypted, result_encrypted, success, pii_found,
                    created_at, key_epoch, payload_minimized_version
             FROM mcp_log WHERE tool_name = 'd068_noncanonical_replay'",
            [],
        )
        .expect("the binary TEXT uniqueness index admits a noncanonical UUID spelling");
    let record_ids = connection
        .prepare(
            "SELECT audit_record_id FROM mcp_log
             WHERE tool_name = 'd068_noncanonical_replay' ORDER BY id",
        )
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(record_ids.len(), 2);
    assert_eq!(record_ids[0].replace('-', ""), record_ids[1]);
    assert_eq!(
        Uuid::parse_str(&record_ids[0]).unwrap(),
        Uuid::parse_str(&record_ids[1]).unwrap(),
        "the two SQLite identities are one authenticated UUID"
    );
    drop(connection);

    let before_live_reads = artifact_family(&path);
    let live = ProductReadObservation {
        list: store.list_logs(100).map_err(|error| error.to_string()),
        export: store.export_logs(30).map_err(|error| error.to_string()),
    };
    let after_live_reads = artifact_family(&path);
    assert!(
        live.both_failed_closed(),
        "a noncanonical spelling must not become a second authenticated product DTO: {live:#?}"
    );
    assert!(
        live.list
            .as_ref()
            .unwrap_err()
            .contains("mcp_audit_record_id_noncanonical"),
        "live list must fail for the canonical identity invariant"
    );
    assert!(
        live.export
            .as_ref()
            .unwrap_err()
            .contains("mcp_audit_record_id_noncanonical"),
        "live export must fail for the canonical identity invariant"
    );
    assert_eq!(
        after_live_reads, before_live_reads,
        "failed live reads must not rewrite the SQLite family"
    );

    drop(store);
    let before_restart = artifact_family(&path);
    let restarted = restart_and_observe(&path);
    let after_restart = artifact_family(&path);
    assert!(
        restarted.both_failed_closed(),
        "restart preflight must reject the same noncanonical identity"
    );
    assert!(
        restarted
            .list
            .as_ref()
            .unwrap_err()
            .contains("mcp_audit_record_id_noncanonical")
            && restarted
                .export
                .as_ref()
                .unwrap_err()
                .contains("mcp_audit_record_id_noncanonical"),
        "restart list/export must preserve the same root-cause disposition"
    );
    assert_eq!(
        after_restart, before_restart,
        "failed restart preflight must be byte-exact and zero-write"
    );
}

#[test]
fn d068_distinct_canonical_record_identities_remain_live_and_restart_readable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp_audit.db");
    let store = create_store(&path);
    for _ in 0..2 {
        store
            .insert_log(
                "d068_canonical_identity_control",
                &json!({"bounded": true}),
                "bounded-result",
                true,
                false,
            )
            .unwrap();
    }
    let record_ids = store
        .conn()
        .unwrap()
        .prepare(
            "SELECT audit_record_id FROM mcp_log
             WHERE tool_name = 'd068_canonical_identity_control' ORDER BY id",
        )
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(record_ids.len(), 2);
    assert_ne!(record_ids[0], record_ids[1]);
    for record_id in record_ids {
        assert_eq!(Uuid::parse_str(&record_id).unwrap().to_string(), record_id);
    }
    assert_eq!(store.list_logs(100).unwrap().len(), 2);
    assert_eq!(store.export_logs(30).unwrap().entry_count, 2);
    drop(store);
    let (logs, export) = restart_and_observe(&path).into_successes();
    assert_eq!(logs.len(), 2);
    assert_eq!(export.entry_count, 2);
}

#[test]
fn d068_epoch_above_sqlite_range_is_rejected_before_create_rotate_or_write() {
    let directory = tempfile::tempdir().unwrap();
    let new_path = directory.path().join("must-not-be-created.db");
    let before_new = artifact_family(&new_path);
    let constructor =
        McpAuditStore::with_key_materials(&new_path, vec![material_at_epoch(u64::MAX)]);
    assert!(
        constructor
            .err()
            .expect("oversized epoch hydration is rejected")
            .to_string()
            .contains("mcp_audit_key_epoch_exceeds_sqlite_range"),
        "oversized epoch must fail hydration at the representation boundary"
    );
    assert_eq!(
        artifact_family(&new_path),
        before_new,
        "rejected hydration must not create a SQLite family"
    );

    let path = directory.path().join("existing.db");
    let mut store = create_store(&path);
    store
        .insert_log(
            "d068_epoch_control",
            &json!({"bounded": true}),
            "bounded-result",
            true,
            false,
        )
        .unwrap();
    let before_existing = artifact_family(&path);
    let original_epoch = store.key_config().epoch;
    assert!(store
        .rotate_key_material(material_at_epoch(u64::MAX))
        .unwrap_err()
        .to_string()
        .contains("mcp_audit_key_epoch_exceeds_sqlite_range"));
    assert_eq!(store.key_config().epoch, original_epoch);
    assert_eq!(artifact_family(&path), before_existing);

    let mut poisoned_writer = store.clone();
    poisoned_writer.key_config.epoch = u64::MAX;
    let write_error = poisoned_writer
        .insert_log(
            "d068_epoch_must_not_wrap",
            &json!({"bounded": true}),
            "bounded-result",
            true,
            false,
        )
        .unwrap_err()
        .to_string();
    assert!(
        write_error.contains("mcp_audit_key_epoch_exceeds_sqlite_range"),
        "write boundary must independently reject an unrepresentable epoch: {write_error}"
    );
    assert_eq!(
        artifact_family(&path),
        before_existing,
        "rejected rotation/write must not mutate or migrate the SQLite family"
    );
    assert_eq!(store.list_logs(100).unwrap().len(), 1);

    drop(store);
    let before_reopen = artifact_family(&path);
    let reopen =
        McpAuditStore::with_key_materials(&path, vec![material(), material_at_epoch(u64::MAX)]);
    assert!(
        reopen
            .err()
            .expect("oversized epoch reopen is rejected")
            .to_string()
            .contains("mcp_audit_key_epoch_exceeds_sqlite_range"),
        "oversized epoch must fail before migration"
    );
    let read_only_reopen = McpAuditStore::open_read_only_existing_with_key_materials(
        &path,
        vec![material(), material_at_epoch(u64::MAX)],
    );
    assert!(
        read_only_reopen
            .err()
            .expect("oversized epoch read-only reopen is rejected")
            .to_string()
            .contains("mcp_audit_key_epoch_exceeds_sqlite_range"),
        "read-only hydration shares the same epoch authority boundary"
    );
    assert_eq!(
        artifact_family(&path),
        before_reopen,
        "rejected reopen must not migrate an existing database"
    );
}

#[test]
fn d068_valid_current_fixture_uses_the_same_authenticated_envelope_decoder() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp_audit.db");
    let store = create_store(&path);
    let (arguments_receipt, result_receipt) = legal_receipts();
    insert_current_fixture(
        &store,
        "d068_current_fixture_control",
        "arguments",
        MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
        &arguments_receipt,
        "result",
        MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
        &result_receipt,
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

fn valid_receipt_value(role: &str) -> Value {
    let value_type = if role == "arguments" {
        "object"
    } else {
        "string"
    };
    json!({
        "kind": role,
        "payloadStored": false,
        "valueType": value_type,
        "bytes": 1,
        "digest": "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    })
}

fn receipt_with(role: &str, field: &str, value: Value) -> Value {
    let mut receipt = valid_receipt_value(role);
    receipt
        .as_object_mut()
        .expect("D068 receipt fixture is an object")
        .insert(field.to_string(), value);
    receipt
}

fn receipt_without(role: &str, field: &str) -> Value {
    let mut receipt = valid_receipt_value(role);
    receipt
        .as_object_mut()
        .expect("D068 receipt fixture is an object")
        .remove(field);
    receipt
}

fn invalid_current_receipt_variants() -> Vec<(String, &'static str, Value)> {
    let mut variants = Vec::new();
    for (role, wrong_kind) in [("arguments", "result"), ("result", "arguments")] {
        variants.extend([
            (
                format!("{role}_wrong_kind"),
                role,
                receipt_with(role, "kind", json!(wrong_kind)),
            ),
            (
                format!("{role}_payload_stored"),
                role,
                receipt_with(role, "payloadStored", json!(true)),
            ),
            (
                format!("{role}_invalid_value_type"),
                role,
                receipt_with(role, "valueType", json!("secret_object")),
            ),
            (
                format!("{role}_negative_bytes"),
                role,
                receipt_with(role, "bytes", json!(-1)),
            ),
            (
                format!("{role}_fractional_bytes"),
                role,
                receipt_with(role, "bytes", json!(1.5)),
            ),
            (
                format!("{role}_oversized_bytes"),
                role,
                receipt_with(role, "bytes", json!(1e100)),
            ),
            (
                format!("{role}_bad_digest"),
                role,
                receipt_with(role, "digest", json!("sha256:not-a-sha256-digest")),
            ),
            (
                format!("{role}_unknown_field"),
                role,
                receipt_with(role, "raw", json!("must-not-be-accepted")),
            ),
        ]);

        for field in ["kind", "payloadStored", "valueType", "bytes", "digest"] {
            variants.push((
                format!("{role}_missing_{field}"),
                role,
                receipt_without(role, field),
            ));
        }

        for (field, wrong_type) in [
            ("kind", json!(false)),
            ("payloadStored", json!("false")),
            ("valueType", json!(false)),
            ("bytes", json!("1")),
            ("digest", json!(false)),
        ] {
            variants.push((
                format!("{role}_{field}_wrong_type"),
                role,
                receipt_with(role, field, wrong_type),
            ));
        }
    }
    variants
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
        let invalid_receipt = invalid_receipt.to_string();
        let arguments = if role == "arguments" {
            invalid_receipt.as_str()
        } else {
            legal_arguments.as_str()
        };
        let result = if role == "result" {
            invalid_receipt.as_str()
        } else {
            legal_result.as_str()
        };
        insert_current_fixture(
            &store,
            &format!("d068_invalid_{label}"),
            "arguments",
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            arguments,
            "result",
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            result,
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
        let row_id = insert_current_fixture(
            &store,
            &format!("d068_current_corrupt_{corrupt_role}"),
            "arguments",
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            &arguments_receipt,
            "result",
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
            &result_receipt,
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION,
        );
        let (mut arguments, mut result) = current_ciphertexts(&store, row_id);
        if corrupt_role == "arguments" {
            arguments = flip_authenticated_ciphertext(&arguments);
        } else {
            result = flip_authenticated_ciphertext(&result);
        }
        store
            .conn()
            .unwrap()
            .execute(
                "UPDATE mcp_log SET arguments_encrypted = ?1, result_encrypted = ?2 WHERE id = ?3",
                params![arguments, result, row_id],
            )
            .unwrap();
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
    for scenario in [
        "envelope_version",
        "arguments_envelope_role",
        "result_envelope_role",
        "swap_columns",
        "column_version",
        "matching_unsupported_version",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit.db");
        let store = create_store(&path);
        let (arguments_receipt, result_receipt) = legal_receipts();
        let envelope_version = if matches!(
            scenario,
            "envelope_version" | "matching_unsupported_version"
        ) {
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION + 1
        } else {
            MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION
        };
        let arguments_envelope_role = if scenario == "arguments_envelope_role" {
            "result"
        } else {
            "arguments"
        };
        let result_envelope_role = if scenario == "result_envelope_role" {
            "arguments"
        } else {
            "result"
        };
        let database_version =
            if matches!(scenario, "column_version" | "matching_unsupported_version") {
                MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION + 1
            } else {
                MCP_AUDIT_PAYLOAD_MINIMIZED_VERSION
            };
        let row_id = insert_current_fixture(
            &store,
            &format!("d068_{scenario}"),
            arguments_envelope_role,
            envelope_version,
            &arguments_receipt,
            result_envelope_role,
            envelope_version,
            &result_receipt,
            database_version,
        );
        if scenario == "swap_columns" {
            let (arguments, result) = current_ciphertexts(&store, row_id);
            store
                .conn()
                .unwrap()
                .execute(
                    "UPDATE mcp_log SET arguments_encrypted = ?1, result_encrypted = ?2 WHERE id = ?3",
                    params![result, arguments, row_id],
                )
                .unwrap();
        }
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
    let corrupted_arguments = flip_authenticated_ciphertext(&arguments);
    insert_encrypted_row(&store, "d068_legacy_valid_first", &arguments, &result, 0);
    insert_encrypted_row(
        &store,
        "d068_legacy_corrupt_second",
        &corrupted_arguments,
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
