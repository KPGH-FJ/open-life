use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::AppState;

pub(crate) fn start_canonical_chat_stream_with_state<'a>(
    input: crate::canonical_chat_runtime::CanonicalChatInput,
    state: &'a Arc<AppState>,
    mut emit_stream_event: impl FnMut(&str, serde_json::Value) + Send + 'a,
) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send + 'a>> {
    Box::pin(async move {
        crate::canonical_chat_runtime::run_canonical_chat(input, state, &mut emit_stream_event)
            .await
            .map(|output| output.done_payload)
    })
}

pub(crate) fn start_canonical_work_stream_with_state<'a>(
    input: crate::canonical_work_runtime::CanonicalWorkInput,
    state: &'a Arc<AppState>,
    mut emit_stream_event: impl FnMut(&str, serde_json::Value) + Send + 'a,
) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send + 'a>> {
    Box::pin(async move {
        crate::canonical_work_runtime::run_canonical_work(input, state, &mut emit_stream_event)
            .await
            .map(|output| output.done_payload)
    })
}
