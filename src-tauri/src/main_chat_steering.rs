use crate::errors::AppError;
use crate::state::AppState;
use openlife_core::agent::metadata_safe_text_digest;
use openlife_core::llm::ChatMessage;
use openlife_core::task_runtime::{CanonicalReportSteeringRecord, SubmitReportSteeringInput};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

const MAX_STEERING_CHARS: usize = 4_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubmitMainChatSteeringResponse {
    pub steering: CanonicalReportSteeringRecord,
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
    task_session_id: &str,
    run_id: &str,
    session_id: &str,
    content: &str,
    state: &Arc<AppState>,
) -> Result<SubmitMainChatSteeringResponse, String> {
    if steering_id.trim().is_empty()
        || task_session_id.trim().is_empty()
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
    let (task_id, base_plan_revision) = store
        .lock()
        .await
        .resolve_report_run_target_for_conversation(task_session_id, run_id, session_id)
        .map_err(|error| format!("load steering target failed: {error}"))?
        .ok_or_else(|| "canonical_report_steering_target_missing".to_string())?;
    let message = ChatMessage {
        role: "user".into(),
        content: content.to_string(),
    };
    let operation_id = format!("steering:{steering_id}");
    let commit = crate::memory_gateway::save_conversation_message_idempotent_with_state(
        session_id,
        &message,
        &operation_id,
        state,
    )
    .await?;
    let steering_digest = metadata_safe_text_digest(content).1;
    if commit.content_digest != steering_digest {
        return Err("main_chat_steering_message_digest_drift".into());
    }
    let scope_expansion_blocked = steering_requests_scope_expansion(content);
    let steering = store
        .lock()
        .await
        .submit_report_steering(SubmitReportSteeringInput {
            steering_id,
            task_id: &task_id,
            run_id,
            source_message_ref: &commit.canonical_ref,
            source_message_digest: &commit.content_digest,
            steering_digest: &steering_digest,
            base_plan_revision,
            scope_expansion_blocked,
        })
        .map_err(|error| format!("submit canonical report steering failed: {error}"))?;
    Ok(SubmitMainChatSteeringResponse {
        steering,
        scope_expansion_blocked,
    })
}

#[tauri::command]
pub(crate) async fn submit_main_chat_task_steering(
    steering_id: String,
    task_session_id: String,
    run_id: String,
    session_id: String,
    content: String,
    state: State<'_, Arc<AppState>>,
) -> Result<SubmitMainChatSteeringResponse, AppError> {
    submit_main_chat_task_steering_with_state(
        &steering_id,
        &task_session_id,
        &run_id,
        &session_id,
        &content,
        state.inner(),
    )
    .await
    .map_err(AppError::internal)
}
