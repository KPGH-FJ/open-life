use std::collections::HashMap;
use std::sync::Arc;

use openlife_core::agent::{MemoryLifecycleCategory, MemoryLifecycleRecord};
use openlife_core::config::AgentRuntimeMode;
use openlife_core::embedding::{
    execute_embedding, prepare_embedding_request_recorded, EmbeddingInvocationReceipt,
    EmbeddingProfile, EmbeddingRouteConfig, PreparedEmbeddingRequestOutcome,
};
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::memory::MemorySearchHit;
use openlife_core::privacy::PrivacyEngine;
use openlife_core::vectors::{
    plan_embedding_privacy, CanonicalVectorOwnerRef, MemoryChunk, VectorRebuildEvidence,
    VectorSearchOutcome,
};

use crate::main_chat_hs_runtime::classify_hs_policy_topic;
use crate::memory_gateway::{
    prepare_memory_search_access_telemetry, prepare_vector_search_access_telemetry,
    record_text_search_access_telemetry_with_state,
    record_vector_search_access_telemetry_with_state, MemoryVectorDegradedEvidence,
};
use crate::AppState;

const MEMORY_LIFECYCLE_SOURCE_PREFIX: &str = "memory_lifecycle:";
const MEMORY_LIFECYCLE_CANDIDATE_LIMIT: i64 = 25;
const MEMORY_LIFECYCLE_CONTEXT_LIMIT: usize = 5;

async fn search_session_vectors_with_optional_telemetry(
    state: &Arc<AppState>,
    session_id: &str,
    embedding: &[f32],
    profile: &EmbeddingProfile,
    top_k: usize,
    limit: usize,
) -> anyhow::Result<(VectorSearchOutcome, Option<MemoryVectorDegradedEvidence>)> {
    let telemetry_ticket = prepare_vector_search_access_telemetry(state);
    let store = state.vector_store.lock().await.clone();
    let outcome = store.search_by_session(session_id, embedding, profile, top_k, limit)?;
    let telemetry_evidence = match &outcome {
        VectorSearchOutcome::Matches { matches, .. } => {
            record_vector_search_access_telemetry_with_state(matches, state, telemetry_ticket).await
        }
        VectorSearchOutcome::RebuildRequired(_) => None,
    };
    Ok((outcome, telemetry_evidence))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapabilityPrivacyMode {
    ExistingDefault,
    CapabilityFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MainChatPreprocessOptions {
    pub capability_privacy_mode: CapabilityPrivacyMode,
}

impl Default for MainChatPreprocessOptions {
    fn default() -> Self {
        Self {
            capability_privacy_mode: CapabilityPrivacyMode::ExistingDefault,
        }
    }
}

impl MainChatPreprocessOptions {
    pub(crate) fn from_runtime_mode(mode: &AgentRuntimeMode) -> Self {
        Self {
            capability_privacy_mode: match mode {
                AgentRuntimeMode::LocalFirstDefault => CapabilityPrivacyMode::ExistingDefault,
                AgentRuntimeMode::CapabilityFirst => CapabilityPrivacyMode::CapabilityFirst,
            },
        }
    }
}

pub(crate) async fn filter_canonical_retrievable_memory_results(
    results: Vec<(MemoryChunk, f32)>,
    state: &Arc<AppState>,
) -> Result<Vec<(MemoryChunk, f32)>, String> {
    let lifecycle_reader = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(|| "memory_retrieval_degraded:lifecycle_store_unavailable".to_string())?
        .lock()
        .await
        .retrieval_reader();
    lifecycle_reader.ensure_available().map_err(|error| {
        format!("memory_retrieval_degraded:lifecycle_store_query_failed:{error}")
    })?;
    let lifecycle_ids = results
        .iter()
        .filter_map(|(chunk, _)| lifecycle_memory_id_from_source(&chunk.source))
        .collect::<Vec<_>>();
    let mut active_lifecycle_ids = std::collections::HashSet::new();
    for memory_id in lifecycle_ids {
        if lifecycle_reader
            .is_memory_retrievable(memory_id)
            .map_err(|error| {
                format!("memory_retrieval_degraded:lifecycle_state_query_failed:{error}")
            })?
        {
            active_lifecycle_ids.insert(memory_id.to_string());
        }
    }
    let verified_canonical_retrieval_sources = {
        let store = state.memory_store.lock().await;
        let mut verified = std::collections::HashSet::new();
        for (chunk, _) in &results {
            let Some(owner) = canonical_memory_owner_from_source(&chunk.source) else {
                continue;
            };
            let proven = store
                .is_verified_canonical_memory_owner(&owner)
                .map_err(|error| {
                    format!("memory_retrieval_degraded:owner_proof_query_failed:{error}")
                })?;
            let retrieval_active = if owner.kind() == "memory_lifecycle" {
                // Lifecycle disposition belongs exclusively to the reader
                // above. A stale MemoryStore row cannot override it.
                true
            } else {
                store.is_memory_retrieval_active(&owner).map_err(|error| {
                    format!("memory_retrieval_degraded:retrieval_state_query_failed:{error}")
                })?
            };
            if proven && retrieval_active {
                verified.insert(chunk.source.clone());
            }
        }
        verified
    };
    Ok(results
        .into_iter()
        .filter(|(chunk, _)| {
            if let Some(memory_id) = lifecycle_memory_id_from_source(&chunk.source) {
                return active_lifecycle_ids.contains(memory_id)
                    && verified_canonical_retrieval_sources.contains(&chunk.source);
            }
            if chunk.source.starts_with("knowledge_note:")
                || chunk.source.starts_with("memory_record:")
            {
                return verified_canonical_retrieval_sources.contains(&chunk.source);
            }
            true
        })
        .collect())
}

fn canonical_memory_owner_from_source(source: &str) -> Option<CanonicalVectorOwnerRef> {
    for kind in ["knowledge_note", "memory_record", "memory_lifecycle"] {
        if let Some(id) = source.strip_prefix(&format!("{kind}:")) {
            return CanonicalVectorOwnerRef::new(kind, id).ok();
        }
    }
    None
}

fn lifecycle_memory_id_from_source(source: &str) -> Option<&str> {
    let memory_id = source.strip_prefix(MEMORY_LIFECYCLE_SOURCE_PREFIX)?;
    if memory_id.starts_with("memory:")
        && !memory_id.is_empty()
        && !memory_id
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        Some(memory_id)
    } else {
        None
    }
}

fn relevant_active_lifecycle_records(
    records: Vec<MemoryLifecycleRecord>,
    query: &str,
    limit: usize,
) -> Vec<MemoryLifecycleRecord> {
    records
        .into_iter()
        .filter(|record| is_lifecycle_record_relevant(record, query))
        .take(limit)
        .collect()
}

fn is_lifecycle_record_relevant(record: &MemoryLifecycleRecord, query: &str) -> bool {
    if record.runtime_context_excluded_at.is_some() {
        return false;
    }

    let terms = relevance_terms(query);
    if terms.is_empty() {
        return false;
    }

    let haystack = format!(
        "{} {} {} {}",
        record.content,
        record.scope,
        record.category,
        record.evidence_ids.join(" ")
    )
    .to_lowercase();

    if terms.iter().any(|term| haystack.contains(term)) {
        return true;
    }

    record.category == MemoryLifecycleCategory::Boundary
        && terms.iter().any(|term| {
            matches!(
                term.as_str(),
                "安全"
                    | "边界"
                    | "权限"
                    | "隐私"
                    | "外部"
                    | "写入"
                    | "删除"
                    | "api"
                    | "key"
                    | "token"
            )
        })
}

fn relevance_terms(value: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut ascii = String::new();
    let mut cjk = String::new();

    let flush_ascii = |buffer: &mut String, terms: &mut Vec<String>| {
        if buffer.len() >= 3 {
            terms.push(buffer.to_lowercase());
        }
        buffer.clear();
    };
    let flush_cjk = |buffer: &mut String, terms: &mut Vec<String>| {
        let chars = buffer.chars().collect::<Vec<_>>();
        if chars.len() >= 2 {
            for window in chars.windows(2) {
                let token = window.iter().collect::<String>();
                if !is_common_cjk_token(&token) {
                    terms.push(token);
                }
            }
        }
        buffer.clear();
    };

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            flush_cjk(&mut cjk, &mut terms);
            ascii.push(ch);
        } else if is_cjk(ch) {
            flush_ascii(&mut ascii, &mut terms);
            cjk.push(ch);
        } else {
            flush_ascii(&mut ascii, &mut terms);
            flush_cjk(&mut cjk, &mut terms);
        }
    }
    flush_ascii(&mut ascii, &mut terms);
    flush_cjk(&mut cjk, &mut terms);

    terms.sort();
    terms.dedup();
    terms
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
    )
}

fn is_common_cjk_token(token: &str) -> bool {
    matches!(
        token,
        "这个"
            | "那个"
            | "什么"
            | "现在"
            | "今天"
            | "明天"
            | "昨天"
            | "当前"
            | "请你"
            | "帮我"
            | "一下"
            | "可以"
            | "需要"
            | "回复"
            | "问题"
            | "测试"
            | "输入"
            | "输出"
            | "进行"
            | "一个"
            | "我们"
    )
}

#[allow(dead_code)]
/// Shared preprocessing for chat commands:
/// loads model/tools/config, applies privacy filter,
/// values filter, and vector memory retrieval.
pub(crate) async fn preprocess_chat_input(
    session_id: &str,
    messages: &[ChatMessage],
    state: &Arc<AppState>,
) -> Result<
    (
        LifeModel,
        String,
        PrivacyEngine,
        HashMap<String, String>,
        Vec<ChatMessage>,
        Option<String>,
        openlife_core::agent::types::ContextSummary,
    ),
    String,
> {
    preprocess_chat_input_with_options(
        session_id,
        messages,
        state,
        MainChatPreprocessOptions::default(),
    )
    .await
}

/// Shared preprocessing for chat commands:
/// loads model/tools/config, applies privacy filter,
/// values filter, and vector memory retrieval.
pub(crate) async fn preprocess_chat_input_with_options(
    session_id: &str,
    messages: &[ChatMessage],
    state: &Arc<AppState>,
    options: MainChatPreprocessOptions,
) -> Result<
    (
        LifeModel,
        String,
        PrivacyEngine,
        HashMap<String, String>,
        Vec<ChatMessage>,
        Option<String>,
        openlife_core::agent::types::ContextSummary,
    ),
    String,
> {
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };

    {
        let mut cache = state.hot_cache.write().await;
        if cache.is_stale(&life_model) {
            cache.refresh(&life_model);
        }
    }

    let tools_prompt = {
        let reg = state.mcp_registry.lock().await;
        reg.tools_prompt()
    };

    let privacy_engine = state.privacy_engine.lock().await.clone();
    let mut desensitized_messages = Vec::new();
    let mut privacy_map = HashMap::new();
    for msg in messages {
        if msg.role == "user" {
            let hs_local_only = classify_hs_policy_topic(&msg.content, &tools_prompt)
                != openlife_core::agent::PolicyTopic::General;
            let (masked, map) = sanitize_for_capability_privacy_mode(
                &privacy_engine,
                &msg.content,
                options.capability_privacy_mode,
                hs_local_only,
            );
            privacy_map.extend(map);
            let mut final_text = masked;
            if openlife_core::core_value_signal_extractor::contains_core_value_signal(&msg.content)
            {
                final_text = format!("[该消息涉及你的核心价值观] {}", final_text);
            }
            desensitized_messages.push(ChatMessage {
                role: msg.role.clone(),
                content: final_text,
            });
        } else {
            desensitized_messages.push(msg.clone());
        }
    }

    let (
        provider,
        openai_base,
        openai_key,
        embedding_model,
        embedding_enabled,
        credential_version,
        network_policy,
    ) = {
        let cfg = state.config.lock().await;
        (
            cfg.llm.provider.clone(),
            cfg.llm.openai_base.clone(),
            cfg.llm.openai_key.clone(),
            cfg.llm.embedding_model.clone(),
            cfg.llm.embedding_enabled,
            cfg.llm.credential_version,
            cfg.system.network_policy.clone(),
        )
    };

    let mut embed_err = None;
    let mut memory_sources: Vec<String> = Vec::new();
    let mut memory_hit_count = 0usize;
    let memory_top_k = {
        let cfg = state.config.lock().await;
        cfg.system.memory_search_top_k
    };
    let memory_context = if let Some(user_msg) = messages.last() {
        if user_msg.role == "user" {
            let hs_local_only = classify_hs_policy_topic(&user_msg.content, &tools_prompt)
                != openlife_core::agent::PolicyTopic::General;
            let (memory_query, _) = sanitize_for_capability_privacy_mode(
                &privacy_engine,
                &user_msg.content,
                options.capability_privacy_mode,
                hs_local_only,
            );
            let text_telemetry_ticket = prepare_memory_search_access_telemetry(state);
            let text_hits = {
                let store = state.memory_store.lock().await;
                store
                    .search_text_memories(Some(session_id), &memory_query, memory_top_k)
                    .map_err(|error| {
                        format!("memory_retrieval_degraded:memory_store_query_failed:{error}")
                    })?
            };

            let hs_local_only = classify_hs_policy_topic(&user_msg.content, &tools_prompt)
                != openlife_core::agent::PolicyTopic::General;
            let vector_hits = match prepare_embedding_request_recorded(
                &memory_query,
                EmbeddingRouteConfig::from_product_config(
                    provider.clone(),
                    openai_base.clone(),
                    embedding_model.clone(),
                    embedding_enabled,
                    &openai_key,
                    credential_version,
                    network_policy.clone(),
                ),
                plan_embedding_privacy(&memory_query, &privacy_engine, hs_local_only),
            ) {
                PreparedEmbeddingRequestOutcome::Rejected(outcome) => {
                    embed_err = Some(embedding_runtime_evidence(
                        "vector_memory_embedding_prepare_failed",
                        &outcome.profile,
                        &outcome.receipt,
                    ));
                    Vec::new()
                }
                PreparedEmbeddingRequestOutcome::Prepared(prepared) => {
                    let outcome = execute_embedding(prepared).await;
                    let profile = outcome.profile;
                    let receipt = outcome.receipt;
                    match outcome.result {
                        Err(_) => {
                            embed_err = Some(embedding_runtime_evidence(
                                "vector_memory_embedding_failed",
                                &profile,
                                &receipt,
                            ));
                            Vec::new()
                        }
                        Ok(embedding) => {
                            match search_session_vectors_with_optional_telemetry(
                                state,
                                session_id,
                                &embedding,
                                &profile,
                                memory_top_k,
                                1000,
                            )
                            .await
                            {
                                Ok((
                                    VectorSearchOutcome::Matches { matches, rebuild },
                                    telemetry,
                                )) => {
                                    if let Some(rebuild) = rebuild {
                                        embed_err = Some(vector_rebuild_evidence(
                                            "vector_memory_search",
                                            &receipt,
                                            rebuild,
                                        ));
                                    }
                                    if let Some(telemetry) = telemetry {
                                        append_runtime_evidence(
                                            &mut embed_err,
                                            serde_json::json!({
                                                "operation": "vector_memory_access_telemetry",
                                                "status": "skipped",
                                                "reasonCode": telemetry.reason_code,
                                                "errorDigest": telemetry.error_digest,
                                            }),
                                        );
                                    }
                                    matches
                                }
                                Ok((VectorSearchOutcome::RebuildRequired(rebuild), _)) => {
                                    embed_err = Some(vector_rebuild_evidence(
                                        "vector_memory_search",
                                        &receipt,
                                        rebuild,
                                    ));
                                    Vec::new()
                                }
                                Err(error) => {
                                    embed_err = Some(format!(
                                        "vector_memory_search_failed: {error}; {}",
                                        embedding_runtime_evidence(
                                            "vector_memory_search",
                                            &profile,
                                            &receipt,
                                        )
                                    ));
                                    Vec::new()
                                }
                            }
                        }
                    }
                }
            };

            if let Some(telemetry) = record_text_search_access_telemetry_with_state(
                &text_hits,
                state,
                text_telemetry_ticket,
            )
            .await
            {
                append_runtime_evidence(
                    &mut embed_err,
                    serde_json::json!({
                        "operation": "text_memory_access_telemetry",
                        "status": "skipped",
                        "reasonCode": telemetry.reason_code,
                        "errorDigest": telemetry.error_digest,
                    }),
                );
            }

            let results = filter_canonical_retrievable_memory_results(
                merge_memory_hits(vector_hits, text_hits, memory_top_k),
                state,
            )
            .await?;
            let lifecycle_store = state.memory_lifecycle_store.as_ref().ok_or_else(|| {
                "memory_retrieval_degraded:lifecycle_store_unavailable".to_string()
            })?;
            let active_lifecycle_records = {
                let store = lifecycle_store.lock().await;
                relevant_active_lifecycle_records(
                    store
                        .list_retrievable_records(None, MEMORY_LIFECYCLE_CANDIDATE_LIMIT)
                        .map_err(|error| {
                            format!(
                                "memory_retrieval_degraded:lifecycle_records_query_failed:{error}"
                            )
                        })?,
                    &memory_query,
                    MEMORY_LIFECYCLE_CONTEXT_LIMIT,
                )
            };
            memory_hit_count = results.len() + active_lifecycle_records.len();
            memory_sources = results
                .iter()
                .map(|(chunk, _)| chunk.source.clone())
                .chain(
                    active_lifecycle_records
                        .iter()
                        .map(|record| format!("memory_lifecycle:{}", record.memory_id)),
                )
                .collect();
            if results.is_empty() && active_lifecycle_records.is_empty() {
                String::new()
            } else {
                let mut snippets: Vec<String> = results
                    .iter()
                    .map(|(chunk, score)| {
                        format!(
                            "- [{}] {} (相关度: {:.2})",
                            chunk.source,
                            chunk.content.replace('\n', " "),
                            score
                        )
                    })
                    .collect();
                for record in active_lifecycle_records {
                    snippets.push(format!(
                        "- [memory_lifecycle:{}] Accepted memory [{}:{}]: {}",
                        record.memory_id,
                        record.scope,
                        record.category,
                        record.content.replace('\n', " ")
                    ));
                }
                format!(
                    "\n以下是已确认且与当前问题相关的 active 记忆/历史检索结果。仅在和当前用户指令直接相关时参考它们；当前用户指令优先，不要把 rejected、rolled-back 或无关历史当作事实：\n{}",
                    snippets.join("\n")
                )
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let hot_context = {
        let cache = state.hot_cache.read().await;
        cache.to_context_string()
    };
    if !hot_context.is_empty() {
        desensitized_messages.insert(
            0,
            ChatMessage {
                role: "system".into(),
                content: hot_context,
            },
        );
    }

    if !memory_context.is_empty() {
        if let Some(last_user) = desensitized_messages.iter_mut().rfind(|m| m.role == "user") {
            last_user.content = format!("{}\n\n{}", last_user.content, memory_context);
        }
    }

    let context_summary = openlife_core::agent::types::ContextSummary {
        life_model_empty: life_model.identity.name.is_empty(),
        included_life_model_sections: vec![
            "identity".to_string(),
            "goals".to_string(),
            "capabilities".to_string(),
            "state".to_string(),
        ],
        memory_hit_count: memory_hit_count as i64,
        memory_sources,
        used_tools_prompt: !tools_prompt.is_empty(),
        redaction_applied: !privacy_map.is_empty(),
        redaction_level: if privacy_map.is_empty() {
            openlife_core::agent::types::RedactionLevel::None
        } else {
            openlife_core::agent::types::RedactionLevel::Light
        },
    };

    Ok((
        life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        context_summary,
    ))
}

fn embedding_runtime_evidence(
    operation: &str,
    profile: &EmbeddingProfile,
    receipt: &EmbeddingInvocationReceipt,
) -> String {
    serde_json::json!({
        "operation": operation,
        "profileId": profile.id,
        "profileRoute": profile.route,
        "profileDimension": profile.dimension,
        "receiptStatus": receipt.status,
        "receiptSource": receipt.source,
        "routeReasonCode": receipt.route_reason_code,
        "cacheHit": receipt.cache_hit,
        "providerDispatches": receipt.provider_dispatches,
        "errorDigest": receipt.error_digest,
    })
    .to_string()
}

fn append_runtime_evidence(target: &mut Option<String>, additional: serde_json::Value) {
    let Some(existing) = target.take() else {
        *target = Some(additional.to_string());
        return;
    };
    let existing = serde_json::from_str::<serde_json::Value>(&existing)
        .unwrap_or(serde_json::Value::String(existing));
    *target = Some(
        serde_json::json!({
            "kind": "runtime_evidence_bundle",
            "entries": [existing, additional],
        })
        .to_string(),
    );
}

fn vector_rebuild_evidence(
    operation: &str,
    receipt: &EmbeddingInvocationReceipt,
    evidence: VectorRebuildEvidence,
) -> String {
    let VectorRebuildEvidence {
        expected_profile_id,
        expected_dimension,
        incompatible_profiles,
        unknown_profile_count,
        profile_mismatch_count,
        dimension_mismatch_count,
        corrupt_embedding_count,
    } = evidence;
    serde_json::json!({
        "operation": operation,
        "status": "rebuild_required",
        "expectedProfileId": expected_profile_id,
        "expectedDimension": expected_dimension,
        "incompatibleProfiles": incompatible_profiles,
        "unknownProfileCount": unknown_profile_count,
        "profileMismatchCount": profile_mismatch_count,
        "dimensionMismatchCount": dimension_mismatch_count,
        "corruptEmbeddingCount": corrupt_embedding_count,
        "embeddingReceiptStatus": receipt.status,
        "embeddingReceiptSource": receipt.source,
    })
    .to_string()
}

fn sanitize_for_capability_privacy_mode(
    privacy_engine: &PrivacyEngine,
    content: &str,
    mode: CapabilityPrivacyMode,
    hs_local_only: bool,
) -> (String, HashMap<String, String>) {
    if mode == CapabilityPrivacyMode::CapabilityFirst && !hs_local_only {
        privacy_engine.desensitize_secrets_only(content)
    } else {
        privacy_engine.desensitize(content)
    }
}

pub(crate) fn merge_memory_hits(
    vector_hits: Vec<(MemoryChunk, f32)>,
    text_hits: Vec<MemorySearchHit>,
    top_k: usize,
) -> Vec<(MemoryChunk, f32)> {
    let mut merged: HashMap<(String, String), (MemoryChunk, f32)> = HashMap::new();

    for (chunk, score) in vector_hits {
        let key = (chunk.session_id.clone(), chunk.content.clone());
        merged
            .entry(key)
            .and_modify(|(_, existing_score)| *existing_score = existing_score.max(score))
            .or_insert((chunk, score));
    }

    for hit in text_hits {
        let key = (hit.chunk.session_id.clone(), hit.chunk.content.clone());
        merged
            .entry(key)
            .and_modify(|(_, existing_score)| {
                *existing_score = existing_score.max(hit.relevance_score)
            })
            .or_insert((hit.chunk, hit.relevance_score));
    }

    let mut results: Vec<_> = merged.into_values().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{
        MemoryLifecycleCategory, MemoryLifecycleRiskLevel, MemoryLifecycleScope,
        MemoryLifecycleSensitivity, MemoryLifecycleStatus, MemoryMaterializationStatus,
    };

    fn lifecycle_record(memory_id: &str, content: &str) -> MemoryLifecycleRecord {
        MemoryLifecycleRecord {
            memory_id: memory_id.to_string(),
            proposal_id: format!("proposal-{memory_id}"),
            source_task_session_id: None,
            source_run_id: None,
            content: content.to_string(),
            scope: MemoryLifecycleScope::Global,
            category: MemoryLifecycleCategory::Fact,
            risk_level: MemoryLifecycleRiskLevel::Low,
            sensitivity: MemoryLifecycleSensitivity::Internal,
            audit_digest: "sha256:test-memory-preprocess".into(),
            status: MemoryLifecycleStatus::Materialized,
            materialization_status: MemoryMaterializationStatus::Materialized,
            materialization_error_code: None,
            created_by: "test".into(),
            accepted_by: Some("test".into()),
            accepted_at: None,
            materialized_view_id: Some("view".into()),
            materialized_view_version: Some(1),
            evidence_ids: vec![],
            confidence: 0.9,
            conflict_ids: vec![],
            supersedes_memory_id: None,
            replacement_memory_id: None,
            rolled_back_by_event_id: None,
            runtime_context_excluded_at: None,
        }
    }

    fn accepted_lifecycle_memory_proposal(content: &str) -> openlife_core::agent::AgentProposal {
        let mut proposal = openlife_core::agent::AgentProposal::new(
            openlife_core::agent::ProposalType::MemoryWrite,
            "memory.candidates",
            serde_json::json!({
                "content": content,
                "scope": "global",
                "category": "fact",
                "candidateKind": "semantic_user_fact",
                "riskLevel": "low",
                "sensitivity": "internal"
            }),
            "User reviewed a lifecycle Memory candidate.",
            0.9,
            openlife_core::agent::RiskLevel::Low,
            openlife_core::agent::ProposalSource::Manual,
        );
        proposal.id = format!("proposal:preprocess:retrieval:{}", uuid::Uuid::new_v4());
        proposal
    }

    #[test]
    fn lifecycle_memory_filter_excludes_unrelated_active_records() {
        let records = vec![lifecycle_record(
            "memory:kaizhou",
            "用户曾询问重庆开州的位置和天气。",
        )];

        let filtered = relevant_active_lifecycle_records(
            records,
            "QA-2026-06-21 current-ui smoke: reply with one short sentence.",
            5,
        );

        assert!(
            filtered.is_empty(),
            "unrelated active lifecycle memory must not pollute an unrelated current-ui smoke prompt"
        );
    }

    #[test]
    fn lifecycle_memory_filter_keeps_query_relevant_records() {
        let records = vec![
            lifecycle_record("memory:kaizhou", "用户曾询问重庆开州的位置和天气。"),
            lifecycle_record("memory:other", "用户正在测试 current-ui。"),
        ];

        let filtered = relevant_active_lifecycle_records(records, "重庆开州在哪里？", 5);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].memory_id, "memory:kaizhou");
    }

    #[tokio::test]
    async fn lifecycle_vector_body_is_excluded_by_canonical_archive_before_projection() {
        const SENTINEL: &str = "LIFECYCLE_VECTOR_ARCHIVE_LAG_SENTINEL";
        let state = crate::test_utils::test_app_state();
        let proposal = accepted_lifecycle_memory_proposal(SENTINEL);
        let accepted = {
            let store = state
                .memory_lifecycle_store
                .as_ref()
                .expect("lifecycle store")
                .lock()
                .await;
            store
                .accept_memory_proposal(
                    openlife_core::agent::MemoryLifecycleAcceptanceInput::from_memory_proposal(
                        &proposal,
                        SENTINEL.to_string(),
                    )
                    .expect("typed lifecycle acceptance"),
                )
                .expect("accept lifecycle memory")
        };
        let creation_event = accepted
            .canonical_mutation
            .as_ref()
            .expect("lifecycle creation outbox")
            .event_id
            .clone();
        state
            .memory_store
            .lock()
            .await
            .project_lifecycle_memory(
                &creation_event,
                &accepted.record.memory_id,
                "preprocess-retrieval-session",
                SENTINEL,
                "lifecycle_memory_projection",
                &[
                    "canonical_owner:memory_lifecycle".into(),
                    format!("memory_id:{}", accepted.record.memory_id),
                    format!("proposal_id:{}", accepted.record.proposal_id),
                    format!("memory_category:{}", accepted.record.category),
                ],
                "private",
                None,
            )
            .expect("project lifecycle compatibility owner");
        let raw_body = MemoryChunk {
            id: 7,
            session_id: "preprocess-retrieval-session".into(),
            content: SENTINEL.into(),
            source: format!("memory_lifecycle:{}", accepted.record.memory_id),
            created_at: chrono::Utc::now().to_rfc3339(),
            tier: 2,
            access_count: 0,
            last_accessed_at: String::new(),
            importance_score: 0.5,
            archived: false,
            archived_at: None,
            summary: None,
        };
        assert_eq!(
            filter_canonical_retrievable_memory_results(vec![(raw_body.clone(), 1.0)], &state)
                .await
                .unwrap()
                .len(),
            1
        );

        let archived = {
            let store = state
                .memory_lifecycle_store
                .as_ref()
                .expect("lifecycle store")
                .lock()
                .await;
            let archived = store
                .set_memory_retrieval_disposition(
                    &accepted.record.memory_id,
                    openlife_core::memory::MemoryRetrievalDisposition::Archived,
                    "user_reviewed_archive",
                )
                .expect("archive lifecycle owner");
            assert_eq!(
                store
                    .projection_summary(
                        &archived
                            .canonical_mutation
                            .as_ref()
                            .expect("archive outbox")
                            .event_id,
                    )
                    .expect("archive projection summary")
                    .pending,
                1
            );
            archived
        };
        assert!(archived.changed);
        assert!(
            !raw_body.archived,
            "derived vector body intentionally remains unprojected"
        );
        assert!(
            filter_canonical_retrievable_memory_results(vec![(raw_body, 1.0)], &state)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn canonical_filter_reports_missing_lifecycle_authority() {
        let mut state = crate::test_utils::test_app_state();
        Arc::get_mut(&mut state)
            .expect("isolated state owner")
            .memory_lifecycle_store = None;

        let error = filter_canonical_retrievable_memory_results(Vec::new(), &state)
            .await
            .expect_err("missing lifecycle authority must not become a healthy empty result");
        assert_eq!(
            error,
            "memory_retrieval_degraded:lifecycle_store_unavailable"
        );
    }

    #[test]
    fn capability_privacy_mode_preserves_ordinary_personal_context() {
        let engine = PrivacyEngine::new();
        let text = "我叫张三，偏好深度工作，项目目标是完成 OpenLife 能力版。";

        let (existing_default, _) = sanitize_for_capability_privacy_mode(
            &engine,
            text,
            CapabilityPrivacyMode::ExistingDefault,
            false,
        );
        let (capability_first, map) = sanitize_for_capability_privacy_mode(
            &engine,
            text,
            CapabilityPrivacyMode::CapabilityFirst,
            false,
        );

        assert!(!existing_default.contains("张三"));
        assert!(capability_first.contains("张三"));
        assert!(capability_first.contains("深度工作"));
        assert!(capability_first.contains("OpenLife 能力版"));
        assert!(map.is_empty());
    }

    #[test]
    fn capability_privacy_mode_redacts_credentials() {
        let engine = PrivacyEngine::new();
        let text = [
            "API key: sk-test-secret-123456",
            "password=hunter2-secret",
            "token abcdefghijkl",
            "-----BEGIN PRIVATE KEY-----\nabc123\n-----END PRIVATE KEY-----",
        ]
        .join("\n");

        let (masked, map) = sanitize_for_capability_privacy_mode(
            &engine,
            &text,
            CapabilityPrivacyMode::CapabilityFirst,
            false,
        );

        for secret in [
            "sk-test-secret-123456",
            "hunter2-secret",
            "abcdefghijkl",
            "abc123",
        ] {
            assert!(
                !masked.contains(secret),
                "capability mode leaked credential marker {secret}"
            );
            assert!(
                !map.values().any(|value| value.contains(secret)),
                "privacy reconstruction map retained credential marker {secret}"
            );
        }
        assert!(masked.contains("<SECRET_"));
        assert!(!map.is_empty());
    }

    #[test]
    fn capability_privacy_mode_keeps_sensitive_local_only_stricter() {
        let engine = PrivacyEngine::new();
        let text = "我叫张三，电话 13800138000。";

        let (masked, map) = sanitize_for_capability_privacy_mode(
            &engine,
            text,
            CapabilityPrivacyMode::CapabilityFirst,
            true,
        );

        assert!(!masked.contains("张三"));
        assert!(!masked.contains("13800138000"));
        assert!(masked.contains("<NAME_") || masked.contains("<PHONE_"));
        assert!(!map.is_empty());
    }

    #[tokio::test]
    async fn canonical_main_chat_preprocessor_surfaces_legacy_vector_rebuild_evidence() {
        openlife_core::embedding::clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        {
            let mut cfg = state.config.lock().await;
            cfg.llm.provider = "deepseek".into();
            cfg.llm.openai_key = "test-key".into();
            cfg.llm.embedding_enabled = true;
        }
        {
            let store = state.vector_store.lock().await;
            store
                .import_chunks(&[openlife_core::vectors::ExportedVectorChunk {
                    session_id: "legacy-main-chat".into(),
                    content: "legacy vector without profile".into(),
                    embedding: vec![0.1, 0.2, 0.3, 0.4],
                    embedding_profile_id: openlife_core::embedding::UNKNOWN_EMBEDDING_PROFILE_ID
                        .into(),
                    embedding_dimension: 0,
                    source: "note".into(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    tier: 2,
                    access_count: 0,
                    last_accessed_at: String::new(),
                    importance_score: 0.5,
                    archived: false,
                    archived_at: None,
                    summary: None,
                }])
                .unwrap();
        }
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "find legacy vector".into(),
        }];

        let direct = preprocess_chat_input_with_options(
            "legacy-main-chat",
            &messages,
            &state,
            MainChatPreprocessOptions::default(),
        )
        .await
        .unwrap();
        let evidence = direct.5.expect("rebuild evidence must reach Main Chat");
        assert!(evidence.contains("rebuild_required"), "{evidence}");
        assert!(evidence.contains("\"unknownProfileCount\":1"), "{evidence}");
    }

    #[tokio::test]
    async fn main_chat_session_search_commits_explicit_telemetry_after_the_read() {
        let state = crate::test_utils::test_app_state();
        let profile = EmbeddingProfile::new(
            openlife_core::embedding::EmbeddingRouteKind::DeterministicHash,
            "openlife-test",
            "main-chat-telemetry-v1",
            "builtin:test",
            "main-chat-telemetry-artifact-v1",
            4,
        )
        .unwrap();
        let embedding = [1.0, 0.0, 0.0, 0.0];
        let store = state.vector_store.lock().await.clone();
        store
            .insert(
                "main-chat-telemetry-session",
                "MAIN_CHAT_RESULT_MUST_SURVIVE_TELEMETRY_FENCE",
                &embedding,
                &profile,
                "manual_note",
            )
            .unwrap();
        let (outcome, telemetry) = search_session_vectors_with_optional_telemetry(
            &state,
            "main-chat-telemetry-session",
            &embedding,
            &profile,
            5,
            100,
        )
        .await
        .expect("pure session search must remain available");
        let matches = match outcome {
            VectorSearchOutcome::Matches { matches, .. } => matches,
            VectorSearchOutcome::RebuildRequired(evidence) => {
                panic!("test vector profile unexpectedly requires rebuild: {evidence:?}")
            }
        };
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0].0.content,
            "MAIN_CHAT_RESULT_MUST_SURVIVE_TELEMETRY_FENCE"
        );
        assert!(
            telemetry.is_none(),
            "healthy telemetry must not be degraded"
        );
        let stored = store.export_all_chunks().unwrap();
        assert_eq!(stored[0].access_count, 1);
        assert!(!stored[0].last_accessed_at.is_empty());
    }

    #[tokio::test]
    async fn main_chat_text_search_commits_explicit_telemetry_after_provider_work() {
        openlife_core::embedding::clear_embedding_cache();
        let state = crate::test_utils::test_app_state();
        state.config.lock().await.llm.embedding_enabled = false;
        let store = state.memory_store.lock().await.clone();
        let memory_id = store
            .save_memory_record(
                "main-chat-text-telemetry-session",
                "MAIN_CHAT_TEXT_TELEMETRY_SENTINEL",
                "note",
                "manual",
                &[],
                "private",
                None,
            )
            .unwrap();
        let before = store.vector_rebuild_source_snapshot().unwrap();
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "MAIN_CHAT_TEXT_TELEMETRY_SENTINEL".into(),
        }];

        preprocess_chat_input_with_options(
            "main-chat-text-telemetry-session",
            &messages,
            &state,
            MainChatPreprocessOptions::default(),
        )
        .await
        .expect("text retrieval must survive unavailable optional embeddings");

        let record = store
            .get_active_memory_record(memory_id)
            .unwrap()
            .expect("searched Memory row");
        assert_eq!(record.access_count, 1);
        assert!(record.last_accessed_at.is_some());
        assert_ne!(
            store
                .vector_rebuild_source_snapshot()
                .unwrap()
                .metadata_digest,
            before.metadata_digest
        );
    }
}
