use openlife_core::llm::ChatMessage;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::main_chat_turn_runtime::{
    MainChatTurnDelivery, MainChatTurnStreamMode, OpenLifeTurnInput, OpenLifeTurnRuntime,
};
use crate::AppState;

pub(crate) fn start_stream_message_with_operation_state<'a>(
    operation_id: String,
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &'a Arc<AppState>,
    mut emit_stream_event: impl FnMut(&str, serde_json::Value) + Send + 'a,
) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send + 'a>> {
    Box::pin(async move {
        state
            .persistence_coordinator
            .require_effects_allowed()
            .map_err(|error| error.to_string())?;
        let runtime = OpenLifeTurnRuntime::new(state);
        let output = runtime
            .run_streaming(
                OpenLifeTurnInput {
                    operation_id,
                    session_id,
                    messages,
                    selected_skill_id,
                    stream_mode: MainChatTurnStreamMode::Streaming,
                },
                &mut emit_stream_event,
            )
            .await?;
        debug_assert!(!output.route_decision.reason_code.is_empty());
        debug_assert_eq!(
            output.terminal.runtime_owner,
            crate::main_chat_turn_runtime::OPENLIFE_TURN_RUNTIME_OWNER
        );

        match output.delivery {
            MainChatTurnDelivery::Streamed {
                run_id,
                legacy_fallback_used,
                kernel_event_count,
                durable_event_count,
                done_payload,
            } => {
                debug_assert!(run_id.as_deref().map_or(true, |id| !id.trim().is_empty()));
                let _ = (
                    legacy_fallback_used,
                    kernel_event_count,
                    durable_event_count,
                );
                Ok(done_payload)
            }
            MainChatTurnDelivery::Buffered { .. } => Err(
                "MainChatTurnPipeline returned buffered delivery to start_stream_message".into(),
            ),
        }
    })
}

/// Explicit test-only compatibility for historical fixtures. Shipped IPC and
/// new D050 tests must provide the stable logical-turn UUIDv4 themselves.
#[cfg(test)]
pub(crate) fn start_stream_message_with_state<'a>(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &'a Arc<AppState>,
    emit_stream_event: impl FnMut(&str, serde_json::Value) + Send + 'a,
) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send + 'a>> {
    start_stream_message_with_operation_state(
        uuid::Uuid::new_v4().to_string(),
        session_id,
        messages,
        selected_skill_id,
        state,
        emit_stream_event,
    )
}
