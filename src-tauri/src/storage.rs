use crate::errors::AppError;
use openlife_core::mcp_audit::AuditKeyConfig;
use openlife_core::privacy::PrivacyPolicy;
#[cfg(test)]
use std::collections::HashSet;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::sync::{LazyLock, Mutex};

const RELEASE_APP_DIR_NAME: &str = "ai.openlife.app";
const DEV_APP_DIR_NAME: &str = "ai.openlife.app.dev";
const QA_APP_DIR_NAME: &str = "ai.openlife.app.qa";

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

#[derive(Debug, Clone)]
pub(crate) enum McpAuditKeyringLoad {
    Absent,
    Present(Vec<AuditKeyConfig>),
    PresentInvalid { error: String },
    Unreadable { error: String },
}

pub(crate) fn load_mcp_audit_keyring_from_path(path: &std::path::Path) -> McpAuditKeyringLoad {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return McpAuditKeyringLoad::Absent;
        }
        Err(error) => {
            return McpAuditKeyringLoad::Unreadable {
                error: AppError::from(error).to_string(),
            };
        }
    };
    let configs = match serde_json::from_slice::<Vec<AuditKeyConfig>>(&bytes) {
        Ok(configs) => configs,
        Err(error) => {
            return McpAuditKeyringLoad::PresentInvalid {
                error: error.to_string(),
            };
        }
    };
    if configs.is_empty() {
        return McpAuditKeyringLoad::PresentInvalid {
            error: "MCP audit keyring is present but contains no key authority".into(),
        };
    }
    McpAuditKeyringLoad::Present(configs)
}

pub(crate) fn save_mcp_audit_keyring_to_path(
    path: &std::path::Path,
    configs: &[AuditKeyConfig],
) -> Result<(), AppError> {
    #[cfg(test)]
    if MCP_AUDIT_KEYRING_SAVE_FAILURES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(path)
    {
        return Err(AppError::db(
            "injected MCP audit key-reference save failure",
        ));
    }
    let text = serde_json::to_string_pretty(configs).map_err(AppError::from)?;
    openlife_core::atomic_file::write_atomic(path, text.as_bytes()).map_err(AppError::from)
}

#[cfg(test)]
static MCP_AUDIT_KEYRING_SAVE_FAILURES: LazyLock<Mutex<HashSet<PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[cfg(test)]
pub(crate) fn fail_next_mcp_audit_keyring_save_for_test(path: impl Into<PathBuf>) {
    MCP_AUDIT_KEYRING_SAVE_FAILURES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(path.into());
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
        let McpAuditKeyringLoad::Present(loaded) = loaded else {
            panic!("saved MCP audit keyring must be present");
        };
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[1].epoch, 123);
    }

    #[test]
    fn missing_and_present_invalid_mcp_audit_keyrings_are_distinct() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp_audit_keys.json");

        assert!(matches!(
            load_mcp_audit_keyring_from_path(&path),
            McpAuditKeyringLoad::Absent
        ));

        std::fs::write(&path, b"[]\n").unwrap();
        assert!(matches!(
            load_mcp_audit_keyring_from_path(&path),
            McpAuditKeyringLoad::PresentInvalid { .. }
        ));

        std::fs::write(&path, b"{ malformed\n").unwrap();
        assert!(matches!(
            load_mcp_audit_keyring_from_path(&path),
            McpAuditKeyringLoad::PresentInvalid { .. }
        ));
    }
}
