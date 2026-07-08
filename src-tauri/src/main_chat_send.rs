use openlife_core::llm::ChatMessage;
use std::sync::Arc;

use crate::main_chat_turn_runtime::{
    MainChatTurnDelivery, MainChatTurnStreamMode, OpenLifeTurnInput, OpenLifeTurnRuntime,
};
use crate::{AppState, SendMessageResult};

pub(crate) async fn send_message_with_state(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
) -> Result<SendMessageResult, String> {
    let runtime = OpenLifeTurnRuntime::new(state);
    let output = runtime
        .run_buffered(OpenLifeTurnInput {
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
}
