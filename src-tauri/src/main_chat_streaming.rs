use openlife_core::llm::ChatMessage;
use std::sync::Arc;
use std::time::Duration;

use crate::main_chat_turn_pipeline::{
    run_main_chat_turn_pipeline_streaming, MainChatTurnDelivery, MainChatTurnPipelineInput,
    MainChatTurnStreamMode,
};
use crate::AppState;

const STREAM_INIT_TIMEOUT_SECS: u64 = 45;
const STREAM_CHUNK_TIMEOUT_SECS: u64 = 90;

pub(crate) async fn start_stream_message_with_state(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
    mut emit_stream_event: impl FnMut(&str, serde_json::Value) + Send,
) -> Result<serde_json::Value, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(STREAM_INIT_TIMEOUT_SECS + STREAM_CHUNK_TIMEOUT_SECS),
        run_main_chat_turn_pipeline_streaming(
            MainChatTurnPipelineInput {
                session_id,
                messages,
                selected_skill_id,
                stream_mode: MainChatTurnStreamMode::Streaming,
            },
            state,
            &mut emit_stream_event,
        ),
    )
    .await
    .map_err(|_| "start_stream_message timed out before stream completion".to_string())??;
    debug_assert!(!output.route_decision.reason_code.is_empty());

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
        MainChatTurnDelivery::Buffered { .. } => {
            Err("MainChatTurnPipeline returned buffered delivery to start_stream_message".into())
        }
    }
}
