use std::sync::Arc;

use openlife_core::agent::ReasoningTrace;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;
use openlife_core::vectors::{embed_text_with_privacy, VectorInsertItem};
use tokio::time::{timeout, Duration};

use crate::main_chat_hs_runtime::classify_hs_policy_topic;
use crate::AppState;

const NON_STREAM_FALLBACK_TIMEOUT_SECS: u64 = 120;
const CHAT_VECTOR_PERSIST_TIMEOUT_SECS: u64 = 8;
const CHAT_PROPOSAL_GENERATION_TIMEOUT_SECS: u64 = 5;

async fn generate_and_persist_chat_proposals(
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
                log::warn!("[ChatProposal] Proposal generation failed: {}", e);
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
                log::warn!("[ChatProposal] Failed to save proposal: {}", e);
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
                log::warn!("[AgentRun] 关联 Chat Proposal 失败: {}", e);
            }
        }
    }
}

fn should_generate_chat_proposals(
    agent_run: &openlife_core::agent::AgentRun,
    reasoning_trace: &ReasoningTrace,
) -> bool {
    if !agent_run.generated_proposals.is_empty() || agent_run.tool_call_count > 0 {
        return false;
    }

    let Some(metadata) = reasoning_trace.generation_result.as_ref() else {
        return true;
    };
    let selected_strategy = metadata
        .get("selectedStrategy")
        .and_then(serde_json::Value::as_str);
    let kernel_read_only = metadata
        .get("kernelBackedReadOnlyToolLoop")
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    let proposal_only_write = metadata
        .get("kernelBackedProposalOnlyWrite")
        .and_then(serde_json::Value::as_bool)
        == Some(true);

    if kernel_read_only || proposal_only_write {
        return false;
    }

    selected_strategy
        .map(|strategy| strategy == "direct_answer")
        .unwrap_or(true)
}

pub(crate) async fn persist_chat_message_if_needed(
    session_id: &str,
    msg: &ChatMessage,
    state: &Arc<AppState>,
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
    state: &Arc<AppState>,
) {
    if let Some(reason) = state.vector_persistence_mode.skip_reason() {
        log::debug!(
            "[memory] vector persistence skipped for {} message in session {}: {}",
            msg.role,
            session_id,
            reason
        );
        return;
    }

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
    let privacy_engine = state.privacy_engine.lock().await.clone();
    let hs_local_only =
        classify_hs_policy_topic(content, "") != openlife_core::agent::PolicyTopic::General;
    let embedding = match embed_text_with_privacy(
        content,
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
        Ok(embedding) if !embedding.is_empty() => embedding,
        Ok(_) => return,
        Err(e) => {
            log::warn!(
                "[memory] embedding generation failed for {} message in session {}: {}",
                msg.role,
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
        source: if msg.role == "assistant" {
            "assistant_reply"
        } else {
            "user_message"
        },
    };
    if let Err(e) = store.insert_batch(&[item]) {
        log::warn!(
            "[memory] vector insert failed for {} message in session {}: {}",
            msg.role,
            session_id,
            e
        );
    }
}

fn mark_vector_persistence_skipped(reasoning_trace: &mut ReasoningTrace, reason: &str) {
    match reasoning_trace.generation_result.as_mut() {
        Some(serde_json::Value::Object(metadata)) => {
            metadata.insert(
                "vectorPersistenceSkipped".into(),
                serde_json::Value::String(reason.to_string()),
            );
        }
        _ => {
            reasoning_trace.generation_result = Some(serde_json::json!({
                "vectorPersistenceSkipped": reason,
            }));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn finalize_chat_agent_run(
    session_id: &str,
    assistant_message: &ChatMessage,
    reply: &str,
    reasoning_trace: &mut ReasoningTrace,
    agent_run: &mut openlife_core::agent::AgentRun,
    life_model: &LifeModel,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let inserted = persist_chat_message_if_needed(session_id, assistant_message, state).await?;

    reasoning_trace.generation_result = Some(match reasoning_trace.generation_result.take() {
        Some(serde_json::Value::Object(mut metadata)) => {
            metadata.insert("text".into(), serde_json::Value::String(reply.to_string()));
            serde_json::Value::Object(metadata)
        }
        _ => serde_json::json!({ "text": reply }),
    });
    if inserted {
        if let Some(reason) = state.vector_persistence_mode.skip_reason() {
            mark_vector_persistence_skipped(reasoning_trace, reason);
        }
    }
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
                    log::warn!("[AgentRun] 更新运行记录失败: {}", e);
                }
            }
            Ok(None) => {
                if let Err(e) = store.create_run(agent_run) {
                    log::warn!("[AgentRun] 保存运行记录失败: {}", e);
                }
            }
            Err(e) => {
                log::warn!("[AgentRun] 查询运行记录失败: {}", e);
                if let Err(e) = store.create_run(agent_run) {
                    log::warn!("[AgentRun] 保存运行记录失败: {}", e);
                }
            }
        }
    }

    if inserted
        && state.vector_persistence_mode.skip_reason().is_none()
        && timeout(
            Duration::from_secs(CHAT_VECTOR_PERSIST_TIMEOUT_SECS),
            persist_vector_memory_for_message(session_id, assistant_message, state),
        )
        .await
        .is_err()
    {
        log::warn!(
            "[memory] vector persistence timed out after {}s for assistant message in session {}",
            CHAT_VECTOR_PERSIST_TIMEOUT_SECS,
            session_id
        );
    }

    if should_generate_chat_proposals(agent_run, reasoning_trace) {
        if timeout(
            Duration::from_secs(CHAT_PROPOSAL_GENERATION_TIMEOUT_SECS),
            generate_and_persist_chat_proposals(state, agent_run, reply, life_model),
        )
        .await
        .is_err()
        {
            eprintln!(
                "[ChatProposal] Proposal generation timed out after {}s for run {}",
                CHAT_PROPOSAL_GENERATION_TIMEOUT_SECS, agent_run.id
            );
        }
    }
    Ok(())
}

pub(crate) async fn generate_non_stream_fallback(
    scheduler: &InferenceScheduler,
    messages: Vec<ChatMessage>,
    life_model: &LifeModel,
    tools_prompt: &str,
    hs_packet: Option<openlife_core::agent::RuntimeHSPacket>,
) -> Result<String, String> {
    let fallback = async {
        if let Some(packet) = hs_packet {
            scheduler
                .generate_with_hs_packet(messages, life_model, Some(tools_prompt), &packet)
                .await
        } else {
            scheduler
                .generate(messages, life_model, Some(tools_prompt))
                .await
        }
    };

    timeout(
        Duration::from_secs(NON_STREAM_FALLBACK_TIMEOUT_SECS),
        fallback,
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

pub(crate) fn main_chat_provider_endpoint_kind(
    scheduler: &InferenceScheduler,
    scripted_provider_response: bool,
) -> &'static str {
    if scripted_provider_response {
        return "scripted_scheduler_response";
    }

    let base = scheduler.openai_base.trim().to_ascii_lowercase();
    if base.starts_with("http://127.0.0.1")
        || base.starts_with("http://localhost")
        || base.starts_with("http://[::1]")
    {
        return "local_test_http";
    }

    if scheduler.provider.trim().eq_ignore_ascii_case("none") {
        "none"
    } else {
        "external_provider"
    }
}

pub(crate) fn preview_text(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}
