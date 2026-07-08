use std::sync::Arc;

use openlife_core::agent::ReasoningTrace;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;
use tokio::time::{timeout, Duration};

use crate::memory_gateway;
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
            match openlife_core::agent::ReviewWorkflow::new(&store).submit(
                openlife_core::agent::DurableWriteRequest::from_agent_proposal(
                    openlife_core::agent::DurableWriteSource::MainChat,
                    openlife_core::agent::DurableWriteSubject::from_proposal_type(
                        proposal.proposal_type,
                    ),
                    proposal,
                    "Main Chat generated proposal is pending Review Center approval.",
                )
                .with_evidence_refs(vec![format!("agent_run:{}", agent_run.id)]),
            ) {
                Ok(outcome) => created_proposal_ids.push(outcome.proposal_id().to_string()),
                Err(e) => log::warn!("[ChatProposal] Failed to save proposal: {}", e),
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
    memory_gateway::save_turn_message_if_needed_with_state(session_id, msg, state).await
}

pub(crate) async fn persist_vector_memory_for_message(
    session_id: &str,
    msg: &ChatMessage,
    state: &Arc<AppState>,
) {
    memory_gateway::persist_vector_memory_for_message_with_state(session_id, msg, state).await;
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

    if should_generate_chat_proposals(agent_run, reasoning_trace)
        && timeout(
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
