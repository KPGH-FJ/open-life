use crate::errors::AppError;
use openlife_core::mcp_audit::AuditKeyConfig;
use openlife_core::privacy::PrivacyPolicy;
use serde::{Deserialize, Serialize};

const RELEASE_APP_DIR_NAME: &str = "ai.openlife.app";
const DEV_APP_DIR_NAME: &str = "ai.openlife.app.dev";
const QA_APP_DIR_NAME: &str = "ai.openlife.app.qa";
const MCP_AUDIT_KEY_REFERENCE_DOCUMENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct McpAuditKeyReferenceDocument {
    pub(crate) version: u32,
    pub(crate) store_identity: String,
    pub(crate) keys: Vec<AuditKeyConfig>,
}

impl McpAuditKeyReferenceDocument {
    pub(crate) fn new(keys: Vec<AuditKeyConfig>) -> Self {
        Self {
            version: MCP_AUDIT_KEY_REFERENCE_DOCUMENT_VERSION,
            store_identity: uuid::Uuid::new_v4().to_string(),
            keys,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.version != MCP_AUDIT_KEY_REFERENCE_DOCUMENT_VERSION {
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

#[derive(Debug, Clone)]
pub(crate) enum McpAuditKeyReferenceLoadState {
    Missing,
    Versioned(McpAuditKeyReferenceDocument),
    Legacy(Vec<AuditKeyConfig>),
    Invalid(String),
    Unreadable(String),
}

fn validate_mcp_audit_key_configs(configs: &[AuditKeyConfig]) -> Result<(), String> {
    if configs.is_empty() {
        return Err("mcp_audit_key_reference_set_empty".into());
    }
    let mut epochs = std::collections::HashSet::new();
    for config in configs {
        if !epochs.insert(config.epoch) {
            return Err(format!("mcp_audit_key_epoch_duplicate:{}", config.epoch));
        }
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

/// Thread-local fault injection used by D064 to fail only the post-hydration
/// reference save. This keeps the keyring path genuinely missing during load,
/// which is distinct from an invalid/unreadable pre-existing reference store.
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
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return McpAuditKeyReferenceLoadState::Missing;
        }
        Err(error) => {
            return McpAuditKeyReferenceLoadState::Unreadable(error.to_string());
        }
    };
    match serde_json::from_str::<McpAuditKeyReferenceDocument>(&text) {
        Ok(document) => match document.validate() {
            Ok(()) => McpAuditKeyReferenceLoadState::Versioned(document),
            Err(error) => McpAuditKeyReferenceLoadState::Invalid(error),
        },
        Err(document_error) => match serde_json::from_str::<Vec<AuditKeyConfig>>(&text) {
            Ok(configs) => match validate_mcp_audit_key_configs(&configs) {
                Ok(()) => McpAuditKeyReferenceLoadState::Legacy(configs),
                Err(error) => McpAuditKeyReferenceLoadState::Invalid(error),
            },
            Err(legacy_error) => McpAuditKeyReferenceLoadState::Invalid(format!(
                "mcp_audit_key_reference_decode_failed:document={document_error}; legacy={legacy_error}"
            )),
        },
    }
}

#[cfg(test)]
pub(crate) fn load_mcp_audit_keyring_from_path(path: &std::path::Path) -> Vec<AuditKeyConfig> {
    match load_mcp_audit_key_reference_state_from_path(path) {
        McpAuditKeyReferenceLoadState::Versioned(document) => document.keys,
        McpAuditKeyReferenceLoadState::Legacy(configs) => configs,
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
        McpAuditKeyReferenceLoadState::Versioned(mut document) => {
            document.keys = configs.to_vec();
            document
        }
        McpAuditKeyReferenceLoadState::Missing | McpAuditKeyReferenceLoadState::Legacy(_) => {
            McpAuditKeyReferenceDocument::new(configs.to_vec())
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
    save_mcp_audit_key_reference_document_to_path(path, &document)
}

pub(crate) fn save_mcp_audit_key_reference_document_to_path(
    path: &std::path::Path,
    document: &McpAuditKeyReferenceDocument,
) -> Result<(), AppError> {
    #[cfg(test)]
    if mcp_audit_keyring_save_failure_injected(path) {
        return Err(AppError::db(
            "injected_mcp_audit_keyring_reference_save_failure",
        ));
    }
    document.validate().map_err(|error| {
        AppError::db(format!("invalid MCP audit key reference document: {error}"))
    })?;
    let text = serde_json::to_string_pretty(document).map_err(AppError::from)?;
    openlife_core::atomic_file::write_atomic(path, text.as_bytes()).map_err(AppError::from)
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
            McpAuditKeyReferenceLoadState::Versioned(document) => {
                let identity = uuid::Uuid::parse_str(&document.store_identity).unwrap();
                assert_eq!(identity.get_version_num(), 4);
                document.store_identity
            }
            state => panic!("expected versioned key reference document, got {state:?}"),
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
            McpAuditKeyReferenceLoadState::Versioned(document) => {
                assert_eq!(document.store_identity, first_identity);
                assert_eq!(document.keys.len(), 2);
            }
            state => panic!("expected versioned key reference document, got {state:?}"),
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
                if error.contains("mcp_audit_key_epoch_duplicate")
        ));
    }
}
