use crate::bootstrap::bootstrap_with_secret_store_for_test;
use crate::persistence_coordinator::PersistenceStoreMode;
use crate::secret_store::{SecretStore, MCP_AUDIT_KEY_REF_PREFIX};
use anyhow::Result;
use base64::Engine as _;
use openlife_core::mcp_audit::{AuditKeyConfig, AuditKeyMaterial, KeyMode, McpAuditStore};
use ring::digest::{digest, SHA256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

const RAW_ARGUMENT_SENTINEL: &str = "D068-BOOTSTRAP-RAW-HEALTH-ARGUMENT-4312";
const RAW_RESULT_SENTINEL: &str = "D068-BOOTSTRAP-RAW-FINANCE-RESULT-8675";

#[derive(Default)]
struct D068SecretState {
    values: HashMap<String, String>,
    sets: Vec<String>,
    deletes: Vec<String>,
}

#[derive(Default)]
struct D068SecretStore {
    state: Mutex<D068SecretState>,
}

impl D068SecretStore {
    fn preload(&self, secret_ref: &str, key: [u8; 32]) {
        self.state.lock().unwrap().values.insert(
            secret_ref.to_string(),
            base64::engine::general_purpose::STANDARD.encode(key),
        );
    }

    fn mcp_mutations(&self) -> (Vec<String>, Vec<String>) {
        let state = self.state.lock().unwrap();
        let filter = |values: &[String]| {
            values
                .iter()
                .filter(|value| value.starts_with(MCP_AUDIT_KEY_REF_PREFIX))
                .cloned()
                .collect::<Vec<_>>()
        };
        (filter(&state.sets), filter(&state.deletes))
    }
}

impl SecretStore for D068SecretStore {
    fn get(&self, secret_ref: &str) -> Result<Option<String>> {
        Ok(self.state.lock().unwrap().values.get(secret_ref).cloned())
    }

    fn set(&self, secret_ref: &str, value: &str) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.sets.push(secret_ref.to_string());
        state
            .values
            .insert(secret_ref.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, secret_ref: &str) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.deletes.push(secret_ref.to_string());
        state.values.remove(secret_ref);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactReceipt {
    name: String,
    len: u64,
    readonly: bool,
    modified_nanos: Option<u128>,
    sha256: String,
}

fn artifact_family(path: &Path) -> Vec<ArtifactReceipt> {
    let parent = path.parent().unwrap();
    let base = path.file_name().and_then(|name| name.to_str()).unwrap();
    let mut receipts = std::fs::read_dir(parent)
        .unwrap()
        .map(|entry| entry.unwrap().path())
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
            let metadata = std::fs::symlink_metadata(&candidate).unwrap();
            let bytes = std::fs::read(&candidate).unwrap();
            ArtifactReceipt {
                name: candidate
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap()
                    .to_string(),
                len: metadata.len(),
                readonly: metadata.permissions().readonly(),
                modified_nanos: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_nanos()),
                sha256: base64::engine::general_purpose::STANDARD_NO_PAD
                    .encode(digest(&SHA256, &bytes).as_ref()),
            }
        })
        .collect::<Vec<_>>();
    receipts.sort_by(|left, right| left.name.cmp(&right.name));
    receipts
}

fn store_mode(result: &crate::bootstrap::BootstrapResult, name: &str) -> PersistenceStoreMode {
    result
        .state
        .persistence_coordinator
        .snapshot()
        .stores
        .into_iter()
        .find(|health| health.store == name)
        .unwrap_or_else(|| panic!("missing D068 persistence health for {name}"))
        .mode
}

#[test]
fn d068_bootstrap_legal_current_payload_remains_available_and_minimized() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let keyring_path = directory.path().join("mcp_audit_keys.json");
    let secret_ref = format!("{MCP_AUDIT_KEY_REF_PREFIX}67");
    let key = [0x67; 32];
    let config = AuditKeyConfig {
        mode: KeyMode::Keychain,
        salt_b64: None,
        env_var: None,
        key_ref: Some(secret_ref.clone()),
        epoch: 67,
        created_at: "2026-07-13T12:00:00Z".into(),
    };
    let store = McpAuditStore::with_key_materials(
        &database_path,
        vec![AuditKeyMaterial {
            config: config.clone(),
            key,
        }],
    )
    .expect("create D068 legal current bootstrap database");
    store
        .insert_log(
            "d068_bootstrap_current_control",
            &serde_json::json!({"health": RAW_ARGUMENT_SENTINEL}),
            RAW_RESULT_SENTINEL,
            true,
            true,
        )
        .expect("insert legal current D068 product payload");
    drop(store);
    let keyring_bytes = serde_json::to_vec_pretty(&vec![config]).unwrap();
    std::fs::write(&keyring_path, &keyring_bytes).unwrap();
    let secrets = D068SecretStore::default();
    secrets.preload(&secret_ref, key);

    let result = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
    let audit_read = result
        .state
        .mcp_audit_store
        .try_lock()
        .expect("D068 audit store lock is not held")
        .list_logs(10)
        .expect("legal current D068 payload remains readable after bootstrap");
    let audit_json = serde_json::to_string(&audit_read).unwrap();

    assert_eq!(
        store_mode(&result, "McpAuditKeyReferenceStore"),
        PersistenceStoreMode::ReadWriteCanonical
    );
    assert_eq!(
        store_mode(&result, "McpAuditStore"),
        PersistenceStoreMode::ReadWriteCanonical
    );
    assert_eq!(audit_read.len(), 1);
    assert!(audit_json.contains("payloadStored"));
    assert!(!audit_json.contains(RAW_ARGUMENT_SENTINEL));
    assert!(!audit_json.contains(RAW_RESULT_SENTINEL));
    assert_eq!(std::fs::read(&keyring_path).unwrap(), keyring_bytes);
    assert_eq!(secrets.mcp_mutations(), (Vec::new(), Vec::new()));
}

#[test]
fn d068_bootstrap_version_flip_preserves_key_authority_and_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("mcp_audit.db");
    let keyring_path = directory.path().join("mcp_audit_keys.json");
    let secret_ref = format!("{MCP_AUDIT_KEY_REF_PREFIX}68");
    let key = [0x68; 32];
    let config = AuditKeyConfig {
        mode: KeyMode::Keychain,
        salt_b64: None,
        env_var: None,
        key_ref: Some(secret_ref.clone()),
        epoch: 68,
        created_at: "2026-07-13T12:00:00Z".into(),
    };
    let store = McpAuditStore::with_key_materials(
        &database_path,
        vec![AuditKeyMaterial {
            config: config.clone(),
            key,
        }],
    )
    .expect("create D068 bootstrap attack database");
    let row_id = store
        .d068_insert_legacy_payload_fixture_for_test(
            "d068_bootstrap_version_flip",
            &serde_json::json!({"health": RAW_ARGUMENT_SENTINEL}),
            RAW_RESULT_SENTINEL,
        )
        .expect("insert source-backed D068 legacy payload");
    store
        .d068_flip_payload_version_to_current_for_test(row_id)
        .expect("flip only the plaintext payload version");
    drop(store);
    let keyring_bytes = serde_json::to_vec_pretty(&vec![config]).unwrap();
    std::fs::write(&keyring_path, &keyring_bytes).unwrap();
    let artifacts_before = artifact_family(&database_path);
    let secrets = D068SecretStore::default();
    secrets.preload(&secret_ref, key);

    let result = bootstrap_with_secret_store_for_test(directory.path().to_path_buf(), &secrets);
    let audit_read = result
        .state
        .mcp_audit_store
        .try_lock()
        .expect("D068 audit store lock is not held")
        .list_logs(10);
    let audit_json = audit_read
        .as_ref()
        .ok()
        .map(serde_json::to_string)
        .transpose()
        .unwrap_or_default()
        .unwrap_or_default();
    let persistence = result.state.persistence_coordinator.snapshot();
    let observation = (
        store_mode(&result, "McpAuditKeyReferenceStore"),
        store_mode(&result, "McpAuditStore"),
        audit_read.is_err(),
        !audit_json.contains(RAW_ARGUMENT_SENTINEL) && !audit_json.contains(RAW_RESULT_SENTINEL),
        !persistence.provider_dispatch_allowed && !persistence.tool_dispatch_allowed,
        std::fs::read(&keyring_path).unwrap() == keyring_bytes,
        secrets.mcp_mutations(),
        artifact_family(&database_path) == artifacts_before,
    );

    assert_eq!(
        observation,
        (
            PersistenceStoreMode::ReadWriteCanonical,
            PersistenceStoreMode::Unavailable,
            true,
            true,
            true,
            true,
            (Vec::new(), Vec::new()),
            true,
        ),
        "D068 RED: a version-flipped legacy ciphertext must not activate product audit truth or mutate the valid key-reference owner, secret, or SQLite family"
    );
}
