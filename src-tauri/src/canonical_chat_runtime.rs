//! Canonical ordinary Chat runtime.
//!
//! This path owns Conversation -> Turn -> Item. It deliberately has no Task,
//! retired Work lifecycle stores, Review proposals, or effect writers.

use crate::main_chat_kernel::{
    BufferedMainChatEventSink, MainChatEventSink, MainChatKernel, MainChatKernelContextConfig,
    MainChatKernelEvent, MainChatProviderAuthorization, MainChatTurnInput,
    SchedulerMainChatModelClient,
};
use crate::provider_invocation_state::ProviderInvocationState;
use crate::state::AppState;
use crate::{SendMessageResult, ToolCallResult};
use openlife_core::agent::main_chat_agent_v1::{AgentIngress, PolicyRouteKind};
use openlife_core::agent::ProductAgentTrace;
use openlife_core::conversation::{BeginChatTurn, ProviderBinding, TurnStatus};
use openlife_core::llm::ChatMessage;
use serde_json::Value;
use std::sync::Arc;

pub(crate) struct CanonicalChatInput {
    pub turn_id: String,
    pub conversation_id: String,
    pub messages: Vec<ChatMessage>,
    pub selected_skill_id: Option<String>,
    pub stream: bool,
}

#[derive(Debug)]
pub(crate) struct CanonicalChatOutput {
    pub result: SendMessageResult,
    pub done_payload: Value,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CancelCanonicalChatResult {
    pub conversation_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    pub active_turn_found: bool,
}

pub(crate) async fn cancel_canonical_chat(
    conversation_id: &str,
    turn_id: &str,
    state: &Arc<AppState>,
) -> Result<CancelCanonicalChatResult, String> {
    validate_uuid_field("conversation_id", conversation_id)?;
    validate_uuid_field("turn_id", turn_id)?;
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| "conversation_store_unavailable".to_string())?;
    let existing = store
        .lock()
        .await
        .get_turn(turn_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "conversation_turn_missing".to_string())?
        .turn;
    if existing.conversation_id != conversation_id {
        return Err("conversation_turn_identity_mismatch".into());
    }
    let cancellation_registry = state
        .main_chat_runtime_state
        .lock()
        .await
        .cancellation_registry
        .clone();
    let request = cancellation_registry.request_cancel(turn_id);
    let turn = if existing.status == TurnStatus::Running {
        store
            .lock()
            .await
            .cancel_chat_turn(turn_id)
            .map_err(|error| error.to_string())?
    } else {
        existing
    };
    if let Some(task_store) = state.canonical_task_runtime_store.as_ref() {
        let target = {
            let store = task_store.lock().await;
            store
                .resolve_general_run_by_execution_session(turn_id)
                .map_err(|error| error.to_string())?
        };
        if let Some((task_id, run_id)) = target {
            task_store
                .lock()
                .await
                .terminalize_general_run(
                    &task_id,
                    &run_id,
                    openlife_core::task_runtime::CanonicalTaskStatus::Cancelled,
                )
                .map_err(|error| format!("cancel canonical Work Task failed: {error}"))?;
        }
    }
    Ok(CancelCanonicalChatResult {
        conversation_id: conversation_id.to_string(),
        turn_id: turn_id.to_string(),
        status: turn.status,
        active_turn_found: request.outcome.active_turn_found,
    })
}

pub(crate) struct CanonicalChatEventSink<'a> {
    pub(crate) buffered: BufferedMainChatEventSink,
    pub(crate) conversation_id: &'a str,
    pub(crate) turn_id: &'a str,
    pub(crate) emit: &'a mut (dyn FnMut(&str, Value) + Send),
    pub(crate) cancellation_registry: crate::main_chat_cancellation::MainChatCancellationRegistry,
    pub(crate) work_provider_lifecycle: Option<CanonicalWorkProviderLifecycle>,
    pub(crate) work_provider_lifecycle_error: Option<String>,
}

pub(crate) struct CanonicalWorkProviderLifecycle {
    pub(crate) store: openlife_core::task_runtime::CanonicalTaskRuntimeStore,
    pub(crate) task_id: String,
    pub(crate) run_id: String,
    pub(crate) request_digest_seed: String,
    pub(crate) provider_profile_id: String,
    pub(crate) provider_model_id: String,
    invocation_ordinal: u64,
    active_attempt_id: Option<String>,
    next_invocation_is_plan: bool,
}

impl CanonicalWorkProviderLifecycle {
    pub(crate) fn new(
        store: openlife_core::task_runtime::CanonicalTaskRuntimeStore,
        task_id: String,
        run_id: String,
        request_digest_seed: String,
        provider_profile_id: String,
        provider_model_id: String,
    ) -> Self {
        Self {
            store,
            task_id,
            run_id,
            request_digest_seed,
            provider_profile_id,
            provider_model_id,
            invocation_ordinal: 0,
            active_attempt_id: None,
            next_invocation_is_plan: false,
        }
    }

    #[cfg(not(test))]
    pub(crate) fn prepare_plan_invocation(&mut self) -> Result<(), String> {
        if self.active_attempt_id.is_some() || self.next_invocation_is_plan {
            return Err("canonical_work_plan_invocation_already_prepared".into());
        }
        self.next_invocation_is_plan = true;
        Ok(())
    }

    #[cfg(not(test))]
    pub(crate) fn clear_unobserved_plan_invocation(&mut self) {
        if self.active_attempt_id.is_none() {
            self.next_invocation_is_plan = false;
        }
    }

    fn begin(&mut self, request_id: &str) -> Result<(), String> {
        if self.active_attempt_id.is_some() {
            return Err("canonical_work_provider_attempt_already_active".into());
        }
        self.invocation_ordinal = self
            .invocation_ordinal
            .checked_add(1)
            .ok_or_else(|| "canonical_work_provider_invocation_overflow".to_string())?;
        let usage = self
            .store
            .work_run_budget_usage(&self.run_id)
            .map_err(|error| error.to_string())?;
        let budget = self
            .store
            .work_run_budget_policy(&self.run_id)
            .map_err(|error| error.to_string())?;
        if self.next_invocation_is_plan {
            budget.admit_plan(usage)?;
        } else {
            budget.admit_provider(usage)?;
        }
        let item_id = format!("item:provider:{}:{}", self.run_id, self.invocation_ordinal);
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let request_digest =
            openlife_core::agent::metadata_safe::metadata_safe_text_digest(&format!(
                "{}\0{}\0{}",
                self.request_digest_seed, self.invocation_ordinal, request_id
            ))
            .1;
        self.store
            .append_general_item(
                &self.task_id,
                &self.run_id,
                &item_id,
                openlife_core::task_runtime::CanonicalTaskItemKind::ProviderGeneration,
                if self.next_invocation_is_plan {
                    "work_plan_generation"
                } else {
                    "work_provider_generation"
                },
                &request_digest,
            )
            .map_err(|error| error.to_string())?;
        self.store
            .begin_item_attempt(openlife_core::task_runtime::BeginItemAttemptInput {
                attempt_id: &attempt_id,
                task_id: &self.task_id,
                run_id: &self.run_id,
                item_id: &item_id,
                executor_kind: "provider",
                provider_profile_id: Some(&self.provider_profile_id),
                provider_model_id: Some(&self.provider_model_id),
                request_digest: &request_digest,
            })
            .map_err(|error| error.to_string())?;
        self.active_attempt_id = Some(attempt_id);
        self.next_invocation_is_plan = false;
        Ok(())
    }

    fn terminalize(
        &mut self,
        status: openlife_core::task_runtime::CanonicalTaskItemStatus,
        receipt_digest: &str,
    ) -> Result<(), String> {
        let attempt_id = self
            .active_attempt_id
            .take()
            .ok_or_else(|| "canonical_work_provider_attempt_missing".to_string())?;
        self.store
            .terminalize_item_attempt(&attempt_id, status, Some(receipt_digest))
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

impl MainChatEventSink for CanonicalChatEventSink<'_> {
    fn emit_provider_started(
        &mut self,
        request_id: String,
        provider: String,
        model: String,
        started_at: chrono::DateTime<chrono::Utc>,
        policy_evidence: openlife_core::llm::ProviderPolicyReceiptEvidence,
    ) -> Result<(), String> {
        if let Some(lifecycle) = self.work_provider_lifecycle.as_mut() {
            lifecycle.begin(&request_id)?;
        }
        self.cancellation_registry
            .admit_provider_start(
                self.turn_id,
                &request_id,
                &provider,
                &model,
                started_at,
                &policy_evidence,
            )
            .map_err(|error| error.to_string())?;
        self.emit(MainChatKernelEvent::ProviderStarted {
            request_id: request_id.clone(),
            provider,
            model,
            started_at,
            policy_evidence: policy_evidence.clone(),
        });
        self.emit(MainChatKernelEvent::ProviderPolicyEvidence {
            request_id,
            policy_evidence,
        });
        Ok(())
    }

    fn emit(&mut self, event: MainChatKernelEvent) {
        match &event {
            MainChatKernelEvent::ProviderCompleted {
                request_id,
                provider,
                model,
                finished_at,
            } => {
                if let Some(lifecycle) = self.work_provider_lifecycle.as_mut() {
                    let digest = openlife_core::agent::metadata_safe::metadata_safe_value_digest(
                        &serde_json::json!({
                            "requestId": request_id,
                            "provider": provider,
                            "model": model,
                            "status": "completed",
                        }),
                    )
                    .1;
                    if let Err(error) = lifecycle.terminalize(
                        openlife_core::task_runtime::CanonicalTaskItemStatus::Completed,
                        &digest,
                    ) {
                        self.work_provider_lifecycle_error = Some(error);
                    }
                }
                let _ = self.cancellation_registry.record_provider_completed(
                    self.turn_id,
                    request_id,
                    provider,
                    model,
                    *finished_at,
                );
            }
            MainChatKernelEvent::ProviderFailed {
                request_id,
                provider,
                model,
                finished_at,
                error_digest,
            } => {
                if let Some(lifecycle) = self.work_provider_lifecycle.as_mut() {
                    let digest = openlife_core::agent::metadata_safe::metadata_safe_value_digest(
                        &serde_json::json!({
                            "requestId": request_id,
                            "provider": provider,
                            "model": model,
                            "status": "failed",
                            "errorDigest": error_digest,
                        }),
                    )
                    .1;
                    if let Err(error) = lifecycle.terminalize(
                        openlife_core::task_runtime::CanonicalTaskItemStatus::Failed,
                        &digest,
                    ) {
                        self.work_provider_lifecycle_error = Some(error);
                    }
                }
                let _ = self.cancellation_registry.record_provider_failed(
                    self.turn_id,
                    request_id,
                    provider,
                    model,
                    *finished_at,
                    error_digest,
                );
            }
            MainChatKernelEvent::ProviderRemoteUnknown {
                request_id,
                provider,
                model,
                reason_digest,
                ..
            } => {
                if let Some(lifecycle) = self.work_provider_lifecycle.as_mut() {
                    let digest = openlife_core::agent::metadata_safe::metadata_safe_value_digest(
                        &serde_json::json!({
                            "requestId": request_id,
                            "provider": provider,
                            "model": model,
                            "status": "effect_unknown",
                            "reasonDigest": reason_digest,
                        }),
                    )
                    .1;
                    if let Err(error) = lifecycle.terminalize(
                        openlife_core::task_runtime::CanonicalTaskItemStatus::EffectUnknown,
                        &digest,
                    ) {
                        self.work_provider_lifecycle_error = Some(error);
                    }
                }
            }
            _ => {}
        }
        if let MainChatKernelEvent::ProviderToken {
            session_id,
            request_id,
            chunk,
        } = &event
        {
            (self.emit)(
                "stream-message-chunk",
                serde_json::json!({
                    "session_id": session_id,
                    "operation_id": self.turn_id,
                    "conversation_id": self.conversation_id,
                    "turn_id": self.turn_id,
                    "task_id": self.work_provider_lifecycle.as_ref().map(|owner| &owner.task_id),
                    "run_id": self.work_provider_lifecycle.as_ref().map(|owner| &owner.run_id),
                    "request_id": request_id,
                    "chunk": chunk,
                }),
            );
        }
        self.buffered.emit(event);
    }

    fn events(&self) -> &[MainChatKernelEvent] {
        self.buffered.events()
    }
}

pub(crate) async fn run_canonical_chat(
    input: CanonicalChatInput,
    state: &Arc<AppState>,
    emit: &mut (dyn FnMut(&str, Value) + Send),
) -> Result<CanonicalChatOutput, String> {
    validate_input(&input)?;
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| error.to_string())?;
    let conversation_store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| "conversation_store_unavailable".to_string())?;
    let current_user = input
        .messages
        .last()
        .filter(|message| message.role == "user")
        .ok_or_else(|| "canonical_chat_current_user_missing".to_string())?;

    let selected_provider = crate::provider_registry::selected_provider_profile(state).await?;
    let provider_runtime = state.provider_runtime_snapshot().await;
    if !provider_runtime.coherent {
        return Err("provider_runtime_generation_incoherent".into());
    }
    let route = selected_provider.route;
    let provider = selected_provider.binding;

    let begun = conversation_store
        .lock()
        .await
        .begin_chat_turn_with_proof(BeginChatTurn {
            turn_id: &input.turn_id,
            conversation_id: &input.conversation_id,
            user_message: &current_user.content,
            provider: &provider,
        })
        .map_err(|error| format!("begin canonical Chat Turn failed: {error}"))?;
    if begun.snapshot.turn.status == TurnStatus::Completed {
        let reply = begun
            .snapshot
            .items
            .iter()
            .find(|item| {
                item.kind == openlife_core::conversation::ConversationItemKind::AssistantMessage
            })
            .map(|item| item.content.clone())
            .ok_or_else(|| "completed_chat_turn_assistant_item_missing".to_string())?;
        return Ok(output_from_result(
            &input,
            reply,
            Vec::new(),
            ProviderInvocationState::Completed,
            route,
            None,
        ));
    }
    if begun.snapshot.turn.status != TurnStatus::Running {
        return Err(format!(
            "canonical_chat_turn_terminal:{}",
            begun.snapshot.turn.status.as_str()
        ));
    }
    let cancellation_registry = state
        .main_chat_runtime_state
        .lock()
        .await
        .cancellation_registry
        .clone();
    let cancellation = cancellation_registry
        .try_register(&input.turn_id)
        .map_err(|error| error.to_string())?;
    let cancellation_token = cancellation.token.clone();

    let history = conversation_store
        .lock()
        .await
        .list_model_context_items(&input.conversation_id, &input.turn_id, 200)
        .map_err(|error| format!("load canonical Chat history failed: {error}"))?
        .into_iter()
        .filter_map(|item| match item.kind {
            openlife_core::conversation::ConversationItemKind::UserMessage => Some(ChatMessage {
                role: "user".into(),
                content: item.content,
            }),
            openlife_core::conversation::ConversationItemKind::AssistantMessage => {
                Some(ChatMessage {
                    role: "assistant".into(),
                    content: item.content,
                })
            }
            openlife_core::conversation::ConversationItemKind::UserSteering
            | openlife_core::conversation::ConversationItemKind::SystemNotice => None,
        })
        .collect::<Vec<_>>();
    let ingress = AgentIngress::default()
        .decide_with_conversation_user_item(
            &begun.user_message_proof,
            &current_user.content,
            &history,
        )
        .map_err(|error| format!("canonical Chat policy admission failed: {error}"))?;
    if ingress.policy_route != PolicyRouteKind::DirectAnswer {
        conversation_store
            .lock()
            .await
            .fail_chat_turn(&input.turn_id, "chat_requires_work_mode")
            .map_err(|error| format!("terminalize non-Chat request failed: {error}"))?;
        return Err("chat_requires_work_mode".into());
    }
    let authorization = MainChatProviderAuthorization::from_ingress_decision(&ingress)?;
    let privacy_engine = state.privacy_engine.lock().await.clone();
    let client = SchedulerMainChatModelClient::new(
        provider_runtime.scheduler,
        privacy_engine,
        provider_runtime.config.system.network_policy,
    );
    let personal_context = crate::personal_intelligence_ports::load_personal_intelligence_context(
        state,
        crate::personal_intelligence_ports::PersonalIntelligenceContextRequest {
            conversation_id: &input.conversation_id,
            user_text: &current_user.content,
        },
    )
    .await;
    debug_assert!(!personal_context.life_model_contract_version.is_empty());
    let kernel = MainChatKernel::new(client).with_context_config(MainChatKernelContextConfig {
        extra_candidates: personal_context.memory.candidates,
        life_model_context: Some(personal_context.life_model),
        stream_provider_tokens: input.stream,
        ..MainChatKernelContextConfig::default()
    });
    let mut sink = CanonicalChatEventSink {
        buffered: BufferedMainChatEventSink::default(),
        conversation_id: &input.conversation_id,
        turn_id: &input.turn_id,
        emit,
        cancellation_registry,
        work_provider_lifecycle: None,
        work_provider_lifecycle_error: None,
    };
    (sink.emit)(
        "stream-message-start",
        serde_json::json!({
            "session_id": input.conversation_id,
            "operation_id": input.turn_id,
            "conversation_id": input.conversation_id,
            "turn_id": input.turn_id,
            "status": "running",
            "provider": provider.provider_id,
            "model": provider.model_id,
        }),
    );
    let kernel_result = {
        let kernel_future = kernel.run_canonical_chat(
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
        tokio::pin!(kernel_future);
        tokio::select! {
            result = &mut kernel_future => result,
            _ = cancellation_token.cancelled() => {
                conversation_store
                    .lock()
                    .await
                    .cancel_chat_turn(&input.turn_id)
                    .map_err(|error| format!("cancel canonical Chat Turn failed: {error}"))?;
                return Err("canonical_chat_turn_cancelled".into());
            }
        }
    };
    let invocation = provider_state(sink.events());
    if let Err(code) = verify_provider_binding(sink.events(), &provider) {
        conversation_store
            .lock()
            .await
            .fail_chat_turn(&input.turn_id, &code)
            .map_err(|error| format!("terminalize provider-binding failure failed: {error}"))?;
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
            .unwrap_or_else(|| "chat_generation_failed".into());
        conversation_store
            .lock()
            .await
            .fail_chat_turn(&input.turn_id, &code)
            .map_err(|error| format!("terminalize failed Chat Turn failed: {error}"))?;
        return Err(code);
    };
    conversation_store
        .lock()
        .await
        .complete_chat_turn(&input.turn_id, &reply)
        .map_err(|error| format!("complete canonical Chat Turn failed: {error}"))?;
    Ok(output_from_result(
        &input,
        reply,
        kernel_result.blockers,
        invocation,
        route,
        kernel_result
            .context_metadata
            .as_ref()
            .and_then(|metadata| metadata.life_model_context.as_ref())
            .map(|metadata| metadata.product_receipt()),
    ))
}

pub(crate) fn verify_provider_binding(
    events: &[MainChatKernelEvent],
    binding: &ProviderBinding,
) -> Result<(), String> {
    let observed = events.iter().find_map(|event| match event {
        MainChatKernelEvent::ProviderStarted {
            provider, model, ..
        } => Some((provider, model)),
        _ => None,
    });
    let Some((provider, model)) = observed else {
        return Err("canonical_chat_provider_start_missing".into());
    };
    if provider != &binding.provider_id || model != &binding.model_id {
        return Err("canonical_chat_provider_binding_mismatch".into());
    }
    Ok(())
}

fn validate_input(input: &CanonicalChatInput) -> Result<(), String> {
    for (field, value) in [
        ("turn_id", input.turn_id.as_str()),
        ("conversation_id", input.conversation_id.as_str()),
    ] {
        validate_uuid_field(field, value)?;
    }
    if input
        .messages
        .last()
        .is_none_or(|message| message.role != "user" || message.content.trim().is_empty())
    {
        return Err("invalid_chat_user_turn".into());
    }
    Ok(())
}

fn validate_uuid_field(field: &str, value: &str) -> Result<(), String> {
    let parsed = uuid::Uuid::parse_str(value).map_err(|_| format!("invalid_{field}"))?;
    if parsed.get_version() != Some(uuid::Version::Random)
        || parsed.hyphenated().to_string() != value
    {
        return Err(format!("invalid_{field}"));
    }
    Ok(())
}

pub(crate) fn provider_state(events: &[MainChatKernelEvent]) -> ProviderInvocationState {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            MainChatKernelEvent::ProviderCompleted { .. } => {
                Some(ProviderInvocationState::Completed)
            }
            MainChatKernelEvent::ProviderFailed { .. } => Some(ProviderInvocationState::Failed),
            MainChatKernelEvent::ProviderRemoteUnknown { .. } => {
                Some(ProviderInvocationState::RemoteUnknown)
            }
            MainChatKernelEvent::ProviderStarted { .. } => Some(ProviderInvocationState::Started),
            _ => None,
        })
        .unwrap_or_default()
}

fn output_from_result(
    input: &CanonicalChatInput,
    reply: String,
    blockers: Vec<String>,
    invocation: ProviderInvocationState,
    route: openlife_core::agent::ModelRouteTrace,
    life_model_influence: Option<crate::main_chat_kernel::MainChatLifeModelProductReceipt>,
) -> CanonicalChatOutput {
    let reasoning_trace = ProductAgentTrace {
        generation_result: Some(serde_json::json!({
            "canonicalConversation": true,
            "conversationId": input.conversation_id,
            "turnId": input.turn_id,
            "modelRoute": route,
        })),
    };
    let result = SendMessageResult {
        reply: reply.clone(),
        status: "completed".into(),
        blockers: blockers.clone(),
        reasoning_trace: reasoning_trace.clone(),
        tool_calls: Vec::<ToolCallResult>::new(),
        run_id: None,
        agent_ingress: None,
        provider_invocation_status: invocation,
        model_invoked: invocation.observed_adapter_start(),
        tool_invoked: false,
        life_model_influence: life_model_influence.clone(),
    };
    let done_payload = serde_json::json!({
        "session_id": input.conversation_id,
        "operation_id": input.turn_id,
        "conversation_id": input.conversation_id,
        "turn_id": input.turn_id,
        "reply": reply,
        "status": "completed",
        "blockers": blockers,
        "provider_invocation_status": invocation,
        "model_invoked": invocation.observed_adapter_start(),
        "tool_invoked": false,
        "reasoning_trace": reasoning_trace,
        "tool_calls": [],
        "life_model_influence": life_model_influence,
        "runtime_owner": "CanonicalChatRuntime",
    });
    CanonicalChatOutput {
        result,
        done_payload,
    }
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

    #[tokio::test]
    async fn chat_commits_exact_conversation_turn_and_items() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_captured_streaming_local_http_provider(
            &state,
            vec![
                ("canonical ", std::time::Duration::ZERO),
                ("reply", std::time::Duration::ZERO),
            ],
        )
        .await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Canonical Chat")
            .unwrap();
        let mut events = Vec::new();
        let started = std::time::Instant::now();
        let mut first_chunk_elapsed = None;
        let output = run_canonical_chat(
            CanonicalChatInput {
                turn_id: turn_id.clone(),
                conversation_id: conversation_id.clone(),
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: "hello".into(),
                }],
                selected_skill_id: None,
                stream: true,
            },
            &state,
            &mut |kind, payload| {
                if kind == "stream-message-chunk" && first_chunk_elapsed.is_none() {
                    first_chunk_elapsed = Some(started.elapsed());
                }
                events.push((kind.to_string(), payload));
            },
        )
        .await
        .unwrap();
        let terminal_elapsed = started.elapsed();
        assert!(
            first_chunk_elapsed
                .is_some_and(|elapsed| { elapsed < std::time::Duration::from_secs(3) }),
            "controlled canonical Chat first visible chunk exceeded 3s: {first_chunk_elapsed:?}"
        );
        assert!(
            terminal_elapsed < std::time::Duration::from_secs(3),
            "controlled canonical Chat terminalization exceeded 3s: {terminal_elapsed:?}"
        );
        assert_eq!(output.result.reply, "canonical reply");
        let snapshot = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_turn(&turn_id)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.turn.status, TurnStatus::Completed);
        assert_eq!(snapshot.items.len(), 2);
        assert!(events
            .iter()
            .any(|(kind, _)| kind == "stream-message-start"));
        assert!(events
            .iter()
            .any(|(kind, _)| kind == "stream-message-chunk"));
    }

    #[tokio::test]
    async fn chinese_and_english_chat_share_the_same_conversation_turn_contract() {
        for (title, prompt, reply) in [
            (
                "English Chat",
                "Explain the current status.",
                "English reply",
            ),
            ("中文对话", "请解释当前状态。", "中文回答"),
        ] {
            let state = canonical_state(reply).await;
            let conversation_id = uuid::Uuid::new_v4().to_string();
            let turn_id = uuid::Uuid::new_v4().to_string();
            state
                .conversation_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .create_conversation(&conversation_id, title)
                .unwrap();

            let output = run_canonical_chat(
                CanonicalChatInput {
                    turn_id: turn_id.clone(),
                    conversation_id: conversation_id.clone(),
                    messages: vec![ChatMessage {
                        role: "user".into(),
                        content: prompt.into(),
                    }],
                    selected_skill_id: None,
                    stream: false,
                },
                &state,
                &mut |_, _| {},
            )
            .await
            .unwrap();

            assert_eq!(output.result.reply, reply);
            let snapshot = state
                .conversation_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_turn(&turn_id)
                .unwrap()
                .unwrap();
            assert_eq!(snapshot.turn.status, TurnStatus::Completed);
            assert_eq!(snapshot.items.len(), 2);
            assert_eq!(snapshot.items[0].content, prompt);
            assert_eq!(snapshot.items[1].content, reply);
            assert!(state
                .canonical_task_runtime_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_task_snapshots(10)
                .unwrap()
                .is_empty());
        }
    }

    #[tokio::test]
    async fn completed_turn_replays_without_a_second_provider_request() {
        let state = canonical_state("one reply").await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Replay")
            .unwrap();
        let input = || CanonicalChatInput {
            turn_id: turn_id.clone(),
            conversation_id: conversation_id.clone(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "same".into(),
            }],
            selected_skill_id: None,
            stream: false,
        };
        let mut first_events = Vec::new();
        run_canonical_chat(input(), &state, &mut |kind, _| {
            first_events.push(kind.to_string())
        })
        .await
        .unwrap();
        let mut replay_events = Vec::new();
        let replay = run_canonical_chat(input(), &state, &mut |kind, _| {
            replay_events.push(kind.to_string())
        })
        .await
        .unwrap();
        assert_eq!(replay.result.reply, "one reply");
        assert!(replay_events.is_empty());
    }

    #[tokio::test]
    async fn unavailable_provider_does_not_create_a_partial_turn() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Unavailable provider")
            .unwrap();

        let error = run_canonical_chat(
            CanonicalChatInput {
                turn_id: turn_id.clone(),
                conversation_id: conversation_id.clone(),
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: "hello".into(),
                }],
                selected_skill_id: None,
                stream: false,
            },
            &state,
            &mut |_, _| {},
        )
        .await
        .unwrap_err();

        assert_eq!(error, "configured_provider_unavailable");
        let store = state.conversation_store.as_ref().unwrap().lock().await;
        assert!(store.get_turn(&turn_id).unwrap().is_none());
        assert!(store.list_items(&conversation_id, 10).unwrap().is_empty());
    }

    #[tokio::test]
    async fn cancellation_is_terminal_and_rejects_a_late_provider_reply() {
        use std::sync::atomic::Ordering;

        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let (request_observed, _client_closed, release_late_response, late_response_attempted) =
            crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_hanging_local_http_provider(&state).await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Cancellation")
            .unwrap();

        let run_state = Arc::clone(&state);
        let run_conversation_id = conversation_id.clone();
        let run_turn_id = turn_id.clone();
        let run = tokio::spawn(async move {
            run_canonical_chat(
                CanonicalChatInput {
                    turn_id: run_turn_id,
                    conversation_id: run_conversation_id,
                    messages: vec![ChatMessage {
                        role: "user".into(),
                        content: "wait for me".into(),
                    }],
                    selected_skill_id: None,
                    stream: false,
                },
                &run_state,
                &mut |_, _| {},
            )
            .await
        });

        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while !request_observed.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("provider request observed");
        let cancelled = cancel_canonical_chat(&conversation_id, &turn_id, &state)
            .await
            .unwrap();
        assert_eq!(cancelled.status, TurnStatus::Cancelled);
        assert!(cancelled.active_turn_found);
        assert_eq!(
            run.await.unwrap().unwrap_err(),
            "canonical_chat_turn_cancelled"
        );

        release_late_response.store(true, Ordering::SeqCst);
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while !late_response_attempted.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("late provider response attempted");
        let snapshot = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_turn(&turn_id)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.turn.status, TurnStatus::Cancelled);
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(
            snapshot.items[0].kind,
            openlife_core::conversation::ConversationItemKind::UserMessage
        );
    }
}
