use crate::errors::AppError;
use openlife_core::mcp_audit::AuditKeyConfig;
use openlife_core::privacy::PrivacyPolicy;

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

pub(crate) fn load_mcp_audit_keyring_from_path(path: &std::path::Path) -> Vec<AuditKeyConfig> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<AuditKeyConfig>>(&text).ok())
        .filter(|configs| !configs.is_empty())
        .unwrap_or_else(|| vec![AuditKeyConfig::default()])
}

pub(crate) fn save_mcp_audit_keyring_to_path(
    path: &std::path::Path,
    configs: &[AuditKeyConfig],
) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::from)?;
    }
    let text = serde_json::to_string_pretty(configs).map_err(AppError::from)?;
    std::fs::write(path, text).map_err(AppError::from)
}

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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::from)?;
    }
    let text = policy.to_yaml().map_err(AppError::from)?;
    std::fs::write(path, text).map_err(AppError::from)
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct OnboardingStatus {
    pub completed: bool,
    pub completed_at: Option<String>,
}

pub(crate) fn onboarding_status_path() -> std::path::PathBuf {
    app_data_dir().join("onboarding.json")
}

pub(crate) fn load_onboarding_status_from_path(path: &std::path::Path) -> OnboardingStatus {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub(crate) fn save_onboarding_status_to_path(
    path: &std::path::Path,
    status: &OnboardingStatus,
) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::from)?;
    }
    let text = serde_json::to_string_pretty(status).map_err(AppError::from)?;
    std::fs::write(path, text).map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::privacy::{PrivacyAction, PrivacyType};

    #[test]
    fn onboarding_status_persists_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("onboarding.json");
        assert!(!load_onboarding_status_from_path(&path).completed);

        let status = OnboardingStatus {
            completed: true,
            completed_at: Some("2026-04-21T00:00:00Z".to_string()),
        };
        save_onboarding_status_to_path(&path, &status).unwrap();

        let loaded = load_onboarding_status_from_path(&path);
        assert!(loaded.completed);
        assert_eq!(loaded.completed_at.as_deref(), Some("2026-04-21T00:00:00Z"));
    }

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
