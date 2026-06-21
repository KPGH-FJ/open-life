use std::collections::HashMap;
use std::sync::Arc;

use openlife_core::agent::{ContextAssembler, MemoryLifecycleCategory, MemoryLifecycleRecord};
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::memory::MemorySearchHit;
use openlife_core::privacy::PrivacyEngine;
use openlife_core::vectors::{embed_text_with_privacy, MemoryChunk};

use crate::main_chat_generation_support::{
    persist_chat_message_if_needed, persist_vector_memory_for_message,
};
use crate::main_chat_hs_runtime::classify_hs_policy_topic;
use crate::AppState;

const MEMORY_LIFECYCLE_SOURCE_PREFIX: &str = "memory_lifecycle:";
const MEMORY_LIFECYCLE_CANDIDATE_LIMIT: i64 = 25;
const MEMORY_LIFECYCLE_CONTEXT_LIMIT: usize = 5;

pub(crate) async fn filter_lifecycle_active_memory_results(
    results: Vec<(MemoryChunk, f32)>,
    state: &Arc<AppState>,
) -> Vec<(MemoryChunk, f32)> {
    let lifecycle_ids = results
        .iter()
        .filter_map(|(chunk, _)| lifecycle_memory_id_from_source(&chunk.source))
        .collect::<Vec<_>>();
    if lifecycle_ids.is_empty() {
        return results;
    }
    let Some(store_arc) = state.memory_lifecycle_store.as_ref() else {
        return results
            .into_iter()
            .filter(|(chunk, _)| lifecycle_memory_id_from_source(&chunk.source).is_none())
            .collect();
    };
    let store = store_arc.lock().await;
    results
        .into_iter()
        .filter(|(chunk, _)| {
            lifecycle_memory_id_from_source(&chunk.source)
                .map(|memory_id| store.is_memory_active(memory_id).unwrap_or(false))
                .unwrap_or(true)
        })
        .collect()
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

#[allow(clippy::too_many_arguments)]
/// Shared preprocessing for chat commands:
/// saves user message, loads model/tools/config, applies privacy filter,
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
    if let Some(user_msg) = messages.last() {
        if user_msg.role == "user" {
            let inserted = persist_chat_message_if_needed(session_id, user_msg, state).await?;
            if inserted {
                persist_vector_memory_for_message(session_id, user_msg, state).await;
            }
        }
    }

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
            let (masked, map) = privacy_engine.desensitize(&msg.content);
            privacy_map.extend(map);
            let mut final_text = masked;
            let router = state.intent_router.lock().await;
            if router.values_filter(&msg.content) {
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

    let mut embed_err = None;
    let mut memory_sources: Vec<String> = Vec::new();
    let mut memory_hit_count = 0usize;
    let memory_top_k = {
        let cfg = state.config.lock().await;
        cfg.system.memory_search_top_k
    };
    let memory_context = if let Some(user_msg) = messages.last() {
        if user_msg.role == "user" {
            let (memory_query, _) = privacy_engine.desensitize(&user_msg.content);
            let text_hits = {
                let store = state.memory_store.lock().await;
                store
                    .search_text_memories(Some(session_id), &memory_query, memory_top_k)
                    .unwrap_or_default()
            };

            let hs_local_only = classify_hs_policy_topic(&user_msg.content, &tools_prompt)
                != openlife_core::agent::PolicyTopic::General;
            let vector_hits = match embed_text_with_privacy(
                &memory_query,
                &provider,
                &openai_base,
                &openai_key,
                &embedding_model,
                embedding_enabled,
                &privacy_engine,
                hs_local_only,
            )
            .await
            {
                Ok(emb) => {
                    let store = state.vector_store.lock().await;
                    store
                        .search_by_session(session_id, &emb, 3, 1000)
                        .unwrap_or_default()
                }
                Err(e) => {
                    embed_err = Some(format!("向量记忆检索失败，已降级到关键词检索: {}", e));
                    vec![]
                }
            };

            let results = filter_lifecycle_active_memory_results(
                merge_memory_hits(vector_hits, text_hits, 3),
                state,
            )
            .await;
            let active_lifecycle_records =
                if let Some(lifecycle_store) = state.memory_lifecycle_store.as_ref() {
                    let store = lifecycle_store.lock().await;
                    relevant_active_lifecycle_records(
                        store
                            .list_active_records(None, MEMORY_LIFECYCLE_CANDIDATE_LIMIT)
                            .unwrap_or_default(),
                        &memory_query,
                        MEMORY_LIFECYCLE_CONTEXT_LIMIT,
                    )
                } else {
                    Vec::new()
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

/// V2 preprocessing using ContextAssembler.
/// This is functionally equivalent to preprocess_chat_input but uses
/// the modular ContextAssembler trait for better testability and extensibility.
pub(crate) async fn preprocess_chat_input_v2(
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
    let start = std::time::Instant::now();

    if let Some(user_msg) = messages.last() {
        if user_msg.role == "user" {
            let inserted = persist_chat_message_if_needed(session_id, user_msg, state).await?;
            if inserted {
                persist_vector_memory_for_message(session_id, user_msg, state).await;
            }
        }
    }

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

    let memory_top_k = {
        let cfg = state.config.lock().await;
        cfg.system.memory_search_top_k
    };
    let (memory_context_opt, memory_hits, memory_retrieval_time_ms) =
        if let Some(user_msg) = messages.last() {
            if user_msg.role == "user" {
                let cfg = state.config.lock().await;
                let embedding_config = openlife_core::agent::EmbeddingConfig {
                    enabled: cfg.llm.embedding_enabled,
                    provider: cfg.llm.provider.clone(),
                    openai_base: cfg.llm.openai_base.clone(),
                    openai_key: cfg.llm.openai_key.clone(),
                    embedding_model: cfg.llm.embedding_model.clone(),
                    hs_local_only: classify_hs_policy_topic(&user_msg.content, &tools_prompt)
                        != openlife_core::agent::PolicyTopic::General,
                };
                drop(cfg);

                let service = openlife_core::agent::MemoryService::new();
                let memory_store = state.memory_store.lock().await;
                let vector_store = state.vector_store.lock().await;
                let (memory_query, _) = privacy_engine.desensitize(&user_msg.content);

                match service
                    .retrieve_context(
                        session_id,
                        &memory_query,
                        &memory_store,
                        &vector_store,
                        &embedding_config,
                        memory_top_k,
                    )
                    .await
                {
                    Ok(ctx) => {
                        eprintln!(
                            "[MemoryService] Retrieved {} hits in {}ms (embedding: {})",
                            ctx.hits.len(),
                            ctx.retrieval_time_ms,
                            ctx.used_embedding
                        );
                        (Some(ctx.context), ctx.hits, ctx.retrieval_time_ms)
                    }
                    Err(e) => {
                        log::warn!("[MemoryService] Failed: {}, falling back to no context", e);
                        (None, vec![], 0)
                    }
                }
            } else {
                (None, vec![], 0)
            }
        } else {
            (None, vec![], 0)
        };

    let input = openlife_core::agent::AssembleInput {
        session_id: session_id.to_string(),
        messages: std::sync::Arc::new(messages.to_vec()),
        life_model: std::sync::Arc::new(life_model),
        tools_prompt: tools_prompt.clone(),
        privacy_engine: privacy_engine.clone(),
        memory_context: memory_context_opt,
        memory_hits,
        memory_retrieval_time_ms,
    };

    let assembler = openlife_core::agent::CompositeAssembler::new()
        .with(Box::new(openlife_core::agent::LifeModelAssembler))
        .with(Box::new(openlife_core::agent::PrivacyAssembler))
        .with(Box::new(openlife_core::agent::MemoryAssembler))
        .with(Box::new(openlife_core::agent::ToolsAssembler));

    let output = assembler.assemble(&input).map_err(|e| e.to_string())?;

    let mut desensitized_messages = output.desensitized_messages.to_vec();
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

    let lifecycle_query = messages
        .last()
        .filter(|message| message.role == "user")
        .map(|message| privacy_engine.desensitize(&message.content).0)
        .unwrap_or_default();
    let active_lifecycle_records =
        if let Some(lifecycle_store) = state.memory_lifecycle_store.as_ref() {
            let store = lifecycle_store.lock().await;
            relevant_active_lifecycle_records(
                store
                    .list_active_records(None, MEMORY_LIFECYCLE_CANDIDATE_LIMIT)
                    .unwrap_or_default(),
                &lifecycle_query,
                MEMORY_LIFECYCLE_CONTEXT_LIMIT,
            )
        } else {
            Vec::new()
        };
    let active_lifecycle_context = if active_lifecycle_records.is_empty() {
        String::new()
    } else {
        let snippets = active_lifecycle_records
            .iter()
            .map(|record| {
                format!(
                    "- [memory_lifecycle:{}] Accepted memory [{}:{}]: {}",
                    record.memory_id,
                    record.scope,
                    record.category,
                    record.content.replace('\n', " ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "\n以下是已确认且与当前问题相关的 active 记忆。仅在和当前用户指令直接相关时参考它们；当前用户指令优先，不要把 rejected、rolled-back 或无关历史当作事实：\n{snippets}"
        )
    };
    let combined_memory_context = [
        output.memory_context.as_str(),
        active_lifecycle_context.as_str(),
    ]
    .into_iter()
    .filter(|part| !part.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");

    if !combined_memory_context.is_empty() {
        if let Some(last_user) = desensitized_messages.iter_mut().rfind(|m| m.role == "user") {
            last_user.content = format!("{}\n\n{}", last_user.content, combined_memory_context);
        }
    }

    let embed_err = None;

    if let Some(ref store_arc) = state.rollout_metrics_store {
        let elapsed_ms = start.elapsed().as_millis() as i64;
        let metric = openlife_core::agent::RolloutMetric {
            id: None,
            experiment: "context_assembler".into(),
            version: "v2".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms: elapsed_ms,
            success: true,
            error: None,
            metadata: Some(format!("memory_hits:{}", input.memory_hits.len())),
        };
        let store = store_arc.lock().await;
        let _ = store.record_metric(&metric);
    }

    let mut context_summary = output.context_summary;
    context_summary.memory_hit_count += active_lifecycle_records.len() as i64;
    context_summary.memory_sources.extend(
        active_lifecycle_records
            .iter()
            .map(|record| format!("memory_lifecycle:{}", record.memory_id)),
    );

    Ok((
        output.life_model.as_ref().clone(),
        output.tools_prompt,
        privacy_engine,
        output.privacy_map,
        desensitized_messages.to_vec(),
        embed_err,
        context_summary,
    ))
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
        MemoryLifecycleStatus, MemoryMaterializationStatus,
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
}
