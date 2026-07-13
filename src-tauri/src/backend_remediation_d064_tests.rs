use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use openlife_core::mcp_audit::{
    AuditKeyConfig, AuditKeyMaterial, KeyMode, McpAuditStore, McpLogEntry,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::bootstrap::{bootstrap_with_secret_store_for_test, BootstrapResult};
use crate::persistence_coordinator::PersistenceStoreMode;
use crate::secret_store::{
    hydrate_or_create_mcp_audit_keys, inject_fixed_mcp_audit_epoch_for_test,
    write_new_mcp_audit_secret, SecretStore, MCP_AUDIT_KEY_REF_PREFIX,
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

#[test]
fn d064_existing_same_epoch_secret_is_create_only_and_preserves_k1() {
    let store = D064RecordingSecretStore::default();
    let secret_ref =
        "keychain://com.openlife.desktop/mcp-audit-key-store-canonical-alpha-epoch-6401";
    let k2 = general_purpose::STANDARD.encode([0x22; 32]);
    store.preload_key(secret_ref, [0x11; 32]);
    let before = store.get(&secret_ref).unwrap().unwrap().into_bytes();

    let result = write_new_mcp_audit_secret(secret_ref, &k2, &store);
    let after = store.get(&secret_ref).unwrap().unwrap().into_bytes();
    let observation = store.observation();

    assert!(
        result.is_err() && after == before,
        "D064 RED: creating K2 at an occupied store-bound epoch must fail without replacing K1; result_is_ok={}, before={before:?}, k2={:?}, after={after:?}, gets={:?}, sets={:?}",
        result.is_ok(),
        k2.as_bytes(),
        observation.gets,
        observation.sets,
    );
}

#[test]
fn d064_two_store_identities_same_epoch_have_distinct_refs_and_restart_both_dbs() {
    let directory = tempfile::tempdir().unwrap();
    let profile_a = directory.path().join("profile-a");
    let profile_b = directory.path().join("profile-b");
    std::fs::create_dir_all(&profile_a).unwrap();
    std::fs::create_dir_all(&profile_b).unwrap();
    let db_a = profile_a.join("mcp_audit.db");
    let db_b = profile_b.join("mcp_audit.db");
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

    let restarted_a_materials = hydrate_or_create_mcp_audit_keys(configs_a, &secrets)
        .expect("hydrate profile A restart")
        .materials;
    let restarted_b_materials = hydrate_or_create_mcp_audit_keys(configs_b, &secrets)
        .expect("hydrate profile B restart")
        .materials;
    let restarted_a = d064_restart_observation(&db_a, restarted_a_materials);
    let restarted_b = d064_restart_observation(&db_b, restarted_b_materials);
    let observation = secrets.observation();

    assert!(
        ref_a != ref_b
            && secrets.bytes(&ref_a).is_some()
            && secrets.bytes(&ref_b).is_some()
            && restarted_a.complete_for(&["profile_a_tool"])
            && restarted_b.complete_for(&["profile_b_tool"]),
        "D064 RED: two product bootstraps with distinct canonical data stores must disambiguate equal numeric epochs and preserve both real DBs; ref_a={ref_a}, ref_b={ref_b}, restarted_a={restarted_a:?}, restarted_b={restarted_b:?}, sets={:?}",
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
        .try_lock()
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
    let post_drop_reacquired = post_drop_probe.try_lock().is_ok();

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
fn d064_legacy_global_ref_is_read_only_and_new_write_ref_is_store_bound() {
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

    let new_epoch = legacy_epoch + 1;
    let _fixed_epoch = inject_fixed_mcp_audit_epoch_for_test(new_epoch);
    let transition = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
    {
        let transition_owner = transition
            .state
            .mcp_audit_store
            .try_lock()
            .expect("lock product legacy transition owner");
        d064_insert(&transition_owner, "store_bound_new_tool");
    }
    drop(transition);

    let transition_configs = load_mcp_audit_keyring_from_path(&keyring_path);
    let new_ref = transition_configs
        .iter()
        .find(|config| config.epoch == new_epoch)
        .and_then(|config| config.key_ref.clone());
    let restarted_materials = hydrate_or_create_mcp_audit_keys(transition_configs, &secrets)
        .expect("hydrate both generations on restart")
        .materials;
    let restarted = d064_restart_observation(&db_path, restarted_materials);
    let observation = secrets.observation();
    let legacy_was_never_set = observation
        .sets
        .iter()
        .all(|set| set.secret_ref != legacy_ref);
    let legacy_was_never_deleted = observation
        .deletes
        .iter()
        .all(|delete| delete.secret_ref != legacy_ref);
    let new_ref_is_store_bound = new_ref
        .as_deref()
        .is_some_and(|new_ref| new_ref != format!("{MCP_AUDIT_KEY_REF_PREFIX}{new_epoch}"));

    assert!(
        legacy_bytes_before == secrets.bytes(&legacy_ref)
            && legacy_was_never_set
            && legacy_was_never_deleted
            && new_ref_is_store_bound
            && new_ref
                .as_deref()
                .is_some_and(|new_ref| secrets.bytes(new_ref).is_some())
            && restarted.complete_for(&["legacy_global_tool", "store_bound_new_tool"]),
        "D064 RED: product bootstrap must retain legacy global refs only as read-only recovery inputs and create a store-bound write ref; legacy_ref={legacy_ref}, new_ref={new_ref:?}, restarted={restarted:?}, sets={:?}, deletes={:?}",
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
        .try_lock()
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
fn d064_reference_save_failure_rolls_back_secret_and_prevents_database_creation() {
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

    assert!(
        !set_refs.is_empty()
            && deleted_refs == set_refs
            && secrets.live_mcp_refs().is_empty()
            && !keyring_path.exists()
            && !db_path.exists()
            && reference_mode == PersistenceStoreMode::Unavailable
            && audit_mode == PersistenceStoreMode::Unavailable
            && reads_fail_closed,
        "D064 RED: a reference-save failure must roll back the uncommitted secret before audit-store construction; db_exists={}, live_refs={:?}, reference_mode={reference_mode:?}, audit_mode={audit_mode:?}, reads_fail_closed={reads_fail_closed}, sets={:?}, deletes={:?}",
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
    let material = d064_material(
        6406,
        "keychain://com.openlife.desktop/mcp-audit-key-store-control-epoch-6406",
        [0x66; 32],
    );
    let store = McpAuditStore::with_key_materials(&db_path, vec![material.clone()])
        .expect("open single-owner control DB");
    d064_insert(&store, "single_owner_control_tool");
    drop(store);

    assert_eq!(d064_raw_audit_shape(&db_path), (1, 1));
    let restarted = d064_restart_observation(&db_path, vec![material]);
    assert!(
        restarted.complete_for(&["single_owner_control_tool"]),
        "single-owner control must prove the real SQLite/restart fixture is sound: {restarted:?}"
    );
}
