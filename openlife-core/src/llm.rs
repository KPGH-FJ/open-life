use crate::config::NetworkPolicy;
use crate::network_client::NetworkPolicyDecision;
use anyhow::{Context, Result};
use async_stream::try_stream;
use futures::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::IpAddr;
use std::pin::Pin;
use std::time::Duration;

pub type StreamResult = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

const CHAT_REQUEST_TIMEOUT_SECS: u64 = 120;
const STREAM_CONNECT_TIMEOUT_SECS: u64 = 20;
const STREAM_IDLE_TIMEOUT_SECS: u64 = 45;
const PROVIDER_MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const PROVIDER_MAX_SSE_FRAME_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

const MAX_PREPARED_MESSAGES: usize = 128;
pub const MAX_PREPARED_CONTEXT_BLOCKS: usize = 32;
pub const MAX_PREPARED_CONTENT_CHARS: usize = 262_144;

/// A minimal, auditable description of the context selected before a provider call.
///
/// This is evidence about selection, not a second copy of the selected content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManifest {
    pub request_id: String,
    pub privacy_decision_id: String,
    pub selected_context_refs: Vec<String>,
    pub included_context_categories: Vec<String>,
    /// Typed categories for payload carried in chat messages rather than in a
    /// BoundedContextBlock. Arbitrary caller strings cannot claim these lanes.
    #[serde(default)]
    pub declared_payload_categories: Vec<ProviderPayloadCategory>,
    /// Policy/guidance provenance that influenced payload compilation but is
    /// not itself sent as a context block. These refs are independently typed
    /// so they cannot masquerade as outbound content.
    #[serde(default)]
    pub policy_provenance_refs: Vec<ProviderPolicyProvenanceRef>,
    pub raw_life_model_included: bool,
    pub raw_unbounded_memory_included: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPayloadCategory {
    CurrentUserConversation,
    RuntimeCompiledMessages,
    FrozenEvaluationInput,
    MainChatReactCandidateRanking,
    A2aAuthenticatedUserMessage,
    ExplicitProviderProbe,
    PrivacyPolicyMasked,
}

impl ProviderPayloadCategory {
    fn as_str(self) -> &'static str {
        match self {
            Self::CurrentUserConversation => "current_user_conversation",
            Self::RuntimeCompiledMessages => "runtime_compiled_messages",
            Self::FrozenEvaluationInput => "frozen_evaluation_input",
            Self::MainChatReactCandidateRanking => "main_chat_react_candidate_ranking",
            Self::A2aAuthenticatedUserMessage => "a2a_authenticated_user_message",
            Self::ExplicitProviderProbe => "explicit_provider_probe",
            Self::PrivacyPolicyMasked => "privacy_policy_masked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPolicyProvenanceKind {
    MainChatRouteDecision,
    PolicyStoreRouteDecision,
    HsRouteDecision,
    ScheduledRouteDecision,
    ExplicitProviderProbeDecision,
    FailClosedRouteDecision,
    PolicyStorePolicy,
    HsPolicy,
    HsGuidance,
}

impl ProviderPolicyProvenanceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::MainChatRouteDecision => "main_chat_route_decision",
            Self::PolicyStoreRouteDecision => "policy_store_route_decision",
            Self::HsRouteDecision => "hs_route_decision",
            Self::ScheduledRouteDecision => "scheduled_route_decision",
            Self::ExplicitProviderProbeDecision => "explicit_provider_probe_decision",
            Self::FailClosedRouteDecision => "fail_closed_route_decision",
            Self::PolicyStorePolicy => "policy_store_policy",
            Self::HsPolicy => "hs_policy",
            Self::HsGuidance => "hs_guidance",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPolicyProvenanceRef {
    kind: ProviderPolicyProvenanceKind,
    reference_id: String,
    digest: String,
}

impl ProviderPolicyProvenanceRef {
    pub(crate) fn new(
        kind: ProviderPolicyProvenanceKind,
        reference_id: impl Into<String>,
        digest: impl Into<String>,
    ) -> Self {
        let digest = digest.into();
        let normalized_digest = if digest
            .strip_prefix("sha256:")
            .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            digest
        } else if digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            format!("sha256:{digest}")
        } else {
            response_body_digest(&digest)
        };
        Self {
            kind,
            reference_id: reference_id.into(),
            digest: normalized_digest,
        }
    }

    pub fn kind(&self) -> ProviderPolicyProvenanceKind {
        self.kind
    }

    pub fn reference_id(&self) -> &str {
        &self.reference_id
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn validate(&self) -> Result<()> {
        let digest_hex = self
            .digest
            .strip_prefix("sha256:")
            .ok_or_else(|| anyhow::anyhow!("provider policy provenance digest is not sha256"))?;
        if self.reference_id.trim().is_empty()
            || digest_hex.len() != 64
            || !digest_hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            anyhow::bail!("provider policy provenance reference is invalid");
        }
        Ok(())
    }
}

/// A bounded context block selected by the caller's privacy/context policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedContextBlock {
    pub source_ref: String,
    pub category: String,
    pub content: String,
}

impl ContextManifest {
    pub fn validate_context_truth(&self, context_blocks: &[BoundedContextBlock]) -> Result<()> {
        if self.raw_life_model_included || self.raw_unbounded_memory_included {
            anyhow::bail!("provider context manifest includes forbidden raw canonical data");
        }
        let mut expected_refs = context_blocks
            .iter()
            .map(|block| block.source_ref.clone())
            .collect::<Vec<_>>();
        let mut expected_categories = context_blocks
            .iter()
            .map(|block| block.category.clone())
            .collect::<Vec<_>>();
        if expected_refs.iter().any(|value| value.trim().is_empty())
            || expected_categories
                .iter()
                .any(|value| value.trim().is_empty())
        {
            anyhow::bail!("prepared provider context block has an empty reference or category");
        }
        expected_refs.sort();
        expected_refs.dedup();
        if expected_refs.len() != context_blocks.len() {
            anyhow::bail!("prepared provider context block references are not unique");
        }
        expected_categories.sort();
        expected_categories.dedup();
        if self.selected_context_refs != expected_refs {
            anyhow::bail!(
                "context manifest selected refs do not match the outbound context blocks"
            );
        }
        if self.included_context_categories != expected_categories {
            anyhow::bail!("context manifest categories do not match the outbound context blocks");
        }
        if self.declared_payload_categories.is_empty() {
            anyhow::bail!("context manifest is missing a typed message payload category");
        }
        let mut canonical_payload_categories = self.declared_payload_categories.clone();
        canonical_payload_categories.sort();
        canonical_payload_categories.dedup();
        if self.declared_payload_categories != canonical_payload_categories {
            anyhow::bail!("context manifest payload categories are not canonical");
        }
        for provenance in &self.policy_provenance_refs {
            provenance.validate()?;
        }
        let mut canonical_provenance = self.policy_provenance_refs.clone();
        canonical_provenance.sort();
        canonical_provenance.dedup();
        if self.policy_provenance_refs != canonical_provenance {
            anyhow::bail!("context manifest policy provenance is not canonical");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderDataRoute {
    PolicyAllowed,
    LocalOnly,
}

/// Canonical authority that issued a provider data-route decision.
///
/// This is deliberately a closed enum. A caller-visible route label or context
/// manifest string is evidence about a decision, never authority to create one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPolicyAuthority {
    MainChatPolicyRouter,
    PolicyStore,
    HsPolicyStore,
    ScheduledPolicy,
    ExplicitProviderProbePolicy,
    LocalOnlyFailClosed,
}

/// Typed reasons for restricting an otherwise absent or broader provider
/// decision to local execution. This constructor can never authorize cloud.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLocalOnlyReason {
    MissingCanonicalPolicy,
    CloudDisabled,
    CanonicalRouteIntersection,
    DeserializedCapabilityUnavailable,
    TestFixture,
}

/// Closed purposes for provider payloads derived from one canonical policy
/// subject. The purpose is part of the in-memory digest; changing the payload
/// or reusing it for another execution surface invalidates authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPayloadPurpose {
    MainChatDirectAnswer,
    MainChatArtifactDraft,
    MainChatReactRanking,
    AgentLoopStep,
    AgentRuntimeGeneration,
    LayeredReasoningPhase,
    FrozenRuntimeEvaluation,
    ExplicitProviderProbe,
}

impl ProviderPayloadPurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MainChatDirectAnswer => "main_chat_direct_answer",
            Self::MainChatArtifactDraft => "main_chat_artifact_draft",
            Self::MainChatReactRanking => "main_chat_react_ranking",
            Self::AgentLoopStep => "agent_loop_step",
            Self::AgentRuntimeGeneration => "agent_runtime_generation",
            Self::LayeredReasoningPhase => "layered_reasoning_phase",
            Self::FrozenRuntimeEvaluation => "frozen_runtime_evaluation",
            Self::ExplicitProviderProbe => "explicit_provider_probe",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProviderPolicySubject {
    MainChatCurrentUser {
        message_id: String,
        message_digest: String,
    },
    HsCurrentUser {
        message_digest: Option<String>,
    },
    PolicyStoreCurrentUser {
        message_digest: Option<String>,
    },
    ScheduledCurrentUser {
        task_id: String,
        attempt_id: String,
        grant_digest: String,
        message_digest: String,
        provider_digest: Option<String>,
        model_digest: Option<String>,
        grant_expires_at: Option<String>,
    },
    ExplicitProviderProbe {
        authorization_id: String,
        provider_digest: String,
        model_digest: String,
        endpoint_digest: String,
        network_policy_decision_digest: String,
        consent_reference_digest: String,
    },
    LocalOnly,
}

/// An in-process capability issued only after a canonical policy object has
/// been mechanically validated.
///
/// Its fields are private and the capability is intentionally not
/// serializable. Durable records persist the decision reference and route, not
/// a replayable authorization token. Deserialized requests therefore receive a
/// fail-closed local-only default and cannot regain cloud access from strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPolicyAuthorization {
    decision_id: String,
    policy_version: String,
    data_route: ProviderDataRoute,
    authority: ProviderPolicyAuthority,
    effective_local_restriction: Option<ProviderLocalOnlyReason>,
    subject: ProviderPolicySubject,
    authorized_unfiltered_payload_purpose: Option<ProviderPayloadPurpose>,
    authorized_unfiltered_payload_digest: Option<String>,
    prepared_envelope_digest: Option<String>,
}

impl Default for ProviderPolicyAuthorization {
    fn default() -> Self {
        Self::local_only_fail_closed(ProviderLocalOnlyReason::DeserializedCapabilityUnavailable)
    }
}

impl ProviderPolicyAuthorization {
    /// Issue the narrow in-process capability used by the explicit Settings
    /// connection probe. This is intentionally separate from Main Chat: a
    /// command click is not a current-user conversation message and must never
    /// be represented as one.
    ///
    /// The caller must already have resolved an allowed network decision. The
    /// exact provider, model, endpoint, decision, consent reference, and fixed
    /// probe payload are sealed into private digests. The capability is not
    /// serializable, so a persisted or client-supplied request cannot regain
    /// this authority.
    pub(crate) fn from_explicit_provider_probe(
        grant: &crate::network_client::ExplicitProviderProbeGrant,
        messages: &[ChatMessage],
        context_blocks: &[BoundedContextBlock],
    ) -> Result<Self> {
        let provider = grant.provider_target().trim();
        let model = grant.model_target().trim();
        let endpoint = grant.endpoint().trim();
        let consent_reference = grant.consent_reference().trim();
        if provider.is_empty()
            || model.is_empty()
            || endpoint.is_empty()
            || consent_reference.is_empty()
            || grant.network_policy_decision().disposition
                != crate::network_client::NetworkPolicyDisposition::Allow
        {
            anyhow::bail!("explicit provider probe authorization is incomplete");
        }
        let parsed =
            reqwest::Url::parse(endpoint).context("explicit provider probe endpoint is invalid")?;
        if parsed.host_str().is_none() {
            anyhow::bail!("explicit provider probe endpoint has no host");
        }
        let provider_digest = response_body_digest(provider);
        let model_digest = response_body_digest(model);
        let endpoint_digest = response_body_digest(endpoint);
        let network_policy_decision_digest =
            provider_network_policy_decision_digest(grant.network_policy_decision());
        let consent_reference_digest = response_body_digest(consent_reference);
        let authorization_id = response_body_digest(&format!(
            "explicit_provider_probe_v1:{provider_digest}:{model_digest}:{endpoint_digest}:{network_policy_decision_digest}:{consent_reference_digest}"
        ));
        let subject = ProviderPolicySubject::ExplicitProviderProbe {
            authorization_id: authorization_id.clone(),
            provider_digest,
            model_digest,
            endpoint_digest,
            network_policy_decision_digest,
            consent_reference_digest,
        };
        let subject_scope = match &subject {
            ProviderPolicySubject::ExplicitProviderProbe {
                authorization_id,
                provider_digest,
                model_digest,
                endpoint_digest,
                network_policy_decision_digest,
                consent_reference_digest,
            } => format!(
                "explicit_provider_probe:{authorization_id}:{provider_digest}:{model_digest}:{endpoint_digest}:{network_policy_decision_digest}:{consent_reference_digest}"
            ),
            _ => unreachable!("explicit provider probe subject was just constructed"),
        };
        let payload_purpose = ProviderPayloadPurpose::ExplicitProviderProbe;
        let payload_digest = provider_unfiltered_payload_digest(
            &subject_scope,
            payload_purpose,
            messages,
            context_blocks,
        );
        Ok(Self {
            decision_id: authorization_id,
            policy_version: "explicit_provider_probe_v1".into(),
            data_route: ProviderDataRoute::PolicyAllowed,
            authority: ProviderPolicyAuthority::ExplicitProviderProbePolicy,
            effective_local_restriction: None,
            subject,
            authorized_unfiltered_payload_purpose: Some(payload_purpose),
            authorized_unfiltered_payload_digest: Some(payload_digest),
            prepared_envelope_digest: None,
        })
    }

    pub fn from_main_chat_ingress(
        decision: &crate::agent::main_chat_agent_v1::AgentIngressDecision,
    ) -> Result<Self> {
        decision
            .validate_policy_projection()
            .map_err(|reason| anyhow::anyhow!("invalid Main Chat provider policy: {reason}"))?;
        Ok(Self {
            decision_id: decision.request_id.clone(),
            policy_version: decision.policy_decision.policy_version.clone(),
            data_route: decision.policy_decision.data_route,
            authority: ProviderPolicyAuthority::MainChatPolicyRouter,
            effective_local_restriction: None,
            subject: ProviderPolicySubject::MainChatCurrentUser {
                message_id: decision.policy_decision.authorized_user_message_id.clone(),
                message_digest: decision
                    .policy_decision
                    .authorized_user_message_digest
                    .clone(),
            },
            authorized_unfiltered_payload_purpose: None,
            authorized_unfiltered_payload_digest: None,
            prepared_envelope_digest: None,
        })
    }

    pub(crate) fn from_hs_context_decision(
        decision: &crate::agent::ContextPolicyDecision,
        decision_id: impl Into<String>,
    ) -> Result<Self> {
        decision.validate_provider_authority()?;
        let decision_id = decision_id.into();
        if decision_id.trim().is_empty() {
            anyhow::bail!("HS provider policy decision is missing its decision reference");
        }
        let data_route = match decision.route() {
            crate::agent::ModelRoutePolicy::CloudAllowed => ProviderDataRoute::PolicyAllowed,
            crate::agent::ModelRoutePolicy::LocalOnly => ProviderDataRoute::LocalOnly,
        };
        Ok(Self {
            decision_id,
            policy_version: "hs_policy_store_v1".into(),
            data_route,
            authority: ProviderPolicyAuthority::HsPolicyStore,
            effective_local_restriction: None,
            subject: ProviderPolicySubject::HsCurrentUser {
                message_digest: None,
            },
            authorized_unfiltered_payload_purpose: None,
            authorized_unfiltered_payload_digest: None,
            prepared_envelope_digest: None,
        })
    }

    pub(crate) fn from_policy_store_context_decision(
        decision: &crate::agent::ContextPolicyDecision,
        decision_id: impl Into<String>,
    ) -> Result<Self> {
        decision.validate_provider_authority()?;
        let decision_id = decision_id.into();
        if decision_id.trim().is_empty() {
            anyhow::bail!("PolicyStore provider decision is missing its decision reference");
        }
        let data_route = match decision.route() {
            crate::agent::ModelRoutePolicy::CloudAllowed => ProviderDataRoute::PolicyAllowed,
            crate::agent::ModelRoutePolicy::LocalOnly => ProviderDataRoute::LocalOnly,
        };
        Ok(Self {
            decision_id,
            policy_version: "policy_store_v1".into(),
            data_route,
            authority: ProviderPolicyAuthority::PolicyStore,
            effective_local_restriction: None,
            subject: ProviderPolicySubject::PolicyStoreCurrentUser {
                message_digest: None,
            },
            authorized_unfiltered_payload_purpose: None,
            authorized_unfiltered_payload_digest: None,
            prepared_envelope_digest: None,
        })
    }

    pub fn from_scheduled_claim(claim: &crate::tasks::ScheduledTaskClaim) -> Result<Self> {
        claim.validate_policy_authority()?;
        Ok(Self {
            decision_id: claim.provider_grant().policy_decision_digest.clone(),
            policy_version: claim.provider_grant().policy_version.clone(),
            data_route: claim.provider_grant().data_route,
            authority: ProviderPolicyAuthority::ScheduledPolicy,
            effective_local_restriction: None,
            subject: ProviderPolicySubject::ScheduledCurrentUser {
                task_id: claim.task().id.clone(),
                attempt_id: claim.attempt_id().to_string(),
                grant_digest: claim.provider_grant().grant_id.clone(),
                message_digest: claim.provider_grant().subject_digest.clone(),
                provider_digest: claim.provider_grant().provider_digest.clone(),
                model_digest: claim.provider_grant().model_digest.clone(),
                grant_expires_at: claim.provider_grant().grant_expires_at.clone(),
            },
            authorized_unfiltered_payload_purpose: None,
            authorized_unfiltered_payload_digest: None,
            prepared_envelope_digest: None,
        })
    }

    pub fn local_only_fail_closed(reason: ProviderLocalOnlyReason) -> Self {
        let reason_label = match reason {
            ProviderLocalOnlyReason::MissingCanonicalPolicy => "missing_canonical_policy",
            ProviderLocalOnlyReason::CloudDisabled => "cloud_disabled",
            ProviderLocalOnlyReason::CanonicalRouteIntersection => "canonical_route_intersection",
            ProviderLocalOnlyReason::DeserializedCapabilityUnavailable => {
                "deserialized_capability_unavailable"
            }
            ProviderLocalOnlyReason::TestFixture => "test_fixture",
        };
        Self {
            decision_id: response_body_digest(&format!("provider_local_only_v1:{reason_label}")),
            policy_version: "provider_local_only_v1".into(),
            data_route: ProviderDataRoute::LocalOnly,
            authority: ProviderPolicyAuthority::LocalOnlyFailClosed,
            effective_local_restriction: Some(reason),
            subject: ProviderPolicySubject::LocalOnly,
            authorized_unfiltered_payload_purpose: None,
            authorized_unfiltered_payload_digest: None,
            prepared_envelope_digest: None,
        }
    }

    /// Tightening is always allowed; this operation cannot elevate a
    /// local-only capability or mint cloud authority.
    pub fn restrict_to_local(&self, reason: ProviderLocalOnlyReason) -> Self {
        let mut restricted = self.clone();
        restricted.data_route = ProviderDataRoute::LocalOnly;
        restricted.effective_local_restriction = Some(reason);
        restricted
    }

    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }

    pub fn policy_version(&self) -> &str {
        &self.policy_version
    }

    pub fn data_route(&self) -> ProviderDataRoute {
        self.data_route
    }

    pub fn authority(&self) -> ProviderPolicyAuthority {
        self.authority
    }

    pub fn effective_local_restriction(&self) -> Option<ProviderLocalOnlyReason> {
        self.effective_local_restriction
    }

    pub(crate) fn subject_scope_digest(&self) -> String {
        response_body_digest(&self.subject_scope_material())
    }

    fn receipt_evidence_for_request(
        &self,
        manifest: &ContextManifest,
        network_policy_decision: &NetworkPolicyDecision,
        provider_config_generation: &str,
    ) -> ProviderPolicyReceiptEvidence {
        ProviderPolicyReceiptEvidence {
            decision_id: self.decision_id.clone(),
            policy_version: self.policy_version.clone(),
            issuing_authority: self.authority,
            effective_data_route: self.data_route,
            effective_local_restriction: self.effective_local_restriction,
            subject_scope_digest: response_body_digest(&self.subject_scope_material()),
            payload_purpose: self.authorized_unfiltered_payload_purpose,
            unfiltered_payload_digest: self.authorized_unfiltered_payload_digest.clone(),
            context_manifest_digest: provider_context_manifest_digest(manifest),
            prepared_envelope_digest: self.prepared_envelope_digest.clone(),
            provider_config_generation: provider_config_generation.to_string(),
            network_policy_decision_digest: provider_network_policy_decision_digest(
                network_policy_decision,
            ),
            selected_context_refs: manifest.selected_context_refs.clone(),
            included_context_categories: manifest.included_context_categories.clone(),
            declared_payload_categories: manifest.declared_payload_categories.clone(),
            policy_provenance_refs: manifest.policy_provenance_refs.clone(),
            raw_life_model_included: manifest.raw_life_model_included,
            raw_unbounded_memory_included: manifest.raw_unbounded_memory_included,
        }
    }

    /// Bind the HS policy decision to the actual current task text. HS policy
    /// selection is performed before the provider payload is compiled, so the
    /// selector cannot safely infer this subject from an intent summary.
    pub(crate) fn bind_hs_current_user_subject(mut self, user_text: &str) -> Result<Self> {
        match &mut self.subject {
            ProviderPolicySubject::HsCurrentUser { message_digest } => {
                let digest = response_body_digest(user_text);
                if let Some(existing) = message_digest {
                    if existing != &digest {
                        anyhow::bail!("HS provider policy subject cannot be rebound");
                    }
                } else {
                    *message_digest = Some(digest);
                }
                Ok(self)
            }
            _ => anyhow::bail!("only HS policy authority can bind an HS task subject"),
        }
    }

    pub(crate) fn bind_policy_store_current_user_subject(
        mut self,
        user_text: &str,
    ) -> Result<Self> {
        match &mut self.subject {
            ProviderPolicySubject::PolicyStoreCurrentUser { message_digest } => {
                let digest = response_body_digest(user_text);
                if let Some(existing) = message_digest {
                    if existing != &digest {
                        anyhow::bail!("PolicyStore provider subject cannot be rebound");
                    }
                } else {
                    *message_digest = Some(digest);
                }
                Ok(self)
            }
            _ => anyhow::bail!("only PolicyStore authority can bind a PolicyStore task subject"),
        }
    }

    fn validate_subject_text(&self, user_text: &str) -> Result<()> {
        if self.subject == ProviderPolicySubject::LocalOnly {
            return Ok(());
        }
        let actual = response_body_digest(user_text);
        let expected = match &self.subject {
            ProviderPolicySubject::MainChatCurrentUser { message_digest, .. } => {
                Some(message_digest.as_str())
            }
            ProviderPolicySubject::HsCurrentUser { message_digest } => message_digest.as_deref(),
            ProviderPolicySubject::PolicyStoreCurrentUser { message_digest } => {
                message_digest.as_deref()
            }
            ProviderPolicySubject::ScheduledCurrentUser { message_digest, .. } => {
                Some(message_digest.as_str())
            }
            ProviderPolicySubject::ExplicitProviderProbe { .. } => None,
            ProviderPolicySubject::LocalOnly => None,
        }
        .ok_or_else(|| anyhow::anyhow!("provider policy authorization has no bound subject"))?;
        if actual != expected {
            anyhow::bail!("provider policy authorization subject mismatch");
        }
        Ok(())
    }

    fn subject_scope_material(&self) -> String {
        match &self.subject {
            ProviderPolicySubject::MainChatCurrentUser {
                message_id,
                message_digest,
            } => format!("main_chat:{message_id}:{message_digest}"),
            ProviderPolicySubject::HsCurrentUser { message_digest } => format!(
                "hs:{}",
                message_digest.as_deref().unwrap_or("unbound_subject")
            ),
            ProviderPolicySubject::PolicyStoreCurrentUser { message_digest } => format!(
                "policy_store:{}",
                message_digest.as_deref().unwrap_or("unbound_subject")
            ),
            ProviderPolicySubject::ScheduledCurrentUser {
                task_id,
                attempt_id,
                grant_digest,
                message_digest,
                provider_digest,
                model_digest,
                grant_expires_at,
            } => format!(
                "scheduled:{task_id}:{attempt_id}:{grant_digest}:{message_digest}:{}:{}:{}",
                provider_digest.as_deref().unwrap_or("any_provider"),
                model_digest.as_deref().unwrap_or("any_model"),
                grant_expires_at.as_deref().unwrap_or("no_expiry")
            ),
            ProviderPolicySubject::ExplicitProviderProbe {
                authorization_id,
                provider_digest,
                model_digest,
                endpoint_digest,
                network_policy_decision_digest,
                consent_reference_digest,
            } => format!(
                "explicit_provider_probe:{authorization_id}:{provider_digest}:{model_digest}:{endpoint_digest}:{network_policy_decision_digest}:{consent_reference_digest}"
            ),
            ProviderPolicySubject::LocalOnly => "local_only_fail_closed".into(),
        }
    }

    pub(crate) fn validate_explicit_provider_probe_target(
        &self,
        provider_target: &str,
        model_target: &str,
        endpoint: &str,
        network_policy_decision: &NetworkPolicyDecision,
    ) -> Result<()> {
        let ProviderPolicySubject::ExplicitProviderProbe {
            provider_digest,
            model_digest,
            endpoint_digest,
            network_policy_decision_digest,
            ..
        } = &self.subject
        else {
            return Ok(());
        };
        if provider_digest != &response_body_digest(provider_target.trim())
            || model_digest != &response_body_digest(model_target.trim())
            || endpoint_digest != &response_body_digest(endpoint.trim())
            || network_policy_decision_digest
                != &provider_network_policy_decision_digest(network_policy_decision)
            || network_policy_decision.disposition
                != crate::network_client::NetworkPolicyDisposition::Allow
        {
            anyhow::bail!("explicit provider probe target differs from its authorization");
        }
        Ok(())
    }

    /// Authorize one exact compiled payload derived from the already-bound
    /// current user message. The closed purpose plus payload digest prevents a
    /// capability issued for one turn from being reused for another compiled
    /// provider request.
    pub fn authorize_derived_payload(
        mut self,
        purpose: ProviderPayloadPurpose,
        current_user_text: &str,
        messages: &[ChatMessage],
        context_blocks: &[BoundedContextBlock],
    ) -> Result<Self> {
        self.validate_subject_text(current_user_text)?;
        let digest = provider_unfiltered_payload_digest(
            &self.subject_scope_material(),
            purpose,
            messages,
            context_blocks,
        );
        match (
            self.authorized_unfiltered_payload_purpose,
            self.authorized_unfiltered_payload_digest.as_deref(),
        ) {
            (None, None) => {
                self.authorized_unfiltered_payload_purpose = Some(purpose);
                self.authorized_unfiltered_payload_digest = Some(digest);
                Ok(self)
            }
            (Some(existing_purpose), Some(existing_digest))
                if existing_purpose == purpose && existing_digest == digest =>
            {
                Ok(self)
            }
            (Some(_), Some(_)) => {
                anyhow::bail!("provider policy exact payload scope cannot be rebound")
            }
            _ => anyhow::bail!("provider policy exact payload scope is incomplete"),
        }
    }

    pub(crate) fn validate_unfiltered_payload(
        &self,
        messages: &[ChatMessage],
        context_blocks: &[BoundedContextBlock],
    ) -> Result<()> {
        if self.prepared_envelope_digest.is_some() {
            anyhow::bail!("provider policy capability is already bound to a prepared envelope");
        }
        match (
            self.authorized_unfiltered_payload_purpose,
            self.authorized_unfiltered_payload_digest.as_deref(),
        ) {
            (Some(purpose), Some(expected)) => {
                let actual = provider_unfiltered_payload_digest(
                    &self.subject_scope_material(),
                    purpose,
                    messages,
                    context_blocks,
                );
                if actual != expected {
                    anyhow::bail!("provider policy derived payload mismatch");
                }
                Ok(())
            }
            (None, None) => anyhow::bail!(
                "provider policy authorization is missing an exact unfiltered payload scope"
            ),
            _ => anyhow::bail!("provider policy derived payload scope is incomplete"),
        }
    }

    // Every provider envelope field is independently bound into the authenticated digest.
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub(crate) fn bind_prepared_envelope(
        mut self,
        messages: &[ChatMessage],
        context_blocks: &[BoundedContextBlock],
        manifest: &ContextManifest,
        provider_target: &str,
        model_target: &str,
        provider_endpoint: &str,
        provider_config_generation: &str,
        credential_version: u64,
        tools_required: bool,
    ) -> Result<Self> {
        if self.prepared_envelope_digest.is_some() {
            anyhow::bail!("provider policy prepared envelope cannot be rebound");
        }
        self.prepared_envelope_digest = Some(provider_prepared_envelope_digest(
            messages,
            context_blocks,
            manifest,
            provider_target,
            model_target,
            provider_endpoint,
            provider_config_generation,
            credential_version,
            tools_required,
        ));
        Ok(self)
    }

    // Validation recomputes the exact envelope from all independently bound fields.
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    fn validate_for_request(
        &self,
        messages: &[ChatMessage],
        context_blocks: &[BoundedContextBlock],
        manifest: &ContextManifest,
        provider_target: &str,
        model_target: &str,
        provider_endpoint: &str,
        provider_config_generation: &str,
        credential_version: u64,
        network_policy_decision: &NetworkPolicyDecision,
        tools_required: bool,
    ) -> Result<()> {
        if self.decision_id.trim().is_empty() || self.policy_version.trim().is_empty() {
            anyhow::bail!("provider policy authorization is incomplete");
        }
        if manifest.privacy_decision_id != self.decision_id {
            anyhow::bail!(
                "prepared provider request policy authorization does not match its context manifest"
            );
        }
        if self.authority == ProviderPolicyAuthority::LocalOnlyFailClosed
            && self.data_route != ProviderDataRoute::LocalOnly
        {
            anyhow::bail!("fail-closed provider authorization cannot allow cloud execution");
        }
        if self.effective_local_restriction.is_some()
            && self.data_route != ProviderDataRoute::LocalOnly
        {
            anyhow::bail!("provider local restriction cannot authorize cloud execution");
        }
        if let ProviderPolicySubject::ScheduledCurrentUser {
            provider_digest,
            model_digest,
            grant_expires_at,
            ..
        } = &self.subject
        {
            let actual_provider_digest = response_body_digest(provider_target);
            let actual_model_digest = response_body_digest(model_target);
            if provider_digest.as_deref() != Some(actual_provider_digest.as_str())
                || model_digest
                    .as_deref()
                    .is_some_and(|expected| expected != actual_model_digest)
            {
                anyhow::bail!("scheduled provider target differs from its reviewed grant");
            }
            if let Some(expires_at) = grant_expires_at {
                let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at)
                    .context("scheduled provider grant expiry is invalid")?
                    .with_timezone(&chrono::Utc);
                if expires_at <= chrono::Utc::now() {
                    anyhow::bail!("scheduled provider grant expired before adapter dispatch");
                }
            }
        }
        if let ProviderPolicySubject::ExplicitProviderProbe {
            provider_digest,
            model_digest,
            endpoint_digest,
            network_policy_decision_digest,
            ..
        } = &self.subject
        {
            if provider_digest != &response_body_digest(provider_target.trim())
                || model_digest != &response_body_digest(model_target.trim())
                || endpoint_digest != &response_body_digest(provider_endpoint.trim())
                || network_policy_decision_digest
                    != &provider_network_policy_decision_digest(network_policy_decision)
                || network_policy_decision.disposition
                    != crate::network_client::NetworkPolicyDisposition::Allow
            {
                anyhow::bail!("explicit provider probe request binding mismatch");
            }
        }
        let expected_envelope = provider_prepared_envelope_digest(
            messages,
            context_blocks,
            manifest,
            provider_target,
            model_target,
            provider_endpoint,
            provider_config_generation,
            credential_version,
            tools_required,
        );
        if self.prepared_envelope_digest.as_deref() != Some(expected_envelope.as_str()) {
            anyhow::bail!("prepared provider request policy authorization envelope mismatch");
        }
        Ok(())
    }
}

fn append_scope_part(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u64).to_be_bytes());
    target.extend_from_slice(value);
}

// The digest commits every prepared-provider field without an unauthenticated wrapper.
#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
fn provider_prepared_envelope_digest(
    messages: &[ChatMessage],
    context_blocks: &[BoundedContextBlock],
    manifest: &ContextManifest,
    provider_target: &str,
    model_target: &str,
    provider_endpoint: &str,
    provider_config_generation: &str,
    credential_version: u64,
    tools_required: bool,
) -> String {
    let mut scope = Vec::new();
    append_scope_part(&mut scope, b"prepared_provider_envelope_v1");
    append_scope_part(&mut scope, manifest.request_id.as_bytes());
    append_scope_part(&mut scope, manifest.privacy_decision_id.as_bytes());
    for context_ref in &manifest.selected_context_refs {
        append_scope_part(&mut scope, context_ref.as_bytes());
    }
    for category in &manifest.included_context_categories {
        append_scope_part(&mut scope, category.as_bytes());
    }
    for category in &manifest.declared_payload_categories {
        append_scope_part(&mut scope, category.as_str().as_bytes());
    }
    for provenance in &manifest.policy_provenance_refs {
        append_scope_part(&mut scope, provenance.kind().as_str().as_bytes());
        append_scope_part(&mut scope, provenance.reference_id().as_bytes());
        append_scope_part(&mut scope, provenance.digest().as_bytes());
    }
    append_scope_part(
        &mut scope,
        &[
            u8::from(manifest.raw_life_model_included),
            u8::from(manifest.raw_unbounded_memory_included),
        ],
    );
    append_scope_part(&mut scope, provider_target.as_bytes());
    append_scope_part(&mut scope, model_target.as_bytes());
    append_scope_part(&mut scope, provider_endpoint.as_bytes());
    append_scope_part(&mut scope, provider_config_generation.as_bytes());
    append_scope_part(&mut scope, &credential_version.to_be_bytes());
    append_scope_part(&mut scope, &[u8::from(tools_required)]);
    for message in messages {
        append_scope_part(&mut scope, message.role.as_bytes());
        append_scope_part(&mut scope, message.content.as_bytes());
    }
    for block in context_blocks {
        append_scope_part(&mut scope, block.source_ref.as_bytes());
        append_scope_part(&mut scope, block.category.as_bytes());
        append_scope_part(&mut scope, block.content.as_bytes());
    }
    response_bytes_digest(&scope)
}

fn provider_context_manifest_digest(manifest: &ContextManifest) -> String {
    let mut scope = Vec::new();
    append_scope_part(&mut scope, b"provider_context_manifest_v1");
    append_scope_part(&mut scope, manifest.request_id.as_bytes());
    append_scope_part(&mut scope, manifest.privacy_decision_id.as_bytes());
    for context_ref in &manifest.selected_context_refs {
        append_scope_part(&mut scope, context_ref.as_bytes());
    }
    for category in &manifest.included_context_categories {
        append_scope_part(&mut scope, category.as_bytes());
    }
    for category in &manifest.declared_payload_categories {
        append_scope_part(&mut scope, category.as_str().as_bytes());
    }
    for provenance in &manifest.policy_provenance_refs {
        append_scope_part(&mut scope, provenance.kind().as_str().as_bytes());
        append_scope_part(&mut scope, provenance.reference_id().as_bytes());
        append_scope_part(&mut scope, provenance.digest().as_bytes());
    }
    append_scope_part(
        &mut scope,
        &[
            u8::from(manifest.raw_life_model_included),
            u8::from(manifest.raw_unbounded_memory_included),
        ],
    );
    response_bytes_digest(&scope)
}

pub(crate) fn provider_network_policy_decision_digest(decision: &NetworkPolicyDecision) -> String {
    let mut scope = Vec::new();
    append_scope_part(&mut scope, b"provider_network_policy_decision_v1");
    append_scope_part(&mut scope, decision.decision_id.as_bytes());
    append_scope_part(&mut scope, decision.disposition.as_str().as_bytes());
    append_scope_part(&mut scope, decision.reason_code.as_bytes());
    append_scope_part(&mut scope, decision.capability.as_bytes());
    append_scope_part(&mut scope, decision.host.as_bytes());
    append_scope_part(&mut scope, decision.endpoint_digest.as_bytes());
    response_bytes_digest(&scope)
}

fn provider_unfiltered_payload_digest(
    subject_scope: &str,
    purpose: ProviderPayloadPurpose,
    messages: &[ChatMessage],
    context_blocks: &[BoundedContextBlock],
) -> String {
    let mut scope = Vec::new();
    append_scope_part(&mut scope, b"provider_unfiltered_payload_v1");
    append_scope_part(&mut scope, subject_scope.as_bytes());
    append_scope_part(&mut scope, purpose.as_str().as_bytes());
    for message in messages {
        append_scope_part(&mut scope, message.role.as_bytes());
        append_scope_part(&mut scope, message.content.as_bytes());
    }
    for block in context_blocks {
        append_scope_part(&mut scope, block.source_ref.as_bytes());
        append_scope_part(&mut scope, block.category.as_bytes());
        append_scope_part(&mut scope, block.content.as_bytes());
    }
    response_bytes_digest(&scope)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderInvocationStatus {
    /// A response terminal was observed and the response satisfied the
    /// prepared-request contract.
    Completed,
    /// A provider terminal was observed and explicitly rejected the request,
    /// or the complete response failed the adapter contract. This is not used
    /// for transport loss after dispatch.
    Failed,
    /// The adapter edge was crossed, but no trustworthy remote terminal was
    /// observed (for example timeout, disconnect, or local abort).
    RemoteUnknown,
}

#[derive(Debug)]
struct ConfirmedProviderTerminalFailure {
    reason_code: &'static str,
}

impl std::fmt::Display for ConfirmedProviderTerminalFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.reason_code)
    }
}

impl std::error::Error for ConfirmedProviderTerminalFailure {}

/// Bind an adapter error to a terminal that was actually observed. Errors
/// without this marker are deliberately treated as `remote_unknown` once a
/// dispatch start has been recorded.
pub(crate) fn confirmed_provider_terminal_failure(
    reason_code: &'static str,
    source: anyhow::Error,
) -> anyhow::Error {
    source.context(ConfirmedProviderTerminalFailure { reason_code })
}

pub(crate) fn provider_error_terminal_status(error: &anyhow::Error) -> ProviderInvocationStatus {
    // `ConfirmedProviderTerminalFailure` is stored as typed anyhow context.
    // Iterating std::error::Error::source() does not expose that context value
    // for downcasting; anyhow's own `is` walks both contexts and sources.
    if error.is::<ConfirmedProviderTerminalFailure>() {
        ProviderInvocationStatus::Failed
    } else {
        ProviderInvocationStatus::RemoteUnknown
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPolicyReceiptEvidence {
    pub decision_id: String,
    pub policy_version: String,
    pub issuing_authority: ProviderPolicyAuthority,
    pub effective_data_route: ProviderDataRoute,
    pub effective_local_restriction: Option<ProviderLocalOnlyReason>,
    pub subject_scope_digest: String,
    pub payload_purpose: Option<ProviderPayloadPurpose>,
    pub unfiltered_payload_digest: Option<String>,
    pub context_manifest_digest: String,
    pub prepared_envelope_digest: Option<String>,
    /// Exact immutable scheduler generation that prepared and executed this
    /// request. This is metadata lineage, not adapter-origin authorization.
    #[serde(default)]
    pub provider_config_generation: String,
    pub network_policy_decision_digest: String,
    pub selected_context_refs: Vec<String>,
    pub included_context_categories: Vec<String>,
    pub declared_payload_categories: Vec<ProviderPayloadCategory>,
    pub policy_provenance_refs: Vec<ProviderPolicyProvenanceRef>,
    pub raw_life_model_included: bool,
    pub raw_unbounded_memory_included: bool,
}

impl ProviderPolicyReceiptEvidence {
    /// Validate the metadata-only truth carried with an observed provider
    /// attempt. This never authorizes execution; it only prevents an
    /// incomplete or internally contradictory provenance record from becoming
    /// durable evidence.
    pub fn validate_minimal_truth(&self) -> Result<()> {
        let valid_digest = |value: &str| {
            value.strip_prefix("sha256:").is_some_and(|hex| {
                hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        };
        if self.decision_id.trim().is_empty()
            || self.policy_version.trim().is_empty()
            || !valid_digest(&self.subject_scope_digest)
            || !valid_digest(&self.context_manifest_digest)
            || self.provider_config_generation.trim().is_empty()
            || !valid_digest(&self.network_policy_decision_digest)
        {
            anyhow::bail!("provider policy receipt evidence is incomplete");
        }
        if self.payload_purpose.is_none()
            || self
                .unfiltered_payload_digest
                .as_deref()
                .is_none_or(|digest| !valid_digest(digest))
            || self
                .prepared_envelope_digest
                .as_deref()
                .is_none_or(|digest| !valid_digest(digest))
        {
            anyhow::bail!("provider policy receipt evidence is missing exact payload scope");
        }
        if self.declared_payload_categories.is_empty()
            || self.raw_life_model_included
            || self.raw_unbounded_memory_included
        {
            anyhow::bail!("provider policy receipt evidence contains unsafe payload truth");
        }
        if self.issuing_authority == ProviderPolicyAuthority::LocalOnlyFailClosed
            && self.effective_data_route != ProviderDataRoute::LocalOnly
        {
            anyhow::bail!("fail-closed provider evidence cannot claim cloud authorization");
        }
        if self.effective_local_restriction.is_some()
            && self.effective_data_route != ProviderDataRoute::LocalOnly
        {
            anyhow::bail!("provider evidence local restriction conflicts with its route");
        }
        for provenance in &self.policy_provenance_refs {
            provenance.validate()?;
        }
        Ok(())
    }

    /// Stable metadata-only digest used to bind durable start and terminal
    /// facts. It intentionally excludes provider request/response bodies.
    pub fn evidence_digest(&self) -> Result<String> {
        self.validate_minimal_truth()?;
        let encoded = serde_json::to_vec(self)
            .context("provider policy receipt evidence serialization failed")?;
        Ok(response_bytes_digest(&encoded))
    }
}

/// Stable lifecycle identity shared by the adapter start and terminal facts for
/// one exact provider request. The policy evidence is validated and normalized
/// before hashing; request/provider/model/config generation are bound outside
/// the serialized evidence so a receipt cannot be replayed onto another
/// adapter attempt while retaining the same policy digest.
pub fn provider_lifecycle_evidence_digest(
    request_id: &str,
    provider: &str,
    model: &str,
    evidence: &ProviderPolicyReceiptEvidence,
) -> Result<String> {
    evidence.validate_minimal_truth()?;
    if request_id.trim().is_empty() || provider.trim().is_empty() || model.trim().is_empty() {
        anyhow::bail!("provider lifecycle identity is incomplete");
    }
    let policy_evidence_digest = evidence.evidence_digest()?;
    let mut scope = Vec::new();
    append_scope_part(&mut scope, b"provider_lifecycle_evidence_v1");
    append_scope_part(&mut scope, request_id.as_bytes());
    append_scope_part(&mut scope, provider.as_bytes());
    append_scope_part(&mut scope, model.as_bytes());
    append_scope_part(&mut scope, evidence.provider_config_generation.as_bytes());
    append_scope_part(&mut scope, policy_evidence_digest.as_bytes());
    Ok(response_bytes_digest(&scope))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderInvocationReceipt {
    pub request_id: String,
    pub provider: String,
    pub model: String,
    pub status: ProviderInvocationStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub finished_at: chrono::DateTime<chrono::Utc>,
    pub error_digest: Option<String>,
    pub simulated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_evidence: Option<ProviderPolicyReceiptEvidence>,
}

#[derive(Debug)]
pub struct PreparedProviderOutcome {
    /// Present only when the adapter edge was actually crossed. A routing,
    /// validation, or credential rejection before dispatch is not a provider
    /// invocation and must remain `not_attempted` at higher projections.
    pub receipt: Option<ProviderInvocationReceipt>,
    /// Runtime-only authority proving that the exact terminal receipt came
    /// from this process's prepared-provider adapter edge. The proof is
    /// intentionally non-serde and cannot be reconstructed from `receipt`.
    pub terminal_proof: Option<crate::scheduler::ProviderInvocationTerminalProof>,
    pub result: std::result::Result<String, String>,
}

/// Transient execution facts sealed at preparation time.
///
/// The credential is deliberately absent from serde and debug output.  A
/// deserialized request has no binding and therefore cannot cross the adapter
/// edge.  The public generation/version fields below are metadata-only; this
/// private binding is the execution proof.
#[derive(Clone, Default)]
pub(crate) struct ProviderExecutionBinding {
    endpoint: String,
    api_key: String,
    provider_config_generation: String,
    credential_version: u64,
}

impl std::fmt::Debug for ProviderExecutionBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderExecutionBinding")
            .field("endpoint", &self.endpoint)
            .field("api_key", &"[REDACTED]")
            .field(
                "provider_config_generation",
                &self.provider_config_generation,
            )
            .field("credential_version", &self.credential_version)
            .finish()
    }
}

impl ProviderExecutionBinding {
    pub(crate) fn new(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        provider_config_generation: impl Into<String>,
        credential_version: u64,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            provider_config_generation: provider_config_generation.into(),
            credential_version,
        }
    }

    fn validate_public_binding(&self, request: &PreparedProviderRequest) -> Result<()> {
        if self.endpoint != request.provider_endpoint
            || self.provider_config_generation != request.provider_config_generation
            || self.credential_version != request.provider_credential_version
        {
            anyhow::bail!("prepared provider execution binding mismatch");
        }
        Ok(())
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn api_key(&self) -> &str {
        &self.api_key
    }
}

/// The only request shape accepted by the privacy-safe provider execution seam.
///
/// Provider credentials and canonical LifeModel/HS stores deliberately cannot be
/// represented by this type.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(Clone))]
pub struct PreparedProviderRequest {
    pub messages: Vec<ChatMessage>,
    pub context_blocks: Vec<BoundedContextBlock>,
    pub context_manifest: ContextManifest,
    pub provider_target: String,
    pub model_target: String,
    /// Exact final adapter URL, including path, selected during preparation.
    pub provider_endpoint: String,
    /// In-process scheduler generation that prepared this request.
    pub provider_config_generation: String,
    /// Non-secret credential identity generation from canonical configuration.
    pub provider_credential_version: u64,
    pub data_route: ProviderDataRoute,
    #[serde(skip)]
    pub(crate) policy_authorization: ProviderPolicyAuthorization,
    pub network_policy: NetworkPolicy,
    pub network_policy_decision: NetworkPolicyDecision,
    pub tools_required: bool,
    #[serde(skip)]
    pub(crate) execution_binding: Option<ProviderExecutionBinding>,
}

impl PreparedProviderRequest {
    pub fn policy_receipt_evidence(&self) -> ProviderPolicyReceiptEvidence {
        self.policy_authorization.receipt_evidence_for_request(
            &self.context_manifest,
            &self.network_policy_decision,
            &self.provider_config_generation,
        )
    }

    pub fn validate(&self) -> Result<()> {
        if self.context_manifest.request_id.trim().is_empty() {
            anyhow::bail!("prepared provider request is missing request_id");
        }
        if self.context_manifest.privacy_decision_id.trim().is_empty() {
            anyhow::bail!("prepared provider request is missing privacy_decision_id");
        }
        self.context_manifest
            .validate_context_truth(&self.context_blocks)?;
        self.policy_authorization.validate_for_request(
            &self.messages,
            &self.context_blocks,
            &self.context_manifest,
            &self.provider_target,
            &self.model_target,
            &self.provider_endpoint,
            &self.provider_config_generation,
            self.provider_credential_version,
            &self.network_policy_decision,
            self.tools_required,
        )?;
        self.policy_receipt_evidence().validate_minimal_truth()?;
        if self.data_route != self.policy_authorization.data_route() {
            anyhow::bail!(
                "prepared provider request data route does not match policy authorization"
            );
        }
        if self.provider_target.trim().is_empty() || self.model_target.trim().is_empty() {
            anyhow::bail!("prepared provider request is missing its provider/model target");
        }
        if self.provider_endpoint.trim().is_empty()
            || self.provider_config_generation.trim().is_empty()
        {
            anyhow::bail!("prepared provider request is missing its endpoint/config generation");
        }
        self.execution_binding
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!("prepared provider request has no in-process execution binding")
            })?
            .validate_public_binding(self)?;
        if self.context_manifest.raw_life_model_included {
            anyhow::bail!("prepared provider request cannot include raw LifeModel data");
        }
        if self.context_manifest.raw_unbounded_memory_included {
            anyhow::bail!("prepared provider request cannot include unbounded memory data");
        }
        if self.data_route == ProviderDataRoute::LocalOnly && self.provider_target != "ollama" {
            anyhow::bail!("local-only prepared request cannot target a cloud provider");
        }
        if self.provider_target != "ollama"
            && (self.data_route != ProviderDataRoute::PolicyAllowed
                || self.policy_authorization.authority()
                    == ProviderPolicyAuthority::LocalOnlyFailClosed)
        {
            anyhow::bail!("cloud provider request is missing verified policy authorization");
        }
        if self.network_policy_decision.decision_id.trim().is_empty()
            || self.network_policy_decision.capability.trim().is_empty()
        {
            anyhow::bail!("prepared provider request is missing its network policy decision");
        }
        if self.messages.len() > MAX_PREPARED_MESSAGES {
            anyhow::bail!("prepared provider request exceeds the message count limit");
        }
        if self.context_blocks.len() > MAX_PREPARED_CONTEXT_BLOCKS {
            anyhow::bail!("prepared provider request exceeds the context block limit");
        }
        let content_chars = self
            .messages
            .iter()
            .map(|message| message.content.chars().count())
            .chain(
                self.context_blocks
                    .iter()
                    .map(|block| block.content.chars().count()),
            )
            .sum::<usize>();
        if content_chars > MAX_PREPARED_CONTENT_CHARS {
            anyhow::bail!("prepared provider request exceeds the content limit");
        }
        Ok(())
    }

    pub fn policy_authorization(&self) -> &ProviderPolicyAuthorization {
        &self.policy_authorization
    }

    pub fn system_prompt(&self) -> Option<String> {
        let prompt = self
            .context_blocks
            .iter()
            .filter_map(|block| {
                let content = block.content.trim();
                (!content.is_empty()).then(|| {
                    format!(
                        "[context:{}:{}]\n{}",
                        block.category, block.source_ref, content
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        (!prompt.is_empty()).then_some(prompt)
    }
}

pub fn provider_label(provider: &str) -> String {
    match provider {
        "deepseek" => "DeepSeek".to_string(),
        "openrouter" => "OpenRouter".to_string(),
        "openai" => "OpenAI".to_string(),
        "siliconflow" => "SiliconFlow".to_string(),
        "moonshot" => "Moonshot/Kimi".to_string(),
        "dashscope" => "通义千问 DashScope".to_string(),
        "zhipu" => "智谱 GLM".to_string(),
        _ => "OpenAI-compatible".to_string(),
    }
}

fn provider_endpoint_identity(provider: &str, openai_base: &str) -> Option<String> {
    let provider = provider.trim().to_ascii_lowercase();
    if provider.is_empty() {
        return None;
    }
    let endpoint = chat_completions_url(&provider, openai_base);
    let parsed = reqwest::Url::parse(&endpoint).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }
    let host = parsed
        .host_str()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let port = parsed.port_or_known_default()?;
    let path = parsed.path().trim_end_matches('/');
    Some(format!(
        "{provider}|{}://{host}:{port}{path}",
        parsed.scheme()
    ))
}

/// Whether `openai_base` resolves to the provider's canonical credential
/// endpoint. Environment credentials are scoped to this exact identity and
/// must never follow a user-editable proxy or custom base URL.
pub fn provider_endpoint_is_official(provider: &str, openai_base: &str) -> bool {
    let provider = provider.trim().to_ascii_lowercase();
    if !matches!(
        provider.as_str(),
        "deepseek" | "openrouter" | "openai" | "siliconflow" | "moonshot" | "dashscope" | "zhipu"
    ) {
        return false;
    }
    provider_endpoint_identity(&provider, openai_base).is_some_and(|identity| {
        provider_endpoint_identity(&provider, default_base_for_provider(&provider)).as_deref()
            == Some(identity.as_str())
    })
}

/// Resolve a credential for one exact provider endpoint.
///
/// A configured/keychain-hydrated key is already bound by the configuration
/// write path and can be used for that configured endpoint. Provider-specific
/// environment variables are implicit credentials and are therefore eligible
/// only for the provider's canonical endpoint.
pub fn effective_api_key_for_endpoint(
    provider: &str,
    openai_base: &str,
    configured_key: &str,
) -> String {
    if !configured_key.trim().is_empty() {
        return configured_key.to_string();
    }
    if !provider_endpoint_is_official(provider, openai_base) {
        return String::new();
    }
    match provider {
        "deepseek" => std::env::var("DEEPSEEK_API_KEY").unwrap_or_default(),
        "openrouter" => std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
        "openai" => std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        "siliconflow" => std::env::var("SILICONFLOW_API_KEY").unwrap_or_default(),
        "moonshot" => std::env::var("MOONSHOT_API_KEY").unwrap_or_default(),
        "dashscope" => std::env::var("DASHSCOPE_API_KEY").unwrap_or_default(),
        "zhipu" => std::env::var("ZHIPU_API_KEY").unwrap_or_default(),
        _ => String::new(),
    }
}

pub fn default_base_for_provider(provider: &str) -> &'static str {
    match provider {
        "deepseek" => "https://api.deepseek.com",
        "openrouter" => "https://openrouter.ai/api/v1",
        "siliconflow" => "https://api.siliconflow.cn/v1",
        "moonshot" => "https://api.moonshot.cn/v1",
        "dashscope" => "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "zhipu" => "https://open.bigmodel.cn/api/paas/v4",
        _ => "https://api.openai.com/v1",
    }
}

pub fn chat_completions_url(provider: &str, openai_base: &str) -> String {
    let base = if openai_base.trim().is_empty() {
        default_base_for_provider(provider).to_string()
    } else {
        openai_base.trim().trim_end_matches('/').to_string()
    };
    if base.ends_with("/chat/completions") {
        base
    } else {
        format!("{}/chat/completions", base)
    }
}

pub fn provider_models_url(provider: &str, openai_base: &str) -> String {
    let base = if openai_base.trim().is_empty() {
        default_base_for_provider(provider).to_string()
    } else {
        openai_base.trim().trim_end_matches('/').to_string()
    };
    if base.ends_with("/models") {
        base
    } else {
        format!("{base}/models")
    }
}

/// Return the metadata-safe identity used to bind provider credentials to one
/// executable runtime generation. The credential itself is never persisted in
/// config, receipts, or validation projections.
pub fn provider_credential_identity(api_key: &str) -> String {
    let hash = digest(&SHA256, api_key.as_bytes());
    let hex = hash
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

/// Resolve the concrete chat model before a provider request is prepared.
///
/// Adapters must send this exact model and must never silently substitute a
/// stream-only target. Any future compatibility mapping belongs in the
/// ProviderRouter before the prepared envelope and receipt are sealed.
pub fn resolve_provider_chat_model(_provider: &str, chat_model: &str) -> String {
    chat_model.trim().to_string()
}

fn extract_chat_content(json: &serde_json::Value) -> Option<String> {
    json["choices"][0]["message"]["content"]
        .as_str()
        .or_else(|| json["choices"][0]["text"].as_str())
        .or_else(|| json["output_text"].as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn extract_stream_content(json: &serde_json::Value) -> Option<String> {
    json["choices"][0]["delta"]["content"]
        .as_str()
        .or_else(|| json["choices"][0]["message"]["content"].as_str())
        .or_else(|| json["choices"][0]["text"].as_str())
        .or_else(|| json["delta"]["content"].as_str())
        .or_else(|| json["content"].as_str())
        .map(ToString::to_string)
        .filter(|s| !s.is_empty())
}

fn has_reasoning_content(json: &serde_json::Value) -> bool {
    json["choices"][0]["delta"]["reasoning_content"]
        .as_str()
        .or_else(|| json["choices"][0]["message"]["reasoning_content"].as_str())
        .or_else(|| json["delta"]["reasoning_content"].as_str())
        .or_else(|| json["reasoning_content"].as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some()
}

fn has_provider_stream_error(json: &serde_json::Value) -> bool {
    json.get("error").is_some_and(|error| !error.is_null())
        || json
            .get("type")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| kind.eq_ignore_ascii_case("error"))
}

fn response_bytes_digest(body: &[u8]) -> String {
    let hash = digest(&SHA256, body);
    let mut hex = String::with_capacity(hash.as_ref().len() * 2);
    for byte in hash.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    format!("sha256:{hex}")
}

fn response_body_digest(body: &str) -> String {
    response_bytes_digest(body.as_bytes())
}

fn provider_network_client(
    provider: &str,
    url: &str,
) -> Result<crate::network_client::NetworkClient> {
    let parsed = reqwest::Url::parse(url).context("provider endpoint is not a valid URL")?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("provider endpoint is missing its host"))?;
    let explicitly_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    let official_endpoint = provider_endpoint_allows_system_fake_ip_proxy(provider, &parsed);

    Ok(crate::network_client::NetworkClient::new(
        crate::network_client::NetworkClientPolicy {
            // Public provider credentials must never cross plaintext HTTP.
            // Explicit loopback remains available for local OpenAI-compatible
            // providers and the repository's capture-adapter evidence.
            require_https: !explicitly_loopback,
            allow_loopback: explicitly_loopback,
            fake_ip_proxy_domain_allowlist: if official_endpoint {
                vec![host.to_string()]
            } else {
                Vec::new()
            },
            max_redirects: 0,
            max_body_bytes: PROVIDER_MAX_RESPONSE_BYTES,
            connect_timeout: Duration::from_secs(STREAM_CONNECT_TIMEOUT_SECS),
            request_timeout: Duration::from_secs(CHAT_REQUEST_TIMEOUT_SECS),
            ..Default::default()
        },
    ))
}

fn provider_endpoint_allows_system_fake_ip_proxy(provider: &str, endpoint: &reqwest::Url) -> bool {
    endpoint.scheme() == "https"
        && reqwest::Url::parse(&chat_completions_url(
            provider,
            default_base_for_provider(provider),
        ))
        .ok()
        .is_some_and(|expected| expected == *endpoint)
}

fn provider_http_error(label: &str, status: reqwest::StatusCode, body: &str) -> anyhow::Error {
    confirmed_provider_terminal_failure(
        "provider_http_terminal_failed",
        anyhow::anyhow!(
            "{} provider returned HTTP {} (body_digest={})",
            label,
            status,
            response_body_digest(body)
        ),
    )
}

/// Transient, adapter-internal input for an OpenAI-compatible HTTP request.
///
/// This deliberately is not serializable or debuggable: it temporarily owns
/// provider content and borrows credentials, while `PreparedProviderRequest`
/// remains the only privacy-safe request contract outside the adapter edge.
pub(crate) struct OpenAiCompatibleAdapterRequest<'a> {
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) system_prompt: Option<&'a str>,
    pub(crate) provider: &'a str,
    /// Exact final URL fixed by `PreparedProviderRequest`; the adapter must not
    /// reconstruct it from mutable scheduler configuration.
    pub(crate) endpoint: &'a str,
    pub(crate) api_key: &'a str,
    pub(crate) model: &'a str,
    /// Derived from the policy-bound provider payload purpose. The adapter may
    /// use a provider-native JSON mode, but callers cannot infer this from
    /// prompt text or relax downstream schema validation.
    pub(crate) structured_json_output: bool,
    pub(crate) network_policy: &'a NetworkPolicy,
    pub(crate) network_policy_decision: &'a NetworkPolicyDecision,
    pub(crate) request_id: Option<&'a str>,
}

pub(crate) async fn chat_with_openrouter_raw_with_start_observer<F>(
    request: OpenAiCompatibleAdapterRequest<'_>,
    on_started: F,
) -> Result<String>
where
    F: FnOnce() -> Result<()>,
{
    let OpenAiCompatibleAdapterRequest {
        messages,
        system_prompt,
        provider,
        endpoint,
        api_key: configured_api_key,
        model,
        structured_json_output,
        network_policy,
        network_policy_decision,
        request_id,
    } = request;
    // The scheduler already resolved and sealed the endpoint-scoped
    // credential. Re-resolving an environment variable here would lose the
    // endpoint identity and could send an official credential to a proxy.
    let api_key = configured_api_key.to_string();
    let label = provider_label(provider);

    if api_key.is_empty() {
        return Err(anyhow::anyhow!(
            "{} provider credentials are not configured",
            label
        ));
    }

    let mut req_messages: Vec<serde_json::Value> = Vec::new();
    if let Some(sp) = system_prompt {
        req_messages.push(json!({
            "role": "system",
            "content": sp
        }));
    }

    for msg in messages {
        req_messages.push(json!({
            "role": msg.role,
            "content": msg.content
        }));
    }

    let mut body = json!({
        "model": model,
        "messages": req_messages,
        "temperature": 0.7,
        "max_tokens": 2048,
    });
    if structured_json_output && provider == "deepseek" {
        body["response_format"] = json!({ "type": "json_object" });
    }

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse()?);
    headers.insert(AUTHORIZATION, format!("Bearer {}", api_key).parse()?);
    if let Some(request_id) = request_id {
        headers.insert(
            HeaderName::from_static("x-openlife-request-id"),
            HeaderValue::from_str(request_id)?,
        );
    }

    let url = endpoint.to_string();

    let mut on_started = Some(on_started);
    // `Attempting` is the local adapter-start edge: URL/policy/DNS and request
    // construction have succeeded and the non-idempotent HTTP send is about to
    // begin. It does not claim that the remote provider accepted the request;
    // cancellation before a terminal observation must therefore remain
    // `remote_unknown`.
    let res = provider_network_client(provider, &url)?
        .post_json_text_with_decision_and_start_observer(
            &url,
            network_policy,
            network_policy_decision,
            headers,
            &body,
            move |phase| {
                let result =
                    if phase == crate::network_client::NetworkDispatchAttemptPhase::Attempting {
                        on_started.take().map_or(Ok(()), |observer| observer())
                    } else {
                        Ok(())
                    };
                std::future::ready(result)
            },
        )
        .await
        .map_err(|error| anyhow::anyhow!("{} 请求失败: {}", label, error))?;

    let status = res.status;
    let text = res.body;

    if !status.is_success() {
        return Err(provider_http_error(&label, status, &text));
    }

    let json: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        confirmed_provider_terminal_failure(
            "provider_response_json_invalid",
            anyhow::Error::new(error).context(format!(
                "解析响应 JSON 失败 (body_digest={})",
                response_body_digest(&text)
            )),
        )
    })?;

    let content = match extract_chat_content(&json) {
        Some(content) => content,
        None if has_reasoning_content(&json) => {
            return Err(confirmed_provider_terminal_failure(
                "provider_reasoning_without_final_content",
                anyhow::anyhow!("provider_reasoning_without_final_content: {}", label),
            ))
        }
        None => {
            return Err(confirmed_provider_terminal_failure(
                "provider_final_content_missing",
                anyhow::anyhow!(
                    "{} 响应为空或格式不兼容 (body_digest={})",
                    label,
                    response_body_digest(&text)
                ),
            ))
        }
    };

    Ok(content)
}

pub(crate) async fn chat_with_openrouter_raw_stream_with_start_observer<F>(
    request: OpenAiCompatibleAdapterRequest<'_>,
    on_started: F,
) -> Result<StreamResult>
where
    F: FnOnce() -> Result<()>,
{
    let OpenAiCompatibleAdapterRequest {
        messages,
        system_prompt,
        provider,
        endpoint,
        api_key: configured_api_key,
        model,
        structured_json_output: _,
        network_policy,
        network_policy_decision,
        request_id,
    } = request;
    let api_key = configured_api_key.to_string();
    let label = provider_label(provider);
    if api_key.is_empty() {
        return Err(anyhow::anyhow!(
            "请设置 {} API Key，或在设置页填写 API Key 以使用对话功能。",
            label
        ));
    }

    let mut req_messages: Vec<serde_json::Value> = Vec::new();
    if let Some(sp) = system_prompt {
        req_messages.push(json!({
            "role": "system",
            "content": sp
        }));
    }

    for msg in messages {
        req_messages.push(json!({
            "role": msg.role,
            "content": msg.content
        }));
    }

    let body = json!({
        "model": model,
        "messages": req_messages,
        "temperature": 0.7,
        "max_tokens": 2048,
        "stream": true,
    });

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse()?);
    headers.insert(AUTHORIZATION, format!("Bearer {}", api_key).parse()?);
    if let Some(request_id) = request_id {
        headers.insert(
            HeaderName::from_static("x-openlife-request-id"),
            HeaderValue::from_str(request_id)?,
        );
    }

    let url = endpoint.to_string();

    let mut on_started = Some(on_started);
    // Keep streaming and buffered provider truth on the same local adapter
    // edge. Waiting for response headers would lose an in-flight request when
    // cancellation drops the HTTP future after the body may have left.
    let res = provider_network_client(provider, &url)?
        .post_json_stream_with_decision_and_start_observer(
            &url,
            network_policy,
            network_policy_decision,
            headers,
            &body,
            move |phase| {
                let result =
                    if phase == crate::network_client::NetworkDispatchAttemptPhase::Attempting {
                        on_started.take().map_or(Ok(()), |observer| observer())
                    } else {
                        Ok(())
                    };
                std::future::ready(result)
            },
        )
        .await
        .map_err(|error| anyhow::anyhow!("{} 请求失败: {}", label, error))?;

    let status = res.status;
    let mut byte_stream = res.body;
    if !status.is_success() {
        let mut bytes = Vec::new();
        while let Some(chunk) = byte_stream.next().await {
            let chunk = chunk.map_err(|error| {
                confirmed_provider_terminal_failure("provider_http_terminal_failed", error)
            })?;
            bytes.extend_from_slice(&chunk);
        }
        let text = String::from_utf8_lossy(&bytes);
        return Err(provider_http_error(&label, status, &text));
    }

    let stream = try_stream! {
        let mut buffer = Vec::<u8>::new();
        let mut observed_reasoning = false;
        let mut emitted_final_content = false;
        loop {
            let next = tokio::time::timeout(
                Duration::from_secs(STREAM_IDLE_TIMEOUT_SECS),
                byte_stream.next(),
            )
            .await
            .map_err(|_| anyhow::anyhow!("provider stream idle timeout"))?;
            let Some(chunk) = next else { break; };
            let bytes = chunk.with_context(|| "stream read error")?;
            buffer.extend_from_slice(&bytes);
            while let Some(pos) = buffer.iter().position(|byte| *byte == b'\n') {
                let mut line_bytes = buffer.drain(..=pos).collect::<Vec<_>>();
                line_bytes.pop();
                if line_bytes.last() == Some(&b'\r') {
                    line_bytes.pop();
                }
                if line_bytes.len() > PROVIDER_MAX_SSE_FRAME_BYTES {
                    Err(anyhow::anyhow!("provider stream frame too large"))?;
                }
                let line = std::str::from_utf8(&line_bytes)
                    .context("provider stream frame is not valid UTF-8")?
                    .trim();
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        if emitted_final_content {
                            return;
                        }
                        if observed_reasoning {
                            Err(confirmed_provider_terminal_failure(
                                "provider_reasoning_without_final_content",
                                anyhow::anyhow!("provider_reasoning_without_final_content"),
                            ))?;
                        }
                        Err(confirmed_provider_terminal_failure(
                            "provider_final_content_missing",
                            anyhow::anyhow!("provider_final_content_missing"),
                        ))?;
                    }
                    let json = serde_json::from_str::<serde_json::Value>(data)
                        .context("provider stream frame is not valid JSON")?;
                    if has_provider_stream_error(&json) {
                        Err(confirmed_provider_terminal_failure(
                            "provider_stream_reported_error",
                            anyhow::anyhow!(
                                "provider_stream_error: body_digest={}",
                                response_body_digest(data)
                            ),
                        ))?;
                    } else if let Some(content) = extract_stream_content(&json) {
                        emitted_final_content = true;
                        yield content;
                    } else if has_reasoning_content(&json) {
                        observed_reasoning = true;
                    }
                }
            }
            if buffer.len() > PROVIDER_MAX_SSE_FRAME_BYTES {
                Err(anyhow::anyhow!("provider stream frame too large"))?;
            }
        }
        if buffer.len() > PROVIDER_MAX_SSE_FRAME_BYTES {
            Err(anyhow::anyhow!("provider stream frame too large"))?;
        }
        let remainder = std::str::from_utf8(&buffer)
            .context("provider stream frame is not valid UTF-8")?
            .trim();
        if let Some(data) = remainder.strip_prefix("data:") {
            let data = data.trim();
            if data != "[DONE]" {
                let json = serde_json::from_str::<serde_json::Value>(data)
                    .context("provider stream frame is not valid JSON")?;
                if has_provider_stream_error(&json) {
                    Err(confirmed_provider_terminal_failure(
                        "provider_stream_reported_error",
                        anyhow::anyhow!(
                            "provider_stream_error: body_digest={}",
                            response_body_digest(data)
                        ),
                    ))?;
                } else if let Some(content) = extract_stream_content(&json) {
                    yield content;
                } else if has_reasoning_content(&json) {
                    Err(confirmed_provider_terminal_failure(
                        "provider_reasoning_without_final_content",
                        anyhow::anyhow!("provider_reasoning_without_final_content"),
                    ))?;
                }
            } else if emitted_final_content {
                return;
            } else if observed_reasoning {
                Err(confirmed_provider_terminal_failure(
                    "provider_reasoning_without_final_content",
                    anyhow::anyhow!("provider_reasoning_without_final_content"),
                ))?;
            } else {
                Err(confirmed_provider_terminal_failure(
                    "provider_final_content_missing",
                    anyhow::anyhow!("provider_final_content_missing"),
                ))?;
            }
        }
        Err(anyhow::anyhow!("provider stream ended before the terminal marker"))?;
    };

    Ok(Box::pin(stream))
}

#[cfg(test)]
mod tests {
    use super::{
        chat_completions_url, default_base_for_provider, effective_api_key_for_endpoint,
        extract_chat_content, extract_stream_content, has_reasoning_content,
        provider_credential_identity, provider_endpoint_allows_system_fake_ip_proxy,
        provider_label, resolve_provider_chat_model,
    };
    use futures::StreamExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    async fn serve_provider_response(
        listener: tokio::net::TcpListener,
        status: &str,
        body: Vec<u8>,
        declared_length: Option<usize>,
    ) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0_u8; 32 * 1024];
        let read = socket.read(&mut request).await.unwrap();
        let request = String::from_utf8_lossy(&request[..read]).into_owned();
        let header = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            declared_length.unwrap_or(body.len())
        );
        socket.write_all(header.as_bytes()).await.unwrap();
        if !body.is_empty() {
            socket.write_all(&body).await.unwrap();
        }
        request
    }

    async fn serve_split_provider_response(
        listener: tokio::net::TcpListener,
        body: Vec<u8>,
        split_at: usize,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = vec![0_u8; 32 * 1024];
        let _ = socket.read(&mut request).await.unwrap();
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        socket.write_all(header.as_bytes()).await.unwrap();
        socket.write_all(&body[..split_at]).await.unwrap();
        socket.flush().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        socket.write_all(&body[split_at..]).await.unwrap();
    }

    async fn local_provider_base() -> (tokio::net::TcpListener, String) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        (listener, base)
    }

    fn allow_provider_network(
        provider: &str,
        base: &str,
    ) -> (
        crate::config::NetworkPolicy,
        crate::network_client::NetworkPolicyDecision,
    ) {
        let policy = crate::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..crate::config::NetworkPolicy::default()
        };
        let decision = crate::network_client::resolve_network_policy_decision(
            &policy,
            &super::chat_completions_url(provider, base),
            &format!("provider.{provider}"),
        )
        .unwrap();
        (policy, decision)
    }

    async fn test_chat_with_openrouter_raw(
        messages: Vec<super::ChatMessage>,
        system_prompt: Option<&str>,
        provider: &str,
        base: &str,
        key: &str,
        model: &str,
    ) -> anyhow::Result<String> {
        let (policy, decision) = allow_provider_network(provider, base);
        let endpoint = super::chat_completions_url(provider, base);
        super::chat_with_openrouter_raw_with_start_observer(
            super::OpenAiCompatibleAdapterRequest {
                messages,
                system_prompt,
                provider,
                endpoint: &endpoint,
                api_key: key,
                model,
                structured_json_output: false,
                network_policy: &policy,
                network_policy_decision: &decision,
                request_id: None,
            },
            || Ok(()),
        )
        .await
    }

    async fn test_chat_with_openrouter_raw_stream(
        messages: Vec<super::ChatMessage>,
        system_prompt: Option<&str>,
        provider: &str,
        base: &str,
        key: &str,
        model: &str,
    ) -> anyhow::Result<super::StreamResult> {
        let (policy, decision) = allow_provider_network(provider, base);
        let endpoint = super::chat_completions_url(provider, base);
        super::chat_with_openrouter_raw_stream_with_start_observer(
            super::OpenAiCompatibleAdapterRequest {
                messages,
                system_prompt,
                provider,
                endpoint: &endpoint,
                api_key: key,
                model,
                structured_json_output: false,
                network_policy: &policy,
                network_policy_decision: &decision,
                request_id: None,
            },
            || Ok(()),
        )
        .await
    }

    #[test]
    fn deepseek_provider_uses_expected_label_and_base() {
        assert_eq!(provider_label("deepseek"), "DeepSeek");
        assert_eq!(
            default_base_for_provider("deepseek"),
            "https://api.deepseek.com"
        );
    }

    #[tokio::test]
    async fn deepseek_structured_artifact_request_uses_official_json_output_mode() {
        let (listener, base) = local_provider_base().await;
        let server = tokio::spawn(serve_provider_response(
            listener,
            "200 OK",
            br#"{"choices":[{"message":{"content":"{\"markdown\":\"ok\"}"}}]}"#.to_vec(),
            None,
        ));
        let (policy, decision) = allow_provider_network("deepseek", &base);
        let endpoint = super::chat_completions_url("deepseek", &base);

        let result = super::chat_with_openrouter_raw_with_start_observer(
            super::OpenAiCompatibleAdapterRequest {
                messages: vec![super::ChatMessage {
                    role: "user".into(),
                    content: "Return JSON.".into(),
                }],
                system_prompt: Some("Return only one JSON object."),
                provider: "deepseek",
                endpoint: &endpoint,
                api_key: "sk-test",
                model: "deepseek-v4-flash",
                structured_json_output: true,
                network_policy: &policy,
                network_policy_decision: &decision,
                request_id: None,
            },
            || Ok(()),
        )
        .await
        .expect("structured provider response");
        assert_eq!(result, r#"{"markdown":"ok"}"#);

        let request = server.await.unwrap();
        let body = request.split("\r\n\r\n").nth(1).expect("HTTP request body");
        let body: serde_json::Value = serde_json::from_str(body).expect("JSON request body");
        assert_eq!(body["response_format"]["type"], "json_object");
    }

    #[test]
    fn system_fake_ip_proxy_is_limited_to_official_https_provider_endpoints() {
        let official_deepseek = reqwest::Url::parse(&chat_completions_url(
            "deepseek",
            default_base_for_provider("deepseek"),
        ))
        .unwrap();
        assert!(provider_endpoint_allows_system_fake_ip_proxy(
            "deepseek",
            &official_deepseek
        ));

        let custom = reqwest::Url::parse("https://provider.example/v1/chat/completions").unwrap();
        assert!(!provider_endpoint_allows_system_fake_ip_proxy(
            "custom", &custom
        ));
        assert!(!provider_endpoint_allows_system_fake_ip_proxy(
            "deepseek", &custom
        ));

        let plaintext = reqwest::Url::parse("http://api.deepseek.com/chat/completions").unwrap();
        assert!(!provider_endpoint_allows_system_fake_ip_proxy(
            "deepseek", &plaintext
        ));
    }

    #[test]
    fn provider_specific_env_fallbacks_are_used_when_config_key_is_empty() {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("DEEPSEEK_API_KEY", "sk-deepseek-test");
        std::env::set_var("OPENROUTER_API_KEY", "sk-openrouter-test");
        std::env::set_var("OPENAI_API_KEY", "sk-openai-test");

        assert_eq!(
            effective_api_key_for_endpoint("deepseek", "https://api.deepseek.com", ""),
            "sk-deepseek-test"
        );
        assert_eq!(
            effective_api_key_for_endpoint("openrouter", "https://openrouter.ai/api/v1", "",),
            "sk-openrouter-test"
        );
        assert_eq!(
            effective_api_key_for_endpoint("openai", "https://api.openai.com/v1", ""),
            "sk-openai-test"
        );
        assert_eq!(
            effective_api_key_for_endpoint("openai", "https://custom.example/v1", "",),
            ""
        );
        assert_eq!(
            effective_api_key_for_endpoint("deepseek", "https://custom.example/v1", "sk-config",),
            "sk-config"
        );
        assert_eq!(
            effective_api_key_for_endpoint("custom", "https://custom.example/v1", ""),
            ""
        );

        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("OPENROUTER_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn chat_url_accepts_base_or_full_endpoint() {
        assert_eq!(
            chat_completions_url("deepseek", "https://api.deepseek.com"),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            chat_completions_url("openai", "https://api.openai.com/v1"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("custom", "http://localhost:1234/v1/chat/completions"),
            "http://localhost:1234/v1/chat/completions"
        );
    }

    #[tokio::test]
    async fn missing_api_key_is_a_provider_failure_not_assistant_content() {
        let result = test_chat_with_openrouter_raw(
            Vec::new(),
            None,
            "custom-provider-without-env-fallback",
            "https://example.invalid/v1",
            "",
            "test-model",
        )
        .await;

        assert!(result.is_err(), "missing credentials must fail closed");
    }

    #[tokio::test]
    async fn non_stream_provider_rejects_oversized_body_at_the_network_reader() {
        let (listener, base) = local_provider_base().await;
        let server = tokio::spawn(serve_provider_response(
            listener,
            "200 OK",
            Vec::new(),
            Some(super::PROVIDER_MAX_RESPONSE_BYTES + 1),
        ));

        let error =
            test_chat_with_openrouter_raw(vec![], None, "openai", &base, "sk-test", "gpt-test")
                .await
                .unwrap_err();

        assert!(error
            .to_string()
            .contains("network_response_body_too_large"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn provider_http_error_does_not_copy_the_remote_body() {
        let (listener, base) = local_provider_base().await;
        let body = b"TOP_SECRET_PROVIDER_ERROR_BODY".to_vec();
        let server = tokio::spawn(serve_provider_response(
            listener,
            "500 Internal Server Error",
            body,
            None,
        ));

        let error =
            test_chat_with_openrouter_raw(vec![], None, "openai", &base, "sk-test", "gpt-test")
                .await
                .unwrap_err();
        let message = format!("{error:#}");

        assert!(message.contains("HTTP 500"), "{message}");
        assert!(message.contains("body_digest=sha256:"));
        assert!(!message.contains("TOP_SECRET_PROVIDER_ERROR_BODY"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn bounded_loopback_provider_keeps_non_stream_generation_capability() {
        let (listener, base) = local_provider_base().await;
        let body = br#"{"choices":[{"message":{"content":"bounded hello"}}]}"#.to_vec();
        let server = tokio::spawn(serve_provider_response(listener, "200 OK", body, None));

        let content =
            test_chat_with_openrouter_raw(vec![], None, "openai", &base, "sk-test", "gpt-test")
                .await
                .unwrap();

        assert_eq!(content, "bounded hello");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn non_stream_reasoning_without_final_content_is_a_provider_failure() {
        let (listener, base) = local_provider_base().await;
        let body =
            br#"{"choices":[{"message":{"reasoning_content":"private chain","content":""}}]}"#
                .to_vec();
        let server = tokio::spawn(serve_provider_response(listener, "200 OK", body, None));

        let error =
            test_chat_with_openrouter_raw(vec![], None, "openai", &base, "sk-test", "gpt-test")
                .await
                .expect_err("reasoning-only payload must not become assistant success text");

        assert!(error
            .to_string()
            .contains("provider_reasoning_without_final_content"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn public_provider_plaintext_http_fails_before_dispatch() {
        let started = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&started);

        let base = "http://example.com/v1";
        let (policy, decision) = allow_provider_network("openai", base);
        let endpoint = super::chat_completions_url("openai", base);
        let error = super::chat_with_openrouter_raw_with_start_observer(
            super::OpenAiCompatibleAdapterRequest {
                messages: vec![],
                system_prompt: None,
                provider: "openai",
                endpoint: &endpoint,
                api_key: "sk-test",
                model: "gpt-test",
                structured_json_output: false,
                network_policy: &policy,
                network_policy_decision: &decision,
                request_id: None,
            },
            move || {
                observed.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("network_url_scheme_blocked"));
        assert!(!started.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn provider_stream_rejects_an_oversized_sse_frame() {
        let (listener, base) = local_provider_base().await;
        let body = format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":\"{}\"}}}}]}}\n\n",
            "x".repeat(super::PROVIDER_MAX_SSE_FRAME_BYTES)
        )
        .into_bytes();
        let server = tokio::spawn(serve_provider_response(listener, "200 OK", body, None));

        let mut stream = test_chat_with_openrouter_raw_stream(
            vec![],
            None,
            "openai",
            &base,
            "sk-test",
            "gpt-test",
        )
        .await
        .unwrap();
        let error = stream.next().await.unwrap().unwrap_err();

        assert!(error.to_string().contains("frame too large"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn provider_stream_reports_missing_terminal_marker_after_observed_content() {
        let (listener, base) = local_provider_base().await;
        let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n".to_vec();
        let server = tokio::spawn(serve_provider_response(listener, "200 OK", body, None));

        let mut stream = test_chat_with_openrouter_raw_stream(
            vec![],
            None,
            "openai",
            &base,
            "sk-test",
            "gpt-test",
        )
        .await
        .unwrap();

        assert_eq!(stream.next().await.unwrap().unwrap(), "hello");
        let error = stream.next().await.unwrap().unwrap_err();
        assert!(error.to_string().contains("terminal marker"));
        assert_eq!(
            super::provider_error_terminal_status(&error),
            super::ProviderInvocationStatus::RemoteUnknown,
            "EOF without a provider terminal cannot be upgraded to confirmed failure"
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn bounded_loopback_provider_keeps_terminal_stream_capability() {
        let (listener, base) = local_provider_base().await;
        let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: [DONE]\n\n"
            .to_vec();
        let server = tokio::spawn(serve_provider_response(listener, "200 OK", body, None));

        let mut stream = test_chat_with_openrouter_raw_stream(
            vec![],
            None,
            "openai",
            &base,
            "sk-test",
            "gpt-test",
        )
        .await
        .unwrap();

        assert_eq!(stream.next().await.unwrap().unwrap(), "hello");
        assert!(stream.next().await.is_none());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn provider_stream_preserves_utf8_split_inside_a_multibyte_character() {
        let (listener, base) = local_provider_base().await;
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\ndata: [DONE]\n\n"
            .as_bytes()
            .to_vec();
        let first_chinese_byte = body
            .windows("你".len())
            .position(|window| window == "你".as_bytes())
            .expect("Chinese content byte offset");
        let server = tokio::spawn(serve_split_provider_response(
            listener,
            body,
            first_chinese_byte + 1,
        ));

        let mut stream = test_chat_with_openrouter_raw_stream(
            vec![],
            None,
            "openai",
            &base,
            "sk-test",
            "gpt-test",
        )
        .await
        .unwrap();
        let mut content = String::new();
        while let Some(item) = stream.next().await {
            content.push_str(&item.unwrap());
        }

        assert_eq!(content, "你好");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn provider_stream_reasoning_without_final_content_fails_at_terminal_marker() {
        let (listener, base) = local_provider_base().await;
        let body = b"data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"private chain\"}}]}\n\ndata: [DONE]\n\n".to_vec();
        let server = tokio::spawn(serve_provider_response(listener, "200 OK", body, None));

        let mut stream = test_chat_with_openrouter_raw_stream(
            vec![],
            None,
            "openai",
            &base,
            "sk-test",
            "gpt-test",
        )
        .await
        .unwrap();
        let error = stream
            .next()
            .await
            .expect("reasoning-only stream terminal failure")
            .expect_err("reasoning-only stream must not emit assistant notice text");

        assert!(error
            .to_string()
            .contains("provider_reasoning_without_final_content"));
        assert_eq!(
            super::provider_error_terminal_status(&error),
            super::ProviderInvocationStatus::Failed,
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn provider_stream_error_frame_after_partial_content_is_a_failure() {
        let (listener, base) = local_provider_base().await;
        let body = b"data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\ndata: {\"error\":{\"message\":\"remote secret detail\",\"type\":\"provider_error\"}}\n\ndata: [DONE]\n\n".to_vec();
        let server = tokio::spawn(serve_provider_response(listener, "200 OK", body, None));

        let mut stream = test_chat_with_openrouter_raw_stream(
            vec![],
            None,
            "openai",
            &base,
            "sk-test",
            "gpt-test",
        )
        .await
        .unwrap();

        assert_eq!(stream.next().await.unwrap().unwrap(), "partial");
        let error = stream
            .next()
            .await
            .expect("structured provider error must terminate the stream")
            .expect_err("structured provider error cannot become completed output");
        let message = format!("{error:#}");
        assert!(message.contains("provider_stream_error"), "{message}");
        assert!(message.contains("body_digest=sha256:"));
        assert!(!message.contains("remote secret detail"));
        assert_eq!(
            super::provider_error_terminal_status(&error),
            super::ProviderInvocationStatus::Failed,
        );
        server.await.unwrap();
    }

    #[test]
    fn extracts_content_from_common_openai_compatible_shapes() {
        let normal = serde_json::json!({
            "choices": [{"message": {"content": "hello"}}]
        });
        let text = serde_json::json!({
            "choices": [{"text": "hello text"}]
        });
        let stream = serde_json::json!({
            "choices": [{"delta": {"content": "hi"}}]
        });
        let stream_alt = serde_json::json!({
            "delta": {"content": "alt"}
        });
        let reasoning = serde_json::json!({
            "choices": [{"delta": {"reasoning_content": "thinking"}}]
        });
        let reasoning_message = serde_json::json!({
            "choices": [{"message": {"reasoning_content": "thinking", "content": ""}}]
        });
        assert_eq!(extract_chat_content(&normal).as_deref(), Some("hello"));
        assert_eq!(extract_chat_content(&text).as_deref(), Some("hello text"));
        assert_eq!(extract_stream_content(&stream).as_deref(), Some("hi"));
        assert_eq!(extract_stream_content(&stream_alt).as_deref(), Some("alt"));
        assert!(has_reasoning_content(&reasoning));
        assert!(has_reasoning_content(&reasoning_message));
        assert_eq!(extract_stream_content(&reasoning), None);
    }

    #[test]
    fn provider_model_resolution_is_identical_for_buffered_and_streaming_adapters() {
        assert_eq!(
            resolve_provider_chat_model("deepseek", "deepseek-reasoner"),
            "deepseek-reasoner"
        );
        assert_eq!(
            resolve_provider_chat_model("deepseek", "deepseek-chat"),
            "deepseek-chat"
        );
        assert_eq!(
            resolve_provider_chat_model("openai", "  gpt-4o-mini  "),
            "gpt-4o-mini"
        );
    }

    #[tokio::test]
    async fn deepseek_reasoner_wire_model_is_identical_for_buffered_and_streaming() {
        let (buffered_listener, buffered_base) = local_provider_base().await;
        let buffered_server = tokio::spawn(serve_provider_response(
            buffered_listener,
            "200 OK",
            br#"{"choices":[{"message":{"content":"buffered"}}]}"#.to_vec(),
            None,
        ));
        let buffered = test_chat_with_openrouter_raw(
            vec![],
            None,
            "deepseek",
            &buffered_base,
            "sk-test",
            "deepseek-reasoner",
        )
        .await
        .unwrap();
        let buffered_request = buffered_server.await.unwrap();

        let (stream_listener, stream_base) = local_provider_base().await;
        let stream_server = tokio::spawn(serve_provider_response(
            stream_listener,
            "200 OK",
            b"data: {\"choices\":[{\"delta\":{\"content\":\"stream\"}}]}\n\ndata: [DONE]\n\n"
                .to_vec(),
            None,
        ));
        let mut stream = test_chat_with_openrouter_raw_stream(
            vec![],
            None,
            "deepseek",
            &stream_base,
            "sk-test",
            "deepseek-reasoner",
        )
        .await
        .unwrap();
        assert_eq!(stream.next().await.unwrap().unwrap(), "stream");
        assert!(stream.next().await.is_none());
        let stream_request = stream_server.await.unwrap();

        assert_eq!(buffered, "buffered");
        for request in [buffered_request, stream_request] {
            assert!(request.contains("\"model\":\"deepseek-reasoner\""));
            assert!(!request.contains("\"model\":\"deepseek-chat\""));
            assert!(!request.contains("\"response_format\""));
        }
    }

    #[test]
    fn provider_credential_identity_is_safe_and_key_specific() {
        let first = provider_credential_identity("sk-first");
        let second = provider_credential_identity("sk-second");
        assert!(first.starts_with("sha256:"));
        assert_eq!(first.len(), "sha256:".len() + 64);
        assert_ne!(first, second);
        assert!(!first.contains("sk-first"));
    }
}
