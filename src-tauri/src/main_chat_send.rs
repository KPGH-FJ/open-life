use openlife_core::llm::ChatMessage;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::{AppState, SendMessageResult};

pub(crate) fn send_canonical_chat_with_state(
    turn_id: String,
    conversation_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
) -> Pin<Box<dyn Future<Output = Result<SendMessageResult, String>> + Send + '_>> {
    Box::pin(async move {
        let mut discard_stream_event = |_event: &str, _payload: serde_json::Value| {};
        crate::canonical_chat_runtime::run_canonical_chat(
            crate::canonical_chat_runtime::CanonicalChatInput {
                turn_id,
                conversation_id,
                messages,
                selected_skill_id,
                stream: false,
            },
            state,
            &mut discard_stream_event,
        )
        .await
        .map(|output| output.result)
    })
}

pub(crate) fn send_canonical_work_with_state(
    turn_id: String,
    task_id: String,
    run_id: String,
    conversation_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
) -> Pin<Box<dyn Future<Output = Result<SendMessageResult, String>> + Send + '_>> {
    Box::pin(async move {
        let mut discard_stream_event = |_event: &str, _payload: serde_json::Value| {};
        crate::canonical_work_runtime::run_canonical_work(
            crate::canonical_work_runtime::CanonicalWorkInput {
                task_id,
                run_id,
                turn_id,
                conversation_id,
                messages,
                selected_skill_id,
                stream: false,
            },
            state,
            &mut discard_stream_event,
        )
        .await
        .map(|output| output.result)
    })
}
