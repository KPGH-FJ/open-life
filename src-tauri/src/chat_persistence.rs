use openlife_core::agent::ReasoningTrace;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::vectors::{embed_text_with_config, VectorInsertItem};
use std::sync::Arc;
use tauri::State;
use tokio::time::{timeout, Duration};

use crate::state::AppState;
use crate::types::preview_text;

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
                if let Some(ref es) = state.agent_run_event_store {
                    if let Err(e) = es.append_event(&openlife_core::agent::AgentRunEvent::new(
                        &agent_run.id,
                        openlife_core::agent::AgentRunEventType::RunFailed,
                        openlife_core::agent::AgentEventActor::Runtime,
                        format!("chat proposal generation failed: {}", e),
                        serde_json::json!({"phase": "proposal_generation", "error": e.to_string()}),
                    )) {
                        log::error!("[AgentRun] Failed to append RunFailed event: {}", e);
                    }
                }
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

pub(crate) async fn persist_chat_message_if_needed(
    session_id: &str,
    msg: &ChatMessage,
    state: &State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    persist_chat_message_if_needed_inner(session_id, msg, state.inner()).await
}

pub(crate) async fn persist_chat_message_if_needed_inner(
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
    state: &State<'_, Arc<AppState>>,
) {
    persist_vector_memory_for_message_inner(session_id, msg, state.inner()).await;
}

pub(crate) async fn persist_vector_memory_for_message_inner(
    session_id: &str,
    msg: &ChatMessage,
    state: &Arc<AppState>,
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
                "[memory] embedding generation failed for {} message in session {}: {} - chat_persistence.rs",
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
            "[memory] vector insert failed for {} message in session {}: {} - chat_persistence.rs",
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
    finalize_chat_agent_run_inner(
        session_id,
        assistant_message,
        reply,
        reasoning_trace,
        agent_run,
        life_model,
        state.inner(),
    )
    .await
}

pub(crate) async fn finalize_chat_agent_run_inner(
    session_id: &str,
    assistant_message: &ChatMessage,
    reply: &str,
    reasoning_trace: &mut ReasoningTrace,
    agent_run: &mut openlife_core::agent::AgentRun,
    life_model: &LifeModel,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let inserted =
        persist_chat_message_if_needed_inner(session_id, assistant_message, state).await?;

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
        && timeout(
            Duration::from_secs(CHAT_VECTOR_PERSIST_TIMEOUT_SECS),
            persist_vector_memory_for_message_inner(session_id, assistant_message, state),
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
