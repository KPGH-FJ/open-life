//! Narrow runtime ports for optional Agent Memory and LifeModel participation.
//!
//! The canonical Chat/Work runtimes depend only on the bounded snapshot below.
//! Store schemas, retrieval implementations, and future LifeModel maintenance
//! can change behind these ports without becoming Task/Run/Item authorities.

use async_trait::async_trait;
use chrono::Utc;
use openlife_core::agent::main_chat_agent_v1::{ContextSourceCandidate, ContextSourceKind};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

pub(crate) const AGENT_MEMORY_CONTEXT_PORT_VERSION: &str = "agent_memory_context.v1";
pub(crate) const LIFE_MODEL_CONTEXT_PORT_VERSION: &str = "life_model_context.v1";
pub(crate) const PERSONAL_INTELLIGENCE_SUGGESTION_PORT_VERSION: &str =
    "personal_intelligence_suggestion.v1";
const MAX_CONTEXT_CONTENT_CHARS: usize = 4_800;
const MAX_LIFEMODEL_STATEMENT_CHARS: usize = 320;
const MAX_REASON_CHARS: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PersonalIntelligenceContextRequest<'a> {
    pub(crate) conversation_id: &'a str,
    pub(crate) user_text: &'a str,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelInfluenceReceipt {
    pub schema: String,
    pub status: String,
    pub applied_surfaces: Vec<String>,
    pub selected_item_refs: Vec<String>,
    pub reason_codes: Vec<String>,
    pub current_instruction_priority_preserved: bool,
    pub policy_priority_preserved: bool,
    pub permission_granted: bool,
    pub durable_write_authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelSelectedItemReceipt {
    pub item_ref: String,
    pub statement: String,
    pub source_refs: Vec<String>,
    pub confirmed_at: String,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelProductReceipt {
    pub status: String,
    pub source_id: Option<String>,
    pub model_version: Option<u64>,
    pub version_digest: Option<String>,
    pub document_digest: Option<String>,
    pub selected_items: Vec<LifeModelSelectedItemReceipt>,
    pub applied_surfaces: Vec<String>,
    pub current_instruction_priority_preserved: bool,
    pub policy_priority_preserved: bool,
    pub permission_granted: bool,
    pub durable_write_authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelContextMetadata {
    pub available: bool,
    pub source_id: Option<String>,
    pub model_id: Option<String>,
    pub model_version: Option<u64>,
    pub version_digest: Option<String>,
    pub document_digest: Option<String>,
    pub selected_sections: Vec<String>,
    pub selected_item_refs: Vec<String>,
    pub selected_items: Vec<LifeModelSelectedItemReceipt>,
    pub context_digest: Option<String>,
    pub context_chars: usize,
    pub omitted_relevant_fact_count: usize,
    pub warning_codes: Vec<String>,
    pub influence_receipt: LifeModelInfluenceReceipt,
}

impl LifeModelContextMetadata {
    pub(crate) fn product_receipt(&self) -> LifeModelProductReceipt {
        LifeModelProductReceipt {
            status: self.influence_receipt.status.clone(),
            source_id: self.source_id.clone(),
            model_version: self.model_version,
            version_digest: self.version_digest.clone(),
            document_digest: self.document_digest.clone(),
            selected_items: self.selected_items.clone(),
            applied_surfaces: self.influence_receipt.applied_surfaces.clone(),
            current_instruction_priority_preserved: self
                .influence_receipt
                .current_instruction_priority_preserved,
            policy_priority_preserved: self.influence_receipt.policy_priority_preserved,
            permission_granted: self.influence_receipt.permission_granted,
            durable_write_authorized: self.influence_receipt.durable_write_authorized,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelContextSnapshot {
    pub metadata: LifeModelContextMetadata,
    pub candidates: Vec<ContextSourceCandidate>,
    pub memory_rerank_terms: Vec<String>,
    pub tool_preference_hints: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentMemoryContextSnapshot {
    pub(crate) contract_version: &'static str,
    pub(crate) candidates: Vec<ContextSourceCandidate>,
}

#[derive(Debug, Clone)]
pub(crate) struct PersonalIntelligenceContextSnapshot {
    pub(crate) memory: AgentMemoryContextSnapshot,
    pub(crate) life_model_contract_version: &'static str,
    pub(crate) life_model: LifeModelContextSnapshot,
}

pub(crate) struct PersonalIntelligenceSuggestionRequest<'a> {
    pub(crate) conversation_id: &'a str,
    pub(crate) task_id: Option<&'a str>,
    pub(crate) run_id: Option<&'a str>,
    pub(crate) user_text: &'a str,
    pub(crate) policy: &'a openlife_core::agent::main_chat_agent_v1::PolicyDecision,
    pub(crate) memory_routing: &'a openlife_core::agent::MainChatMemoryRoutingResult,
    pub(crate) execution_epoch: &'a crate::main_chat_cancellation::MainChatExecutionEpoch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PersonalIntelligenceSuggestionReceipt {
    MemoryCommitted {
        memory_id: String,
        receipt_id: String,
        newly_committed: bool,
        undo_available: bool,
    },
    MemoryArchived {
        memory_id: String,
        undo_available: bool,
    },
    MemoryForgetNotFound,
    MemoryForgetAmbiguous {
        match_count: usize,
    },
    MemoryScopeUnsupported,
    LifeModelCandidateCaptured {
        candidate_id: String,
        replayed: bool,
    },
    NotApplicable,
}

#[async_trait]
pub(crate) trait AgentMemoryContextPort: Send + Sync {
    async fn load(
        &self,
        request: &PersonalIntelligenceContextRequest<'_>,
        life_model_rerank_terms: &[String],
    ) -> Result<AgentMemoryContextSnapshot, String>;
}

#[async_trait]
pub(crate) trait LifeModelContextPort: Send + Sync {
    async fn load(
        &self,
        request: &PersonalIntelligenceContextRequest<'_>,
    ) -> Result<LifeModelContextSnapshot, String>;
}

#[async_trait]
pub(crate) trait PersonalIntelligenceSuggestionPort: Send + Sync {
    async fn apply(
        &self,
        request: PersonalIntelligenceSuggestionRequest<'_>,
    ) -> Result<PersonalIntelligenceSuggestionReceipt, String>;
}

struct AppStateAgentMemoryContextPort {
    state: Arc<AppState>,
}

struct AppStateLifeModelContextPort {
    state: Arc<AppState>,
}

struct AppStatePersonalIntelligenceSuggestionPort {
    state: Arc<AppState>,
}

#[async_trait]
impl AgentMemoryContextPort for AppStateAgentMemoryContextPort {
    async fn load(
        &self,
        request: &PersonalIntelligenceContextRequest<'_>,
        life_model_rerank_terms: &[String],
    ) -> Result<AgentMemoryContextSnapshot, String> {
        if !agent_memory_use_is_enabled(&self.state, request.conversation_id).await? {
            return Ok(AgentMemoryContextSnapshot {
                contract_version: AGENT_MEMORY_CONTEXT_PORT_VERSION,
                candidates: Vec::new(),
            });
        }
        let candidates = crate::main_chat_context_loader::retrievable_lifecycle_context_candidates(
            &self.state,
            request.conversation_id,
            request.user_text,
            life_model_rerank_terms,
        )
        .await?;
        Ok(AgentMemoryContextSnapshot {
            contract_version: AGENT_MEMORY_CONTEXT_PORT_VERSION,
            candidates,
        })
    }
}

#[async_trait]
impl LifeModelContextPort for AppStateLifeModelContextPort {
    async fn load(
        &self,
        request: &PersonalIntelligenceContextRequest<'_>,
    ) -> Result<LifeModelContextSnapshot, String> {
        let version = self
            .state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .map_err(|error| error.to_string())?;
        Ok(build_life_model_context(
            version.as_ref(),
            request.user_text,
            Vec::new(),
        ))
    }
}

pub(crate) async fn load_personal_intelligence_context(
    state: &Arc<AppState>,
    request: PersonalIntelligenceContextRequest<'_>,
) -> PersonalIntelligenceContextSnapshot {
    let life_model_port = AppStateLifeModelContextPort {
        state: Arc::clone(state),
    };
    let life_model = life_model_port
        .load(&request)
        .await
        .unwrap_or_else(|error| {
            log::warn!("LifeModel context port unavailable: {error}");
            build_life_model_context(
                None,
                request.user_text,
                vec!["lifemodel_v2_unavailable".into()],
            )
        });
    let memory_port = AppStateAgentMemoryContextPort {
        state: Arc::clone(state),
    };
    let memory = memory_port
        .load(&request, &life_model.memory_rerank_terms)
        .await
        .unwrap_or_else(|error| {
            log::warn!("Agent Memory context port unavailable: {error}");
            AgentMemoryContextSnapshot {
                contract_version: AGENT_MEMORY_CONTEXT_PORT_VERSION,
                candidates: vec![degraded_memory_candidate("memory_context_port_unavailable")],
            }
        });
    debug_assert!(!memory.contract_version.is_empty());
    debug_assert!(!LIFE_MODEL_CONTEXT_PORT_VERSION.is_empty());
    PersonalIntelligenceContextSnapshot {
        memory,
        life_model_contract_version: LIFE_MODEL_CONTEXT_PORT_VERSION,
        life_model,
    }
}

pub(crate) async fn apply_authorized_personal_intelligence_suggestion(
    state: &Arc<AppState>,
    request: PersonalIntelligenceSuggestionRequest<'_>,
) -> Result<PersonalIntelligenceSuggestionReceipt, String> {
    AppStatePersonalIntelligenceSuggestionPort {
        state: Arc::clone(state),
    }
    .apply(request)
    .await
}

pub(crate) fn has_explicit_memory_forget_request(user_text: &str) -> bool {
    explicit_memory_forget_query(user_text).is_some()
}

pub(crate) fn personal_intelligence_product_reply(
    receipt: &PersonalIntelligenceSuggestionReceipt,
) -> Option<String> {
    match receipt {
        PersonalIntelligenceSuggestionReceipt::MemoryCommitted {
            memory_id,
            receipt_id,
            newly_committed,
            undo_available,
        } => Some(format!(
            "已按你的明确要求记住。你可以随时在个人智能中编辑或撤销。memory_id={memory_id} receipt_id={receipt_id} newly_committed={newly_committed} undo_available={undo_available}"
        )),
        PersonalIntelligenceSuggestionReceipt::MemoryArchived {
            memory_id,
            undo_available,
        } => Some(format!(
            "已忘记这条 Agent Memory，后续不会再召回；你仍可在个人智能中撤销。memory_id={memory_id} undo_available={undo_available}"
        )),
        PersonalIntelligenceSuggestionReceipt::MemoryForgetNotFound => Some(
            "没有找到与你描述相符的 Agent Memory；没有更改任何内容。你可以在个人智能中查看后再选择忘记。".into(),
        ),
        PersonalIntelligenceSuggestionReceipt::MemoryForgetAmbiguous { match_count } => Some(
            format!(
                "找到 {match_count} 条可能匹配的 Agent Memory；为避免忘错，我没有更改任何内容。请说得更具体，或在个人智能中选择。"
            ),
        ),
        PersonalIntelligenceSuggestionReceipt::MemoryScopeUnsupported => Some(
            "记忆现在只支持“个人”和“当前 Project”两个范围；没有保存。请改为“请记住……”或“请在当前 Project 记住……”。".into(),
        ),
        PersonalIntelligenceSuggestionReceipt::LifeModelCandidateCaptured {
            candidate_id,
            replayed,
        } => Some(format!(
            "已记录一条 LifeModel 候选供你在个人智能中查看；尚未创建提案，也没有修改 LifeModel。candidate_id={candidate_id} replayed={replayed}"
        )),
        PersonalIntelligenceSuggestionReceipt::NotApplicable => None,
    }
}

#[async_trait]
impl PersonalIntelligenceSuggestionPort for AppStatePersonalIntelligenceSuggestionPort {
    async fn apply(
        &self,
        request: PersonalIntelligenceSuggestionRequest<'_>,
    ) -> Result<PersonalIntelligenceSuggestionReceipt, String> {
        apply_authorized_suggestion_with_state(&self.state, request).await
    }
}

async fn apply_authorized_suggestion_with_state(
    state: &Arc<AppState>,
    request: PersonalIntelligenceSuggestionRequest<'_>,
) -> Result<PersonalIntelligenceSuggestionReceipt, String> {
    use openlife_core::agent::main_chat_agent_v1::{AllowedCapability, IntentSourceKind};
    use openlife_core::agent::{
        bind_memory_fact_scope_owner, explicit_memory_scope_from_user_text,
        CanonicalMemoryFactDescriptor, MemoryDestination, MemoryLifecycleRiskLevel,
        MemoryLifecycleSensitivity,
    };

    debug_assert!(!PERSONAL_INTELLIGENCE_SUGGESTION_PORT_VERSION.is_empty());
    if let Some(query) = explicit_memory_forget_query(request.user_text) {
        return forget_memory_with_state(state, request.conversation_id, &query).await;
    }
    let candidate = request.memory_routing.candidates.iter().find(|candidate| {
        request
            .policy
            .allows_memory_candidate(&candidate.candidate_id)
    });
    let Some(candidate) = candidate else {
        return Ok(PersonalIntelligenceSuggestionReceipt::NotApplicable);
    };

    if request
        .policy
        .allows(AllowedCapability::ReversibleMemoryCommit)
        && candidate.destination == MemoryDestination::MemoryProposal
    {
        if !state.config.lock().await.system.agent_memory_enabled {
            return Err("agent_memory_disabled_globally".into());
        }
        let scope = match explicit_memory_scope_from_user_text(request.user_text) {
            Ok(scope) => scope,
            Err(error) if error.to_string() == "agent_memory_scope_not_supported" => {
                return Ok(PersonalIntelligenceSuggestionReceipt::MemoryScopeUnsupported);
            }
            Err(error) => return Err(error.to_string()),
        };
        let mut fact = CanonicalMemoryFactDescriptor::from_candidate(
            candidate.normalized_claim.clone(),
            candidate.kind,
            scope,
            MemoryLifecycleRiskLevel::from_intent_risk(request.policy.risk),
            MemoryLifecycleSensitivity::from_policy_and_candidate(
                request.policy.sensitivity,
                &candidate.sensitivity,
            ),
        )
        .map_err(|error| format!("personal_intelligence_memory_descriptor_rejected:{error}"))?;
        let (workspace_owner, project_owner) =
            canonical_scope_owners(state, request.conversation_id).await?;
        bind_memory_fact_scope_owner(
            &mut fact,
            Some(request.conversation_id),
            workspace_owner.as_deref(),
            project_owner.as_deref(),
        )
        .map_err(|error| format!("personal_intelligence_memory_scope_rejected:{error}"))?;
        let admission = request
            .policy
            .authorize_explicit_memory_admission(
                IntentSourceKind::CurrentAuthenticatedUserMessage,
                request.user_text,
                candidate,
                &fact,
            )
            .map_err(|error| error.to_string())?;
        let receipt = crate::memory_gateway::commit_explicit_user_memory_for_turn_with_state(
            state,
            request.task_id.map(str::to_string),
            request.run_id.map(str::to_string),
            request.policy.authorized_user_message_id.clone(),
            fact,
            admission,
            request.user_text,
            candidate,
            request.execution_epoch,
        )
        .await?;
        return Ok(PersonalIntelligenceSuggestionReceipt::MemoryCommitted {
            memory_id: receipt.memory_id,
            receipt_id: receipt.receipt_id,
            newly_committed: receipt.newly_committed,
            undo_available: receipt.undo_available,
        });
    }

    if request.policy.allows(AllowedCapability::LifeModelProposal)
        && candidate.destination == MemoryDestination::LifeModelProposal
    {
        let receipt = crate::life_model_learning::capture_explicit_main_chat_candidate(
            state,
            candidate,
            request.policy,
            request.user_text,
        )
        .await?;
        return Ok(
            PersonalIntelligenceSuggestionReceipt::LifeModelCandidateCaptured {
                candidate_id: receipt.candidate.id,
                replayed: receipt.replayed,
            },
        );
    }

    Ok(PersonalIntelligenceSuggestionReceipt::NotApplicable)
}

async fn forget_memory_with_state(
    state: &Arc<AppState>,
    conversation_id: &str,
    query: &str,
) -> Result<PersonalIntelligenceSuggestionReceipt, String> {
    use openlife_core::agent::MemoryLifecycleScope;

    let (workspace_owner, project_owner) = canonical_scope_owners(state, conversation_id).await?;
    let conversation_scope_owner = openlife_core::agent::memory_scope_owner_ref(
        MemoryLifecycleScope::Conversation,
        conversation_id,
    )
    .map_err(|error| error.to_string())?;
    let project_scope_owner = project_owner
        .as_deref()
        .map(|owner| {
            openlife_core::agent::memory_scope_owner_ref(MemoryLifecycleScope::Project, owner)
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    let workspace_scope_owner = workspace_owner
        .as_deref()
        .map(|owner| {
            openlife_core::agent::memory_scope_owner_ref(MemoryLifecycleScope::Workspace, owner)
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    let records = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(|| "memory_lifecycle_store_unavailable".to_string())?
        .lock()
        .await
        .list_active_records(None, 200)
        .map_err(|error| error.to_string())?;
    let exact_memory_id = query
        .split_whitespace()
        .find(|token| token.starts_with("memory:"))
        .map(|token| token.trim_matches(|ch: char| ch.is_ascii_punctuation()));
    let normalized_query = normalize_memory_match_text(query);
    let mut matches = records
        .into_iter()
        .filter(|record| match record.scope {
            MemoryLifecycleScope::Global => true,
            MemoryLifecycleScope::Conversation => {
                record.scope_owner_ref.as_deref() == Some(conversation_scope_owner.as_str())
            }
            MemoryLifecycleScope::Project => {
                record.scope_owner_ref.as_deref() == project_scope_owner.as_deref()
            }
            MemoryLifecycleScope::Workspace => {
                record.scope_owner_ref.as_deref() == workspace_scope_owner.as_deref()
            }
        })
        .filter(|record| {
            if let Some(memory_id) = exact_memory_id {
                return record.memory_id == memory_id;
            }
            let normalized_content = normalize_memory_match_text(&record.content);
            !normalized_query.is_empty()
                && (normalized_content == normalized_query
                    || normalized_content.contains(&normalized_query)
                    || normalized_query.contains(&normalized_content))
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(PersonalIntelligenceSuggestionReceipt::MemoryForgetNotFound);
    }
    if matches.len() > 1 {
        return Ok(
            PersonalIntelligenceSuggestionReceipt::MemoryForgetAmbiguous {
                match_count: matches.len(),
            },
        );
    }
    let memory_id = matches.remove(0).memory_id;
    crate::memory_gateway::archive_memory_asset_with_state(memory_id.clone(), state)
        .await
        .map_err(|error| error.to_string())?;
    Ok(PersonalIntelligenceSuggestionReceipt::MemoryArchived {
        memory_id,
        undo_available: true,
    })
}

fn explicit_memory_forget_query(user_text: &str) -> Option<String> {
    let trimmed = user_text.trim();
    let lower = trimmed.to_ascii_lowercase();
    if (lower.contains("remember") && lower.contains("forget"))
        || (trimmed.contains("记住") && trimmed.contains("忘记"))
    {
        return None;
    }
    for prefix in [
        "请忘记",
        "忘记",
        "不要再记住",
        "please forget",
        "forget",
        "remove memory about",
    ] {
        let comparison = if prefix.is_ascii() { &lower } else { trimmed };
        if let Some(remainder) = comparison.strip_prefix(prefix) {
            let offset = comparison.len().saturating_sub(remainder.len());
            let original_remainder = trimmed.get(offset..).unwrap_or(remainder);
            let query = original_remainder
                .trim_matches(|ch: char| ch.is_whitespace() || "：:，,。.!！?？\"'“”".contains(ch));
            return (!query.is_empty()).then(|| query.to_string());
        }
    }
    None
}

fn normalize_memory_match_text(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

async fn agent_memory_use_is_enabled(
    state: &Arc<AppState>,
    conversation_id: &str,
) -> Result<bool, String> {
    if !state.config.lock().await.system.agent_memory_enabled {
        return Ok(false);
    }
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
    Ok(conversation.memory_mode.uses_memory())
}

async fn canonical_scope_owners(
    state: &Arc<AppState>,
    conversation_id: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let Some(store) = state.conversation_store.as_ref() else {
        return Ok((None, None));
    };
    let store = store.lock().await;
    let Some(conversation) = store
        .get_conversation(conversation_id)
        .map_err(|error| error.to_string())?
    else {
        return Ok((None, None));
    };
    let Some(project_id) = conversation.project_id else {
        return Ok((None, None));
    };
    let project = store
        .get_project(&project_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "personal_intelligence_project_missing".to_string())?;
    Ok((project.workspace_root, Some(project.id)))
}

fn degraded_memory_candidate(code: &str) -> ContextSourceCandidate {
    ContextSourceCandidate::new(
        ContextSourceKind::RuntimePolicy,
        "memory.context.unavailable",
        format!("memory_retrieval_degraded:{code}; optional Agent Memory context is unavailable for this turn and must not be inferred"),
        "optional Agent Memory port degradation",
        "internal",
        12,
    )
}

pub(crate) fn build_life_model_context(
    life_model: Option<&openlife_core::life_model::v2::LifeModelVersionV2>,
    task_text: &str,
    mut warning_codes: Vec<String>,
) -> LifeModelContextSnapshot {
    let current_instruction_override =
        openlife_core::agent::life_model_runtime_context::task_explicitly_disables_lifemodel(
            task_text,
        );
    let packet = match life_model {
        Some(version) => match openlife_core::agent::LifeModelRuntimeContextV2::build(
            version,
            task_text,
            Utc::now(),
        ) {
            Ok(packet) => packet,
            Err(error) => {
                log::warn!("LifeModel runtime packet rejected: {error}");
                warning_codes.push("lifemodel_v2_runtime_packet_rejected".into());
                None
            }
        },
        None => None,
    };
    warning_codes.sort();
    warning_codes.dedup();
    let selected_sections = packet
        .as_ref()
        .map(|packet| {
            packet
                .selected_sections
                .iter()
                .map(|section| section_label(*section).to_string())
                .collect()
        })
        .unwrap_or_default();
    let selected_items = packet
        .as_ref()
        .map(|packet| {
            packet
                .facts
                .iter()
                .map(|fact| LifeModelSelectedItemReceipt {
                    item_ref: format!("{}:{}", section_label(fact.section), fact.item_id),
                    statement: bounded(&fact.value, MAX_LIFEMODEL_STATEMENT_CHARS),
                    source_refs: fact.source_refs.to_vec(),
                    confirmed_at: fact.confirmed_at.clone(),
                    reason_code: bounded(&fact.selected_reason, MAX_REASON_CHARS),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let selected_item_refs = selected_items
        .iter()
        .map(|item| item.item_ref.clone())
        .collect::<Vec<_>>();
    let mut reason_codes = selected_items
        .iter()
        .map(|item| item.reason_code.clone())
        .collect::<Vec<_>>();
    if current_instruction_override {
        reason_codes.push("current_instruction_override".into());
    }
    let source_id = packet.as_ref().map(|_| "lifemodel.v2.runtime".to_string());
    let content = packet
        .as_ref()
        .map(|packet| bounded(&packet.render_prompt(), MAX_CONTEXT_CONTENT_CHARS));
    let context_digest = content.as_ref().map(|content| {
        let (bytes, hash) = openlife_core::agent::metadata_safe::metadata_safe_text_digest(content);
        format!("bytes:{bytes} hash:{hash}")
    });
    let context_chars = content
        .as_ref()
        .map(|value| value.chars().count())
        .unwrap_or_default();
    let metadata = LifeModelContextMetadata {
        available: false,
        source_id: source_id.clone(),
        model_id: packet.as_ref().map(|packet| packet.model_id.clone()),
        model_version: packet.as_ref().map(|packet| packet.model_version),
        version_digest: packet.as_ref().map(|packet| packet.version_digest.clone()),
        document_digest: packet.as_ref().map(|packet| packet.document_digest.clone()),
        selected_sections,
        selected_item_refs: selected_item_refs.clone(),
        selected_items,
        context_digest,
        context_chars,
        omitted_relevant_fact_count: packet
            .as_ref()
            .map(|packet| packet.omitted_relevant_fact_count)
            .unwrap_or_default(),
        warning_codes,
        influence_receipt: LifeModelInfluenceReceipt {
            schema: "openlife.lifemodel.influence-receipt.v1".into(),
            status: if current_instruction_override {
                "current_instruction_override"
            } else if packet.is_some() {
                "eligible_for_context"
            } else if life_model.is_some() {
                "not_task_relevant"
            } else {
                "canonical_model_unavailable"
            }
            .into(),
            applied_surfaces: Vec::new(),
            selected_item_refs,
            reason_codes,
            current_instruction_priority_preserved: true,
            policy_priority_preserved: true,
            permission_granted: false,
            durable_write_authorized: false,
        },
    };
    let candidates = match (source_id, content) {
        (Some(source_id), Some(content)) => vec![ContextSourceCandidate::new(
            ContextSourceKind::LifeModelContext,
            source_id,
            content,
            "bounded canonical LifeModel context from optional typed port",
            "private",
            18,
        )],
        _ => Vec::new(),
    };
    let memory_rerank_terms = packet
        .as_ref()
        .map(|packet| packet.facts.iter().map(|fact| fact.value.clone()).collect())
        .unwrap_or_default();
    let tool_preference_hints = packet
        .as_ref()
        .map(|packet| {
            packet
                .facts
                .iter()
                .filter(|fact| {
                    matches!(
        fact.section,
        openlife_core::life_model::v2::LifeModelSectionV2::StablePreferences
            | openlife_core::life_model::v2::LifeModelSectionV2::CollaborationPreferences
    )
                })
                .map(|fact| fact.value.clone())
                .collect()
        })
        .unwrap_or_default();
    LifeModelContextSnapshot {
        metadata,
        candidates,
        memory_rerank_terms,
        tool_preference_hints,
    }
}

fn section_label(section: openlife_core::life_model::v2::LifeModelSectionV2) -> &'static str {
    use openlife_core::life_model::v2::LifeModelSectionV2;
    match section {
        LifeModelSectionV2::Identity => "identity",
        LifeModelSectionV2::Values => "values",
        LifeModelSectionV2::LongTermGoals => "long_term_goals",
        LifeModelSectionV2::StablePreferences => "stable_preferences",
        LifeModelSectionV2::PersonalBoundaries => "personal_boundaries",
        LifeModelSectionV2::ImportantRelationships => "important_relationships",
        LifeModelSectionV2::Capabilities => "capabilities",
        LifeModelSectionV2::Resources => "resources",
        LifeModelSectionV2::DecisionPrinciples => "decision_principles",
        LifeModelSectionV2::CollaborationPreferences => "collaboration_preferences",
    }
}

fn bounded(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::life_model::v2::{
        LifeModelDocumentV2, LifeModelItemV2, LifeModelSectionV2, LifeModelStatementV2,
        LifeModelTypedDiffV2, LifeModelTypedOperationV2, LIFE_MODEL_V2_TYPED_DIFF_SCHEMA,
    };

    #[test]
    fn empty_optional_ports_keep_security_authority_false() {
        let snapshot = build_life_model_context(None, "prepare a report", Vec::new());
        assert!(snapshot.candidates.is_empty());
        assert_eq!(
            snapshot.metadata.influence_receipt.status,
            "canonical_model_unavailable"
        );
        assert!(
            snapshot
                .metadata
                .influence_receipt
                .current_instruction_priority_preserved
        );
        assert!(
            snapshot
                .metadata
                .influence_receipt
                .policy_priority_preserved
        );
        assert!(!snapshot.metadata.influence_receipt.permission_granted);
        assert!(!snapshot.metadata.influence_receipt.durable_write_authorized);
    }

    #[tokio::test]
    async fn global_and_conversation_memory_controls_gate_recall_without_touching_lifemodel() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Memory controls")
            .unwrap();
        assert!(agent_memory_use_is_enabled(&state, &conversation_id)
            .await
            .unwrap());

        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .set_memory_mode(
                &conversation_id,
                openlife_core::conversation::ConversationMemoryMode::Off,
            )
            .unwrap();
        assert!(!agent_memory_use_is_enabled(&state, &conversation_id)
            .await
            .unwrap());

        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .set_memory_mode(
                &conversation_id,
                openlife_core::conversation::ConversationMemoryMode::UseOnly,
            )
            .unwrap();
        state.config.lock().await.system.agent_memory_enabled = false;
        assert!(!agent_memory_use_is_enabled(&state, &conversation_id)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn canonical_context_ports_use_lifemodel_without_task_ownership() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let now = chrono::Utc::now().to_rfc3339();
        let statement = LifeModelStatementV2 {
            id: "concise".into(),
            statement: "沟通保持简洁直接".into(),
            source_refs: vec!["message:user:confirmed".into()],
            confirmed_at: now.clone(),
        };
        let mut result = LifeModelDocumentV2::empty("primary");
        result.collaboration_preferences.push(statement.clone());
        let diff = LifeModelTypedDiffV2 {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: "primary".into(),
            base_version: None,
            base_document_digest: None,
            operations: vec![LifeModelTypedOperationV2::Add {
                section: LifeModelSectionV2::CollaborationPreferences,
                item: LifeModelItemV2::Statement(statement),
            }],
            result_document_digest: result.digest().unwrap(),
        };
        state
            .life_model_manager
            .lock()
            .await
            .materialize_reviewed_v2_typed_diff(&diff, "context-port-test", &[], &now)
            .unwrap();
        let before_tasks = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_task_snapshots(10)
            .unwrap();
        let snapshot = load_personal_intelligence_context(
            &state,
            PersonalIntelligenceContextRequest {
                conversation_id: &uuid::Uuid::new_v4().to_string(),
                user_text: "请写一封简洁的项目邮件",
            },
        )
        .await;
        assert!(
            snapshot.life_model.metadata.warning_codes.is_empty(),
            "{:?}",
            snapshot.life_model.metadata.warning_codes
        );
        assert_eq!(snapshot.life_model.metadata.model_version, Some(1));
        assert_eq!(
            snapshot.memory.contract_version,
            AGENT_MEMORY_CONTEXT_PORT_VERSION
        );
        assert_eq!(
            snapshot.life_model_contract_version,
            LIFE_MODEL_CONTEXT_PORT_VERSION
        );
        assert!(snapshot
            .life_model
            .candidates
            .iter()
            .any(|candidate| candidate.content.contains("沟通保持简洁直接")));
        assert!(
            !snapshot
                .life_model
                .metadata
                .influence_receipt
                .permission_granted
        );
        assert!(
            !snapshot
                .life_model
                .metadata
                .influence_receipt
                .durable_write_authorized
        );
        let after_tasks = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_task_snapshots(10)
            .unwrap();
        assert_eq!(before_tasks.len(), after_tasks.len());
    }

    #[tokio::test]
    async fn explicit_lifemodel_suggestion_creates_only_a_candidate() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        let user_text = "Update my life model: communication style is concise and direct.";
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "LifeModel suggestion")
            .unwrap();
        let provider = openlife_core::conversation::ProviderBinding {
            profile_id: "local-test".into(),
            provider_id: "ollama".into(),
            model_id: "test".into(),
            endpoint_class: "local".into(),
            config_generation: "1".into(),
        };
        let begun = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_chat_turn_with_proof(openlife_core::conversation::BeginChatTurn {
                conversation_id: &conversation_id,
                turn_id: &turn_id,
                user_message: user_text,
                provider: &provider,
            })
            .unwrap();
        let ingress = openlife_core::agent::main_chat_agent_v1::AgentIngress::default()
            .decide_with_conversation_user_item(&begun.user_message_proof, user_text, &[])
            .unwrap();
        let proposals_before = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_all_proposals(100, 0)
            .unwrap()
            .len();
        let epoch = state
            .main_chat_runtime_state
            .lock()
            .await
            .cancellation_registry
            .try_register(&turn_id)
            .unwrap()
            .execution_epoch();
        let receipt = apply_authorized_personal_intelligence_suggestion(
            &state,
            PersonalIntelligenceSuggestionRequest {
                conversation_id: &conversation_id,
                task_id: Some(&uuid::Uuid::new_v4().to_string()),
                run_id: Some(&uuid::Uuid::new_v4().to_string()),
                user_text,
                policy: &ingress.policy_decision,
                memory_routing: &ingress.intent_frame.memory_routing,
                execution_epoch: &epoch,
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            receipt,
            PersonalIntelligenceSuggestionReceipt::LifeModelCandidateCaptured { .. }
        ));
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_all_proposals(100, 0)
                .unwrap()
                .len(),
            proposals_before
        );
        assert!(state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .is_none());
    }
}
#[cfg(test)]
mod explicit_forget_tests {
    use super::*;
    use openlife_core::agent::{
        AgentProposal, MemoryLifecycleAcceptanceInput, ProposalSource, ProposalType, RiskLevel,
    };

    #[test]
    fn explicit_forget_requires_a_specific_memory_target() {
        assert_eq!(
            explicit_memory_forget_query("请忘记：我喜欢先看结论").as_deref(),
            Some("我喜欢先看结论")
        );
        assert_eq!(
            explicit_memory_forget_query("please forget memory:preference-1").as_deref(),
            Some("memory:preference-1")
        );
        assert!(explicit_memory_forget_query("请忘记").is_none());
        assert!(explicit_memory_forget_query("请记住这点，然后忘记这条记忆").is_none());
    }

    #[tokio::test]
    async fn explicit_forget_archives_one_matching_memory_and_keeps_undo() {
        let state = crate::test_utils::test_app_state();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Forget Memory")
            .unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.personal",
            serde_json::json!({
                "content": "我喜欢先看结论",
                "scope": "global",
                "category": "preference",
                "candidateKind": "preference",
                "riskLevel": "low",
                "sensitivity": "internal"
            }),
            "accepted Memory for explicit forget test",
            1.0,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.id = "proposal:explicit-forget-test".into();
        let input = MemoryLifecycleAcceptanceInput::from_memory_proposal(
            &proposal,
            "我喜欢先看结论".into(),
        )
        .unwrap();
        let memory_id = state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .accept_memory_proposal(input)
            .unwrap()
            .record
            .memory_id;

        let receipt = forget_memory_with_state(&state, &conversation_id, "我喜欢先看结论")
            .await
            .unwrap();
        assert_eq!(
            receipt,
            PersonalIntelligenceSuggestionReceipt::MemoryArchived {
                memory_id: memory_id.clone(),
                undo_available: true,
            }
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
}
