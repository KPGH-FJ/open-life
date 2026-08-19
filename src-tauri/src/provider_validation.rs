use chrono::{DateTime, Duration, Utc};
use openlife_core::config::{AppConfig, NetworkPolicy};
use openlife_core::llm::default_base_for_provider;
use openlife_core::llm::{
    ProviderInvocationReceipt, ProviderInvocationStatus, ProviderPayloadCategory,
    ProviderPayloadPurpose, ProviderPolicyAuthority, ProviderPolicyProvenanceKind,
};
use openlife_core::network_client::NetworkPolicyDecision;
use openlife_core::scheduler::ProviderInvocationTerminalProof;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::errors::AppError;
use crate::storage::app_data_dir;

pub(crate) const PROVIDER_VALIDATION_TTL_HOURS: i64 = 24;
const PROVIDER_VALIDATION_MAX_CLOCK_SKEW_MINUTES: i64 = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderValidationIdentity {
    pub provider: String,
    pub endpoint_hash: String,
    pub model_hash: String,
    pub key_present: bool,
    pub credential_identity: String,
    #[serde(default)]
    pub credential_version: u64,
    pub network_policy_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderValidationRecord {
    pub provider: String,
    pub endpoint_hash: String,
    pub model_hash: String,
    pub key_present: bool,
    #[serde(default)]
    pub credential_identity: String,
    #[serde(default)]
    pub credential_version: u64,
    pub network_policy_hash: String,
    pub validated_at: Option<String>,
    pub failed_at: Option<String>,
    pub last_error: Option<String>,
    pub validation_source: String,
    /// Exact metadata-only terminal produced at the provider adapter edge.
    /// The response body, request content, API key, and endpoint are never
    /// copied into this durable projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_receipt: Option<ProviderInvocationReceipt>,
    /// Credential-keyed HMAC over every metadata field above. A copied or
    /// hand-edited JSON record is not an independent validation authority.
    #[serde(default)]
    pub authenticity_tag: String,
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

/// Typed load truth for the durable provider-validation projection.
///
/// Missing evidence, malformed evidence, and an unreadable store have
/// materially different product meanings.  None of them may be collapsed into
/// the legacy `None => never validated` interpretation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderValidationLoad {
    Missing,
    Valid(Box<ProviderValidationRecord>),
    Corrupt,
    IoError,
}

impl ProviderValidationLoad {
    #[cfg(test)]
    pub(crate) fn as_record(&self) -> Option<&ProviderValidationRecord> {
        match self {
            Self::Valid(record) => Some(record),
            Self::Missing | Self::Corrupt | Self::IoError => None,
        }
    }
}

pub(crate) fn provider_validation_path() -> PathBuf {
    app_data_dir().join("provider_validation.json")
}

pub(crate) fn load_provider_validation_record_from_path(path: &Path) -> ProviderValidationLoad {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return ProviderValidationLoad::Missing;
        }
        Err(_) => return ProviderValidationLoad::IoError,
    };
    match serde_json::from_str::<ProviderValidationRecord>(&text) {
        Ok(record) if validate_provider_validation_record_semantics(&record).is_ok() => {
            ProviderValidationLoad::Valid(Box::new(record))
        }
        Ok(_) | Err(_) => ProviderValidationLoad::Corrupt,
    }
}

pub(crate) fn save_provider_validation_record_to_path(
    path: &Path,
    record: &ProviderValidationRecord,
) -> Result<(), AppError> {
    validate_provider_validation_record_semantics(record).map_err(AppError::external)?;
    let text = serde_json::to_string_pretty(record).map_err(AppError::from)?;
    openlife_core::atomic_file::write_atomic(path, text.as_bytes()).map_err(AppError::from)
}

pub(crate) fn current_provider_validation_identity(
    config: &AppConfig,
) -> ProviderValidationIdentity {
    let provider = normalized_provider(config);
    let effective_key = config.effective_cloud_api_key();
    ProviderValidationIdentity {
        provider: provider.clone(),
        endpoint_hash: digest_label(&normalized_endpoint(config, &provider)),
        model_hash: digest_label(config.llm.chat_model.trim()),
        key_present: !effective_key.trim().is_empty(),
        credential_identity: openlife_core::llm::provider_credential_identity(&effective_key),
        credential_version: config.llm.credential_version,
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

pub(crate) fn failed_provider_validation_record(
    config: &AppConfig,
    validation_source: impl Into<String>,
    safe_error: impl Into<String>,
    now: DateTime<Utc>,
) -> ProviderValidationRecord {
    let identity = current_provider_validation_identity(config);
    let mut record = ProviderValidationRecord {
        provider: identity.provider,
        endpoint_hash: identity.endpoint_hash,
        model_hash: identity.model_hash,
        key_present: identity.key_present,
        credential_identity: identity.credential_identity,
        credential_version: identity.credential_version,
        network_policy_hash: identity.network_policy_hash,
        validated_at: None,
        failed_at: Some(now.to_rfc3339()),
        last_error: Some(metadata_safe_validation_error(safe_error.into())),
        validation_source: metadata_safe_label(validation_source.into()),
        invocation_receipt: None,
        authenticity_tag: String::new(),
    };
    sign_provider_validation_record(config, &mut record);
    record
}

/// Persist one validation result only when it is backed by the non-serde
/// runtime proof issued at the scheduler's prepared-provider adapter terminal.
/// A public/deserialized receipt is observation data and is never accepted as
/// validation authority. The non-clone proof is consumed so one capability
/// cannot authorize multiple durable writes.
// Validation authenticity binds the full provider generation, endpoint,
// credential, terminal proof, and observation times as separate fields.
#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
pub(crate) fn provider_validation_record_with_terminal_proof(
    config: &AppConfig,
    validation_source: impl Into<String>,
    proof: ProviderInvocationTerminalProof,
    provider_config_generation: &str,
    executed_network_policy: &NetworkPolicy,
    network_policy_decision: &NetworkPolicyDecision,
    validation_succeeded: bool,
    safe_error: Option<&str>,
    now: DateTime<Utc>,
) -> Result<ProviderValidationRecord, AppError> {
    validate_runtime_terminal_proof(
        config,
        &proof,
        provider_config_generation,
        executed_network_policy,
        network_policy_decision,
    )?;
    build_provider_validation_record_from_proof(
        config,
        validation_source,
        &proof,
        validation_succeeded,
        safe_error,
        now,
    )
}

#[cfg(test)]
#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
fn provider_validation_record_with_synthetic_test_proof(
    config: &AppConfig,
    validation_source: impl Into<String>,
    proof: ProviderInvocationTerminalProof,
    provider_config_generation: &str,
    executed_network_policy: &NetworkPolicy,
    network_policy_decision: &NetworkPolicyDecision,
    validation_succeeded: bool,
    safe_error: Option<&str>,
    now: DateTime<Utc>,
) -> Result<ProviderValidationRecord, AppError> {
    validate_synthetic_terminal_proof(
        config,
        &proof,
        provider_config_generation,
        executed_network_policy,
        network_policy_decision,
    )?;
    build_provider_validation_record_from_proof(
        config,
        validation_source,
        &proof,
        validation_succeeded,
        safe_error,
        now,
    )
}

fn build_provider_validation_record_from_proof(
    config: &AppConfig,
    validation_source: impl Into<String>,
    proof: &ProviderInvocationTerminalProof,
    validation_succeeded: bool,
    safe_error: Option<&str>,
    now: DateTime<Utc>,
) -> Result<ProviderValidationRecord, AppError> {
    let receipt = proof.receipt();
    validate_receipt_for_identity(config, receipt).map_err(AppError::external)?;
    validate_terminal_write_window(receipt.finished_at, now)?;
    if validation_succeeded && receipt.status != ProviderInvocationStatus::Completed {
        return Err(AppError::external(
            "provider validation cannot succeed without a completed receipt",
        ));
    }
    if validation_succeeded && safe_error.is_some() {
        return Err(AppError::internal(
            "successful provider validation cannot retain a failure reason",
        ));
    }
    let identity = current_provider_validation_identity(config);
    let mut record = ProviderValidationRecord {
        provider: identity.provider,
        endpoint_hash: identity.endpoint_hash,
        model_hash: identity.model_hash,
        key_present: identity.key_present,
        credential_identity: identity.credential_identity,
        credential_version: identity.credential_version,
        network_policy_hash: identity.network_policy_hash,
        // Freshness is a projection of the observed adapter terminal, never of
        // when a caller happened to replay or persist the proof. Anchoring both
        // branches here prevents an old proof from minting a new 24-hour TTL.
        validated_at: validation_succeeded.then(|| receipt.finished_at.to_rfc3339()),
        failed_at: (!validation_succeeded).then(|| receipt.finished_at.to_rfc3339()),
        last_error: (!validation_succeeded)
            .then(|| metadata_safe_validation_error(safe_error.unwrap_or("validation_failed"))),
        validation_source: metadata_safe_label(validation_source.into()),
        invocation_receipt: Some(receipt.clone()),
        authenticity_tag: String::new(),
    };
    sign_provider_validation_record(config, &mut record);
    Ok(record)
}

fn validate_terminal_write_window(
    terminal_at: DateTime<Utc>,
    caller_now: DateTime<Utc>,
) -> Result<(), AppError> {
    let wall_now = Utc::now();
    let skew = Duration::minutes(PROVIDER_VALIDATION_MAX_CLOCK_SKEW_MINUTES);
    let caller_vs_wall = caller_now.signed_duration_since(wall_now);
    let caller_vs_terminal = caller_now.signed_duration_since(terminal_at);
    if caller_vs_wall < -skew
        || caller_vs_wall > skew
        || caller_vs_terminal < -skew
        || caller_vs_terminal > skew
    {
        return Err(AppError::external(
            "provider validation terminal proof is outside the write clock boundary",
        ));
    }
    Ok(())
}

fn validate_runtime_terminal_proof(
    config: &AppConfig,
    proof: &ProviderInvocationTerminalProof,
    provider_config_generation: &str,
    executed_network_policy: &NetworkPolicy,
    network_policy_decision: &NetworkPolicyDecision,
) -> Result<(), AppError> {
    let identity = current_provider_validation_identity(config);
    let provider = normalized_provider(config);
    let endpoint = normalized_endpoint(config, &provider);
    proof
        .validate_runtime_binding(
            &provider,
            config.llm.chat_model.trim(),
            &endpoint,
            provider_config_generation,
            &identity.credential_identity,
            identity.credential_version,
            executed_network_policy,
            network_policy_decision,
        )
        .map_err(|_| AppError::external("provider terminal proof runtime binding mismatch"))
}

#[cfg(test)]
fn validate_synthetic_terminal_proof(
    config: &AppConfig,
    proof: &ProviderInvocationTerminalProof,
    provider_config_generation: &str,
    executed_network_policy: &NetworkPolicy,
    network_policy_decision: &NetworkPolicyDecision,
) -> Result<(), AppError> {
    let identity = current_provider_validation_identity(config);
    let provider = normalized_provider(config);
    let endpoint = normalized_endpoint(config, &provider);
    proof
        .validate_synthetic_test_binding(
            &provider,
            config.llm.chat_model.trim(),
            &endpoint,
            provider_config_generation,
            &identity.credential_identity,
            identity.credential_version,
            executed_network_policy,
            network_policy_decision,
        )
        .map_err(|_| AppError::external("synthetic provider terminal proof binding mismatch"))
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
    if !provider_validation_record_authenticity_valid(config, record) {
        return ProviderValidationSummary {
            configured: true,
            validated: false,
            status: "validation_record_corrupt",
            last_error: Some("provider_validation_record_unauthenticated".into()),
            validated_at: record.validated_at.clone(),
            failed_at: record.failed_at.clone(),
            validation_source: Some(record.validation_source.clone()),
        };
    }

    let receipt_status = record.invocation_receipt.as_ref().and_then(|receipt| {
        validate_receipt_for_identity(config, receipt)
            .ok()
            .map(|_| receipt.status)
    });

    if record.validated_at.is_some() && receipt_status != Some(ProviderInvocationStatus::Completed)
    {
        return ProviderValidationSummary {
            configured: true,
            validated: false,
            status: "unknown",
            last_error: Some("provider_receipt_missing_or_invalid".into()),
            validated_at: record.validated_at.clone(),
            failed_at: record.failed_at.clone(),
            validation_source: Some(record.validation_source.clone()),
        };
    }

    if receipt_status == Some(ProviderInvocationStatus::RemoteUnknown) {
        return ProviderValidationSummary {
            configured: true,
            validated: false,
            status: "remote_unknown",
            last_error: record
                .last_error
                .clone()
                .or_else(|| Some("provider_remote_state_unknown".into())),
            validated_at: record.validated_at.clone(),
            failed_at: record.failed_at.clone(),
            validation_source: Some(record.validation_source.clone()),
        };
    }

    if let Some(validated_at) = parse_rfc3339_utc(record.validated_at.as_deref()) {
        let age = now.signed_duration_since(validated_at);
        let is_fresh = age >= -Duration::minutes(PROVIDER_VALIDATION_MAX_CLOCK_SKEW_MINUTES)
            && age <= Duration::hours(PROVIDER_VALIDATION_TTL_HOURS);
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

pub(crate) fn summarize_loaded_provider_validation(
    config: &AppConfig,
    load: &ProviderValidationLoad,
    now: DateTime<Utc>,
) -> ProviderValidationSummary {
    match load {
        ProviderValidationLoad::Valid(record) => {
            summarize_provider_validation(config, Some(record), now)
        }
        ProviderValidationLoad::Missing => summarize_provider_validation(config, None, now),
        ProviderValidationLoad::Corrupt | ProviderValidationLoad::IoError => {
            let mut summary = summarize_provider_validation(config, None, now);
            let (status, error) = match load {
                ProviderValidationLoad::Corrupt => (
                    "validation_record_corrupt",
                    "provider_validation_record_corrupt",
                ),
                ProviderValidationLoad::IoError => (
                    "validation_record_io_error",
                    "provider_validation_record_unreadable",
                ),
                ProviderValidationLoad::Missing | ProviderValidationLoad::Valid(_) => {
                    unreachable!("covered by outer match")
                }
            };
            summary.validated = false;
            summary.status = status;
            summary.last_error = Some(error.into());
            summary
        }
    }
}

fn validate_receipt_for_identity(
    config: &AppConfig,
    receipt: &ProviderInvocationReceipt,
) -> Result<(), String> {
    let expected = current_provider_validation_identity(config);
    if receipt.request_id.trim().is_empty()
        || receipt.provider.trim().to_ascii_lowercase() != expected.provider
        || digest_label(&receipt.model) != expected.model_hash
        || receipt.simulated
    {
        return Err("provider validation receipt identity mismatch".into());
    }
    validate_explicit_probe_receipt_semantics(receipt)?;
    Ok(())
}

fn provider_validation_auth_material(record: &ProviderValidationRecord) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "domain": "openlife_provider_validation_v1",
        "provider": record.provider,
        "endpointHash": record.endpoint_hash,
        "modelHash": record.model_hash,
        "keyPresent": record.key_present,
        "credentialIdentity": record.credential_identity,
        "credentialVersion": record.credential_version,
        "networkPolicyHash": record.network_policy_hash,
        "validatedAt": record.validated_at,
        "failedAt": record.failed_at,
        "lastError": record.last_error,
        "validationSource": record.validation_source,
        "invocationReceipt": record.invocation_receipt,
    }))
    .expect("provider validation authentication material")
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> String {
    const BLOCK_SIZE: usize = 64;
    let mut normalized_key = [0_u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let hashed = Sha256::digest(key);
        normalized_key[..hashed.len()].copy_from_slice(&hashed);
    } else {
        normalized_key[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36_u8; BLOCK_SIZE];
    let mut outer_pad = [0x5c_u8; BLOCK_SIZE];
    for index in 0..BLOCK_SIZE {
        inner_pad[index] ^= normalized_key[index];
        outer_pad[index] ^= normalized_key[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    format!("hmac-sha256:{:x}", outer.finalize())
}

fn sign_provider_validation_record(config: &AppConfig, record: &mut ProviderValidationRecord) {
    record.authenticity_tag = hmac_sha256(
        config.effective_cloud_api_key().as_bytes(),
        &provider_validation_auth_material(record),
    );
}

fn constant_time_tag_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn provider_validation_record_authenticity_valid(
    config: &AppConfig,
    record: &ProviderValidationRecord,
) -> bool {
    if !is_hmac_sha256_tag(&record.authenticity_tag)
        || config.effective_cloud_api_key().trim().is_empty()
    {
        return false;
    }
    let expected = hmac_sha256(
        config.effective_cloud_api_key().as_bytes(),
        &provider_validation_auth_material(record),
    );
    constant_time_tag_eq(&record.authenticity_tag, &expected)
}

fn validate_provider_validation_record_semantics(
    record: &ProviderValidationRecord,
) -> Result<(), String> {
    if record.provider.trim().is_empty()
        || record.provider != record.provider.trim().to_ascii_lowercase()
        || !record
            .provider
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        || !is_sha256_digest(&record.endpoint_hash)
        || !is_sha256_digest(&record.model_hash)
        || !is_sha256_digest(&record.credential_identity)
        || !is_sha256_digest(&record.network_policy_hash)
        || !is_hmac_sha256_tag(&record.authenticity_tag)
        || record.validation_source.trim().is_empty()
        || record.validation_source.len() > 80
        || !record
            .validation_source
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err("provider validation record identity is invalid".into());
    }

    let validated_at = record
        .validated_at
        .as_deref()
        .map(DateTime::parse_from_rfc3339)
        .transpose()
        .map_err(|_| "provider validation success timestamp is invalid")?;
    let failed_at = record
        .failed_at
        .as_deref()
        .map(DateTime::parse_from_rfc3339)
        .transpose()
        .map_err(|_| "provider validation failure timestamp is invalid")?;
    if validated_at.is_some() == failed_at.is_some() {
        return Err(
            "provider validation record must contain exactly one terminal timestamp".into(),
        );
    }
    let latest_allowed = Utc::now() + Duration::minutes(PROVIDER_VALIDATION_MAX_CLOCK_SKEW_MINUTES);
    if validated_at
        .as_ref()
        .is_some_and(|timestamp| timestamp.with_timezone(&Utc) > latest_allowed)
        || failed_at
            .as_ref()
            .is_some_and(|timestamp| timestamp.with_timezone(&Utc) > latest_allowed)
    {
        return Err("provider validation terminal timestamp is in the future".into());
    }

    if let Some(error) = record.last_error.as_deref() {
        if error.trim().is_empty()
            || error.len() > 120
            || !error
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
        {
            return Err("provider validation error is not metadata-safe".into());
        }
    }

    match (validated_at, failed_at, record.invocation_receipt.as_ref()) {
        (Some(validated_at), None, Some(receipt)) => {
            if !record.key_present
                || record.last_error.is_some()
                || receipt.status != ProviderInvocationStatus::Completed
                || receipt.error_digest.is_some()
                || validated_at.with_timezone(&Utc) != receipt.finished_at
            {
                return Err("provider validation success state is incoherent".into());
            }
        }
        (Some(_), None, None) => {
            return Err("provider validation success receipt is missing".into());
        }
        (None, Some(failed_at), receipt) => {
            if record.last_error.is_none() {
                return Err("provider validation failure reason is missing".into());
            }
            if let Some(receipt) = receipt {
                let completed_but_inconsistent = receipt.status
                    == ProviderInvocationStatus::Completed
                    && receipt.error_digest.is_none()
                    && record.last_error.as_deref() == Some("provider_completion_inconsistent");
                if (receipt.status == ProviderInvocationStatus::Completed
                    && !completed_but_inconsistent)
                    || failed_at.with_timezone(&Utc) != receipt.finished_at
                {
                    return Err("provider validation failure receipt is incoherent".into());
                }
            }
        }
        (None, None, _) | (Some(_), Some(_), _) => unreachable!("validated above"),
    }

    if let Some(receipt) = record.invocation_receipt.as_ref() {
        if receipt.started_at > latest_allowed
            || receipt.finished_at > latest_allowed
            || receipt.provider.trim().to_ascii_lowercase() != record.provider
            || digest_label(&receipt.model) != record.model_hash
        {
            return Err("provider validation receipt does not match record identity".into());
        }
        validate_explicit_probe_receipt_semantics(receipt)?;
    }
    Ok(())
}

fn validate_explicit_probe_receipt_semantics(
    receipt: &ProviderInvocationReceipt,
) -> Result<(), String> {
    if receipt.request_id.trim().is_empty()
        || receipt.provider.trim().is_empty()
        || receipt.model.trim().is_empty()
        || receipt.simulated
        || receipt
            .error_digest
            .as_deref()
            .is_some_and(|digest| !is_sha256_digest(digest))
        || (receipt.status != ProviderInvocationStatus::Completed && receipt.error_digest.is_none())
    {
        return Err("provider validation receipt terminal state is invalid".into());
    }
    let evidence = receipt
        .policy_evidence
        .as_ref()
        .ok_or_else(|| "provider validation receipt policy evidence missing".to_string())?;
    evidence
        .validate_minimal_truth()
        .map_err(|_| "provider validation receipt policy evidence invalid".to_string())?;
    if evidence.issuing_authority != ProviderPolicyAuthority::ExplicitProviderProbePolicy
        || evidence.payload_purpose != Some(ProviderPayloadPurpose::ExplicitProviderProbe)
        || evidence.policy_version != "explicit_provider_probe_v1"
        || evidence.declared_payload_categories != [ProviderPayloadCategory::ExplicitProviderProbe]
        || !evidence.selected_context_refs.is_empty()
        || !evidence.included_context_categories.is_empty()
        || !evidence.policy_provenance_refs.iter().any(|reference| {
            reference.kind() == ProviderPolicyProvenanceKind::ExplicitProviderProbeDecision
        })
    {
        return Err("provider validation receipt did not come from an explicit probe".into());
    }
    Ok(())
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn is_hmac_sha256_tag(value: &str) -> bool {
    value.strip_prefix("hmac-sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

pub(crate) fn metadata_safe_validation_error(value: impl AsRef<str>) -> String {
    let value = value.as_ref().trim().to_ascii_lowercase();
    if value.is_empty() {
        return "unknown_validation_error".into();
    }
    if value.contains("network_policy_disabled") {
        return "network_policy_disabled".into();
    }
    if value.contains("runtime_generation_incoherent") {
        return "provider_runtime_generation_incoherent".into();
    }
    if value.contains("terminal_proof_missing") {
        return "provider_terminal_proof_missing".into();
    }
    if value.contains("terminal_proof_mismatch") {
        return "provider_terminal_proof_mismatch".into();
    }
    if value.contains("terminal_proof_invalid")
        || value.contains("terminal proof")
        || value.contains("terminal binding")
    {
        return "provider_terminal_proof_invalid".into();
    }
    if value.contains("completion_inconsistent") {
        return "provider_completion_inconsistent".into();
    }
    if value.contains("remote_unknown") || value.contains("remote state unknown") {
        return "provider_remote_state_unknown".into();
    }
    if value.contains("not_attempted") || value.contains("not attempted") {
        return "provider_not_attempted".into();
    }
    if value.contains("confirmed_failure") {
        return "provider_confirmed_failure".into();
    }
    if value.contains("missing_api_key") || value.contains("api key") {
        return "missing_api_key".into();
    }
    if value.contains("missing_model") {
        return "missing_model".into();
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

fn record_matches_identity(
    record: &ProviderValidationRecord,
    identity: &ProviderValidationIdentity,
) -> bool {
    record.provider == identity.provider
        && record.endpoint_hash == identity.endpoint_hash
        && record.model_hash == identity.model_hash
        && record.key_present == identity.key_present
        && record.credential_identity == identity.credential_identity
        && record.credential_version == identity.credential_version
        && record.network_policy_hash == identity.network_policy_hash
}

fn normalized_provider(config: &AppConfig) -> String {
    config.llm.provider.trim().to_ascii_lowercase()
}

fn normalized_endpoint(config: &AppConfig, provider: &str) -> String {
    let base = config.llm.openai_base.trim();
    let base = if base.is_empty() {
        default_base_for_provider(provider)
            .trim_end_matches('/')
            .to_string()
    } else {
        base.trim_end_matches('/').to_string()
    };
    openlife_core::llm::chat_completions_url(provider, &base)
}

fn digest_network_policy(config: &AppConfig) -> String {
    let provider = normalized_provider(config);
    let endpoint = normalized_endpoint(config, &provider);
    let capability = format!("provider.{provider}");
    let decision_material = openlife_core::network_client::resolve_network_policy_decision(
        &config.system.network_policy,
        &endpoint,
        &capability,
    )
    .map(|decision| decision.decision_id)
    .unwrap_or_else(|_| "network_policy_decision_invalid".into());
    digest_label(&decision_material)
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
    use openlife_core::llm::ProviderInvocationStatus;

    fn configured_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.llm.provider = "deepseek".into();
        config.llm.openai_base = "https://api.deepseek.com".into();
        config.llm.openai_key = "sk-provider-secret".into();
        config.llm.chat_model = "deepseek-chat".into();
        config.system.network_policy = NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        config
    }

    struct SyntheticTerminalFixture {
        proof: ProviderInvocationTerminalProof,
        provider_config_generation: String,
        network_policy: NetworkPolicy,
        network_policy_decision: NetworkPolicyDecision,
    }

    fn synthetic_terminal_fixture(
        config: &AppConfig,
        status: ProviderInvocationStatus,
        finished_at: DateTime<Utc>,
    ) -> SyntheticTerminalFixture {
        let scheduler = openlife_core::scheduler::InferenceScheduler::new(
            config.local_model.clone(),
            false,
            config.llm.provider.clone(),
            config.llm.openai_base.clone(),
            config.llm.openai_key.clone(),
            config.llm.chat_model.clone(),
            config.llm.embedding_model.clone(),
            false,
        )
        .with_provider_credential_version(config.llm.credential_version);
        let permission_store =
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let scheduler = permission_store.bind_explicit_provider_probe_scheduler(scheduler);
        let endpoint =
            openlife_core::llm::chat_completions_url(&config.llm.provider, &config.llm.openai_base);
        let decision = openlife_core::network_client::resolve_network_policy_decision(
            &config.system.network_policy,
            &endpoint,
            &format!("provider.{}", config.llm.provider),
        )
        .unwrap();
        let challenge = scheduler.explicit_provider_probe_challenge().unwrap();
        let grant = permission_store
            .issue_explicit_provider_probe_grant(
                challenge,
                config.system.network_policy.clone(),
                &decision,
                decision.clone(),
                None,
            )
            .unwrap();
        let prepared = scheduler.prepare_explicit_provider_probe(grant).unwrap();
        let provider_config_generation = prepared.provider_config_generation.clone();
        let network_policy = prepared.network_policy.clone();
        let network_policy_decision = prepared.network_policy_decision.clone();
        let proof = scheduler
            .synthetic_explicit_probe_terminal_proof_for_test(prepared, status, finished_at)
            .unwrap();
        SyntheticTerminalFixture {
            proof,
            provider_config_generation,
            network_policy,
            network_policy_decision,
        }
    }

    fn successful_record(config: &AppConfig, now: DateTime<Utc>) -> ProviderValidationRecord {
        let fixture = synthetic_terminal_fixture(config, ProviderInvocationStatus::Completed, now);
        provider_validation_record_with_synthetic_test_proof(
            config,
            "manual_test",
            fixture.proof,
            &fixture.provider_config_generation,
            &fixture.network_policy,
            &fixture.network_policy_decision,
            true,
            None,
            now,
        )
        .unwrap()
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
        let record = successful_record(&config, now);

        let summary = summarize_provider_validation(&config, Some(&record), now);

        assert!(summary.configured);
        assert!(summary.validated);
        assert_eq!(summary.status, "validated");
        assert_eq!(summary.validation_source.as_deref(), Some("manual_test"));
    }

    #[test]
    fn validation_freshness_is_anchored_to_the_adapter_terminal() {
        let config = configured_config();
        let caller_now = Utc::now();
        let terminal_at = caller_now - Duration::minutes(1);
        let fixture =
            synthetic_terminal_fixture(&config, ProviderInvocationStatus::Completed, terminal_at);

        let record = provider_validation_record_with_synthetic_test_proof(
            &config,
            "manual_test",
            fixture.proof,
            &fixture.provider_config_generation,
            &fixture.network_policy,
            &fixture.network_policy_decision,
            true,
            None,
            caller_now,
        )
        .unwrap();

        let terminal_text = terminal_at.to_rfc3339();
        let caller_text = caller_now.to_rfc3339();
        assert_eq!(record.validated_at.as_deref(), Some(terminal_text.as_str()));
        assert_ne!(record.validated_at.as_deref(), Some(caller_text.as_str()));
    }

    #[test]
    fn explicit_probe_validation_survives_wall_clock_rollback() {
        let config = configured_config();
        let terminal_at = Utc::now();
        let scheduler = openlife_core::scheduler::InferenceScheduler::new(
            config.local_model.clone(),
            false,
            config.llm.provider.clone(),
            config.llm.openai_base.clone(),
            config.llm.openai_key.clone(),
            config.llm.chat_model.clone(),
            config.llm.embedding_model.clone(),
            false,
        )
        .with_provider_credential_version(config.llm.credential_version);
        let permission_store =
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let scheduler = permission_store.bind_explicit_provider_probe_scheduler(scheduler);
        let endpoint =
            openlife_core::llm::chat_completions_url(&config.llm.provider, &config.llm.openai_base);
        let decision = openlife_core::network_client::resolve_network_policy_decision(
            &config.system.network_policy,
            &endpoint,
            &format!("provider.{}", config.llm.provider),
        )
        .unwrap();
        let challenge = scheduler.explicit_provider_probe_challenge().unwrap();
        let grant = permission_store
            .issue_explicit_provider_probe_grant(
                challenge,
                config.system.network_policy.clone(),
                &decision,
                decision.clone(),
                None,
            )
            .unwrap();
        let prepared = scheduler.prepare_explicit_provider_probe(grant).unwrap();
        let provider_config_generation = prepared.provider_config_generation.clone();
        let network_policy = prepared.network_policy.clone();
        let network_policy_decision = prepared.network_policy_decision.clone();
        let proof = scheduler
            .synthetic_explicit_probe_terminal_proof_with_started_at_for_test(
                prepared,
                ProviderInvocationStatus::Completed,
                terminal_at + Duration::milliseconds(1),
                terminal_at,
            )
            .expect("typed terminal proof tolerates a backwards wall clock");

        let record = provider_validation_record_with_synthetic_test_proof(
            &config,
            "manual_test",
            proof,
            &provider_config_generation,
            &network_policy,
            &network_policy_decision,
            true,
            None,
            terminal_at,
        )
        .expect("provider validation accepts proof observation order across clock rollback");
        let receipt = record
            .invocation_receipt
            .expect("durable validation receipt");
        assert!(receipt.finished_at < receipt.started_at);
        assert_eq!(receipt.status, ProviderInvocationStatus::Completed);
    }

    #[test]
    fn replaying_an_old_terminal_proof_cannot_refresh_validation_ttl() {
        let config = configured_config();
        let now = Utc::now();
        let terminal_at = now - Duration::hours(PROVIDER_VALIDATION_TTL_HOURS + 1);
        let fixture =
            synthetic_terminal_fixture(&config, ProviderInvocationStatus::Completed, terminal_at);

        assert!(provider_validation_record_with_synthetic_test_proof(
            &config,
            "manual_test",
            fixture.proof,
            &fixture.provider_config_generation,
            &fixture.network_policy,
            &fixture.network_policy_decision,
            true,
            None,
            now,
        )
        .is_err());
    }

    #[test]
    fn a_future_caller_timestamp_cannot_extend_provider_validation() {
        let config = configured_config();
        let terminal_at = Utc::now();
        let fixture =
            synthetic_terminal_fixture(&config, ProviderInvocationStatus::Completed, terminal_at);

        assert!(provider_validation_record_with_synthetic_test_proof(
            &config,
            "manual_test",
            fixture.proof,
            &fixture.provider_config_generation,
            &fixture.network_policy,
            &fixture.network_policy_decision,
            true,
            None,
            terminal_at + Duration::hours(PROVIDER_VALIDATION_TTL_HOURS),
        )
        .is_err());
    }

    #[test]
    fn stale_validation_is_not_validated() {
        let config = configured_config();
        let now = Utc::now();
        let old = now - Duration::hours(PROVIDER_VALIDATION_TTL_HOURS + 1);
        let mut record = successful_record(&config, now);
        let receipt = record
            .invocation_receipt
            .as_mut()
            .expect("successful fixture has a receipt");
        let elapsed = receipt
            .finished_at
            .signed_duration_since(receipt.started_at);
        receipt.finished_at = old;
        receipt.started_at = old - elapsed;
        record.validated_at = Some(old.to_rfc3339());
        sign_provider_validation_record(&config, &mut record);

        let summary = summarize_provider_validation(&config, Some(&record), now);

        assert!(summary.configured);
        assert!(!summary.validated);
        assert_eq!(summary.status, "stale");
    }

    #[test]
    fn provider_identity_changes_invalidate_validation() {
        let config = configured_config();
        let record = successful_record(&config, Utc::now());

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

        let mut path_changed = config.clone();
        path_changed.llm.openai_base = "https://api.deepseek.com/v2".into();
        assert_eq!(
            summarize_provider_validation(&path_changed, Some(&record), Utc::now()).status,
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

        let mut credential_changed = config.clone();
        credential_changed.llm.openai_key = "sk-replaced-provider-secret".into();
        credential_changed.llm.credential_version =
            credential_changed.llm.credential_version.saturating_add(1);
        assert_eq!(
            summarize_provider_validation(&credential_changed, Some(&record), Utc::now()).status,
            "stale"
        );

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
    fn replacing_key_without_a_version_bump_still_invalidates_validation() {
        let config = configured_config();
        let now = Utc::now();
        let record = successful_record(&config, now);
        let original_identity = current_provider_validation_identity(&config);

        let mut replaced = config;
        replaced.llm.openai_key = "sk-different-same-version".into();
        let replaced_identity = current_provider_validation_identity(&replaced);

        assert_eq!(
            original_identity.credential_version,
            replaced_identity.credential_version
        );
        assert_ne!(
            original_identity.credential_identity,
            replaced_identity.credential_identity
        );
        assert_eq!(
            summarize_provider_validation(&replaced, Some(&record), now).status,
            "stale"
        );
    }

    #[test]
    fn terminal_proof_is_exactly_bound_to_generation_endpoint_and_credential() {
        let config_a = configured_config();
        let now = Utc::now();
        let generation_fixture =
            synthetic_terminal_fixture(&config_a, ProviderInvocationStatus::Completed, now);

        assert!(provider_validation_record_with_synthetic_test_proof(
            &config_a,
            "manual_test",
            generation_fixture.proof,
            "different-provider-generation",
            &generation_fixture.network_policy,
            &generation_fixture.network_policy_decision,
            true,
            None,
            now,
        )
        .is_err());

        let mut endpoint_b = config_a.clone();
        endpoint_b.llm.openai_base = "https://different.example/v1".into();
        let endpoint_fixture =
            synthetic_terminal_fixture(&config_a, ProviderInvocationStatus::Completed, now);
        assert!(provider_validation_record_with_synthetic_test_proof(
            &endpoint_b,
            "manual_test",
            endpoint_fixture.proof,
            &endpoint_fixture.provider_config_generation,
            &endpoint_fixture.network_policy,
            &endpoint_fixture.network_policy_decision,
            true,
            None,
            now,
        )
        .is_err());

        let mut credential_b = config_a.clone();
        credential_b.llm.openai_key = "sk-different-provider-secret".into();
        credential_b.llm.credential_version = credential_b.llm.credential_version.saturating_add(1);
        let credential_fixture =
            synthetic_terminal_fixture(&config_a, ProviderInvocationStatus::Completed, now);
        assert!(provider_validation_record_with_synthetic_test_proof(
            &credential_b,
            "manual_test",
            credential_fixture.proof,
            &credential_fixture.provider_config_generation,
            &credential_fixture.network_policy,
            &credential_fixture.network_policy_decision,
            true,
            None,
            now,
        )
        .is_err());

        let policy_fixture =
            synthetic_terminal_fixture(&config_a, ProviderInvocationStatus::Completed, now);
        let mut different_executed_policy = policy_fixture.network_policy.clone();
        different_executed_policy.default_decision = "deny".into();
        assert!(provider_validation_record_with_synthetic_test_proof(
            &config_a,
            "manual_test",
            policy_fixture.proof,
            &policy_fixture.provider_config_generation,
            &different_executed_policy,
            &policy_fixture.network_policy_decision,
            true,
            None,
            now,
        )
        .is_err());

        let decision_fixture =
            synthetic_terminal_fixture(&config_a, ProviderInvocationStatus::Completed, now);
        let different_decision = openlife_core::network_client::resolve_network_policy_decision(
            &decision_fixture.network_policy,
            "https://api.deepseek.com/v1/chat/completions",
            "provider.different",
        )
        .unwrap();
        assert!(provider_validation_record_with_synthetic_test_proof(
            &config_a,
            "manual_test",
            decision_fixture.proof,
            &decision_fixture.provider_config_generation,
            &decision_fixture.network_policy,
            &different_decision,
            true,
            None,
            now,
        )
        .is_err());
    }

    #[test]
    fn synthetic_and_deserialized_receipts_cannot_cross_the_production_write_api() {
        let config = configured_config();
        let now = Utc::now();
        let fixture = synthetic_terminal_fixture(&config, ProviderInvocationStatus::Completed, now);
        let receipt_json = serde_json::to_vec(fixture.proof.receipt()).unwrap();
        let deserialized: ProviderInvocationReceipt =
            serde_json::from_slice(&receipt_json).unwrap();

        assert_eq!(&deserialized, fixture.proof.receipt());
        assert!(validate_receipt_for_identity(&config, &deserialized).is_ok());
        let mut manually_forged_receipt = deserialized.clone();
        manually_forged_receipt.status = ProviderInvocationStatus::Failed;
        manually_forged_receipt.error_digest = Some(format!("sha256:{}", "a".repeat(64)));
        assert!(
            validate_receipt_for_identity(&config, &manually_forged_receipt).is_ok(),
            "a structurally valid public receipt is still not proof authority"
        );
        assert!(
            provider_validation_record_with_terminal_proof(
                &config,
                "manual_test",
                fixture.proof,
                &fixture.provider_config_generation,
                &fixture.network_policy,
                &fixture.network_policy_decision,
                true,
                None,
                now,
            )
            .is_err(),
            "production validation must reject a synthetic proof origin"
        );

        let validation_source = include_str!("provider_validation.rs");
        let receipt_writer = ["fn provider_validation_record_with_", "receipt("].concat();
        assert!(
            !validation_source.contains(&receipt_writer),
            "no durable write API may accept a public receipt"
        );
        assert!(validation_source.contains("proof: ProviderInvocationTerminalProof"));

        let scheduler_source = include_str!("../../openlife-core/src/scheduler.rs");
        let proof_declaration = scheduler_source
            .split("pub struct ProviderInvocationTerminalProof")
            .nth(1)
            .and_then(|tail| {
                tail.split("struct ProviderInvocationTerminalBinding")
                    .next()
            })
            .expect("terminal proof declaration must remain present");
        assert!(!proof_declaration.contains("pub receipt:"));
        assert!(!proof_declaration.contains("Serialize"));
        assert!(!proof_declaration.contains("Deserialize"));
        assert!(scheduler_source.contains("#[cfg(feature = \"test-utils\")]"));
    }

    #[test]
    fn future_timestamp_is_rejected_even_with_a_valid_authenticity_tag() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("provider_validation.json");
        let config = configured_config();
        let now = Utc::now();
        let mut record = successful_record(&config, now);
        record.validated_at = Some("2999-01-01T00:00:00Z".into());
        sign_provider_validation_record(&config, &mut record);

        assert!(save_provider_validation_record_to_path(&path, &record).is_err());
        std::fs::write(&path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
        assert_eq!(
            load_provider_validation_record_from_path(&path),
            ProviderValidationLoad::Corrupt
        );
        let summary = summarize_provider_validation(&config, Some(&record), now);
        assert!(!summary.validated);
        assert_eq!(summary.status, "stale");
    }

    #[test]
    fn editing_a_signed_validation_record_invalidates_authenticity() {
        let config = configured_config();
        let now = Utc::now();
        let mut record = successful_record(&config, now);
        record.validation_source = "fabricated_source".into();

        let summary = summarize_provider_validation(&config, Some(&record), now);
        assert!(!summary.validated);
        assert_eq!(summary.status, "validation_record_corrupt");
        assert_eq!(
            summary.last_error.as_deref(),
            Some("provider_validation_record_unauthenticated")
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
    fn remote_unknown_receipt_never_projects_provider_as_validated() {
        let config = configured_config();
        let now = Utc::now();
        let fixture =
            synthetic_terminal_fixture(&config, ProviderInvocationStatus::RemoteUnknown, now);
        let record = provider_validation_record_with_synthetic_test_proof(
            &config,
            "manual_test",
            fixture.proof,
            &fixture.provider_config_generation,
            &fixture.network_policy,
            &fixture.network_policy_decision,
            false,
            Some("provider_remote_state_unknown"),
            now,
        )
        .unwrap();

        let summary = summarize_provider_validation(&config, Some(&record), now);
        assert!(!summary.validated);
        assert_eq!(summary.status, "remote_unknown");
    }

    #[test]
    fn confirmed_failure_and_inconsistent_completion_keep_proof_terminal_time() {
        let config = configured_config();
        let now = Utc::now();
        let failed_fixture =
            synthetic_terminal_fixture(&config, ProviderInvocationStatus::Failed, now);
        let failed = provider_validation_record_with_synthetic_test_proof(
            &config,
            "manual_test",
            failed_fixture.proof,
            &failed_fixture.provider_config_generation,
            &failed_fixture.network_policy,
            &failed_fixture.network_policy_decision,
            false,
            Some("provider_confirmed_failure"),
            now,
        )
        .unwrap();
        let terminal_text = now.to_rfc3339();
        assert_eq!(failed.failed_at.as_deref(), Some(terminal_text.as_str()));
        assert_eq!(
            failed
                .invocation_receipt
                .as_ref()
                .map(|receipt| receipt.status),
            Some(ProviderInvocationStatus::Failed)
        );

        let completed_fixture =
            synthetic_terminal_fixture(&config, ProviderInvocationStatus::Completed, now);
        let inconsistent = provider_validation_record_with_synthetic_test_proof(
            &config,
            "manual_test",
            completed_fixture.proof,
            &completed_fixture.provider_config_generation,
            &completed_fixture.network_policy,
            &completed_fixture.network_policy_decision,
            false,
            Some("provider_completion_inconsistent"),
            now,
        )
        .unwrap();
        assert_eq!(
            inconsistent.last_error.as_deref(),
            Some("provider_completion_inconsistent")
        );
        let inconsistent_terminal_text = inconsistent
            .invocation_receipt
            .as_ref()
            .map(|receipt| receipt.finished_at.to_rfc3339())
            .unwrap();
        assert_eq!(
            inconsistent.failed_at.as_deref(),
            Some(inconsistent_terminal_text.as_str())
        );
        assert!(validate_provider_validation_record_semantics(&inconsistent).is_ok());
    }

    #[test]
    fn unauthenticated_legacy_success_is_corrupt_not_validated() {
        let config = configured_config();
        let now = Utc::now();
        let identity = current_provider_validation_identity(&config);
        let record = ProviderValidationRecord {
            provider: identity.provider,
            endpoint_hash: identity.endpoint_hash,
            model_hash: identity.model_hash,
            key_present: identity.key_present,
            credential_identity: identity.credential_identity,
            credential_version: identity.credential_version,
            network_policy_hash: identity.network_policy_hash,
            validated_at: Some(now.to_rfc3339()),
            failed_at: None,
            last_error: None,
            validation_source: "legacy_manual_test".into(),
            invocation_receipt: None,
            authenticity_tag: String::new(),
        };

        let summary = summarize_provider_validation(&config, Some(&record), now);
        assert!(!summary.validated);
        assert_eq!(summary.status, "validation_record_corrupt");
    }

    #[test]
    fn validation_record_persists_metadata_safe_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("provider_validation.json");
        let config = configured_config();
        let record = successful_record(&config, Utc::now());

        save_provider_validation_record_to_path(&path, &record).unwrap();
        let load = load_provider_validation_record_from_path(&path);
        let loaded = load
            .as_record()
            .expect("saved validation record must load as valid");
        let raw = std::fs::read_to_string(path).unwrap();

        assert_eq!(loaded.provider, "deepseek");
        assert_eq!(
            loaded
                .invocation_receipt
                .as_ref()
                .map(|receipt| receipt.status),
            Some(ProviderInvocationStatus::Completed)
        );
        assert!(loaded.endpoint_hash.starts_with("sha256:"));
        assert!(loaded.model_hash.starts_with("sha256:"));
        assert!(loaded.credential_identity.starts_with("sha256:"));
        assert!(loaded.authenticity_tag.starts_with("hmac-sha256:"));
        assert!(!raw.contains("api.deepseek.com"));
        assert!(!raw.contains("sk-provider-secret"));
        assert!(!raw.contains("ping"));
    }

    #[test]
    fn validation_load_distinguishes_missing_corrupt_and_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.json");
        assert_eq!(
            load_provider_validation_record_from_path(&missing),
            ProviderValidationLoad::Missing
        );

        let corrupt = dir.path().join("corrupt.json");
        std::fs::write(&corrupt, b"{not-provider-validation-json").unwrap();
        assert_eq!(
            load_provider_validation_record_from_path(&corrupt),
            ProviderValidationLoad::Corrupt
        );

        let semantic_corrupt = dir.path().join("semantic-corrupt.json");
        let empty_shell = ProviderValidationRecord {
            provider: String::new(),
            endpoint_hash: String::new(),
            model_hash: String::new(),
            key_present: false,
            credential_identity: String::new(),
            credential_version: 0,
            network_policy_hash: String::new(),
            validated_at: None,
            failed_at: None,
            last_error: None,
            validation_source: String::new(),
            invocation_receipt: None,
            authenticity_tag: String::new(),
        };
        std::fs::write(&semantic_corrupt, serde_json::to_vec(&empty_shell).unwrap()).unwrap();
        assert_eq!(
            load_provider_validation_record_from_path(&semantic_corrupt),
            ProviderValidationLoad::Corrupt,
            "syntactically valid JSON without coherent validation truth must fail closed"
        );

        let unreadable = dir.path().join("validation-directory");
        std::fs::create_dir(&unreadable).unwrap();
        assert_eq!(
            load_provider_validation_record_from_path(&unreadable),
            ProviderValidationLoad::IoError
        );

        let config = configured_config();
        assert_eq!(
            summarize_loaded_provider_validation(
                &config,
                &ProviderValidationLoad::Corrupt,
                Utc::now(),
            )
            .status,
            "validation_record_corrupt"
        );
        assert_eq!(
            summarize_loaded_provider_validation(
                &config,
                &ProviderValidationLoad::IoError,
                Utc::now(),
            )
            .status,
            "validation_record_io_error"
        );
    }
}
