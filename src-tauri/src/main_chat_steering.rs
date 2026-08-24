use crate::errors::AppError;
use crate::state::AppState;
use openlife_core::agent::metadata_safe_text_digest;
use openlife_core::task_runtime::{CanonicalSteeringRecord, SubmitSteeringInput};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

const MAX_STEERING_CHARS: usize = 4_000;

fn steering_app_error(error: String) -> AppError {
    let safe_code = [
        "main_chat_steering_input_invalid",
        "canonical_task_runtime_store_unavailable",
        "conversation_store_unavailable",
        "canonical_steering_target_missing",
        "canonical_steering_target_terminal",
        "canonical_steering_plan_revision_stale",
        "canonical_steering_checkpoint_passed",
        "canonical_steering_pending_conflict",
        "conversation_steering_target_not_running",
        "main_chat_steering_message_digest_drift",
    ]
    .into_iter()
    .find(|code| error.contains(code))
    .unwrap_or("main_chat_steering_failed");
    AppError::internal_with_code("Work steering was not accepted", safe_code)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubmitMainChatSteeringResponse {
    pub steering: CanonicalSteeringRecord,
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
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore", "CanonicalTaskRuntimeStore"])
        .map_err(|error| error.to_string())?;
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let task_store = store.lock().await;
    let target = task_store
        .resolve_general_run_target_for_conversation(task_id, run_id, session_id)
        .map_err(|error| format!("load steering target failed: {error}"))?
        .ok_or_else(|| "canonical_steering_target_missing".to_string())?;
    task_store
        .validate_steering_admission(&target.task_id, run_id, target.plan_revision)
        .map_err(|error| format!("validate canonical Work steering failed: {error}"))?;
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
    let steering = task_store
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
        })
        .map_err(|error| format!("submit canonical Work steering failed: {error}"))?;
    Ok(SubmitMainChatSteeringResponse { steering })
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
    .map_err(steering_app_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::conversation::{BeginChatTurn, ConversationItemKind};
    use openlife_core::task_runtime::{
        BeginGeneralTaskRunInput, BeginItemAttemptInput, CanonicalSteeringStatus,
        CanonicalTaskItemKind,
    };

    #[test]
    fn command_error_exposes_only_a_stable_steering_code() {
        let error = steering_app_error(
            "validate canonical Work steering failed: canonical_steering_checkpoint_passed".into(),
        );
        let serialized = serde_json::to_value(error).expect("serialize steering AppError");
        assert_eq!(
            serialized["detail"]["code"],
            serde_json::Value::String("canonical_steering_checkpoint_passed".into())
        );
        assert_eq!(
            serialized["detail"]["message"],
            serde_json::Value::String("Work steering was not accepted".into())
        );
    }

    async fn running_work_target() -> (Arc<AppState>, String, String, String, String) {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_local_http_provider(
            &state,
            "unused",
        )
        .await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Steer active Work")
            .unwrap();
        let provider = crate::provider_registry::selected_provider_profile(&state)
            .await
            .unwrap()
            .binding;
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_chat_turn_with_proof(BeginChatTurn {
                turn_id: &turn_id,
                conversation_id: &conversation_id,
                user_message: "prepare a brief",
                provider: &provider,
            })
            .unwrap();
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_general_task_run(BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: &turn_id,
                instruction_digest: &metadata_safe_text_digest("prepare a brief").1,
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
                execution_mode: openlife_core::task_runtime::WorkExecutionMode::ScopedAgent,
            })
            .unwrap();
        (state, conversation_id, task_id, run_id, turn_id)
    }

    #[tokio::test]
    async fn planning_phase_steering_binds_the_same_conversation_turn_and_work_run() {
        let (state, conversation_id, task_id, run_id, turn_id) = running_work_target().await;
        let request_digest = metadata_safe_text_digest("planning request").1;
        {
            let store = state
                .canonical_task_runtime_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            store
                .append_general_item(
                    &task_id,
                    &run_id,
                    "planning-provider-item",
                    CanonicalTaskItemKind::ProviderGeneration,
                    "work_plan_generation",
                    &request_digest,
                )
                .unwrap();
            store
                .begin_item_attempt(BeginItemAttemptInput {
                    attempt_id: &uuid::Uuid::new_v4().to_string(),
                    task_id: &task_id,
                    run_id: &run_id,
                    item_id: "planning-provider-item",
                    executor_kind: "provider",
                    provider_profile_id: None,
                    provider_model_id: None,
                    provider_reasoning_effort: None,
                    request_digest: &request_digest,
                })
                .unwrap();
        }
        let steering_id = uuid::Uuid::new_v4().to_string();
        let response = submit_main_chat_task_steering_with_state(
            &steering_id,
            &task_id,
            &run_id,
            &conversation_id,
            "put privacy risks first",
            &state,
        )
        .await
        .unwrap();
        assert_eq!(response.steering.status, CanonicalSteeringStatus::Pending);
        let turn = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_turn(&turn_id)
            .unwrap()
            .unwrap();
        assert!(turn.items.iter().any(|item| {
            item.kind == ConversationItemKind::UserSteering
                && item.content == "put privacy risks first"
        }));
    }

    #[tokio::test]
    async fn closed_final_generation_checkpoint_rejects_before_conversation_append() {
        let (state, conversation_id, task_id, run_id, turn_id) = running_work_target().await;
        let request_digest = metadata_safe_text_digest("final provider request").1;
        {
            let store = state
                .canonical_task_runtime_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            store
                .append_general_item(
                    &task_id,
                    &run_id,
                    "final-provider-item",
                    CanonicalTaskItemKind::ProviderGeneration,
                    "work_provider_generation",
                    &request_digest,
                )
                .unwrap();
            store
                .begin_item_attempt(BeginItemAttemptInput {
                    attempt_id: &uuid::Uuid::new_v4().to_string(),
                    task_id: &task_id,
                    run_id: &run_id,
                    item_id: "final-provider-item",
                    executor_kind: "provider",
                    provider_profile_id: None,
                    provider_model_id: None,
                    provider_reasoning_effort: None,
                    request_digest: &request_digest,
                })
                .unwrap();
        }
        let before = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_turn(&turn_id)
            .unwrap()
            .unwrap()
            .items
            .len();
        let error = submit_main_chat_task_steering_with_state(
            &uuid::Uuid::new_v4().to_string(),
            &task_id,
            &run_id,
            &conversation_id,
            "change the conclusion",
            &state,
        )
        .await
        .unwrap_err();
        assert!(error.contains("canonical_steering_checkpoint_passed"));
        let after = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_turn(&turn_id)
            .unwrap()
            .unwrap()
            .items
            .len();
        assert_eq!(after, before);
    }
}
