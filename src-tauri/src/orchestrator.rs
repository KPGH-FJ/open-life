use crate::state::AppState;
use crate::streaming::{emit_agent_status_update, emit_stream_error, TauriStreamingCallback};
use crate::{SendMessageResult, ToolCallResult, ToolCallStatus};
use anyhow::Result;
use futures::StreamExt;
use openlife_core::agent::ContextAssembler;
use openlife_core::agent::ReasoningTrace;
use openlife_core::layer_router::Layer;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::memory::MemorySearchHit;
use openlife_core::privacy::PrivacyEngine;
use openlife_core::scheduler::InferenceScheduler;
use openlife_core::vectors::{embed_text_with_config, MemoryChunk, VectorInsertItem};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::time::{timeout, Duration};

// ── Orchestration functions ──
pub(crate) async fn generate_and_persist_chat_proposals(
    state: &Arc<AppState>,
    agent_run: &openlife_core::agent::AgentRun,
    reply: &str,
    life_model: &LifeModel,
) {
    let Some(ref proposal_store_arc) = state.proposal_store else {
        return;
    };

    let proposals = {
        let engine = state.proposal_engine.lock().await;
        match engine.generate_from_run(agent_run, reply, life_model) {
            Ok(proposals) => proposals,
            Err(e) => {
                eprintln!("[ChatProposal] Proposal generation failed: {}", e);
                return;
            }
        }
    };

    if proposals.is_empty() {
        return;
    }

    let mut created_proposal_ids = Vec::new();
    {
        let store = proposal_store_arc.lock().await;
        for proposal in proposals {
            let proposal_id = proposal.id.clone();
            if let Err(e) = store.create_proposal(&proposal) {
                eprintln!("[ChatProposal] Failed to save proposal: {}", e);
            } else {
                created_proposal_ids.push(proposal_id);
            }
        }
    }

    if created_proposal_ids.is_empty() {
        return;
    }

    if let Some(ref run_store_arc) = state.agent_run_store {
        let run_store = run_store_arc.lock().await;
        for proposal_id in created_proposal_ids {
            if let Err(e) = run_store.add_generated_proposal(&agent_run.id, &proposal_id) {
                eprintln!("[AgentRun] 关联 Chat Proposal 失败: {}", e);
            }
        }
    }
}

pub(crate) async fn persist_life_model(
    state: &Arc<AppState>,
    mut life_model: LifeModel,
    create_daily_snapshot: bool,
) -> Result<LifeModel, String> {
    let previous_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().ok()
    };
    openlife_core::versioning::prepare_model_for_save(previous_model.as_ref(), &mut life_model);
    {
        let manager = state.life_model_manager.lock().await;
        manager.save(&life_model).map_err(|e| e.to_string())?;
    }
    if create_daily_snapshot {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let should_snapshot = {
            let vm = state.version_manager.lock().await;
            !vm.has_snapshot_tag_on_date("auto:daily-save", &today)
                .map_err(|e| e.to_string())?
        };
        if should_snapshot {
            let vm = state.version_manager.lock().await;
            vm.snapshot(&life_model, "auto:daily-save", "当日首次保存自动快照")
                .map_err(|e| e.to_string())?;
            let mut last_snapshot_date = state.last_snapshot_date.lock().await;
            *last_snapshot_date = Some(today);
        }
    }
    Ok(life_model)
}
pub(crate) fn try_auto_checkin_daily_goals(content: &str, life_model: &mut LifeModel) -> Option<String> {
    let lower = content.to_lowercase();
    let triggers = [
        "我今天完成了",
        "我完成了",
        "我已经完成了",
        "刚刚完成了",
        "我搞定了",
        "我做完了",
        "已经打卡了",
        "今天搞定了",
    ];
    let triggered = triggers.iter().any(|t| lower.contains(t));
    if !triggered {
        return None;
    }
    let mut checked = Vec::new();
    for goal in &mut life_model.goals.daily {
        if goal.done {
            continue;
        }
        if lower.contains(&goal.name.to_lowercase()) {
            goal.done = true;
            checked.push(goal.name.clone());
        }
    }
    if !checked.is_empty() {
        Some(format!("已自动打卡今日目标：{}", checked.join("、")))
    } else {
        None
    }
}

pub(crate) async fn persist_chat_message_if_needed(
    session_id: &str,
    msg: &ChatMessage,
    state: &State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    let store = state.memory_store.lock().await;
    let should_skip = store
        .load_recent_messages(session_id, 1)
        .map_err(|e| e.to_string())?
        .last()
        .map(|last| last.role == msg.role && last.content == msg.content)
        .unwrap_or(false);
    if should_skip {
        let _ = store.touch_chat_session(session_id);
        return Ok(false);
    }
    store
        .save_message(session_id, msg)
        .map_err(|e| e.to_string())?;
    let _ = store.touch_chat_session(session_id);
    Ok(true)
}

pub(crate) async fn persist_vector_memory_for_message(
    session_id: &str,
    msg: &ChatMessage,
    state: &State<'_, Arc<AppState>>,
) {
    let content = msg.content.trim();
    if content.is_empty() {
        return;
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
    let embedding = match embed_text_with_config(
        content,
        &provider,
        &openai_base,
        &openai_key,
        &embedding_model,
        embedding_enabled,
    )
    .await
    {
        Ok(embedding) if !embedding.is_empty() => embedding,
        Ok(_) => return,
        Err(e) => {
            eprintln!(
                "[memory] embedding generation failed for {} message in session {}: {} - lib.rs:520",
                msg.role, session_id, e
            );
            return;
        }
    };
    let store = state.vector_store.lock().await;
    let item = VectorInsertItem {
        session_id,
        content,
        embedding: &embedding,
        source: if msg.role == "assistant" {
            "assistant_reply"
        } else {
            "user_message"
        },
    };
    if let Err(e) = store.insert_batch(&[item]) {
        eprintln!(
            "[memory] vector insert failed for {} message in session {}: {} - lib.rs:537",
            msg.role, session_id, e
        );
    }
}

pub(crate) async fn finalize_chat_agent_run(
    session_id: &str,
    assistant_message: &ChatMessage,
    reply: &str,
    reasoning_trace: &mut ReasoningTrace,
    agent_run: &mut openlife_core::agent::AgentRun,
    life_model: &LifeModel,
    state: &State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let inserted = persist_chat_message_if_needed(session_id, assistant_message, state).await?;

    reasoning_trace.generation_result = Some(serde_json::json!({ "text": reply }));
    agent_run.output_preview = Some(preview_text(reply, 200));
    if agent_run.status == openlife_core::agent::AgentRunStatus::Running {
        agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    }
    agent_run.finished_at = Some(chrono::Utc::now());
    agent_run.reasoning_trace = Some(reasoning_trace.clone());

    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        match store.get_run(&agent_run.id) {
            Ok(Some(_)) => {
                if let Err(e) = store.update_run(agent_run) {
                    eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                }
            }
            Ok(None) => {
                if let Err(e) = store.create_run(agent_run) {
                    eprintln!("[AgentRun] 保存运行记录失败: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[AgentRun] 查询运行记录失败: {}", e);
                if let Err(e) = store.create_run(agent_run) {
                    eprintln!("[AgentRun] 保存运行记录失败: {}", e);
                }
            }
        }
    }

    if inserted
        && timeout(
            Duration::from_secs(CHAT_VECTOR_PERSIST_TIMEOUT_SECS),
            persist_vector_memory_for_message(session_id, assistant_message, state),
        )
        .await
        .is_err()
    {
        eprintln!(
            "[memory] vector persistence timed out after {}s for assistant message in session {}",
            CHAT_VECTOR_PERSIST_TIMEOUT_SECS, session_id
        );
    }

    if timeout(
        Duration::from_secs(CHAT_PROPOSAL_GENERATION_TIMEOUT_SECS),
        generate_and_persist_chat_proposals(state.inner(), agent_run, reply, life_model),
    )
    .await
    .is_err()
    {
        eprintln!(
            "[ChatProposal] Proposal generation timed out after {}s for run {}",
            CHAT_PROPOSAL_GENERATION_TIMEOUT_SECS, agent_run.id
        );
    }
    Ok(())
}

/// Shared preprocessing for chat commands:
/// saves user message, loads model/tools/config, applies privacy filter,
/// values filter, and vector memory retrieval.
async fn preprocess_chat_input(
    session_id: &str,
    messages: &[ChatMessage],
    state: &State<'_, Arc<AppState>>,
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

    // Refresh hot cache if stale
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
            let text_hits = {
                let store = state.memory_store.lock().await;
                store
                    .search_text_memories(Some(session_id), &user_msg.content, memory_top_k)
                    .unwrap_or_default()
            };

            let vector_hits = match embed_text_with_config(
                &user_msg.content,
                &provider,
                &openai_base,
                &openai_key,
                &embedding_model,
                embedding_enabled,
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

            let results = merge_memory_hits(vector_hits, text_hits, 3);
            memory_hit_count = results.len();
            memory_sources = results
                .iter()
                .map(|(chunk, _)| chunk.source.clone())
                .collect();
            if results.is_empty() {
                String::new()
            } else {
                let snippets: Vec<String> = results
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
                format!(
                    "\n以下是你过去记忆中的相关内容，请在回应中自然地参考它们：\n{}",
                    snippets.join("\n")
                )
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Prepend hot memory cache as a system message (always injected)
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
    state: &State<'_, Arc<AppState>>,
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

    // Step 1: Persist user message (same as v1)
    if let Some(user_msg) = messages.last() {
        if user_msg.role == "user" {
            let inserted = persist_chat_message_if_needed(session_id, user_msg, state).await?;
            if inserted {
                persist_vector_memory_for_message(session_id, user_msg, state).await;
            }
        }
    }

    // Step 2: Load LifeModel
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };

    // Step 3: Refresh hot cache
    {
        let mut cache = state.hot_cache.write().await;
        if cache.is_stale(&life_model) {
            cache.refresh(&life_model);
        }
    }

    // Step 4: Get tools prompt
    let tools_prompt = {
        let reg = state.mcp_registry.lock().await;
        reg.tools_prompt()
    };

    // Step 5: Prefetch memory using MemoryService
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
                };
                drop(cfg);

                let service = openlife_core::agent::MemoryService::new();
                let memory_store = state.memory_store.lock().await;
                let vector_store = state.vector_store.lock().await;

                match service
                    .retrieve_context(
                        session_id,
                        &user_msg.content,
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
                        eprintln!("[MemoryService] Failed: {}, falling back to no context", e);
                        (None, vec![], 0)
                    }
                }
            } else {
                (None, vec![], 0)
            }
        } else {
            (None, vec![], 0)
        };

    // Step 6: Build privacy engine
    let privacy_engine = state.privacy_engine.lock().await.clone();

    // Step 7: Assemble using ContextAssembler
    let input = openlife_core::agent::AssembleInput {
        session_id: session_id.to_string(),
        messages: messages.to_vec(),
        life_model,
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

    // Step 8: Apply hot cache (same as v1)
    let mut desensitized_messages = output.desensitized_messages;
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

    // Step 9: Apply memory context to last user message (same as v1)
    if !output.memory_context.is_empty() {
        if let Some(last_user) = desensitized_messages.iter_mut().rfind(|m| m.role == "user") {
            last_user.content = format!("{}\n\n{}", last_user.content, output.memory_context);
        }
    }

    // Step 10: Build embed_err if memory retrieval had issues
    let embed_err = None; // Memory retrieval succeeded or wasn't attempted

    // Record rollout metric for context assembler v2
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

    Ok((
        output.life_model,
        output.tools_prompt,
        privacy_engine,
        output.privacy_map,
        desensitized_messages,
        embed_err,
        output.context_summary,
    ))
}

#[allow(dead_code)]
pub(crate) fn build_reasoning_trace_prompt(trace: &ReasoningTrace) -> String {
    let mut prompt = String::new();
    if let Some(ref m) = trace.meaning_result {
        if let Some(text) = m.get("text").and_then(|t| t.as_str()) {
            prompt.push_str(&format!("【意义层约束】{}\n", text));
        }
    }
    if let Some(ref s) = trace.strategy_result {
        if let Some(text) = s.get("text").and_then(|t| t.as_str()) {
            prompt.push_str(&format!("【策略层约束】{}\n", text));
        }
        if let Some(tools) = s.get("suggested_tools").and_then(|v| v.as_array()) {
            let tools_text = tools
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join("、");
            if !tools_text.is_empty() {
                prompt.push_str(&format!("【建议工具】{}\n", tools_text));
            }
        }
    }
    if let Some(ref safety) = trace.safety_check_result {
        if let Some(warnings) = safety.get("warnings").and_then(|v| v.as_array()) {
            let text = warnings
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join("；");
            if !text.is_empty() {
                prompt.push_str(&format!("【安全检查提醒】{}\n", text));
            }
        }
    }
    prompt
}
pub(crate) async fn capture_conversation_signals(
    session_id: &str,
    user_text: &str,
    life_model: &LifeModel,
    state: &Arc<AppState>,
) {
    let normalized = user_text.trim().to_lowercase();
    if normalized.is_empty() {
        return;
    }

    let positive = [
        "想继续",
        "想做",
        "准备",
        "投入",
        "推进",
        "喜欢",
        "热爱",
        "重要",
        "值得",
        "继续",
    ];
    let negative = [
        "不想",
        "放弃",
        "没意义",
        "拖延",
        "讨厌",
        "厌倦",
        "卡住",
        "做不到",
    ];
    let has_positive = positive.iter().any(|kw| normalized.contains(kw));
    let has_negative = negative.iter().any(|kw| normalized.contains(kw));
    let base_delta = if has_negative {
        -0.02
    } else if has_positive {
        0.02
    } else {
        0.01
    };
    let confidence = if has_positive || has_negative {
        0.68
    } else {
        0.45
    };

    let store = state.feedback_store.lock().await;

    for value in &life_model.identity.values {
        if normalized.contains(&value.name.to_lowercase()) {
            if let Err(e) = store.log_event(
                &format!("value_focus:{}", value.name),
                Some(session_id),
                Some("chat_match"),
            ) {
                eprintln!("[Memory] 记录价值观焦点事件失败: {}", e);
            }
            if let Err(e) = store.save_conversation_inference(
                Some(session_id),
                "identity.values",
                &value.name,
                base_delta,
                confidence,
                "用户在对话中主动提及或强化了该价值观",
            ) {
                eprintln!("[Memory] 保存价值观推断失败: {}", e);
            }
        }
    }

    for goal in life_model
        .goals
        .short_term
        .iter()
        .chain(life_model.goals.medium_term.iter())
        .chain(life_model.goals.long_term.iter())
        .chain(life_model.goals.life_goals.iter())
    {
        if normalized.contains(&goal.name.to_lowercase()) {
            if let Err(e) = store.save_conversation_inference(
                Some(session_id),
                "goals",
                &goal.name,
                base_delta,
                (confidence - 0.05).max(0.2),
                "用户在对话中直接提到该目标，表明关注度发生变化",
            ) {
                eprintln!("[Memory] 保存目标推断失败: {}", e);
            }
        }
    }

    for skill in &life_model.capabilities.skills {
        if normalized.contains(&skill.name.to_lowercase()) {
            let skill_delta = if normalized.contains("学习")
                || normalized.contains("练习")
                || normalized.contains("提升")
            {
                0.02
            } else {
                base_delta
            };
            if let Err(e) = store.save_conversation_inference(
                Some(session_id),
                "capabilities.skills",
                &skill.name,
                skill_delta,
                0.55,
                "用户在对话中主动提及技能投入或受阻情况",
            ) {
                eprintln!("[Memory] 保存技能推断失败: {}", e);
            }
        }
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

pub(crate) async fn send_message_impl(
    session_id: String,
    messages: Vec<ChatMessage>,
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<SendMessageResult, String> {
    let user_msg = messages.last().cloned();
    let intent = if let Some(ref m) = user_msg {
        if m.role == "user" {
            let router = state.intent_router.lock().await;
            Some(router.classify(&m.content))
        } else {
            None
        }
    } else {
        None
    };

    let layer = if let (Some(ref i), Some(ref m)) = (&intent, &user_msg) {
        let lr = state.layer_router.lock().await;
        lr.resolve(i, &m.content)
    } else {
        Layer::L2
    };

    // Layer 1: direct reflex response
    if layer == Layer::L1 {
        if let Some(ref i) = intent {
            if let Some(reply) = i.direct_response() {
                // Persist user message
                if let Some(ref user) = user_msg {
                    if user.role == "user" {
                        let inserted =
                            persist_chat_message_if_needed(&session_id, user, &state).await?;
                        if inserted {
                            persist_vector_memory_for_message(&session_id, user, &state).await;
                        }
                    }
                }
                let assistant_msg = ChatMessage {
                    role: "assistant".into(),
                    content: reply.clone(),
                };

                // Create and finalize AgentRun for L1
                let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(
                    &session_id,
                    &user_msg
                        .as_ref()
                        .map(|m| m.content.clone())
                        .unwrap_or_default(),
                );
                let model_route = openlife_core::agent::ModelRouteTrace {
                    provider: "direct".to_string(),
                    model: "L1_reflex".to_string(),
                    route_type: "direct".to_string(),
                    prefer_local: false,
                    local_model: "".to_string(),
                    reason: "layer_1_direct_response".to_string(),
                    privacy_level: openlife_core::agent::types::RedactionLevel::None,
                    latency_ms: None,
                    retry_count: 0,
                    fallback_reason: None,
                    provider_health_is_estimated: Some(false),
                };
                let context_summary = openlife_core::agent::ContextSummary {
                    life_model_empty: false,
                    included_life_model_sections: vec![],
                    memory_hit_count: 0,
                    memory_sources: vec![],
                    used_tools_prompt: false,
                    redaction_applied: false,
                    redaction_level: openlife_core::agent::types::RedactionLevel::None,
                };
                agent_run.complete(&preview_text(&reply, 200), model_route, context_summary);
                let life_model = {
                    let manager = state.life_model_manager.lock().await;
                    manager
                        .load()
                        .map_err(|e| format!("人生模型加载失败: {}", e))?
                };
                let mut reasoning_trace = ReasoningTrace::default();
                finalize_chat_agent_run(
                    &session_id,
                    &assistant_msg,
                    &reply,
                    &mut reasoning_trace,
                    &mut agent_run,
                    &life_model,
                    &state,
                )
                .await?;

                return Ok(SendMessageResult {
                    reply,
                    reasoning_trace,
                    tool_calls: vec![],
                    run_id: Some(agent_run.id.clone()),
                });
            }
        }
    }

    // Gradual rollout: use v2 if experimental flag is enabled
    let use_v2 = {
        let cfg = state.config.lock().await;
        cfg.experimental_context_assembler
    };

    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        _context_summary,
    ) = if use_v2 {
        preprocess_chat_input_v2(&session_id, &messages, &state).await?
    } else {
        preprocess_chat_input(&session_id, &messages, &state).await?
    };

    let auto_checkin_msg = if let Some(ref m) = user_msg {
        let msg = try_auto_checkin_daily_goals(&m.content, &mut life_model);
        capture_conversation_signals(&session_id, &m.content, &life_model, state.inner()).await;
        if msg.is_some() {
            let _ = persist_life_model(&state.inner().clone(), life_model.clone(), false).await?;
        }
        msg
    } else {
        None
    };

    return send_message_with_agent_loop(
        session_id,
        messages,
        user_msg,
        life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        auto_checkin_msg,
        layer,
        state,
        app_handle,
    )
    .await;
}

/// AgentLoop-based chat execution (primary path).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn send_message_with_agent_loop(
    session_id: String,
    _messages: Vec<ChatMessage>,
    user_msg: Option<ChatMessage>,
    life_model: LifeModel,
    tools_prompt: String,
    privacy_engine: PrivacyEngine,
    privacy_map: HashMap<String, String>,
    desensitized_messages: Vec<ChatMessage>,
    embed_err: Option<String>,
    auto_checkin_msg: Option<String>,
    layer: Layer,
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<SendMessageResult, String> {
    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    let calendar_ics_paths = cfg.system.calendar_ics_paths.clone();
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    let action_executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let loop_config = openlife_core::agent::AgentLoopConfig {
        max_steps: 4,
        max_tool_calls: 6,
        timeout_seconds: 90,
        allow_writes: true,
        allow_cloud: true,
        shutdown_notify: Some(state.shutdown_notify.clone()),
    };
    let agent_loop = openlife_core::agent::AgentLoop::new(
        agent_runtime,
        action_executor,
        scheduler.clone(),
        loop_config,
    );

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.clone(),
        user_text: user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages: desensitized_messages.clone(),
        layer,
    };

    let network_policy = cfg.system.network_policy.clone();

    let loop_result = {
        let (reg, audit) = state.get_mcp_state().await;
        let permission_store = state.tool_permission_store.lock().await;
        let memory_store = state.memory_store.lock().await;
        let proposal_store_guard = if let Some(ref store) = state.proposal_store {
            Some(store.lock().await)
        } else {
            None
        };
        let agent_run_store_guard = if let Some(ref store) = state.agent_run_store {
            Some(store.lock().await)
        } else {
            None
        };
        let action_ctx = openlife_core::agent::ActionExecutionContext {
            registry: &reg,
            permission_store: &permission_store,
            audit_store: &audit,
            privacy_engine: &privacy_engine,
            safe_paths: &safe_paths,
            calendar_ics_paths: &calendar_ics_paths,
            life_model: Some(&life_model),
            memory_store: Some(&memory_store),
            proposal_store: proposal_store_guard.as_deref(),
            agent_run_store: agent_run_store_guard.as_deref(),
            network_policy: Some(&network_policy),
        };

        agent_loop
            .run(
                &task,
                &life_model,
                &tools_prompt,
                None,
                privacy_engine.clone(),
                &action_ctx,
            )
            .await
    };

    let (mut reply, mut agent_run, _status_updates) = match loop_result {
        Ok(result) => {
            // Emit AgentLoop status updates as Tauri events
            for update in &result.status_updates {
                emit_agent_status_update(
                    &app_handle,
                    &session_id,
                    &result.run.id,
                    &update.phase.to_string(),
                    &update.message,
                    update.step_index,
                    update.tool_call_index,
                );
            }
            (result.final_response, result.run, result.status_updates)
        }
        Err(e) => {
            eprintln!(
                "[warn] AgentLoop failed in send_message, falling back to legacy: {}",
                e
            );
            let fallback_reply = generate_non_stream_fallback(
                &scheduler,
                desensitized_messages.clone(),
                &life_model,
                &tools_prompt,
            )
            .await
            .map_err(|fallback_err| {
                format!(
                    "AgentLoop failed: {}. Fallback also failed: {}",
                    e, fallback_err
                )
            })?;

            let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(
                &session_id,
                &user_msg
                    .as_ref()
                    .map(|m| m.content.clone())
                    .unwrap_or_default(),
            );
            agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
            agent_run.output_preview = Some(preview_text(&fallback_reply, 200));
            agent_run
                .warnings
                .push(format!("fallback: agent_loop_error: {}", e));
            agent_run.finished_at = Some(chrono::Utc::now());

            if let Some(ref store_arc) = state.agent_run_store {
                let store = store_arc.lock().await;
                let _ = store.create_run(&agent_run);
            }

            return Ok(SendMessageResult {
                reply: fallback_reply,
                reasoning_trace: ReasoningTrace::default(),
                tool_calls: Vec::new(),
                run_id: Some(agent_run.id),
            });
        }
    };

    // Store status_updates in agent_run for persistence
    // (This requires adding a field to AgentRun, which we'll skip for now
    // and just use the Tauri events for real-time UI updates)

    // Apply privacy reconstruction
    reply = privacy_engine.reconstruct(&reply, &privacy_map);

    // Apply auto checkin message
    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    let mut reasoning_trace = agent_run.reasoning_trace.clone().unwrap_or_default();
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }

    finalize_chat_agent_run(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        &state,
    )
    .await?;

    let tool_calls = agent_actions_to_tool_call_results(&agent_run.actions, &agent_run.id);

    Ok(SendMessageResult {
        reply,
        reasoning_trace,
        tool_calls,
        run_id: Some(agent_run.id.clone()),
    })
}

#[derive(serde::Deserialize, Clone, Debug)]
pub(crate) struct StartStreamMessageArgs {
    session_id: String,
    messages: Vec<ChatMessage>,
}

const STREAM_INIT_TIMEOUT_SECS: u64 = 45;
const STREAM_CHUNK_TIMEOUT_SECS: u64 = 90;
const NON_STREAM_FALLBACK_TIMEOUT_SECS: u64 = 120;
const CHAT_VECTOR_PERSIST_TIMEOUT_SECS: u64 = 8;
const CHAT_PROPOSAL_GENERATION_TIMEOUT_SECS: u64 = 5;

pub(crate) async fn generate_non_stream_fallback(
    scheduler: &InferenceScheduler,
    messages: Vec<ChatMessage>,
    life_model: &LifeModel,
    tools_prompt: &str,
) -> Result<String, String> {
    timeout(
        Duration::from_secs(NON_STREAM_FALLBACK_TIMEOUT_SECS),
        scheduler.generate(messages, life_model, Some(tools_prompt)),
    )
    .await
    .map_err(|_| {
        format!(
            "非流式重试超时（{} 秒），请检查模型服务或切换后端。",
            NON_STREAM_FALLBACK_TIMEOUT_SECS
        )
    })?
    .map_err(|e| e.to_string())
}

pub(crate) fn preview_text(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

pub(crate) fn included_life_model_sections(life_model: &LifeModel) -> Vec<String> {
    if life_model.is_effectively_empty() {
        Vec::new()
    } else {
        vec![
            "identity".to_string(),
            "goals".to_string(),
            "capabilities".to_string(),
            "state".to_string(),
        ]
    }
}

pub(crate) fn agent_actions_to_tool_call_results(
    actions: &[openlife_core::agent::AgentAction],
    run_id: &str,
) -> Vec<ToolCallResult> {
    actions
        .iter()
        .map(|action| {
            let output = action.output.as_ref().and_then(|value| {
                value
                    .get("text")
                    .and_then(|text| text.as_str())
                    .map(ToString::to_string)
                    .or_else(|| value.as_str().map(ToString::to_string))
            });
            ToolCallResult {
                name: action.target.clone().unwrap_or_default(),
                arguments: action.input.clone(),
                sanitized_arguments: None,
                success: matches!(
                    action.status.as_str(),
                    "succeeded" | "completed" | "success"
                ),
                output,
                error: action.error.clone(),
                permission_level: action
                    .tool_scope
                    .as_ref()
                    .map(|scope| scope.risk_level.clone())
                    .unwrap_or_else(|| "low".to_string()),
                status: match action.status.as_str() {
                    "success" | "succeeded" | "completed" => ToolCallStatus::Success,
                    "needs_confirmation" => ToolCallStatus::NeedsConfirmation,
                    "blocked" => ToolCallStatus::Blocked,
                    _ => ToolCallStatus::Error,
                },
                requires_confirmation: action.status == "needs_confirmation",
                pii_found: false,
                privacy_warnings: Vec::new(),
                action_id: Some(action.id.clone()),
                run_id: Some(run_id.to_string()),
                permission_decision: action.permission_decision.clone(),
            }
        })
        .collect()
}

/// Stream-mode AgentLoop execution: runs AgentLoop and emits real token-level stream events.
/// This provides consistency when use_agent_loop=true in stream mode.
pub(crate) async fn start_stream_message_with_agent_loop(
    session_id: String,
    messages: Vec<ChatMessage>,
    user_msg: Option<ChatMessage>,
    _layer: Layer,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Non-persisted placeholder id for pre-run errors. The stream start/done
    // events use the authoritative AgentLoop run id after execution.
    let user_input_text = user_msg
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let placeholder_run_id =
        openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text).id;

    // Preprocess
    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        _context_summary,
    ) = match preprocess_chat_input_v2(&session_id, &messages, &state).await {
        Ok(result) => result,
        Err(message) => return Err(message),
    };

    let auto_checkin_msg = if let Some(ref m) = user_msg {
        let msg = try_auto_checkin_daily_goals(&m.content, &mut life_model);
        if msg.is_some() {
            let _ = persist_life_model(&state.inner().clone(), life_model.clone(), false).await;
        }
        msg
    } else {
        None
    };

    // Run AgentLoop
    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    let calendar_ics_paths = cfg.system.calendar_ics_paths.clone();
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    let action_executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let loop_config = openlife_core::agent::AgentLoopConfig {
        max_steps: 4,
        max_tool_calls: 6,
        timeout_seconds: 90,
        allow_writes: true,
        allow_cloud: true,
        shutdown_notify: Some(state.shutdown_notify.clone()),
    };
    let agent_loop = openlife_core::agent::AgentLoop::new(
        agent_runtime,
        action_executor,
        scheduler.clone(),
        loop_config,
    );

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.clone(),
        user_text: user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages: desensitized_messages.clone(),
        layer: _layer,
    };

    let network_policy = cfg.system.network_policy.clone();

    // Create streaming callback with run_id placeholder (will be updated after AgentLoop starts)
    let callback = Arc::new(TauriStreamingCallback {
        app_handle: app_handle.clone(),
        session_id: session_id.clone(),
        run_id: placeholder_run_id.clone(),
    });

    // Emit stream-message-start before running agent loop
    let _ = app_handle.emit(
        "stream-message-start",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": placeholder_run_id,
            "reasoning_trace": ReasoningTrace::default(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );

    let loop_result = {
        let (reg, audit) = state.get_mcp_state().await;
        let permission_store = state.tool_permission_store.lock().await;
        let memory_store = state.memory_store.lock().await;
        let proposal_store_guard = if let Some(ref store) = state.proposal_store {
            Some(store.lock().await)
        } else {
            None
        };
        let agent_run_store_guard = if let Some(ref store) = state.agent_run_store {
            Some(store.lock().await)
        } else {
            None
        };
        let action_ctx = openlife_core::agent::ActionExecutionContext {
            registry: &reg,
            permission_store: &permission_store,
            audit_store: &audit,
            privacy_engine: &privacy_engine,
            safe_paths: &safe_paths,
            calendar_ics_paths: &calendar_ics_paths,
            life_model: Some(&life_model),
            memory_store: Some(&memory_store),
            proposal_store: proposal_store_guard.as_deref(),
            agent_run_store: agent_run_store_guard.as_deref(),
            network_policy: Some(&network_policy),
        };

        agent_loop
            .run_streaming(
                &task,
                &life_model,
                &tools_prompt,
                None,
                privacy_engine.clone(),
                &action_ctx,
                callback,
            )
            .await
    };

    let (mut reply, mut agent_run) = match loop_result {
        Ok(result) => (result.final_response, result.run),
        Err(e) => {
            eprintln!(
                "[warn] AgentLoop streaming failed, falling back to legacy: {}",
                e
            );
            let fallback_reply = match generate_non_stream_fallback(
                &scheduler,
                desensitized_messages.clone(),
                &life_model,
                &tools_prompt,
            )
            .await
            {
                Ok(reply) => reply,
                Err(fallback_err) => {
                    let error_msg = format!(
                        "AgentLoop failed: {}. Fallback also failed: {}",
                        e, fallback_err
                    );
                    let _ = app_handle.emit(
                        "stream-message-error",
                        serde_json::json!({
                            "session_id": &session_id,
                            "run_id": placeholder_run_id,
                            "error": error_msg.clone(),
                        }),
                    );
                    return Err(error_msg);
                }
            };

            let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(
                &session_id,
                &user_msg
                    .as_ref()
                    .map(|m| m.content.clone())
                    .unwrap_or_default(),
            );
            agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
            agent_run.output_preview = Some(preview_text(&fallback_reply, 200));
            agent_run
                .warnings
                .push(format!("fallback: agent_loop_error: {}", e));
            agent_run.finished_at = Some(chrono::Utc::now());

            // Emit fallback reply as a single chunk
            let _ = app_handle.emit(
                "stream-message-chunk",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": placeholder_run_id,
                    "chunk": fallback_reply,
                }),
            );

            (fallback_reply, agent_run)
        }
    };

    // Apply privacy reconstruction
    reply = privacy_engine.reconstruct(&reply, &privacy_map);

    // Apply auto checkin message
    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    let mut reasoning_trace = agent_run.reasoning_trace.clone().unwrap_or_default();
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    // Finalize AgentRun
    let result = finalize_chat_agent_run(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        &state,
    )
    .await;

    match result {
        Ok(_) => {
            let tool_calls = agent_actions_to_tool_call_results(&agent_run.actions, &agent_run.id);
            let _ = app_handle.emit(
                "stream-message-done",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": agent_run.id,
                    "reply": reply,
                    "reasoning_trace": reasoning_trace,
                    "tool_calls": tool_calls,
                }),
            );
            Ok(())
        }
        Err(e) => {
            let _ = app_handle.emit(
                "stream-message-error",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": agent_run.id,
                    "error": e,
                }),
            );
            Ok(())
        }
    }
}

pub(crate) async fn start_stream_message_impl(
    args: Option<StartStreamMessageArgs>,
    session_id: Option<String>,
    messages: Option<Vec<ChatMessage>>,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let (session_id, messages) = if let Some(args) = args {
        (args.session_id, args.messages)
    } else {
        (
            session_id.ok_or_else(|| "start_stream_message 缺少 session_id".to_string())?,
            messages.ok_or_else(|| "start_stream_message 缺少 messages".to_string())?,
        )
    };

    let user_msg = messages.last().cloned();
    let intent = if let Some(ref m) = user_msg {
        if m.role == "user" {
            let router = state.intent_router.lock().await;
            Some(router.classify(&m.content))
        } else {
            None
        }
    } else {
        None
    };

    let layer = if let (Some(ref i), Some(ref m)) = (&intent, &user_msg) {
        let lr = state.layer_router.lock().await;
        lr.resolve(i, &m.content)
    } else {
        Layer::L2
    };

    // Route L2/L3 to AgentLoop streaming path; L1 uses direct reflex below
    if layer != Layer::L1 {
        return start_stream_message_with_agent_loop(
            session_id, messages, user_msg, layer, app_handle, state,
        )
        .await;
    }

    // L1 direct reflex — no AgentLoop needed
    let user_input_text = messages
        .last()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text);
    let _agent_run_id = agent_run.id.clone();
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(&agent_run) {
            eprintln!("[AgentRun] 保存运行记录失败: {}", e);
        }
    }

    // Layer 1: direct reflex response (non-streaming, emit as single chunk)
    if layer == Layer::L1 {
        if let Some(ref i) = intent {
            if let Some(reply) = i.direct_response() {
                // 先保存用户消息
                if let Some(ref user) = user_msg {
                    if user.role == "user" {
                        let user_inserted =
                            persist_chat_message_if_needed(&session_id, user, &state).await?;
                        if user_inserted {
                            persist_vector_memory_for_message(&session_id, user, &state).await;
                        }
                    }
                }

                // 加载 LifeModel 用于 finalizer
                let life_model = {
                    let manager = state.life_model_manager.lock().await;
                    manager.load().map_err(|e| e.to_string())?
                };

                let assistant_msg = ChatMessage {
                    role: "assistant".into(),
                    content: reply.clone(),
                };

                let _ = app_handle.emit(
                    "stream-message-start",
                    serde_json::json!({
                        "session_id": &session_id,
                        "run_id": agent_run.id,
                        "reasoning_trace": ReasoningTrace::default(),
                        "tool_calls": Vec::<ToolCallResult>::new(),
                    }),
                );
                let _ = app_handle.emit(
                    "stream-message-chunk",
                    serde_json::json!({
                        "session_id": &session_id,
                        "run_id": agent_run.id,
                        "chunk": reply.clone(),
                    }),
                );

                // 使用统一的 finalizer，避免重复保存和手动更新 AgentRun
                let mut reasoning_trace = ReasoningTrace::default();
                let model_route = openlife_core::agent::ModelRouteTrace {
                    provider: "direct".to_string(),
                    model: "L1_reflex".to_string(),
                    route_type: "direct".to_string(),
                    prefer_local: false,
                    local_model: "".to_string(),
                    reason: "layer_1_direct_response".to_string(),
                    privacy_level: openlife_core::agent::types::RedactionLevel::None,
                    latency_ms: None,
                    retry_count: 0,
                    fallback_reason: None,
                    provider_health_is_estimated: Some(false),
                };
                let context_summary = openlife_core::agent::ContextSummary {
                    life_model_empty: true,
                    included_life_model_sections: vec![],
                    memory_hit_count: 0,
                    memory_sources: vec![],
                    used_tools_prompt: false,
                    redaction_applied: false,
                    redaction_level: openlife_core::agent::types::RedactionLevel::None,
                };
                let preview = preview_text(&reply, 200);
                agent_run.complete(&preview, model_route, context_summary);

                if let Err(e) = finalize_chat_agent_run(
                    &session_id,
                    &assistant_msg,
                    &reply,
                    &mut reasoning_trace,
                    &mut agent_run,
                    &life_model,
                    &state,
                )
                .await
                {
                    eprintln!("[L1 Stream] finalize_chat_agent_run failed: {}", e);
                    let _ = app_handle.emit(
                        "stream-message-error",
                        serde_json::json!({
                            "session_id": &session_id,
                            "run_id": agent_run.id,
                            "error": format!("AgentRun 持久化失败: {}", e),
                        }),
                    );
                    return Err(e);
                }

                let _ = app_handle.emit(
                    "stream-message-done",
                    serde_json::json!({
                        "session_id": &session_id,
                        "run_id": agent_run.id,
                        "reply": reply,
                        "reasoning_trace": ReasoningTrace::default(),
                        "tool_calls": Vec::<ToolCallResult>::new(),
                    }),
                );
                return Ok(());
            }
        }
    }

    // Gradual rollout: use v2 if experimental flag is enabled
    let use_v2 = {
        let cfg = state.config.lock().await;
        cfg.experimental_context_assembler
    };

    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        _embed_err,
        context_summary,
    ) = match if use_v2 {
        preprocess_chat_input_v2(&session_id, &messages, &state).await
    } else {
        preprocess_chat_input(&session_id, &messages, &state).await
    } {
        Ok(result) => result,
        Err(message) => {
            let error = openlife_core::agent::AgentRunError {
                message: message.clone(),
                phase: "preprocess".to_string(),
                recoverable: true,
            };
            agent_run.fail(error);
            if let Some(ref store_arc) = state.agent_run_store {
                let store = store_arc.lock().await;
                if let Err(e) = store.update_run(&agent_run) {
                    eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                }
            }
            return Err(message);
        }
    };

    let auto_checkin_msg_stream = if let Some(ref m) = user_msg {
        let msg = try_auto_checkin_daily_goals(&m.content, &mut life_model);
        capture_conversation_signals(&session_id, &m.content, &life_model, state.inner()).await;
        if msg.is_some() {
            if let Err(message) =
                persist_life_model(&state.inner().clone(), life_model.clone(), false).await
            {
                let error = openlife_core::agent::AgentRunError {
                    message: message.clone(),
                    phase: "preprocess".to_string(),
                    recoverable: true,
                };
                agent_run.fail(error);
                if let Some(ref store_arc) = state.agent_run_store {
                    let store = store_arc.lock().await;
                    if let Err(e) = store.update_run(&agent_run) {
                        eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                    }
                }
                return Err(message);
            }
        }
        msg
    } else {
        None
    };

    let scheduler_clone = state.scheduler.lock().await.clone();
    let model_route = scheduler_clone
        .preview_chat_route(Some(&tools_prompt))
        .await;
    let cfg = state.config.lock().await;
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler_clone.clone(), &cfg);
    drop(cfg);

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.clone(),
        user_text: user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages: desensitized_messages.clone(),
        layer,
    };

    let mut reasoning_trace = ReasoningTrace::default();
    let mut messages_with_reasoning = desensitized_messages.clone();

    let _actual_layer = if layer == Layer::L3 {
        let runtime_output = agent_runtime
            .execute_task(
                &task,
                &life_model,
                &tools_prompt,
                None,
                vec![],
                privacy_engine.clone(),
            )
            .await;

        match runtime_output {
            Ok(output) => {
                messages_with_reasoning = output.final_messages;
                reasoning_trace = output.reasoning_trace;
                agent_run.reasoning_strategy = Some("layered".to_string());
                agent_run.reasoning_trace = Some(reasoning_trace.clone());
                Layer::L3
            }
            Err(e) => {
                eprintln!("[AgentRuntime] Reasoning failed: {}, falling back to L2", e);
                agent_run.reasoning_strategy = Some("direct".to_string());
                let lr = state.layer_router.lock().await;
                lr.fallback(Layer::L3).unwrap_or(Layer::L2)
            }
        }
    } else {
        agent_run.reasoning_strategy = Some("direct".to_string());
        layer
    };

    let _ = app_handle.emit(
        "stream-message-start",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reasoning_trace": reasoning_trace.clone(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );

    let mut full_reply = String::new();
    if let Some(ref ex) = reasoning_trace.generation_result {
        if let Some(text) = ex.get("text").and_then(|t| t.as_str()) {
            full_reply = text.to_string();
            let _ = app_handle.emit(
                "stream-message-chunk",
                serde_json::json!({
                    "session_id": &session_id,
                    "chunk": text,
                }),
            );
        }
    }

    if full_reply.is_empty() {
        match timeout(
            Duration::from_secs(STREAM_INIT_TIMEOUT_SECS),
            scheduler_clone.generate_stream(
                messages_with_reasoning.clone(),
                &life_model,
                Some(&tools_prompt),
            ),
        )
        .await
        .map_err(|_| format!("流式响应初始化超时（{} 秒）", STREAM_INIT_TIMEOUT_SECS))
        .and_then(|result| result.map_err(|e| e.to_string()))
        {
            Ok(mut stream) => loop {
                let next_chunk = match timeout(
                    Duration::from_secs(STREAM_CHUNK_TIMEOUT_SECS),
                    stream.next(),
                )
                .await
                {
                    Ok(next) => next,
                    Err(_) => {
                        let stream_error =
                            format!("超过 {} 秒没有收到模型输出", STREAM_CHUNK_TIMEOUT_SECS);
                        match generate_non_stream_fallback(
                            &scheduler_clone,
                            messages_with_reasoning.clone(),
                            &life_model,
                            &tools_prompt,
                        )
                        .await
                        {
                            Ok(reply) => {
                                let fallback_text = if full_reply.is_empty() {
                                    reply
                                } else {
                                    format!(
                                            "\n\n[系统] 流式连接长时间无输出，已自动用非流式请求重试并补全回复：\n\n{}",
                                            reply
                                        )
                                };
                                reasoning_trace.errors.push(format!(
                                    "流式响应超时，已降级为非流式响应：{}",
                                    stream_error
                                ));
                                full_reply.push_str(&fallback_text);
                                let _ = app_handle.emit(
                                    "stream-message-chunk",
                                    serde_json::json!({
                                        "session_id": &session_id,
                                        "chunk": fallback_text,
                                    }),
                                );
                                break;
                            }
                            Err(fallback_error) => {
                                let message = format!(
                                    "流式响应超时，非流式重试也失败：{}；重试错误：{}",
                                    stream_error, fallback_error
                                );
                                emit_stream_error(
                                    &app_handle,
                                    &session_id,
                                    &agent_run.id,
                                    message.clone(),
                                );
                                let error = openlife_core::agent::AgentRunError {
                                    message: message.clone(),
                                    phase: "stream".to_string(),
                                    recoverable: true,
                                };
                                agent_run.fail(error);
                                if let Some(ref store_arc) = state.agent_run_store {
                                    let store = store_arc.lock().await;
                                    if let Err(e) = store.update_run(&agent_run) {
                                        eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                                    }
                                }
                                return Err(message);
                            }
                        }
                    }
                };
                let Some(chunk_result) = next_chunk else {
                    break;
                };
                match chunk_result {
                    Ok(chunk) => {
                        if !chunk.is_empty() {
                            full_reply.push_str(&chunk);
                            let _ = app_handle.emit(
                                "stream-message-chunk",
                                serde_json::json!({
                                    "session_id": &session_id,
                                    "chunk": chunk,
                                }),
                            );
                        }
                    }
                    Err(e) => {
                        let stream_error = e.to_string();
                        match generate_non_stream_fallback(
                            &scheduler_clone,
                            messages_with_reasoning.clone(),
                            &life_model,
                            &tools_prompt,
                        )
                        .await
                        {
                            Ok(reply) => {
                                let fallback_text = if full_reply.is_empty() {
                                    reply
                                } else {
                                    format!(
                                            "\n\n[系统] 流式连接中断，已自动用非流式请求重试并补全回复：\n\n{}",
                                            reply
                                        )
                                };
                                reasoning_trace.errors.push(format!(
                                    "流式响应中断，已降级为非流式响应：{}",
                                    stream_error
                                ));
                                full_reply.push_str(&fallback_text);
                                let _ = app_handle.emit(
                                    "stream-message-chunk",
                                    serde_json::json!({
                                        "session_id": &session_id,
                                        "chunk": fallback_text,
                                    }),
                                );
                                break;
                            }
                            Err(fallback_error) => {
                                let message = format!(
                                    "流式响应失败，非流式重试也失败：{}；重试错误：{}",
                                    stream_error, fallback_error
                                );
                                emit_stream_error(
                                    &app_handle,
                                    &session_id,
                                    &agent_run.id,
                                    message.clone(),
                                );
                                let error = openlife_core::agent::AgentRunError {
                                    message: message.clone(),
                                    phase: "stream".to_string(),
                                    recoverable: true,
                                };
                                agent_run.fail(error);
                                if let Some(ref store_arc) = state.agent_run_store {
                                    let store = store_arc.lock().await;
                                    if let Err(e) = store.update_run(&agent_run) {
                                        eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                                    }
                                }
                                return Err(message);
                            }
                        }
                    }
                }
            },
            Err(stream_error) => {
                let stream_error = stream_error.to_string();
                match generate_non_stream_fallback(
                    &scheduler_clone,
                    messages_with_reasoning.clone(),
                    &life_model,
                    &tools_prompt,
                )
                .await
                {
                    Ok(reply) => {
                        reasoning_trace.errors.push(format!(
                            "流式响应初始化失败，已降级为非流式响应：{}",
                            stream_error
                        ));
                        full_reply = reply.clone();
                        let _ = app_handle.emit(
                            "stream-message-chunk",
                            serde_json::json!({
                                "session_id": &session_id,
                                "chunk": reply,
                            }),
                        );
                    }
                    Err(fallback_error) => {
                        let message = format!(
                            "流式响应初始化失败，非流式重试也失败：{}；重试错误：{}",
                            stream_error, fallback_error
                        );
                        emit_stream_error(&app_handle, &session_id, &agent_run.id, message.clone());
                        let error = openlife_core::agent::AgentRunError {
                            message: message.clone(),
                            phase: "stream".to_string(),
                            recoverable: true,
                        };
                        agent_run.fail(error);
                        if let Some(ref store_arc) = state.agent_run_store {
                            let store = store_arc.lock().await;
                            if let Err(e) = store.update_run(&agent_run) {
                                eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                            }
                        }
                        return Err(message);
                    }
                }
            }
        }
        if full_reply.trim().is_empty() {
            let stream_error = "流式响应已结束，但没有收到可显示内容".to_string();
            match generate_non_stream_fallback(
                &scheduler_clone,
                messages_with_reasoning.clone(),
                &life_model,
                &tools_prompt,
            )
            .await
            {
                Ok(reply) => {
                    reasoning_trace.errors.push(format!(
                        "流式响应为空，已降级为非流式响应：{}",
                        stream_error
                    ));
                    full_reply = reply.clone();
                    let _ = app_handle.emit(
                        "stream-message-chunk",
                        serde_json::json!({
                            "session_id": &session_id,
                            "chunk": reply,
                        }),
                    );
                }
                Err(fallback_error) => {
                    let message = format!(
                        "流式响应为空，非流式重试也失败：{}；重试错误：{}",
                        stream_error, fallback_error
                    );
                    emit_stream_error(&app_handle, &session_id, &agent_run.id, message.clone());
                    let error = openlife_core::agent::AgentRunError {
                        message: message.clone(),
                        phase: "stream".to_string(),
                        recoverable: true,
                    };
                    agent_run.fail(error);
                    if let Some(ref store_arc) = state.agent_run_store {
                        let store = store_arc.lock().await;
                        if let Err(e) = store.update_run(&agent_run) {
                            eprintln!("[AgentRun] 更新运行记录失败: {}", e);
                        }
                    }
                    return Err(message);
                }
            }
        }
    }

    let mut first_reply = privacy_engine.reconstruct(&full_reply, &privacy_map);
    if let Some(msg) = auto_checkin_msg_stream {
        if !first_reply.contains(&msg) {
            first_reply = format!("{}\n\n[系统] {}", first_reply, msg);
        }
    }

    let reply = first_reply;
    let tool_calls: Vec<ToolCallResult> = vec![];

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    let context_summary = openlife_core::agent::ContextSummary {
        life_model_empty: life_model.is_effectively_empty(),
        included_life_model_sections: included_life_model_sections(&life_model),
        memory_hit_count: context_summary.memory_hit_count,
        memory_sources: context_summary.memory_sources,
        used_tools_prompt: !tools_prompt.is_empty(),
        redaction_applied: !privacy_map.is_empty(),
        redaction_level: if privacy_map.is_empty() {
            openlife_core::agent::types::RedactionLevel::None
        } else {
            openlife_core::agent::types::RedactionLevel::Light
        },
    };
    let preview = preview_text(&reply, 200);
    agent_run.complete(&preview, model_route, context_summary);
    if let Err(e) = finalize_chat_agent_run(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        &state,
    )
    .await
    {
        eprintln!("[Stream] finalize_chat_agent_run failed: {}", e);
        let _ = app_handle.emit(
            "stream-message-error",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": agent_run.id,
                "error": format!("AgentRun 持久化失败: {}", e),
            }),
        );
        return Err(e);
    }

    let _ = app_handle.emit(
        "stream-message-done",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reply": reply,
            "reasoning_trace": reasoning_trace,
            "tool_calls": tool_calls,
        }),
    );

    Ok(())
}
