use crate::config::NetworkPolicy;
use anyhow::{Context, Result};
use async_stream::try_stream;
use futures::{Stream, StreamExt};
use reqwest::header::{HeaderMap, CONTENT_TYPE, LOCATION};
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_DNS_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_MAX_REDIRECTS: usize = 5;
const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_IDEMPOTENT_ATTEMPTS: usize = 2;

#[derive(Debug, Clone)]
pub struct NetworkClientPolicy {
    pub require_https: bool,
    /// Allows an explicitly configured loopback endpoint while continuing to
    /// reject private, link-local and reserved networks. This must never be
    /// enabled for a hostname merely because DNS happened to return loopback.
    pub allow_loopback: bool,
    /// Exact/domain-suffix rules whose HTTPS destinations may use the macOS
    /// user-configured loopback HTTP proxy when every local DNS answer is in
    /// RFC 2544 fake-IP space. Adapters must bind this to their fixed endpoint
    /// or to the user's explicit NetworkPolicy domain allowlist; an empty list
    /// keeps generic/caller-selected hosts fail closed.
    pub fake_ip_proxy_domain_allowlist: Vec<String>,
    pub max_redirects: usize,
    pub max_body_bytes: usize,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub dns_timeout: Duration,
}

impl Default for NetworkClientPolicy {
    fn default() -> Self {
        Self {
            require_https: false,
            allow_loopback: false,
            fake_ip_proxy_domain_allowlist: Vec::new(),
            max_redirects: DEFAULT_MAX_REDIRECTS,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            dns_timeout: DEFAULT_DNS_TIMEOUT,
        }
    }
}

#[derive(Debug)]
enum NetworkEgressRoute {
    DirectPinned(Vec<SocketAddr>),
    LoopbackSystemProxy(Url),
}

#[derive(Debug, Clone)]
pub struct NetworkTextResponse {
    pub status: StatusCode,
    pub final_url: Url,
    pub body: String,
}

pub type NetworkByteStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>;

pub struct NetworkStreamResponse {
    pub status: StatusCode,
    pub final_url: Url,
    pub body: NetworkByteStream,
}

/// What the HTTP adapter can prove for one reqwest attempt. Entering
/// `.send()` proves only an attempt; a concrete dispatch edge is credited once
/// response headers are observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkDispatchAttemptPhase {
    Attempting,
    ResponseHeadersObserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicyDisposition {
    Allow,
    Deny,
    Ask,
}

impl NetworkPolicyDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Ask => "ask",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicyDecision {
    pub decision_id: String,
    pub disposition: NetworkPolicyDisposition,
    pub reason_code: String,
    pub capability: String,
    pub host: String,
    /// Metadata-safe digest of the exact normalized URL, including path and
    /// query.  Host-level policy matching remains unchanged, but an authority
    /// resolved for one endpoint cannot be replayed against another path.
    #[serde(default)]
    pub endpoint_digest: String,
}

/// One in-process authority to prepare the fixed Settings provider probe.
///
/// The fields are intentionally private and this type is neither `Clone` nor
/// serde-capable. Only the canonical ToolPermissionStore owns the issuer;
/// schedulers retain verifiers only. Naked policy labels or caller-provided
/// consent strings therefore cannot mint a grant accepted by a scheduler.
#[derive(Debug)]
pub struct ExplicitProviderProbeGrant {
    issuer_id: String,
    authorization_tag: [u8; 32],
    network_policy: NetworkPolicy,
    network_policy_decision: NetworkPolicyDecision,
    provider_target: String,
    model_target: String,
    endpoint: String,
    provider_config_generation: String,
    credential_version: u64,
    credential_identity: String,
    consent_reference: String,
}

/// Non-authorizing description of one immutable scheduler generation. It is
/// safe to hand to the canonical permission store because it carries no secret
/// and cannot itself prepare or execute a provider request.
#[derive(Debug)]
pub struct ExplicitProviderProbeChallenge {
    provider_target: String,
    model_target: String,
    endpoint: String,
    provider_config_generation: String,
    credential_version: u64,
    credential_identity: String,
}

/// Reusable in-process issuer owned by the canonical ToolPermissionStore. The
/// target scheduler retains only the paired verifier. Product code cannot
/// obtain an issuer from a scheduler or turn serialized policy labels into an
/// accepted grant.
#[derive(Clone)]
pub(crate) struct ExplicitProviderProbeIssuer {
    issuer_id: String,
    secret: Arc<[u8; 32]>,
}

#[derive(Clone)]
pub struct ExplicitProviderProbeVerifier {
    issuer_id: String,
    secret: Arc<[u8; 32]>,
}

impl ExplicitProviderProbeChallenge {
    pub(crate) fn new(
        provider_target: impl Into<String>,
        model_target: impl Into<String>,
        endpoint: impl Into<String>,
        provider_config_generation: impl Into<String>,
        credential_version: u64,
        credential_identity: impl Into<String>,
    ) -> Self {
        Self {
            provider_target: provider_target.into(),
            model_target: model_target.into(),
            endpoint: endpoint.into(),
            provider_config_generation: provider_config_generation.into(),
            credential_version,
            credential_identity: credential_identity.into(),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn provider_target(&self) -> &str {
        &self.provider_target
    }
}

impl ExplicitProviderProbeGrant {
    pub(crate) fn network_policy_decision(&self) -> &NetworkPolicyDecision {
        &self.network_policy_decision
    }

    pub(crate) fn provider_target(&self) -> &str {
        &self.provider_target
    }

    pub(crate) fn model_target(&self) -> &str {
        &self.model_target
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn provider_config_generation(&self) -> &str {
        &self.provider_config_generation
    }

    pub(crate) fn credential_version(&self) -> u64 {
        self.credential_version
    }

    pub(crate) fn credential_identity(&self) -> &str {
        &self.credential_identity
    }

    pub(crate) fn consent_reference(&self) -> &str {
        &self.consent_reference
    }

    pub(crate) fn into_network_authority(self) -> (NetworkPolicy, NetworkPolicyDecision) {
        (self.network_policy, self.network_policy_decision)
    }
}

impl ExplicitProviderProbeIssuer {
    pub(crate) fn issue_governed_probe_grant(
        &self,
        challenge: ExplicitProviderProbeChallenge,
        network_policy: NetworkPolicy,
        network_policy_decision: NetworkPolicyDecision,
        consent_reference: String,
    ) -> Result<ExplicitProviderProbeGrant> {
        let consent_reference = consent_reference.trim().to_string();
        if consent_reference.is_empty() {
            anyhow::bail!("explicit_provider_probe_consent_reference_missing");
        }
        if challenge.provider_target.is_empty()
            || challenge.provider_target == "ollama"
            || challenge.model_target.is_empty()
            || challenge.provider_config_generation.is_empty()
            || challenge.credential_identity.is_empty()
        {
            anyhow::bail!("explicit_provider_probe_grant_scope_incomplete");
        }
        let parsed =
            Url::parse(&challenge.endpoint).context("explicit_provider_probe_endpoint_invalid")?;
        if parsed.host_str().is_none() || parsed.path().is_empty() || parsed.path() == "/" {
            anyhow::bail!("explicit_provider_probe_final_endpoint_missing");
        }
        let expected_capability = format!("provider.{}", challenge.provider_target);
        if network_policy_decision.capability != expected_capability {
            anyhow::bail!("explicit_provider_probe_capability_mismatch");
        }
        let observed = resolve_network_policy_decision(
            &network_policy,
            &challenge.endpoint,
            &expected_capability,
        )?;
        if observed != network_policy_decision
            || observed.disposition != NetworkPolicyDisposition::Allow
        {
            anyhow::bail!("explicit_provider_probe_network_authority_mismatch");
        }
        let mut grant = ExplicitProviderProbeGrant {
            issuer_id: self.issuer_id.clone(),
            authorization_tag: [0; 32],
            network_policy,
            network_policy_decision,
            provider_target: challenge.provider_target,
            model_target: challenge.model_target,
            endpoint: challenge.endpoint,
            provider_config_generation: challenge.provider_config_generation,
            credential_version: challenge.credential_version,
            credential_identity: challenge.credential_identity,
            consent_reference,
        };
        grant.authorization_tag = explicit_provider_probe_grant_tag(&self.secret, &grant)?;
        Ok(grant)
    }
}

impl ExplicitProviderProbeVerifier {
    pub(crate) fn verify(&self, grant: &ExplicitProviderProbeGrant) -> Result<()> {
        if grant.issuer_id != self.issuer_id {
            anyhow::bail!("explicit_provider_probe_issuer_mismatch");
        }
        let material = explicit_provider_probe_grant_material(grant)?;
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, self.secret.as_ref());
        ring::hmac::verify(&key, &material, &grant.authorization_tag)
            .map_err(|_| anyhow::anyhow!("explicit_provider_probe_grant_authentication_failed"))
    }
}

impl NetworkPolicyDecision {
    pub fn local_only(capability: impl Into<String>) -> Self {
        let capability = capability.into();
        Self {
            decision_id: format!("network-policy:local-only:{capability}"),
            disposition: NetworkPolicyDisposition::Allow,
            reason_code: "network_local_transport".into(),
            capability,
            host: "local".into(),
            endpoint_digest: network_endpoint_digest("local://ollama"),
        }
    }
}

fn network_endpoint_digest(endpoint: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, endpoint.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{digest}")
}

pub fn resolve_network_policy_decision(
    policy: &NetworkPolicy,
    url: &str,
    capability: &str,
) -> Result<NetworkPolicyDecision> {
    let parsed = Url::parse(url).context("network_invalid_url")?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("network_url_host_missing"))?
        .to_ascii_lowercase();
    let capability = capability.trim();
    if capability.is_empty() {
        anyhow::bail!("network_policy_capability_missing");
    }

    let denylisted = policy
        .domain_denylist
        .iter()
        .any(|rule| domain_matches(&host, rule));
    let allowlisted = !policy.domain_allowlist.is_empty()
        && policy
            .domain_allowlist
            .iter()
            .any(|rule| domain_matches(&host, rule));
    let capability_override = policy
        .tool_overrides
        .get(capability)
        .or_else(|| policy.tool_overrides.get("provider"))
        .map(|value| value.trim().to_ascii_lowercase());
    let default_decision = policy.default_decision.trim().to_ascii_lowercase();

    let (disposition, reason_code) = if !policy.enabled {
        (NetworkPolicyDisposition::Deny, "network_policy_disabled")
    } else if denylisted {
        (NetworkPolicyDisposition::Deny, "network_domain_denied")
    } else if !policy.domain_allowlist.is_empty() && !allowlisted {
        (
            NetworkPolicyDisposition::Deny,
            "network_domain_not_allowlisted",
        )
    } else if let Some(override_decision) = capability_override.as_deref() {
        match override_decision {
            "allow" => (
                NetworkPolicyDisposition::Allow,
                "network_policy_override_allow",
            ),
            "ask" => (
                NetworkPolicyDisposition::Ask,
                "network_policy_consent_required",
            ),
            "deny" => (
                NetworkPolicyDisposition::Deny,
                "network_policy_override_deny",
            ),
            _ => (
                NetworkPolicyDisposition::Deny,
                "network_policy_override_invalid",
            ),
        }
    } else if allowlisted {
        (
            NetworkPolicyDisposition::Allow,
            "network_domain_allowlisted",
        )
    } else {
        match default_decision.as_str() {
            "allow" => (
                NetworkPolicyDisposition::Allow,
                "network_policy_default_allow",
            ),
            "ask" => (
                NetworkPolicyDisposition::Ask,
                "network_policy_consent_required",
            ),
            "deny" => (
                NetworkPolicyDisposition::Deny,
                "network_policy_default_deny",
            ),
            _ => (
                NetworkPolicyDisposition::Deny,
                "network_policy_default_invalid",
            ),
        }
    };
    let canonical_domain_rules = |rules: &[String]| {
        let mut rules = rules
            .iter()
            .map(|rule| {
                rule.trim()
                    .trim_start_matches("*.")
                    .trim_matches('.')
                    .to_ascii_lowercase()
            })
            .filter(|rule| !rule.is_empty())
            .collect::<Vec<_>>();
        rules.sort();
        rules.dedup();
        rules
    };
    // HashMap iteration order and domain-rule order are not policy semantics.
    // Hash only the normalized inputs that can affect this capability's
    // decision so semantically identical policies produce one stable ID.
    let digest_input = serde_json::to_vec(&serde_json::json!({
        "policy": {
            "enabled": policy.enabled,
            "defaultDecision": default_decision,
            "domainAllowlist": canonical_domain_rules(&policy.domain_allowlist),
            "domainDenylist": canonical_domain_rules(&policy.domain_denylist),
            "capabilityOverride": capability_override,
        },
        "scheme": parsed.scheme(),
        "host": host,
        "port": parsed.port_or_known_default(),
        "path": parsed.path(),
        "query": parsed.query(),
        "capability": capability,
        "disposition": disposition.as_str(),
        "reasonCode": reason_code,
    }))
    .context("network_policy_decision_serialize_failed")?;
    let digest = ring::digest::digest(&ring::digest::SHA256, &digest_input)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(NetworkPolicyDecision {
        decision_id: format!("network-policy:sha256:{digest}"),
        disposition,
        reason_code: reason_code.into(),
        capability: capability.to_string(),
        host,
        endpoint_digest: network_endpoint_digest(parsed.as_str()),
    })
}

/// Create an in-process issuer/verifier pair owned by the canonical permission
/// store. Neither half is serializable; the issuer never crosses that store's
/// module boundary.
pub(crate) fn create_explicit_provider_probe_authority(
) -> (ExplicitProviderProbeVerifier, ExplicitProviderProbeIssuer) {
    let issuer_id = uuid::Uuid::new_v4().to_string();
    let first = uuid::Uuid::new_v4();
    let second = uuid::Uuid::new_v4();
    let mut secret = [0_u8; 32];
    secret[..16].copy_from_slice(first.as_bytes());
    secret[16..].copy_from_slice(second.as_bytes());
    let secret = Arc::new(secret);
    let verifier = ExplicitProviderProbeVerifier {
        issuer_id: issuer_id.clone(),
        secret: Arc::clone(&secret),
    };
    let issuer = ExplicitProviderProbeIssuer { issuer_id, secret };
    (verifier, issuer)
}

fn explicit_provider_probe_grant_tag(
    secret: &[u8; 32],
    grant: &ExplicitProviderProbeGrant,
) -> Result<[u8; 32]> {
    let material = explicit_provider_probe_grant_material(grant)?;
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret);
    let signed = ring::hmac::sign(&key, &material);
    let mut tag = [0_u8; 32];
    tag.copy_from_slice(signed.as_ref());
    Ok(tag)
}

fn explicit_provider_probe_grant_material(grant: &ExplicitProviderProbeGrant) -> Result<Vec<u8>> {
    serde_json::to_vec(&serde_json::json!({
        "authorityVersion": "explicit_provider_probe_issuer_v1",
        "issuerId": &grant.issuer_id,
        "networkPolicy": &grant.network_policy,
        "networkPolicyDecision": &grant.network_policy_decision,
        "providerTarget": &grant.provider_target,
        "modelTarget": &grant.model_target,
        "endpoint": &grant.endpoint,
        "providerConfigGeneration": &grant.provider_config_generation,
        "credentialVersion": grant.credential_version,
        "credentialIdentity": &grant.credential_identity,
        "consentReference": &grant.consent_reference,
    }))
    .context("explicit_provider_probe_grant_material_serialize_failed")
}

fn verify_network_policy_decision(
    url: &str,
    policy: &NetworkPolicy,
    expected: &NetworkPolicyDecision,
) -> Result<()> {
    let observed = resolve_network_policy_decision(policy, url, &expected.capability)?;
    if &observed != expected {
        anyhow::bail!("network_policy_decision_mismatch");
    }
    match observed.disposition {
        NetworkPolicyDisposition::Allow => Ok(()),
        NetworkPolicyDisposition::Ask => anyhow::bail!(
            "network_policy_consent_required:decision_id={}",
            observed.decision_id
        ),
        NetworkPolicyDisposition::Deny => anyhow::bail!(
            "{}:decision_id={}",
            observed.reason_code,
            observed.decision_id
        ),
    }
}

#[derive(Debug, Clone)]
pub struct NetworkClient {
    policy: NetworkClientPolicy,
}

impl NetworkClient {
    pub fn new(policy: NetworkClientPolicy) -> Self {
        Self { policy }
    }

    pub async fn get_text(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
    ) -> Result<NetworkTextResponse> {
        self.get_text_with_start_observer(url, network_policy, |_| async { Ok(()) })
            .await
    }

    /// Send an idempotent GET and report both the ambiguous `.send()` attempt
    /// and the later concrete response-header edge. Callers may count both, but
    /// must emit `tool.started` only for `ResponseHeadersObserved`.
    pub async fn get_text_with_start_observer<F, Fut>(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
        on_started: F,
    ) -> Result<NetworkTextResponse>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        self.get_text_with_headers_and_start_observer(
            url,
            network_policy,
            HeaderMap::new(),
            on_started,
        )
        .await
    }

    pub async fn get_text_with_headers(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
        headers: HeaderMap,
    ) -> Result<NetworkTextResponse> {
        self.get_text_with_headers_and_start_observer(url, network_policy, headers, |_| async {
            Ok(())
        })
        .await
    }

    pub async fn get_text_with_headers_and_start_observer<F, Fut>(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
        headers: HeaderMap,
        on_started: F,
    ) -> Result<NetworkTextResponse>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        self.get_text_with_headers_for_capability_and_start_observer(
            url,
            network_policy,
            "network.generic",
            headers,
            on_started,
        )
        .await
    }

    async fn get_text_with_headers_for_capability(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
        capability: &str,
        headers: HeaderMap,
    ) -> Result<NetworkTextResponse> {
        self.get_text_with_headers_for_capability_and_start_observer(
            url,
            network_policy,
            capability,
            headers,
            |_| async { Ok(()) },
        )
        .await
    }

    pub(crate) async fn get_text_with_headers_for_capability_and_start_observer<F, Fut>(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
        capability: &str,
        headers: HeaderMap,
        mut on_started: F,
    ) -> Result<NetworkTextResponse>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let mut current = Url::parse(url).context("network_invalid_url")?;
        let mut current_headers = headers;
        for redirect_index in 0..=self.policy.max_redirects {
            validate_url_policy(
                &current,
                network_policy,
                capability,
                self.policy.require_https,
            )?;
            let response = self
                .send_pinned_get(&current, current_headers.clone(), &mut on_started)
                .await?;

            if response.status().is_redirection() {
                if redirect_index == self.policy.max_redirects {
                    anyhow::bail!("network_redirect_limit_exceeded");
                }
                let location = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| anyhow::anyhow!("network_redirect_location_missing"))?;
                let next = current
                    .join(location)
                    .context("network_redirect_location_invalid")?;
                if !same_origin(&current, &next) {
                    // Caller-provided headers can contain provider keys, cookies,
                    // bearer tokens or product-specific secrets. A cross-origin
                    // redirect is still allowed only after the next-hop policy
                    // check, but it never inherits those headers.
                    current_headers.clear();
                }
                current = next;
                continue;
            }

            return self.read_text_response(current, response).await;
        }
        anyhow::bail!("network_redirect_limit_exceeded")
    }

    pub async fn get_text_with_headers_for_decision(
        &self,
        url: &str,
        network_policy: &NetworkPolicy,
        network_policy_decision: &NetworkPolicyDecision,
        headers: HeaderMap,
    ) -> Result<NetworkTextResponse> {
        verify_network_policy_decision(url, network_policy, network_policy_decision)?;
        self.get_text_with_headers_for_capability(
            url,
            Some(network_policy),
            &network_policy_decision.capability,
            headers,
        )
        .await
    }

    /// Send a non-idempotent JSON request exactly once. Redirects are rejected
    /// because replaying a POST could duplicate an external effect.
    pub async fn post_json_text(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
        headers: HeaderMap,
        body: &Value,
    ) -> Result<NetworkTextResponse> {
        self.post_json_text_with_start_observer(url, network_policy, headers, body, |_| async {
            Ok(())
        })
        .await
    }

    /// Send one non-idempotent JSON request and distinguish entering `.send()`
    /// from observing response headers. The callback is not called for
    /// validation, DNS, credential/header or request-size rejection.
    pub async fn post_json_text_with_start_observer<F, Fut>(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
        headers: HeaderMap,
        body: &Value,
        on_started: F,
    ) -> Result<NetworkTextResponse>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        self.post_json_text_for_capability_with_start_observer(
            url,
            network_policy,
            "network.generic",
            headers,
            body,
            on_started,
        )
        .await
    }

    pub(crate) async fn post_json_text_for_capability_with_start_observer<F, Fut>(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
        capability: &str,
        mut headers: HeaderMap,
        body: &Value,
        mut on_started: F,
    ) -> Result<NetworkTextResponse>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let parsed = Url::parse(url).context("network_invalid_url")?;
        validate_url_policy(
            &parsed,
            network_policy,
            capability,
            self.policy.require_https,
        )?;
        let serialized = serde_json::to_vec(body).context("network_json_serialize_failed")?;
        if serialized.len() > self.policy.max_body_bytes {
            anyhow::bail!("network_request_body_too_large");
        }
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        let response = self
            .send_pinned_post_once(&parsed, headers, serialized, &mut on_started)
            .await?;
        if response.status().is_redirection() {
            anyhow::bail!("network_non_idempotent_redirect_blocked");
        }
        self.read_text_response(parsed, response).await
    }

    pub async fn post_json_text_with_decision_and_start_observer<F, Fut>(
        &self,
        url: &str,
        network_policy: &NetworkPolicy,
        network_policy_decision: &NetworkPolicyDecision,
        headers: HeaderMap,
        body: &Value,
        on_started: F,
    ) -> Result<NetworkTextResponse>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        verify_network_policy_decision(url, network_policy, network_policy_decision)?;
        self.post_json_text_for_capability_with_start_observer(
            url,
            Some(network_policy),
            &network_policy_decision.capability,
            headers,
            body,
            on_started,
        )
        .await
    }

    /// Streaming counterpart to `post_json_text_with_start_observer`. The
    /// returned stream applies the same total byte cap and idle timeout as the
    /// buffered response path, so consumers cannot accidentally reintroduce an
    /// unbounded `reqwest::Response` reader.
    pub async fn post_json_stream_with_start_observer<F, Fut>(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
        headers: HeaderMap,
        body: &Value,
        on_started: F,
    ) -> Result<NetworkStreamResponse>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        self.post_json_stream_for_capability_with_start_observer(
            url,
            network_policy,
            "network.generic",
            headers,
            body,
            on_started,
        )
        .await
    }

    async fn post_json_stream_for_capability_with_start_observer<F, Fut>(
        &self,
        url: &str,
        network_policy: Option<&NetworkPolicy>,
        capability: &str,
        mut headers: HeaderMap,
        body: &Value,
        mut on_started: F,
    ) -> Result<NetworkStreamResponse>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let parsed = Url::parse(url).context("network_invalid_url")?;
        validate_url_policy(
            &parsed,
            network_policy,
            capability,
            self.policy.require_https,
        )?;
        let serialized = serde_json::to_vec(body).context("network_json_serialize_failed")?;
        if serialized.len() > self.policy.max_body_bytes {
            anyhow::bail!("network_request_body_too_large");
        }
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        let response = self
            .send_pinned_post_once(&parsed, headers, serialized, &mut on_started)
            .await?;
        if response.status().is_redirection() {
            anyhow::bail!("network_non_idempotent_redirect_blocked");
        }
        if let Some(length) = response.content_length() {
            if length > self.policy.max_body_bytes as u64 {
                anyhow::bail!("network_response_body_too_large");
            }
        }

        let status = response.status();
        let max_body_bytes = self.policy.max_body_bytes;
        let idle_timeout = self.policy.request_timeout;
        let mut response_stream = response.bytes_stream();
        let body = try_stream! {
            let mut observed_bytes = 0usize;
            while let Some(chunk) = tokio::time::timeout(idle_timeout, response_stream.next())
                .await
                .map_err(|_| anyhow::anyhow!("network_response_idle_timeout"))?
            {
                let chunk = chunk.context("network_response_body_read_failed")?;
                observed_bytes = observed_bytes.saturating_add(chunk.len());
                if observed_bytes > max_body_bytes {
                    Err(anyhow::anyhow!("network_response_body_too_large"))?;
                }
                yield chunk.to_vec();
            }
        };
        Ok(NetworkStreamResponse {
            status,
            final_url: parsed,
            body: Box::pin(body),
        })
    }

    pub async fn post_json_stream_with_decision_and_start_observer<F, Fut>(
        &self,
        url: &str,
        network_policy: &NetworkPolicy,
        network_policy_decision: &NetworkPolicyDecision,
        headers: HeaderMap,
        body: &Value,
        on_started: F,
    ) -> Result<NetworkStreamResponse>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        verify_network_policy_decision(url, network_policy, network_policy_decision)?;
        self.post_json_stream_for_capability_with_start_observer(
            url,
            Some(network_policy),
            &network_policy_decision.capability,
            headers,
            body,
            on_started,
        )
        .await
    }

    async fn read_text_response(
        &self,
        final_url: Url,
        response: reqwest::Response,
    ) -> Result<NetworkTextResponse> {
        let status = response.status();
        if let Some(length) = response.content_length() {
            if length > self.policy.max_body_bytes as u64 {
                anyhow::bail!("network_response_body_too_large");
            }
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = tokio::time::timeout(self.policy.request_timeout, stream.next())
            .await
            .map_err(|_| anyhow::anyhow!("network_response_idle_timeout"))?
        {
            let chunk = chunk.context("network_response_body_read_failed")?;
            if bytes.len().saturating_add(chunk.len()) > self.policy.max_body_bytes {
                anyhow::bail!("network_response_body_too_large");
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(NetworkTextResponse {
            status,
            final_url,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }

    async fn send_pinned_get<F, Fut>(
        &self,
        url: &Url,
        headers: HeaderMap,
        on_started: &mut F,
    ) -> Result<reqwest::Response>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let host = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("network_url_host_missing"))?;
        let route = resolve_network_egress_route(url, &self.policy).await?;
        let builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(self.policy.connect_timeout)
            .timeout(self.policy.request_timeout);
        let builder = match route {
            NetworkEgressRoute::DirectPinned(resolved) => {
                let mut builder = builder.no_proxy();
                if host_without_ipv6_brackets(host).parse::<IpAddr>().is_err() {
                    builder = builder.resolve_to_addrs(host, &resolved);
                }
                builder
            }
            NetworkEgressRoute::LoopbackSystemProxy(proxy) => builder.proxy(
                reqwest::Proxy::all(proxy.as_str())
                    .context("network_system_proxy_client_configuration_failed")?,
            ),
        };
        let client = builder.build().context("network_client_build_failed")?;

        let mut last_error = None;
        for attempt in 0..MAX_IDEMPOTENT_ATTEMPTS {
            on_started(NetworkDispatchAttemptPhase::Attempting).await?;
            let result = client
                .get(url.clone())
                .headers(headers.clone())
                .send()
                .await;
            match result {
                Ok(response)
                    if attempt + 1 < MAX_IDEMPOTENT_ATTEMPTS
                        && matches!(
                            response.status(),
                            StatusCode::BAD_GATEWAY
                                | StatusCode::SERVICE_UNAVAILABLE
                                | StatusCode::GATEWAY_TIMEOUT
                        ) =>
                {
                    on_started(NetworkDispatchAttemptPhase::ResponseHeadersObserved).await?;
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Ok(response) => {
                    on_started(NetworkDispatchAttemptPhase::ResponseHeadersObserved).await?;
                    return Ok(response);
                }
                Err(error) if attempt + 1 < MAX_IDEMPOTENT_ATTEMPTS => {
                    last_error = Some(error);
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Err(error) => return Err(error).context("network_request_failed"),
            }
        }
        Err(last_error
            .map(anyhow::Error::from)
            .unwrap_or_else(|| anyhow::anyhow!("network_request_failed")))
    }

    async fn send_pinned_post_once<F, Fut>(
        &self,
        url: &Url,
        headers: HeaderMap,
        body: Vec<u8>,
        on_started: &mut F,
    ) -> Result<reqwest::Response>
    where
        F: FnMut(NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let host = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("network_url_host_missing"))?;
        let route = resolve_network_egress_route(url, &self.policy).await?;
        let builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(self.policy.connect_timeout)
            .timeout(self.policy.request_timeout);
        let builder = match route {
            NetworkEgressRoute::DirectPinned(resolved) => {
                let mut builder = builder.no_proxy();
                if host_without_ipv6_brackets(host).parse::<IpAddr>().is_err() {
                    builder = builder.resolve_to_addrs(host, &resolved);
                }
                builder
            }
            NetworkEgressRoute::LoopbackSystemProxy(proxy) => builder.proxy(
                reqwest::Proxy::all(proxy.as_str())
                    .context("network_system_proxy_client_configuration_failed")?,
            ),
        };
        let request = builder
            .build()
            .context("network_client_build_failed")?
            .post(url.clone())
            .headers(headers)
            .body(body);
        on_started(NetworkDispatchAttemptPhase::Attempting)
            .await
            .context("network_dispatch_attempt_observer_rejected")?;
        let response = request
            .send()
            .await
            .context("network_non_idempotent_request_failed")?;
        on_started(NetworkDispatchAttemptPhase::ResponseHeadersObserved)
            .await
            .context("network_dispatch_observed_observer_rejected")?;
        Ok(response)
    }
}

fn retry_delay(attempt: usize) -> Duration {
    let jitter = rand::random::<u64>() % 75;
    Duration::from_millis(100 + (attempt as u64 * 150) + jitter)
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str().map(str::to_ascii_lowercase)
            == right.host_str().map(str::to_ascii_lowercase)
        && left.port_or_known_default() == right.port_or_known_default()
}

fn explicitly_configured_loopback_host(host: &str) -> bool {
    let host = host_without_ipv6_brackets(host);
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn host_without_ipv6_brackets(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
}

pub fn domain_matches(host: &str, rule: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let rule = rule
        .trim()
        .trim_start_matches("*.")
        .trim_matches('.')
        .to_ascii_lowercase();
    !rule.is_empty()
        && (host == rule
            || host
                .strip_suffix(&rule)
                .is_some_and(|prefix| prefix.ends_with('.')))
}

pub fn is_private_or_reserved_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, d] = ip.octets();
            ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_broadcast()
                || a == 0
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 192 && b == 0 && c == 0 && !matches!(d, 9 | 10))
                || (a == 192 && b == 0 && c == 2)
                || (a == 192 && b == 88 && c == 99)
                || (a == 198 && (b == 18 || b == 19))
                || (a == 198 && b == 51 && c == 100)
                || (a == 203 && b == 0 && c == 113)
        }
        IpAddr::V6(ip) => {
            let value = u128::from(ip);
            let mapped_private = ip
                .to_ipv4_mapped()
                .is_some_and(|mapped| is_private_or_reserved_ip(IpAddr::V4(mapped)));
            let nat64_well_known =
                ipv6_has_prefix(value, 0x0064_ff9b_0000_0000_0000_0000_0000_0000, 96);
            let nat64_embedded_private = nat64_well_known
                && is_private_or_reserved_ip(IpAddr::V4(std::net::Ipv4Addr::from(value as u32)));
            let global_unicast =
                ipv6_has_prefix(value, 0x2000_0000_0000_0000_0000_0000_0000_0000, 3);
            let ietf_protocol_assignment =
                ipv6_has_prefix(value, 0x2001_0000_0000_0000_0000_0000_0000_0000, 23);
            let globally_reachable_ietf_exception =
                matches!(
                    value,
                    0x2001_0001_0000_0000_0000_0000_0000_0001
                        ..=0x2001_0001_0000_0000_0000_0000_0000_0003
                ) || ipv6_has_prefix(value, 0x2001_0003_0000_0000_0000_0000_0000_0000, 32)
                    || ipv6_has_prefix(value, 0x2001_0004_0112_0000_0000_0000_0000_0000, 48)
                    || ipv6_has_prefix(value, 0x2001_0020_0000_0000_0000_0000_0000_0000, 28)
                    || ipv6_has_prefix(value, 0x2001_0030_0000_0000_0000_0000_0000_0000, 28);
            let special_non_global =
                ipv6_has_prefix(value, 0x0064_ff9b_0001_0000_0000_0000_0000_0000, 48)
                    || ipv6_has_prefix(value, 0x0100_0000_0000_0000_0000_0000_0000_0000, 64)
                    || ipv6_has_prefix(value, 0x0100_0000_0000_0001_0000_0000_0000_0000, 64)
                    || (ietf_protocol_assignment && !globally_reachable_ietf_exception)
                    || ipv6_has_prefix(value, 0x2001_0db8_0000_0000_0000_0000_0000_0000, 32)
                    || ipv6_has_prefix(value, 0x2002_0000_0000_0000_0000_0000_0000_0000, 16)
                    || ipv6_has_prefix(value, 0x3fff_0000_0000_0000_0000_0000_0000_0000, 20);
            ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_multicast()
                || mapped_private
                || nat64_embedded_private
                || (!global_unicast && !nat64_well_known)
                || special_non_global
        }
    }
}

fn ipv6_has_prefix(value: u128, network: u128, prefix: u32) -> bool {
    let mask = if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix)
    };
    value & mask == network & mask
}

fn validate_url_policy(
    url: &Url,
    network_policy: Option<&NetworkPolicy>,
    capability: &str,
    require_https: bool,
) -> Result<()> {
    let scheme_allowed = if require_https {
        url.scheme() == "https"
    } else {
        matches!(url.scheme(), "http" | "https")
    };
    if !scheme_allowed {
        anyhow::bail!("network_url_scheme_blocked");
    }
    if !url.username().is_empty() || url.password().is_some() {
        anyhow::bail!("network_url_userinfo_blocked");
    }
    url.host_str()
        .ok_or_else(|| anyhow::anyhow!("network_url_host_missing"))?;
    if let Some(policy) = network_policy {
        let decision = resolve_network_policy_decision(policy, url.as_str(), capability)?;
        match decision.disposition {
            NetworkPolicyDisposition::Allow => {}
            NetworkPolicyDisposition::Ask => anyhow::bail!(
                "network_policy_consent_required:decision_id={}",
                decision.decision_id
            ),
            NetworkPolicyDisposition::Deny => anyhow::bail!(
                "{}:decision_id={}",
                decision.reason_code,
                decision.decision_id
            ),
        }
    }
    Ok(())
}

fn is_fake_ip_benchmark_address(ip: IpAddr) -> bool {
    fn is_benchmarking_v4(ip: std::net::Ipv4Addr) -> bool {
        u32::from(ip) & 0xfffe_0000 == 0xc612_0000
    }

    match ip {
        IpAddr::V4(ip) => is_benchmarking_v4(ip),
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return is_benchmarking_v4(mapped);
            }
            let value = u128::from(ip);
            // macOS may surface RFC 2544 fake IPv4 answers through the
            // IPv4-translatable ::ffff:0:0:0/96 form.
            value >> 32 == 0x0000_0000_0000_0000_ffff_0000
                && is_benchmarking_v4(std::net::Ipv4Addr::from(value as u32))
        }
    }
}

#[cfg(any(target_os = "macos", test))]
fn validated_loopback_proxy_url(host: &str, port: i32) -> Result<Url> {
    let address = host
        .trim()
        .parse::<IpAddr>()
        .map_err(|_| anyhow::anyhow!("network_system_proxy_host_not_ip_literal"))?;
    if !address.is_loopback() {
        anyhow::bail!("network_system_proxy_not_loopback");
    }
    let port = u16::try_from(port)
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| anyhow::anyhow!("network_system_proxy_port_invalid"))?;
    let authority = match address {
        IpAddr::V4(address) => format!("{address}:{port}"),
        IpAddr::V6(address) => format!("[{address}]:{port}"),
    };
    Url::parse(&format!("http://{authority}")).context("network_system_proxy_url_invalid")
}

#[cfg(target_os = "macos")]
fn configured_system_proxy_url(destination_scheme: &str) -> Result<Option<Url>> {
    use system_configuration::core_foundation::{base::CFType, number::CFNumber, string::CFString};
    use system_configuration::dynamic_store::SCDynamicStoreBuilder;

    fn number(
        proxies: &system_configuration::core_foundation::dictionary::CFDictionary<CFString, CFType>,
        key: &str,
    ) -> Option<i32> {
        proxies
            .find(CFString::new(key))
            .and_then(|value| value.downcast::<CFNumber>())
            .and_then(|value| value.to_i32())
    }

    fn string(
        proxies: &system_configuration::core_foundation::dictionary::CFDictionary<CFString, CFType>,
        key: &str,
    ) -> Option<String> {
        proxies
            .find(CFString::new(key))
            .and_then(|value| value.downcast::<CFString>())
            .map(|value| value.to_string())
    }

    let (enable_key, host_key, port_key) = match destination_scheme {
        "https" => ("HTTPSEnable", "HTTPSProxy", "HTTPSPort"),
        "http" => ("HTTPEnable", "HTTPProxy", "HTTPPort"),
        _ => return Ok(None),
    };
    let store = SCDynamicStoreBuilder::new("OpenLife Network Egress")
        .build()
        .ok_or_else(|| anyhow::anyhow!("network_system_proxy_store_unavailable"))?;
    let proxies = store
        .get_proxies()
        .ok_or_else(|| anyhow::anyhow!("network_system_proxy_settings_unavailable"))?;
    if number(&proxies, enable_key).unwrap_or_default() != 1 {
        return Ok(None);
    }
    let host = string(&proxies, host_key)
        .ok_or_else(|| anyhow::anyhow!("network_system_proxy_host_missing"))?;
    let port = number(&proxies, port_key)
        .ok_or_else(|| anyhow::anyhow!("network_system_proxy_port_missing"))?;
    validated_loopback_proxy_url(&host, port).map(Some)
}

#[cfg(not(target_os = "macos"))]
fn configured_system_proxy_url(_destination_scheme: &str) -> Result<Option<Url>> {
    Ok(None)
}

fn select_network_egress_route(
    host: &str,
    addresses: Vec<SocketAddr>,
    allow_loopback: bool,
    fake_ip_proxy_domain_allowlist: &[String],
    destination_is_https: bool,
    configured_proxy: Option<Url>,
) -> Result<NetworkEgressRoute> {
    if allow_loopback && !explicitly_configured_loopback_host(host) {
        anyhow::bail!("network_loopback_policy_requires_explicit_host");
    }
    if addresses.is_empty() {
        anyhow::bail!("network_dns_no_addresses");
    }
    if allow_loopback {
        if addresses.iter().all(|address| address.ip().is_loopback()) {
            return Ok(NetworkEgressRoute::DirectPinned(addresses));
        }
        anyhow::bail!("network_loopback_endpoint_resolved_non_loopback");
    }
    if addresses
        .iter()
        .all(|address| !is_private_or_reserved_ip(address.ip()))
    {
        return Ok(NetworkEgressRoute::DirectPinned(addresses));
    }
    let host_is_name = host_without_ipv6_brackets(host).parse::<IpAddr>().is_err();
    if fake_ip_proxy_domain_allowlist
        .iter()
        .any(|rule| domain_matches(host, rule))
        && destination_is_https
        && host_is_name
        && addresses
            .iter()
            .all(|address| is_fake_ip_benchmark_address(address.ip()))
    {
        if let Some(proxy) = configured_proxy {
            return Ok(NetworkEgressRoute::LoopbackSystemProxy(proxy));
        }
    }
    anyhow::bail!("network_private_or_reserved_address_blocked")
}

async fn resolve_network_egress_route(
    url: &Url,
    policy: &NetworkClientPolicy,
) -> Result<NetworkEgressRoute> {
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("network_url_host_missing"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow::anyhow!("network_url_port_unknown"))?;
    let normalized_host = host_without_ipv6_brackets(host);
    let addresses = if let Ok(ip) = normalized_host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::time::timeout(
            policy.dns_timeout,
            tokio::net::lookup_host((normalized_host, port)),
        )
        .await
        .map_err(|_| anyhow::anyhow!("network_dns_timeout"))?
        .context("network_dns_failed")?
        .collect::<Vec<_>>()
    };
    let needs_proxy_lookup = policy
        .fake_ip_proxy_domain_allowlist
        .iter()
        .any(|rule| domain_matches(host, rule))
        && addresses
            .iter()
            .any(|address| is_private_or_reserved_ip(address.ip()));
    let configured_proxy = if needs_proxy_lookup {
        configured_system_proxy_url(url.scheme())?
    } else {
        None
    };
    select_network_egress_route(
        host,
        addresses,
        policy.allow_loopback,
        &policy.fake_ip_proxy_domain_allowlist,
        url.scheme() == "https",
        configured_proxy,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "external-live HTTPS and macOS system-proxy evidence"]
    async fn external_live_allowed_https_host_can_use_the_fake_ip_system_proxy() {
        let policy = NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        let response = NetworkClient::new(NetworkClientPolicy {
            fake_ip_proxy_domain_allowlist: vec!["example.com".into()],
            ..Default::default()
        })
        .get_text_with_headers_for_capability_and_start_observer(
            "https://example.com",
            Some(&policy),
            "web.fetch",
            HeaderMap::new(),
            |_| async { Ok(()) },
        )
        .await
        .expect("the policy-approved public HTTPS host should be reachable");
        assert!(response.status.is_success());
        assert!(response.body.contains("Example Domain"));
    }

    #[derive(Default)]
    struct DurableToolStartObserver {
        starts: std::sync::Mutex<Vec<crate::tool_execution_receipt::ToolExecutionReceipt>>,
    }

    #[async_trait::async_trait]
    impl crate::agent::ToolStartedTransitionObserver for DurableToolStartObserver {
        async fn after_dispatch(
            &self,
            receipt: &crate::tool_execution_receipt::ToolExecutionReceipt,
        ) -> anyhow::Result<()> {
            self.starts
                .lock()
                .expect("durable start observer mutex")
                .push(receipt.clone());
            Ok(())
        }
    }

    async fn read_http_request(stream: &mut tokio::net::TcpStream) -> String {
        use tokio::io::AsyncReadExt;

        let mut request = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let count = stream.read(&mut chunk).await.unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..count]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        String::from_utf8_lossy(&request).into_owned()
    }

    #[test]
    fn fake_ip_route_requires_an_enabled_verified_loopback_proxy() {
        let fake_v4 = SocketAddr::new("198.18.0.93".parse().unwrap(), 443);
        let fake_v6 = SocketAddr::new("::ffff:0:c612:5d".parse().unwrap(), 443);
        let proxy = validated_loopback_proxy_url("127.0.0.1", 1082).unwrap();
        let allowed_domains = vec!["example.com".into()];
        let route = select_network_egress_route(
            "api.example.com",
            vec![fake_v4, fake_v6],
            false,
            &allowed_domains,
            true,
            Some(proxy.clone()),
        )
        .unwrap();
        assert!(matches!(
            route,
            NetworkEgressRoute::LoopbackSystemProxy(observed) if observed == proxy
        ));

        assert!(select_network_egress_route(
            "api.example.com",
            vec![fake_v4],
            false,
            &[],
            true,
            Some(proxy.clone()),
        )
        .is_err());
        assert!(select_network_egress_route(
            "api.example.com",
            vec![fake_v4],
            false,
            &allowed_domains,
            true,
            None,
        )
        .is_err());
        assert!(select_network_egress_route(
            "api.example.com",
            vec![fake_v4],
            false,
            &allowed_domains,
            false,
            Some(proxy),
        )
        .is_err());
    }

    #[test]
    fn fake_ip_proxy_does_not_admit_literal_private_or_mixed_dns_targets() {
        let proxy = validated_loopback_proxy_url("127.0.0.1", 1082).unwrap();
        let allowed_domains = vec!["example.com".into()];
        assert!(select_network_egress_route(
            "192.168.1.2",
            vec![SocketAddr::new("192.168.1.2".parse().unwrap(), 443)],
            false,
            &["192.168.1.2".into()],
            true,
            Some(proxy.clone()),
        )
        .is_err());
        assert!(select_network_egress_route(
            "api.example.com",
            vec![
                SocketAddr::new("198.18.0.93".parse().unwrap(), 443),
                SocketAddr::new("8.8.8.8".parse().unwrap(), 443),
            ],
            false,
            &allowed_domains,
            true,
            Some(proxy),
        )
        .is_err());
        assert!(select_network_egress_route(
            "attacker.example.net",
            vec![SocketAddr::new("198.18.0.93".parse().unwrap(), 443)],
            false,
            &allowed_domains,
            true,
            Some(validated_loopback_proxy_url("127.0.0.1", 1082).unwrap()),
        )
        .is_err());
        assert!(validated_loopback_proxy_url("192.168.1.10", 1082).is_err());
        assert!(validated_loopback_proxy_url("localhost", 1082).is_err());
        assert!(validated_loopback_proxy_url("127.0.0.1", 0).is_err());
    }

    #[test]
    fn fake_ip_detection_covers_ipv4_and_macos_translatable_ipv6() {
        assert!(is_fake_ip_benchmark_address("198.18.0.93".parse().unwrap()));
        assert!(is_fake_ip_benchmark_address(
            "::ffff:0:c612:5d".parse().unwrap()
        ));
        assert!(!is_fake_ip_benchmark_address("8.8.8.8".parse().unwrap()));
        assert!(!is_fake_ip_benchmark_address(
            "192.168.1.2".parse().unwrap()
        ));
    }

    #[test]
    fn domain_policy_uses_dns_label_boundaries() {
        assert!(domain_matches("api.example.com", "example.com"));
        assert!(domain_matches("example.com", "example.com"));
        assert!(!domain_matches("evil-example.com", "example.com"));
        assert!(!domain_matches("example.com.evil.test", "example.com"));
    }

    #[test]
    fn provider_network_policy_decision_is_deterministic_and_fail_closed() {
        let url = "https://api.example.com/v1/chat/completions";
        let capability = "provider.openai";
        let allow = NetworkPolicy {
            default_decision: "allow".into(),
            ..NetworkPolicy::default()
        };
        let allow_decision = resolve_network_policy_decision(&allow, url, capability).unwrap();
        assert_eq!(allow_decision.disposition, NetworkPolicyDisposition::Allow);
        assert_eq!(
            allow_decision,
            resolve_network_policy_decision(&allow, url, capability).unwrap()
        );
        let other_path = "https://api.example.com/v2/responses";
        let other_path_decision =
            resolve_network_policy_decision(&allow, other_path, capability).unwrap();
        assert_ne!(allow_decision.decision_id, other_path_decision.decision_id);
        assert_ne!(
            allow_decision.endpoint_digest,
            other_path_decision.endpoint_digest
        );
        assert!(verify_network_policy_decision(other_path, &allow, &allow_decision).is_err());

        let mut ordered_overrides = std::collections::HashMap::new();
        ordered_overrides.insert("unrelated.second".into(), "deny".into());
        ordered_overrides.insert("provider".into(), "ALLOW".into());
        ordered_overrides.insert("unrelated.first".into(), "ask".into());
        let ordered_policy = NetworkPolicy {
            default_decision: " ASK ".into(),
            domain_allowlist: vec!["*.EXAMPLE.com".into(), "api.example.com".into()],
            tool_overrides: ordered_overrides,
            ..NetworkPolicy::default()
        };
        let mut reordered_overrides = std::collections::HashMap::new();
        reordered_overrides.insert("unrelated.first".into(), "ask".into());
        reordered_overrides.insert("provider".into(), " allow ".into());
        reordered_overrides.insert("unrelated.second".into(), "deny".into());
        let reordered_policy = NetworkPolicy {
            default_decision: "ask".into(),
            domain_allowlist: vec!["api.example.com.".into(), "example.com".into()],
            tool_overrides: reordered_overrides,
            ..NetworkPolicy::default()
        };
        assert_eq!(
            resolve_network_policy_decision(&ordered_policy, url, capability).unwrap(),
            resolve_network_policy_decision(&reordered_policy, url, capability).unwrap(),
            "semantically identical policy ordering must not change the decision ID"
        );

        let disabled = NetworkPolicy {
            enabled: false,
            ..allow.clone()
        };
        let disabled_decision =
            resolve_network_policy_decision(&disabled, url, capability).unwrap();
        assert_eq!(
            disabled_decision.disposition,
            NetworkPolicyDisposition::Deny
        );
        assert_eq!(disabled_decision.reason_code, "network_policy_disabled");

        let default_deny = NetworkPolicy {
            default_decision: "deny".into(),
            ..NetworkPolicy::default()
        };
        let deny_decision =
            resolve_network_policy_decision(&default_deny, url, capability).unwrap();
        assert_eq!(deny_decision.disposition, NetworkPolicyDisposition::Deny);
        assert_eq!(deny_decision.reason_code, "network_policy_default_deny");

        let ask = NetworkPolicy::default();
        let ask_decision = resolve_network_policy_decision(&ask, url, capability).unwrap();
        assert_eq!(ask_decision.disposition, NetworkPolicyDisposition::Ask);
        assert_eq!(ask_decision.reason_code, "network_policy_consent_required");

        let denylisted = NetworkPolicy {
            default_decision: "allow".into(),
            domain_denylist: vec!["example.com".into()],
            ..NetworkPolicy::default()
        };
        let denylisted_decision =
            resolve_network_policy_decision(&denylisted, url, capability).unwrap();
        assert_eq!(
            denylisted_decision.disposition,
            NetworkPolicyDisposition::Deny
        );
        assert_eq!(denylisted_decision.reason_code, "network_domain_denied");
    }

    #[tokio::test]
    async fn blocked_provider_decisions_never_reach_the_dispatch_observer() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let client = NetworkClient::new(NetworkClientPolicy {
            allow_loopback: true,
            max_redirects: 0,
            ..NetworkClientPolicy::default()
        });
        let url = "http://127.0.0.1:9/v1/chat/completions";
        let policies = [
            NetworkPolicy {
                enabled: false,
                default_decision: "allow".into(),
                ..NetworkPolicy::default()
            },
            NetworkPolicy {
                default_decision: "deny".into(),
                ..NetworkPolicy::default()
            },
            NetworkPolicy::default(),
            NetworkPolicy {
                default_decision: "allow".into(),
                domain_denylist: vec!["127.0.0.1".into()],
                ..NetworkPolicy::default()
            },
        ];
        for policy in policies {
            let decision =
                resolve_network_policy_decision(&policy, url, "provider.openai").unwrap();
            let observed = Arc::new(AtomicBool::new(false));
            let observer = Arc::clone(&observed);
            let error = client
                .post_json_text_with_decision_and_start_observer(
                    url,
                    &policy,
                    &decision,
                    HeaderMap::new(),
                    &serde_json::json!({"model": "test"}),
                    move |_| {
                        let observer = Arc::clone(&observer);
                        async move {
                            observer.store(true, Ordering::SeqCst);
                            Ok(())
                        }
                    },
                )
                .await
                .unwrap_err();
            assert_ne!(decision.disposition, NetworkPolicyDisposition::Allow);
            assert!(format!("{error:#}").contains(&decision.reason_code));
            assert!(!observed.load(Ordering::SeqCst));
        }
    }

    #[tokio::test]
    async fn generic_policy_api_rejects_ask_and_deny_before_dispatch() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let client = NetworkClient::new(NetworkClientPolicy {
            allow_loopback: true,
            ..NetworkClientPolicy::default()
        });
        let url = "http://127.0.0.1:9/effect";
        for policy in [
            NetworkPolicy::default(),
            NetworkPolicy {
                default_decision: "deny".into(),
                ..NetworkPolicy::default()
            },
        ] {
            let observed = Arc::new(AtomicBool::new(false));
            let observer = Arc::clone(&observed);
            let error = client
                .post_json_text_with_start_observer(
                    url,
                    Some(&policy),
                    HeaderMap::new(),
                    &serde_json::json!({"effect": true}),
                    move |_| {
                        let observer = Arc::clone(&observer);
                        async move {
                            observer.store(true, Ordering::SeqCst);
                            Ok(())
                        }
                    },
                )
                .await
                .unwrap_err();
            let rendered = format!("{error:#}");
            assert!(
                rendered.contains("network_policy_consent_required")
                    || rendered.contains("network_policy_default_deny")
            );
            assert!(!observed.load(Ordering::SeqCst));
        }
    }

    #[tokio::test]
    async fn capability_specific_http_edges_honor_web_search_override_before_dispatch() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let url = "http://127.0.0.1:9/search";
        let policy = NetworkPolicy {
            default_decision: "allow".into(),
            tool_overrides: std::collections::HashMap::from([
                ("web.fetch".into(), "allow".into()),
                ("web.search".into(), "deny".into()),
            ]),
            ..NetworkPolicy::default()
        };
        assert_eq!(
            resolve_network_policy_decision(&policy, url, "web.fetch")
                .unwrap()
                .disposition,
            NetworkPolicyDisposition::Allow
        );
        assert_eq!(
            resolve_network_policy_decision(&policy, url, "web.search")
                .unwrap()
                .disposition,
            NetworkPolicyDisposition::Deny
        );
        let client = NetworkClient::new(NetworkClientPolicy {
            allow_loopback: true,
            require_https: false,
            ..NetworkClientPolicy::default()
        });

        let get_observed = Arc::new(AtomicBool::new(false));
        let get_observer = Arc::clone(&get_observed);
        let get_error = client
            .get_text_with_headers_for_capability_and_start_observer(
                url,
                Some(&policy),
                "web.search",
                HeaderMap::new(),
                move |_| {
                    let get_observer = Arc::clone(&get_observer);
                    async move {
                        get_observer.store(true, Ordering::SeqCst);
                        Ok(())
                    }
                },
            )
            .await
            .unwrap_err();
        assert!(format!("{get_error:#}").contains("network_policy_override_deny"));
        assert!(!get_observed.load(Ordering::SeqCst));

        let post_observed = Arc::new(AtomicBool::new(false));
        let post_observer = Arc::clone(&post_observed);
        let post_error = client
            .post_json_text_for_capability_with_start_observer(
                url,
                Some(&policy),
                "web.search",
                HeaderMap::new(),
                &serde_json::json!({"query": "bounded"}),
                move |_| {
                    let post_observer = Arc::clone(&post_observer);
                    async move {
                        post_observer.store(true, Ordering::SeqCst);
                        Ok(())
                    }
                },
            )
            .await
            .unwrap_err();
        assert!(format!("{post_error:#}").contains("network_policy_override_deny"));
        assert!(!post_observed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn fallible_start_observer_rejects_before_any_http_bytes_are_dispatched() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let policy = NetworkPolicy {
            default_decision: "allow".into(),
            ..NetworkPolicy::default()
        };
        let decision = resolve_network_policy_decision(&policy, &url, "provider.openai").unwrap();
        let client = NetworkClient::new(NetworkClientPolicy {
            allow_loopback: true,
            require_https: false,
            ..NetworkClientPolicy::default()
        });

        let error = client
            .post_json_text_with_decision_and_start_observer(
                &url,
                &policy,
                &decision,
                HeaderMap::new(),
                &serde_json::json!({"model": "test"}),
                |_| async { anyhow::bail!("durable_start_persistence_failed") },
            )
            .await
            .unwrap_err();

        assert!(format!("{error:#}").contains("durable_start_persistence_failed"));
        assert!(matches!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        ));
    }

    #[test]
    fn private_reserved_and_rebinding_addresses_fail_closed() {
        for value in [
            "127.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "169.254.169.254",
            "192.0.2.1",
            "198.18.0.1",
            "224.0.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "64:ff9b:1::1",
            "64:ff9b::127.0.0.1",
            "100::1",
            "100:0:0:1::1",
            "2001:5::1",
            "2002::1",
            "3fff::1",
            "4000::1",
        ] {
            let ip = value.parse::<IpAddr>().unwrap();
            assert!(is_private_or_reserved_ip(ip), "{value}");
        }
        assert!(!is_private_or_reserved_ip("8.8.8.8".parse().unwrap()));
        assert!(!is_private_or_reserved_ip(
            "2606:4700:4700::1111".parse().unwrap()
        ));
        for value in [
            "192.0.0.9",
            "192.0.0.10",
            "192.31.196.1",
            "192.52.193.1",
            "192.175.48.1",
            "198.51.99.1",
            "203.0.112.1",
            "2001:1::1",
            "2001:1::2",
            "2001:1::3",
            "2001:3::1",
            "2001:4:112::1",
            "2001:20::1",
            "2001:30::1",
            "64:ff9b::8.8.8.8",
        ] {
            let ip = value.parse::<IpAddr>().unwrap();
            assert!(!is_private_or_reserved_ip(ip), "{value}");
        }
    }

    #[tokio::test]
    async fn loopback_fetch_is_blocked_before_connection() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let observed = Arc::new(AtomicBool::new(false));
        let observer = Arc::clone(&observed);
        let error = NetworkClient::new(NetworkClientPolicy::default())
            .get_text_with_start_observer("http://127.0.0.1:9/private", None, move |_| {
                let observer = Arc::clone(&observer);
                async move {
                    observer.store(true, Ordering::SeqCst);
                    Ok(())
                }
            })
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("network_private_or_reserved_address_blocked"));
        assert!(!observed.load(Ordering::SeqCst));
    }

    #[test]
    fn loopback_exception_requires_an_explicit_loopback_host() {
        assert!(explicitly_configured_loopback_host("localhost"));
        assert!(explicitly_configured_loopback_host("127.0.0.1"));
        assert!(explicitly_configured_loopback_host("::1"));
        assert!(explicitly_configured_loopback_host("[::1]"));
        assert!(!explicitly_configured_loopback_host("localhost.example"));
        assert!(!explicitly_configured_loopback_host("capture.example"));
    }

    #[tokio::test]
    async fn cross_origin_redirect_does_not_forward_caller_headers() {
        use tokio::io::AsyncWriteExt;

        let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_address = target.local_addr().unwrap();
        let (request_sender, request_receiver) = tokio::sync::oneshot::channel();
        let target_task = tokio::spawn(async move {
            let (mut stream, _) = target.accept().await.unwrap();
            let request = read_http_request(&mut stream).await;
            let _ = request_sender.send(request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        });

        let redirect = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let redirect_address = redirect.local_addr().unwrap();
        let redirect_task = tokio::spawn(async move {
            let (mut stream, _) = redirect.accept().await.unwrap();
            let _ = read_http_request(&mut stream).await;
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer must-not-leak".parse().unwrap());
        headers.insert("cookie", "session=must-not-leak".parse().unwrap());
        headers.insert("x-openlife-secret", "must-not-leak".parse().unwrap());
        let response = NetworkClient::new(NetworkClientPolicy {
            allow_loopback: true,
            max_redirects: 1,
            ..Default::default()
        })
        .get_text_with_headers(&format!("http://{redirect_address}/start"), None, headers)
        .await
        .unwrap();
        assert_eq!(response.body, "ok");

        let forwarded = request_receiver.await.unwrap().to_ascii_lowercase();
        assert!(!forwarded.contains("authorization:"));
        assert!(!forwarded.contains("cookie:"));
        assert!(!forwarded.contains("x-openlife-secret:"));
        redirect_task.await.unwrap();
        target_task.await.unwrap();
    }

    #[tokio::test]
    async fn idempotent_retry_records_two_attempts_but_one_durable_start_transition() {
        use std::sync::Arc;
        use tokio::io::AsyncWriteExt;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for status_line in ["503 Service Unavailable", "200 OK"] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let _ = read_http_request(&mut stream).await;
                let body = if status_line.starts_with("200") {
                    "ok"
                } else {
                    "retry"
                };
                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let tracker = crate::tool_execution_receipt::ToolExecutionReceiptTracker::new(
            Some("run-network-retry-observer".into()),
            Some("web.fetch".into()),
            "network-retry-observer".into(),
            crate::tool_execution_receipt::ToolActionEffect::ReadOnly,
            crate::tool_manifest::ToolIdempotencyContract::Idempotent,
        );
        let durable_observer = Arc::new(DurableToolStartObserver::default());
        let observed_tracker = tracker.clone();
        let observed_durable = Arc::clone(&durable_observer);

        let response = NetworkClient::new(NetworkClientPolicy {
            allow_loopback: true,
            ..Default::default()
        })
        .get_text_with_start_observer(&format!("http://{address}/retry"), None, move |phase| {
            let observed_tracker = observed_tracker.clone();
            let observed_durable = Arc::clone(&observed_durable);
            async move {
                match phase {
                    NetworkDispatchAttemptPhase::Attempting => {
                        observed_tracker.mark_network_dispatch_attempted();
                        Ok(())
                    }
                    NetworkDispatchAttemptPhase::ResponseHeadersObserved => {
                        observed_tracker.mark_network_dispatch_observed();
                        crate::agent::action_executor::observe_first_tool_started_transition(
                            &observed_tracker,
                            Some(observed_durable.as_ref()),
                        )
                        .await
                    }
                }
            }
        })
        .await
        .unwrap();

        tracker.mark_response_observed();
        tracker.mark_execution_succeeded();
        tracker.finish();
        let terminal = tracker.snapshot();
        assert_eq!(response.body, "ok");
        assert_eq!(terminal.dispatch_attempt_count, 2);
        {
            let starts = durable_observer
                .starts
                .lock()
                .expect("durable start observer mutex");
            assert_eq!(starts.len(), 1);
            assert_eq!(starts[0].dispatch_attempt_count, 1);
            assert_eq!(
                starts[0].transport_status,
                crate::tool_execution_receipt::ToolTransportStatus::Dispatched
            );
        }
        server.await.unwrap();
    }

    #[tokio::test]
    async fn send_errors_record_attempts_without_inventing_concrete_tool_start() {
        use std::sync::Arc;

        let unused = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = unused.local_addr().unwrap();
        drop(unused);
        let tracker = crate::tool_execution_receipt::ToolExecutionReceiptTracker::new(
            Some("run-network-send-error".into()),
            Some("web.fetch".into()),
            "network-send-error".into(),
            crate::tool_execution_receipt::ToolActionEffect::ReadOnly,
            crate::tool_manifest::ToolIdempotencyContract::Idempotent,
        );
        let durable_observer = Arc::new(DurableToolStartObserver::default());
        let observed_tracker = tracker.clone();
        let observed_durable = Arc::clone(&durable_observer);

        let error = NetworkClient::new(NetworkClientPolicy {
            allow_loopback: true,
            connect_timeout: Duration::from_millis(100),
            request_timeout: Duration::from_millis(250),
            ..Default::default()
        })
        .get_text_with_start_observer(
            &format!("http://{address}/send-error"),
            None,
            move |phase| {
                let observed_tracker = observed_tracker.clone();
                let observed_durable = Arc::clone(&observed_durable);
                async move {
                    match phase {
                        NetworkDispatchAttemptPhase::Attempting => {
                            observed_tracker.mark_network_dispatch_attempted();
                            Ok(())
                        }
                        NetworkDispatchAttemptPhase::ResponseHeadersObserved => {
                            observed_tracker.mark_network_dispatch_observed();
                            crate::agent::action_executor::observe_first_tool_started_transition(
                                &observed_tracker,
                                Some(observed_durable.as_ref()),
                            )
                            .await
                        }
                    }
                }
            },
        )
        .await
        .expect_err("closed listener must fail both GET attempts");
        assert!(error.to_string().contains("network_request_failed"));

        tracker.mark_remote_unknown();
        tracker.finish();
        let terminal = tracker.snapshot();
        assert_eq!(terminal.dispatch_attempt_count, 2);
        assert!(!terminal.dispatch_observed);
        assert!(terminal.dispatched_at.is_none());
        assert_eq!(
            terminal.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::RemoteUnknown
        );
        assert!(durable_observer
            .starts
            .lock()
            .expect("durable observer mutex")
            .is_empty());
        terminal
            .mechanically_valid_terminal()
            .expect("send-error ambiguity is a valid unknown terminal");
    }

    #[tokio::test]
    async fn total_response_duration_is_bounded_even_when_chunks_keep_arriving() {
        use tokio::io::AsyncWriteExt;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _ = read_http_request(&mut stream).await;
            if stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n")
                .await
                .is_err()
            {
                return;
            }
            for _ in 0..10 {
                if stream.write_all(b"x").await.is_err() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });

        let result = tokio::time::timeout(
            Duration::from_secs(5),
            NetworkClient::new(NetworkClientPolicy {
                allow_loopback: true,
                request_timeout: Duration::from_millis(70),
                ..Default::default()
            })
            .get_text(&format!("http://{address}/trickle"), None),
        )
        .await
        .expect("request must finish within the test watchdog");
        let error = result.unwrap_err();

        assert!(format!("{error:#}").contains("timed out"));
        server.await.unwrap();
    }
}
