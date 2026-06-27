use chrono::{DateTime, Duration, Utc};
use openlife_core::config::AppConfig;
use openlife_core::llm::default_base_for_provider;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::errors::AppError;
use crate::storage::app_data_dir;

pub(crate) const PROVIDER_VALIDATION_TTL_HOURS: i64 = 24;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderValidationIdentity {
    pub provider: String,
    pub endpoint_hash: String,
    pub model_hash: String,
    pub key_present: bool,
    pub network_policy_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderValidationRecord {
    pub provider: String,
    pub endpoint_hash: String,
    pub model_hash: String,
    pub key_present: bool,
    pub network_policy_hash: String,
    pub validated_at: Option<String>,
    pub failed_at: Option<String>,
    pub last_error: Option<String>,
    pub validation_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderValidationSummary {
    pub configured: bool,
    pub validated: bool,
    pub status: &'static str,
    pub last_error: Option<String>,
    pub validated_at: Option<String>,
    pub failed_at: Option<String>,
    pub validation_source: Option<String>,
}

pub(crate) fn provider_validation_path() -> PathBuf {
    app_data_dir().join("provider_validation.json")
}

pub(crate) fn load_provider_validation_record_from_path(
    path: &Path,
) -> Option<ProviderValidationRecord> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<ProviderValidationRecord>(&text).ok())
}

pub(crate) fn save_provider_validation_record_to_path(
    path: &Path,
    record: &ProviderValidationRecord,
) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::from)?;
    }
    let text = serde_json::to_string_pretty(record).map_err(AppError::from)?;
    std::fs::write(path, text).map_err(AppError::from)
}

pub(crate) fn current_provider_validation_identity(
    config: &AppConfig,
) -> ProviderValidationIdentity {
    let provider = normalized_provider(config);
    ProviderValidationIdentity {
        provider: provider.clone(),
        endpoint_hash: digest_label(&normalized_endpoint(config, &provider)),
        model_hash: digest_label(config.llm.chat_model.trim()),
        key_present: !config.effective_cloud_api_key().trim().is_empty(),
        network_policy_hash: digest_network_policy(config),
    }
}

pub(crate) fn cloud_api_configured(config: &AppConfig) -> bool {
    let provider = normalized_provider(config);
    let endpoint = normalized_endpoint(config, &provider);
    !provider.is_empty()
        && !endpoint.is_empty()
        && !config.llm.chat_model.trim().is_empty()
        && !config.effective_cloud_api_key().trim().is_empty()
}

pub(crate) fn successful_provider_validation_record(
    config: &AppConfig,
    validation_source: impl Into<String>,
    now: DateTime<Utc>,
) -> ProviderValidationRecord {
    let identity = current_provider_validation_identity(config);
    ProviderValidationRecord {
        provider: identity.provider,
        endpoint_hash: identity.endpoint_hash,
        model_hash: identity.model_hash,
        key_present: identity.key_present,
        network_policy_hash: identity.network_policy_hash,
        validated_at: Some(now.to_rfc3339()),
        failed_at: None,
        last_error: None,
        validation_source: metadata_safe_label(validation_source.into()),
    }
}

pub(crate) fn failed_provider_validation_record(
    config: &AppConfig,
    validation_source: impl Into<String>,
    safe_error: impl Into<String>,
    now: DateTime<Utc>,
) -> ProviderValidationRecord {
    let identity = current_provider_validation_identity(config);
    ProviderValidationRecord {
        provider: identity.provider,
        endpoint_hash: identity.endpoint_hash,
        model_hash: identity.model_hash,
        key_present: identity.key_present,
        network_policy_hash: identity.network_policy_hash,
        validated_at: None,
        failed_at: Some(now.to_rfc3339()),
        last_error: Some(metadata_safe_validation_error(safe_error.into())),
        validation_source: metadata_safe_label(validation_source.into()),
    }
}

pub(crate) fn summarize_provider_validation(
    config: &AppConfig,
    record: Option<&ProviderValidationRecord>,
    now: DateTime<Utc>,
) -> ProviderValidationSummary {
    if !cloud_api_configured(config) {
        return ProviderValidationSummary {
            configured: false,
            validated: false,
            status: "unconfigured",
            last_error: None,
            validated_at: None,
            failed_at: None,
            validation_source: None,
        };
    }

    let Some(record) = record else {
        return ProviderValidationSummary {
            configured: true,
            validated: false,
            status: "unvalidated",
            last_error: None,
            validated_at: None,
            failed_at: None,
            validation_source: None,
        };
    };

    let current = current_provider_validation_identity(config);
    if !record_matches_identity(record, &current) {
        return ProviderValidationSummary {
            configured: true,
            validated: false,
            status: "stale",
            last_error: record.last_error.clone(),
            validated_at: record.validated_at.clone(),
            failed_at: record.failed_at.clone(),
            validation_source: Some(record.validation_source.clone()),
        };
    }

    if let Some(validated_at) = parse_rfc3339_utc(record.validated_at.as_deref()) {
        let is_fresh = now.signed_duration_since(validated_at)
            <= Duration::hours(PROVIDER_VALIDATION_TTL_HOURS);
        return ProviderValidationSummary {
            configured: true,
            validated: is_fresh,
            status: if is_fresh { "validated" } else { "stale" },
            last_error: None,
            validated_at: record.validated_at.clone(),
            failed_at: record.failed_at.clone(),
            validation_source: Some(record.validation_source.clone()),
        };
    }

    if record.failed_at.is_some() {
        return ProviderValidationSummary {
            configured: true,
            validated: false,
            status: "failed",
            last_error: record.last_error.clone(),
            validated_at: record.validated_at.clone(),
            failed_at: record.failed_at.clone(),
            validation_source: Some(record.validation_source.clone()),
        };
    }

    ProviderValidationSummary {
        configured: true,
        validated: false,
        status: "unvalidated",
        last_error: None,
        validated_at: None,
        failed_at: None,
        validation_source: Some(record.validation_source.clone()),
    }
}

pub(crate) fn metadata_safe_validation_error(value: impl AsRef<str>) -> String {
    let value = value.as_ref().trim().to_ascii_lowercase();
    if value.is_empty() {
        return "unknown_validation_error".into();
    }
    if value.contains("network_policy_disabled") {
        return "network_policy_disabled".into();
    }
    if value.contains("missing_api_key") || value.contains("api key") {
        return "missing_api_key".into();
    }
    if value.contains("timeout") || value.contains("timed out") {
        return "request_timeout".into();
    }
    if value.contains("connect") || value.contains("dns") || value.contains("tcp") {
        return "connection_failed".into();
    }
    if let Some(status) = value
        .split(|ch: char| !ch.is_ascii_digit())
        .find(|part| part.len() == 3)
    {
        return format!("http_status:{status}");
    }
    "validation_failed".into()
}

pub(crate) fn reqwest_validation_error_label(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "request_timeout".into()
    } else if error.is_connect() {
        "connection_failed".into()
    } else if error.is_builder() {
        "request_builder_failed".into()
    } else {
        "request_failed".into()
    }
}

fn record_matches_identity(
    record: &ProviderValidationRecord,
    identity: &ProviderValidationIdentity,
) -> bool {
    record.provider == identity.provider
        && record.endpoint_hash == identity.endpoint_hash
        && record.model_hash == identity.model_hash
        && record.key_present == identity.key_present
        && record.network_policy_hash == identity.network_policy_hash
}

fn normalized_provider(config: &AppConfig) -> String {
    config.llm.provider.trim().to_ascii_lowercase()
}

fn normalized_endpoint(config: &AppConfig, provider: &str) -> String {
    let base = config.llm.openai_base.trim();
    if base.is_empty() {
        default_base_for_provider(provider)
            .trim_end_matches('/')
            .to_string()
    } else {
        base.trim_end_matches('/').to_string()
    }
}

fn digest_network_policy(config: &AppConfig) -> String {
    let bytes = serde_json::to_vec(&config.system.network_policy).unwrap_or_default();
    digest_bytes(&bytes)
}

fn digest_label(value: &str) -> String {
    digest_bytes(value.trim().as_bytes())
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn parse_rfc3339_utc(value: Option<&str>) -> Option<DateTime<Utc>> {
    value
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

fn metadata_safe_label(value: String) -> String {
    let normalized = value
        .trim()
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                Some(ch)
            } else if ch.is_whitespace() {
                Some('_')
            } else {
                None
            }
        })
        .take(80)
        .collect::<String>();
    if normalized.is_empty() {
        "manual_validation".into()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::config::NetworkPolicy;

    fn configured_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.llm.provider = "deepseek".into();
        config.llm.openai_base = "https://api.deepseek.com".into();
        config.llm.openai_key = "sk-provider-secret".into();
        config.llm.chat_model = "deepseek-chat".into();
        config
    }

    #[test]
    fn configured_but_unvalidated_is_not_validated() {
        let config = configured_config();
        let summary = summarize_provider_validation(&config, None, Utc::now());

        assert!(summary.configured);
        assert!(!summary.validated);
        assert_eq!(summary.status, "unvalidated");
    }

    #[test]
    fn fresh_matching_validation_is_validated() {
        let config = configured_config();
        let now = Utc::now();
        let record = successful_provider_validation_record(&config, "manual_test", now);

        let summary = summarize_provider_validation(&config, Some(&record), now);

        assert!(summary.configured);
        assert!(summary.validated);
        assert_eq!(summary.status, "validated");
        assert_eq!(summary.validation_source.as_deref(), Some("manual_test"));
    }

    #[test]
    fn stale_validation_is_not_validated() {
        let config = configured_config();
        let old = Utc::now() - Duration::hours(PROVIDER_VALIDATION_TTL_HOURS + 1);
        let record = successful_provider_validation_record(&config, "manual_test", old);

        let summary = summarize_provider_validation(&config, Some(&record), Utc::now());

        assert!(summary.configured);
        assert!(!summary.validated);
        assert_eq!(summary.status, "stale");
    }

    #[test]
    fn provider_identity_changes_invalidate_validation() {
        let config = configured_config();
        let record = successful_provider_validation_record(&config, "manual_test", Utc::now());

        let mut provider_changed = config.clone();
        provider_changed.llm.provider = "openai".into();
        assert_eq!(
            summarize_provider_validation(&provider_changed, Some(&record), Utc::now()).status,
            "stale"
        );

        let mut base_changed = config.clone();
        base_changed.llm.openai_base = "https://example.invalid/v1".into();
        assert_eq!(
            summarize_provider_validation(&base_changed, Some(&record), Utc::now()).status,
            "stale"
        );

        let mut model_changed = config.clone();
        model_changed.llm.chat_model = "different-model".into();
        assert_eq!(
            summarize_provider_validation(&model_changed, Some(&record), Utc::now()).status,
            "stale"
        );

        let mut key_presence_changed = config.clone();
        key_presence_changed.llm.openai_key.clear();
        let summary =
            summarize_provider_validation(&key_presence_changed, Some(&record), Utc::now());
        assert!(!summary.configured);
        assert!(!summary.validated);
        assert_eq!(summary.status, "unconfigured");

        let mut network_policy_changed = config;
        network_policy_changed.system.network_policy = NetworkPolicy {
            enabled: false,
            ..Default::default()
        };
        assert_eq!(
            summarize_provider_validation(&network_policy_changed, Some(&record), Utc::now())
                .status,
            "stale"
        );
    }

    #[test]
    fn failed_validation_keeps_only_metadata_safe_error() {
        let config = configured_config();
        let record = failed_provider_validation_record(
            &config,
            "manual test/raw",
            "HTTP 401 body included sk-raw-secret",
            Utc::now(),
        );
        let serialized = serde_json::to_string(&record).unwrap();

        assert_eq!(record.last_error.as_deref(), Some("http_status:401"));
        assert_eq!(record.validation_source, "manual_testraw");
        assert!(!serialized.contains("sk-raw-secret"));
        assert!(!serialized.contains("api.deepseek.com"));
        assert!(!serialized.contains("deepseek-chat"));
        assert!(!serialized.contains("sk-provider-secret"));
    }

    #[test]
    fn validation_record_persists_metadata_safe_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("provider_validation.json");
        let config = configured_config();
        let record = successful_provider_validation_record(&config, "manual_test", Utc::now());

        save_provider_validation_record_to_path(&path, &record).unwrap();
        let loaded = load_provider_validation_record_from_path(&path).unwrap();
        let raw = std::fs::read_to_string(path).unwrap();

        assert_eq!(loaded.provider, "deepseek");
        assert!(loaded.endpoint_hash.starts_with("sha256:"));
        assert!(loaded.model_hash.starts_with("sha256:"));
        assert!(!raw.contains("api.deepseek.com"));
        assert!(!raw.contains("deepseek-chat"));
        assert!(!raw.contains("sk-provider-secret"));
    }
}
