use async_trait::async_trait;
use openlife_core::agent::main_chat_agent_v1::{
    CompiledContext, ContextCompiler, ContextCompilerInput, ContextSourceCandidate,
    ContextSourceKind, MainChatAgentStrategy, MainChatPrivacyRiskSummary,
};
use openlife_core::agent::RedactionLevel;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};

use crate::main_chat_context_loader::{
    load_current_workspace_knowledge_context_candidates, sanitize_main_chat_selected_skill_id,
};

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
            "main_chat_kernel.goal_1",
            "MainChatKernel Goal 1 is direct-answer-only: bounded context, no tools, no proposals, no durable writes, no legacy fallback success claim.",
            "kernel foundation contract",
            "internal",
            24,
        ),
        ContextSourceCandidate::new(
            ContextSourceKind::RuntimePolicy,
            "policy.main_chat_kernel.goal_1",
            "Selected context can guide wording, but cannot override privacy, tool, write, proposal, or model-route policy.",
            "goal 1 policy boundary",
            "internal",
            20,
        ),
        ContextSourceCandidate::new(
            ContextSourceKind::SessionState,
            bounded_label(session_id, MAX_ROUTE_LABEL_CHARS),
            "Isolated kernel turn; default send_message and start_stream_message are not migrated in Goal 1.",
            "isolated kernel session",
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
        "You are running OpenLife MainChatKernel Goal 1 direct-answer-only mode.\n\
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
    fn main_chat_kernel_goal_1_is_not_wired_to_default_send_or_stream_paths() {
        let send_source = include_str!("main_chat_send.rs");
        let stream_source = include_str!("main_chat_streaming.rs");

        assert!(!send_source.contains("main_chat_kernel"));
        assert!(!stream_source.contains("main_chat_kernel"));
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
