//! Provider request contract used by canonical Chat and Work.
//!
//! The provider client is an execution adapter. It does not decide user intent,
//! select tools, own Task lifecycle, or authorize durable effects.

use async_trait::async_trait;
use openlife_core::conversation::ConversationUserMessageProof;
use openlife_core::llm::{
    BoundedContextBlock, ChatMessage, ProviderDataRoute, ProviderInvocationReceipt,
    ProviderPayloadPurpose, ProviderPolicyAuthorization, ProviderPolicyReceiptEvidence,
    ProviderToolDefinition,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAuthorization {
    pub data_route: ProviderDataRoute,
    pub privacy_decision_id: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(skip)]
    pub(crate) policy_authorization: ProviderPolicyAuthorization,
}

impl ProviderAuthorization {
    pub(crate) fn from_conversation_user_message(
        proof: &ConversationUserMessageProof,
        user_text: &str,
    ) -> Result<Self, String> {
        let policy_authorization =
            ProviderPolicyAuthorization::from_conversation_user_message(proof, user_text)
                .map_err(|error| error.to_string())?;
        Ok(Self {
            data_route: policy_authorization.data_route(),
            privacy_decision_id: policy_authorization.decision_id().to_string(),
            task_id: None,
            policy_authorization,
        })
    }

    pub(crate) fn validate_projection(&self) -> bool {
        self.data_route == self.policy_authorization.data_route()
            && self.privacy_decision_id == self.policy_authorization.decision_id()
    }
}

#[derive(Debug, Clone)]
pub struct ProviderModelRequest {
    pub session_id: String,
    /// Stable canonical Turn/Run identity used to bind citations across the
    /// multiple provider calls that may draft and verify one result.
    pub citation_scope_id: String,
    pub messages: Vec<ChatMessage>,
    pub provider_authorization: ProviderAuthorization,
    pub system_prompt: String,
    pub supplemental_context_blocks: Vec<BoundedContextBlock>,
    pub context_snapshot_ref: String,
    pub raw_life_model_included: bool,
    pub raw_unbounded_memory_included: bool,
    pub payload_purpose: ProviderPayloadPurpose,
    pub provider_tools: Vec<ProviderToolDefinition>,
    pub stream_provider_tokens: bool,
    pub additional_resource_context_allowed: bool,
    /// Exact canonical resource selection previously observed by a governed
    /// `document.read`. The provider request must reproduce this selection
    /// with citations bound to `citation_scope_id` before any payload is sent.
    pub required_resource_selection_digest: Option<String>,
}

#[derive(Debug)]
pub enum ProviderModelProgress {
    Started {
        request_id: String,
        provider: String,
        model: String,
        started_at: chrono::DateTime<chrono::Utc>,
        policy_evidence: Box<ProviderPolicyReceiptEvidence>,
    },
    Token {
        request_id: String,
        chunk: String,
    },
}

#[derive(Debug)]
pub struct ProviderModelGeneration {
    pub content: String,
    pub provider_receipt: Option<ProviderInvocationReceipt>,
    /// Request-scoped selected-file source authority. The provider adapter
    /// transports it but does not render or independently repair citations;
    /// the canonical Agent runtime validates it together with every other
    /// source class in the terminal AgentStep.
    pub resource_citations: Option<openlife_core::resource_selection::ResourceCitationSet>,
}

#[derive(Debug)]
pub struct ProviderModelFailure {
    pub message: String,
    pub provider_receipt: Option<ProviderInvocationReceipt>,
    pub blocker_code: Option<String>,
    pub proposal_ids: Vec<String>,
}

impl ProviderModelFailure {
    pub(crate) fn blocker_or(&self, fallback: &str) -> String {
        let blocker = self
            .blocker_code
            .clone()
            .unwrap_or_else(|| fallback.to_string());
        let (_, error_digest) = openlife_core::agent::metadata_safe_text_digest(&self.message);
        log::warn!("Provider generation blocked: blocker={blocker} error_digest={error_digest}");
        blocker
    }
}

#[async_trait]
pub trait ProviderModelClient: Send + Sync {
    async fn generate_direct_answer(
        &self,
        request: ProviderModelRequest,
        emit_progress: &mut (dyn FnMut(ProviderModelProgress) -> anyhow::Result<()> + Send),
    ) -> Result<ProviderModelGeneration, ProviderModelFailure>;
}

pub(crate) fn emit_provider_progress<S>(
    progress: ProviderModelProgress,
    session_id: &str,
    event_sink: &mut S,
) -> anyhow::Result<()>
where
    S: crate::runtime_events::RuntimeEventSink + ?Sized,
{
    use crate::runtime_events::RuntimeEvent;
    match progress {
        ProviderModelProgress::Started {
            request_id,
            provider,
            model,
            started_at,
            policy_evidence,
        } => event_sink
            .emit_provider_started(request_id, provider, model, started_at, *policy_evidence)
            .map_err(anyhow::Error::new),
        ProviderModelProgress::Token { request_id, chunk } => {
            event_sink.emit(RuntimeEvent::ProviderToken {
                session_id: session_id.to_string(),
                request_id,
                chunk,
            });
            Ok(())
        }
    }
}
