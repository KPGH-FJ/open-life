use openlife_core::llm::ChatMessage;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::main_chat_turn_runtime::{
    MainChatTurnDelivery, MainChatTurnStreamMode, OpenLifeTurnInput, OpenLifeTurnRuntime,
};
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

pub(crate) fn send_message_with_operation_state(
    operation_id: String,
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
) -> Pin<Box<dyn Future<Output = Result<SendMessageResult, String>> + Send + '_>> {
    Box::pin(async move {
        state
            .persistence_coordinator
            .require_effects_allowed()
            .map_err(|error| error.to_string())?;
        let runtime = OpenLifeTurnRuntime::new(state);
        let output = runtime
            .run_buffered(OpenLifeTurnInput {
                operation_id,
                session_id,
                messages,
                selected_skill_id,
                stream_mode: MainChatTurnStreamMode::Buffered,
            })
            .await?;
        debug_assert!(!output.route_decision.reason_code.is_empty());
        debug_assert_eq!(
            output.terminal.runtime_owner,
            crate::main_chat_turn_runtime::OPENLIFE_TURN_RUNTIME_OWNER
        );

        match output.delivery {
            MainChatTurnDelivery::Buffered { result } => Ok(*result),
            MainChatTurnDelivery::Streamed { .. } => {
                Err("MainChatTurnPipeline returned streaming delivery to send_message".into())
            }
        }
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

/// Explicit test-only compatibility for historical fixtures that predate the
/// shipped operation contract. Product IPC and new D050 tests must call
/// `send_message_with_operation_state` with a caller-owned UUIDv4.
#[cfg(test)]
pub(crate) fn send_message_with_state(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
) -> Pin<Box<dyn Future<Output = Result<SendMessageResult, String>> + Send + '_>> {
    send_message_with_operation_state(
        uuid::Uuid::new_v4().to_string(),
        session_id,
        messages,
        selected_skill_id,
        state,
    )
}
