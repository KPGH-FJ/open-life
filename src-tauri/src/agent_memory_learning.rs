//! Bounded background Agent Memory learning for an idle Conversation.
//!
//! This module deliberately has no direct Memory write path. It may use the
//! exact provider authorization from one completed Turn to decide whether one
//! source-bound, low-risk fact is worth suggesting. A retained candidate is
//! submitted to the existing ReviewWorkflow; only a later explicit user
//! decision can materialize it.

use crate::provider_client::OpenLifeProviderClient;
use crate::provider_runtime::{
    ProviderAuthorization as MainChatProviderAuthorization,
    ProviderModelClient as MainChatModelClient, ProviderModelRequest as MainChatModelRequest,
};
use crate::state::AppState;
use openlife_core::agent::{
    AgentProposal, CanonicalMemoryFactDescriptor, DurableWriteRequest, DurableWriteSource,
    DurableWriteSubject, MemoryCandidateKind, MemoryLifecycleRiskLevel, MemoryLifecycleScope,
    MemoryLifecycleSensitivity, ProposalSource, ProposalType, ReviewWorkflow, RiskLevel,
};
use openlife_core::conversation::ConversationUserMessageProof;
use openlife_core::conversation::{ConversationMemoryMode, TurnStatus};
use openlife_core::llm::{ChatMessage, ProviderPayloadPurpose};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const IDLE_DELAY: Duration = Duration::from_millis(1_500);
const MIN_EXTRACTION_CONFIDENCE: f32 = 0.80;

fn active_extractions() -> &'static Mutex<HashSet<String>> {
    static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashSet::new()))
}

struct ExtractionGuard {
    conversation_id: String,
}

impl Drop for ExtractionGuard {
    fn drop(&mut self) {
        active_extractions()
            .lock()
            .expect("Agent Memory extraction registry poisoned")
            .remove(&self.conversation_id);
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtractionDecision {
    keep: bool,
    confidence: f32,
    source_span: Option<String>,
}

/// Schedule one best-effort extraction without delaying the visible reply.
/// Repeated completed Turns supersede an older pending idle check because the
/// worker revalidates that its Turn is still the latest before provider use.
pub(crate) fn schedule_after_idle(
    state: Arc<AppState>,
    conversation_id: String,
    turn_id: String,
    user_text: String,
    user_message_proof: ConversationUserMessageProof,
) {
    {
        let mut active = active_extractions()
            .lock()
            .expect("Agent Memory extraction registry poisoned");
        if !active.insert(conversation_id.clone()) {
            return;
        }
    }
    tauri::async_runtime::spawn(async move {
        let _guard = ExtractionGuard {
            conversation_id: conversation_id.clone(),
        };
        tokio::time::sleep(IDLE_DELAY).await;
        if let Err(error) = run_one(
            &state,
            &conversation_id,
            &turn_id,
            &user_text,
            &user_message_proof,
        )
        .await
        {
            let (_, digest) = openlife_core::agent::metadata_safe_text_digest(&error);
            log::warn!("Agent Memory background extraction skipped: error_digest={digest}");
        }
    });
}

async fn run_one(
    state: &Arc<AppState>,
    conversation_id: &str,
    turn_id: &str,
    user_text: &str,
    user_message_proof: &ConversationUserMessageProof,
) -> Result<Option<String>, String> {
    let (conversation, latest_turn) = {
        let store = state
            .conversation_store
            .as_ref()
            .ok_or_else(|| "conversation_store_unavailable".to_string())?
            .lock()
            .await;
        let conversation = store
            .get_conversation(conversation_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "conversation_not_found".to_string())?;
        let latest_turn = store
            .latest_turn(conversation_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "conversation_latest_turn_missing".to_string())?;
        (conversation, latest_turn)
    };
    if !state.config.lock().await.system.agent_memory_enabled
        || conversation.memory_mode != ConversationMemoryMode::UseAndLearn
        || latest_turn.id != turn_id
        || latest_turn.status != TurnStatus::Completed
    {
        return Ok(None);
    }

    let selected = crate::provider_registry::selected_provider_profile(state).await?;
    if selected.binding != latest_turn.provider {
        return Ok(None);
    }
    let runtime = state.provider_runtime_snapshot().await;
    if !runtime.coherent {
        return Ok(None);
    }
    let authorization = MainChatProviderAuthorization::from_conversation_user_message(
        user_message_proof,
        user_text,
    )?;
    let (_, context_digest) = openlife_core::agent::metadata_safe_text_digest(user_text);
    let messages = vec![ChatMessage {
        role: "user".into(),
        content: user_text.to_string(),
    }];
    let client = OpenLifeProviderClient::new(
        runtime.scheduler,
        state.privacy_engine.lock().await.clone(),
        runtime.config.system.network_policy,
    )
    .with_runtime_state(Arc::clone(state));
    let request = MainChatModelRequest {
        session_id: conversation_id.to_string(),
        citation_scope_id: turn_id.to_string(),
        messages,
        provider_authorization: authorization,
        system_prompt: extraction_prompt(),
        supplemental_context_blocks: Vec::new(),
        context_snapshot_ref: context_digest,
        raw_life_model_included: false,
        raw_unbounded_memory_included: false,
        payload_purpose: ProviderPayloadPurpose::AgentMemoryExtraction,
        provider_tools: Vec::new(),
        stream_provider_tokens: false,
        additional_resource_context_allowed: false,
        required_resource_selection_digest: None,
    };
    let generation = client
        .generate_direct_answer(request, &mut |_| Ok(()))
        .await
        .map_err(|failure| {
            failure
                .blocker_code
                .unwrap_or_else(|| "agent_memory_extraction_provider_failed".into())
        })?;
    let decision: ExtractionDecision = serde_json::from_str(generation.content.trim())
        .map_err(|_| "agent_memory_extraction_json_invalid".to_string())?;
    if !decision.keep
        || !decision.confidence.is_finite()
        || decision.confidence < MIN_EXTRACTION_CONFIDENCE
        || decision.confidence > 1.0
    {
        return Ok(None);
    }
    let source_span = decision
        .source_span
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "agent_memory_extraction_source_span_missing".to_string())?;
    if source_span.chars().count() > 320 || !user_text.contains(source_span) {
        return Err("agent_memory_extraction_source_span_invalid".into());
    }
    if openlife_core::privacy::assess_sensitive_content(source_span).requires_memory_review() {
        return Ok(None);
    }

    let (scope, scope_owner_ref) = match conversation.project_id.as_deref() {
        Some(project_id) => (
            MemoryLifecycleScope::Project,
            Some(
                openlife_core::agent::memory_scope_owner_ref(
                    MemoryLifecycleScope::Project,
                    project_id,
                )
                .map_err(|error| error.to_string())?,
            ),
        ),
        None => (MemoryLifecycleScope::Global, None),
    };
    let mut fact = CanonicalMemoryFactDescriptor::from_candidate(
        source_span.to_string(),
        MemoryCandidateKind::SemanticUserFact,
        scope,
        MemoryLifecycleRiskLevel::Low,
        MemoryLifecycleSensitivity::Internal,
    )
    .map_err(|error| error.to_string())?;
    if let Some(owner) = scope_owner_ref.as_deref() {
        fact = fact
            .with_scope_owner_ref(owner)
            .map_err(|error| error.to_string())?;
    }

    let (supersedes, already_known) = dedupe_or_replacement(state, &fact).await?;
    if already_known {
        return Ok(None);
    }
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ProposalStore"])
        .map_err(|error| error.to_string())?;
    let mut after = serde_json::json!({
        "content": fact.canonical_body,
        "scope": fact.scope,
        "scopeOwnerRef": fact.scope_owner_ref,
        "category": fact.category,
        "riskLevel": fact.risk_level,
        "sensitivity": fact.sensitivity,
        "candidateKind": MemoryCandidateKind::SemanticUserFact,
        "source": "conversation_idle_extraction",
        "sourceTurnId": turn_id,
        "providerProfileId": selected.binding.profile_id,
        "providerModelId": selected.binding.model_id,
    });
    if let Some(memory_id) = supersedes.as_deref() {
        after["supersedesMemoryId"] = serde_json::Value::String(memory_id.to_string());
    }
    let mut proposal = AgentProposal::new(
        ProposalType::MemoryWrite,
        if scope == MemoryLifecycleScope::Project {
            "memory.project"
        } else {
            "memory.personal"
        },
        after,
        "OpenLife noticed one reusable fact after the Conversation became idle. Review is required before it becomes Memory.",
        decision.confidence,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    proposal.source_detail = Some(conversation_id.to_string());
    if let Some(previous) = supersedes.as_ref() {
        proposal.before = Some(serde_json::json!({ "memoryId": previous }));
    }
    let request = DurableWriteRequest::from_agent_proposal(
        DurableWriteSource::MainChat,
        DurableWriteSubject::Memory,
        proposal,
        "发现一条可能帮助后续对话的记忆；需要你确认后才会保存。",
    )
    .with_evidence_refs(vec![format!("conversation_turn:{turn_id}")])
    .with_idempotency_key(format!("agent_memory_extraction:{turn_id}"));
    let registration = state
        .main_chat_runtime_state
        .lock()
        .await
        .cancellation_registry
        .clone()
        .try_register(&format!("agent-memory-extraction:{turn_id}"))
        .map_err(|error| error.to_string())?;
    let epoch = registration.execution_epoch();
    let outcome = {
        let proposal_store = state
            .proposal_store
            .as_ref()
            .ok_or_else(|| "proposal_store_unavailable".to_string())?
            .lock()
            .await;
        ReviewWorkflow::new(&proposal_store)
            .submit_with_admission(request, &epoch)
            .map_err(|error| error.to_string())?
    };
    Ok(Some(outcome.proposal_id().to_string()))
}

async fn dedupe_or_replacement(
    state: &Arc<AppState>,
    fact: &CanonicalMemoryFactDescriptor,
) -> Result<(Option<String>, bool), String> {
    let store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(|| "memory_lifecycle_store_unavailable".to_string())?
        .lock()
        .await;
    if store
        .get_active_record_for_fact(fact)
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Ok((None, true));
    }
    let incoming = comparison_text(&fact.canonical_body);
    for existing in store
        .list_active_records(Some(fact.scope), 200)
        .map_err(|error| error.to_string())?
    {
        if existing.scope_owner_ref != fact.scope_owner_ref || existing.category != fact.category {
            continue;
        }
        let prior = comparison_text(&existing.content);
        if incoming == prior || (!incoming.is_empty() && prior.contains(&incoming)) {
            return Ok((None, true));
        }
        if prior.len() >= 8
            && incoming.contains(&prior)
            && incoming.len() >= prior.len().saturating_add(prior.len() / 5)
        {
            return Ok((Some(existing.memory_id), false));
        }
    }
    Ok((None, false))
}

fn comparison_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn extraction_prompt() -> String {
    "Decide whether the authenticated user's message contains one stable, low-sensitivity fact that will probably help future conversations. Do not infer identity, health, finance, secrets, relationships, values, boundaries, or any fact not written by the user. If one qualifies, copy one exact contiguous source span from the user message. Return only strict JSON with exactly three fields: {\"keep\":true|false,\"confidence\":0.0..1.0,\"source_span\":\"exact user text or empty\"}. Use keep=false and an empty source_span when uncertain.".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::conversation::BeginChatTurn;

    #[test]
    fn extraction_decision_is_strict_and_bounded() {
        let accepted: ExtractionDecision = serde_json::from_str(
            r#"{"keep":true,"confidence":0.91,"source_span":"My timezone is CET."}"#,
        )
        .unwrap();
        assert!(accepted.keep);
        assert!(serde_json::from_str::<ExtractionDecision>(
            r#"{"keep":true,"confidence":0.91,"source_span":"fact","content":"invented"}"#
        )
        .is_err());
    }

    #[test]
    fn comparison_text_supports_bounded_clearer_replacement_matching() {
        assert_eq!(comparison_text("先结论，再说明。"), "先结论再说明");
        assert!(
            comparison_text("请先给结论，再说明必要依据").contains(&comparison_text("先给结论"))
        );
    }

    #[tokio::test]
    async fn idle_extraction_uses_the_turn_provider_and_creates_review_only() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_local_http_provider(
            &state,
            r#"{"keep":true,"confidence":0.91,"source_span":"My work timezone is Central European Time."}"#,
        )
        .await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        let user_text = "My work timezone is Central European Time.";
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Memory extraction")
            .unwrap();
        let provider = crate::provider_registry::selected_provider_profile(&state)
            .await
            .unwrap()
            .binding;
        let begun = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_chat_turn_with_proof(BeginChatTurn {
                turn_id: &turn_id,
                conversation_id: &conversation_id,
                user_message: user_text,
                provider: &provider,
            })
            .unwrap();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .complete_chat_turn(&turn_id, "Understood.")
            .unwrap();

        let proposal_id = run_one(
            &state,
            &conversation_id,
            &turn_id,
            user_text,
            &begun.user_message_proof,
        )
        .await
        .unwrap()
        .expect("eligible idle extraction should create one Review proposal");
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(proposal.proposal_type, ProposalType::MemoryWrite);
        assert_eq!(
            proposal.status,
            openlife_core::agent::ProposalStatus::Pending
        );
        assert_eq!(
            proposal.after["content"],
            "My work timezone is Central European Time."
        );
        assert!(state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_active_records(None, 10)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn use_only_and_off_never_start_idle_memory_learning() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let provider = openlife_core::conversation::ProviderBinding {
            profile_id: "must-not-be-resolved".into(),
            provider_id: "must-not-be-called".into(),
            model_id: "must-not-be-called".into(),
            endpoint_class: "cloud".into(),
            config_generation: "test".into(),
        };
        let user_text = "My work timezone is Central European Time.";

        for mode in [ConversationMemoryMode::UseOnly, ConversationMemoryMode::Off] {
            let conversation_id = uuid::Uuid::new_v4().to_string();
            let turn_id = uuid::Uuid::new_v4().to_string();
            state
                .conversation_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .create_conversation(&conversation_id, "Memory learning disabled")
                .unwrap();
            state
                .conversation_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .set_memory_mode(&conversation_id, mode)
                .unwrap();
            let begun = state
                .conversation_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .begin_chat_turn_with_proof(BeginChatTurn {
                    turn_id: &turn_id,
                    conversation_id: &conversation_id,
                    user_message: user_text,
                    provider: &provider,
                })
                .unwrap();
            state
                .conversation_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .complete_chat_turn(&turn_id, "Understood.")
                .unwrap();

            assert_eq!(
                run_one(
                    &state,
                    &conversation_id,
                    &turn_id,
                    user_text,
                    &begun.user_message_proof,
                )
                .await
                .unwrap(),
                None
            );
        }

        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_all_proposals(10, 0)
            .unwrap()
            .is_empty());
    }
}
