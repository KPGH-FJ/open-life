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
    provider_profile_id: Option<String>,
    reasoning_effort: Option<openlife_core::conversation::ReasoningEffort>,
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
                provider_profile_id,
                reasoning_effort,
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
    input: crate::canonical_work_runtime::CanonicalWorkInput,
    state: &Arc<AppState>,
) -> Pin<Box<dyn Future<Output = Result<SendMessageResult, String>> + Send + '_>> {
    Box::pin(async move {
        let mut discard_stream_event = |_event: &str, _payload: serde_json::Value| {};
        crate::canonical_work_runtime::run_canonical_work(input, state, &mut discard_stream_event)
            .await
            .map(|output| output.result)
    })
}
