use crate::config::NetworkPolicy;
use crate::network_client::{NetworkClient, NetworkClientPolicy, NetworkPolicyDecision};
use crate::vectors::EmbeddingPrivacyPlan;
use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::Duration;

pub const UNKNOWN_EMBEDDING_PROFILE_ID: &str = "unknown";
pub const DETERMINISTIC_HASH_MODEL_V1: &str = "openlife-hash-ngram-v1";
pub const DETERMINISTIC_HASH_DIMENSION_V1: usize = 384;
pub const DETERMINISTIC_HASH_ARTIFACT_V1: &str = "openlife-hash-ngram-artifact-v1";
const BUILTIN_DEPLOYMENT_IDENTITY: &str = "builtin:openlife";
const MAX_EMBEDDING_TEXT_CHARS: usize = 32_768;
const MAX_EMBEDDING_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_EMBEDDING_DIMENSION: usize = 32_768;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingRouteKind {
    Unknown,
    Cloud,
    Ollama,
    DeterministicHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingRoutePreference {
    Cloud,
    Ollama,
    DeterministicHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingProfile {
    pub id: String,
    pub route: EmbeddingRouteKind,
    pub provider: String,
    pub model: String,
    /// Digest-like identity of the concrete endpoint/deployment. The raw URL is
    /// deliberately not copied into receipts or vector metadata.
    pub deployment_identity: String,
    /// Provider revision, immutable model digest, or an explicit provider model
    /// contract. This is separate from the human-readable model route.
    pub model_artifact_identity: String,
    pub dimension: usize,
}

impl EmbeddingProfile {
    pub fn new(
        route: EmbeddingRouteKind,
        provider: impl Into<String>,
        model: impl Into<String>,
        deployment_identity: impl Into<String>,
        model_artifact_identity: impl Into<String>,
        dimension: usize,
    ) -> Result<Self> {
        if route == EmbeddingRouteKind::Unknown {
            anyhow::bail!("embedding_profile_route_unknown");
        }
        let provider = provider.into();
        let model = model.into();
        let deployment_identity = deployment_identity.into();
        let model_artifact_identity = model_artifact_identity.into();
        validate_profile_label("provider", &provider)?;
        validate_profile_label("model", &model)?;
        validate_profile_identity("deployment_identity", &deployment_identity)?;
        validate_profile_identity("model_artifact_identity", &model_artifact_identity)?;
        if dimension == 0 {
            anyhow::bail!("embedding_profile_dimension_unknown");
        }
        if dimension > MAX_EMBEDDING_DIMENSION {
            anyhow::bail!("embedding_profile_dimension_limit_exceeded");
        }
        Ok(Self {
            id: profile_id(
                route,
                &provider,
                &model,
                &deployment_identity,
                &model_artifact_identity,
                dimension,
            ),
            route,
            provider,
            model,
            deployment_identity,
            model_artifact_identity,
            dimension,
        })
    }

    pub fn unknown() -> Self {
        Self {
            id: UNKNOWN_EMBEDDING_PROFILE_ID.into(),
            route: EmbeddingRouteKind::Unknown,
            provider: UNKNOWN_EMBEDDING_PROFILE_ID.into(),
            model: UNKNOWN_EMBEDDING_PROFILE_ID.into(),
            deployment_identity: UNKNOWN_EMBEDDING_PROFILE_ID.into(),
            model_artifact_identity: UNKNOWN_EMBEDDING_PROFILE_ID.into(),
            dimension: 0,
        }
    }

    pub fn validate_known_identity(&self) -> Result<()> {
        if self.route == EmbeddingRouteKind::Unknown
            || self.id == UNKNOWN_EMBEDDING_PROFILE_ID
            || self.dimension == 0
            || self.dimension > MAX_EMBEDDING_DIMENSION
        {
            anyhow::bail!("embedding_profile_unknown");
        }
        validate_profile_label("provider", &self.provider)?;
        validate_profile_label("model", &self.model)?;
        validate_profile_identity("deployment_identity", &self.deployment_identity)?;
        validate_profile_identity("model_artifact_identity", &self.model_artifact_identity)?;
        let expected = profile_id(
            self.route,
            &self.provider,
            &self.model,
            &self.deployment_identity,
            &self.model_artifact_identity,
            self.dimension,
        );
        if self.id != expected {
            anyhow::bail!("embedding_profile_identity_mismatch");
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddingRouteConfig {
    pub preferred_route: EmbeddingRoutePreference,
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub expected_dimension: Option<usize>,
    /// Optional immutable model revision/artifact supplied by an advanced
    /// provider configuration. Local mutable tags are never promoted to an
    /// immutable identity without this value or a model digest.
    pub model_artifact_identity: Option<String>,
    /// Credential presence is resolved before route selection. The secret itself
    /// never enters a serializable request or route receipt.
    pub cloud_credentials_available: bool,
    cloud_authority: Option<CloudEmbeddingAuthority>,
}

#[derive(Debug, Clone)]
struct CloudEmbeddingAuthority {
    api_key: String,
    credential_identity: String,
    credential_version: u64,
    network_policy: NetworkPolicy,
}

impl EmbeddingRouteConfig {
    pub fn from_product_config(
        provider: impl Into<String>,
        endpoint: impl Into<String>,
        model: impl Into<String>,
        embedding_enabled: bool,
        configured_key: &str,
        credential_version: u64,
        network_policy: NetworkPolicy,
    ) -> Self {
        let provider = provider.into();
        let endpoint = endpoint.into();
        let preferred_route = if !embedding_enabled {
            EmbeddingRoutePreference::DeterministicHash
        } else if provider.eq_ignore_ascii_case("ollama") {
            EmbeddingRoutePreference::Ollama
        } else {
            EmbeddingRoutePreference::Cloud
        };
        let effective_key =
            crate::llm::effective_api_key_for_endpoint(&provider, &endpoint, configured_key);
        let cloud_credentials_available = !effective_key.trim().is_empty();
        let cloud_authority = cloud_credentials_available.then(|| CloudEmbeddingAuthority {
            credential_identity: crate::llm::provider_credential_identity(&effective_key),
            api_key: effective_key,
            credential_version,
            network_policy,
        });
        Self {
            preferred_route,
            provider,
            endpoint,
            model: model.into(),
            expected_dimension: None,
            model_artifact_identity: None,
            cloud_credentials_available,
            cloud_authority,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingRouteDecision {
    pub request_id: String,
    pub route: EmbeddingRouteKind,
    pub provider: String,
    pub endpoint: Option<String>,
    pub model: String,
    pub expected_dimension: Option<usize>,
    pub deployment_identity: Option<String>,
    pub model_artifact_identity: Option<String>,
    pub reason_code: String,
    pub privacy_blocking_reasons: Vec<String>,
    /// Metadata-only execution identity sealed at preparation time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_policy_decision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_version: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct PreparedEmbeddingRequest {
    pub decision: EmbeddingRouteDecision,
    pub embedding_text: String,
    cloud_execution: Option<CloudEmbeddingExecutionBinding>,
}

#[derive(Debug, Clone)]
struct CloudEmbeddingExecutionBinding {
    endpoint: String,
    api_key: String,
    credential_identity: String,
    credential_version: u64,
    network_policy: NetworkPolicy,
    network_policy_decision: NetworkPolicyDecision,
    prepared_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingInvocationStatus {
    NotAttempted,
    Completed,
    Failed,
    RemoteUnknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingInvocationSource {
    CacheHit,
    CloudProvider,
    Ollama,
    DeterministicHash,
    RouteInvalid,
    PreDispatchRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingProviderDispatchKind {
    ModelManifest,
    Embedding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingProviderDispatch {
    pub kind: EmbeddingProviderDispatchKind,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingInvocationReceipt {
    pub request_id: String,
    pub route: EmbeddingRouteKind,
    pub provider: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_identity: Option<String>,
    pub profile_id: String,
    pub status: EmbeddingInvocationStatus,
    pub source: EmbeddingInvocationSource,
    pub route_reason_code: String,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: chrono::DateTime<chrono::Utc>,
    pub error_digest: Option<String>,
    pub cache_hit: bool,
    /// Minimal provider-edge facts. This never includes request text, model
    /// input, response content, or credentials.
    #[serde(default)]
    pub provider_dispatches: Vec<EmbeddingProviderDispatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_policy_decision_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_version: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct EmbeddingOutcome {
    pub profile: EmbeddingProfile,
    pub receipt: EmbeddingInvocationReceipt,
    pub result: std::result::Result<Vec<f32>, String>,
}

#[derive(Debug, Clone)]
// Both outcomes are consumed immediately; keeping them inline avoids an
// allocation on every embedding request.
#[expect(
    clippy::large_enum_variant,
    reason = "owner=backend-platform; expires=2026-10-01; measured inline allocation tradeoff"
)]
pub enum PreparedEmbeddingRequestOutcome {
    Prepared(PreparedEmbeddingRequest),
    Rejected(EmbeddingOutcome),
}

impl EmbeddingOutcome {
    pub fn into_result(self) -> std::result::Result<(Vec<f32>, EmbeddingProfile), String> {
        self.result.map(|embedding| (embedding, self.profile))
    }
}

#[derive(Clone)]
struct EmbeddingCacheEntry {
    embedding: Vec<f32>,
    profile: EmbeddingProfile,
    /// Ollama cache entries may only bypass `/api/embed` after the vector was
    /// produced between two identical manifest observations. Other routes do
    /// not use this flag.
    ollama_artifact_stability_verified: bool,
    cached_at: std::time::Instant,
}

struct EmbeddingCache {
    entries: std::collections::HashMap<String, EmbeddingCacheEntry>,
    access_order: Vec<String>,
    max_size: usize,
    ttl: Duration,
}

impl EmbeddingCache {
    fn new(max_size: usize, ttl: Duration) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            access_order: Vec::new(),
            max_size,
            ttl,
        }
    }

    fn get(&mut self, key: &str) -> Option<(Vec<f32>, EmbeddingProfile, bool)> {
        let entry = self.entries.get(key)?.clone();
        if entry.cached_at.elapsed() >= self.ttl {
            self.entries.remove(key);
            self.access_order.retain(|item| item != key);
            return None;
        }
        self.access_order.retain(|item| item != key);
        self.access_order.push(key.to_string());
        Some((
            entry.embedding,
            entry.profile,
            entry.ollama_artifact_stability_verified,
        ))
    }

    fn put(
        &mut self,
        key: String,
        embedding: Vec<f32>,
        profile: EmbeddingProfile,
        ollama_artifact_stability_verified: bool,
    ) {
        self.access_order.retain(|item| item != &key);
        while self.entries.len() >= self.max_size && !self.access_order.is_empty() {
            let oldest = self.access_order.remove(0);
            self.entries.remove(&oldest);
        }
        self.entries.insert(
            key.clone(),
            EmbeddingCacheEntry {
                embedding,
                profile,
                ollama_artifact_stability_verified,
                cached_at: std::time::Instant::now(),
            },
        );
        self.access_order.push(key);
    }
}

static EMBEDDING_CACHE: std::sync::OnceLock<std::sync::Mutex<EmbeddingCache>> =
    std::sync::OnceLock::new();

fn embedding_cache() -> &'static std::sync::Mutex<EmbeddingCache> {
    EMBEDDING_CACHE.get_or_init(|| {
        std::sync::Mutex::new(EmbeddingCache::new(1_000, Duration::from_secs(3600)))
    })
}

pub fn clear_embedding_cache() {
    let mut cache = embedding_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache.entries.clear();
    cache.access_order.clear();
}

pub fn prepare_embedding_request(
    text: &str,
    config: EmbeddingRouteConfig,
    privacy_plan: EmbeddingPrivacyPlan,
) -> Result<PreparedEmbeddingRequest> {
    prepare_embedding_request_with_id(text, config, privacy_plan, uuid::Uuid::new_v4().to_string())
}

/// Prepare an embedding request without losing the pre-dispatch fact when
/// validation rejects the input. Product memory paths use this form so a
/// preparation error remains observable as `not_attempted`.
pub fn prepare_embedding_request_recorded(
    text: &str,
    config: EmbeddingRouteConfig,
    privacy_plan: EmbeddingPrivacyPlan,
) -> PreparedEmbeddingRequestOutcome {
    let request_id = uuid::Uuid::new_v4().to_string();
    match prepare_embedding_request_with_id(text, config, privacy_plan, request_id.clone()) {
        Ok(prepared) => PreparedEmbeddingRequestOutcome::Prepared(prepared),
        Err(error) => PreparedEmbeddingRequestOutcome::Rejected(failed_outcome(
            &EmbeddingRouteDecision {
                request_id,
                route: EmbeddingRouteKind::Unknown,
                provider: UNKNOWN_EMBEDDING_PROFILE_ID.into(),
                endpoint: None,
                model: UNKNOWN_EMBEDDING_PROFILE_ID.into(),
                expected_dimension: None,
                deployment_identity: None,
                model_artifact_identity: None,
                reason_code: "embedding_prepare_rejected".into(),
                privacy_blocking_reasons: Vec::new(),
                network_policy_decision_id: None,
                credential_identity: None,
                credential_version: None,
            },
            EmbeddingProfile::unknown(),
            EmbeddingInvocationSource::PreDispatchRejected,
            None,
            error.to_string(),
        )),
    }
}

fn prepare_embedding_request_with_id(
    text: &str,
    config: EmbeddingRouteConfig,
    privacy_plan: EmbeddingPrivacyPlan,
    request_id: String,
) -> Result<PreparedEmbeddingRequest> {
    if text.trim().is_empty() || privacy_plan.embedding_text.trim().is_empty() {
        anyhow::bail!("embedding_text_empty");
    }
    if text.chars().count() > MAX_EMBEDDING_TEXT_CHARS
        || privacy_plan.embedding_text.chars().count() > MAX_EMBEDDING_TEXT_CHARS
    {
        anyhow::bail!("embedding_text_too_large");
    }
    let cloud_authority = config.cloud_authority.clone();
    let (
        route,
        provider,
        endpoint,
        model,
        expected_dimension,
        deployment_identity,
        model_artifact_identity,
        reason_code,
    ) = match config.preferred_route {
        EmbeddingRoutePreference::DeterministicHash => (
            EmbeddingRouteKind::DeterministicHash,
            "openlife".to_string(),
            None,
            DETERMINISTIC_HASH_MODEL_V1.to_string(),
            Some(DETERMINISTIC_HASH_DIMENSION_V1),
            Some(BUILTIN_DEPLOYMENT_IDENTITY.to_string()),
            Some(DETERMINISTIC_HASH_ARTIFACT_V1.to_string()),
            "configured_deterministic_hash".to_string(),
        ),
        EmbeddingRoutePreference::Ollama => (
            EmbeddingRouteKind::Ollama,
            "ollama".to_string(),
            None,
            local_embedding_model(&config.model),
            config.expected_dimension,
            None,
            config.model_artifact_identity,
            "configured_ollama".to_string(),
        ),
        EmbeddingRoutePreference::Cloud if !privacy_plan.cloud_allowed => (
            EmbeddingRouteKind::DeterministicHash,
            "openlife".to_string(),
            None,
            DETERMINISTIC_HASH_MODEL_V1.to_string(),
            Some(DETERMINISTIC_HASH_DIMENSION_V1),
            Some(BUILTIN_DEPLOYMENT_IDENTITY.to_string()),
            Some(DETERMINISTIC_HASH_ARTIFACT_V1.to_string()),
            "privacy_forced_local_hash".to_string(),
        ),
        EmbeddingRoutePreference::Cloud
            if !cloud_embedding_provider_supported(&config.provider) =>
        {
            (
                EmbeddingRouteKind::DeterministicHash,
                "openlife".to_string(),
                None,
                DETERMINISTIC_HASH_MODEL_V1.to_string(),
                Some(DETERMINISTIC_HASH_DIMENSION_V1),
                Some(BUILTIN_DEPLOYMENT_IDENTITY.to_string()),
                Some(DETERMINISTIC_HASH_ARTIFACT_V1.to_string()),
                "provider_embedding_unsupported_local_hash".to_string(),
            )
        }
        EmbeddingRoutePreference::Cloud if !config.cloud_credentials_available => (
            EmbeddingRouteKind::DeterministicHash,
            "openlife".to_string(),
            None,
            DETERMINISTIC_HASH_MODEL_V1.to_string(),
            Some(DETERMINISTIC_HASH_DIMENSION_V1),
            Some(BUILTIN_DEPLOYMENT_IDENTITY.to_string()),
            Some(DETERMINISTIC_HASH_ARTIFACT_V1.to_string()),
            "cloud_credentials_missing_local_hash".to_string(),
        ),
        EmbeddingRoutePreference::Cloud => {
            let provider = config.provider.trim().to_ascii_lowercase();
            let model = if config.model.trim().is_empty() {
                "text-embedding-3-small".to_string()
            } else {
                config.model.trim().to_string()
            };
            validate_profile_label("provider", &provider)?;
            validate_profile_label("model", &model)?;
            let endpoint = if config.endpoint.trim().is_empty() {
                crate::llm::default_base_for_provider(&provider).to_string()
            } else {
                config.endpoint.trim_end_matches('/').to_string()
            };
            let deployment_identity =
                deployment_identity_for_endpoint(&embedding_endpoint(&endpoint)).ok();
            let model_artifact_identity = config
                .model_artifact_identity
                .or_else(|| verified_remote_model_contract_identity(&provider, &endpoint, &model));
            (
                EmbeddingRouteKind::Cloud,
                provider,
                Some(endpoint),
                model.clone(),
                config
                    .expected_dimension
                    .or_else(|| known_embedding_dimension(&model)),
                deployment_identity,
                model_artifact_identity,
                "configured_cloud".to_string(),
            )
        }
    };

    if expected_dimension.is_some_and(|dimension| dimension > MAX_EMBEDDING_DIMENSION) {
        anyhow::bail!("embedding_expected_dimension_limit_exceeded");
    }

    let mut decision = EmbeddingRouteDecision {
        request_id,
        route,
        provider,
        endpoint,
        model,
        expected_dimension,
        deployment_identity,
        model_artifact_identity,
        reason_code,
        privacy_blocking_reasons: privacy_plan.blocking_reasons,
        network_policy_decision_id: None,
        credential_identity: None,
        credential_version: None,
    };
    let embedding_text = privacy_plan.embedding_text;
    let cloud_execution = if decision.route == EmbeddingRouteKind::Cloud {
        let authority =
            cloud_authority.ok_or_else(|| anyhow::anyhow!("cloud_embedding_authority_missing"))?;
        let base = decision
            .endpoint
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("cloud_embedding_endpoint_missing"))?;
        let endpoint = embedding_endpoint(base);
        let capability = format!("provider.{}.embedding", decision.provider);
        let network_policy_decision = crate::network_client::resolve_network_policy_decision(
            &authority.network_policy,
            &endpoint,
            &capability,
        )?;
        decision.network_policy_decision_id = Some(network_policy_decision.decision_id.clone());
        decision.credential_identity = Some(authority.credential_identity.clone());
        decision.credential_version = Some(authority.credential_version);
        let prepared_digest = cloud_prepared_digest(
            &decision,
            &embedding_text,
            &endpoint,
            &network_policy_decision,
        );
        Some(CloudEmbeddingExecutionBinding {
            endpoint,
            api_key: authority.api_key,
            credential_identity: authority.credential_identity,
            credential_version: authority.credential_version,
            network_policy: authority.network_policy,
            network_policy_decision,
            prepared_digest,
        })
    } else {
        None
    };
    Ok(PreparedEmbeddingRequest {
        decision,
        embedding_text,
        cloud_execution,
    })
}

fn cloud_prepared_digest(
    decision: &EmbeddingRouteDecision,
    embedding_text: &str,
    endpoint: &str,
    network_policy_decision: &NetworkPolicyDecision,
) -> String {
    let text_digest = digest(&SHA256, embedding_text.as_bytes());
    let material = serde_json::json!({
        "requestId": decision.request_id,
        "route": decision.route,
        "provider": decision.provider,
        "endpoint": decision.endpoint,
        "dispatchEndpoint": endpoint,
        "model": decision.model,
        "expectedDimension": decision.expected_dimension,
        "deploymentIdentity": decision.deployment_identity,
        "modelArtifactIdentity": decision.model_artifact_identity,
        "reasonCode": decision.reason_code,
        "privacyBlockingReasons": decision.privacy_blocking_reasons,
        "embeddingTextDigest": format!("sha256:{}", text_digest.as_ref().iter().map(|byte| format!("{byte:02x}")).collect::<String>()),
        "networkPolicyDecision": network_policy_decision,
        "credentialIdentity": decision.credential_identity,
        "credentialVersion": decision.credential_version,
    });
    let encoded = serde_json::to_vec(&material).expect("embedding prepared digest material");
    let prepared = digest(&SHA256, &encoded);
    format!(
        "sha256:{}",
        prepared
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn validate_cloud_prepared_request(
    request: &PreparedEmbeddingRequest,
) -> Result<&CloudEmbeddingExecutionBinding> {
    if request.decision.route != EmbeddingRouteKind::Cloud {
        anyhow::bail!("cloud_embedding_route_mismatch");
    }
    let binding = request
        .cloud_execution
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("cloud_embedding_execution_binding_missing"))?;
    let base = request
        .decision
        .endpoint
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("cloud_embedding_endpoint_missing"))?;
    let expected_endpoint = embedding_endpoint(base);
    let expected_capability = format!("provider.{}.embedding", request.decision.provider);
    if binding.endpoint != expected_endpoint
        || binding.api_key.trim().is_empty()
        || binding.credential_identity != crate::llm::provider_credential_identity(&binding.api_key)
        || request.decision.credential_identity.as_deref()
            != Some(binding.credential_identity.as_str())
        || request.decision.credential_version != Some(binding.credential_version)
        || request.decision.network_policy_decision_id.as_deref()
            != Some(binding.network_policy_decision.decision_id.as_str())
        || binding.network_policy_decision.capability != expected_capability
        || cloud_prepared_digest(
            &request.decision,
            &request.embedding_text,
            &binding.endpoint,
            &binding.network_policy_decision,
        ) != binding.prepared_digest
    {
        anyhow::bail!("cloud_embedding_prepared_binding_mismatch");
    }
    Ok(binding)
}

pub async fn execute_embedding(request: PreparedEmbeddingRequest) -> EmbeddingOutcome {
    let mut request = request;
    if request.decision.route == EmbeddingRouteKind::Ollama {
        let endpoint = match crate::ollama::configured_ollama_embedding_base_url() {
            Ok(endpoint) => endpoint,
            Err(error) => {
                return failed_outcome(
                    &request.decision,
                    EmbeddingProfile::unknown(),
                    EmbeddingInvocationSource::RouteInvalid,
                    None,
                    error.to_string(),
                )
            }
        };
        let dispatch_endpoint = format!("{}/api/embed", endpoint.trim_end_matches('/'));
        let deployment_identity = match deployment_identity_for_endpoint(&dispatch_endpoint) {
            Ok(identity) => identity,
            Err(error) => {
                return failed_outcome(
                    &request.decision,
                    EmbeddingProfile::unknown(),
                    EmbeddingInvocationSource::RouteInvalid,
                    None,
                    error.to_string(),
                )
            }
        };
        request.decision.endpoint = Some(endpoint);
        request.decision.deployment_identity = Some(deployment_identity);
        return execute_ollama_embedding(request).await;
    }
    if request.decision.route == EmbeddingRouteKind::Cloud {
        if let Err(error) = validate_cloud_prepared_request(&request) {
            return failed_outcome(
                &request.decision,
                planned_profile(&request.decision),
                EmbeddingInvocationSource::PreDispatchRejected,
                None,
                error.to_string(),
            );
        }
    }
    let cache_key = embedding_cache_key(&request);
    if let Some((embedding, profile, _)) = embedding_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&cache_key)
    {
        return EmbeddingOutcome {
            receipt: receipt(
                &request.decision,
                &profile,
                EmbeddingInvocationStatus::NotAttempted,
                EmbeddingInvocationSource::CacheHit,
                None,
                None,
                true,
            ),
            profile,
            result: Ok(embedding),
        };
    }

    match request.decision.route {
        EmbeddingRouteKind::Unknown => failed_outcome(
            &request.decision,
            EmbeddingProfile::unknown(),
            EmbeddingInvocationSource::PreDispatchRejected,
            None,
            "embedding_route_unknown".into(),
        ),
        EmbeddingRouteKind::DeterministicHash => {
            let embedding = crate::ollama::deterministic_hash_embed_v1(&request.embedding_text);
            let profile = EmbeddingProfile::new(
                EmbeddingRouteKind::DeterministicHash,
                "openlife",
                DETERMINISTIC_HASH_MODEL_V1,
                BUILTIN_DEPLOYMENT_IDENTITY,
                DETERMINISTIC_HASH_ARTIFACT_V1,
                embedding.len(),
            )
            .expect("fixed deterministic embedding profile");
            cache_success(cache_key, &embedding, &profile, false);
            EmbeddingOutcome {
                receipt: receipt(
                    &request.decision,
                    &profile,
                    EmbeddingInvocationStatus::NotAttempted,
                    EmbeddingInvocationSource::DeterministicHash,
                    None,
                    None,
                    false,
                ),
                profile,
                result: Ok(embedding),
            }
        }
        EmbeddingRouteKind::Ollama => unreachable!("Ollama executes after endpoint validation"),
        EmbeddingRouteKind::Cloud => execute_cloud_embedding(request, cache_key).await,
    }
}

async fn execute_ollama_embedding(mut request: PreparedEmbeddingRequest) -> EmbeddingOutcome {
    let endpoint = request
        .decision
        .endpoint
        .clone()
        .expect("validated Ollama embedding endpoint");
    let configured_artifact = request.decision.model_artifact_identity.clone();
    let requested_model = request.decision.model.clone();
    let mut provider_dispatches = Vec::with_capacity(3);
    let identity = crate::ollama::resolve_ollama_embedding_model_at_with_start_observer(
        &endpoint,
        &requested_model,
        || {
            provider_dispatches.push(EmbeddingProviderDispatch {
                kind: EmbeddingProviderDispatchKind::ModelManifest,
                started_at: chrono::Utc::now(),
            });
        },
    )
    .await;
    let identity = match identity {
        Ok(identity) => identity,
        Err(error) => {
            return failed_outcome_with_dispatches(
                &request.decision,
                planned_profile(&request.decision),
                if provider_dispatches.is_empty() {
                    EmbeddingInvocationSource::RouteInvalid
                } else {
                    EmbeddingInvocationSource::Ollama
                },
                provider_dispatches,
                error.to_string(),
            )
        }
    };
    if configured_artifact
        .as_deref()
        .is_some_and(|expected| expected != identity.digest)
    {
        return failed_outcome_with_dispatches(
            &request.decision,
            EmbeddingProfile::unknown(),
            EmbeddingInvocationSource::Ollama,
            provider_dispatches,
            "ollama_embedding_manifest_digest_mismatch".into(),
        );
    }
    request.decision.model = identity.model.clone();
    request.decision.model_artifact_identity = Some(identity.digest.clone());
    let cache_key = embedding_cache_key(&request);
    if let Some((embedding, profile, artifact_stability_verified)) = embedding_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&cache_key)
    {
        if artifact_stability_verified
            && profile.route == EmbeddingRouteKind::Ollama
            && profile.model.eq_ignore_ascii_case(&identity.model)
            && profile.model_artifact_identity == identity.digest
        {
            return EmbeddingOutcome {
                receipt: receipt_with_dispatches(
                    &request.decision,
                    &profile,
                    EmbeddingInvocationStatus::Completed,
                    EmbeddingInvocationSource::Ollama,
                    None,
                    true,
                    provider_dispatches,
                ),
                profile,
                result: Ok(embedding),
            };
        }
    }
    let resolved_model = request.decision.model.clone();
    let result = crate::ollama::ollama_embed_resolved_at_with_start_observer(
        &endpoint,
        &request.embedding_text,
        &resolved_model,
        || {
            provider_dispatches.push(EmbeddingProviderDispatch {
                kind: EmbeddingProviderDispatchKind::Embedding,
                started_at: chrono::Utc::now(),
            });
        },
    )
    .await;
    let embedding = match result {
        Ok(embedding) => embedding,
        Err(error) => {
            return failed_outcome_with_dispatches(
                &request.decision,
                EmbeddingProfile::unknown(),
                EmbeddingInvocationSource::Ollama,
                provider_dispatches,
                error.to_string(),
            )
        }
    };
    let post_identity = crate::ollama::resolve_ollama_embedding_model_at_with_start_observer(
        &endpoint,
        &requested_model,
        || {
            provider_dispatches.push(EmbeddingProviderDispatch {
                kind: EmbeddingProviderDispatchKind::ModelManifest,
                started_at: chrono::Utc::now(),
            });
        },
    )
    .await;
    let post_identity = match post_identity {
        Ok(identity) => identity,
        Err(error) => {
            return failed_outcome_with_dispatches(
                &request.decision,
                EmbeddingProfile::unknown(),
                EmbeddingInvocationSource::Ollama,
                provider_dispatches,
                format!("ollama_embedding_post_manifest_failed: {error}"),
            )
        }
    };
    if post_identity != identity {
        return failed_outcome_with_dispatches(
            &request.decision,
            EmbeddingProfile::unknown(),
            EmbeddingInvocationSource::Ollama,
            provider_dispatches,
            "ollama_embedding_artifact_changed_during_dispatch".into(),
        );
    }
    finish_provider_success_with_dispatches(
        request,
        cache_key,
        embedding,
        EmbeddingInvocationSource::Ollama,
        provider_dispatches,
        true,
    )
}

async fn execute_cloud_embedding(
    request: PreparedEmbeddingRequest,
    cache_key: String,
) -> EmbeddingOutcome {
    let binding = match validate_cloud_prepared_request(&request) {
        Ok(binding) => binding.clone(),
        Err(error) => {
            return failed_outcome(
                &request.decision,
                planned_profile(&request.decision),
                EmbeddingInvocationSource::PreDispatchRejected,
                None,
                error.to_string(),
            )
        }
    };
    let decision = &request.decision;
    let profile = planned_profile(decision);
    let client = match embedding_network_client(&binding.endpoint) {
        Ok(client) => client,
        Err(error) => {
            return failed_outcome(
                decision,
                profile,
                EmbeddingInvocationSource::PreDispatchRejected,
                None,
                error.to_string(),
            )
        }
    };
    let mut headers = HeaderMap::new();
    let authorization = match HeaderValue::from_str(&format!("Bearer {}", binding.api_key)) {
        Ok(value) => value,
        Err(error) => {
            return failed_outcome(
                decision,
                profile,
                EmbeddingInvocationSource::PreDispatchRejected,
                None,
                error.to_string(),
            )
        }
    };
    headers.insert(AUTHORIZATION, authorization);
    let body = serde_json::json!({
        "model": decision.model,
        "input": request.embedding_text,
    });
    let mut started_at = None;
    let response = client
        .post_json_text_with_decision_and_start_observer(
            &binding.endpoint,
            &binding.network_policy,
            &binding.network_policy_decision,
            headers,
            &body,
            |phase| {
                if phase == crate::network_client::NetworkDispatchAttemptPhase::Attempting {
                    started_at = Some(chrono::Utc::now());
                }
                std::future::ready(Ok::<(), anyhow::Error>(()))
            },
        )
        .await;
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return if let Some(started_at) = started_at {
                remote_unknown_outcome(
                    decision,
                    profile,
                    EmbeddingInvocationSource::CloudProvider,
                    started_at,
                    error.to_string(),
                )
            } else {
                failed_outcome(
                    decision,
                    profile,
                    EmbeddingInvocationSource::PreDispatchRejected,
                    None,
                    error.to_string(),
                )
            }
        }
    };
    if !response.status.is_success() {
        return failed_outcome(
            decision,
            profile,
            EmbeddingInvocationSource::CloudProvider,
            started_at,
            format!(
                "embedding_provider_http_status_{}",
                response.status.as_u16()
            ),
        );
    }
    let json: serde_json::Value =
        match serde_json::from_str(&response.body).context("embedding_response_json_invalid") {
            Ok(json) => json,
            Err(error) => {
                return failed_outcome(
                    decision,
                    profile,
                    EmbeddingInvocationSource::CloudProvider,
                    started_at,
                    error.to_string(),
                )
            }
        };
    if let Err(error) = validate_cloud_response_contract(decision, &json) {
        return failed_outcome(
            decision,
            profile,
            EmbeddingInvocationSource::CloudProvider,
            started_at,
            error.to_string(),
        );
    }
    let embedding = match parse_embedding_array(&json) {
        Ok(embedding) => embedding,
        Err(error) => {
            return failed_outcome(
                decision,
                profile,
                EmbeddingInvocationSource::CloudProvider,
                started_at,
                error.to_string(),
            )
        }
    };
    let Some(started_at) = started_at else {
        return failed_outcome(
            decision,
            profile,
            EmbeddingInvocationSource::RouteInvalid,
            None,
            "cloud_embedding_completed_without_dispatch".into(),
        );
    };
    finish_provider_success(
        request,
        cache_key,
        embedding,
        EmbeddingInvocationSource::CloudProvider,
        started_at,
    )
}

fn finish_provider_success(
    request: PreparedEmbeddingRequest,
    cache_key: String,
    embedding: Vec<f32>,
    source: EmbeddingInvocationSource,
    started_at: chrono::DateTime<chrono::Utc>,
) -> EmbeddingOutcome {
    finish_provider_success_with_dispatches(
        request,
        cache_key,
        embedding,
        source,
        vec![EmbeddingProviderDispatch {
            kind: EmbeddingProviderDispatchKind::Embedding,
            started_at,
        }],
        false,
    )
}

fn finish_provider_success_with_dispatches(
    request: PreparedEmbeddingRequest,
    cache_key: String,
    embedding: Vec<f32>,
    source: EmbeddingInvocationSource,
    provider_dispatches: Vec<EmbeddingProviderDispatch>,
    ollama_artifact_stability_verified: bool,
) -> EmbeddingOutcome {
    if let Err(error) = validate_embedding_vector(&embedding, request.decision.expected_dimension) {
        return failed_outcome_with_dispatches(
            &request.decision,
            planned_profile(&request.decision),
            source,
            provider_dispatches,
            error.to_string(),
        );
    }
    // A provider response can be real even when its mutable model route cannot
    // identify an immutable vector space. Preserve that execution fact, but do
    // not fabricate a compatibility key or cache it for later projection.
    let profile = profile_for_decision(&request.decision, embedding.len())
        .unwrap_or_else(|_| EmbeddingProfile::unknown());
    if profile.id != UNKNOWN_EMBEDDING_PROFILE_ID {
        cache_success(
            cache_key,
            &embedding,
            &profile,
            ollama_artifact_stability_verified,
        );
    }
    EmbeddingOutcome {
        receipt: receipt_with_dispatches(
            &request.decision,
            &profile,
            EmbeddingInvocationStatus::Completed,
            source,
            None,
            false,
            provider_dispatches,
        ),
        profile,
        result: Ok(embedding),
    }
}

fn failed_outcome(
    decision: &EmbeddingRouteDecision,
    profile: EmbeddingProfile,
    source: EmbeddingInvocationSource,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    error: String,
) -> EmbeddingOutcome {
    let provider_dispatches = started_at
        .map(|started_at| {
            vec![EmbeddingProviderDispatch {
                kind: EmbeddingProviderDispatchKind::Embedding,
                started_at,
            }]
        })
        .unwrap_or_default();
    failed_outcome_with_dispatches(decision, profile, source, provider_dispatches, error)
}

fn remote_unknown_outcome(
    decision: &EmbeddingRouteDecision,
    profile: EmbeddingProfile,
    source: EmbeddingInvocationSource,
    started_at: chrono::DateTime<chrono::Utc>,
    error: String,
) -> EmbeddingOutcome {
    let provider_dispatches = vec![EmbeddingProviderDispatch {
        kind: EmbeddingProviderDispatchKind::Embedding,
        started_at,
    }];
    EmbeddingOutcome {
        receipt: receipt_with_dispatches(
            decision,
            &profile,
            EmbeddingInvocationStatus::RemoteUnknown,
            source,
            Some(error_digest(&error)),
            false,
            provider_dispatches,
        ),
        profile,
        result: Err(error),
    }
}

fn failed_outcome_with_dispatches(
    decision: &EmbeddingRouteDecision,
    profile: EmbeddingProfile,
    source: EmbeddingInvocationSource,
    provider_dispatches: Vec<EmbeddingProviderDispatch>,
    error: String,
) -> EmbeddingOutcome {
    let attempted = !provider_dispatches.is_empty();
    EmbeddingOutcome {
        receipt: receipt_with_dispatches(
            decision,
            &profile,
            if attempted {
                EmbeddingInvocationStatus::Failed
            } else {
                EmbeddingInvocationStatus::NotAttempted
            },
            source,
            Some(error_digest(&error)),
            false,
            provider_dispatches,
        ),
        profile,
        result: Err(error),
    }
}

fn receipt(
    decision: &EmbeddingRouteDecision,
    profile: &EmbeddingProfile,
    status: EmbeddingInvocationStatus,
    source: EmbeddingInvocationSource,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    error_digest: Option<String>,
    cache_hit: bool,
) -> EmbeddingInvocationReceipt {
    let provider_dispatches = started_at
        .map(|started_at| {
            vec![EmbeddingProviderDispatch {
                kind: EmbeddingProviderDispatchKind::Embedding,
                started_at,
            }]
        })
        .unwrap_or_default();
    receipt_with_dispatches(
        decision,
        profile,
        status,
        source,
        error_digest,
        cache_hit,
        provider_dispatches,
    )
}

fn receipt_with_dispatches(
    decision: &EmbeddingRouteDecision,
    profile: &EmbeddingProfile,
    status: EmbeddingInvocationStatus,
    source: EmbeddingInvocationSource,
    error_digest: Option<String>,
    cache_hit: bool,
    provider_dispatches: Vec<EmbeddingProviderDispatch>,
) -> EmbeddingInvocationReceipt {
    let started_at = provider_dispatches
        .first()
        .map(|dispatch| dispatch.started_at);
    EmbeddingInvocationReceipt {
        request_id: decision.request_id.clone(),
        route: decision.route,
        provider: decision.provider.clone(),
        model: decision.model.clone(),
        deployment_identity: decision.deployment_identity.clone(),
        profile_id: profile.id.clone(),
        status,
        source,
        route_reason_code: decision.reason_code.clone(),
        started_at,
        finished_at: chrono::Utc::now(),
        error_digest,
        cache_hit,
        provider_dispatches,
        network_policy_decision_id: decision.network_policy_decision_id.clone(),
        credential_identity: decision.credential_identity.clone(),
        credential_version: decision.credential_version,
    }
}

pub(crate) fn validate_embedding_vector(
    embedding: &[f32],
    expected_dimension: Option<usize>,
) -> Result<()> {
    if embedding.is_empty() {
        anyhow::bail!("embedding_provider_vector_empty");
    }
    if embedding.len() > MAX_EMBEDDING_DIMENSION {
        anyhow::bail!("embedding_dimension_limit_exceeded");
    }
    if expected_dimension.is_some_and(|expected| expected != embedding.len()) {
        anyhow::bail!(
            "embedding_dimension_mismatch expected={} actual={}",
            expected_dimension.unwrap_or_default(),
            embedding.len()
        );
    }
    let mut norm_squared = 0.0_f64;
    for value in embedding {
        if !value.is_finite() {
            anyhow::bail!("embedding_provider_vector_non_finite");
        }
        let value = f64::from(*value);
        norm_squared += value * value;
    }
    if !norm_squared.is_finite() {
        anyhow::bail!("embedding_provider_vector_norm_overflow");
    }
    if norm_squared == 0.0 {
        anyhow::bail!("embedding_provider_vector_zero_norm");
    }
    Ok(())
}

fn cache_success(
    cache_key: String,
    embedding: &[f32],
    profile: &EmbeddingProfile,
    ollama_artifact_stability_verified: bool,
) {
    embedding_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .put(
            cache_key,
            embedding.to_vec(),
            profile.clone(),
            ollama_artifact_stability_verified,
        );
}

fn parse_embedding_array(json: &serde_json::Value) -> Result<Vec<f32>> {
    let values = json
        .get("data")
        .and_then(serde_json::Value::as_array)
        .and_then(|data| data.first())
        .and_then(|item| item.get("embedding"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("embedding_response_vector_missing"))?;
    if values.is_empty() {
        anyhow::bail!("embedding_response_vector_empty");
    }
    values
        .iter()
        .map(|value| {
            let value = value
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("embedding_response_value_invalid"))?;
            if !value.is_finite() || value > f32::MAX as f64 || value < f32::MIN as f64 {
                anyhow::bail!("embedding_response_value_invalid");
            }
            Ok(value as f32)
        })
        .collect()
}

fn validate_cloud_response_contract(
    decision: &EmbeddingRouteDecision,
    json: &serde_json::Value,
) -> Result<()> {
    let observed_model = json
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty());
    if decision.model_artifact_identity.is_some() && observed_model.is_none() {
        anyhow::bail!("embedding_response_model_missing");
    }
    if observed_model.is_some_and(|model| !cloud_response_model_matches(&decision.model, model)) {
        anyhow::bail!("embedding_response_model_mismatch");
    }
    Ok(())
}

fn cloud_response_model_matches(requested: &str, observed: &str) -> bool {
    requested
        .trim()
        .trim_start_matches("openai/")
        .eq_ignore_ascii_case(observed.trim().trim_start_matches("openai/"))
}

fn embedding_network_client(url: &str) -> Result<NetworkClient> {
    let parsed = reqwest::Url::parse(url).context("embedding_endpoint_invalid")?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("embedding_endpoint_host_missing"))?;
    let explicitly_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    Ok(NetworkClient::new(NetworkClientPolicy {
        require_https: !explicitly_loopback,
        allow_loopback: explicitly_loopback,
        max_redirects: 0,
        max_body_bytes: MAX_EMBEDDING_RESPONSE_BYTES,
        connect_timeout: Duration::from_secs(10),
        request_timeout: Duration::from_secs(30),
        ..Default::default()
    }))
}

fn planned_profile(decision: &EmbeddingRouteDecision) -> EmbeddingProfile {
    let dimension = decision.expected_dimension.unwrap_or_default();
    profile_for_decision(decision, dimension).unwrap_or_else(|_| EmbeddingProfile::unknown())
}

fn profile_for_decision(
    decision: &EmbeddingRouteDecision,
    dimension: usize,
) -> Result<EmbeddingProfile> {
    let deployment_identity = decision
        .deployment_identity
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("embedding_deployment_identity_unknown"))?;
    let model_artifact_identity = decision
        .model_artifact_identity
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("embedding_model_artifact_identity_unknown"))?;
    EmbeddingProfile::new(
        decision.route,
        decision.provider.clone(),
        decision.model.clone(),
        deployment_identity,
        model_artifact_identity,
        dimension,
    )
}

fn profile_id(
    route: EmbeddingRouteKind,
    provider: &str,
    model: &str,
    deployment_identity: &str,
    model_artifact_identity: &str,
    dimension: usize,
) -> String {
    let route_label = match route {
        EmbeddingRouteKind::Unknown => "unknown",
        EmbeddingRouteKind::Cloud => "cloud",
        EmbeddingRouteKind::Ollama => "ollama",
        EmbeddingRouteKind::DeterministicHash => "hash",
    };
    let canonical = format!(
        "{route_label}\0{provider}\0{model}\0{deployment_identity}\0{model_artifact_identity}\0{dimension}"
    );
    let identity = digest(&SHA256, canonical.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("embedding:{route_label}:sha256:{identity}:dim:{dimension}")
}

fn embedding_cache_key(request: &PreparedEmbeddingRequest) -> String {
    let text_digest = digest(&SHA256, request.embedding_text.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(
        "route={:?}|provider={}|endpoint={}|model={}|dimension={:?}|deployment={}|artifact={}|text_sha256={}",
        request.decision.route,
        request.decision.provider,
        request.decision.endpoint.as_deref().unwrap_or("none"),
        request.decision.model,
        request.decision.expected_dimension,
        request
            .decision
            .deployment_identity
            .as_deref()
            .unwrap_or("unknown"),
        request
            .decision
            .model_artifact_identity
            .as_deref()
            .unwrap_or("unknown"),
        text_digest
    )
}

fn known_embedding_dimension(model: &str) -> Option<usize> {
    match model.trim().trim_start_matches("openai/") {
        "text-embedding-3-small" | "text-embedding-ada-002" => Some(1_536),
        "text-embedding-3-large" => Some(3_072),
        _ => None,
    }
}

fn cloud_embedding_provider_supported(provider: &str) -> bool {
    matches!(
        provider.trim().to_ascii_lowercase().as_str(),
        "openai" | "openrouter" | "custom" | "openai-compatible"
    )
}

fn local_embedding_model(configured: &str) -> String {
    let configured = configured.trim();
    if configured.is_empty() || configured.starts_with("text-embedding-") {
        "nomic-embed-text".into()
    } else {
        configured.into()
    }
}

fn verified_remote_model_contract_identity(
    provider: &str,
    endpoint: &str,
    model: &str,
) -> Option<String> {
    if !provider.trim().eq_ignore_ascii_case("openai") || known_embedding_dimension(model).is_none()
    {
        return None;
    }
    let parsed = reqwest::Url::parse(endpoint).ok()?;
    if parsed.scheme() != "https"
        || !parsed
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case("api.openai.com"))
    {
        return None;
    }
    let model = model.trim();
    Some(format!("openai-response-model-contract:v1:{model}"))
}

fn deployment_identity_for_endpoint(endpoint: &str) -> Result<String> {
    let mut parsed = reqwest::Url::parse(endpoint).context("embedding_endpoint_invalid")?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        anyhow::bail!("embedding_endpoint_identity_invalid");
    }
    let normalized_path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(if normalized_path.is_empty() {
        "/"
    } else {
        &normalized_path
    });
    let canonical = parsed.to_string();
    Ok(format!("endpoint:{}", error_digest(&canonical)))
}

fn validate_profile_label(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.chars().count() > 128
        || value.chars().any(|character| character.is_control())
    {
        anyhow::bail!("embedding_profile_{field}_invalid");
    }
    Ok(())
}

fn validate_profile_identity(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.chars().count() > 256
        || value.chars().any(|character| character.is_control())
    {
        anyhow::bail!("embedding_profile_{field}_invalid");
    }
    Ok(())
}

fn embedding_endpoint(base: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.ends_with("/embeddings") {
        base.to_string()
    } else {
        format!("{base}/embeddings")
    }
}

fn error_digest(error: &str) -> String {
    let hash = digest(&SHA256, error.as_bytes());
    let mut value = String::with_capacity(hash.as_ref().len() * 2 + 7);
    value.push_str("sha256:");
    for byte in hash.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(value, "{byte:02x}");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectors::EmbeddingPrivacyPlan;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    static EMBEDDING_CACHE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn privacy_plan(text: &str, cloud_allowed: bool) -> EmbeddingPrivacyPlan {
        EmbeddingPrivacyPlan {
            embedding_text: text.to_string(),
            cloud_allowed,
            hs_local_only: !cloud_allowed,
            detected_privacy_types: Vec::new(),
            sensitive_topic: None,
            blocking_reasons: if !cloud_allowed {
                vec!["test_local_only".to_string()]
            } else {
                Default::default()
            },
        }
    }

    fn route_config(
        preference: EmbeddingRoutePreference,
        endpoint: String,
        model: &str,
        credentials_available: bool,
    ) -> EmbeddingRouteConfig {
        let provider = match preference {
            EmbeddingRoutePreference::Ollama => "ollama".to_string(),
            _ => "openai".to_string(),
        };
        let network_policy = NetworkPolicy {
            default_decision: "allow".into(),
            ..NetworkPolicy::default()
        };
        let cloud_authority = credentials_available.then(|| test_cloud_authority(network_policy));
        EmbeddingRouteConfig {
            preferred_route: preference,
            provider,
            endpoint,
            model: model.into(),
            expected_dimension: None,
            model_artifact_identity: (preference != EmbeddingRoutePreference::Ollama)
                .then(|| "test-artifact-v1".into()),
            cloud_credentials_available: credentials_available,
            cloud_authority,
        }
    }

    fn test_cloud_authority(network_policy: NetworkPolicy) -> CloudEmbeddingAuthority {
        CloudEmbeddingAuthority {
            api_key: "test-key".into(),
            credential_identity: crate::llm::provider_credential_identity("test-key"),
            credential_version: 7,
            network_policy,
        }
    }

    fn allow_network_policy() -> NetworkPolicy {
        NetworkPolicy {
            default_decision: "allow".into(),
            ..NetworkPolicy::default()
        }
    }

    async fn one_response_server(
        status_line: &'static str,
        body: &'static str,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let call_count = Arc::new(AtomicUsize::new(0));
        let server_count = call_count.clone();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            server_count.fetch_add(1, Ordering::SeqCst);
            let mut request = vec![0_u8; 16 * 1024];
            let read = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..read]).into_owned();
            let response = format!(
                "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            request
        });
        (format!("http://{address}"), call_count, server)
    }

    async fn response_sequence_server(
        responses: Vec<(&'static str, String)>,
    ) -> (
        String,
        Arc<AtomicUsize>,
        tokio::task::JoinHandle<Vec<String>>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let call_count = Arc::new(AtomicUsize::new(0));
        let server_count = call_count.clone();
        let server = tokio::spawn(async move {
            let mut requests = Vec::with_capacity(responses.len());
            for (status_line, body) in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                server_count.fetch_add(1, Ordering::SeqCst);
                let mut request = vec![0_u8; 16 * 1024];
                let read = socket.read(&mut request).await.unwrap();
                requests.push(String::from_utf8_lossy(&request[..read]).into_owned());
                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        (format!("http://{address}"), call_count, server)
    }

    fn ollama_embedding_manifest(model: &str, digest: &str) -> String {
        serde_json::json!({
            "models": [{
                "name": model,
                "model": model,
                "digest": digest,
                "size": 1234
            }]
        })
        .to_string()
    }

    fn ollama_embedding_response(model: &str) -> String {
        serde_json::json!({
            "model": model,
            "embeddings": [[0.4, 0.5, 0.6, 0.7]]
        })
        .to_string()
    }

    #[test]
    fn missing_cloud_credentials_selects_hash_before_dispatch() {
        let prepared = prepare_embedding_request(
            "credential-free",
            EmbeddingRouteConfig::from_product_config(
                "openai",
                "https://api.openai.com/v1",
                "text-embedding-3-small",
                true,
                "",
                0,
                NetworkPolicy {
                    default_decision: "allow".into(),
                    ..NetworkPolicy::default()
                },
            ),
            privacy_plan("credential-free", true),
        )
        .unwrap();

        assert_eq!(
            prepared.decision.route,
            EmbeddingRouteKind::DeterministicHash
        );
        assert_eq!(
            prepared.decision.reason_code,
            "cloud_credentials_missing_local_hash"
        );
        assert!(prepared.decision.endpoint.is_none());
    }

    #[test]
    fn privacy_blocked_cloud_selects_hash_before_dispatch() {
        let prepared = prepare_embedding_request(
            "local-only",
            route_config(
                EmbeddingRoutePreference::Cloud,
                "https://api.openai.com/v1".into(),
                "text-embedding-3-small",
                true,
            ),
            privacy_plan("local-only", false),
        )
        .unwrap();

        assert_eq!(
            prepared.decision.route,
            EmbeddingRouteKind::DeterministicHash
        );
        assert_eq!(prepared.decision.reason_code, "privacy_forced_local_hash");
        assert_eq!(
            prepared.decision.privacy_blocking_reasons,
            ["test_local_only"]
        );
    }

    #[test]
    fn unsupported_chat_provider_selects_hash_instead_of_inventing_endpoint() {
        let prepared = prepare_embedding_request(
            "unsupported-provider",
            EmbeddingRouteConfig {
                preferred_route: EmbeddingRoutePreference::Cloud,
                provider: "deepseek".into(),
                endpoint: "https://api.deepseek.com".into(),
                model: "text-embedding-3-small".into(),
                expected_dimension: None,
                model_artifact_identity: Some("unsupported-test-artifact-v1".into()),
                cloud_credentials_available: true,
                cloud_authority: Some(test_cloud_authority(allow_network_policy())),
            },
            privacy_plan("unsupported-provider", true),
        )
        .unwrap();

        assert_eq!(
            prepared.decision.route,
            EmbeddingRouteKind::DeterministicHash
        );
        assert_eq!(
            prepared.decision.reason_code,
            "provider_embedding_unsupported_local_hash"
        );
        assert!(prepared.decision.endpoint.is_none());
    }

    #[test]
    fn legacy_unknown_profile_does_not_claim_hash_route() {
        let profile = EmbeddingProfile::unknown();
        assert_eq!(profile.id, UNKNOWN_EMBEDDING_PROFILE_ID);
        assert_eq!(profile.route, EmbeddingRouteKind::Unknown);
        assert_eq!(profile.dimension, 0);
    }

    #[test]
    fn explicit_ollama_route_accepts_tagged_model() {
        let prepared = prepare_embedding_request(
            "ollama-tag",
            route_config(
                EmbeddingRoutePreference::Ollama,
                String::new(),
                "nomic-embed-text:latest",
                false,
            ),
            privacy_plan("ollama-tag", false),
        )
        .unwrap();
        let profile = EmbeddingProfile::new(
            EmbeddingRouteKind::Ollama,
            "ollama",
            "nomic-embed-text:latest",
            "endpoint:sha256:test",
            "sha256:test-artifact",
            768,
        )
        .unwrap();

        assert_eq!(prepared.decision.route, EmbeddingRouteKind::Ollama);
        assert_eq!(prepared.decision.model, "nomic-embed-text:latest");
        assert!(profile.id.contains("embedding:ollama:sha256:"));
    }

    #[tokio::test]
    async fn deterministic_hash_and_cache_receipts_preserve_profile() {
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let first = execute_embedding(
            prepare_embedding_request(
                "stable-cache-profile",
                route_config(
                    EmbeddingRoutePreference::DeterministicHash,
                    String::new(),
                    "ignored",
                    false,
                ),
                privacy_plan("stable-cache-profile", true),
            )
            .unwrap(),
        )
        .await;
        let second = execute_embedding(
            prepare_embedding_request(
                "stable-cache-profile",
                route_config(
                    EmbeddingRoutePreference::DeterministicHash,
                    String::new(),
                    "ignored",
                    false,
                ),
                privacy_plan("stable-cache-profile", true),
            )
            .unwrap(),
        )
        .await;

        assert!(first.result.is_ok());
        assert_eq!(
            first.receipt.status,
            EmbeddingInvocationStatus::NotAttempted
        );
        assert_eq!(
            first.receipt.source,
            EmbeddingInvocationSource::DeterministicHash
        );
        assert_eq!(first.profile.dimension, DETERMINISTIC_HASH_DIMENSION_V1);
        assert!(second.result.is_ok());
        assert_eq!(second.profile, first.profile);
        assert_eq!(
            second.receipt.status,
            EmbeddingInvocationStatus::NotAttempted
        );
        assert_eq!(second.receipt.source, EmbeddingInvocationSource::CacheHit);
        assert!(second.receipt.cache_hit);
    }

    #[tokio::test]
    async fn cloud_503_fails_without_switching_profile() {
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let (base, call_count, server) =
            one_response_server("503 Service Unavailable", r#"{"error":"offline"}"#).await;
        let mut config = route_config(
            EmbeddingRoutePreference::Cloud,
            format!("{base}/v1"),
            "custom-test-embedding",
            true,
        );
        config.expected_dimension = Some(3);
        let prepared = prepare_embedding_request(
            "cloud-503-no-fallback",
            config,
            privacy_plan("cloud-503-no-fallback", true),
        )
        .unwrap();
        let outcome = execute_embedding(prepared).await;
        let request = server.await.unwrap();

        assert!(request.starts_with("POST /v1/embeddings "));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert!(outcome.result.is_err());
        assert_eq!(outcome.profile.route, EmbeddingRouteKind::Cloud);
        assert_eq!(outcome.receipt.status, EmbeddingInvocationStatus::Failed);
        assert_eq!(
            outcome.receipt.source,
            EmbeddingInvocationSource::CloudProvider
        );
    }

    #[tokio::test]
    async fn cloud_disconnect_after_dispatch_is_remote_unknown() {
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            // Close after observing the request without producing a remote
            // terminal. The client cannot truthfully call this failed_remote.
        });
        let prepared = prepare_embedding_request(
            "cloud-disconnect",
            route_config(
                EmbeddingRoutePreference::Cloud,
                format!("{base}/v1"),
                "custom-test-embedding",
                true,
            ),
            privacy_plan("cloud-disconnect", true),
        )
        .unwrap();

        let outcome = execute_embedding(prepared).await;
        server.await.unwrap();
        assert!(outcome.result.is_err());
        assert_eq!(
            outcome.receipt.status,
            EmbeddingInvocationStatus::RemoteUnknown
        );
        assert!(outcome.receipt.started_at.is_some());
    }

    #[tokio::test]
    async fn cloud_success_uses_full_embeddings_endpoint_once() {
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let (base, call_count, server) = one_response_server(
            "200 OK",
            r#"{"model":"custom-test-embedding","data":[{"embedding":[0.1,0.2,0.3]}]}"#,
        )
        .await;
        let prepared = prepare_embedding_request(
            "full-endpoint-once",
            route_config(
                EmbeddingRoutePreference::Cloud,
                format!("{base}/v1/embeddings"),
                "custom-test-embedding",
                true,
            ),
            privacy_plan("full-endpoint-once", true),
        )
        .unwrap();
        let outcome = execute_embedding(prepared).await;
        let request = server.await.unwrap();

        assert!(request.starts_with("POST /v1/embeddings "));
        assert!(!request.contains("/embeddings/embeddings"));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert_eq!(outcome.result.unwrap().len(), 3);
        assert_eq!(outcome.profile.dimension, 3);
        assert_eq!(outcome.receipt.status, EmbeddingInvocationStatus::Completed);
        assert!(outcome.receipt.network_policy_decision_id.is_some());
        let expected_credential_identity = crate::llm::provider_credential_identity("test-key");
        assert_eq!(
            outcome.receipt.credential_identity.as_deref(),
            Some(expected_credential_identity.as_str())
        );
        assert_eq!(outcome.receipt.credential_version, Some(7));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer test-key"));
    }

    #[tokio::test]
    async fn cloud_embedding_requires_exact_typed_capability_not_network_generic() {
        let mut policy = allow_network_policy();
        policy
            .tool_overrides
            .insert("network.generic".into(), "allow".into());
        policy
            .tool_overrides
            .insert("provider.openai.embedding".into(), "deny".into());
        let config = EmbeddingRouteConfig {
            preferred_route: EmbeddingRoutePreference::Cloud,
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            model: "text-embedding-3-small".into(),
            expected_dimension: Some(3),
            model_artifact_identity: Some("openai-contract-v1".into()),
            cloud_credentials_available: true,
            cloud_authority: Some(test_cloud_authority(policy)),
        };
        let prepared = prepare_embedding_request(
            "typed-capability-denied",
            config,
            privacy_plan("typed-capability-denied", true),
        )
        .unwrap();
        assert_eq!(
            prepared
                .cloud_execution
                .as_ref()
                .unwrap()
                .network_policy_decision
                .capability,
            "provider.openai.embedding"
        );

        let outcome = execute_embedding(prepared).await;
        assert!(outcome.result.is_err());
        assert_eq!(
            outcome.receipt.status,
            EmbeddingInvocationStatus::NotAttempted
        );
        assert!(outcome.receipt.provider_dispatches.is_empty());
    }

    #[tokio::test]
    async fn cloud_embedding_payload_and_credential_mutation_fail_before_dispatch() {
        let prepared = prepare_embedding_request(
            "sealed-cloud-input",
            route_config(
                EmbeddingRoutePreference::Cloud,
                "https://api.openai.com/v1".into(),
                "text-embedding-3-small",
                true,
            ),
            privacy_plan("sealed-cloud-input", true),
        )
        .unwrap();

        let mut payload_tampered = prepared.clone();
        payload_tampered.embedding_text = "different-input".into();
        let payload_outcome = execute_embedding(payload_tampered).await;
        assert!(payload_outcome.result.is_err());
        assert_eq!(
            payload_outcome.receipt.status,
            EmbeddingInvocationStatus::NotAttempted
        );

        let mut credential_tampered = prepared;
        credential_tampered
            .cloud_execution
            .as_mut()
            .unwrap()
            .api_key = "test-key-replaced".into();
        let credential_outcome = execute_embedding(credential_tampered).await;
        assert!(credential_outcome.result.is_err());
        assert_eq!(
            credential_outcome.receipt.status,
            EmbeddingInvocationStatus::NotAttempted
        );
        assert!(credential_outcome.receipt.provider_dispatches.is_empty());
    }

    // The process-global environment lock intentionally spans dispatch so no
    // concurrently running test can observe a temporary Ollama endpoint.
    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test]
    async fn ollama_observes_manifest_digest_and_uses_current_embed_contract() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let digest = format!("sha256:{}", "a".repeat(64));
        let tags = ollama_embedding_manifest("nomic-embed-text:latest", &digest);
        let embed = ollama_embedding_response("nomic-embed-text:latest");
        let (base, call_count, server) = response_sequence_server(vec![
            ("200 OK", tags.clone()),
            ("200 OK", embed),
            ("200 OK", tags),
        ])
        .await;
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", &base);
        let prepared = prepare_embedding_request(
            "explicit-ollama-success",
            route_config(
                EmbeddingRoutePreference::Ollama,
                String::new(),
                "nomic-embed-text:latest",
                false,
            ),
            privacy_plan("explicit-ollama-success", false),
        )
        .unwrap();
        let outcome = execute_embedding(prepared).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        let requests = server.await.unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 3);
        assert!(requests[0].starts_with("GET /api/tags "));
        assert!(requests[1].starts_with("POST /api/embed "));
        assert!(requests[2].starts_with("GET /api/tags "));
        assert!(requests[1].contains("\"input\":\"explicit-ollama-success\""));
        assert!(!requests[1].contains("\"prompt\""));
        assert_eq!(outcome.result.unwrap().len(), 4);
        assert_eq!(outcome.profile.route, EmbeddingRouteKind::Ollama);
        assert_eq!(outcome.profile.model_artifact_identity, digest);
        assert_eq!(outcome.receipt.status, EmbeddingInvocationStatus::Completed);
        assert_eq!(outcome.receipt.source, EmbeddingInvocationSource::Ollama);
        assert_eq!(
            outcome
                .receipt
                .provider_dispatches
                .iter()
                .map(|dispatch| dispatch.kind)
                .collect::<Vec<_>>(),
            [
                EmbeddingProviderDispatchKind::ModelManifest,
                EmbeddingProviderDispatchKind::Embedding,
                EmbeddingProviderDispatchKind::ModelManifest,
            ]
        );
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test]
    async fn ollama_tag_change_during_embed_fails_unknown_without_caching() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let pre_digest = format!("sha256:{}", "a".repeat(64));
        let post_digest = format!("sha256:{}", "b".repeat(64));
        let (base, call_count, server) = response_sequence_server(vec![
            (
                "200 OK",
                ollama_embedding_manifest("nomic-embed-text:latest", &pre_digest),
            ),
            (
                "200 OK",
                ollama_embedding_response("nomic-embed-text:latest"),
            ),
            (
                "200 OK",
                ollama_embedding_manifest("nomic-embed-text:latest", &post_digest),
            ),
        ])
        .await;
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", &base);
        let prepared = prepare_embedding_request(
            "ollama-changing-artifact",
            route_config(
                EmbeddingRoutePreference::Ollama,
                String::new(),
                "nomic-embed-text:latest",
                false,
            ),
            privacy_plan("ollama-changing-artifact", false),
        )
        .unwrap();

        let outcome = execute_embedding(prepared).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        let requests = server.await.unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 3);
        assert_eq!(requests.len(), 3);
        assert!(outcome.result.is_err());
        assert_eq!(outcome.profile, EmbeddingProfile::unknown());
        assert_eq!(outcome.receipt.status, EmbeddingInvocationStatus::Failed);
        assert!(!outcome.receipt.cache_hit);
        assert_eq!(
            outcome
                .receipt
                .provider_dispatches
                .iter()
                .map(|dispatch| dispatch.kind)
                .collect::<Vec<_>>(),
            [
                EmbeddingProviderDispatchKind::ModelManifest,
                EmbeddingProviderDispatchKind::Embedding,
                EmbeddingProviderDispatchKind::ModelManifest,
            ]
        );
        assert!(embedding_cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .is_empty());
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test]
    async fn ollama_post_manifest_failure_fails_unknown_without_caching() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let digest = format!("sha256:{}", "c".repeat(64));
        let (base, call_count, server) = response_sequence_server(vec![
            (
                "200 OK",
                ollama_embedding_manifest("nomic-embed-text:latest", &digest),
            ),
            (
                "200 OK",
                ollama_embedding_response("nomic-embed-text:latest"),
            ),
            ("503 Service Unavailable", r#"{"error":"offline"}"#.into()),
        ])
        .await;
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", &base);
        let prepared = prepare_embedding_request(
            "ollama-post-manifest-failure",
            route_config(
                EmbeddingRoutePreference::Ollama,
                String::new(),
                "nomic-embed-text:latest",
                false,
            ),
            privacy_plan("ollama-post-manifest-failure", false),
        )
        .unwrap();

        let outcome = execute_embedding(prepared).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        let _ = server.await.unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 3);
        assert!(outcome.result.is_err());
        assert_eq!(outcome.profile, EmbeddingProfile::unknown());
        assert_eq!(outcome.receipt.status, EmbeddingInvocationStatus::Failed);
        assert_eq!(outcome.receipt.provider_dispatches.len(), 3);
        assert!(embedding_cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .is_empty());
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test]
    async fn ollama_verified_cache_hit_dispatches_only_current_manifest() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let digest = format!("sha256:{}", "d".repeat(64));
        let tags = ollama_embedding_manifest("nomic-embed-text:latest", &digest);
        let (base, call_count, server) = response_sequence_server(vec![
            ("200 OK", tags.clone()),
            (
                "200 OK",
                ollama_embedding_response("nomic-embed-text:latest"),
            ),
            ("200 OK", tags.clone()),
            ("200 OK", tags),
        ])
        .await;
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", &base);
        let config = route_config(
            EmbeddingRoutePreference::Ollama,
            String::new(),
            "nomic-embed-text:latest",
            false,
        );
        let first = execute_embedding(
            prepare_embedding_request(
                "ollama-verified-cache",
                config.clone(),
                privacy_plan("ollama-verified-cache", false),
            )
            .unwrap(),
        )
        .await;
        let second = execute_embedding(
            prepare_embedding_request(
                "ollama-verified-cache",
                config,
                privacy_plan("ollama-verified-cache", false),
            )
            .unwrap(),
        )
        .await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        let requests = server.await.unwrap();

        assert!(first.result.is_ok());
        assert!(second.result.is_ok());
        assert_eq!(first.profile, second.profile);
        assert_eq!(call_count.load(Ordering::SeqCst), 4);
        assert_eq!(requests.len(), 4);
        assert!(second.receipt.cache_hit);
        assert_eq!(second.receipt.status, EmbeddingInvocationStatus::Completed);
        assert_eq!(second.receipt.source, EmbeddingInvocationSource::Ollama);
        assert_eq!(
            second
                .receipt
                .provider_dispatches
                .iter()
                .map(|dispatch| dispatch.kind)
                .collect::<Vec<_>>(),
            [EmbeddingProviderDispatchKind::ModelManifest]
        );
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test]
    async fn ollama_does_not_use_cache_entry_without_sandwich_proof() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let model = "nomic-embed-text:latest";
        let digest = format!("sha256:{}", "e".repeat(64));
        let tags = ollama_embedding_manifest(model, &digest);
        let (base, call_count, server) = response_sequence_server(vec![
            ("200 OK", tags.clone()),
            ("200 OK", ollama_embedding_response(model)),
            ("200 OK", tags),
        ])
        .await;
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", &base);
        let mut prepared = prepare_embedding_request(
            "ollama-unverified-cache",
            route_config(
                EmbeddingRoutePreference::Ollama,
                String::new(),
                model,
                false,
            ),
            privacy_plan("ollama-unverified-cache", false),
        )
        .unwrap();
        let dispatch_endpoint = format!("{}/api/embed", base.trim_end_matches('/'));
        prepared.decision.endpoint = Some(base.clone());
        prepared.decision.deployment_identity =
            Some(deployment_identity_for_endpoint(&dispatch_endpoint).unwrap());
        prepared.decision.model_artifact_identity = Some(digest.clone());
        let unverified_profile = profile_for_decision(&prepared.decision, 4).unwrap();
        cache_success(
            embedding_cache_key(&prepared),
            &[0.4, 0.5, 0.6, 0.7],
            &unverified_profile,
            false,
        );

        let outcome = execute_embedding(prepared).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        let _ = server.await.unwrap();

        assert!(outcome.result.is_ok());
        assert!(!outcome.receipt.cache_hit);
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
        assert_eq!(outcome.receipt.provider_dispatches.len(), 3);
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test]
    async fn ollama_manifest_failure_records_only_the_manifest_dispatch() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let (base, call_count, server) =
            one_response_server("503 Service Unavailable", r#"{"error":"offline"}"#).await;
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", &base);
        let prepared = prepare_embedding_request(
            "ollama-manifest-failure",
            route_config(
                EmbeddingRoutePreference::Ollama,
                String::new(),
                "nomic-embed-text:latest",
                false,
            ),
            privacy_plan("ollama-manifest-failure", false),
        )
        .unwrap();
        let outcome = execute_embedding(prepared).await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        let request = server.await.unwrap();

        assert!(request.starts_with("GET /api/tags "));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert!(outcome.result.is_err());
        assert_eq!(outcome.receipt.status, EmbeddingInvocationStatus::Failed);
        assert_eq!(
            outcome
                .receipt
                .provider_dispatches
                .iter()
                .map(|dispatch| dispatch.kind)
                .collect::<Vec<_>>(),
            [EmbeddingProviderDispatchKind::ModelManifest]
        );
    }

    #[tokio::test]
    async fn custom_provider_without_explicit_revision_stays_unknown_and_is_not_cached() {
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let body = serde_json::json!({
            "model": "custom-test-embedding",
            "data": [{"embedding": [0.1, 0.2, 0.3]}]
        })
        .to_string();
        let (base, call_count, server) =
            response_sequence_server(vec![("200 OK", body.clone()), ("200 OK", body)]).await;
        let config = EmbeddingRouteConfig {
            preferred_route: EmbeddingRoutePreference::Cloud,
            provider: "custom".into(),
            endpoint: format!("{base}/v1"),
            model: "custom-test-embedding".into(),
            expected_dimension: Some(3),
            model_artifact_identity: None,
            cloud_credentials_available: true,
            cloud_authority: Some(test_cloud_authority(allow_network_policy())),
        };
        let first = execute_embedding(
            prepare_embedding_request(
                "custom-unknown-profile",
                config.clone(),
                privacy_plan("custom-unknown-profile", true),
            )
            .unwrap(),
        )
        .await;
        let second = execute_embedding(
            prepare_embedding_request(
                "custom-unknown-profile",
                config,
                privacy_plan("custom-unknown-profile", true),
            )
            .unwrap(),
        )
        .await;
        let _ = server.await.unwrap();

        assert_eq!(call_count.load(Ordering::SeqCst), 2);
        assert!(first.result.is_ok());
        assert!(second.result.is_ok());
        assert_eq!(first.profile, EmbeddingProfile::unknown());
        assert_eq!(second.profile, EmbeddingProfile::unknown());
        assert!(!first.receipt.cache_hit);
        assert!(!second.receipt.cache_hit);
    }

    #[tokio::test]
    async fn openai_contract_rejects_response_model_mismatch_after_dispatch() {
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        clear_embedding_cache();
        let (base, _, server) = one_response_server(
            "200 OK",
            r#"{"model":"text-embedding-3-large","data":[{"embedding":[0.1,0.2,0.3]}]}"#,
        )
        .await;
        let prepared = prepare_embedding_request(
            "model-contract-mismatch",
            EmbeddingRouteConfig {
                preferred_route: EmbeddingRoutePreference::Cloud,
                provider: "openai".into(),
                endpoint: format!("{base}/v1"),
                model: "text-embedding-3-small".into(),
                expected_dimension: Some(3),
                model_artifact_identity: None,
                cloud_credentials_available: true,
                cloud_authority: Some(test_cloud_authority(allow_network_policy())),
            },
            privacy_plan("model-contract-mismatch", true),
        )
        .unwrap();
        let outcome = execute_embedding(prepared).await;
        let _ = server.await.unwrap();

        assert!(outcome.result.is_err());
        assert_eq!(outcome.receipt.status, EmbeddingInvocationStatus::Failed);
        assert!(outcome.receipt.started_at.is_some());
    }

    #[test]
    fn deployment_endpoint_is_part_of_profile_compatibility_identity() {
        let build = |request_id: &str, endpoint: &str| PreparedEmbeddingRequest {
            decision: EmbeddingRouteDecision {
                request_id: request_id.into(),
                route: EmbeddingRouteKind::Cloud,
                provider: "custom".into(),
                endpoint: Some(endpoint.into()),
                model: "immutable-model-revision-a".into(),
                expected_dimension: Some(3),
                deployment_identity: deployment_identity_for_endpoint(endpoint).ok(),
                model_artifact_identity: Some("revision-a".into()),
                reason_code: "test_identity".into(),
                privacy_blocking_reasons: Vec::new(),
                network_policy_decision_id: None,
                credential_identity: None,
                credential_version: None,
            },
            embedding_text: format!("identity-{request_id}"),
            cloud_execution: None,
        };
        let first = finish_provider_success(
            build("endpoint-a", "https://embedding-a.example/v1"),
            "endpoint-a-cache".into(),
            vec![0.1, 0.2, 0.3],
            EmbeddingInvocationSource::CloudProvider,
            chrono::Utc::now(),
        );
        let second = finish_provider_success(
            build("endpoint-b", "https://embedding-b.example/v1"),
            "endpoint-b-cache".into(),
            vec![0.1, 0.2, 0.3],
            EmbeddingInvocationSource::CloudProvider,
            chrono::Utc::now(),
        );

        assert_ne!(
            first.profile.id, second.profile.id,
            "different deployments must never share a vector compatibility profile"
        );
    }

    #[test]
    fn equivalent_endpoint_spelling_has_one_non_raw_deployment_identity() {
        let first =
            deployment_identity_for_endpoint("https://EMBEDDING.example:443/v1/embeddings/")
                .unwrap();
        let second =
            deployment_identity_for_endpoint("https://embedding.example/v1/embeddings").unwrap();

        assert_eq!(first, second);
        assert!(first.starts_with("endpoint:sha256:"));
        assert!(!first.contains("embedding.example"));
    }

    #[test]
    fn mutable_local_tag_without_artifact_revision_is_unknown() {
        let outcome = finish_provider_success(
            PreparedEmbeddingRequest {
                decision: EmbeddingRouteDecision {
                    request_id: "mutable-local-tag".into(),
                    route: EmbeddingRouteKind::Ollama,
                    provider: "ollama".into(),
                    endpoint: Some("http://127.0.0.1:11434".into()),
                    model: "nomic-embed-text:latest".into(),
                    expected_dimension: Some(4),
                    deployment_identity: deployment_identity_for_endpoint(
                        "http://127.0.0.1:11434/api/embed",
                    )
                    .ok(),
                    model_artifact_identity: None,
                    reason_code: "configured_ollama".into(),
                    privacy_blocking_reasons: Vec::new(),
                    network_policy_decision_id: None,
                    credential_identity: None,
                    credential_version: None,
                },
                embedding_text: "mutable-local-profile".into(),
                cloud_execution: None,
            },
            "mutable-local-cache".into(),
            vec![0.1, 0.2, 0.3, 0.4],
            EmbeddingInvocationSource::Ollama,
            chrono::Utc::now(),
        );

        assert!(outcome.result.is_ok());
        assert_eq!(outcome.profile.id, UNKNOWN_EMBEDDING_PROFILE_ID);
        assert_eq!(outcome.profile.route, EmbeddingRouteKind::Unknown);
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test]
    async fn invalid_ollama_endpoint_has_zero_dispatch_receipt() {
        let _env_guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let _cache_guard = EMBEDDING_CACHE_TEST_LOCK.lock().await;
        for (case, endpoint) in [
            ("remote", "https://remote-model.example:11434"),
            ("empty", "   "),
        ] {
            clear_embedding_cache();
            std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", endpoint);
            let text = format!("invalid-ollama-route-{case}");
            let prepared = prepare_embedding_request(
                &text,
                route_config(
                    EmbeddingRoutePreference::Ollama,
                    String::new(),
                    "nomic-embed-text:latest",
                    false,
                ),
                privacy_plan(&text, false),
            )
            .unwrap();
            let outcome = execute_embedding(prepared).await;

            assert!(outcome.result.is_err(), "case={case}");
            assert_eq!(
                outcome.receipt.status,
                EmbeddingInvocationStatus::NotAttempted,
                "case={case}"
            );
            assert_eq!(
                outcome.receipt.source,
                EmbeddingInvocationSource::RouteInvalid,
                "case={case}"
            );
            assert!(outcome.receipt.started_at.is_none(), "case={case}");
        }
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
    }

    #[test]
    fn model_artifact_revision_is_part_of_profile_compatibility_identity() {
        let first = EmbeddingProfile::new(
            EmbeddingRouteKind::Cloud,
            "custom",
            "embedding-model",
            "endpoint:sha256:deployment",
            "revision-a",
            3,
        )
        .unwrap();
        let second = EmbeddingProfile::new(
            EmbeddingRouteKind::Cloud,
            "custom",
            "embedding-model",
            "endpoint:sha256:deployment",
            "revision-b",
            3,
        )
        .unwrap();

        assert_ne!(first.id, second.id);
    }

    #[test]
    fn automatic_cloud_contract_is_limited_to_the_official_constrained_endpoint() {
        assert!(verified_remote_model_contract_identity(
            "openai",
            "https://api.openai.com/v1",
            "text-embedding-3-small"
        )
        .is_some());
        assert!(verified_remote_model_contract_identity(
            "openai",
            "http://127.0.0.1:8080/v1",
            "text-embedding-3-small"
        )
        .is_none());
        assert!(verified_remote_model_contract_identity(
            "custom",
            "https://embedding.example/v1",
            "text-embedding-3-small"
        )
        .is_none());
    }

    #[test]
    fn provider_dimension_mismatch_is_failed_after_real_dispatch_fact() {
        let outcome = finish_provider_success(
            PreparedEmbeddingRequest {
                decision: EmbeddingRouteDecision {
                    request_id: "dimension-mismatch".into(),
                    route: EmbeddingRouteKind::Cloud,
                    provider: "custom".into(),
                    endpoint: Some("https://embedding.example/v1".into()),
                    model: "model-a".into(),
                    expected_dimension: Some(4),
                    deployment_identity: deployment_identity_for_endpoint(
                        "https://embedding.example/v1/embeddings",
                    )
                    .ok(),
                    model_artifact_identity: Some("revision-a".into()),
                    reason_code: "configured_cloud".into(),
                    privacy_blocking_reasons: Vec::new(),
                    network_policy_decision_id: None,
                    credential_identity: None,
                    credential_version: None,
                },
                embedding_text: "dimension mismatch".into(),
                cloud_execution: None,
            },
            "dimension-mismatch-cache".into(),
            vec![0.1, 0.2, 0.3],
            EmbeddingInvocationSource::CloudProvider,
            chrono::Utc::now(),
        );

        assert!(outcome.result.is_err());
        assert_eq!(outcome.receipt.status, EmbeddingInvocationStatus::Failed);
        assert!(outcome.receipt.started_at.is_some());
    }

    #[test]
    fn zero_norm_and_oversized_provider_vectors_are_rejected() {
        let build = |request_id: &str, expected_dimension: usize| PreparedEmbeddingRequest {
            decision: EmbeddingRouteDecision {
                request_id: request_id.into(),
                route: EmbeddingRouteKind::Cloud,
                provider: "custom".into(),
                endpoint: Some("https://embedding.example/v1".into()),
                model: "model-a".into(),
                expected_dimension: Some(expected_dimension),
                deployment_identity: deployment_identity_for_endpoint(
                    "https://embedding.example/v1/embeddings",
                )
                .ok(),
                model_artifact_identity: Some("revision-a".into()),
                reason_code: "configured_cloud".into(),
                privacy_blocking_reasons: Vec::new(),
                network_policy_decision_id: None,
                credential_identity: None,
                credential_version: None,
            },
            embedding_text: request_id.into(),
            cloud_execution: None,
        };
        let zero = finish_provider_success(
            build("zero-norm", 3),
            "zero-norm-cache".into(),
            vec![0.0, 0.0, 0.0],
            EmbeddingInvocationSource::CloudProvider,
            chrono::Utc::now(),
        );
        let oversized_dimension = MAX_EMBEDDING_DIMENSION + 1;
        let oversized = finish_provider_success(
            build("oversized", oversized_dimension),
            "oversized-cache".into(),
            vec![1.0; oversized_dimension],
            EmbeddingInvocationSource::CloudProvider,
            chrono::Utc::now(),
        );

        assert!(zero.result.is_err());
        assert!(oversized.result.is_err());
    }

    #[test]
    fn prepare_rejection_has_a_not_attempted_receipt() {
        let outcome = prepare_embedding_request_recorded(
            "",
            route_config(
                EmbeddingRoutePreference::Cloud,
                "https://api.openai.com/v1".into(),
                "text-embedding-3-small",
                true,
            ),
            privacy_plan("", true),
        );
        let PreparedEmbeddingRequestOutcome::Rejected(outcome) = outcome else {
            panic!("empty input must be rejected before dispatch");
        };

        assert_eq!(
            outcome.receipt.status,
            EmbeddingInvocationStatus::NotAttempted
        );
        assert!(outcome.receipt.started_at.is_none());
        assert!(outcome.receipt.provider_dispatches.is_empty());
        assert!(outcome.result.is_err());
    }

    #[test]
    fn empty_embedding_text_is_rejected_before_hashing() {
        let result = prepare_embedding_request(
            "",
            route_config(
                EmbeddingRoutePreference::DeterministicHash,
                String::new(),
                "ignored",
                false,
            ),
            privacy_plan("", true),
        );
        assert!(result.is_err());
    }

    #[test]
    fn cloud_adapter_uses_governed_bounded_network_client() {
        let source = include_str!("embedding.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(production.contains("NetworkClient"));
        assert!(production.contains("MAX_EMBEDDING_RESPONSE_BYTES"));
        assert!(production.contains("provider.{}.embedding"));
        assert!(production.contains("post_json_text_with_decision_and_start_observer"));
        assert!(!production.contains("Option<&NetworkPolicy>"));
        assert!(!production.contains("post_json_text_with_start_observer"));
        assert!(!production.contains("reqwest::Client"));
        assert!(!production.contains(".json::<serde_json::Value>().await"));
    }
}
