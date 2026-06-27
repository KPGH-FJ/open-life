use openlife_core::llm::ChatMessage;
use std::sync::Arc;

use crate::main_chat_turn_pipeline::{
    run_main_chat_turn_pipeline_buffered, MainChatTurnDelivery, MainChatTurnPipelineInput,
    MainChatTurnStreamMode,
};
use crate::{AppState, SendMessageResult};

pub(crate) async fn send_message_with_state(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
) -> Result<SendMessageResult, String> {
    let output = run_main_chat_turn_pipeline_buffered(
        MainChatTurnPipelineInput {
            session_id,
            messages,
            selected_skill_id,
            stream_mode: MainChatTurnStreamMode::Buffered,
        },
        state,
    )
    .await?;
    debug_assert!(!output.route_decision.reason_code.is_empty());

    match output.delivery {
        MainChatTurnDelivery::Buffered { result } => Ok(result),
        MainChatTurnDelivery::Streamed { .. } => {
            Err("MainChatTurnPipeline returned streaming delivery to send_message".into())
        }
    }
}
