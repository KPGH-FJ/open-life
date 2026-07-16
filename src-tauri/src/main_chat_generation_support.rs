use std::sync::Arc;

use openlife_core::agent::ReasoningTrace;
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;

use crate::AppState;

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

pub(crate) async fn finalize_chat_agent_run(
    session_id: &str,
    assistant_message: &ChatMessage,
    reply: &str,
    reasoning_trace: &mut ReasoningTrace,
    agent_run: &mut openlife_core::agent::AgentRun,
    state: &Arc<AppState>,
) -> Result<(), String> {
    if assistant_message.role != "assistant" {
        return Err("Main Chat finalization requires role=assistant".into());
    }
    if agent_run.task_id.trim().is_empty() || agent_run.id.trim().is_empty() {
        return Err("Main Chat finalization requires task/run-bound message identity".into());
    }
    commit_main_chat_assistant_message(
        session_id,
        assistant_message,
        &agent_run.task_id,
        &agent_run.id,
        state,
    )
    .await?;

    reasoning_trace.generation_result = Some(match reasoning_trace.generation_result.take() {
        Some(serde_json::Value::Object(mut metadata)) => {
            metadata.insert("text".into(), serde_json::Value::String(reply.to_string()));
            serde_json::Value::Object(metadata)
        }
        _ => serde_json::json!({ "text": reply }),
    });
    mark_vector_persistence_skipped(reasoning_trace, "chat_turn_canonical_conversation_only");
    agent_run.output_preview = Some(preview_text(reply, 200));
    if agent_run.status == openlife_core::agent::AgentRunStatus::Running {
        agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    }
    agent_run.finished_at = Some(chrono::Utc::now());
    agent_run.reasoning_trace = Some(reasoning_trace.clone());

    crate::terminal_owner_write_gateway::update_agent_run(state, agent_run)
        .await
        .map_err(|err| format!("update canonical AgentRun during finalization failed: {err}"))?;

    // Proposal creation is intentionally absent from terminalization. Typed
    // PolicyDecision/ReviewWorkflow paths must stage proposals before this
    // point; ordinary chat completion cannot reinterpret prose into writes.
    Ok(())
}

pub(crate) fn main_chat_assistant_message_operation_id(
    task_session_id: &str,
    run_id: &str,
) -> String {
    format!("main_chat_assistant_message:{task_session_id}:{run_id}")
}

pub(crate) async fn commit_main_chat_assistant_message(
    session_id: &str,
    assistant_message: &ChatMessage,
    task_session_id: &str,
    run_id: &str,
    state: &Arc<AppState>,
) -> Result<openlife_core::memory::CanonicalConversationMessageReceipt, String> {
    if assistant_message.role != "assistant" {
        return Err("Main Chat assistant commit requires role=assistant".into());
    }
    crate::memory_gateway::save_conversation_message_idempotent_with_state(
        session_id,
        assistant_message,
        &main_chat_assistant_message_operation_id(task_session_id, run_id),
        state,
    )
    .await
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
