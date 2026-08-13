//! R2 canonical provider-only Work coordinator.
//!
//! Conversation owns user/assistant transcript; CanonicalTaskRuntimeStore owns
//! Task -> Run -> Item -> ItemAttempt -> FinalResult. This release path never
//! creates TaskSession, AgentRun, ActionQueue, or durable Main Chat Events.

use crate::canonical_chat_runtime::{
    provider_state, verify_provider_binding, CanonicalChatEventSink,
};
use crate::main_chat_kernel::{
    MainChatEventSink, MainChatKernel, MainChatKernelContextConfig, MainChatProviderAuthorization,
    MainChatTurnInput, SchedulerMainChatModelClient,
};
use crate::main_chat_turn_runtime::ProviderInvocationState;
use crate::state::AppState;
use crate::{SendMessageResult, ToolCallResult};
use openlife_core::agent::main_chat_agent_v1::AgentIngress;
use openlife_core::agent::metadata_safe::metadata_safe_text_digest;
use openlife_core::agent::ReasoningTrace;
use openlife_core::conversation::{BeginChatTurn, ConversationItemKind, TurnStatus};
use openlife_core::llm::ChatMessage;
use openlife_core::task_runtime::{
    BeginGeneralTaskRunInput, BeginItemAttemptInput, CanonicalTaskItemKind,
    CanonicalTaskItemStatus, CanonicalTaskStatus, CompleteGeneralTaskInput,
};
use serde_json::Value;
use std::sync::Arc;

pub(crate) struct CanonicalWorkInput {
    pub task_id: String,
    pub run_id: String,
    pub turn_id: String,
    pub conversation_id: String,
    pub messages: Vec<ChatMessage>,
    pub selected_skill_id: Option<String>,
    pub stream: bool,
}

#[derive(Debug)]
pub(crate) struct CanonicalWorkOutput {
    pub result: SendMessageResult,
    pub done_payload: Value,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CanonicalWorkControlResult {
    pub task_id: String,
    pub run_id: String,
    pub turn_id: String,
    pub status: CanonicalTaskStatus,
}

pub(crate) async fn cancel_canonical_work_task(
    task_id: &str,
    state: &Arc<AppState>,
) -> Result<CanonicalWorkControlResult, String> {
    validate_uuid("task_id", task_id)?;
    let task_store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let snapshot = task_store
        .lock()
        .await
        .load_task_snapshot(task_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_work_task_missing".to_string())?;
    if snapshot.task.task_kind != "work" {
        return Err("canonical_work_task_kind_invalid".into());
    }
    let run = snapshot
        .runs
        .iter()
        .rev()
        .find(|run| run.status == CanonicalTaskStatus::Running)
        .ok_or_else(|| "canonical_work_task_not_running".to_string())?;
    let cancelled = crate::canonical_chat_runtime::cancel_canonical_chat(
        &snapshot.task.conversation_id,
        &run.execution_session_id,
        state,
    )
    .await?;
    Ok(CanonicalWorkControlResult {
        task_id: task_id.to_string(),
        run_id: run.run_id.clone(),
        turn_id: run.execution_session_id.clone(),
        status: match cancelled.status {
            TurnStatus::Cancelled => CanonicalTaskStatus::Cancelled,
            _ => CanonicalTaskStatus::Running,
        },
    })
}

pub(crate) async fn retry_canonical_work_task(
    task_id: String,
    prior_run_id: String,
    new_run_id: String,
    new_turn_id: String,
    state: &Arc<AppState>,
) -> Result<CanonicalWorkOutput, String> {
    for (field, value) in [
        ("task_id", task_id.as_str()),
        ("prior_run_id", prior_run_id.as_str()),
        ("new_run_id", new_run_id.as_str()),
        ("new_turn_id", new_turn_id.as_str()),
    ] {
        validate_uuid(field, value)?;
    }
    let task_store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let snapshot = task_store
        .lock()
        .await
        .load_task_snapshot(&task_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_work_task_missing".to_string())?;
    if snapshot.task.task_kind != "work"
        || !matches!(
            snapshot.task.status,
            CanonicalTaskStatus::Failed
                | CanonicalTaskStatus::Blocked
                | CanonicalTaskStatus::Cancelled
                | CanonicalTaskStatus::Interrupted
        )
    {
        return Err("canonical_work_task_not_retryable".into());
    }
    let prior_run = snapshot
        .runs
        .iter()
        .find(|run| run.run_id == prior_run_id)
        .ok_or_else(|| "canonical_work_prior_run_missing".to_string())?;
    let conversation_store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| "conversation_store_unavailable".to_string())?;
    let original_turn = conversation_store
        .lock()
        .await
        .get_turn(&prior_run.execution_session_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_work_prior_turn_missing".to_string())?;
    let user_message = original_turn
        .items
        .iter()
        .find(|item| item.kind == ConversationItemKind::UserMessage)
        .ok_or_else(|| "canonical_work_prior_user_item_missing".to_string())?;
    let selected_skill_id = conversation_store
        .lock()
        .await
        .get_conversation(&snapshot.task.conversation_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_work_conversation_missing".to_string())?
        .selected_skill_id;
    let mut discard = |_: &str, _: Value| {};
    run_canonical_work(
        CanonicalWorkInput {
            task_id,
            run_id: new_run_id,
            turn_id: new_turn_id,
            conversation_id: snapshot.task.conversation_id,
            messages: vec![ChatMessage {
                role: "user".into(),
                content: user_message.content.clone(),
            }],
            selected_skill_id,
            stream: false,
        },
        state,
        &mut discard,
    )
    .await
}

pub(crate) async fn run_canonical_work(
    input: CanonicalWorkInput,
    state: &Arc<AppState>,
    emit: &mut (dyn FnMut(&str, Value) + Send),
) -> Result<CanonicalWorkOutput, String> {
    validate_input(&input)?;
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| error.to_string())?;
    let conversation_store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| "conversation_store_unavailable".to_string())?;
    let task_store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let current_user = input
        .messages
        .last()
        .filter(|message| message.role == "user")
        .ok_or_else(|| "canonical_work_current_user_missing".to_string())?;
    let selected_provider = crate::provider_registry::selected_provider_profile(state).await?;
    let provider_runtime = state.provider_runtime_snapshot().await;
    if !provider_runtime.coherent {
        return Err("provider_runtime_generation_incoherent".into());
    }
    let route = selected_provider.route;
    let provider = selected_provider.binding;
    let begun_turn = conversation_store
        .lock()
        .await
        .begin_chat_turn_with_proof(BeginChatTurn {
            turn_id: &input.turn_id,
            conversation_id: &input.conversation_id,
            user_message: &current_user.content,
            provider: &provider,
        })
        .map_err(|error| format!("begin canonical Work Turn failed: {error}"))?;
    if begun_turn.snapshot.turn.status == TurnStatus::Completed {
        return replay_completed(&input, state, route).await;
    }
    if begun_turn.snapshot.turn.status != TurnStatus::Running {
        return Err(format!(
            "canonical_work_turn_terminal:{}",
            begun_turn.snapshot.turn.status.as_str()
        ));
    }
    let history = conversation_store
        .lock()
        .await
        .list_items(&input.conversation_id, 200)
        .map_err(|error| format!("load Work conversation history failed: {error}"))?
        .into_iter()
        .filter_map(|item| match item.kind {
            ConversationItemKind::UserMessage => Some(ChatMessage {
                role: "user".into(),
                content: item.content,
            }),
            ConversationItemKind::AssistantMessage => Some(ChatMessage {
                role: "assistant".into(),
                content: item.content,
            }),
            ConversationItemKind::SystemNotice => None,
        })
        .collect::<Vec<_>>();
    let ingress = AgentIngress::default()
        .decide_with_conversation_user_item(
            &begun_turn.user_message_proof,
            &current_user.content,
            &history,
        )
        .map_err(|error| format!("canonical Work policy admission failed: {error}"))?;
    let (_, instruction_digest) = metadata_safe_text_digest(&current_user.content);
    let plan_digest = (ingress.policy_decision.route_kind
        == openlife_core::agent::main_chat_agent_v1::PolicyRouteKind::PlanDraft)
        .then_some(instruction_digest.as_str());
    task_store
        .lock()
        .await
        .begin_general_task_run(BeginGeneralTaskRunInput {
            task_id: &input.task_id,
            conversation_id: &input.conversation_id,
            run_id: &input.run_id,
            // Bind the canonical Run to the exact Conversation Turn. A Run is
            // an execution attempt, not a second task-session identity.
            execution_session_id: &input.turn_id,
            instruction_digest: &instruction_digest,
            plan_digest,
        })
        .map_err(|error| format!("begin canonical Work Task failed: {error}"))?;
    let provider_item_id = format!("item:provider:{}", input.run_id);
    let (_, request_digest) = metadata_safe_text_digest(&format!(
        "{}\0{}\0{}\0{}",
        input.task_id, input.run_id, provider.profile_id, instruction_digest
    ));
    task_store
        .lock()
        .await
        .append_general_item(
            &input.task_id,
            &input.run_id,
            &provider_item_id,
            CanonicalTaskItemKind::ProviderGeneration,
            "work_provider_generation",
            &request_digest,
        )
        .map_err(|error| format!("append Work provider Item failed: {error}"))?;
    let attempt_id = input.run_id.clone();
    task_store
        .lock()
        .await
        .begin_item_attempt(BeginItemAttemptInput {
            attempt_id: &attempt_id,
            task_id: &input.task_id,
            run_id: &input.run_id,
            item_id: &provider_item_id,
            executor_kind: "provider",
            provider_profile_id: Some(&provider.profile_id),
            provider_model_id: Some(&provider.model_id),
            request_digest: &request_digest,
        })
        .map_err(|error| format!("begin Work provider attempt failed: {error}"))?;
    let cancellation_registry = state
        .main_chat_runtime_state
        .lock()
        .await
        .cancellation_registry
        .clone();
    let cancellation = cancellation_registry
        .try_register(&input.turn_id)
        .map_err(|error| error.to_string())?;
    let authorization = MainChatProviderAuthorization::from_ingress_decision(&ingress)?;
    let privacy_engine = state.privacy_engine.lock().await.clone();
    let client = SchedulerMainChatModelClient::new(
        provider_runtime.scheduler,
        privacy_engine,
        provider_runtime.config.system.network_policy,
    )
    .with_configured_conversation_provider_grant();
    let kernel = MainChatKernel::new(client).with_context_config(MainChatKernelContextConfig {
        stream_provider_tokens: input.stream,
        ..MainChatKernelContextConfig::default()
    });
    let mut sink = CanonicalChatEventSink {
        buffered: Default::default(),
        conversation_id: &input.conversation_id,
        turn_id: &input.turn_id,
        emit,
        cancellation_registry,
    };
    (sink.emit)(
        "stream-message-start",
        serde_json::json!({
            "session_id": input.conversation_id,
            "operation_id": input.turn_id,
            "conversation_id": input.conversation_id,
            "turn_id": input.turn_id,
            "task_id": input.task_id,
            "run_id": input.run_id,
            "status": "running",
            "provider": provider.provider_id,
            "model": provider.model_id,
        }),
    );
    let kernel_result = {
        let future = kernel.run_canonical_work_provider_only(
            MainChatTurnInput {
                session_id: input.conversation_id.clone(),
                messages: history,
                provider_authorization: authorization,
                selected_skill_id: input.selected_skill_id.clone(),
                policy_decision: ingress.policy_decision.clone(),
                model_supplied_tool_arguments: None,
                runtime_fact_direct_answer: false,
            },
            &mut sink,
        );
        tokio::pin!(future);
        tokio::select! {
            result = &mut future => result,
            _ = cancellation.token.cancelled() => {
                terminalize_failure(state, &input, CanonicalTaskStatus::Cancelled,
                    CanonicalTaskItemStatus::Cancelled, "work_cancelled").await?;
                return Err("canonical_work_cancelled".into());
            }
        }
    };
    let invocation = provider_state(sink.events());
    if let Err(code) = verify_provider_binding(sink.events(), &provider) {
        terminalize_failure(
            state,
            &input,
            CanonicalTaskStatus::Failed,
            CanonicalTaskItemStatus::Failed,
            &code,
        )
        .await?;
        return Err(code);
    }
    if let Some((task_status, attempt_status, default_code)) =
        provider_non_success_terminal(invocation)
    {
        let code = kernel_result
            .blockers
            .first()
            .cloned()
            .unwrap_or_else(|| default_code.into());
        terminalize_failure(state, &input, task_status, attempt_status, &code).await?;
        return Err(code);
    }
    let reply = kernel_result
        .assistant_message
        .map(|message| message.content)
        .filter(|reply| !reply.trim().is_empty());
    let Some(reply) = reply else {
        let code = kernel_result
            .blockers
            .first()
            .cloned()
            .unwrap_or_else(|| "work_generation_failed".into());
        terminalize_failure(
            state,
            &input,
            CanonicalTaskStatus::Blocked,
            CanonicalTaskItemStatus::Blocked,
            &code,
        )
        .await?;
        return Err(code);
    };
    let (_, receipt_digest) = metadata_safe_text_digest(&format!(
        "{}\0{}\0{}",
        provider.profile_id, provider.model_id, reply
    ));
    task_store
        .lock()
        .await
        .terminalize_item_attempt(
            &attempt_id,
            CanonicalTaskItemStatus::Completed,
            Some(&receipt_digest),
        )
        .map_err(|error| format!("complete Work provider attempt failed: {error}"))?;
    let completed_turn = conversation_store
        .lock()
        .await
        .complete_work_turn(&input.turn_id, &reply)
        .map_err(|error| format!("complete canonical Work Turn failed: {error}"))?;
    let assistant_item = completed_turn
        .items
        .iter()
        .find(|item| item.kind == ConversationItemKind::AssistantMessage)
        .ok_or_else(|| "canonical_work_assistant_item_missing".to_string())?;
    let final_item_id = format!("item:final:{}", input.run_id);
    task_store
        .lock()
        .await
        .complete_general_task(CompleteGeneralTaskInput {
            task_id: &input.task_id,
            run_id: &input.run_id,
            final_item_id: &final_item_id,
            conversation_item_id: &assistant_item.id,
            result_digest: &assistant_item.content_digest,
            summary_code: "work_completed",
        })
        .map_err(|error| format!("complete canonical Work Task failed: {error}"))?;
    Ok(output(
        &input,
        reply,
        kernel_result.blockers,
        invocation,
        route,
    ))
}

fn provider_non_success_terminal(
    invocation: ProviderInvocationState,
) -> Option<(CanonicalTaskStatus, CanonicalTaskItemStatus, &'static str)> {
    match invocation {
        ProviderInvocationState::Failed => Some((
            CanonicalTaskStatus::Failed,
            CanonicalTaskItemStatus::Failed,
            "work_provider_failed",
        )),
        ProviderInvocationState::Started | ProviderInvocationState::RemoteUnknown => Some((
            CanonicalTaskStatus::EffectUnknown,
            CanonicalTaskItemStatus::EffectUnknown,
            "work_provider_effect_unknown",
        )),
        ProviderInvocationState::Invalid => Some((
            CanonicalTaskStatus::Failed,
            CanonicalTaskItemStatus::Failed,
            "work_provider_lifecycle_invalid",
        )),
        ProviderInvocationState::NotAttempted | ProviderInvocationState::Completed => None,
        ProviderInvocationState::LocallyAborted => Some((
            CanonicalTaskStatus::Interrupted,
            CanonicalTaskItemStatus::Interrupted,
            "work_provider_locally_aborted",
        )),
    }
}

async fn terminalize_failure(
    state: &Arc<AppState>,
    input: &CanonicalWorkInput,
    task_status: CanonicalTaskStatus,
    attempt_status: CanonicalTaskItemStatus,
    code: &str,
) -> Result<(), String> {
    if let Some(store) = state.canonical_task_runtime_store.as_ref() {
        let _ = store.lock().await.terminalize_item_attempt(
            &input.run_id,
            attempt_status,
            Some(&metadata_safe_text_digest(code).1),
        );
        store
            .lock()
            .await
            .terminalize_general_run(&input.task_id, &input.run_id, task_status)
            .map_err(|error| error.to_string())?;
    }
    if let Some(store) = state.conversation_store.as_ref() {
        match task_status {
            CanonicalTaskStatus::Cancelled => store.lock().await.cancel_chat_turn(&input.turn_id),
            _ => store.lock().await.fail_chat_turn(&input.turn_id, code),
        }
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

async fn replay_completed(
    input: &CanonicalWorkInput,
    state: &Arc<AppState>,
    route: openlife_core::agent::ModelRouteTrace,
) -> Result<CanonicalWorkOutput, String> {
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| "conversation_store_unavailable".to_string())?;
    let turn = store
        .lock()
        .await
        .get_turn(&input.turn_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_work_turn_missing".to_string())?;
    let assistant_item = turn
        .items
        .iter()
        .find(|item| item.kind == ConversationItemKind::AssistantMessage)
        .ok_or_else(|| "canonical_work_assistant_item_missing".to_string())?;
    let task_store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let snapshot = task_store
        .lock()
        .await
        .load_task_snapshot(&input.task_id)
        .map_err(|error| format!("load replayed canonical Work Task failed: {error}"))?
        .ok_or_else(|| "canonical_work_replay_task_missing".to_string())?;
    if snapshot.task.conversation_id != input.conversation_id
        || !snapshot
            .runs
            .iter()
            .any(|run| run.run_id == input.run_id && run.execution_session_id == input.turn_id)
    {
        return Err("canonical_work_replay_identity_conflict".into());
    }
    let final_item_id = format!("item:final:{}", input.run_id);
    task_store
        .lock()
        .await
        .complete_general_task(CompleteGeneralTaskInput {
            task_id: &input.task_id,
            run_id: &input.run_id,
            final_item_id: &final_item_id,
            conversation_item_id: &assistant_item.id,
            result_digest: &assistant_item.content_digest,
            summary_code: "work_completed",
        })
        .map_err(|error| format!("reconcile replayed canonical Work Task failed: {error}"))?;
    Ok(output(
        input,
        assistant_item.content.clone(),
        Vec::new(),
        ProviderInvocationState::Completed,
        route,
    ))
}

fn output(
    input: &CanonicalWorkInput,
    reply: String,
    blockers: Vec<String>,
    invocation: ProviderInvocationState,
    route: openlife_core::agent::ModelRouteTrace,
) -> CanonicalWorkOutput {
    let reasoning_trace = ReasoningTrace {
        generation_result: Some(serde_json::json!({
            "canonicalWork": true,
            "conversationId": input.conversation_id,
            "turnId": input.turn_id,
            "taskId": input.task_id,
            "runId": input.run_id,
            "modelRoute": route,
        })),
        ..ReasoningTrace::default()
    };
    let result = SendMessageResult {
        reply: reply.clone(),
        status: "completed".into(),
        blockers: blockers.clone(),
        reasoning_trace: reasoning_trace.clone(),
        tool_calls: Vec::<ToolCallResult>::new(),
        run_id: Some(input.run_id.clone()),
        agent_ingress: None,
        agent_state: None,
        execution_transcript: Vec::new(),
        legacy_fallback_used: false,
        legacy_runtime_invoked: false,
        provider_invocation_status: invocation,
        model_invoked: invocation.observed_adapter_start(),
        tool_invoked: false,
        life_model_influence: None,
        turn_terminal: None,
    };
    CanonicalWorkOutput {
        result,
        done_payload: serde_json::json!({
            "session_id": input.conversation_id,
            "operation_id": input.turn_id,
            "conversation_id": input.conversation_id,
            "turn_id": input.turn_id,
            "task_id": input.task_id,
            "task_session_id": input.task_id,
            "run_id": input.run_id,
            "reply": reply,
            "status": "completed",
            "blockers": blockers,
            "provider_invocation_status": invocation,
            "model_invoked": invocation.observed_adapter_start(),
            "tool_invoked": false,
            "reasoning_trace": reasoning_trace,
            "tool_calls": [],
            "runtime_owner": "CanonicalWorkRuntime",
        }),
    }
}

fn validate_input(input: &CanonicalWorkInput) -> Result<(), String> {
    for (field, value) in [
        ("task_id", input.task_id.as_str()),
        ("run_id", input.run_id.as_str()),
        ("turn_id", input.turn_id.as_str()),
        ("conversation_id", input.conversation_id.as_str()),
    ] {
        validate_uuid(field, value)?;
    }
    if input
        .messages
        .last()
        .is_none_or(|message| message.role != "user" || message.content.trim().is_empty())
    {
        return Err("invalid_work_user_turn".into());
    }
    Ok(())
}

fn validate_uuid(field: &str, value: &str) -> Result<(), String> {
    let parsed = uuid::Uuid::parse_str(value).map_err(|_| format!("invalid_{field}"))?;
    if parsed.get_version() != Some(uuid::Version::Random)
        || parsed.hyphenated().to_string() != value
    {
        return Err(format!("invalid_{field}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn canonical_state(reply: &'static str) -> Arc<AppState> {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_local_http_provider(
            &state, reply,
        )
        .await;
        state
    }

    fn input(conversation_id: &str) -> CanonicalWorkInput {
        CanonicalWorkInput {
            task_id: uuid::Uuid::new_v4().to_string(),
            run_id: uuid::Uuid::new_v4().to_string(),
            turn_id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "Summarize the current situation.".into(),
            }],
            selected_skill_id: None,
            stream: false,
        }
    }

    #[tokio::test]
    async fn work_owns_task_run_attempt_and_final_result_without_legacy_growth() {
        let state = canonical_state("canonical Work result").await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Work")
            .unwrap();
        let input = input(&conversation_id);
        let legacy_sessions_before = state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_sessions(None, 100, 0)
            .unwrap()
            .len();
        let legacy_runs_before = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_runs(100, 0)
            .unwrap()
            .len();

        let output = run_canonical_work(input, &state, &mut |_, _| {})
            .await
            .unwrap();
        assert_eq!(output.result.reply, "canonical Work result");
        let snapshots = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_task_snapshots(10)
            .unwrap();
        assert_eq!(snapshots.len(), 1);
        let snapshot = &snapshots[0];
        assert_eq!(snapshot.task.task_kind, "work");
        assert_eq!(snapshot.task.status, CanonicalTaskStatus::Completed);
        assert_eq!(snapshot.runs.len(), 1);
        assert_eq!(snapshot.runs[0].status, CanonicalTaskStatus::Completed);
        assert_eq!(snapshot.attempts.len(), 1);
        assert_eq!(
            snapshot.attempts[0].status,
            CanonicalTaskItemStatus::Completed
        );
        assert!(snapshot.final_result.is_some());
        assert_eq!(
            state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_sessions(None, 100, 0)
                .unwrap()
                .len(),
            legacy_sessions_before
        );
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_runs(100, 0)
                .unwrap()
                .len(),
            legacy_runs_before
        );
    }

    #[tokio::test]
    async fn exact_work_replay_reuses_final_result_without_a_second_task() {
        let state = canonical_state("one Work result").await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Replay")
            .unwrap();
        let input = input(&conversation_id);
        let replay_input = CanonicalWorkInput {
            task_id: input.task_id.clone(),
            run_id: input.run_id.clone(),
            turn_id: input.turn_id.clone(),
            conversation_id: input.conversation_id.clone(),
            messages: input.messages.clone(),
            selected_skill_id: None,
            stream: false,
        };
        run_canonical_work(input, &state, &mut |_, _| {})
            .await
            .unwrap();
        let replay = run_canonical_work(replay_input, &state, &mut |_, _| {})
            .await
            .unwrap();
        assert_eq!(replay.result.reply, "one Work result");
        let snapshots = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_task_snapshots(10)
            .unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].runs.len(), 1);
        assert_eq!(snapshots[0].attempts.len(), 1);
        assert!(snapshots[0].final_result.is_some());
    }

    #[tokio::test]
    async fn one_conversation_can_own_multiple_distinct_work_tasks() {
        let state = canonical_state("Work result").await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Multiple Work tasks")
            .unwrap();
        run_canonical_work(input(&conversation_id), &state, &mut |_, _| {})
            .await
            .unwrap();
        run_canonical_work(input(&conversation_id), &state, &mut |_, _| {})
            .await
            .unwrap();
        let snapshots = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_task_snapshots(10)
            .unwrap();
        assert_eq!(snapshots.len(), 2);
        assert!(snapshots
            .iter()
            .all(|snapshot| snapshot.task.conversation_id == conversation_id));
    }

    #[tokio::test]
    async fn planning_request_is_a_plan_item_inside_the_work_run() {
        let state = canonical_state("1. Clarify outcome\n2. Execute\n3. Verify").await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Plan as Item")
            .unwrap();
        let mut request = input(&conversation_id);
        request.messages[0].content = "请把这个目标拆解成一个执行计划".into();
        let task_id = request.task_id.clone();
        run_canonical_work(request, &state, &mut |_, _| {})
            .await
            .unwrap();
        let snapshot = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .load_task_snapshot(&task_id)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.task.task_kind, "work");
        assert_eq!(snapshot.runs.len(), 1);
        assert!(snapshot.items.iter().any(|item| {
            item.kind == CanonicalTaskItemKind::Plan
                && item.status == CanonicalTaskItemStatus::Completed
        }));
        assert!(snapshot.final_result.is_some());
    }

    #[tokio::test]
    async fn failed_work_retries_as_a_new_run_of_the_same_task() {
        let state = canonical_state("retry result").await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Retry")
            .unwrap();
        let task_id = uuid::Uuid::new_v4().to_string();
        let prior_run_id = uuid::Uuid::new_v4().to_string();
        let prior_turn_id = uuid::Uuid::new_v4().to_string();
        let provider = crate::provider_registry::selected_provider_profile(&state)
            .await
            .unwrap()
            .binding;
        let begun = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_chat_turn_with_proof(BeginChatTurn {
                turn_id: &prior_turn_id,
                conversation_id: &conversation_id,
                user_message: "Summarize the current situation.",
                provider: &provider,
            })
            .unwrap();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .fail_chat_turn(&prior_turn_id, "provider_failed")
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
                run_id: &prior_run_id,
                execution_session_id: &prior_turn_id,
                instruction_digest: begun.user_message_proof.content_digest(),
                plan_digest: None,
            })
            .unwrap();
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .terminalize_general_run(&task_id, &prior_run_id, CanonicalTaskStatus::Failed)
            .unwrap();
        let retried = retry_canonical_work_task(
            task_id.clone(),
            prior_run_id,
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            &state,
        )
        .await
        .unwrap();
        assert_eq!(retried.result.reply, "retry result");
        let snapshot = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .load_task_snapshot(&task_id)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.runs.len(), 2);
        assert_eq!(snapshot.runs[0].status, CanonicalTaskStatus::Failed);
        assert_eq!(snapshot.runs[1].status, CanonicalTaskStatus::Completed);
        assert_eq!(snapshot.task.status, CanonicalTaskStatus::Completed);
    }

    #[tokio::test]
    async fn active_work_cancel_terminalizes_turn_run_item_and_attempt() {
        use std::sync::atomic::Ordering;

        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let (request_observed, _client_closed, _release, _late) =
            crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_hanging_local_http_provider(&state).await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Cancel Work")
            .unwrap();
        let input = input(&conversation_id);
        let task_id = input.task_id.clone();
        let run_state = Arc::clone(&state);
        let run =
            tokio::spawn(
                async move { run_canonical_work(input, &run_state, &mut |_, _| {}).await },
            );
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while !request_observed.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let cancelled = cancel_canonical_work_task(&task_id, &state).await.unwrap();
        assert_eq!(cancelled.status, CanonicalTaskStatus::Cancelled);
        assert_eq!(run.await.unwrap().unwrap_err(), "canonical_work_cancelled");
        let snapshot = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .load_task_snapshot(&task_id)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.task.status, CanonicalTaskStatus::Cancelled);
        assert_eq!(snapshot.runs[0].status, CanonicalTaskStatus::Cancelled);
        assert_eq!(
            snapshot.attempts[0].status,
            CanonicalTaskItemStatus::Cancelled
        );
        assert!(snapshot.final_result.is_none());
    }

    #[tokio::test]
    async fn provider_failure_terminalizes_work_without_a_final_result() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let captured = crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_failing_local_http_provider(&state).await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Failed Work")
            .unwrap();
        let request = input(&conversation_id);
        let task_id = request.task_id.clone();
        let turn_id = request.turn_id.clone();

        let error = run_canonical_work(request, &state, &mut |_, _| {})
            .await
            .unwrap_err();
        assert!(!error.trim().is_empty());
        assert_eq!(captured.lock().unwrap().len(), 1);
        let snapshot = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .load_task_snapshot(&task_id)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.task.status, CanonicalTaskStatus::Failed);
        assert_eq!(snapshot.runs[0].status, CanonicalTaskStatus::Failed);
        assert_eq!(snapshot.attempts[0].status, CanonicalTaskItemStatus::Failed);
        assert!(snapshot.final_result.is_none());
        let turn = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_turn(&turn_id)
            .unwrap()
            .unwrap();
        assert_eq!(turn.turn.status, TurnStatus::Failed);
        assert!(turn
            .items
            .iter()
            .all(|item| item.kind != ConversationItemKind::AssistantMessage));
    }

    #[test]
    fn uncertain_provider_attempt_maps_to_effect_unknown_not_blocked() {
        for state in [
            ProviderInvocationState::Started,
            ProviderInvocationState::RemoteUnknown,
        ] {
            let terminal = provider_non_success_terminal(state).unwrap();
            assert_eq!(terminal.0, CanonicalTaskStatus::EffectUnknown);
            assert_eq!(terminal.1, CanonicalTaskItemStatus::EffectUnknown);
        }
        let failed = provider_non_success_terminal(ProviderInvocationState::Failed).unwrap();
        assert_eq!(failed.0, CanonicalTaskStatus::Failed);
        assert_eq!(failed.1, CanonicalTaskItemStatus::Failed);
        assert!(provider_non_success_terminal(ProviderInvocationState::Completed).is_none());
    }
}
