use async_trait::async_trait;
use openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentStateSnapshot;
use openlife_core::agent::main_chat_agent_v1::{
    AgentIngressDecision, AgentTaskSessionStatus, CompiledContext, ContextCompiler,
    ContextCompilerInput, ContextSourceCandidate, ContextSourceKind, ExecutionTranscriptEntry,
    ExecutionTranscriptEntryKind, MainChatAgentStrategy, MainChatPrivacyRiskSummary,
};
use openlife_core::agent::{
    AgentRun, AgentRunError, ContextSummary, ModelRouteTrace, ReasoningTrace, RedactionLevel,
};
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn;
use crate::main_chat_context_loader::{
    load_configured_knowledge_context_candidates,
    load_current_workspace_knowledge_context_candidates, sanitize_main_chat_selected_skill_id,
};
use crate::main_chat_event_stream::{
    materialize_optional_main_chat_agent_events, MainChatAgentDurableEvent,
};
use crate::main_chat_generation_support::{
    finalize_chat_agent_run, main_chat_provider_endpoint_kind, persist_chat_message_if_needed,
    persist_vector_memory_for_message, preview_text,
};
use crate::main_chat_runtime_support::{
    append_main_chat_agent_transcript, append_main_chat_direct_answer_contract_transcript,
    complete_main_chat_agent_turn_session, MainChatAgentTurn,
};
use crate::{AppState, SendMessageResult, ToolCallResult};

const KERNEL_CONTEXT_TOKEN_BUDGET: u32 = 120;
const MAX_ROUTE_LABEL_CHARS: usize = 96;
const MAX_REASON_CHARS: usize = 180;
const MAX_CONTEXT_CONTENT_CHARS: usize = 700;
const MAX_SYSTEM_PROMPT_CHARS: usize = 4_000;
const MAX_ASSISTANT_PREVIEW_CHARS: usize = 180;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatTurnInput {
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub selected_skill_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatTurnResult {
    pub assistant_message: Option<ChatMessage>,
    pub blockers: Vec<String>,
    pub proposals: Vec<String>,
    pub tool_calls: Vec<MainChatKernelToolCall>,
    pub route_metadata: Option<MainChatRouteMetadata>,
    pub context_metadata: Option<MainChatKernelContextMetadata>,
    pub direct_writes_executed: bool,
    pub legacy_fallback_used: bool,
}

impl MainChatTurnResult {
    fn blocked(code: impl Into<String>) -> Self {
        Self {
            assistant_message: None,
            blockers: vec![code.into()],
            proposals: Vec::new(),
            tool_calls: Vec::new(),
            route_metadata: None,
            context_metadata: None,
            direct_writes_executed: false,
            legacy_fallback_used: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatKernelToolCall {
    pub name: String,
    pub action_type: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatRouteMetadata {
    pub provider: String,
    pub model: String,
    pub route_type: String,
    pub prefer_local: bool,
    pub local_model: String,
    pub reason: String,
    pub privacy_level: RedactionLevel,
    pub tools_enabled: bool,
    pub live_eval_required: bool,
    pub final_acceptance_gate_required: bool,
    pub readiness_gate_required: bool,
    pub scripted_response_configured: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatKernelContextMetadata {
    pub context_snapshot_ref: String,
    pub selected_source_ids: Vec<String>,
    pub selected_source_count: usize,
    pub selected_skill_id: Option<String>,
    pub selected_skill_instruction_loaded: bool,
    pub raw_life_model_yaml_included: bool,
    pub raw_topk_memory_trusted: bool,
    pub workspace_policy_override_blocked: bool,
    pub system_prompt_chars: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MainChatKernelEvent {
    TurnStarted {
        session_id: String,
        selected_skill_id: Option<String>,
    },
    ContextLoaded {
        context_snapshot_ref: String,
        selected_source_count: usize,
        selected_skill_instruction_loaded: bool,
    },
    RouteSelected {
        route_metadata: MainChatRouteMetadata,
    },
    FinalAnswer {
        content_preview: String,
        content_chars: usize,
    },
    Blocker {
        code: String,
    },
}

pub trait MainChatEventSink {
    fn emit(&mut self, event: MainChatKernelEvent);

    fn events(&self) -> &[MainChatKernelEvent] {
        &[]
    }
}

#[derive(Debug, Default, Clone)]
pub struct BufferedMainChatEventSink {
    events: Vec<MainChatKernelEvent>,
}

impl BufferedMainChatEventSink {
    pub fn events(&self) -> &[MainChatKernelEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<MainChatKernelEvent> {
        self.events
    }
}

impl MainChatEventSink for BufferedMainChatEventSink {
    fn emit(&mut self, event: MainChatKernelEvent) {
        self.events.push(event);
    }

    fn events(&self) -> &[MainChatKernelEvent] {
        &self.events
    }
}

pub struct StreamingMainChatEventSink<'a> {
    emit_stream_event: &'a mut (dyn FnMut(&str, serde_json::Value) + Send),
    events: Vec<MainChatKernelEvent>,
}

impl<'a> StreamingMainChatEventSink<'a> {
    pub fn new<F>(emit_stream_event: &'a mut F) -> Self
    where
        F: FnMut(&str, serde_json::Value) + Send + 'a,
    {
        Self {
            emit_stream_event,
            events: Vec::new(),
        }
    }

    pub fn events(&self) -> &[MainChatKernelEvent] {
        &self.events
    }
}

impl MainChatEventSink for StreamingMainChatEventSink<'_> {
    fn emit(&mut self, event: MainChatKernelEvent) {
        let payload = serde_json::to_value(&event).unwrap_or_else(|_| {
            serde_json::json!({
                "type": "kernel_event_serialization_failed",
            })
        });
        (self.emit_stream_event)("main-chat-kernel-event", payload);
        self.events.push(event);
    }

    fn events(&self) -> &[MainChatKernelEvent] {
        &self.events
    }
}

pub(crate) struct MainChatKernelCommandSurfaceResult {
    pub(crate) reply: String,
    pub(crate) reasoning_trace: ReasoningTrace,
    pub(crate) tool_calls: Vec<ToolCallResult>,
    pub(crate) run_id: Option<String>,
    pub(crate) agent_ingress: Option<AgentIngressDecision>,
    pub(crate) agent_state: Option<MainChatAgentStateSnapshot>,
    pub(crate) execution_transcript: Vec<ExecutionTranscriptEntry>,
    pub(crate) legacy_fallback_used: bool,
    pub(crate) durable_events: Vec<MainChatAgentDurableEvent>,
    pub(crate) kernel_events: Vec<MainChatKernelEvent>,
}

impl MainChatKernelCommandSurfaceResult {
    pub(crate) fn into_send_message_result(self) -> SendMessageResult {
        SendMessageResult {
            reply: self.reply,
            reasoning_trace: self.reasoning_trace,
            tool_calls: self.tool_calls,
            run_id: self.run_id,
            agent_ingress: self.agent_ingress,
            agent_state: self.agent_state,
            execution_transcript: self.execution_transcript,
            legacy_fallback_used: self.legacy_fallback_used,
        }
    }
}

pub(crate) async fn run_main_chat_kernel_direct_answer_with_state<S>(
    session_id: &str,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
    main_chat_agent_turn: &MainChatAgentTurn,
    event_sink: &mut S,
    event_sink_label: &'static str,
) -> Result<MainChatKernelCommandSurfaceResult, String>
where
    S: MainChatEventSink + ?Sized,
{
    if main_chat_agent_turn.decision.selected_strategy != MainChatAgentStrategy::DirectAnswer {
        return Err(format!(
            "MainChatKernel direct-answer adapter received non-direct strategy {}",
            main_chat_agent_turn.decision.selected_strategy.as_str()
        ));
    }

    let requested_task_session_id = main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref()
        .ok_or_else(|| "Main Chat kernel task session missing".to_string())?;
    let task_session_id = resolve_kernel_task_session_id(
        state,
        requested_task_session_id,
        session_id,
        main_chat_agent_turn.decision.selected_strategy,
    )
    .await;
    let mut effective_main_chat_agent_turn = main_chat_agent_turn.clone();
    if effective_main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref()
        != Some(task_session_id.as_str())
    {
        effective_main_chat_agent_turn
            .decision
            .agent_task_session_id = Some(task_session_id.clone());
    }
    let main_chat_agent_turn = &effective_main_chat_agent_turn;
    let user_msg = messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .cloned();
    let user_text = user_msg
        .as_ref()
        .map(|message| message.content.clone())
        .unwrap_or_default();
    let sanitized_selected_skill_id =
        sanitize_main_chat_selected_skill_id(selected_skill_id.as_deref());
    let mut execution_transcript = main_chat_agent_turn.transcript_entries.clone();
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(&task_session_id),
            ExecutionTranscriptEntryKind::Plan,
            "Main Chat Agent strategy execution started.",
            serde_json::json!({
                "selectedStrategy": MainChatAgentStrategy::DirectAnswer.as_str(),
                "policyReasonCode": main_chat_agent_turn.decision.privacy_risk.policy_reason_code,
                "silentWritesAllowed": false,
                "kernelBackedDirectAnswer": true,
                "kernelEventSink": event_sink_label,
            }),
        )
        .await,
    );
    execution_transcript.extend(
        append_main_chat_direct_answer_contract_transcript(
            state,
            main_chat_agent_turn,
            &user_text,
            sanitized_selected_skill_id.as_deref(),
        )
        .await,
    );

    let scheduler = state.scheduler.lock().await.clone();
    let direct_reply = if user_text.trim().is_empty() {
        None
    } else {
        let router = state.intent_router.lock().await;
        router.classify(&user_text).direct_response()
    };
    let life_model = LifeModel::default();
    let extra_candidates =
        command_surface_kernel_context_candidates(state, sanitized_selected_skill_id.as_deref())
            .await;
    let kernel = MainChatKernel::new(CommandSurfaceDirectAnswerModelClient::new(
        scheduler.clone(),
        life_model.clone(),
        direct_reply.clone(),
    ))
    .with_context_config(MainChatKernelContextConfig {
        load_workspace_knowledge: true,
        token_budget: 160,
        extra_candidates,
    });

    let kernel_result = kernel
        .run_turn(
            MainChatTurnInput {
                session_id: session_id.to_string(),
                messages,
                selected_skill_id: sanitized_selected_skill_id.clone(),
            },
            event_sink,
        )
        .await;

    let kernel_events = event_sink.events().to_vec();

    if !kernel_result.blockers.is_empty() {
        return build_blocked_kernel_command_surface_result(
            session_id,
            &task_session_id,
            state,
            main_chat_agent_turn,
            execution_transcript,
            kernel_result,
            event_sink_label,
            kernel_events,
        )
        .await;
    }

    if let Some(user) = user_msg.as_ref() {
        let inserted = persist_chat_message_if_needed(session_id, user, state).await?;
        if inserted {
            persist_vector_memory_for_message(session_id, user, state).await;
        }
    }

    build_successful_kernel_command_surface_result(
        session_id,
        &user_text,
        state,
        main_chat_agent_turn,
        execution_transcript,
        kernel_result,
        scheduler,
        life_model,
        direct_reply.is_some(),
        event_sink_label,
        kernel_events,
    )
    .await
}

async fn resolve_kernel_task_session_id(
    state: &Arc<AppState>,
    requested_task_session_id: &str,
    chat_session_id: &str,
    selected_strategy: MainChatAgentStrategy,
) -> String {
    let Some(ref store_arc) = state.main_chat_agent_session_store else {
        return requested_task_session_id.to_string();
    };
    let store = store_arc.lock().await;
    if matches!(store.load_session(requested_task_session_id), Ok(Some(_))) {
        return requested_task_session_id.to_string();
    }
    match store.list_sessions(None, 50, 0) {
        Ok(sessions) => sessions
            .into_iter()
            .find(|session| {
                session.chat_session_id == chat_session_id
                    && session.selected_strategy == selected_strategy
                    && matches!(
                        session.status,
                        AgentTaskSessionStatus::Running
                            | AgentTaskSessionStatus::WaitingPermission
                            | AgentTaskSessionStatus::Blocked
                    )
            })
            .map(|session| session.id)
            .unwrap_or_else(|| requested_task_session_id.to_string()),
        Err(err) => {
            log::warn!(
                "[MainChatKernel] resolve persisted task session failed: {}",
                err
            );
            requested_task_session_id.to_string()
        }
    }
}

#[derive(Debug, Clone)]
pub struct MainChatKernelContextConfig {
    pub load_workspace_knowledge: bool,
    pub token_budget: u32,
    pub extra_candidates: Vec<ContextSourceCandidate>,
}

impl Default for MainChatKernelContextConfig {
    fn default() -> Self {
        Self {
            load_workspace_knowledge: true,
            token_budget: KERNEL_CONTEXT_TOKEN_BUDGET,
            extra_candidates: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MainChatModelRequest {
    pub messages: Vec<ChatMessage>,
    pub system_prompt: String,
    pub context_snapshot_ref: String,
    pub selected_skill_id: Option<String>,
}

#[async_trait]
pub trait MainChatModelClient: Send + Sync {
    async fn generate_direct_answer(&self, request: MainChatModelRequest)
        -> Result<String, String>;

    fn route_metadata(&self) -> MainChatRouteMetadata;
}

#[derive(Clone)]
pub struct SchedulerMainChatModelClient {
    scheduler: InferenceScheduler,
    life_model: LifeModel,
}

impl SchedulerMainChatModelClient {
    pub fn new(scheduler: InferenceScheduler, life_model: LifeModel) -> Self {
        Self {
            scheduler,
            life_model,
        }
    }
}

#[async_trait]
impl MainChatModelClient for SchedulerMainChatModelClient {
    async fn generate_direct_answer(
        &self,
        request: MainChatModelRequest,
    ) -> Result<String, String> {
        let mut messages = Vec::with_capacity(request.messages.len() + 1);
        messages.push(ChatMessage {
            role: "system".into(),
            content: request.system_prompt,
        });
        messages.extend(request.messages);

        self.scheduler
            .generate(messages, &self.life_model, None)
            .await
            .map_err(|err| err.to_string())
    }

    fn route_metadata(&self) -> MainChatRouteMetadata {
        route_metadata_from_scheduler(&self.scheduler)
    }
}

#[derive(Clone)]
struct CommandSurfaceDirectAnswerModelClient {
    scheduler: InferenceScheduler,
    life_model: LifeModel,
    direct_reply: Option<String>,
}

impl CommandSurfaceDirectAnswerModelClient {
    fn new(
        scheduler: InferenceScheduler,
        life_model: LifeModel,
        direct_reply: Option<String>,
    ) -> Self {
        Self {
            scheduler,
            life_model,
            direct_reply,
        }
    }
}

#[async_trait]
impl MainChatModelClient for CommandSurfaceDirectAnswerModelClient {
    async fn generate_direct_answer(
        &self,
        request: MainChatModelRequest,
    ) -> Result<String, String> {
        if let Some(reply) = self.direct_reply.as_ref() {
            return Ok(reply.clone());
        }

        SchedulerMainChatModelClient::new(self.scheduler.clone(), self.life_model.clone())
            .generate_direct_answer(request)
            .await
    }

    fn route_metadata(&self) -> MainChatRouteMetadata {
        if self.direct_reply.is_some() {
            MainChatRouteMetadata {
                provider: "direct".into(),
                model: "L1_reflex".into(),
                route_type: "direct".into(),
                prefer_local: false,
                local_model: "".into(),
                reason: "main_chat_kernel_direct_reflex".into(),
                privacy_level: RedactionLevel::None,
                tools_enabled: false,
                live_eval_required: false,
                final_acceptance_gate_required: false,
                readiness_gate_required: false,
                scripted_response_configured: false,
            }
        } else {
            route_metadata_from_scheduler(&self.scheduler)
        }
    }
}

#[derive(Debug, Clone)]
pub struct MainChatKernel<C = SchedulerMainChatModelClient> {
    model_client: C,
    context_config: MainChatKernelContextConfig,
}

impl MainChatKernel<SchedulerMainChatModelClient> {
    pub fn with_scheduler(scheduler: InferenceScheduler, life_model: LifeModel) -> Self {
        Self::new(SchedulerMainChatModelClient::new(scheduler, life_model))
    }
}

impl<C> MainChatKernel<C>
where
    C: MainChatModelClient,
{
    pub fn new(model_client: C) -> Self {
        Self {
            model_client,
            context_config: MainChatKernelContextConfig::default(),
        }
    }

    pub fn with_context_config(mut self, context_config: MainChatKernelContextConfig) -> Self {
        self.context_config = context_config;
        self
    }

    pub async fn run_turn<S>(
        &self,
        input: MainChatTurnInput,
        event_sink: &mut S,
    ) -> MainChatTurnResult
    where
        S: MainChatEventSink + ?Sized,
    {
        let selected_skill_id =
            sanitize_main_chat_selected_skill_id(input.selected_skill_id.as_deref());
        let session_id = input.session_id.trim();

        event_sink.emit(MainChatKernelEvent::TurnStarted {
            session_id: bounded_label(session_id, MAX_ROUTE_LABEL_CHARS),
            selected_skill_id: selected_skill_id.clone(),
        });

        if session_id.is_empty() {
            return self.blocked("invalid_session_id", event_sink);
        }

        if !has_valid_user_turn(&input.messages) {
            return self.blocked("invalid_user_turn", event_sink);
        }

        let (context_metadata, system_prompt) =
            self.compile_context(session_id, selected_skill_id.clone());
        event_sink.emit(MainChatKernelEvent::ContextLoaded {
            context_snapshot_ref: context_metadata.context_snapshot_ref.clone(),
            selected_source_count: context_metadata.selected_source_count,
            selected_skill_instruction_loaded: context_metadata.selected_skill_instruction_loaded,
        });

        let route_metadata = self.model_client.route_metadata();
        event_sink.emit(MainChatKernelEvent::RouteSelected {
            route_metadata: route_metadata.clone(),
        });

        let request = MainChatModelRequest {
            messages: input.messages,
            system_prompt,
            context_snapshot_ref: context_metadata.context_snapshot_ref.clone(),
            selected_skill_id,
        };

        match self.model_client.generate_direct_answer(request).await {
            Ok(reply) if !reply.trim().is_empty() => {
                let assistant_message = ChatMessage {
                    role: "assistant".into(),
                    content: reply,
                };
                event_sink.emit(MainChatKernelEvent::FinalAnswer {
                    content_preview: bounded_label(
                        &assistant_message.content,
                        MAX_ASSISTANT_PREVIEW_CHARS,
                    ),
                    content_chars: assistant_message.content.chars().count(),
                });
                MainChatTurnResult {
                    assistant_message: Some(assistant_message),
                    blockers: Vec::new(),
                    proposals: Vec::new(),
                    tool_calls: Vec::new(),
                    route_metadata: Some(route_metadata),
                    context_metadata: Some(context_metadata),
                    direct_writes_executed: false,
                    legacy_fallback_used: false,
                }
            }
            Ok(_) => self.blocked("model_generation_empty", event_sink),
            Err(_) => self.blocked("model_generation_failed", event_sink),
        }
    }

    fn blocked<S>(&self, code: &'static str, event_sink: &mut S) -> MainChatTurnResult
    where
        S: MainChatEventSink + ?Sized,
    {
        event_sink.emit(MainChatKernelEvent::Blocker { code: code.into() });
        MainChatTurnResult::blocked(code)
    }

    fn compile_context(
        &self,
        session_id: &str,
        selected_skill_id: Option<String>,
    ) -> (MainChatKernelContextMetadata, String) {
        let mut candidates = kernel_base_context_candidates(session_id);
        if self.context_config.load_workspace_knowledge {
            candidates.extend(load_current_workspace_knowledge_context_candidates(
                selected_skill_id.as_deref(),
            ));
        }
        candidates.extend(self.context_config.extra_candidates.clone());

        let compiled = ContextCompiler.compile(ContextCompilerInput {
            strategy: MainChatAgentStrategy::DirectAnswer,
            privacy_risk: kernel_privacy_summary(),
            active_session_id: Some(session_id.to_string()),
            token_budget: self.context_config.token_budget.max(1),
            selected_skill_id: selected_skill_id.clone(),
            candidates: candidates.clone(),
        });

        let system_prompt = build_system_prompt(&compiled, &candidates);
        let selected_source_ids = compiled
            .selected_sources
            .iter()
            .map(|source| bounded_label(&source.source_id, MAX_ROUTE_LABEL_CHARS))
            .collect::<Vec<_>>();

        (
            MainChatKernelContextMetadata {
                context_snapshot_ref: compiled.context_snapshot_ref.clone(),
                selected_source_count: compiled.selected_sources.len(),
                selected_source_ids,
                selected_skill_id,
                selected_skill_instruction_loaded: compiled.selected_skill_instruction_loaded,
                raw_life_model_yaml_included: compiled.raw_life_model_yaml_included,
                raw_topk_memory_trusted: compiled.raw_topk_memory_trusted,
                workspace_policy_override_blocked: compiled.workspace_policy_override_blocked,
                system_prompt_chars: system_prompt.chars().count(),
            },
            system_prompt,
        )
    }
}

#[allow(clippy::too_many_arguments)]
async fn build_successful_kernel_command_surface_result(
    session_id: &str,
    user_text: &str,
    state: &Arc<AppState>,
    main_chat_agent_turn: &MainChatAgentTurn,
    mut execution_transcript: Vec<ExecutionTranscriptEntry>,
    kernel_result: MainChatTurnResult,
    scheduler: InferenceScheduler,
    life_model: LifeModel,
    direct_reflex_used: bool,
    event_sink_label: &'static str,
    kernel_events: Vec<MainChatKernelEvent>,
) -> Result<MainChatKernelCommandSurfaceResult, String> {
    let task_session_id = main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref()
        .ok_or_else(|| "Main Chat kernel task session missing".to_string())?;
    let assistant_message = kernel_result
        .assistant_message
        .clone()
        .ok_or_else(|| "Main Chat kernel direct answer missing assistant message".to_string())?;
    let reply = assistant_message.content.clone();
    let route_metadata = kernel_result
        .route_metadata
        .clone()
        .ok_or_else(|| "Main Chat kernel direct answer missing route metadata".to_string())?;
    let model_route = model_route_from_kernel_route(&route_metadata);
    let context_summary = context_summary_from_kernel_result(&kernel_result, &life_model);
    let scripted_provider_response = route_metadata.scripted_response_configured;
    let provider_endpoint_kind = if direct_reflex_used {
        "direct_reflex"
    } else {
        main_chat_provider_endpoint_kind(&scheduler, scripted_provider_response)
    };
    let live_provider_invoked = !direct_reflex_used
        && !scripted_provider_response
        && route_metadata.provider != "none"
        && route_metadata.route_type == "cloud";
    let kernel_event_count = kernel_events.len();
    let generation_metadata = serde_json::json!({
        "hsPacketSelected": false,
        "toolCallCount": 0,
        "directWritesExecuted": false,
        "legacyFallbackUsed": false,
        "kernelBackedDirectAnswer": true,
        "kernelEventSink": event_sink_label,
        "kernelEventCount": kernel_event_count,
        "kernelContextSnapshotRef": kernel_result
            .context_metadata
            .as_ref()
            .map(|metadata| metadata.context_snapshot_ref.clone()),
        "modelGenerated": !direct_reflex_used,
        "schedulerGenerationCalled": !direct_reflex_used,
        "providerGenerationPath": if direct_reflex_used {
            "main_chat_kernel_direct_reflex"
        } else {
            "main_chat_direct_answer_scheduler"
        },
        "provider": route_metadata.provider,
        "model": route_metadata.model,
        "routeType": route_metadata.route_type,
        "routeReason": route_metadata.reason,
        "providerHealthEstimated": false,
        "scriptedProviderResponse": scripted_provider_response,
        "liveProviderInvoked": live_provider_invoked,
        "providerEndpointKind": provider_endpoint_kind,
        "localProviderHttpHarness": live_provider_invoked
            && provider_endpoint_kind == "local_test_http",
        "externalLiveProviderEvalPreflighted": false,
    });
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::Observation,
            "DirectAnswer generated a model response without tools or writes.",
            generation_metadata.clone(),
        )
        .await,
    );

    let mut agent_run = AgentRun::new_chat_run(session_id, user_text);
    agent_run.reasoning_strategy = Some("main_chat_agent_v1_direct_answer".into());
    agent_run.complete(&preview_text(&reply, 200), model_route, context_summary);
    let mut reasoning_trace = ReasoningTrace {
        generation_result: Some({
            let mut generation = generation_metadata;
            if let Some(object) = generation.as_object_mut() {
                object.insert("text".into(), serde_json::Value::String(reply.clone()));
                object.insert("mainChatAgentV1".into(), serde_json::Value::Bool(true));
                object.insert(
                    "selectedStrategy".into(),
                    serde_json::Value::String(MainChatAgentStrategy::DirectAnswer.as_str().into()),
                );
            }
            generation
        }),
        ..Default::default()
    };
    finalize_chat_agent_run(
        session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        state,
    )
    .await?;
    complete_main_chat_agent_turn_session(
        state,
        main_chat_agent_turn,
        "DirectAnswer completed without tool execution.",
    )
    .await;
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::FinalResult,
            "DirectAnswer completed without tool execution.",
            serde_json::json!({
                "runId": agent_run.id,
                "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                "legacyFallbackUsed": false,
                "kernelBackedDirectAnswer": true,
                "pendingBlockerCount": 0,
            }),
        )
        .await,
    );
    let agent_state =
        assemble_main_chat_agent_state_for_turn(state, Some(task_session_id), Some(&agent_run.id))
            .await;
    let durable_events =
        materialize_optional_main_chat_agent_events(state, agent_state.as_ref()).await?;

    Ok(MainChatKernelCommandSurfaceResult {
        reply,
        reasoning_trace,
        tool_calls: Vec::new(),
        run_id: Some(agent_run.id),
        agent_ingress: Some(main_chat_agent_turn.decision.clone()),
        agent_state,
        execution_transcript,
        legacy_fallback_used: false,
        durable_events,
        kernel_events,
    })
}

async fn build_blocked_kernel_command_surface_result(
    session_id: &str,
    task_session_id: &str,
    state: &Arc<AppState>,
    main_chat_agent_turn: &MainChatAgentTurn,
    mut execution_transcript: Vec<ExecutionTranscriptEntry>,
    kernel_result: MainChatTurnResult,
    event_sink_label: &'static str,
    kernel_events: Vec<MainChatKernelEvent>,
) -> Result<MainChatKernelCommandSurfaceResult, String> {
    let blockers = kernel_result.blockers.clone();
    let blocker_summary = blockers.join(",");
    if let Some(ref store_arc) = state.main_chat_agent_session_store {
        let store = store_arc.lock().await;
        if let Err(err) = store.set_pending_blockers(task_session_id, blockers.clone()) {
            log::warn!("[MainChatKernel] set blockers failed: {}", err);
        }
        if let Err(err) = store.block_session(
            task_session_id,
            "MainChatKernel direct answer blocked before model completion.",
        ) {
            log::warn!("[MainChatKernel] block session failed: {}", err);
        }
    }
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::Error,
            "DirectAnswer kernel returned a blocker.",
            serde_json::json!({
                "blockers": blockers,
                "directWritesExecuted": false,
                "legacyFallbackUsed": false,
                "kernelBackedDirectAnswer": true,
                "kernelEventSink": event_sink_label,
                "kernelEventCount": kernel_events.len(),
                "modelGenerated": false,
                "schedulerGenerationCalled": false,
            }),
        )
        .await,
    );
    let reply = format!(
        "I could not run the direct answer because the request was blocked: {}.",
        blocker_summary
    );
    let mut agent_run = AgentRun::new_chat_run(session_id, "");
    agent_run.reasoning_strategy = Some("main_chat_agent_v1_direct_answer".into());
    agent_run.fail(AgentRunError {
        message: blocker_summary.clone(),
        phase: "main_chat_kernel".into(),
        recoverable: true,
    });
    let reasoning_trace = ReasoningTrace {
        generation_result: Some(serde_json::json!({
            "text": reply,
            "mainChatAgentV1": true,
            "selectedStrategy": MainChatAgentStrategy::DirectAnswer.as_str(),
            "legacyFallbackUsed": false,
            "directWritesExecuted": false,
            "kernelBackedDirectAnswer": true,
            "kernelEventSink": event_sink_label,
            "kernelEventCount": kernel_events.len(),
            "modelGenerated": false,
            "schedulerGenerationCalled": false,
            "blockers": kernel_result.blockers,
        })),
        ..Default::default()
    };
    agent_run.reasoning_trace = Some(reasoning_trace.clone());
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(err) = store.create_run(&agent_run) {
            log::warn!("[MainChatKernel] create failed AgentRun failed: {}", err);
        }
    }
    let agent_state =
        assemble_main_chat_agent_state_for_turn(state, Some(task_session_id), Some(&agent_run.id))
            .await;
    let durable_events =
        materialize_optional_main_chat_agent_events(state, agent_state.as_ref()).await?;

    Ok(MainChatKernelCommandSurfaceResult {
        reply,
        reasoning_trace,
        tool_calls: Vec::new(),
        run_id: Some(agent_run.id),
        agent_ingress: Some(main_chat_agent_turn.decision.clone()),
        agent_state,
        execution_transcript,
        legacy_fallback_used: false,
        durable_events,
        kernel_events,
    })
}

async fn command_surface_kernel_context_candidates(
    state: &Arc<AppState>,
    selected_skill_id: Option<&str>,
) -> Vec<ContextSourceCandidate> {
    let mut candidates = Vec::new();
    let configured_knowledge_roots = {
        let config = state.config.lock().await;
        config.system.knowledge_roots.clone()
    };
    candidates.extend(load_configured_knowledge_context_candidates(
        &configured_knowledge_roots,
        selected_skill_id,
    ));
    if let Some(lifecycle_store) = state.memory_lifecycle_store.as_ref() {
        let store = lifecycle_store.lock().await;
        if let Ok(records) = store.list_active_records(None, 8) {
            for record in records {
                candidates.push(ContextSourceCandidate::new(
                    ContextSourceKind::SelectedPersonalContext,
                    &record.memory_id,
                    format!(
                        "Accepted memory [{}:{}]: {}",
                        record.scope, record.category, record.content
                    ),
                    format!(
                        "accepted memory lifecycle; materialized view {} version {}",
                        record.materialized_view_id.as_deref().unwrap_or("unknown"),
                        record.materialized_view_version.unwrap_or_default()
                    ),
                    "private",
                    16,
                ));
            }
        }
    }
    if let Ok(sessions) = {
        let store = state.memory_store.lock().await;
        store.list_sessions(5)
    } {
        candidates.push(ContextSourceCandidate::new(
            ContextSourceKind::SelectedPersonalContext,
            "chat_sessions.recent",
            format!(
                "Recent session count available for search: {}",
                sessions.len()
            ),
            "bounded session search metadata",
            "internal",
            8,
        ));
    }
    candidates
}

fn model_route_from_kernel_route(route: &MainChatRouteMetadata) -> ModelRouteTrace {
    ModelRouteTrace {
        provider: route.provider.clone(),
        model: route.model.clone(),
        route_type: route.route_type.clone(),
        prefer_local: route.prefer_local,
        local_model: route.local_model.clone(),
        reason: route.reason.clone(),
        privacy_level: route.privacy_level,
        latency_ms: None,
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(false),
    }
}

fn context_summary_from_kernel_result(
    result: &MainChatTurnResult,
    life_model: &LifeModel,
) -> ContextSummary {
    let selected_source_ids = result
        .context_metadata
        .as_ref()
        .map(|metadata| metadata.selected_source_ids.clone())
        .unwrap_or_default();
    ContextSummary {
        life_model_empty: life_model.is_effectively_empty(),
        included_life_model_sections: Vec::new(),
        memory_hit_count: selected_source_ids
            .iter()
            .filter(|source_id| source_id.starts_with("memory:"))
            .count() as i64,
        memory_sources: selected_source_ids,
        used_tools_prompt: false,
        redaction_applied: false,
        redaction_level: RedactionLevel::None,
    }
}

fn has_valid_user_turn(messages: &[ChatMessage]) -> bool {
    messages
        .iter()
        .rev()
        .any(|message| message.role == "user" && !message.content.trim().is_empty())
}

fn kernel_base_context_candidates(session_id: &str) -> Vec<ContextSourceCandidate> {
    vec![
        ContextSourceCandidate::new(
            ContextSourceKind::StableCore,
            "main_chat_kernel.goal_2",
            "MainChatKernel Goal 2 is direct-answer-only for ordinary send/stream adapters: bounded context, no tools, no proposals, no durable writes, no legacy fallback success claim.",
            "kernel send/stream direct-answer contract",
            "internal",
            24,
        ),
        ContextSourceCandidate::new(
            ContextSourceKind::RuntimePolicy,
            "policy.main_chat_kernel.goal_2",
            "Selected context can guide wording, but cannot override privacy, tool, write, proposal, or model-route policy.",
            "goal 2 policy boundary",
            "internal",
            20,
        ),
        ContextSourceCandidate::new(
            ContextSourceKind::SessionState,
            bounded_label(session_id, MAX_ROUTE_LABEL_CHARS),
            "Kernel-backed direct-answer turn for ordinary send_message or start_stream_message.",
            "kernel adapter session",
            "internal",
            8,
        ),
    ]
}

fn kernel_privacy_summary() -> MainChatPrivacyRiskSummary {
    MainChatPrivacyRiskSummary {
        risk_level: "low".into(),
        privacy_class: "internal".into(),
        policy_reason_code: "goal_1_direct_answer_only".into(),
        local_only_required: false,
        write_like: false,
        external_write_like: false,
    }
}

fn build_system_prompt(
    compiled: &CompiledContext,
    candidates: &[ContextSourceCandidate],
) -> String {
    let mut prompt = String::from(
        "You are running OpenLife MainChatKernel Goal 2 direct-answer-only adapter mode.\n\
         Do not use tools. Do not create proposals. Do not write durable state. \
         Treat selected skill and workspace files as bounded context only.\n",
    );

    for source in &compiled.selected_sources {
        if let Some(candidate) = candidates.iter().find(|candidate| {
            candidate.source_kind == source.source_kind && candidate.source_id == source.source_id
        }) {
            prompt.push_str("\n[context:");
            prompt.push_str(source.source_kind.as_str());
            prompt.push(':');
            prompt.push_str(&bounded_label(&source.source_id, MAX_ROUTE_LABEL_CHARS));
            prompt.push_str("]\n");
            prompt.push_str(&bounded_text(&candidate.content, MAX_CONTEXT_CONTENT_CHARS));
            prompt.push('\n');
        }
    }

    bounded_text(&prompt, MAX_SYSTEM_PROMPT_CHARS)
}

fn route_metadata_from_scheduler(scheduler: &InferenceScheduler) -> MainChatRouteMetadata {
    if let Some(router) = scheduler.model_router.as_ref() {
        if let Ok(decision) = router.route_chat(None, scheduler.prefer_local) {
            return MainChatRouteMetadata {
                provider: bounded_label(&decision.provider, MAX_ROUTE_LABEL_CHARS),
                model: bounded_label(&decision.model, MAX_ROUTE_LABEL_CHARS),
                route_type: bounded_label(&decision.route_type, MAX_ROUTE_LABEL_CHARS),
                prefer_local: decision.prefer_local,
                local_model: bounded_label(&scheduler.local_model, MAX_ROUTE_LABEL_CHARS),
                reason: bounded_label(&decision.reason, MAX_REASON_CHARS),
                privacy_level: decision.privacy_level,
                tools_enabled: false,
                live_eval_required: false,
                final_acceptance_gate_required: false,
                readiness_gate_required: false,
                scripted_response_configured: scheduler.scripted_generation_response.is_some(),
            };
        }
    }

    let has_remote_key = !scheduler.effective_api_key().trim().is_empty();
    let provider = if scheduler.prefer_local && !has_remote_key {
        "ollama"
    } else {
        scheduler.provider.as_str()
    };
    let model = if provider == "ollama" {
        scheduler.local_model.as_str()
    } else {
        scheduler.chat_model.as_str()
    };
    let route_type = if provider == "ollama" {
        "local"
    } else {
        "cloud"
    };

    MainChatRouteMetadata {
        provider: bounded_label(provider, MAX_ROUTE_LABEL_CHARS),
        model: bounded_label(model, MAX_ROUTE_LABEL_CHARS),
        route_type: route_type.into(),
        prefer_local: scheduler.prefer_local,
        local_model: bounded_label(&scheduler.local_model, MAX_ROUTE_LABEL_CHARS),
        reason: "scheduler_config_direct_answer_no_tools".into(),
        privacy_level: RedactionLevel::Light,
        tools_enabled: false,
        live_eval_required: false,
        final_acceptance_gate_required: false,
        readiness_gate_required: false,
        scripted_response_configured: scheduler.scripted_generation_response.is_some(),
    }
}

fn bounded_label(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    let mut last_was_space = false;
    for ch in value.trim().chars() {
        if ch.is_control() {
            continue;
        }
        if ch.is_whitespace() {
            if !last_was_space {
                output.push(' ');
                last_was_space = true;
            }
            continue;
        }
        output.push(ch);
        last_was_space = false;
        if output.chars().count() >= max_chars {
            break;
        }
    }
    output.trim().to_string()
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        if ch.is_control() && ch != '\n' && ch != '\t' {
            continue;
        }
        output.push(ch);
        if output.chars().count() >= max_chars {
            break;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::model_router::{ModelRouter, ProviderAvailability};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct ScriptedModelClient {
        response: Result<String, String>,
        calls: Arc<AtomicUsize>,
        prompts: Arc<Mutex<Vec<String>>>,
        route_metadata: MainChatRouteMetadata,
    }

    impl ScriptedModelClient {
        fn ok(response: impl Into<String>) -> Self {
            Self {
                response: Ok(response.into()),
                calls: Arc::new(AtomicUsize::new(0)),
                prompts: Arc::new(Mutex::new(Vec::new())),
                route_metadata: MainChatRouteMetadata {
                    provider: "test_provider".into(),
                    model: "test_model".into(),
                    route_type: "direct".into(),
                    prefer_local: false,
                    local_model: "test_local".into(),
                    reason: "test_route".into(),
                    privacy_level: RedactionLevel::Light,
                    tools_enabled: false,
                    live_eval_required: false,
                    final_acceptance_gate_required: false,
                    readiness_gate_required: false,
                    scripted_response_configured: true,
                },
            }
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn observed_prompts(&self) -> Vec<String> {
            self.prompts.lock().expect("prompts lock").clone()
        }
    }

    #[async_trait]
    impl MainChatModelClient for ScriptedModelClient {
        async fn generate_direct_answer(
            &self,
            request: MainChatModelRequest,
        ) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.prompts
                .lock()
                .expect("prompts lock")
                .push(request.system_prompt);
            self.response.clone()
        }

        fn route_metadata(&self) -> MainChatRouteMetadata {
            self.route_metadata.clone()
        }
    }

    fn user_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: "user".into(),
            content: content.into(),
        }
    }

    fn test_kernel(
        model: ScriptedModelClient,
        extra_candidates: Vec<ContextSourceCandidate>,
    ) -> MainChatKernel<ScriptedModelClient> {
        MainChatKernel::new(model).with_context_config(MainChatKernelContextConfig {
            load_workspace_knowledge: false,
            token_budget: 80,
            extra_candidates,
        })
    }

    #[tokio::test]
    async fn main_chat_kernel_direct_answer_returns_one_response_no_tools_or_writes() {
        let model = ScriptedModelClient::ok("Kernel direct answer.");
        let kernel = test_kernel(model.clone(), Vec::new());
        let mut events = BufferedMainChatEventSink::default();

        let result = kernel
            .run_turn(
                MainChatTurnInput {
                    session_id: "session-1".into(),
                    messages: vec![user_message("Say hello from the kernel.")],
                    selected_skill_id: None,
                },
                &mut events,
            )
            .await;

        assert_eq!(model.call_count(), 1);
        assert_eq!(
            result
                .assistant_message
                .as_ref()
                .map(|message| message.role.as_str()),
            Some("assistant")
        );
        assert_eq!(
            result
                .assistant_message
                .as_ref()
                .map(|message| message.content.as_str()),
            Some("Kernel direct answer.")
        );
        assert!(result.tool_calls.is_empty());
        assert!(result.proposals.is_empty());
        assert!(result.blockers.is_empty());
        assert!(!result.direct_writes_executed);
        assert!(!result.legacy_fallback_used);
        assert!(events.events().iter().any(|event| {
            matches!(event, MainChatKernelEvent::FinalAnswer { content_chars, .. } if *content_chars > 0)
        }));
    }

    #[tokio::test]
    async fn main_chat_kernel_empty_input_returns_named_blocker_without_model_call() {
        let model = ScriptedModelClient::ok("should not be called");
        let kernel = test_kernel(model.clone(), Vec::new());
        let mut events = BufferedMainChatEventSink::default();

        let result = kernel
            .run_turn(
                MainChatTurnInput {
                    session_id: "session-1".into(),
                    messages: vec![user_message("   ")],
                    selected_skill_id: None,
                },
                &mut events,
            )
            .await;

        assert_eq!(model.call_count(), 0);
        assert_eq!(result.blockers, vec!["invalid_user_turn".to_string()]);
        assert!(result.assistant_message.is_none());
        assert!(result.route_metadata.is_none());
        assert!(!result.direct_writes_executed);
        assert!(!result.legacy_fallback_used);
        assert!(events.events().iter().any(|event| {
            matches!(event, MainChatKernelEvent::Blocker { code } if code == "invalid_user_turn")
        }));
    }

    #[tokio::test]
    async fn main_chat_kernel_invalid_session_returns_named_blocker_without_model_call() {
        let model = ScriptedModelClient::ok("should not be called");
        let kernel = test_kernel(model.clone(), Vec::new());
        let mut events = BufferedMainChatEventSink::default();

        let result = kernel
            .run_turn(
                MainChatTurnInput {
                    session_id: "   ".into(),
                    messages: vec![user_message("Hello")],
                    selected_skill_id: None,
                },
                &mut events,
            )
            .await;

        assert_eq!(model.call_count(), 0);
        assert_eq!(result.blockers, vec!["invalid_session_id".to_string()]);
        assert!(result.assistant_message.is_none());
        assert!(!result.direct_writes_executed);
        assert!(!result.legacy_fallback_used);
    }

    #[tokio::test]
    async fn main_chat_kernel_selected_skill_context_is_sanitized_and_policy_bound() {
        let model = ScriptedModelClient::ok("Skill-aware direct answer.");
        let skill_candidate = ContextSourceCandidate::new(
            ContextSourceKind::SkillInstruction,
            "skills/summarize/SKILL.md",
            "selected skill instruction: answer concisely",
            "selected skill instruction",
            "internal",
            12,
        )
        .for_skill("summarize");
        let kernel = test_kernel(model.clone(), vec![skill_candidate]);
        let mut events = BufferedMainChatEventSink::default();

        let result = kernel
            .run_turn(
                MainChatTurnInput {
                    session_id: "session-1".into(),
                    messages: vec![user_message("Summarize this.")],
                    selected_skill_id: Some(" summarize ".into()),
                },
                &mut events,
            )
            .await;

        let context = result.context_metadata.as_ref().expect("context metadata");
        assert_eq!(context.selected_skill_id.as_deref(), Some("summarize"));
        assert!(context.selected_skill_instruction_loaded);
        assert!(context.workspace_policy_override_blocked);
        assert!(!context.raw_life_model_yaml_included);
        assert!(!context.raw_topk_memory_trusted);
        assert!(model
            .observed_prompts()
            .join("\n")
            .contains("selected skill instruction: answer concisely"));
        assert!(result.tool_calls.is_empty());
        assert!(result.proposals.is_empty());
        assert!(!result.direct_writes_executed);
        assert!(!result.legacy_fallback_used);
    }

    #[tokio::test]
    async fn main_chat_kernel_provider_route_metadata_is_bounded_without_live_gate() {
        let mut router = ModelRouter::new();
        router.providers.insert(
            "openai".into(),
            ProviderAvailability {
                provider: "openai".into(),
                available: true,
                latency_ms: Some(320),
                models: vec!["gpt-kernel-test".into()],
                last_checked: chrono::Utc::now(),
                last_error: None,
                health_is_estimated: true,
            },
        );
        let scheduler = InferenceScheduler::new(
            "qwen2.5:7b".into(),
            false,
            "openai\n".into(),
            "https://api.openai.com/v1".into(),
            "test-key".into(),
            "gpt-kernel-test-with-a-very-long-model-name-that-should-still-be-bounded-for-audit-metadata".into(),
            "text-embedding-3-small".into(),
            false,
        )
        .with_model_router(router)
        .with_scripted_generation_response("Scheduler-backed direct answer.");
        let kernel = MainChatKernel::with_scheduler(scheduler, LifeModel::default())
            .with_context_config(MainChatKernelContextConfig {
                load_workspace_knowledge: false,
                token_budget: 80,
                extra_candidates: Vec::new(),
            });
        let mut events = BufferedMainChatEventSink::default();

        let result = kernel
            .run_turn(
                MainChatTurnInput {
                    session_id: "session-1".into(),
                    messages: vec![user_message("Route metadata please.")],
                    selected_skill_id: None,
                },
                &mut events,
            )
            .await;

        let route = result.route_metadata.as_ref().expect("route metadata");
        assert!(!route.provider.contains('\n'));
        assert!(route.model.chars().count() <= MAX_ROUTE_LABEL_CHARS);
        assert!(!route.tools_enabled);
        assert!(!route.live_eval_required);
        assert!(!route.final_acceptance_gate_required);
        assert!(!route.readiness_gate_required);
        assert!(route.scripted_response_configured);
        assert_eq!(
            result
                .assistant_message
                .as_ref()
                .map(|message| message.content.as_str()),
            Some("Scheduler-backed direct answer.")
        );
        assert!(events.events().iter().any(|event| {
            matches!(event, MainChatKernelEvent::RouteSelected { route_metadata } if !route_metadata.live_eval_required)
        }));
    }

    #[test]
    fn main_chat_kernel_goal_2_send_and_stream_use_kernel_direct_answer_adapter() {
        let send_source = include_str!("main_chat_send.rs");
        let stream_source = include_str!("main_chat_streaming.rs");

        assert!(send_source.contains("run_main_chat_kernel_direct_answer_with_state"));
        assert!(stream_source.contains("run_main_chat_kernel_direct_answer_with_state"));
        assert!(send_source.contains("BufferedMainChatEventSink"));
        assert!(stream_source.contains("StreamingMainChatEventSink"));
    }

    #[test]
    fn main_chat_kernel_goal_1_has_no_final_live_or_tool_runtime_dependency() {
        let source = include_str!("main_chat_kernel.rs");
        let final_gate = ["main_chat_", "final_gate"].concat();
        let live_provider = ["main_chat_", "live_provider"].concat();
        let action_executor = ["Action", "Executor"].concat();

        assert!(!source.contains(&final_gate));
        assert!(!source.contains(&live_provider));
        assert!(!source.contains(&action_executor));
    }
}
