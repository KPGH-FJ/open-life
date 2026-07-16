use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use fs2::FileExt;
use openlife_core::mcp_audit::{
    AuditKeyConfig, AuditKeyMaterial, KeyMode, McpAuditDurableWriter, McpAuditStore, McpLogEntry,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::bootstrap::{
    bootstrap_with_secret_store_for_test, inject_fixed_mcp_audit_store_identity_for_test,
    BootstrapResult,
};
use crate::commands::settings::{
    inject_mcp_audit_rotation_fault_for_test, rotate_mcp_audit_key_after_confirmation,
    McpAuditRotationFaultKind, McpAuditRotationPostBeginStage,
};
use crate::persistence_coordinator::PersistenceStoreMode;
use crate::secret_store::{
    inject_fixed_mcp_audit_epoch_for_test, SecretStore, MCP_AUDIT_KEY_REF_PREFIX,
};
use crate::storage::{
    inject_mcp_audit_keyring_save_failure_for_test, load_mcp_audit_keyring_from_path,
    save_mcp_audit_keyring_to_path,
};

const DECRYPT_FAILED: &str = "[decrypt failed]";

#[derive(Clone, Debug, PartialEq, Eq)]
struct SecretGetObservation {
    secret_ref: String,
    returned: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SecretSetObservation {
    secret_ref: String,
    previous: Option<Vec<u8>>,
    attempted: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SecretDeleteObservation {
    secret_ref: String,
    removed: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SecretStoreObservation {
    values: HashMap<String, Vec<u8>>,
    gets: Vec<SecretGetObservation>,
    sets: Vec<SecretSetObservation>,
    deletes: Vec<SecretDeleteObservation>,
}

/// Replacement semantics deliberately model the currently shipped
/// `KeyringSecretStore::set -> set_password` boundary. The fake is restricted
/// to secret-store atomicity tests; SQLite and owner-lease evidence below use
/// real files and real SQLite connections.
#[derive(Default)]
struct D064RecordingSecretStore {
    state: Mutex<SecretStoreObservation>,
}

impl D064RecordingSecretStore {
    fn preload_key(&self, secret_ref: &str, key: [u8; 32]) {
        self.preload_bytes(
            secret_ref,
            general_purpose::STANDARD.encode(key).into_bytes(),
        );
    }

    fn preload_bytes(&self, secret_ref: &str, bytes: Vec<u8>) {
        self.state
            .lock()
            .expect("D064 secret fixture mutex")
            .values
            .insert(secret_ref.to_string(), bytes);
    }

    fn bytes(&self, secret_ref: &str) -> Option<Vec<u8>> {
        self.state
            .lock()
            .expect("D064 secret fixture mutex")
            .values
            .get(secret_ref)
            .cloned()
    }

    fn observation(&self) -> SecretStoreObservation {
        self.state
            .lock()
            .expect("D064 secret fixture mutex")
            .clone()
    }

    fn mcp_set_refs(&self) -> Vec<String> {
        self.observation()
            .sets
            .into_iter()
            .filter(|set| set.secret_ref.contains("/mcp-audit-key"))
            .map(|set| set.secret_ref)
            .collect()
    }

    fn mcp_deleted_refs(&self) -> Vec<String> {
        self.observation()
            .deletes
            .into_iter()
            .filter(|delete| delete.secret_ref.contains("/mcp-audit-key"))
            .map(|delete| delete.secret_ref)
            .collect()
    }

    fn live_mcp_refs(&self) -> Vec<String> {
        let mut refs = self
            .observation()
            .values
            .into_keys()
            .filter(|secret_ref| secret_ref.contains("/mcp-audit-key"))
            .collect::<Vec<_>>();
        refs.sort();
        refs
    }
}

impl SecretStore for D064RecordingSecretStore {
    fn get(&self, secret_ref: &str) -> Result<Option<String>> {
        let mut state = self.state.lock().expect("D064 secret fixture mutex");
        let returned = state.values.get(secret_ref).cloned();
        state.gets.push(SecretGetObservation {
            secret_ref: secret_ref.to_string(),
            returned: returned.clone(),
        });
        returned
            .map(|bytes| String::from_utf8(bytes).context("D064 secret fixture is not UTF-8"))
            .transpose()
    }

    fn set(&self, secret_ref: &str, value: &str) -> Result<()> {
        let mut state = self.state.lock().expect("D064 secret fixture mutex");
        let attempted = value.as_bytes().to_vec();
        let previous = state
            .values
            .insert(secret_ref.to_string(), attempted.clone());
        state.sets.push(SecretSetObservation {
            secret_ref: secret_ref.to_string(),
            previous,
            attempted,
        });
        Ok(())
    }

    fn delete(&self, secret_ref: &str) -> Result<()> {
        let mut state = self.state.lock().expect("D064 secret fixture mutex");
        let removed = state.values.remove(secret_ref);
        state.deletes.push(SecretDeleteObservation {
            secret_ref: secret_ref.to_string(),
            removed,
        });
        Ok(())
    }
}

fn d064_keychain_config(epoch: u64, secret_ref: impl Into<String>) -> AuditKeyConfig {
    AuditKeyConfig {
        mode: KeyMode::Keychain,
        salt_b64: None,
        env_var: None,
        key_ref: Some(secret_ref.into()),
        epoch,
        created_at: format!("2026-07-13T01:{:02}:00Z", epoch % 60),
    }
}

fn d064_material(epoch: u64, secret_ref: impl Into<String>, key: [u8; 32]) -> AuditKeyMaterial {
    AuditKeyMaterial {
        config: d064_keychain_config(epoch, secret_ref),
        key,
    }
}

fn d064_insert(store: &McpAuditStore, tool_name: &str) {
    store
        .insert_log(
            tool_name,
            &serde_json::json!({"fixture": tool_name}),
            &format!("result-{tool_name}"),
            true,
            false,
        )
        .unwrap_or_else(|error| panic!("insert D064 audit fixture {tool_name}: {error}"));
}

fn d064_logs_complete(logs: &[McpLogEntry], expected_tools: &[&str]) -> bool {
    let mut actual_tools = logs
        .iter()
        .map(|log| log.tool_name.as_str())
        .collect::<Vec<_>>();
    actual_tools.sort_unstable();
    let mut expected_tools = expected_tools.to_vec();
    expected_tools.sort_unstable();
    actual_tools == expected_tools
        && logs.iter().all(|log| {
            log.arguments != DECRYPT_FAILED
                && log.result != DECRYPT_FAILED
                && log.arguments.contains("payloadStored")
                && log.result.contains("payloadStored")
        })
}

#[derive(Debug)]
struct RestartObservation {
    error: Option<String>,
    row_count: usize,
    decrypt_failed_fields: usize,
    tool_names: Vec<String>,
}

impl RestartObservation {
    fn complete_for(&self, expected_tools: &[&str]) -> bool {
        if self.error.is_some() || self.decrypt_failed_fields != 0 {
            return false;
        }
        let mut actual = self
            .tool_names
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        actual.sort_unstable();
        let mut expected = expected_tools.to_vec();
        expected.sort_unstable();
        self.row_count == expected.len() && actual == expected
    }
}

fn d064_restart_observation(
    db_path: &Path,
    materials: Vec<AuditKeyMaterial>,
) -> RestartObservation {
    let store = match McpAuditStore::with_key_materials(db_path, materials) {
        Ok(store) => store,
        Err(error) => {
            return RestartObservation {
                error: Some(error.to_string()),
                row_count: 0,
                decrypt_failed_fields: 0,
                tool_names: Vec::new(),
            };
        }
    };
    match store.list_logs(100) {
        Ok(logs) => RestartObservation {
            error: None,
            row_count: logs.len(),
            decrypt_failed_fields: logs
                .iter()
                .map(|log| {
                    usize::from(log.arguments == DECRYPT_FAILED)
                        + usize::from(log.result == DECRYPT_FAILED)
                })
                .sum(),
            tool_names: logs.into_iter().map(|log| log.tool_name).collect(),
        },
        Err(error) => RestartObservation {
            error: Some(error.to_string()),
            row_count: 0,
            decrypt_failed_fields: 0,
            tool_names: Vec::new(),
        },
    }
}

fn d064_raw_audit_shape(db_path: &Path) -> (i64, i64) {
    let connection = rusqlite::Connection::open(db_path).expect("open D064 audit DB mechanically");
    connection
        .query_row(
            "SELECT COUNT(*), COUNT(DISTINCT key_epoch) FROM mcp_log",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("inspect D064 audit rows mechanically")
}

fn d064_store_mode(result: &BootstrapResult, store_name: &str) -> PersistenceStoreMode {
    result
        .state
        .persistence_coordinator
        .snapshot()
        .stores
        .into_iter()
        .find(|health| health.store == store_name)
        .unwrap_or_else(|| panic!("missing D064 persistence health for {store_name}"))
        .mode
}

fn d064_audit_reads_fail_closed(result: &BootstrapResult) -> bool {
    result
        .state
        .mcp_audit_store
        .try_lock()
        .expect("D064 audit store is not concurrently held")
        .list_logs(10)
        .is_err()
}

#[test]
fn d064_recording_secret_store_models_exact_replacement_bytes_control() {
    let store = D064RecordingSecretStore::default();
    let secret_ref = format!("{MCP_AUDIT_KEY_REF_PREFIX}6400");
    let first = general_purpose::STANDARD.encode([0x10; 32]).into_bytes();
    let second = general_purpose::STANDARD.encode([0x20; 32]).into_bytes();
    store.preload_bytes(&secret_ref, first.clone());

    assert_eq!(store.get(&secret_ref).unwrap().unwrap().as_bytes(), first);
    store
        .set(&secret_ref, std::str::from_utf8(&second).unwrap())
        .unwrap();

    let observation = store.observation();
    assert_eq!(store.bytes(&secret_ref), Some(second.clone()));
    assert_eq!(
        observation.sets,
        vec![SecretSetObservation {
            secret_ref,
            previous: Some(first),
            attempted: second,
        }]
    );
}

#[tokio::test]
async fn d064_existing_same_epoch_secret_is_create_only_and_preserves_k1() {
    let directory = tempfile::tempdir().unwrap();
    let identity = uuid::Uuid::parse_str("ba72dfd3-821a-4e71-b947-39091e5d9242").unwrap();
    let epoch = 6401;
    let store = D064RecordingSecretStore::default();
    let secret_ref = format!(
        "keychain://com.openlife.desktop/mcp-audit-key-store-{}-epoch-{epoch}",
        identity.simple()
    );
    let k1 = general_purpose::STANDARD.encode([0x11; 32]).into_bytes();
    store.preload_bytes(&secret_ref, k1.clone());
    let _identity = inject_fixed_mcp_audit_store_identity_for_test(identity);
    let _epoch = inject_fixed_mcp_audit_epoch_for_test(epoch);

    let result = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &store);
    let reference_mode = d064_store_mode(&result, "McpAuditKeyReferenceStore");
    let audit_mode = d064_store_mode(&result, "McpAuditStore");
    let reads_fail_closed = d064_audit_reads_fail_closed(&result);
    let observation = store.observation();
    let keyring_path = directory.path().join("mcp_audit_keys.json");
    let database_path = directory.path().join("mcp_audit.db");

    assert!(
        store.bytes(&secret_ref) == Some(k1.clone())
            && !keyring_path.exists()
            && !database_path.exists()
            && store.mcp_set_refs().is_empty()
            && store.mcp_deleted_refs().is_empty()
            && reference_mode == PersistenceStoreMode::Unavailable
            && audit_mode == PersistenceStoreMode::Unavailable
            && reads_fail_closed,
        "D064 RED: a valid UUIDv4 fresh product bootstrap colliding with K1 must fail before publishing Prepared or creating SQLite, while preserving exact secret/reference/database bytes; secret_ref={secret_ref}, before={k1:?}, after={:?}, keyring_exists={}, database_exists={}, reference_mode={reference_mode:?}, audit_mode={audit_mode:?}, gets={:?}, sets={:?}, deletes={:?}",
        store.bytes(&secret_ref),
        keyring_path.exists(),
        database_path.exists(),
        observation.gets,
        observation.sets,
        observation.deletes,
    );

    // Rotation must use the same pre-Pending reservation discipline. Exercise
    // the product post-confirmation state machine against a real Active
    // bootstrap rather than calling a credential primitive in isolation.
    let rotation_directory = tempfile::tempdir().unwrap();
    let rotation_identity = uuid::Uuid::parse_str("c86a5245-4b2f-4ba4-bac1-72174fbcf342").unwrap();
    let active_epoch = 6411;
    let collision_epoch = 6412;
    let rotation_store = std::sync::Arc::new(D064RecordingSecretStore::default());
    let _rotation_identity = inject_fixed_mcp_audit_store_identity_for_test(rotation_identity);
    let _active_epoch = inject_fixed_mcp_audit_epoch_for_test(active_epoch);
    let active = bootstrap_with_secret_store_for_test(
        rotation_directory.path().to_path_buf(),
        rotation_store.as_ref(),
    );
    active.state.persistence_coordinator.seal();
    active
        .state
        .persistence_coordinator
        .require_effects_allowed()
        .expect("rotation counterexample must model post-bootstrap product effects");
    {
        let owner = active
            .state
            .mcp_audit_store
            .try_lock()
            .expect("lock active D064 product audit owner");
        d064_insert(&owner, "before_rotation_collision");
    }
    let rotation_reference_path = rotation_directory.path().join("mcp_audit_keys.json");
    let rotation_database_path = rotation_directory.path().join("mcp_audit.db");
    let collision_ref = format!(
        "keychain://com.openlife.desktop/mcp-audit-key-store-{}-epoch-{collision_epoch}",
        rotation_identity.simple()
    );
    let collision_bytes = general_purpose::STANDARD.encode([0x22; 32]).into_bytes();
    rotation_store.preload_bytes(&collision_ref, collision_bytes.clone());
    let reference_before = std::fs::read(&rotation_reference_path).unwrap();
    let database_before = std::fs::read(&rotation_database_path).unwrap();
    let mcp_sets_before = rotation_store.mcp_set_refs();
    let _collision_epoch = inject_fixed_mcp_audit_epoch_for_test(collision_epoch);

    let rotation_error = rotate_mcp_audit_key_after_confirmation(
        &rotation_reference_path,
        rotation_store.clone(),
        &active.state,
    )
    .await
    .expect_err("occupied rotation reference must fail before Prepared");

    let reference_after = std::fs::read(&rotation_reference_path).unwrap();
    let database_after = std::fs::read(&rotation_database_path).unwrap();
    let rotation_observation = rotation_store.observation();
    let rows_before_followup = active
        .state
        .mcp_audit_store
        .try_lock()
        .expect("rotation collision released product owner")
        .list_logs(10)
        .unwrap();

    assert!(
        rotation_error.message().contains("reference_already_exists")
            && rotation_store.bytes(&collision_ref) == Some(collision_bytes.clone())
            && reference_after == reference_before
            && database_after == database_before
            && rotation_store.mcp_set_refs() == mcp_sets_before
            && rotation_store.mcp_deleted_refs().is_empty()
            && d064_logs_complete(&rows_before_followup, &["before_rotation_collision"]),
        "D064 RED: a valid UUIDv4 rotation collision must preserve exact secret/reference/DB bytes and leave the Active writer healthy; error={}, secret_before={collision_bytes:?}, secret_after={:?}, reference_equal={}, database_equal={}, sets_before={mcp_sets_before:?}, sets_after={:?}, deletes={:?}, rows={:?}",
        rotation_error.message(),
        rotation_store.bytes(&collision_ref),
        reference_after == reference_before,
        database_after == database_before,
        rotation_store.mcp_set_refs(),
        rotation_observation.deletes,
        rows_before_followup,
    );

    {
        let owner = active
            .state
            .mcp_audit_store
            .try_lock()
            .expect("rotation collision must not poison the live writer");
        d064_insert(&owner, "after_rotation_collision");
        assert!(d064_logs_complete(
            &owner.list_logs(10).unwrap(),
            &["before_rotation_collision", "after_rotation_collision"]
        ));
    }
}

#[tokio::test]
async fn d064_post_begin_rotation_failures_fail_closed_with_typed_durable_outcome() {
    #[derive(Clone, Copy)]
    struct Scenario {
        name: &'static str,
        stage: McpAuditRotationPostBeginStage,
        kind: McpAuditRotationFaultKind,
        expected_outcome: &'static str,
    }

    let scenarios = [
        Scenario {
            name: "pending_precommit_drift_makes_exact_abort_fail",
            stage: McpAuditRotationPostBeginStage::PendingPrecommit,
            kind: McpAuditRotationFaultKind::ReferenceDrift,
            expected_outcome: "unknown",
        },
        Scenario {
            name: "pending_reference_drift",
            stage: McpAuditRotationPostBeginStage::PendingReference,
            kind: McpAuditRotationFaultKind::ReferenceDrift,
            expected_outcome: "unknown",
        },
        Scenario {
            name: "verified_reference_drift",
            stage: McpAuditRotationPostBeginStage::VerifiedReference,
            kind: McpAuditRotationFaultKind::ReferenceDrift,
            expected_outcome: "unknown",
        },
        Scenario {
            name: "attempted_reference_drift",
            stage: McpAuditRotationPostBeginStage::AttemptedReference,
            kind: McpAuditRotationFaultKind::ReferenceDrift,
            expected_outcome: "unknown",
        },
        Scenario {
            name: "database_authority_entry_failure",
            stage: McpAuditRotationPostBeginStage::DatabaseAuthority,
            kind: McpAuditRotationFaultKind::ReturnError,
            expected_outcome: "unknown",
        },
        Scenario {
            name: "active_construction_failure_after_database_commit",
            stage: McpAuditRotationPostBeginStage::ActiveConstruction,
            kind: McpAuditRotationFaultKind::ReturnError,
            expected_outcome: "database_committed",
        },
        Scenario {
            name: "active_permit_reference_drift_after_database_commit",
            stage: McpAuditRotationPostBeginStage::ActivePermit,
            kind: McpAuditRotationFaultKind::ReferenceDrift,
            expected_outcome: "database_committed",
        },
        Scenario {
            name: "active_reference_write_preflight_drift_after_database_commit",
            stage: McpAuditRotationPostBeginStage::ActiveReference,
            kind: McpAuditRotationFaultKind::ReferenceDrift,
            expected_outcome: "database_committed",
        },
        Scenario {
            name: "active_install_reference_drift_after_database_commit",
            stage: McpAuditRotationPostBeginStage::LiveInstall,
            kind: McpAuditRotationFaultKind::ReferenceDrift,
            expected_outcome: "database_committed",
        },
    ];

    for (index, scenario) in scenarios.into_iter().enumerate() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = std::sync::Arc::new(D064RecordingSecretStore::default());
        let active_epoch = 6500 + (index as u64 * 2);
        let rotation_epoch = active_epoch + 1;
        let active_epoch_guard = inject_fixed_mcp_audit_epoch_for_test(active_epoch);
        let product =
            bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), secrets.as_ref());
        drop(active_epoch_guard);
        product.state.persistence_coordinator.seal();
        let initial_snapshot = product.state.persistence_coordinator.snapshot();
        assert!(
            initial_snapshot.sealed
                && product
                    .state
                    .persistence_coordinator
                    .require_effects_allowed()
                    .is_ok(),
            "{} must start as a sealed product-like ReadWrite runtime: {initial_snapshot:?}",
            scenario.name
        );
        {
            let owner = product
                .state
                .mcp_audit_store
                .try_lock()
                .expect("lock D064 post-begin failure fixture");
            d064_insert(&owner, scenario.name);
        }
        let db_path = directory.path().join("mcp_audit.db");
        let reference_path = directory.path().join("mcp_audit_keys.json");
        let row_count_before = d064_raw_audit_shape(&db_path).0;
        let rotation_epoch_guard = inject_fixed_mcp_audit_epoch_for_test(rotation_epoch);
        let _fault = inject_mcp_audit_rotation_fault_for_test(
            reference_path.clone(),
            scenario.stage,
            scenario.kind,
        );

        let error = rotate_mcp_audit_key_after_confirmation(
            &reference_path,
            secrets.clone(),
            &product.state,
        )
        .await
        .unwrap_err();
        drop(rotation_epoch_guard);

        let snapshot = product.state.persistence_coordinator.snapshot();
        let reference_mode = d064_store_mode(&product, "McpAuditKeyReferenceStore");
        let audit_mode = d064_store_mode(&product, "McpAuditStore");
        let gate_error = product
            .state
            .persistence_coordinator
            .require_effects_allowed()
            .expect_err("every unsafe post-begin failure must block later effects");
        let late_write_error = product
            .state
            .mcp_audit_store
            .try_lock()
            .expect("post-begin failure released runtime owner")
            .insert_log(
                "forbidden_late_tool_effect",
                &serde_json::json!({"scenario": scenario.name}),
                "must-not-commit",
                false,
                false,
            )
            .expect_err("abandoned post-begin transition must poison retained audit writers");
        let row_count_after = d064_raw_audit_shape(&db_path).0;

        assert!(
            snapshot.sealed
                && reference_mode == PersistenceStoreMode::Unavailable
                && audit_mode == PersistenceStoreMode::Unavailable
                && error
                    .message()
                    .contains(&format!("outcome={}", scenario.expected_outcome))
                && row_count_after == row_count_before,
            "D064 RED: {} must be finalized once with both canonical stores unavailable, a typed unknown/committed outcome, a sealed fail-closed coordinator, and zero later audit/tool commit; error={}, expected_outcome={}, snapshot={snapshot:?}, reference_mode={reference_mode:?}, audit_mode={audit_mode:?}, gate_error={gate_error}, late_write_error={late_write_error}, rows_before={row_count_before}, rows_after={row_count_after}",
            scenario.name,
            error.message(),
            scenario.expected_outcome,
        );
    }
}

#[tokio::test]
async fn d064_prepared_not_committed_exact_abort_is_the_only_healthy_post_begin_error() {
    let directory = tempfile::tempdir().unwrap();
    let secrets = std::sync::Arc::new(D064RecordingSecretStore::default());
    let active_epoch_guard = inject_fixed_mcp_audit_epoch_for_test(6520);
    let product =
        bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), secrets.as_ref());
    drop(active_epoch_guard);
    product.state.persistence_coordinator.seal();
    product
        .state
        .persistence_coordinator
        .require_effects_allowed()
        .expect("safe-abort fixture must begin as sealed ReadWrite product state");
    {
        let owner = product
            .state
            .mcp_audit_store
            .try_lock()
            .expect("lock safe-abort product audit owner");
        d064_insert(&owner, "before_safe_not_committed_abort");
    }
    let reference_path = directory.path().join("mcp_audit_keys.json");
    let database_path = directory.path().join("mcp_audit.db");
    let reference_before = std::fs::read(&reference_path).unwrap();
    let database_before = std::fs::read(&database_path).unwrap();
    let mcp_sets_before = secrets.mcp_set_refs();
    let rotation_epoch_guard = inject_fixed_mcp_audit_epoch_for_test(6521);
    let _save_failure = inject_mcp_audit_rotation_fault_for_test(
        reference_path.clone(),
        McpAuditRotationPostBeginStage::PendingWrite,
        McpAuditRotationFaultKind::ReferenceWriteNotCommitted,
    );

    let error =
        rotate_mcp_audit_key_after_confirmation(&reference_path, secrets.clone(), &product.state)
            .await
            .expect_err("mechanical precommit fault must reject rotation");
    drop(rotation_epoch_guard);

    let snapshot = product.state.persistence_coordinator.snapshot();
    let reference_mode = d064_store_mode(&product, "McpAuditKeyReferenceStore");
    let audit_mode = d064_store_mode(&product, "McpAuditStore");
    assert!(
        snapshot.sealed
            && product
                .state
                .persistence_coordinator
                .require_effects_allowed()
                .is_ok()
            && reference_mode == PersistenceStoreMode::ReadWriteCanonical
            && audit_mode == PersistenceStoreMode::ReadWriteCanonical
            && error.message().contains("outcome=not_committed_aborted")
            && std::fs::read(&reference_path).unwrap() == reference_before
            && std::fs::read(&database_path).unwrap() == database_before
            && secrets.mcp_set_refs() == mcp_sets_before,
        "the only healthy post-begin error must prove NotCommitted plus exact abort with byte-stable reference/database/secret state; error={}, snapshot={snapshot:?}, reference_mode={reference_mode:?}, audit_mode={audit_mode:?}, sets_before={mcp_sets_before:?}, sets_after={:?}",
        error.message(),
        secrets.mcp_set_refs(),
    );
    drop(_save_failure);

    let successful_rotation_epoch = inject_fixed_mcp_audit_epoch_for_test(6522);
    rotate_mcp_audit_key_after_confirmation(&reference_path, secrets.clone(), &product.state)
        .await
        .expect("the same product owner must still complete a later governed rotation");
    drop(successful_rotation_epoch);
    let rotated_configs = load_mcp_audit_keyring_from_path(&reference_path);
    assert!(
        product
            .state
            .persistence_coordinator
            .require_effects_allowed()
            .is_ok()
            && d064_store_mode(&product, "McpAuditKeyReferenceStore")
                == PersistenceStoreMode::ReadWriteCanonical
            && d064_store_mode(&product, "McpAuditStore")
                == PersistenceStoreMode::ReadWriteCanonical
            && rotated_configs
                .iter()
                .map(|config| config.epoch)
                .eq([6520, 6522])
            && secrets.mcp_set_refs().len() == mcp_sets_before.len() + 1,
        "safe abort must not create a hidden dead-end: a later successful product rotation retains both ReadWrite owners and creates exactly one new secret; configs={rotated_configs:?}, sets_before={mcp_sets_before:?}, sets_after={:?}",
        secrets.mcp_set_refs(),
    );

    let owner = product
        .state
        .mcp_audit_store
        .try_lock()
        .expect("safe exact abort released the product owner");
    d064_insert(&owner, "after_safe_not_committed_abort");
    assert!(d064_logs_complete(
        &owner.list_logs(10).unwrap(),
        &[
            "before_safe_not_committed_abort",
            "after_safe_not_committed_abort"
        ]
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn d064_pre_rotation_tool_gateway_snapshot_commits_once_with_rotated_epoch() {
    let directory = tempfile::tempdir().unwrap();
    let secrets = std::sync::Arc::new(D064RecordingSecretStore::default());
    let active_epoch_guard = inject_fixed_mcp_audit_epoch_for_test(6524);
    let product =
        bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), secrets.as_ref());
    drop(active_epoch_guard);
    product.state.persistence_coordinator.seal();
    product
        .state
        .persistence_coordinator
        .require_effects_allowed()
        .expect("snapshot/rotation race fixture must begin ReadWrite");

    let captured =
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_execution(
            &product.state,
        )
        .await
        .expect("capture the real Main Chat ToolGateway resources before rotation");

    let reference_path = directory.path().join("mcp_audit_keys.json");
    let database_path = directory.path().join("mcp_audit.db");
    let rotated_epoch_guard = inject_fixed_mcp_audit_epoch_for_test(6525);
    rotate_mcp_audit_key_after_confirmation(&reference_path, secrets, &product.state)
        .await
        .expect("the product audit owner must complete a real key rotation");
    drop(rotated_epoch_guard);

    let captured_writer = captured.governed.shared.audit_store.clone_owned_writer();
    tokio::task::spawn_blocking(move || {
        captured_writer.insert_log_durably(
            "captured_after_rotation",
            &serde_json::json!({"requestId": "d064-captured-after-rotation"}),
            "rotated-generation-result",
            true,
            false,
        )
    })
    .await
    .expect("captured audit commit worker must not panic")
    .expect("a pre-rotation product snapshot must resolve the current writer at commit time");

    let (row_count, row_epoch): (i64, i64) = rusqlite::Connection::open(&database_path)
        .expect("open rotated D064 database mechanically")
        .query_row(
            "SELECT COUNT(*), MIN(key_epoch) FROM mcp_log WHERE tool_name = ?1",
            ["captured_after_rotation"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("inspect exact captured audit commit mechanically");
    let rows = product
        .state
        .mcp_audit_store
        .try_lock()
        .expect("captured commit released the canonical audit owner")
        .list_logs(10)
        .expect("the rotated canonical owner decrypts the captured commit");

    assert_eq!((row_count, row_epoch), (1, 6525));
    assert!(d064_logs_complete(&rows, &["captured_after_rotation"]));
    assert_eq!(
        d064_store_mode(&product, "McpAuditKeyReferenceStore"),
        PersistenceStoreMode::ReadWriteCanonical
    );
    assert_eq!(
        d064_store_mode(&product, "McpAuditStore"),
        PersistenceStoreMode::ReadWriteCanonical
    );
    product
        .state
        .persistence_coordinator
        .require_effects_allowed()
        .expect("a successful resolved audit commit must not degrade persistence");

    let snapshot_source = include_str!("tool_gateway_resources.rs");
    assert!(
        !snapshot_source.contains("state.mcp_audit_store.lock().await.clone()")
            && !snapshot_source.contains(
                "pub(crate) audit_store: openlife_core::mcp_audit::McpAuditStore"
            )
            && snapshot_source.contains("Arc::clone(&state.mcp_audit_store)"),
        "product snapshots must retain only the canonical writer resolver, never a by-value key/authority clone"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn d064_rotation_race_has_one_monotonic_epoch_cutover_for_captured_writers() {
    use std::sync::Arc;

    const WRITERS: usize = 4;
    const WRITES_PER_WRITER: usize = 8;

    let directory = tempfile::tempdir().unwrap();
    let secrets = Arc::new(D064RecordingSecretStore::default());
    let active_epoch_guard = inject_fixed_mcp_audit_epoch_for_test(6528);
    let product =
        bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), secrets.as_ref());
    drop(active_epoch_guard);
    product.state.persistence_coordinator.seal();
    let captured =
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_execution(
            &product.state,
        )
        .await
        .expect("capture all race writers before rotation");
    let captured_writer = captured.governed.shared.audit_store.clone_owned_writer();
    let start = Arc::new(tokio::sync::Barrier::new(WRITERS + 1));

    let rotation_state = Arc::clone(&product.state);
    let rotation_secrets = Arc::clone(&secrets);
    let rotation_reference_path = directory.path().join("mcp_audit_keys.json");
    let rotation_start = Arc::clone(&start);
    let rotation = tokio::spawn(async move {
        rotation_start.wait().await;
        rotate_mcp_audit_key_after_confirmation(
            &rotation_reference_path,
            rotation_secrets,
            &rotation_state,
        )
        .await
    });

    let mut writers = Vec::with_capacity(WRITERS);
    for writer_id in 0..WRITERS {
        let writer = Arc::clone(&captured_writer);
        let writer_start = Arc::clone(&start);
        writers.push(tokio::spawn(async move {
            writer_start.wait().await;
            for index in 0..WRITES_PER_WRITER {
                let operation_writer = Arc::clone(&writer);
                tokio::task::spawn_blocking(move || {
                    operation_writer.insert_log_durably(
                        &format!("rotation_race_{writer_id}_{index}"),
                        &serde_json::json!({"writer": writer_id, "index": index}),
                        "rotation-race-result",
                        true,
                        false,
                    )
                })
                .await
                .expect("race audit worker must not panic")
                .expect("race audit commit must resolve one canonical generation");
                tokio::task::yield_now().await;
            }
        }));
    }

    rotation
        .await
        .expect("rotation task must not panic")
        .expect("concurrent governed rotation must complete");
    for writer in writers {
        writer.await.expect("captured writer task must not panic");
    }

    let reference_path = directory.path().join("mcp_audit_keys.json");
    let rotated_epoch = load_mcp_audit_keyring_from_path(&reference_path)
        .last()
        .expect("rotation publishes a new active config")
        .epoch;
    assert!(rotated_epoch > 6528);
    let database_path = directory.path().join("mcp_audit.db");
    let connection = rusqlite::Connection::open(&database_path)
        .expect("open race database for mechanical epoch ordering proof");
    let mut statement = connection
        .prepare(
            "SELECT key_epoch FROM mcp_log
             WHERE tool_name LIKE 'rotation_race_%'
             ORDER BY id ASC",
        )
        .expect("prepare race epoch query");
    let epochs = statement
        .query_map([], |row| row.get::<_, u64>(0))
        .expect("query race epochs")
        .collect::<std::result::Result<Vec<_>, _>>()
        .expect("read every race epoch");
    assert_eq!(epochs.len(), WRITERS * WRITES_PER_WRITER);
    let mut observed_rotated_epoch = false;
    for epoch in epochs {
        assert!(epoch == 6528 || epoch == rotated_epoch);
        if epoch == rotated_epoch {
            observed_rotated_epoch = true;
        } else {
            assert!(
                !observed_rotated_epoch,
                "an old-epoch commit cannot linearize after the new generation is observed"
            );
        }
    }

    let post_rotation_writer = Arc::clone(&captured_writer);
    let post_rotation_epoch = tokio::task::spawn_blocking(move || {
        post_rotation_writer.insert_log_durably(
            "rotation_race_post_barrier",
            &serde_json::json!({"phase": "post_rotation"}),
            "post-rotation-result",
            true,
            false,
        )
    })
    .await
    .expect("post-rotation worker must not panic")
    .expect("captured writer remains usable after rotation");
    let committed_epoch: u64 = connection
        .query_row(
            "SELECT key_epoch FROM mcp_log WHERE id = ?1",
            [post_rotation_epoch],
            |row| row.get(0),
        )
        .expect("read post-rotation row epoch");
    assert_eq!(committed_epoch, rotated_epoch);
    assert_eq!(
        d064_store_mode(&product, "McpAuditStore"),
        PersistenceStoreMode::ReadWriteCanonical
    );
    product
        .state
        .persistence_coordinator
        .require_effects_allowed()
        .expect("successful race linearization must preserve ReadWrite admission");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn d064_canonical_reporter_degrades_once_from_tokio_prestart_cancel_drop() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CancelDropReporter {
        writer: Arc<dyn McpAuditDurableWriter>,
        reports: Arc<AtomicUsize>,
    }

    impl Drop for CancelDropReporter {
        fn drop(&mut self) {
            self.writer.report_runtime_failure(
                "mcp_audit_blocking_worker_start_unknown_after_caller_cancelled",
                "injected D064 pre-start cancellation",
            );
            self.reports.fetch_add(1, Ordering::AcqRel);
        }
    }

    let directory = tempfile::tempdir().unwrap();
    let secrets = D064RecordingSecretStore::default();
    let active_epoch_guard = inject_fixed_mcp_audit_epoch_for_test(6526);
    let product = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
    drop(active_epoch_guard);
    product.state.persistence_coordinator.seal();
    product
        .state
        .persistence_coordinator
        .require_effects_allowed()
        .expect("pre-start cancellation fixture must begin ReadWrite");
    let resources =
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_execution(
            &product.state,
        )
        .await
        .expect("capture canonical reporter authority");
    let writer = resources.governed.shared.audit_store.clone_owned_writer();
    let reports = Arc::new(AtomicUsize::new(0));
    let (armed_tx, armed_rx) = tokio::sync::oneshot::channel();
    let cancelled_writer = Arc::clone(&writer);
    let cancelled_reports = Arc::clone(&reports);
    let caller = tokio::spawn(async move {
        let _reporter = CancelDropReporter {
            writer: cancelled_writer,
            reports: cancelled_reports,
        };
        let _ = armed_tx.send(());
        std::future::pending::<()>().await;
    });
    armed_rx
        .await
        .expect("arm reporter inside a real Tokio task before cancellation");
    caller.abort();
    let cancelled = caller
        .await
        .expect_err("pre-start caller must remain cancelled after Drop reporting");
    assert!(cancelled.is_cancelled());
    assert_eq!(reports.load(Ordering::Acquire), 1);

    let first_health = product
        .state
        .persistence_coordinator
        .snapshot()
        .stores
        .into_iter()
        .find(|health| health.store == "McpAuditStore")
        .expect("MCP audit health remains registered");
    assert_eq!(first_health.mode, PersistenceStoreMode::Unavailable);
    assert_eq!(
        first_health.reason_code.as_deref(),
        Some("mcp_audit_blocking_worker_start_unknown_after_caller_cancelled")
    );

    writer.report_runtime_failure(
        "mcp_audit_second_report_must_not_replace_first",
        "injected duplicate report",
    );
    let final_health = product
        .state
        .persistence_coordinator
        .snapshot()
        .stores
        .into_iter()
        .find(|health| health.store == "McpAuditStore")
        .expect("MCP audit health remains registered after duplicate report");
    assert_eq!(final_health, first_health);
    product
        .state
        .persistence_coordinator
        .require_effects_allowed()
        .expect_err("pre-start audit outcome unknown must block later effects");
}

#[test]
fn d064_two_store_identities_same_epoch_have_distinct_refs_and_restart_both_dbs() {
    let directory = tempfile::tempdir().unwrap();
    let profile_a = directory.path().join("profile-a");
    let profile_b = directory.path().join("profile-b");
    std::fs::create_dir_all(&profile_a).unwrap();
    std::fs::create_dir_all(&profile_b).unwrap();
    let secrets = D064RecordingSecretStore::default();
    let epoch = 6402;
    let _fixed_epoch = inject_fixed_mcp_audit_epoch_for_test(epoch);

    let bootstrap_a = bootstrap_with_secret_store_for_test(profile_a.clone(), &secrets);
    {
        let owner_a = bootstrap_a
            .state
            .mcp_audit_store
            .try_lock()
            .expect("lock profile A product audit owner");
        d064_insert(&owner_a, "profile_a_tool");
    }
    drop(bootstrap_a);
    let configs_a = load_mcp_audit_keyring_from_path(&profile_a.join("mcp_audit_keys.json"));
    let ref_a = configs_a
        .iter()
        .find(|config| config.epoch == epoch)
        .and_then(|config| config.key_ref.clone())
        .expect("profile A product bootstrap persists fixed audit epoch");

    let bootstrap_b = bootstrap_with_secret_store_for_test(profile_b.clone(), &secrets);
    {
        let owner_b = bootstrap_b
            .state
            .mcp_audit_store
            .try_lock()
            .expect("lock profile B product audit owner");
        d064_insert(&owner_b, "profile_b_tool");
    }
    drop(bootstrap_b);
    let configs_b = load_mcp_audit_keyring_from_path(&profile_b.join("mcp_audit_keys.json"));
    let ref_b = configs_b
        .iter()
        .find(|config| config.epoch == epoch)
        .and_then(|config| config.key_ref.clone())
        .expect("profile B product bootstrap persists fixed audit epoch");

    let sets_before_restart = secrets.observation().sets.len();
    let mcp_sets_before_restart = secrets.mcp_set_refs();
    let restarted_a = bootstrap_with_secret_store_for_test(profile_a.clone(), &secrets);
    let restarted_a_reference_mode = d064_store_mode(&restarted_a, "McpAuditKeyReferenceStore");
    let restarted_a_mode = d064_store_mode(&restarted_a, "McpAuditStore");
    let restarted_a_rows = restarted_a
        .state
        .mcp_audit_store
        .try_lock()
        .expect("lock restarted profile A product audit owner")
        .list_logs(10)
        .expect("read profile A through restarted product owner");
    drop(restarted_a);
    let restarted_b = bootstrap_with_secret_store_for_test(profile_b.clone(), &secrets);
    let restarted_b_reference_mode = d064_store_mode(&restarted_b, "McpAuditKeyReferenceStore");
    let restarted_b_mode = d064_store_mode(&restarted_b, "McpAuditStore");
    let restarted_b_rows = restarted_b
        .state
        .mcp_audit_store
        .try_lock()
        .expect("lock restarted profile B product audit owner")
        .list_logs(10)
        .expect("read profile B through restarted product owner");
    drop(restarted_b);
    let observation = secrets.observation();

    assert!(
        ref_a != ref_b
            && secrets.bytes(&ref_a).is_some()
            && secrets.bytes(&ref_b).is_some()
            && restarted_a_reference_mode == PersistenceStoreMode::ReadWriteCanonical
            && restarted_a_mode == PersistenceStoreMode::ReadWriteCanonical
            && restarted_b_reference_mode == PersistenceStoreMode::ReadWriteCanonical
            && restarted_b_mode == PersistenceStoreMode::ReadWriteCanonical
            && d064_logs_complete(&restarted_a_rows, &["profile_a_tool"])
            && d064_logs_complete(&restarted_b_rows, &["profile_b_tool"])
            && observation.sets.len() == sets_before_restart
            && secrets.mcp_set_refs() == mcp_sets_before_restart,
        "D064 RED: two product bootstraps with distinct canonical data stores must disambiguate equal numeric epochs, then restart through the same bootstrap path and RecordingSecretStore without any new secret set; ref_a={ref_a}, ref_b={ref_b}, restarted_a_reference_mode={restarted_a_reference_mode:?}, restarted_a_mode={restarted_a_mode:?}, restarted_b_reference_mode={restarted_b_reference_mode:?}, restarted_b_mode={restarted_b_mode:?}, restarted_a_rows={restarted_a_rows:?}, restarted_b_rows={restarted_b_rows:?}, sets_before_restart={sets_before_restart}, sets_after={:?}",
        observation.sets,
    );
}

#[test]
fn d064_first_writable_store_holds_os_slot_and_second_owner_is_typed_unavailable() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("mcp_audit.db");
    let material = d064_material(
        6403,
        "keychain://com.openlife.desktop/mcp-audit-key-store-owner-epoch-6403",
        [0x33; 32],
    );
    let first = McpAuditStore::with_key_materials(&db_path, vec![material.clone()])
        .expect("create first writable D064 owner");
    d064_insert(&first, "first_owner_tool");
    let owner_lock_path = directory.path().join("mcp_audit.db.openlife-owner.lock");
    let direct_os_probe = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&owner_lock_path)
        .expect("open the exact D064 OS owner-lock inode");
    let direct_os_probe_error = direct_os_probe
        .try_lock_exclusive()
        .err()
        .map(|error| error.to_string());
    let second = McpAuditStore::with_key_materials(&db_path, vec![material]);
    let (second_error, second_owner) = match second {
        Ok(owner) => (None, Some(owner)),
        Err(error) => (Some(error.to_string()), None),
    };
    let first_rows = first.list_logs(10).expect("first owner remains readable");
    let typed_lease_code = "mcp_audit_store_sqlite_slot_owner_lease_unavailable";
    drop(second_owner);
    drop(direct_os_probe);
    drop(first);

    let post_drop_probe = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&owner_lock_path)
        .expect("reopen D064 OS owner-lock inode after owner drop");
    let post_drop_reacquired = post_drop_probe.try_lock_exclusive().is_ok();

    assert!(
        direct_os_probe_error.is_some()
            && second_error
                .as_deref()
                .is_some_and(|error| error.contains(typed_lease_code))
            && d064_logs_complete(&first_rows, &["first_owner_tool"])
            && post_drop_reacquired,
        "D064 RED: a live first store must own the real OS file-lock inode, reject a second owner with typed unavailable, and release after drop; direct_probe_error={direct_os_probe_error:?}, second_error={second_error:?}, first_rows={}, post_drop_reacquired={post_drop_reacquired}",
        first_rows.len(),
    );
}

#[test]
fn d064_same_epoch_two_live_writers_cannot_mix_unrecoverable_ciphertext() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("mcp_audit.db");
    let epoch = 6404;
    let material_a = d064_material(
        epoch,
        "keychain://com.openlife.desktop/mcp-audit-key-store-a-epoch-6404",
        [0xA4; 32],
    );
    let material_b = d064_material(
        epoch,
        "keychain://com.openlife.desktop/mcp-audit-key-store-b-epoch-6404",
        [0xB4; 32],
    );
    let owner_a = McpAuditStore::with_key_materials(&db_path, vec![material_a.clone()])
        .expect("construct owner A before either write");
    let owner_b = McpAuditStore::with_key_materials(&db_path, vec![material_b.clone()]);

    let owner_b = match owner_b {
        Err(error) => {
            let error = error.to_string();
            assert!(
                error.contains("mcp_audit_store_sqlite_slot_owner_lease_unavailable"),
                "second-owner prevention must be typed: {error}"
            );
            d064_insert(&owner_a, "owner_a_only_tool");
            drop(owner_a);
            assert_eq!(d064_raw_audit_shape(&db_path), (1, 1));
            return;
        }
        Ok(owner_b) => owner_b,
    };

    d064_insert(&owner_a, "owner_a_mixed_tool");
    d064_insert(&owner_b, "owner_b_mixed_tool");
    drop(owner_a);
    drop(owner_b);

    let raw_shape = d064_raw_audit_shape(&db_path);
    assert_eq!(
        raw_shape,
        (2, 1),
        "the RED fixture must mechanically prove two ciphertext rows share one numeric epoch"
    );
    let restarted_with_a = d064_restart_observation(&db_path, vec![material_a]);
    let restarted_with_b = d064_restart_observation(&db_path, vec![material_b]);
    let expected_tools = ["owner_a_mixed_tool", "owner_b_mixed_tool"];

    assert!(
        restarted_with_a.complete_for(&expected_tools)
            || restarted_with_b.complete_for(&expected_tools),
        "D064 RED: both writers were accepted and real SQLite contains two same-epoch rows, but neither key can recover both; raw_shape={raw_shape:?}, with_a={restarted_with_a:?}, with_b={restarted_with_b:?}"
    );
}

#[test]
fn d064_legacy_global_ref_is_read_only_and_fails_closed_without_d057_cutover_proof() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("mcp_audit.db");
    let keyring_path = directory.path().join("mcp_audit_keys.json");
    let secrets = D064RecordingSecretStore::default();
    let legacy_epoch = 6405;
    let legacy_ref = format!("{MCP_AUDIT_KEY_REF_PREFIX}{legacy_epoch}");
    let legacy_key = [0x55; 32];
    secrets.preload_key(&legacy_ref, legacy_key);
    let legacy_config = d064_keychain_config(legacy_epoch, legacy_ref.clone());
    save_mcp_audit_keyring_to_path(&keyring_path, std::slice::from_ref(&legacy_config))
        .expect("persist historical global reference fixture");
    let legacy_bytes_before = secrets.bytes(&legacy_ref);
    McpAuditStore::write_historical_keychain_fixture(
        &db_path,
        d064_material(legacy_epoch, legacy_ref.clone(), legacy_key),
        "legacy_global_tool",
        &serde_json::json!({"fixture": "legacy_global_tool"}),
        "result-legacy_global_tool",
        true,
        false,
    )
    .expect("seed real historical legacy-global ciphertext without product write authority");

    let reference_bytes_before = std::fs::read(&keyring_path).unwrap();
    let database_shape_before = d064_raw_audit_shape(&db_path);

    // Evidence boundary retained from the first natural post-b359 run: this
    // case was one of the two failures in the 7/9 result because it previously
    // expected a D057 legacy cutover that does not exist. D064 receives only
    // fail-closed/read-only credit here; no migration or new-write credit is
    // awarded until a separately reviewed D057 proof consumer exists.
    let result = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
    let reference_mode = d064_store_mode(&result, "McpAuditKeyReferenceStore");
    let audit_mode = d064_store_mode(&result, "McpAuditStore");
    let audit_error = result
        .state
        .mcp_audit_store
        .try_lock()
        .expect("D064 legacy unavailable store is not concurrently held")
        .list_logs(10)
        .expect_err("legacy authority must remain unavailable without D057 cutover proof")
        .to_string();
    drop(result);

    let observation = secrets.observation();

    assert!(
        legacy_bytes_before == secrets.bytes(&legacy_ref)
            && secrets.mcp_set_refs().is_empty()
            && secrets.mcp_deleted_refs().is_empty()
            && std::fs::read(&keyring_path).unwrap() == reference_bytes_before
            && d064_raw_audit_shape(&db_path) == database_shape_before
            && reference_mode == PersistenceStoreMode::Unavailable
            && audit_mode == PersistenceStoreMode::Unavailable
            && audit_error.contains("mcp_audit_legacy_authority_cutover_proof_required"),
        "D064 partial: legacy global refs and ciphertext must remain byte-stable and effect-free while the explicit D057 cutover blocker keeps product audit unavailable; legacy_ref={legacy_ref}, reference_mode={reference_mode:?}, audit_mode={audit_mode:?}, audit_error={audit_error}, sets={:?}, deletes={:?}",
        observation.sets,
        observation.deletes,
    );
}

#[test]
fn d064_owner_reservation_failure_precedes_secret_reference_and_database_creation() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("mcp_audit.db");
    let keyring_path = directory.path().join("mcp_audit_keys.json");
    let owner_lock_path = directory.path().join("mcp_audit.db.openlife-owner.lock");
    let owner_lock = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&owner_lock_path)
        .expect("open real D064 owner lock fixture");
    owner_lock
        .try_lock_exclusive()
        .expect("hold competing real OS owner lease before bootstrap");
    assert!(
        !db_path.exists(),
        "reservation fixture must start without DB"
    );
    let secrets = D064RecordingSecretStore::default();

    let result = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
    let reference_mode = d064_store_mode(&result, "McpAuditKeyReferenceStore");
    let audit_mode = d064_store_mode(&result, "McpAuditStore");
    let reads_fail_closed = d064_audit_reads_fail_closed(&result);
    let observation = secrets.observation();

    assert!(
        !db_path.exists()
            && !keyring_path.exists()
            && secrets.mcp_set_refs().is_empty()
            && secrets.mcp_deleted_refs().is_empty()
            && secrets.live_mcp_refs().is_empty()
            && reference_mode == PersistenceStoreMode::Unavailable
            && audit_mode == PersistenceStoreMode::Unavailable
            && reads_fail_closed,
        "D064 RED: no-create owner reservation must precede key generation, reference save and DB creation; db_exists={}, keyring_exists={}, reference_mode={reference_mode:?}, audit_mode={audit_mode:?}, reads_fail_closed={reads_fail_closed}, sets={:?}, deletes={:?}",
        db_path.exists(),
        keyring_path.exists(),
        observation.sets,
        observation.deletes,
    );
}

#[test]
fn d064_initial_reference_save_failure_precedes_mcp_secret_and_database_creation() {
    let directory = tempfile::tempdir().unwrap();
    let keyring_path = directory.path().join("mcp_audit_keys.json");
    let db_path = directory.path().join("mcp_audit.db");
    let secrets = D064RecordingSecretStore::default();
    assert!(
        !keyring_path.exists(),
        "D064 save-failure fixture must be genuinely missing during load"
    );

    let _save_failure = inject_mcp_audit_keyring_save_failure_for_test(keyring_path.clone());
    let result = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
    let set_refs = secrets.mcp_set_refs();
    let deleted_refs = secrets.mcp_deleted_refs();
    let reference_mode = d064_store_mode(&result, "McpAuditKeyReferenceStore");
    let audit_mode = d064_store_mode(&result, "McpAuditStore");
    let reads_fail_closed = d064_audit_reads_fail_closed(&result);
    let observation = secrets.observation();

    // Evidence boundary retained from the first natural post-b359 run: this was
    // the second failure in the 7/9 result. The old assertion expected a secret
    // set followed by rollback, but the current two-phase protocol publishes
    // Pending before the credential effect. Therefore an initial reference
    // precommit failure must produce zero MCP credential set/delete operations.
    // Other bootstrap authority credentials are outside this D064 invariant and
    // receive no MCP-audit credit.
    assert!(
        set_refs.is_empty()
            && deleted_refs.is_empty()
            && secrets.live_mcp_refs().is_empty()
            && !keyring_path.exists()
            && !db_path.exists()
            && reference_mode == PersistenceStoreMode::Unavailable
            && audit_mode == PersistenceStoreMode::Unavailable
            && reads_fail_closed,
        "D064: initial reference precommit failure must occur before any MCP credential or database effect; db_exists={}, live_refs={:?}, mcp_set_refs={set_refs:?}, mcp_deleted_refs={deleted_refs:?}, reference_mode={reference_mode:?}, audit_mode={audit_mode:?}, reads_fail_closed={reads_fail_closed}, all_sets={:?}, all_deletes={:?}",
        db_path.exists(),
        secrets.live_mcp_refs(),
        observation.sets,
        observation.deletes,
    );
}

#[test]
fn d064_single_owner_real_sqlite_write_and_restart_positive_control() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory.path().join("mcp_audit.db");
    let identity = uuid::Uuid::parse_str("d3cfad38-ac4d-4526-aa52-0e1236eefadb").unwrap();
    let secrets = D064RecordingSecretStore::default();
    let _identity = inject_fixed_mcp_audit_store_identity_for_test(identity);
    let _epoch = inject_fixed_mcp_audit_epoch_for_test(6406);
    let first = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
    {
        let store = first
            .state
            .mcp_audit_store
            .try_lock()
            .expect("open single-owner product control DB");
        d064_insert(&store, "single_owner_control_tool");
    }
    drop(first);

    assert_eq!(d064_raw_audit_shape(&db_path), (1, 1));
    let sets_before_restart = secrets.observation().sets.len();
    let restarted = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
    let reference_mode = d064_store_mode(&restarted, "McpAuditKeyReferenceStore");
    let audit_mode = d064_store_mode(&restarted, "McpAuditStore");
    let rows = restarted
        .state
        .mcp_audit_store
        .try_lock()
        .expect("lock restarted single-owner product control")
        .list_logs(10)
        .expect("read old row after complete product restart");
    assert!(
        reference_mode == PersistenceStoreMode::ReadWriteCanonical
            && audit_mode == PersistenceStoreMode::ReadWriteCanonical
            && secrets.observation().sets.len() == sets_before_restart
            && d064_logs_complete(&rows, &["single_owner_control_tool"]),
        "single-owner control must prove the real bootstrap/drop/bootstrap path reuses the same secret owner, remains ReadWrite, and reads the old row without another set: reference_mode={reference_mode:?}, audit_mode={audit_mode:?}, sets_before_restart={sets_before_restart}, sets_after={}, rows={rows:?}",
        secrets.observation().sets.len(),
    );

    restarted.state.persistence_coordinator.seal();
    restarted
        .state
        .persistence_coordinator
        .require_effects_allowed()
        .expect("healthy restarted product allows effects before runtime audit failure");
    restarted
        .state
        .mcp_audit_store
        .try_lock()
        .expect("report runtime audit failure through product-owned store")
        .report_runtime_failure(
            "mcp_audit_runtime_durable_write_failed",
            "injected runtime audit durability loss",
        );
    let runtime_reference_mode = d064_store_mode(&restarted, "McpAuditKeyReferenceStore");
    let runtime_audit_mode = d064_store_mode(&restarted, "McpAuditStore");
    let gate_error = restarted
        .state
        .persistence_coordinator
        .require_effects_allowed()
        .expect_err("runtime audit failure must block every later effect");
    restarted
        .state
        .persistence_coordinator
        .register_read_write("McpAuditStore");
    assert!(
        runtime_reference_mode == PersistenceStoreMode::ReadWriteCanonical
            && runtime_audit_mode == PersistenceStoreMode::Unavailable
            && d064_store_mode(&restarted, "McpAuditStore")
                == PersistenceStoreMode::Unavailable,
        "the bootstrapped runtime observer must degrade only MCP audit durability, block later effects, and reject an in-process health upgrade: reference_mode={runtime_reference_mode:?}, audit_mode={runtime_audit_mode:?}, gate_error={gate_error}"
    );
}

#[test]
#[ignore = "performance evidence must run isolated from parallel test I/O"]
fn d064_persistent_product_insert_log_latency_and_throughput_budget() {
    const WARMUP_INSERTS: usize = 16;
    const MEASURED_INSERTS: usize = 256;
    // Frozen from the median of three consecutive local persistent runs on
    // 2026-07-14. The absolute budgets and baseline ratios both apply, so a
    // faster machine cannot hide a large relative regression and a stale
    // baseline cannot authorize an unbounded synchronous stall.
    const FROZEN_BASELINE_P50_US: f64 = 1_115.291;
    const FROZEN_BASELINE_P95_US: f64 = 1_404.083;
    const FROZEN_BASELINE_THROUGHPUT_PER_SEC: f64 = 878.439;
    const P50_BUDGET_US: f64 = 5_000.0;
    const P95_BUDGET_US: f64 = 7_500.0;
    const THROUGHPUT_BUDGET_PER_SEC: f64 = 200.0;
    const MAX_BASELINE_LATENCY_MULTIPLIER: f64 = 4.0;
    const MIN_BASELINE_THROUGHPUT_RATIO: f64 = 0.25;

    let directory = tempfile::tempdir().unwrap();
    let secrets = D064RecordingSecretStore::default();
    let identity = uuid::Uuid::parse_str("1b135e06-aa86-40df-af54-cd0345ebc18f").unwrap();
    let _identity = inject_fixed_mcp_audit_store_identity_for_test(identity);
    let _epoch = inject_fixed_mcp_audit_epoch_for_test(6530);
    let product = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
    product.state.persistence_coordinator.seal();
    product
        .state
        .persistence_coordinator
        .require_effects_allowed()
        .expect("benchmark requires the real sealed product persistence gate");

    // Frozen realistic sample: a bounded network-tool receipt with request
    // metadata plus a non-trivial result. `insert_log` still executes its real
    // minimization, SHA-256 receipts, AES-256-GCM encryption, authority proof,
    // transaction commit, and persistent SQLite identity checks.
    let arguments = serde_json::json!({
        "requestId": "d064-benchmark-request-0001",
        "capability": "network.read",
        "method": "GET",
        "url": "https://docs.example.test/openlife/runtime?section=audit",
        "headers": {
            "accept": "application/json",
            "user-agent": "OpenLife-D064-Persistent-Benchmark/1.0"
        },
        "timeoutMs": 12_000,
        "maxResponseBytes": 262_144,
        "policyDecisionId": "policy-d064-benchmark-allow-read"
    });
    let result = "status=200 content-type=application/json bytes=2048 digest=sha256:fc40c705f5eaf7646d5dfb3f6f8e59996f8d9f55ed80dfc0147a46fe246122cb receipt={\"items\":[{\"title\":\"OpenLife runtime authority\",\"summary\":\"Provider, policy, tool, and persistence facts remain independently attributable.\"},{\"title\":\"Durable audit receipt\",\"summary\":\"The audit owner stores bounded encrypted receipts and confirms the SQLite transaction before returning.\"}],\"continuation\":null}";
    let owner = product
        .state
        .mcp_audit_store
        .try_lock()
        .expect("benchmark owns the sole product MCP audit writer");
    for index in 0..WARMUP_INSERTS {
        owner
            .insert_log(
                "network_fetch",
                &arguments,
                result,
                index % 7 != 0,
                index % 11 == 0,
            )
            .expect("warmup insert_log must durably commit");
    }

    let mut latencies = Vec::with_capacity(MEASURED_INSERTS);
    let wall_start = std::time::Instant::now();
    for index in 0..MEASURED_INSERTS {
        let started = std::time::Instant::now();
        owner
            .insert_log(
                "network_fetch",
                &arguments,
                result,
                index % 7 != 0,
                index % 11 == 0,
            )
            .expect("measured insert_log must durably commit");
        latencies.push(started.elapsed());
    }
    let wall_elapsed = wall_start.elapsed();
    latencies.sort_unstable();
    let percentile_us = |percentile: usize| {
        latencies[(latencies.len() - 1) * percentile / 100].as_secs_f64() * 1_000_000.0
    };
    let p50_us = percentile_us(50);
    let p95_us = percentile_us(95);
    let throughput = MEASURED_INSERTS as f64 / wall_elapsed.as_secs_f64();
    let rows = owner
        .list_logs(WARMUP_INSERTS + MEASURED_INSERTS + 1)
        .expect("benchmark rows must remain decryptable through product owner");
    drop(owner);

    eprintln!(
        "D064 persistent insert_log benchmark: sample_count={MEASURED_INSERTS} p50_us={p50_us:.3} p95_us={p95_us:.3} throughput_per_sec={throughput:.3} baseline_p50_ratio={:.3} baseline_p95_ratio={:.3} baseline_throughput_ratio={:.3}",
        p50_us / FROZEN_BASELINE_P50_US,
        p95_us / FROZEN_BASELINE_P95_US,
        throughput / FROZEN_BASELINE_THROUGHPUT_PER_SEC,
    );

    assert!(
        rows.len() == WARMUP_INSERTS + MEASURED_INSERTS
            && rows.iter().all(|row| {
                row.arguments != DECRYPT_FAILED
                    && row.result != DECRYPT_FAILED
                    && row.arguments.contains("payloadStored")
                    && row.result.contains("payloadStored")
            })
            && p50_us <= P50_BUDGET_US
            && p95_us <= P95_BUDGET_US
            && throughput >= THROUGHPUT_BUDGET_PER_SEC
            && p50_us
                <= FROZEN_BASELINE_P50_US * MAX_BASELINE_LATENCY_MULTIPLIER
            && p95_us
                <= FROZEN_BASELINE_P95_US * MAX_BASELINE_LATENCY_MULTIPLIER
            && throughput
                >= FROZEN_BASELINE_THROUGHPUT_PER_SEC
                    * MIN_BASELINE_THROUGHPUT_RATIO,
        "persistent product insert_log exceeded its frozen synchronous durable budget/baseline or lost readable rows: rows={}, p50_us={p50_us:.3}/{P50_BUDGET_US:.3} baseline={FROZEN_BASELINE_P50_US:.3}, p95_us={p95_us:.3}/{P95_BUDGET_US:.3} baseline={FROZEN_BASELINE_P95_US:.3}, throughput={throughput:.3}/{THROUGHPUT_BUDGET_PER_SEC:.3} baseline={FROZEN_BASELINE_THROUGHPUT_PER_SEC:.3}",
        rows.len(),
    );

    // Non-credit stress observation: four product resource snapshots share
    // the same authenticated writer while a bounded 64 KiB file is repeatedly
    // synced in the same data directory. This does not alter the ordinary-path
    // budget above; it records how lock queueing and shared durable I/O affect
    // the synchronous edge so the spawn_blocking decision is evidence-based.
    const CONCURRENT_WORKERS: usize = 4;
    const INSERTS_PER_WORKER: usize = 32;
    use std::io::{Seek, SeekFrom, Write};
    use std::sync::atomic::{AtomicBool, Ordering};

    let concurrent_store = product
        .state
        .mcp_audit_store
        .try_lock()
        .expect("capture product-like concurrent audit resource")
        .clone();
    let start_barrier = std::sync::Arc::new(std::sync::Barrier::new(CONCURRENT_WORKERS + 2));
    let stop_shared_io = std::sync::Arc::new(AtomicBool::new(false));
    let pressure_barrier = start_barrier.clone();
    let pressure_stop = stop_shared_io.clone();
    let pressure_path = directory.path().join("d064-shared-io-pressure.bin");
    let pressure = std::thread::spawn(move || {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(pressure_path)
            .expect("open bounded shared-I/O pressure file");
        file.set_len(64 * 1024)
            .expect("bound shared-I/O pressure file");
        let block = [0xA5; 64 * 1024];
        pressure_barrier.wait();
        let mut sync_count = 0usize;
        while !pressure_stop.load(Ordering::Acquire) {
            file.seek(SeekFrom::Start(0))
                .expect("seek shared-I/O pressure file");
            file.write_all(&block)
                .expect("write shared-I/O pressure block");
            file.sync_data().expect("sync shared-I/O pressure block");
            sync_count += 1;
        }
        sync_count
    });
    let mut workers = Vec::with_capacity(CONCURRENT_WORKERS);
    for worker_id in 0..CONCURRENT_WORKERS {
        let worker_store = concurrent_store.clone();
        let worker_arguments = arguments.clone();
        let worker_barrier = start_barrier.clone();
        workers.push(std::thread::spawn(move || {
            worker_barrier.wait();
            let mut worker_latencies = Vec::with_capacity(INSERTS_PER_WORKER);
            for index in 0..INSERTS_PER_WORKER {
                let started = std::time::Instant::now();
                worker_store
                    .insert_log(
                        "network_fetch_concurrent",
                        &worker_arguments,
                        result,
                        (worker_id + index) % 7 != 0,
                        (worker_id + index) % 11 == 0,
                    )
                    .expect("concurrent product-like insert_log must durably commit");
                worker_latencies.push(started.elapsed());
            }
            worker_latencies
        }));
    }
    drop(concurrent_store);
    let concurrent_wall_start = std::time::Instant::now();
    start_barrier.wait();
    let mut concurrent_latencies = workers
        .into_iter()
        .flat_map(|worker| worker.join().expect("join concurrent audit writer"))
        .collect::<Vec<_>>();
    let concurrent_elapsed = concurrent_wall_start.elapsed();
    stop_shared_io.store(true, Ordering::Release);
    let pressure_sync_count = pressure.join().expect("join bounded shared-I/O pressure");
    concurrent_latencies.sort_unstable();
    let concurrent_p50_us = concurrent_latencies[(concurrent_latencies.len() - 1) * 50 / 100]
        .as_secs_f64()
        * 1_000_000.0;
    let concurrent_p95_us = concurrent_latencies[(concurrent_latencies.len() - 1) * 95 / 100]
        .as_secs_f64()
        * 1_000_000.0;
    let concurrent_throughput =
        (CONCURRENT_WORKERS * INSERTS_PER_WORKER) as f64 / concurrent_elapsed.as_secs_f64();
    let expected_total =
        WARMUP_INSERTS + MEASURED_INSERTS + CONCURRENT_WORKERS * INSERTS_PER_WORKER;
    let final_rows = product
        .state
        .mcp_audit_store
        .try_lock()
        .expect("read product-like concurrent audit rows")
        .list_logs(expected_total + 1)
        .expect("concurrent product-like audit rows remain decryptable");
    eprintln!(
        "D064 shared-I/O concurrent observation (non-credit): workers={CONCURRENT_WORKERS} inserts_per_worker={INSERTS_PER_WORKER} pressure_syncs={pressure_sync_count} p50_us={concurrent_p50_us:.3} p95_us={concurrent_p95_us:.3} throughput_per_sec={concurrent_throughput:.3}"
    );
    assert!(
        final_rows.len() == expected_total
            && final_rows.iter().all(|row| {
                row.arguments != DECRYPT_FAILED && row.result != DECRYPT_FAILED
            }),
        "shared-I/O concurrent observation lost or corrupted durable audit rows: expected={expected_total}, actual={}",
        final_rows.len(),
    );
}
