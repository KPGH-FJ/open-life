use crate::errors::AppError;
use openlife_core::mcp_audit::AuditKeyConfig;
use openlife_core::privacy::PrivacyPolicy;

pub(crate) fn app_data_dir() -> std::path::PathBuf {
    #[cfg(test)]
    {
        let dir =
            std::env::temp_dir().join(format!("ai.openlife.desktop-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }

    #[cfg(not(test))]
    dirs::data_dir()
        .map(|d| d.join("ai.openlife.desktop"))
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("ai.openlife.desktop")
}

pub(crate) fn privacy_policy_path() -> std::path::PathBuf {
    app_data_dir().join("privacy_policy.yaml")
}

pub(crate) fn mcp_audit_keyring_path() -> std::path::PathBuf {
    app_data_dir().join("mcp_audit_keys.json")
}

#[allow(dead_code)]
pub(crate) fn load_mcp_audit_keyring_from_path(path: &std::path::Path) -> Vec<AuditKeyConfig> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<AuditKeyConfig>>(&text).ok())
        .filter(|configs| !configs.is_empty())
        .unwrap_or_else(|| vec![AuditKeyConfig::default()])
}

/// Load or create the MCP audit keyring, persisting the result to disk.
/// Always includes a legacy `KeyMode::Derived` config for reading old logs.
/// On first run, generates and persists a new `PerInstall` random key.
/// Load or create the MCP audit keyring, persisting the result to disk.
/// Always includes a legacy `KeyMode::Derived` config for reading old logs.
/// On first run, generates and persists a new `PerInstall` random key.
///
/// **Safety**: if the keyring file exists but cannot be parsed (corrupted
/// JSON, invalid schema, read error), this function returns `Err` and does
/// **not** overwrite the original file.  The caller (bootstrap) may then
/// fall back to an ephemeral key so the application can start, but the
/// damaged file is left intact for manual recovery.
pub(crate) fn load_or_create_mcp_audit_keyring(
    path: &std::path::Path,
) -> Result<Vec<AuditKeyConfig>, AppError> {
    if path.exists() {
        let text = std::fs::read_to_string(path).map_err(|e| {
            AppError::internal(format!(
                "failed to read MCP audit keyring file {}: {}",
                path.display(),
                e
            ))
        })?;
        let configs: Vec<AuditKeyConfig> = serde_json::from_str(&text).map_err(|e| {
            AppError::internal(format!(
                "MCP audit keyring file {} contains invalid JSON: {}",
                path.display(),
                e
            ))
        })?;
        if configs.is_empty() {
            return Err(AppError::internal(format!(
                "MCP audit keyring file {} contains an empty key array",
                path.display(),
            )));
        }
        return Ok(configs);
    }

    // Create new keyring: fresh PerInstall key + legacy Derived for old logs
    use rand::RngCore;
    let mut random_key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut random_key);
    let per_install = AuditKeyConfig {
        mode: openlife_core::mcp_audit::KeyMode::PerInstall,
        salt_b64: Some(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            random_key,
        )),
        env_var: None,
        epoch: 1,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let legacy = AuditKeyConfig {
        mode: openlife_core::mcp_audit::KeyMode::Derived,
        salt_b64: None,
        env_var: None,
        epoch: 0,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let configs = vec![per_install, legacy];

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::from)?;
    }
    let text = serde_json::to_string_pretty(&configs).map_err(AppError::from)?;
    std::fs::write(path, text).map_err(AppError::from)?;

    Ok(configs)
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

    #[test]
    fn load_or_create_keyring_creates_new_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keyring.json");
        // File does not exist → should create
        let result = load_or_create_mcp_audit_keyring(&path);
        assert!(result.is_ok(), "should create keyring when file missing");
        assert!(path.exists(), "keyring file should be created");

        let configs = result.unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(
            configs[0].mode,
            openlife_core::mcp_audit::KeyMode::PerInstall
        );
        assert_eq!(configs[1].mode, openlife_core::mcp_audit::KeyMode::Derived);
    }

    #[test]
    fn load_or_create_keyring_loads_existing_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keyring.json");

        // Pre-write a valid keyring
        let original = vec![AuditKeyConfig {
            epoch: 777,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            ..AuditKeyConfig::default()
        }];
        save_mcp_audit_keyring_to_path(&path, &original).unwrap();
        let original_content = std::fs::read_to_string(&path).unwrap();

        // Load — should return the original without rewriting
        let result = load_or_create_mcp_audit_keyring(&path);
        assert!(result.is_ok(), "should load existing valid keyring");
        let loaded = result.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].epoch, 777);

        // File content should be unchanged
        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            after, original_content,
            "valid keyring must not be rewritten"
        );
    }

    #[test]
    fn load_or_create_keyring_returns_err_on_corrupted_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keyring.json");

        // Write corrupted JSON
        std::fs::write(&path, b"this is not valid json {{{").unwrap();
        let original_content = std::fs::read_to_string(&path).unwrap();

        let result = load_or_create_mcp_audit_keyring(&path);
        assert!(result.is_err(), "corrupted JSON must return Err");

        // File content must be unchanged
        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            after, original_content,
            "corrupted file must not be overwritten"
        );
    }

    #[test]
    fn load_or_create_keyring_returns_err_on_empty_array() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keyring.json");

        std::fs::write(&path, b"[]").unwrap();
        let original_content = std::fs::read_to_string(&path).unwrap();

        let result = load_or_create_mcp_audit_keyring(&path);
        assert!(result.is_err(), "empty array must return Err");

        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            after, original_content,
            "empty array file must not be overwritten"
        );
    }

    #[test]
    fn load_or_create_keyring_returns_err_on_invalid_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keyring.json");

        // Write JSON that parses but doesn't match schema (object instead of array)
        std::fs::write(&path, b"{\"not\": \"an array\"}").unwrap();
        let original_content = std::fs::read_to_string(&path).unwrap();

        let result = load_or_create_mcp_audit_keyring(&path);
        assert!(result.is_err(), "schema mismatch must return Err");

        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            after, original_content,
            "invalid schema file must not be overwritten"
        );
    }
}
