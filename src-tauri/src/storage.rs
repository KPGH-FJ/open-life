use crate::errors::AppError;
use openlife_core::mcp_audit::AuditKeyConfig;
use openlife_core::privacy::PrivacyPolicy;

const RELEASE_APP_DIR_NAME: &str = "ai.openlife.app";
const DEV_APP_DIR_NAME: &str = "ai.openlife.app.dev";
const QA_APP_DIR_NAME: &str = "ai.openlife.app.qa";

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

pub(crate) fn load_mcp_audit_keyring_from_path(path: &std::path::Path) -> Vec<AuditKeyConfig> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<AuditKeyConfig>>(&text).ok())
        .unwrap_or_default()
}

pub(crate) fn save_mcp_audit_keyring_to_path(
    path: &std::path::Path,
    configs: &[AuditKeyConfig],
) -> Result<(), AppError> {
    #[cfg(test)]
    if mcp_audit_keyring_save_failure_injected(path) {
        return Err(AppError::db(
            "injected_mcp_audit_keyring_reference_save_failure",
        ));
    }
    let text = serde_json::to_string_pretty(configs).map_err(AppError::from)?;
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
}
