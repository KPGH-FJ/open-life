use crate::errors::AppError;
#[cfg(test)]
use openlife_core::atomic_file::AtomicWriteCommitState;
use openlife_core::mcp_audit::{
    load_mcp_audit_reference_for_store, AuditKeyMaterial, McpAuditDurableReferenceReceipt,
    McpAuditFreshDatabaseCreationCapability, McpAuditLegacyReferenceReceipt,
    McpAuditLoadedReference, McpAuditReferenceMutationError, McpAuditReferenceMutationPermit,
    McpAuditRotationTransition, McpAuditStore,
};
#[cfg(test)]
use openlife_core::mcp_audit::{
    AuditKeyConfig, McpAuditLegacyReferenceFormat, MCP_AUDIT_STORE_KEY_REF_PREFIX,
};
use openlife_core::privacy::PrivacyPolicy;
#[cfg(test)]
use serde::{Deserialize, Serialize};

const RELEASE_APP_DIR_NAME: &str = "ai.openlife.app";
const DEV_APP_DIR_NAME: &str = "ai.openlife.app.dev";
const QA_APP_DIR_NAME: &str = "ai.openlife.app.qa";
#[cfg(test)]
const MCP_AUDIT_LEGACY_VERSIONED_DOCUMENT_VERSION: u32 = 1;

pub(crate) use openlife_core::mcp_audit::{
    McpAuditDurableDatabaseState as McpAuditDatabaseTransitionState,
    McpAuditDurableReferenceDocument as McpAuditKeyReferenceDocument,
    McpAuditDurableReferenceOrigin as McpAuditReferenceOrigin,
    McpAuditDurableReferencePhase as McpAuditReferencePhase,
    McpAuditDurableSecretState as McpAuditSecretState,
};

#[cfg(test)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct McpAuditLegacyVersionedReferenceDocument {
    version: u32,
    pub(crate) store_identity: String,
    pub(crate) keys: Vec<AuditKeyConfig>,
}

#[cfg(test)]
impl McpAuditLegacyVersionedReferenceDocument {
    fn new(keys: Vec<AuditKeyConfig>) -> Self {
        Self {
            version: MCP_AUDIT_LEGACY_VERSIONED_DOCUMENT_VERSION,
            store_identity: uuid::Uuid::new_v4().to_string(),
            keys,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.version != MCP_AUDIT_LEGACY_VERSIONED_DOCUMENT_VERSION {
            return Err(format!(
                "mcp_audit_key_reference_version_unsupported:{}",
                self.version
            ));
        }
        let identity = uuid::Uuid::parse_str(&self.store_identity)
            .map_err(|error| format!("mcp_audit_store_identity_invalid:{error}"))?;
        if identity.is_nil() || identity.get_version_num() != 4 {
            return Err("mcp_audit_store_identity_not_random_v4".into());
        }
        validate_mcp_audit_key_configs(&self.keys)
    }
}

/// Tauri deliberately owns no second receipt schema.  This alias is the
/// Core-sealed generation proof returned by the reference effect edge.
pub(crate) type McpAuditReferenceReceipt = McpAuditDurableReferenceReceipt;

#[derive(Debug, Clone)]
pub(crate) enum McpAuditKeyReferenceLoadState {
    Missing,
    Versioned(McpAuditReferenceReceipt),
    Legacy(McpAuditLegacyReferenceReceipt),
    Invalid(String),
    Unreadable(String),
}

#[cfg(test)]
fn validate_mcp_audit_key_configs(configs: &[AuditKeyConfig]) -> Result<(), String> {
    if configs.is_empty() {
        return Err("mcp_audit_key_reference_set_empty".into());
    }
    for pair in configs.windows(2) {
        if pair[0].epoch >= pair[1].epoch {
            return Err(format!(
                "mcp_audit_key_epoch_not_strictly_increasing:{}:{}",
                pair[0].epoch, pair[1].epoch
            ));
        }
    }
    for config in configs {
        if config.mode == openlife_core::mcp_audit::KeyMode::Keychain
            && config
                .key_ref
                .as_deref()
                .map(str::trim)
                .is_none_or(str::is_empty)
        {
            return Err(format!(
                "mcp_audit_keychain_reference_missing:{}",
                config.epoch
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
std::thread_local! {
    static MCP_AUDIT_KEYRING_SAVE_FAILURE_PATH: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// Thread-local fault injection used by D064 to fail the initial durable
/// reference publication. This keeps the keyring path genuinely missing during
/// load and proves that credential creation remains after the reference effect
/// edge, distinct from invalid/unreadable pre-existing reference state.
#[cfg(test)]
pub(crate) struct McpAuditKeyringSaveFailureGuard {
    previous: Option<std::path::PathBuf>,
}

#[cfg(test)]
impl Drop for McpAuditKeyringSaveFailureGuard {
    fn drop(&mut self) {
        MCP_AUDIT_KEYRING_SAVE_FAILURE_PATH.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

#[cfg(test)]
pub(crate) fn inject_mcp_audit_keyring_save_failure_for_test(
    path: std::path::PathBuf,
) -> McpAuditKeyringSaveFailureGuard {
    let previous = MCP_AUDIT_KEYRING_SAVE_FAILURE_PATH.with(|slot| slot.replace(Some(path)));
    McpAuditKeyringSaveFailureGuard { previous }
}

#[cfg(test)]
fn mcp_audit_keyring_save_failure_injected(path: &std::path::Path) -> bool {
    MCP_AUDIT_KEYRING_SAVE_FAILURE_PATH.with(|slot| {
        slot.borrow()
            .as_deref()
            .is_some_and(|injected| injected == path)
    })
}

pub fn openlife_profile() -> String {
    normalize_openlife_profile(std::env::var("OPENLIFE_PROFILE").ok().as_deref()).to_string()
}

pub fn normalize_openlife_profile(value: Option<&str>) -> &'static str {
    match value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("release")
        .to_ascii_lowercase()
        .as_str()
    {
        "dev" => "dev",
        "qa" => "qa",
        _ => "release",
    }
}

pub fn app_dir_name_for_profile(profile: &str) -> &'static str {
    match normalize_openlife_profile(Some(profile)) {
        "dev" => DEV_APP_DIR_NAME,
        "qa" => QA_APP_DIR_NAME,
        _ => RELEASE_APP_DIR_NAME,
    }
}

pub fn app_data_dir() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("OPENLIFE_DATA_DIR") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return std::path::PathBuf::from(trimmed);
        }
    }

    let profile = openlife_profile();
    let app_dir_name = app_dir_name_for_profile(&profile);
    dirs::data_dir()
        .map(|d| d.join(app_dir_name))
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap()
                .join(format!(".{}", app_dir_name))
        })
}

pub(crate) fn privacy_policy_path() -> std::path::PathBuf {
    app_data_dir().join("privacy_policy.yaml")
}

pub(crate) fn mcp_audit_keyring_path() -> std::path::PathBuf {
    app_data_dir().join("mcp_audit_keys.json")
}

pub(crate) fn load_mcp_audit_key_reference_state_from_path(
    path: &std::path::Path,
) -> McpAuditKeyReferenceLoadState {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return McpAuditKeyReferenceLoadState::Missing;
        }
        Err(error) => {
            return McpAuditKeyReferenceLoadState::Unreadable(error.to_string());
        }
        Ok(_) => {}
    }
    let database_path = path.with_file_name("mcp_audit.db");
    match load_mcp_audit_reference_for_store(path, &database_path) {
        Ok(McpAuditLoadedReference::DurableV2(receipt)) => {
            McpAuditKeyReferenceLoadState::Versioned(receipt)
        }
        Ok(McpAuditLoadedReference::Legacy(receipt)) => {
            McpAuditKeyReferenceLoadState::Legacy(receipt)
        }
        Err(error) => {
            let detail = error.to_string();
            if detail.contains("decode")
                || detail.contains("unsupported")
                || detail.contains("identity_not_random")
                || detail.contains("reference_set_empty")
                || detail.contains("strictly_increasing")
                || detail.contains("keychain_reference_missing")
            {
                McpAuditKeyReferenceLoadState::Invalid(detail)
            } else {
                McpAuditKeyReferenceLoadState::Unreadable(detail)
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn load_mcp_audit_keyring_from_path(path: &std::path::Path) -> Vec<AuditKeyConfig> {
    match load_mcp_audit_key_reference_state_from_path(path) {
        McpAuditKeyReferenceLoadState::Versioned(receipt) => receipt.document().keys().to_vec(),
        McpAuditKeyReferenceLoadState::Legacy(receipt) => receipt.keys().to_vec(),
        McpAuditKeyReferenceLoadState::Missing
        | McpAuditKeyReferenceLoadState::Invalid(_)
        | McpAuditKeyReferenceLoadState::Unreadable(_) => Vec::new(),
    }
}

#[cfg(test)]
pub(crate) fn save_mcp_audit_keyring_to_path(
    path: &std::path::Path,
    configs: &[AuditKeyConfig],
) -> Result<(), AppError> {
    let document = match load_mcp_audit_key_reference_state_from_path(path) {
        McpAuditKeyReferenceLoadState::Versioned(receipt) => {
            McpAuditLegacyVersionedReferenceDocument {
                version: MCP_AUDIT_LEGACY_VERSIONED_DOCUMENT_VERSION,
                store_identity: receipt.document().store_identity().to_string(),
                keys: configs.to_vec(),
            }
        }
        McpAuditKeyReferenceLoadState::Legacy(receipt)
            if receipt.format() == McpAuditLegacyReferenceFormat::VersionedV1 =>
        {
            McpAuditLegacyVersionedReferenceDocument {
                version: MCP_AUDIT_LEGACY_VERSIONED_DOCUMENT_VERSION,
                store_identity: receipt
                    .store_identity()
                    .expect("versioned legacy receipt has identity")
                    .to_string(),
                keys: configs.to_vec(),
            }
        }
        McpAuditKeyReferenceLoadState::Missing | McpAuditKeyReferenceLoadState::Legacy(_) => {
            McpAuditLegacyVersionedReferenceDocument::new(configs.to_vec())
        }
        McpAuditKeyReferenceLoadState::Invalid(error) => {
            return Err(AppError::db(format!(
                "refuse to overwrite invalid MCP audit key reference store: {error}"
            )));
        }
        McpAuditKeyReferenceLoadState::Unreadable(error) => {
            return Err(AppError::db(format!(
                "refuse to overwrite unreadable MCP audit key reference store: {error}"
            )));
        }
    };
    document.validate().map_err(AppError::db)?;
    let bytes = serde_json::to_vec_pretty(&document)?;
    openlife_core::atomic_file::write_atomic(path, &bytes).map_err(AppError::from)
}

pub(crate) type McpAuditReferenceWriteError = McpAuditReferenceMutationError;

pub(crate) fn save_mcp_audit_key_reference_document_commit_aware(
    _path: &std::path::Path,
    permit: McpAuditReferenceMutationPermit<'_>,
) -> Result<McpAuditReferenceReceipt, McpAuditReferenceWriteError> {
    #[cfg(test)]
    if mcp_audit_keyring_save_failure_injected(_path) {
        return Err(McpAuditReferenceWriteError::precommit_rejected(
            "injected_mcp_audit_keyring_reference_save_failure",
        ));
    }
    permit.commit_write()
}

/// Remove a crash-staged fresh reference only after proving the exact receipt
/// still occupies the canonical path.  The canonical SQLite owner reservation
/// excludes every cooperative writer; a non-cooperative swap is detected after
/// the no-replace move and reported as unknown rather than claiming canonical
/// absence for a different generation.
pub(crate) fn remove_mcp_audit_key_reference_exact_commit_aware(
    permit: McpAuditReferenceMutationPermit<'static>,
) -> Result<openlife_core::sqlite_migration::SqliteSlotOwnerReservation, McpAuditReferenceWriteError>
{
    permit.commit_delete()
}

pub(crate) fn activate_mcp_audit_store_from_reference_receipt(
    db_path: &std::path::Path,
    materials: Vec<AuditKeyMaterial>,
    reservation: openlife_core::sqlite_migration::SqliteSlotOwnerReservation,
    receipt: &McpAuditReferenceReceipt,
) -> anyhow::Result<McpAuditStore> {
    receipt.revalidate_visible()?;
    let document = receipt.document();
    let slot_digest = reservation.canonical_slot_digest()?;
    if document.canonical_slot_digest() != slot_digest {
        anyhow::bail!("mcp_audit_reference_canonical_slot_mismatch");
    }
    McpAuditStore::activate_store_bound_authority(db_path, materials, reservation, receipt.clone())
}

pub(crate) fn activate_fresh_mcp_audit_store_from_reference_receipt(
    db_path: &std::path::Path,
    materials: Vec<AuditKeyMaterial>,
    capability: McpAuditFreshDatabaseCreationCapability,
    receipt: &McpAuditReferenceReceipt,
) -> anyhow::Result<McpAuditStore> {
    receipt.revalidate_visible()?;
    McpAuditStore::activate_fresh_store_bound_authority(
        db_path,
        materials,
        capability,
        receipt.clone(),
    )
}

pub(crate) fn authorize_fresh_mcp_audit_database_recovery(
    db_path: &std::path::Path,
    materials: &[AuditKeyMaterial],
    reservation: openlife_core::sqlite_migration::SqliteSlotOwnerReservation,
    receipt: &McpAuditReferenceReceipt,
) -> anyhow::Result<McpAuditFreshDatabaseCreationCapability> {
    receipt.revalidate_visible()?;
    let _ = db_path;
    McpAuditStore::authorize_fresh_database_recovery(reservation, receipt, materials)
}

pub(crate) fn commit_mcp_audit_rotation_from_reference_receipt(
    store: &mut McpAuditStore,
    receipt: &McpAuditReferenceReceipt,
    material: AuditKeyMaterial,
    transition: &mut McpAuditRotationTransition,
) -> anyhow::Result<()> {
    receipt.revalidate_visible()?;
    let document = receipt.document();
    if document.phase() != McpAuditReferencePhase::Prepared
        || document.pending_epoch() != Some(material.config.epoch)
        || document.active_epoch() != Some(store.key_config().epoch)
        || document.canonical_slot_digest()
            != openlife_core::sqlite_migration::canonical_sqlite_slot_digest(
                store.database_path(),
                "mcp_audit_store",
            )?
    {
        anyhow::bail!("mcp_audit_rotation_reference_receipt_mismatch");
    }
    let durable_without_pending = &document.keys()[..document.keys().len().saturating_sub(1)];
    if durable_without_pending.len() != store.key_configs().len()
        || durable_without_pending
            .iter()
            .zip(store.key_configs())
            .any(|(durable, active)| {
                durable.epoch != active.epoch
                    || durable.mode != active.mode
                    || durable.key_ref != active.key_ref
            })
    {
        anyhow::bail!("mcp_audit_rotation_reference_keyring_mismatch");
    }
    store.commit_store_bound_key_rotation(material, receipt.clone(), transition)
}

fn load_active_mcp_audit_reference_receipt(
    store: &McpAuditStore,
    receipt: &McpAuditReferenceReceipt,
) -> anyhow::Result<McpAuditDurableReferenceReceipt> {
    receipt.revalidate_visible()?;
    let document = receipt.document();
    if document.phase() != McpAuditReferencePhase::Active
        || document.pending_epoch().is_some()
        || document.active_epoch() != Some(store.key_config().epoch)
        || document.canonical_slot_digest()
            != openlife_core::sqlite_migration::canonical_sqlite_slot_digest(
                store.database_path(),
                "mcp_audit_store",
            )?
    {
        anyhow::bail!("mcp_audit_active_reference_receipt_mismatch");
    }
    Ok(receipt.clone())
}

pub(crate) fn install_mcp_audit_active_reference_after_rotation(
    store: &mut McpAuditStore,
    receipt: &McpAuditReferenceReceipt,
    transition: &mut McpAuditRotationTransition,
) -> anyhow::Result<()> {
    let durable_receipt = load_active_mcp_audit_reference_receipt(store, receipt)?;
    store.install_active_reference_after_rotation(durable_receipt, transition)
}

pub(crate) fn install_mcp_audit_active_reference_after_bootstrap(
    store: &mut McpAuditStore,
    receipt: &McpAuditReferenceReceipt,
) -> anyhow::Result<()> {
    let durable_receipt = load_active_mcp_audit_reference_receipt(store, receipt)?;
    store.install_active_reference_after_bootstrap(durable_receipt)
}

#[cfg(test)]
pub(crate) fn load_privacy_policy_from_path(path: &std::path::Path) -> PrivacyPolicy {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| PrivacyPolicy::from_yaml(&text).ok())
        .unwrap_or_default()
}

pub(crate) fn save_privacy_policy_to_path(
    path: &std::path::Path,
    policy: &PrivacyPolicy,
) -> Result<(), AppError> {
    let text = policy.to_yaml().map_err(AppError::from)?;
    openlife_core::atomic_file::write_atomic(path, text.as_bytes()).map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::privacy::{PrivacyAction, PrivacyType};

    #[test]
    fn privacy_policy_persists_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("privacy_policy.yaml");
        let mut policy = PrivacyPolicy::default();
        for rule in &mut policy.rules {
            if rule.ptype == PrivacyType::Email {
                rule.action = PrivacyAction::Block;
            }
        }

        save_privacy_policy_to_path(&path, &policy).unwrap();
        let loaded = load_privacy_policy_from_path(&path);
        let email_rule = loaded
            .rules
            .iter()
            .find(|rule| rule.ptype == PrivacyType::Email)
            .unwrap();
        assert_eq!(email_rule.action, PrivacyAction::Block);
    }

    #[test]
    fn mcp_audit_keyring_persists_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp_audit_keys.json");
        let mut configs = vec![AuditKeyConfig::default()];
        configs.push(AuditKeyConfig {
            epoch: 123,
            created_at: "2026-04-21T00:00:00Z".to_string(),
            ..AuditKeyConfig::default()
        });

        save_mcp_audit_keyring_to_path(&path, &configs).unwrap();
        let loaded = load_mcp_audit_keyring_from_path(&path);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[1].epoch, 123);
    }

    #[test]
    fn mcp_audit_key_reference_document_keeps_one_random_store_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp_audit_keys.json");
        let first_configs = vec![AuditKeyConfig {
            mode: openlife_core::mcp_audit::KeyMode::Keychain,
            key_ref: Some(
                "keychain://com.openlife.desktop/mcp-audit-key-store-fixture-epoch-10".into(),
            ),
            epoch: 10,
            ..AuditKeyConfig::default()
        }];
        save_mcp_audit_keyring_to_path(&path, &first_configs).unwrap();
        let first_identity = match load_mcp_audit_key_reference_state_from_path(&path) {
            McpAuditKeyReferenceLoadState::Legacy(receipt)
                if receipt.format() == McpAuditLegacyReferenceFormat::VersionedV1 =>
            {
                let store_identity = receipt.store_identity().unwrap().to_string();
                let identity = uuid::Uuid::parse_str(&store_identity).unwrap();
                assert_eq!(identity.get_version_num(), 4);
                store_identity
            }
            state => panic!("expected legacy-versioned fixture document, got {state:?}"),
        };

        let mut second_configs = first_configs;
        second_configs.push(AuditKeyConfig {
            mode: openlife_core::mcp_audit::KeyMode::Keychain,
            key_ref: Some(
                "keychain://com.openlife.desktop/mcp-audit-key-store-fixture-epoch-11".into(),
            ),
            epoch: 11,
            ..AuditKeyConfig::default()
        });
        save_mcp_audit_keyring_to_path(&path, &second_configs).unwrap();
        match load_mcp_audit_key_reference_state_from_path(&path) {
            McpAuditKeyReferenceLoadState::Legacy(receipt) => {
                assert_eq!(receipt.store_identity(), Some(first_identity.as_str()));
                assert_eq!(receipt.keys().len(), 2);
            }
            state => panic!("expected legacy-versioned fixture document, got {state:?}"),
        }
    }

    #[test]
    fn invalid_or_duplicate_key_reference_files_never_collapse_to_missing() {
        let dir = tempfile::tempdir().unwrap();
        let invalid_path = dir.path().join("invalid.json");
        std::fs::write(&invalid_path, b"{not-json").unwrap();
        assert!(matches!(
            load_mcp_audit_key_reference_state_from_path(&invalid_path),
            McpAuditKeyReferenceLoadState::Invalid(_)
        ));

        let duplicate_path = dir.path().join("duplicate.json");
        let duplicate = AuditKeyConfig {
            mode: openlife_core::mcp_audit::KeyMode::Keychain,
            key_ref: Some("keychain://duplicate/epoch-7".into()),
            epoch: 7,
            ..AuditKeyConfig::default()
        };
        std::fs::write(
            &duplicate_path,
            serde_json::to_vec(&vec![duplicate.clone(), duplicate]).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            load_mcp_audit_key_reference_state_from_path(&duplicate_path),
            McpAuditKeyReferenceLoadState::Invalid(error)
                if error.contains("mcp_audit_key_epoch_not_strictly_increasing")
        ));
    }

    #[test]
    fn post_rename_reference_failure_is_visible_but_durability_unknown() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit_keys.json");
        let database_path = directory.path().join("mcp_audit.db");
        let identity = uuid::Uuid::new_v4();
        let epoch = 41;
        let config = AuditKeyConfig {
            mode: openlife_core::mcp_audit::KeyMode::Keychain,
            key_ref: Some(format!(
                "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-{epoch}",
                identity.simple()
            )),
            epoch,
            created_at: "2026-07-13T00:00:00Z".into(),
            salt_b64: None,
            env_var: None,
        };
        let document = McpAuditKeyReferenceDocument::prepared(
            identity.to_string(),
            openlife_core::sqlite_migration::canonical_sqlite_slot_digest(
                &database_path,
                "mcp_audit_store",
            )
            .unwrap(),
            vec![config],
            None,
            epoch,
            McpAuditReferenceOrigin::FreshCreate,
            format!("sha256:{}", "ab".repeat(32)),
        )
        .unwrap();
        let _fault =
            openlife_core::atomic_file::inject_post_rename_sync_failure_for_test(path.clone());

        let permit =
            McpAuditReferenceMutationPermit::test_only_publish(&path, document.clone()).unwrap();
        let error = save_mcp_audit_key_reference_document_commit_aware(&path, permit).unwrap_err();

        assert_eq!(
            error.commit_state(),
            AtomicWriteCommitState::VisibleDurabilityUnknown
        );
        assert!(matches!(
            load_mcp_audit_key_reference_state_from_path(&path),
            McpAuditKeyReferenceLoadState::Versioned(receipt)
                if receipt.document().phase() == McpAuditReferencePhase::Prepared
        ));
    }

    fn fresh_test_reference(
        database_path: &std::path::Path,
        identity: uuid::Uuid,
        epoch: u64,
        digest_byte: &str,
    ) -> McpAuditKeyReferenceDocument {
        McpAuditKeyReferenceDocument::prepared(
            identity.to_string(),
            openlife_core::sqlite_migration::canonical_sqlite_slot_digest(
                database_path,
                "mcp_audit_store",
            )
            .unwrap(),
            vec![AuditKeyConfig {
                mode: openlife_core::mcp_audit::KeyMode::Keychain,
                salt_b64: None,
                env_var: None,
                key_ref: Some(format!(
                    "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-{epoch}",
                    identity.simple()
                )),
                epoch,
                created_at: "2026-07-13T00:00:00Z".into(),
            }],
            None,
            epoch,
            McpAuditReferenceOrigin::FreshCreate,
            format!("sha256:{}", digest_byte.repeat(32)),
        )
        .unwrap()
    }

    #[test]
    fn absent_reference_publish_is_kernel_no_replace() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit_keys.json");
        let database_path = directory.path().join("mcp_audit.db");
        let first = fresh_test_reference(&database_path, uuid::Uuid::new_v4(), 51, "aa");
        let second = fresh_test_reference(&database_path, uuid::Uuid::new_v4(), 52, "bb");
        let first_permit =
            McpAuditReferenceMutationPermit::test_only_publish(&path, first.clone()).unwrap();
        save_mcp_audit_key_reference_document_commit_aware(&path, first_permit).unwrap();

        let second_permit =
            McpAuditReferenceMutationPermit::test_only_publish(&path, second.clone()).unwrap();
        let error =
            save_mcp_audit_key_reference_document_commit_aware(&path, second_permit).unwrap_err();

        assert_eq!(error.commit_state(), AtomicWriteCommitState::NotCommitted);
        assert!(matches!(
            load_mcp_audit_key_reference_state_from_path(&path),
            McpAuditKeyReferenceLoadState::Versioned(receipt)
                if receipt.document() == &first
        ));
    }

    #[test]
    fn exact_generation_transition_rejects_skipped_state_and_same_bytes_new_inode() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit_keys.json");
        let database_path = directory.path().join("mcp_audit.db");
        let pending = fresh_test_reference(&database_path, uuid::Uuid::new_v4(), 53, "cc");
        let pending_permit =
            McpAuditReferenceMutationPermit::test_only_publish(&path, pending.clone()).unwrap();
        let pending_receipt =
            save_mcp_audit_key_reference_document_commit_aware(&path, pending_permit).unwrap();
        let mut skipped = pending.clone();
        skipped.mark_secret_verified().unwrap();
        skipped.mark_database_attempted().unwrap();
        let skipped_error =
            McpAuditReferenceMutationPermit::test_only_replace(&pending_receipt, skipped)
                .unwrap_err();
        assert!(skipped_error
            .to_string()
            .contains("mcp_audit_reference_transition_not_monotonic"));

        let same_bytes = serde_json::to_vec_pretty(&pending).unwrap();
        openlife_core::atomic_file::write_atomic(&path, &same_bytes).unwrap();
        let mut verified = pending;
        verified.mark_secret_verified().unwrap();
        let inode_error =
            McpAuditReferenceMutationPermit::test_only_replace(&pending_receipt, verified)
                .unwrap_err();
        assert!(inode_error
            .to_string()
            .contains("mcp_audit_reference_changed_after_receipt"));
    }

    #[test]
    fn orchestration_wire_transitions_match_the_core_authority_decoder() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit_keys.json");
        let database_path = directory.path().join("mcp_audit.db");
        let mut document = fresh_test_reference(&database_path, uuid::Uuid::new_v4(), 54, "dd");
        let pending_permit =
            McpAuditReferenceMutationPermit::test_only_publish(&path, document.clone()).unwrap();
        let pending =
            save_mcp_audit_key_reference_document_commit_aware(&path, pending_permit).unwrap();
        McpAuditDurableReferenceReceipt::load_for_store(&path, &database_path)
            .expect("core must accept the Tauri Pending wire generation");

        document.mark_secret_verified().unwrap();
        let verified_permit =
            McpAuditReferenceMutationPermit::test_only_replace(&pending, document.clone()).unwrap();
        let verified =
            save_mcp_audit_key_reference_document_commit_aware(&path, verified_permit).unwrap();
        McpAuditDurableReferenceReceipt::load_for_store(&path, &database_path)
            .expect("core must accept the same transition id at Verified");

        document.mark_database_attempted().unwrap();
        let attempted_permit =
            McpAuditReferenceMutationPermit::test_only_replace(&verified, document.clone())
                .unwrap();
        let attempted =
            save_mcp_audit_key_reference_document_commit_aware(&path, attempted_permit).unwrap();
        McpAuditDurableReferenceReceipt::load_for_store(&path, &database_path)
            .expect("core must accept the same transition id at Attempted");

        document.mark_active().unwrap();
        let active_permit =
            McpAuditReferenceMutationPermit::test_only_replace(&attempted, document.clone())
                .unwrap();
        save_mcp_audit_key_reference_document_commit_aware(&path, active_permit).unwrap();
        McpAuditDurableReferenceReceipt::load_for_store(&path, &database_path)
            .expect("core must accept the Active wire generation");

        let invalid = fresh_test_reference(&database_path, uuid::Uuid::new_v4(), 55, "ee");
        let mut invalid = serde_json::to_value(&invalid).unwrap();
        invalid
            .as_object_mut()
            .unwrap()
            .insert("transitionId".into(), serde_json::Value::Null);
        openlife_core::atomic_file::write_atomic(
            &path,
            &serde_json::to_vec_pretty(&invalid).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            load_mcp_audit_key_reference_state_from_path(&path),
            McpAuditKeyReferenceLoadState::Invalid(_)
        ));
        assert!(McpAuditDurableReferenceReceipt::load_for_store(&path, &database_path).is_err());
    }

    #[test]
    fn duplicate_field_v2_reference_is_rejected_as_ambiguous() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp_audit_keys.json");
        let database_path = directory.path().join("mcp_audit.db");
        let identity = uuid::Uuid::new_v4();
        let epoch = 42;
        let document = McpAuditKeyReferenceDocument::prepared(
            identity.to_string(),
            openlife_core::sqlite_migration::canonical_sqlite_slot_digest(
                &database_path,
                "mcp_audit_store",
            )
            .unwrap(),
            vec![AuditKeyConfig {
                mode: openlife_core::mcp_audit::KeyMode::Keychain,
                salt_b64: None,
                env_var: None,
                key_ref: Some(format!(
                    "{MCP_AUDIT_STORE_KEY_REF_PREFIX}{}-epoch-{epoch}",
                    identity.simple()
                )),
                epoch,
                created_at: "2026-07-13T00:00:00Z".into(),
            }],
            None,
            epoch,
            McpAuditReferenceOrigin::FreshCreate,
            format!("sha256:{}", "ab".repeat(32)),
        )
        .unwrap();
        let canonical = serde_json::to_string_pretty(&document).unwrap();
        let ambiguous = canonical.replacen(
            "  \"version\": 2,",
            "  \"version\": 2,\n  \"version\": 2,",
            1,
        );
        std::fs::write(&path, ambiguous).unwrap();

        assert!(matches!(
            load_mcp_audit_key_reference_state_from_path(&path),
            McpAuditKeyReferenceLoadState::Invalid(_)
        ));

        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        assert!(matches!(
            load_mcp_audit_key_reference_state_from_path(&path),
            McpAuditKeyReferenceLoadState::Invalid(error)
                if error.contains("mcp_audit_reference_noncanonical_or_ambiguous_json")
        ));
    }

    #[test]
    fn every_reference_generation_uses_the_bounded_no_follow_reader() {
        let directory = tempfile::tempdir().unwrap();
        let oversized = directory.path().join("oversized-legacy.json");
        std::fs::write(&oversized, vec![b'x'; 128 * 1024 + 1]).unwrap();
        assert!(matches!(
            load_mcp_audit_key_reference_state_from_path(&oversized),
            McpAuditKeyReferenceLoadState::Unreadable(error)
                if error.contains("mcp_audit_reference_too_large")
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let target = directory.path().join("legacy-target.json");
            let link = directory.path().join("legacy-link.json");
            std::fs::write(&target, b"[]").unwrap();
            symlink(&target, &link).unwrap();
            assert!(matches!(
                load_mcp_audit_key_reference_state_from_path(&link),
                McpAuditKeyReferenceLoadState::Unreadable(error)
                    if error.contains("symlink") || error.contains("Too many levels")
            ));
        }
    }
}
