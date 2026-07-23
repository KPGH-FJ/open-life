use crate::agent::context_assembler::{AssembleInput, CompositeAssembler, ContextAssembler};
use crate::agent::reasoning::{
    DirectReasoner, LayeredReasoner, ReasoningConfig, ReasoningError, ReasoningInput,
    ReasoningStrategy, ReasoningTrace,
};
use crate::agent::types::AgentTask;
use crate::agent::{
    behavior_checks_for_packet, HSBehaviorCheckSummary, HSSelectionAudit,
    RuntimeGuidanceConsumptionMode, RuntimeHSPacket, RuntimeInput, RuntimeOutput,
};
use crate::layer::Layer;
use crate::life_model::LifeModel;
use crate::llm::{
    BoundedContextBlock, ChatMessage, ContextManifest, ProviderLocalOnlyReason,
    ProviderPayloadPurpose, ProviderPolicyAuthorization, ProviderPolicyProvenanceRef,
};
use crate::scheduler::InferenceScheduler;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Runtime configuration.
#[derive(Debug, Clone)]
pub struct AgentRuntimeConfig {
    pub default_strategy: String,
    pub meaning_timeout_ms: u64,
    pub strategy_timeout_ms: u64,
    pub generation_timeout_ms: u64,
}

impl Default for AgentRuntimeConfig {
    fn default() -> Self {
        Self {
            default_strategy: "layered".to_string(),
            meaning_timeout_ms: 5000,
            strategy_timeout_ms: 15000,
            generation_timeout_ms: 30000,
        }
    }
}

/// The AgentRuntime is the central orchestrator for executing AgentTasks.
/// It coordinates context assembly, reasoning strategy selection, and LLM generation.
pub struct AgentRuntime {
    context_assembler: CompositeAssembler,
    reasoning_strategies: HashMap<String, Box<dyn ReasoningStrategy>>,
    scheduler: InferenceScheduler,
    network_policy: crate::config::NetworkPolicy,
    config: AgentRuntimeConfig,
}

fn layered_provider_policy_context(
    hs_packet: Option<&RuntimeHSPacket>,
    _session_id: &str,
) -> (
    ProviderPolicyAuthorization,
    Vec<ProviderPolicyProvenanceRef>,
) {
    let provider_authorization = hs_packet
        .map(|packet| packet.provider_authorization().clone())
        .unwrap_or_else(|| {
            ProviderPolicyAuthorization::local_only_fail_closed(
                ProviderLocalOnlyReason::MissingCanonicalPolicy,
            )
        });
    let policy_provenance_refs = hs_packet
        .map(RuntimeHSPacket::provider_policy_provenance_refs)
        .unwrap_or_default();
    (provider_authorization, policy_provenance_refs)
}

impl AgentRuntime {
    pub fn new(
        life_model: LifeModel,
        scheduler: InferenceScheduler,
        app_config: &crate::config::AppConfig,
    ) -> Self {
        Self::new_with_runtime_config(
            life_model,
            scheduler,
            app_config.system.network_policy.clone(),
            AgentRuntimeConfig {
                default_strategy: app_config.reasoning.default_strategy.clone(),
                meaning_timeout_ms: app_config.reasoning.meaning_timeout_ms,
                strategy_timeout_ms: app_config.reasoning.strategy_timeout_ms,
                generation_timeout_ms: app_config.reasoning.generation_timeout_ms,
            },
        )
    }

    /// Build a runtime from the exact provider-safe configuration fields it
    /// consumes. Product callers can therefore avoid retaining an `AppConfig`
    /// snapshot (and any unrelated hydrated credentials) across execution.
    pub fn new_with_runtime_config(
        _life_model: LifeModel,
        scheduler: InferenceScheduler,
        network_policy: crate::config::NetworkPolicy,
        config: AgentRuntimeConfig,
    ) -> Self {
        let mut strategies: HashMap<String, Box<dyn ReasoningStrategy>> = HashMap::new();

        // Direct reasoning is stateless. Layered reasoning is constructed per turn
        // only after the outer HS data-route decision has been resolved.
        let direct = DirectReasoner::new();
        strategies.insert("direct".to_string(), Box::new(direct));

        Self {
            context_assembler: CompositeAssembler::new()
                .with(Box::new(crate::agent::LifeModelAssembler))
                .with(Box::new(crate::agent::PrivacyAssembler))
                .with(Box::new(crate::agent::MemoryAssembler))
                .with(Box::new(crate::agent::ToolsAssembler)),
            reasoning_strategies: strategies,
            scheduler,
            network_policy,
            config,
        }
    }

    pub fn with_config(
        life_model: LifeModel,
        scheduler: InferenceScheduler,
        config: AgentRuntimeConfig,
    ) -> Self {
        let app_config = crate::config::AppConfig {
            reasoning: crate::config::ReasoningConfig {
                default_strategy: config.default_strategy.clone(),
                meaning_timeout_ms: config.meaning_timeout_ms,
                strategy_timeout_ms: config.strategy_timeout_ms,
                generation_timeout_ms: config.generation_timeout_ms,
            },
            ..Default::default()
        };
        let mut runtime = Self::new(life_model, scheduler, &app_config);
        runtime.config = config;
        runtime
    }

    /// Execute a task and return the reasoning output.
    /// This is the main entry point for the AgentRuntime.
    pub async fn execute_task(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
    ) -> Result<AgentRuntimeOutput, AgentRuntimeError> {
        self.execute_task_inner(
            task,
            life_model,
            tools_prompt,
            memory_context,
            memory_hits,
            privacy_engine,
            None,
            RuntimeGuidanceConsumptionMode::Disabled,
        )
        .await
    }

    /// Execute the current runtime through the RuntimeInput/RuntimeOutput contract.
    ///
    /// This is intentionally a thin adapter over the existing task execution and
    /// scheduler generation path. ReAct tool execution remains owned by AgentLoop.
    pub async fn execute_runtime_input(
        &self,
        input: RuntimeInput,
    ) -> Result<RuntimeOutput, AgentRuntimeError> {
        let (provider_authorization, policy_provenance_refs) =
            layered_provider_policy_context(input.hs_packet.as_ref(), &input.task.session_id);
        let params = input.agent_runtime_params();
        let runtime_output = self
            .execute_task_with_hs_packet_and_guidance_mode(
                params.task,
                params.life_model,
                params.tools_prompt,
                params.memory_context,
                vec![],
                crate::privacy::PrivacyEngine::new(),
                params.hs_packet,
                params.guidance_consumption_mode,
            )
            .await?;

        let tools_required = !input.tools_prompt.trim().is_empty();
        let context_blocks = if tools_required {
            vec![BoundedContextBlock {
                source_ref: "tool_gateway.manifest".into(),
                category: "typed_tool_contract".into(),
                content: input.tools_prompt.clone(),
            }]
        } else {
            Vec::new()
        };
        let provider_authorization = provider_authorization
            .authorize_derived_payload(
                ProviderPayloadPurpose::AgentRuntimeGeneration,
                &input.task.user_text,
                &runtime_output.final_messages,
                &context_blocks,
            )
            .map_err(|e| AgentRuntimeError::Generation(e.to_string()))?;
        let selected_context_refs = context_blocks
            .iter()
            .map(|block| block.source_ref.clone())
            .collect::<Vec<_>>();
        let included_context_categories = context_blocks
            .iter()
            .map(|block| block.category.clone())
            .collect::<Vec<_>>();
        let prepared = self
            .scheduler
            .prepare_chat_request_with_authorization(
                runtime_output.final_messages.clone(),
                context_blocks,
                ContextManifest {
                    request_id: Uuid::new_v4().to_string(),
                    privacy_decision_id: provider_authorization.decision_id().to_string(),
                    selected_context_refs,
                    included_context_categories,
                    declared_payload_categories: vec![
                        crate::llm::ProviderPayloadCategory::RuntimeCompiledMessages,
                    ],
                    policy_provenance_refs,
                    raw_life_model_included: false,
                    raw_unbounded_memory_included: false,
                },
                provider_authorization,
                self.network_policy.clone(),
                tools_required,
            )
            .await
            .map_err(|e| AgentRuntimeError::Generation(e.to_string()))?;
        let provider_outcome = self.scheduler.execute_prepared(prepared).await;
        self.scheduler
            .verify_prepared_outcome_receipt(&provider_outcome)
            .map_err(|e| AgentRuntimeError::Generation(e.to_string()))?;
        let user_output = provider_outcome
            .result
            .map_err(AgentRuntimeError::Generation)?;

        Ok(RuntimeOutput {
            run_id: Some(runtime_output.run_id),
            user_output,
            actions: Vec::new(),
            observations: Vec::new(),
            proposal_ids: Vec::new(),
            life_event_candidates: Vec::new(),
            warnings: Vec::new(),
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub async fn execute_task_with_hs_packet(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
        hs_packet: Option<RuntimeHSPacket>,
    ) -> Result<AgentRuntimeOutput, AgentRuntimeError> {
        self.execute_task_inner(
            task,
            life_model,
            tools_prompt,
            memory_context,
            memory_hits,
            privacy_engine,
            hs_packet,
            RuntimeGuidanceConsumptionMode::Disabled,
        )
        .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub async fn execute_task_with_hs_packet_and_guidance_mode(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
        hs_packet: Option<RuntimeHSPacket>,
        guidance_consumption_mode: RuntimeGuidanceConsumptionMode,
    ) -> Result<AgentRuntimeOutput, AgentRuntimeError> {
        self.execute_task_inner(
            task,
            life_model,
            tools_prompt,
            memory_context,
            memory_hits,
            privacy_engine,
            hs_packet,
            guidance_consumption_mode,
        )
        .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    async fn execute_task_inner(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
        hs_packet: Option<RuntimeHSPacket>,
        guidance_consumption_mode: RuntimeGuidanceConsumptionMode,
    ) -> Result<AgentRuntimeOutput, AgentRuntimeError> {
        // Resolve the outer data route synchronously before a layered reasoner
        // can perform provider selection, health probing, or dispatch.
        let (provider_authorization, selected_policy_refs) =
            layered_provider_policy_context(hs_packet.as_ref(), &task.session_id);
        let layered_privacy_engine = privacy_engine.clone();

        // 1. Build AssembleInput
        let input = AssembleInput {
            session_id: task.session_id.clone(),
            messages: Arc::new(task.messages.clone()),
            life_model: Arc::new(life_model.clone()),
            tools_prompt: tools_prompt.to_string(),
            privacy_engine,
            memory_context,
            memory_hits,
            memory_retrieval_time_ms: 0,
        };

        // 2. Assemble context
        let context = self
            .context_assembler
            .assemble(&input)
            .map_err(|e| AgentRuntimeError::ContextAssembly(e.to_string()))?;

        // 3. Build reasoning input
        let reasoning_input = ReasoningInput {
            task_kind: task.kind,
            user_text: task.user_text.clone(),
            session_id: task.session_id.clone(),
        };

        // 4. Execute reasoning. The LayeredReasoner is per-turn so no default
        // PolicyAllowed instance can race ahead of the outer decision.
        let run_id = Uuid::new_v4().to_string();
        let reasoning_output = if task.layer == Layer::L3 {
            LayeredReasoner::with_config(
                self.scheduler.clone(),
                life_model.clone(),
                ReasoningConfig {
                    meaning_timeout_ms: self.config.meaning_timeout_ms,
                    strategy_timeout_ms: self.config.strategy_timeout_ms,
                    generation_timeout_ms: self.config.generation_timeout_ms,
                    max_retries: 1,
                },
            )
            .with_network_policy(self.network_policy.clone())
            .with_provider_policy_context(provider_authorization, selected_policy_refs)
            .with_provider_subject_text(task.user_text.clone())
            .with_privacy_engine(layered_privacy_engine)
            .reason(&reasoning_input, &context, &run_id)
            .await
            .map_err(AgentRuntimeError::Reasoning)?
        } else {
            self.reasoning_strategies
                .get("direct")
                .ok_or_else(|| AgentRuntimeError::StrategyNotFound("direct".to_string()))?
                .reason(&reasoning_input, &context, &run_id)
                .await
                .map_err(AgentRuntimeError::Reasoning)?
        };

        // 5. Build final messages with system prompt
        let mut final_messages = context.desensitized_messages.to_vec();
        if !reasoning_output.system_prompt.is_empty() {
            final_messages.insert(
                0,
                ChatMessage {
                    role: "system".to_string(),
                    content: reasoning_output.system_prompt.clone(),
                },
            );
        }
        let hs_selection_audit = hs_packet.as_ref().map(|packet| packet.audit.clone());
        let hs_behavior_checks = hs_packet
            .as_ref()
            .map(behavior_checks_for_packet)
            .unwrap_or_default();
        if guidance_consumption_mode.is_enabled() {
            if let Some(prompt) = hs_packet.as_ref().and_then(build_hs_runtime_prompt) {
                final_messages.insert(
                    0,
                    ChatMessage {
                        role: "system".to_string(),
                        content: prompt,
                    },
                );
            }
        }

        Ok(AgentRuntimeOutput {
            run_id,
            final_messages,
            reasoning_trace: reasoning_output.trace,
            suggested_tools: reasoning_output.suggested_tools,
            plan_steps: reasoning_output.plan_steps,
            context_summary: context.context_summary,
            hs_selection_audit,
            hs_behavior_checks,
        })
    }

    /// Generate response directly without reasoning (for L1/L2).
    pub async fn generate_direct(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
    ) -> Result<AgentRuntimeOutput, AgentRuntimeError> {
        self.generate_direct_inner(
            task,
            life_model,
            tools_prompt,
            memory_context,
            memory_hits,
            privacy_engine,
            None,
            RuntimeGuidanceConsumptionMode::Disabled,
        )
        .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub async fn generate_direct_with_hs_packet(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
        hs_packet: Option<RuntimeHSPacket>,
    ) -> Result<AgentRuntimeOutput, AgentRuntimeError> {
        self.generate_direct_inner(
            task,
            life_model,
            tools_prompt,
            memory_context,
            memory_hits,
            privacy_engine,
            hs_packet,
            RuntimeGuidanceConsumptionMode::Disabled,
        )
        .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub async fn generate_direct_with_hs_packet_and_guidance_mode(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
        hs_packet: Option<RuntimeHSPacket>,
        guidance_consumption_mode: RuntimeGuidanceConsumptionMode,
    ) -> Result<AgentRuntimeOutput, AgentRuntimeError> {
        self.generate_direct_inner(
            task,
            life_model,
            tools_prompt,
            memory_context,
            memory_hits,
            privacy_engine,
            hs_packet,
            guidance_consumption_mode,
        )
        .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    async fn generate_direct_inner(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
        hs_packet: Option<RuntimeHSPacket>,
        guidance_consumption_mode: RuntimeGuidanceConsumptionMode,
    ) -> Result<AgentRuntimeOutput, AgentRuntimeError> {
        let input = AssembleInput {
            session_id: task.session_id.clone(),
            messages: Arc::new(task.messages.clone()),
            life_model: Arc::new(life_model.clone()),
            tools_prompt: tools_prompt.to_string(),
            privacy_engine,
            memory_context,
            memory_hits,
            memory_retrieval_time_ms: 0,
        };

        let context = self
            .context_assembler
            .assemble(&input)
            .map_err(|e| AgentRuntimeError::ContextAssembly(e.to_string()))?;

        let run_id = Uuid::new_v4().to_string();
        let mut final_messages = context.desensitized_messages.to_vec();
        let hs_selection_audit = hs_packet.as_ref().map(|packet| packet.audit.clone());
        let hs_behavior_checks = hs_packet
            .as_ref()
            .map(behavior_checks_for_packet)
            .unwrap_or_default();
        if guidance_consumption_mode.is_enabled() {
            if let Some(prompt) = hs_packet.as_ref().and_then(build_hs_runtime_prompt) {
                final_messages.insert(
                    0,
                    ChatMessage {
                        role: "system".to_string(),
                        content: prompt,
                    },
                );
            }
        }

        Ok(AgentRuntimeOutput {
            run_id,
            final_messages,
            reasoning_trace: ReasoningTrace::default(),
            suggested_tools: vec![],
            plan_steps: vec![],
            context_summary: context.context_summary,
            hs_selection_audit,
            hs_behavior_checks,
        })
    }
}

/// Output from AgentRuntime execution.
#[derive(Debug, Clone)]
pub struct AgentRuntimeOutput {
    pub run_id: String,
    pub final_messages: Vec<ChatMessage>,
    pub reasoning_trace: ReasoningTrace,
    pub suggested_tools: Vec<String>,
    pub plan_steps: Vec<String>,
    pub context_summary: crate::agent::types::ContextSummary,
    pub hs_selection_audit: Option<HSSelectionAudit>,
    pub hs_behavior_checks: Vec<HSBehaviorCheckSummary>,
}

fn build_hs_runtime_prompt(packet: &RuntimeHSPacket) -> Option<String> {
    if packet.guidance_refs.is_empty() {
        return None;
    }
    let guidance = packet
        .guidance_refs
        .iter()
        .take(4)
        .map(|guidance| {
            format!(
                "- [{}] {}: {}",
                guidance.guidance_id, guidance.impact_kind, guidance.impact_summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "Selected personal collaboration guidance for this run (metadata-safe summaries only):\n{}",
        guidance
    ))
}

/// Errors from AgentRuntime.
#[derive(Debug, Clone)]
pub enum AgentRuntimeError {
    ContextAssembly(String),
    StrategyNotFound(String),
    Reasoning(ReasoningError),
    Generation(String),
}

impl std::fmt::Display for AgentRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRuntimeError::ContextAssembly(e) => write!(f, "Context assembly failed: {}", e),
            AgentRuntimeError::StrategyNotFound(s) => write!(f, "Strategy not found: {}", s),
            AgentRuntimeError::Reasoning(e) => write!(f, "Reasoning failed: {}", e),
            AgentRuntimeError::Generation(e) => write!(f, "Generation failed: {}", e),
        }
    }
}

impl std::error::Error for AgentRuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{AgentTask, AgentTaskKind};
    use crate::layer::Layer;
    use crate::life_model::LifeModel;
    use crate::llm::ChatMessage;
    use crate::privacy::PrivacyEngine;
    use crate::scheduler::InferenceScheduler;

    fn create_test_life_model() -> LifeModel {
        LifeModel::default()
    }

    fn create_test_task() -> AgentTask {
        AgentTask {
            kind: AgentTaskKind::Conversation,
            user_text: "Hello".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            session_id: "test-session".to_string(),
            layer: Layer::L3,
        }
    }

    #[test]
    fn hs_local_only_policy_is_resolved_before_layered_reasoning() {
        let packet = RuntimeHSPacket {
            selected_policies: vec![crate::agent::SelectedPolicyRef {
                policy_id: "policy.local-only".into(),
                reason: "sensitive route".into(),
                route: Some(crate::agent::ModelRoutePolicy::LocalOnly),
                digest: "policy-digest".into(),
            }],
            selected_heuristics: Vec::new(),
            guidance_refs: Vec::new(),
            estimated_tokens: 0,
            audit: crate::agent::HSSelectionAudit {
                agent_task_id: Some("task-local".into()),
                agent_run_id: None,
                input_digest: "outer-policy-decision".into(),
                selected_policy_ids: vec!["policy.local-only".into()],
                selected_heuristic_ids: Vec::new(),
                selected_guidance_ids: Vec::new(),
                selected_guidance_refs: Vec::new(),
                excluded_assets: Vec::new(),
                estimated_tokens: 0,
                token_budget: 128,
            },
            provider_authorization: ProviderPolicyAuthorization::local_only_fail_closed(
                ProviderLocalOnlyReason::TestFixture,
            ),
        };

        let (authorization, refs) = layered_provider_policy_context(Some(&packet), "session-local");

        assert_eq!(
            authorization.data_route(),
            crate::llm::ProviderDataRoute::LocalOnly
        );
        assert!(refs.iter().any(|reference| {
            reference.kind() == crate::llm::ProviderPolicyProvenanceKind::HsPolicy
                && reference.reference_id() == "policy.local-only"
        }));
        assert!(refs.iter().any(|reference| {
            reference.kind() == crate::llm::ProviderPolicyProvenanceKind::FailClosedRouteDecision
                && reference.reference_id() == authorization.decision_id()
        }));
    }

    #[tokio::test]
    async fn test_execute_task_success() {
        let life_model = create_test_life_model();
        let scheduler = InferenceScheduler::new(
            "llama3.2".to_string(),
            true,
            "openai".to_string(),
            "https://api.openai.com/v1".to_string(),
            "".to_string(),
            "gpt-4".to_string(),
            "text-embedding-3-small".to_string(),
            false,
        )
        .with_scripted_generation_response("scripted layered response: 下一步继续确认目标。");
        let config = AgentRuntimeConfig::default();
        let runtime = AgentRuntime::with_config(life_model, scheduler, config);
        let task = create_test_task();

        let result = runtime
            .execute_task(
                &task,
                &create_test_life_model(),
                "",
                None,
                vec![],
                PrivacyEngine::new(),
            )
            .await;

        let output = result.expect("L3 layered runtime should use scripted raw generation");
        assert!(!output.run_id.trim().is_empty());
        assert!(!output.final_messages.is_empty());
        assert!(output
            .reasoning_trace
            .output
            .as_deref()
            .is_some_and(|output| output.contains("scripted layered response")));
        assert!(!output.plan_steps.is_empty());
    }

    #[tokio::test]
    async fn test_generate_direct_success() {
        let life_model = create_test_life_model();
        let scheduler = InferenceScheduler::new(
            "llama3.2".to_string(),
            true,
            "openai".to_string(),
            "https://api.openai.com/v1".to_string(),
            "".to_string(),
            "gpt-4".to_string(),
            "text-embedding-3-small".to_string(),
            false,
        );
        let config = AgentRuntimeConfig::default();
        let runtime = AgentRuntime::with_config(life_model, scheduler, config);
        let task = create_test_task();

        let result = runtime
            .generate_direct(
                &task,
                &create_test_life_model(),
                "",
                None,
                vec![],
                PrivacyEngine::new(),
            )
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.final_messages.is_empty());
        assert_eq!(output.reasoning_trace, ReasoningTrace::default());
        assert!(output.suggested_tools.is_empty());
        assert!(output.plan_steps.is_empty());
    }

    #[tokio::test]
    async fn test_layer_selection_l1_uses_direct() {
        let life_model = create_test_life_model();
        let scheduler = InferenceScheduler::new(
            "llama3.2".to_string(),
            true,
            "openai".to_string(),
            "https://api.openai.com/v1".to_string(),
            "".to_string(),
            "gpt-4".to_string(),
            "text-embedding-3-small".to_string(),
            false,
        );
        let config = AgentRuntimeConfig::default();
        let runtime = AgentRuntime::with_config(life_model, scheduler, config);

        let mut task = create_test_task();
        task.layer = Layer::L1;

        let result = runtime
            .execute_task(
                &task,
                &create_test_life_model(),
                "",
                None,
                vec![],
                PrivacyEngine::new(),
            )
            .await;

        // L1 should use DirectReasoner which doesn't need API calls
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_error_display() {
        let err = AgentRuntimeError::ContextAssembly("test error".to_string());
        assert_eq!(format!("{}", err), "Context assembly failed: test error");

        let err = AgentRuntimeError::StrategyNotFound("layered".to_string());
        assert_eq!(format!("{}", err), "Strategy not found: layered");

        let err = AgentRuntimeError::Reasoning(ReasoningError {
            phase: "meaning".to_string(),
            message: "timeout".to_string(),
            recoverable: true,
        });
        assert!(format!("{}", err).contains("Reasoning failed"));
    }
}
