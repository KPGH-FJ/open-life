use crate::errors::AppError;
use crate::state::AppState;
use openlife_core::agent::metadata_safe_text_digest;
use openlife_core::task_runtime::{CanonicalSteeringRecord, SubmitSteeringInput};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

const MAX_STEERING_CHARS: usize = 4_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubmitMainChatSteeringResponse {
    pub steering: CanonicalSteeringRecord,
    pub scope_expansion_blocked: bool,
}

fn steering_requests_scope_expansion(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    [
        "full access",
        "访问其他目录",
        "workspace 外",
        "联网",
        "network",
        "切换模型",
        "switch model",
        "换 provider",
        "send email",
        "发送邮件",
        "删除",
        "delete",
        "shell",
        "terminal",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

pub(crate) async fn submit_main_chat_task_steering_with_state(
    steering_id: &str,
    task_id: &str,
    run_id: &str,
    session_id: &str,
    content: &str,
    state: &Arc<AppState>,
) -> Result<SubmitMainChatSteeringResponse, String> {
    if steering_id.trim().is_empty()
        || task_id.trim().is_empty()
        || run_id.trim().is_empty()
        || session_id.trim().is_empty()
        || content.trim().is_empty()
        || content.chars().count() > MAX_STEERING_CHARS
    {
        return Err("main_chat_steering_input_invalid".into());
    }
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let target = store
        .lock()
        .await
        .resolve_general_run_target_for_conversation(task_id, run_id, session_id)
        .map_err(|error| format!("load steering target failed: {error}"))?
        .ok_or_else(|| "canonical_steering_target_missing".to_string())?;
    let conversation_store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| "conversation_store_unavailable".to_string())?;
    let commit = conversation_store
        .lock()
        .await
        .append_work_steering(
            steering_id,
            session_id,
            &target.execution_session_id,
            content,
        )
        .map_err(|error| format!("append canonical Conversation steering failed: {error}"))?;
    let steering_digest = metadata_safe_text_digest(content).1;
    if commit.item.content_digest != steering_digest {
        return Err("main_chat_steering_message_digest_drift".into());
    }
    let scope_expansion_blocked = steering_requests_scope_expansion(content);
    let steering = store
        .lock()
        .await
        .submit_steering(SubmitSteeringInput {
            steering_id,
            task_id: &target.task_id,
            run_id,
            source_message_ref: &format!(
                "conversation://{session_id}/turn/{}/item/{}",
                target.execution_session_id, commit.item.id
            ),
            source_message_digest: &commit.item.content_digest,
            steering_digest: &steering_digest,
            base_plan_revision: target.plan_revision,
            scope_expansion_blocked,
        })
        .map_err(|error| format!("submit canonical Work steering failed: {error}"))?;
    Ok(SubmitMainChatSteeringResponse {
        steering,
        scope_expansion_blocked,
    })
}

#[tauri::command]
pub(crate) async fn submit_main_chat_task_steering(
    steering_id: String,
    task_id: String,
    run_id: String,
    session_id: String,
    content: String,
    state: State<'_, Arc<AppState>>,
) -> Result<SubmitMainChatSteeringResponse, AppError> {
    submit_main_chat_task_steering_with_state(
        &steering_id,
        &task_id,
        &run_id,
        &session_id,
        &content,
        state.inner(),
    )
    .await
    .map_err(AppError::internal)
}
