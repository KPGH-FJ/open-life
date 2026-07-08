use crate::errors::AppError;
use crate::main_chat_hs_runtime::classify_hs_policy_topic;
use crate::main_chat_preprocess::{filter_lifecycle_active_memory_results, merge_memory_hits};
use crate::AppState;
use openlife_core::agent::{
    AgentProposal, MemoryLifecycleAcceptanceInput, MemoryLifecycleScope, MemoryMaterializedView,
    MemoryRollbackReport,
};
use openlife_core::llm::ChatMessage;
use openlife_core::memory_gateway::{
    MemoryGateway, MemoryGatewayDecision, MemoryGatewayRequest, MemoryGatewaySubject,
    MemoryGatewayWriteStatus,
};
use openlife_core::vectors::{
    embed_text_with_privacy, ArchivedChunkSummary, ExportedVectorChunk, MemoryChunk, TierStats,
    VectorInsertItem,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryGatewayWriteReport {
    pub decision: MemoryGatewayDecision,
    pub session_id: Option<String>,
    pub memory_id: Option<String>,
    pub embedding_id: Option<i64>,
    pub proposal_id: Option<String>,
    pub run_id: Option<String>,
    pub evidence_id: Option<String>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub conflict_status: Option<String>,
    pub direct_store_write: bool,
}

impl MemoryGatewayWriteReport {
    fn new(decision: MemoryGatewayDecision) -> Self {
        Self {
            decision,
            session_id: None,
            memory_id: None,
            embedding_id: None,
            proposal_id: None,
            run_id: None,
            evidence_id: None,
            before: None,
            after: None,
            conflict_status: None,
            direct_store_write: false,
        }
    }

    fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    fn with_proposal(mut self, proposal: &AgentProposal) -> Self {
        self.proposal_id = Some(proposal.id.clone());
        self.run_id = proposal.run_id.clone();
        self.before = proposal.before.clone();
        self.after = Some(proposal.after.clone());
        self
    }
}

#[derive(Clone)]
struct EmbeddingPrivacyContext {
    provider: String,
    openai_base: String,
    openai_key: String,
    embedding_model: String,
    embedding_enabled: bool,
    privacy_engine: openlife_core::privacy::PrivacyEngine,
}

async fn embedding_privacy_context(state: &Arc<AppState>) -> EmbeddingPrivacyContext {
    let (provider, openai_base, openai_key, embedding_model, embedding_enabled) = {
        let cfg = state.config.lock().await;
        (
            cfg.llm.provider.clone(),
            cfg.llm.openai_base.clone(),
            cfg.llm.openai_key.clone(),
            cfg.llm.embedding_model.clone(),
            cfg.llm.embedding_enabled,
        )
    };
    let privacy_engine = {
        let engine = state.privacy_engine.lock().await;
        engine.clone()
    };

    EmbeddingPrivacyContext {
        provider,
        openai_base,
        openai_key,
        embedding_model,
        embedding_enabled,
        privacy_engine,
    }
}

async fn embed_memory_text_with_privacy(
    text: &str,
    state: &Arc<AppState>,
) -> Result<Vec<f32>, AppError> {
    let ctx = embedding_privacy_context(state).await;
    let hs_local_only =
        classify_hs_policy_topic(text, "") != openlife_core::agent::PolicyTopic::General;
    embed_text_with_privacy(
        text,
        &ctx.provider,
        &ctx.openai_base,
        &ctx.openai_key,
        &ctx.embedding_model,
        ctx.embedding_enabled,
        &ctx.privacy_engine,
        hs_local_only,
    )
    .await
    .map_err(AppError::from)
}

pub(crate) async fn save_turn_message_with_state(
    session_id: &str,
    message: &ChatMessage,
    state: &Arc<AppState>,
) -> Result<MemoryGatewayWriteReport, AppError> {
    let decision = MemoryGateway::decide(MemoryGatewaySubject::ChatTurn);
    let store = state.memory_store.lock().await;
    store
        .save_message(session_id, message)
        .map_err(AppError::from)?;
    store
        .touch_chat_session(session_id)
        .map_err(AppError::from)?;

    Ok(MemoryGatewayWriteReport::new(decision)
        .with_session_id(session_id)
        .with_gateway_write())
}

pub(crate) async fn save_turn_message_if_needed_with_state(
    session_id: &str,
    message: &ChatMessage,
    state: &Arc<AppState>,
) -> Result<bool, String> {
    let decision = MemoryGateway::decide(MemoryGatewaySubject::ChatTurn);
    let store = state.memory_store.lock().await;
    let should_skip = store
        .load_recent_messages(session_id, 1)
        .map_err(|e| e.to_string())?
        .last()
        .map(|last| last.role == message.role && last.content == message.content)
        .unwrap_or(false);
    if should_skip {
        let _ = store.touch_chat_session(session_id);
        return Ok(false);
    }
    debug_assert_eq!(decision.status, MemoryGatewayWriteStatus::ContextOnly);
    store
        .save_message(session_id, message)
        .map_err(|e| e.to_string())?;
    let _ = store.touch_chat_session(session_id);
    Ok(true)
}

pub(crate) async fn create_chat_session_with_state(
    session_id: &str,
    title: &str,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    let store = state.memory_store.lock().await;
    store
        .create_chat_session(session_id, title)
        .map_err(AppError::from)
}

pub(crate) async fn rename_chat_session_with_state(
    session_id: &str,
    title: &str,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    let store = state.memory_store.lock().await;
    store
        .rename_chat_session(session_id, title)
        .map_err(AppError::from)
}

pub(crate) async fn delete_chat_session_with_state(
    session_id: &str,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    let store = state.memory_store.lock().await;
    store
        .delete_chat_session(session_id)
        .map_err(AppError::from)
}

pub(crate) async fn touch_chat_session_with_state(
    session_id: &str,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    let store = state.memory_store.lock().await;
    store.touch_chat_session(session_id).map_err(AppError::from)
}

pub(crate) async fn record_state_entry_with_state(
    dimension_name: &str,
    value: f64,
    unit: &str,
    note: Option<&str>,
    state: &Arc<AppState>,
) -> Result<i64, AppError> {
    let decision = MemoryGateway::decide(MemoryGatewaySubject::HealthEvent);
    debug_assert_eq!(
        decision.status,
        MemoryGatewayWriteStatus::LocalMemoryWritten
    );
    let store = state.memory_store.lock().await;
    store
        .record_state_entry(dimension_name, value, unit, note)
        .map_err(AppError::from)
}

pub(crate) async fn index_memory_chunk_with_state(
    session_id: String,
    content: String,
    source: String,
    state: &Arc<AppState>,
) -> Result<i64, AppError> {
    let decision = MemoryGateway::decide(MemoryGatewaySubject::ManualIndexedNote);
    let embedding = embed_memory_text_with_privacy(&content, state).await?;
    let embedding_id = {
        let store = state.vector_store.lock().await;
        store
            .insert(&session_id, &content, &embedding, &source)
            .map_err(AppError::from)?
    };
    {
        let store = state.memory_store.lock().await;
        let tags = vec!["manual".to_string(), format!("source:{}", source)];
        store
            .save_memory_record(
                &session_id,
                &content,
                "indexed_note",
                &source,
                &tags,
                "private",
                Some(embedding_id),
            )
            .map_err(AppError::from)?;
    }
    let _report = MemoryGatewayWriteReport::new(decision)
        .with_session_id(session_id)
        .with_embedding_id(embedding_id)
        .with_gateway_write();
    Ok(embedding_id)
}

pub(crate) async fn search_memory_with_state(
    query: String,
    top_k: usize,
    state: &Arc<AppState>,
) -> Result<Vec<(MemoryChunk, f32)>, AppError> {
    let desensitized_query = {
        let privacy_engine = state.privacy_engine.lock().await;
        privacy_engine.desensitize(&query).0
    };
    let text_hits = {
        let store = state.memory_store.lock().await;
        store
            .search_text_memories(None, &desensitized_query, top_k)
            .map_err(AppError::from)?
    };
    let vector_hits = match embed_memory_text_with_privacy(&query, state).await {
        Ok(embedding) => {
            let store = state.vector_store.lock().await;
            store.search(&embedding, top_k).map_err(AppError::from)?
        }
        Err(_) => vec![],
    };
    Ok(filter_lifecycle_active_memory_results(
        merge_memory_hits(vector_hits, text_hits, top_k),
        state,
    )
    .await)
}

pub(crate) async fn run_memory_tier_maintenance_with_state(
    state: &Arc<AppState>,
) -> Result<(usize, usize), AppError> {
    let store = state.vector_store.lock().await;
    store.run_tier_maintenance().map_err(AppError::from)
}

pub(crate) async fn archive_low_access_memories_with_state(
    state: &Arc<AppState>,
) -> Result<usize, AppError> {
    let store = state.vector_store.lock().await;
    store.archive_low_access_memories().map_err(AppError::from)
}

pub(crate) async fn restore_archived_chunks_with_state(
    chunk_ids: &[i64],
    state: &Arc<AppState>,
) -> Result<usize, AppError> {
    let store = state.vector_store.lock().await;
    store.restore_archived(chunk_ids).map_err(AppError::from)
}

pub(crate) async fn list_archived_chunks_with_state(
    limit: usize,
    state: &Arc<AppState>,
) -> Result<Vec<ArchivedChunkSummary>, AppError> {
    let store = state.vector_store.lock().await;
    store.list_archived(limit).map_err(AppError::from)
}

pub(crate) async fn get_memory_tier_stats_with_state(
    state: &Arc<AppState>,
) -> Result<TierStats, AppError> {
    let store = state.vector_store.lock().await;
    store.tier_stats().map_err(AppError::from)
}

pub(crate) async fn count_memory_chunks_with_state(state: &Arc<AppState>) -> Result<i64, AppError> {
    let store = state.vector_store.lock().await;
    store.count_all_chunks().map_err(AppError::from)
}

pub(crate) async fn rebuild_memory_index_with_state(
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    let messages = {
        let store = state.memory_store.lock().await;
        store.export_all_messages().map_err(AppError::from)?
    };
    let previous_vectors = {
        let store = state.vector_store.lock().await;
        store.export_all_chunks().map_err(AppError::from)?
    };

    let mut rebuilt = Vec::<ExportedVectorChunk>::new();
    let mut skipped = 0_usize;
    for message in messages {
        let content = message.content.trim().to_string();
        if content.is_empty() {
            skipped += 1;
            continue;
        }
        let embedding = embed_memory_text_with_privacy(&content, state)
            .await
            .map_err(|e| AppError::internal(format!("rebuild vector index failed: {e}")))?;
        if embedding.is_empty() {
            skipped += 1;
            continue;
        }
        rebuilt.push(ExportedVectorChunk {
            session_id: message.session_id,
            content,
            embedding,
            source: format!("rebuild:{}", message.role),
            created_at: message.created_at,
            tier: 2,
            access_count: 0,
            last_accessed_at: String::new(),
            importance_score: 0.5,
            archived: false,
            archived_at: None,
            summary: None,
        });
    }

    {
        let store = state.vector_store.lock().await;
        if let Err(rebuild_error) = store.replace_all_chunks(&rebuilt) {
            let rollback_error = store.replace_all_chunks(&previous_vectors).err();
            if let Some(rollback_error) = rollback_error {
                return Err(AppError::internal(format!(
                    "rebuild vector index failed and rollback failed. rebuild: {rebuild_error}; rollback: {rollback_error}"
                )));
            }
            return Err(AppError::internal(format!(
                "rebuild vector index failed and rollback completed: {rebuild_error}"
            )));
        }
    }

    Ok(serde_json::json!({
        "processed": rebuilt.len() + skipped,
        "indexed": rebuilt.len(),
        "skipped": skipped,
    }))
}

pub(crate) async fn replace_imported_memory_with_state(
    state: &Arc<AppState>,
    messages: &[openlife_core::memory::ExportedMessage],
    vectors: &[ExportedVectorChunk],
) -> Result<(), AppError> {
    let decision = MemoryGateway::decide(MemoryGatewaySubject::ImportedArchive);
    debug_assert!(decision.local_memory_allowed);
    {
        let store = state.memory_store.lock().await;
        store
            .replace_all_messages(messages)
            .map_err(AppError::from)?;
    }
    {
        let store = state.vector_store.lock().await;
        store.replace_all_chunks(vectors).map_err(AppError::from)?;
    }
    Ok(())
}

pub(crate) async fn persist_vector_memory_for_message_with_state(
    session_id: &str,
    message: &ChatMessage,
    state: &Arc<AppState>,
) {
    if let Some(reason) = state.vector_persistence_mode.skip_reason() {
        log::debug!(
            "[memory] vector persistence skipped for {} message in session {}: {}",
            message.role,
            session_id,
            reason
        );
        return;
    }

    let content = message.content.trim();
    if content.is_empty() {
        return;
    }
    let embedding = match embed_memory_text_with_privacy(content, state).await {
        Ok(embedding) if !embedding.is_empty() => embedding,
        Ok(_) => return,
        Err(e) => {
            log::warn!(
                "[memory] embedding generation failed for {} message in session {}: {}",
                message.role,
                session_id,
                e
            );
            return;
        }
    };
    let store = state.vector_store.lock().await;
    let item = VectorInsertItem {
        session_id,
        content,
        embedding: &embedding,
        source: if message.role == "assistant" {
            "assistant_reply"
        } else {
            "user_message"
        },
    };
    if let Err(e) = store.insert_batch(&[item]) {
        log::warn!(
            "[memory] vector insert failed for {} message in session {}: {}",
            message.role,
            session_id,
            e
        );
    }
}

pub(crate) async fn materialize_memory_proposal_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    content: String,
    session_id: String,
    original_source: String,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    let decision = memory_gateway_decision_for_proposal(
        proposal,
        "accepted_proposal_materialization",
        proposal_memory_evidence_refs(proposal),
    );
    if decision.lane == openlife_core::memory_gateway::MemoryLane::CanonicalLifeModelTruth {
        return Ok(patch_result(
            proposal,
            false,
            "memory_write_blocked",
            Some("canonical_lifemodel_truth_requires_lifemodel_write_gateway".to_string()),
        ));
    }
    let duplicate = {
        let store = state.memory_store.lock().await;
        let hits = store
            .search_text_memories(Some(&session_id), &content, 10)
            .map_err(|e| e.to_string())?;
        hits.iter()
            .any(|hit| hit.chunk.content.trim() == content.trim())
    };
    if duplicate {
        return Ok(patch_result(
            proposal,
            false,
            "memory_write",
            Some("duplicate_memory_content".to_string()),
        ));
    }

    let lifecycle_report = {
        let lifecycle_store = state
            .memory_lifecycle_store
            .as_ref()
            .ok_or_else(memory_lifecycle_store_missing)?;
        let store = lifecycle_store.lock().await;
        let lifecycle_proposal = proposal_with_gateway_memory_category(proposal, &decision);
        store
            .accept_memory_proposal(MemoryLifecycleAcceptanceInput::from_memory_proposal(
                &lifecycle_proposal,
                content.clone(),
            ))
            .map_err(|e| e.to_string())?
    };
    let lifecycle_source = format!("memory_lifecycle:{}", lifecycle_report.record.memory_id);
    let embedding_id = match embed_memory_text_with_privacy(&content, state).await {
        Ok(embedding) if !embedding.is_empty() => {
            let store = state.vector_store.lock().await;
            store
                .insert(&session_id, &content, &embedding, &lifecycle_source)
                .map_err(|e| e.to_string())
                .ok()
        }
        Ok(_) | Err(_) => None,
    };
    {
        let store = state.memory_store.lock().await;
        let tags = vec![
            "proposal".to_string(),
            format!("memory_lane:{}", decision.lane.as_str()),
            format!("memory_gateway_status:{}", decision.status.as_str()),
            format!("proposal_id:{}", proposal.id),
            format!("source:{}", original_source),
            format!("memory_id:{}", lifecycle_report.record.memory_id),
        ];
        store
            .save_memory_record(
                &session_id,
                &content,
                "proposal_memory",
                &lifecycle_source,
                &tags,
                "private",
                embedding_id,
            )
            .map_err(|e| e.to_string())?;
    }

    let _report = MemoryGatewayWriteReport::new(decision)
        .with_proposal(proposal)
        .with_session_id(session_id)
        .with_memory_id(lifecycle_report.record.memory_id)
        .with_optional_embedding_id(embedding_id)
        .with_gateway_write();

    Ok(patch_result(proposal, true, "memory_write", None))
}

pub(crate) fn memory_gateway_decision_for_proposal(
    proposal: &AgentProposal,
    user_intent_kind: &str,
    evidence_refs: Vec<String>,
) -> MemoryGatewayDecision {
    let request = MemoryGatewayRequest::from_proposal(
        proposal,
        &proposal.after,
        user_intent_kind.to_string(),
        evidence_refs,
    );
    MemoryGateway::decide_request(&request)
}

pub(crate) async fn archive_memory_for_proposal_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    ids: &[i64],
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    let archived = {
        let store = state.vector_store.lock().await;
        store.archive_chunks(ids).map_err(|e| e.to_string())?
    };
    if archived == 0 {
        return Ok(patch_result(
            proposal,
            false,
            "memory_archive",
            Some("no_active_memory_chunk_matched".to_string()),
        ));
    }
    Ok(patch_result(proposal, true, "memory_archive", None))
}

pub(crate) async fn rollback_memory_asset_with_state(
    memory_id: String,
    reason: String,
    state: &Arc<AppState>,
) -> Result<MemoryRollbackReport, String> {
    ensure_exact_memory_id(&memory_id)?;
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("rollback_memory_asset requires a rollback reason.".into());
    }
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?;
    let store = lifecycle_store.lock().await;
    let report = store
        .rollback_memory_asset(&memory_id, "user", reason)
        .map_err(|e| e.to_string())?;
    drop(store);

    {
        let memory_store = state.memory_store.lock().await;
        memory_store
            .archive_lifecycle_memory_records(&memory_id)
            .map_err(|e| e.to_string())?;
    }
    {
        let vector_store = state.vector_store.lock().await;
        let lifecycle_source = format!("memory_lifecycle:{memory_id}");
        vector_store
            .archive_chunks_by_source(&lifecycle_source)
            .map_err(|e| e.to_string())?;
    }
    Ok(report)
}

pub(crate) async fn rebuild_materialized_memory_view_with_state(
    scope: Option<MemoryLifecycleScope>,
    state: &Arc<AppState>,
) -> Result<MemoryMaterializedView, String> {
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?;
    let store = lifecycle_store.lock().await;
    store
        .rebuild_materialized_view(scope)
        .map_err(|e| e.to_string())
}

fn memory_lifecycle_store_missing() -> String {
    "MemoryLifecycleStore unavailable; memory lifecycle governance is required.".into()
}

fn proposal_with_gateway_memory_category(
    proposal: &AgentProposal,
    decision: &MemoryGatewayDecision,
) -> AgentProposal {
    let mut proposal = proposal.clone();
    let category = match decision.lane {
        openlife_core::memory_gateway::MemoryLane::EpisodicLifeEvent => "fact",
        openlife_core::memory_gateway::MemoryLane::ProceduralRule => "workflow",
        openlife_core::memory_gateway::MemoryLane::EvidenceRecord => "fact",
        openlife_core::memory_gateway::MemoryLane::SemanticFactPreference
        | openlife_core::memory_gateway::MemoryLane::TurnContext
        | openlife_core::memory_gateway::MemoryLane::CanonicalLifeModelTruth => "preference",
    };
    if let Some(object) = proposal.after.as_object_mut() {
        object
            .entry("category".to_string())
            .or_insert_with(|| serde_json::Value::String(category.to_string()));
        object.insert(
            "memoryGatewayLane".to_string(),
            serde_json::Value::String(decision.lane.as_str().to_string()),
        );
        object.insert(
            "memoryGatewayStatus".to_string(),
            serde_json::Value::String(decision.status.as_str().to_string()),
        );
    }
    proposal
}

fn proposal_memory_evidence_refs(proposal: &AgentProposal) -> Vec<String> {
    let mut refs = Vec::new();
    if let Some(run_id) = proposal.run_id.as_ref() {
        refs.push(format!("agent_run:{run_id}"));
    }
    if let Some(evidence_id) = proposal
        .after
        .get("evidence_id")
        .or_else(|| proposal.after.get("evidenceId"))
        .and_then(serde_json::Value::as_str)
    {
        refs.push(format!("evidence:{evidence_id}"));
    }
    refs
}

fn ensure_exact_memory_id(memory_id: &str) -> Result<(), String> {
    let trimmed = memory_id.trim();
    if trimmed != memory_id
        || trimmed.is_empty()
        || !trimmed.starts_with("memory:")
        || trimmed
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(
            "memory_id must be an exact metadata-safe memory lifecycle id without whitespace."
                .into(),
        );
    }
    Ok(())
}

fn patch_result(
    proposal: &AgentProposal,
    success: bool,
    operation: &str,
    error: Option<String>,
) -> openlife_core::life_model::patch::PatchApplyResult {
    openlife_core::life_model::patch::PatchApplyResult {
        patch_id: proposal.id.clone(),
        success,
        path: proposal.affected_path.clone(),
        operation: operation.to_string(),
        error,
    }
}

trait MemoryGatewayWriteReportExt {
    fn with_embedding_id(self, embedding_id: i64) -> Self;
    fn with_optional_embedding_id(self, embedding_id: Option<i64>) -> Self;
    fn with_memory_id(self, memory_id: impl Into<String>) -> Self;
    fn with_gateway_write(self) -> Self;
}

impl MemoryGatewayWriteReportExt for MemoryGatewayWriteReport {
    fn with_embedding_id(mut self, embedding_id: i64) -> Self {
        self.embedding_id = Some(embedding_id);
        self
    }

    fn with_optional_embedding_id(mut self, embedding_id: Option<i64>) -> Self {
        self.embedding_id = embedding_id;
        self
    }

    fn with_memory_id(mut self, memory_id: impl Into<String>) -> Self {
        self.memory_id = Some(memory_id.into());
        self
    }

    fn with_gateway_write(mut self) -> Self {
        self.direct_store_write = false;
        self
    }
}
