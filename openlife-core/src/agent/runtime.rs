use crate::agent::context_assembler::{
    AssembleInput, CompositeAssembler, ContextAssembler, ContextPolicy, GovernedAssembleOutput,
};
use crate::agent::reasoning::{
    DirectReasoner, LayeredReasoner, ReasoningConfig, ReasoningError, ReasoningInput,
    ReasoningStrategy, ReasoningTrace,
};
use crate::agent::types::{AgentSpec, AgentTask};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
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
    config: AgentRuntimeConfig,
}

impl AgentRuntime {
    pub fn new(
        life_model: LifeModel,
        scheduler: InferenceScheduler,
        app_config: &crate::config::AppConfig,
    ) -> Self {
        let mut strategies: HashMap<String, Box<dyn ReasoningStrategy>> = HashMap::new();

        // Register LayeredReasoner (default)
        let layered = LayeredReasoner::with_config(
            scheduler.clone(),
            life_model.clone(),
            ReasoningConfig {
                meaning_timeout_ms: app_config.reasoning.meaning_timeout_ms,
                strategy_timeout_ms: app_config.reasoning.strategy_timeout_ms,
                generation_timeout_ms: app_config.reasoning.generation_timeout_ms,
                max_retries: 1,
            },
        );
        strategies.insert("layered".to_string(), Box::new(layered));

        // Register DirectReasoner (fallback)
        let direct = DirectReasoner::new();
        strategies.insert("direct".to_string(), Box::new(direct));

        Self {
            context_assembler: CompositeAssembler::new()
                .with(Box::new(crate::agent::LifeModelAssembler))
                .with(Box::new(crate::agent::PrivacyAssembler))
                .with(Box::new(crate::agent::MemoryAssembler))
                .with(Box::new(crate::agent::ToolsAssembler)),
            reasoning_strategies: strategies,
            config: AgentRuntimeConfig {
                default_strategy: app_config.reasoning.default_strategy.clone(),
                meaning_timeout_ms: app_config.reasoning.meaning_timeout_ms,
                strategy_timeout_ms: app_config.reasoning.strategy_timeout_ms,
                generation_timeout_ms: app_config.reasoning.generation_timeout_ms,
            },
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

    /// Assemble context with governance policy applied.
    /// Filters input before delegation to the composite assembler.
    fn assemble_governed(
        &self,
        input: &mut AssembleInput,
        policy: &ContextPolicy,
    ) -> Result<(crate::agent::AssembleOutput, GovernedAssembleOutput), AgentRuntimeError> {
        let governed = policy.filter_input(input);
        let output = self
            .context_assembler
            .assemble(input)
            .map_err(|e| AgentRuntimeError::ContextAssembly(e.to_string()))?;
        Ok((output, governed))
    }

    /// Build a governed PromptStack for the given AgentSpec.
    pub fn prompt_stack_for_spec(
        spec: &crate::agent::types::AgentSpec,
        registry: &crate::agent::prompt_stack::PromptBlockRegistry,
    ) -> Result<crate::agent::prompt_stack::PromptStack, String> {
        crate::agent::prompt_stack::PromptStack::try_from_agentspec(
            &spec.prompt_block_ids,
            registry,
        )
    }

    /// Derive ContextPolicy from an AgentSpec's fields.
    pub fn context_policy_from_spec(spec: &AgentSpec) -> ContextPolicy {
        ContextPolicy {
            allow_lifemodel_summary: spec.can_access_lifemodel,
            allow_goals: spec.can_access_lifemodel,
            allow_state: spec.can_access_lifemodel,
            allow_memory: spec.can_access_memory_evidence,
            allow_session_summary: true,
            allow_tool_observations: true,
        }
    }

    /// Execute a task governed by an AgentSpec.
    ///
    /// Derives ContextPolicy and PromptStack from the AgentSpec before reasoning.
    /// Unknown prompt block ids fail before any model call.
    pub async fn execute_task_with_spec(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
        spec: &AgentSpec,
        prompt_registry: &crate::agent::prompt_stack::PromptBlockRegistry,
    ) -> Result<AgentRuntimeOutput, AgentRuntimeError> {
        // Assemble PromptStack from AgentSpec prompt block ids.
        // Unknown block ids fail before reasoning/model calls.
        let mut prompt_stack = if spec.prompt_block_ids.is_empty() {
            None
        } else {
            let stack = Self::prompt_stack_for_spec(spec, prompt_registry).map_err(|e| {
                AgentRuntimeError::ContextAssembly(format!("prompt stack error: {}", e))
            })?;
            Some(stack)
        };

        // Derive policy from AgentSpec
        let policy = Self::context_policy_from_spec(spec);

        let mut input = AssembleInput {
            session_id: task.session_id.clone(),
            messages: Arc::new(task.messages.clone()),
            life_model: Arc::new(life_model.clone()),
            tools_prompt: tools_prompt.to_string(),
            privacy_engine,
            memory_context,
            memory_hits,
            memory_retrieval_time_ms: 0,
        };

        let (context, _governed) = self.assemble_governed(&mut input, &policy)?;

        let strategy = if task.layer == Layer::L3 {
            self.reasoning_strategies.get("layered")
        } else {
            self.reasoning_strategies.get("direct")
        }
        .ok_or_else(|| {
            AgentRuntimeError::StrategyNotFound(
                if task.layer == Layer::L3 {
                    "layered"
                } else {
                    "direct"
                }
                .to_string(),
            )
        })?;

        let reasoning_input = ReasoningInput {
            task_kind: task.kind,
            user_text: task.user_text.clone(),
            session_id: task.session_id.clone(),
        };

        let run_id = Uuid::new_v4().to_string();
        let reasoning_output = strategy
            .reason(&reasoning_input, &context, &run_id)
            .await
            .map_err(AgentRuntimeError::Reasoning)?;

        let mut final_messages = context.desensitized_messages.to_vec();

        // Inject reasoning system prompt (task-specific instructions).
        // Inserted before PromptStack, so it will be pushed to index 1
        // when PromptStack is inserted at index 0 below.
        if !reasoning_output.system_prompt.is_empty() {
            final_messages.insert(
                0,
                ChatMessage {
                    role: "system".to_string(),
                    content: reasoning_output.system_prompt.clone(),
                },
            );
        }

        // Inject AgentSpec PromptStack at index 0 (foundational blocks).
        // This pushes the reasoning prompt (if any) to index 1, so task-specific
        // instructions remain closer to the conversation messages.
        // Raw prompt content is NOT written into events — only block IDs
        // and versions are traceable (via PromptStack::block_trace).
        if let Some(ref mut stack) = prompt_stack {
            let assembled = stack.assemble();
            if !assembled.is_empty() {
                final_messages.insert(
                    0,
                    ChatMessage {
                        role: "system".to_string(),
                        content: assembled,
                    },
                );
            }
        }

        Ok(AgentRuntimeOutput {
            final_messages,
            reasoning_trace: reasoning_output.trace,
            suggested_tools: reasoning_output.suggested_tools,
            plan_steps: reasoning_output.plan_steps,
            context_summary: context.context_summary,
        })
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
        // 1. Build AssembleInput
        let mut input = AssembleInput {
            session_id: task.session_id.clone(),
            messages: Arc::new(task.messages.clone()),
            life_model: Arc::new(life_model.clone()),
            tools_prompt: tools_prompt.to_string(),
            privacy_engine,
            memory_context,
            memory_hits,
            memory_retrieval_time_ms: 0,
        };

        // 2. Assemble context with governance policy.
        let policy = ContextPolicy::default();
        let (context, _governed) = self.assemble_governed(&mut input, &policy)?;

        // 3. Select reasoning strategy based on layer
        let strategy = if task.layer == Layer::L3 {
            self.reasoning_strategies.get("layered")
        } else {
            self.reasoning_strategies.get("direct")
        }
        .ok_or_else(|| {
            AgentRuntimeError::StrategyNotFound(
                if task.layer == Layer::L3 {
                    "layered"
                } else {
                    "direct"
                }
                .to_string(),
            )
        })?;

        // 4. Build reasoning input
        let reasoning_input = ReasoningInput {
            task_kind: task.kind,
            user_text: task.user_text.clone(),
            session_id: task.session_id.clone(),
        };

        // 5. Execute reasoning
        let run_id = Uuid::new_v4().to_string();
        let reasoning_output = strategy
            .reason(&reasoning_input, &context, &run_id)
            .await
            .map_err(AgentRuntimeError::Reasoning)?;

        // 6. Build final messages with system prompt
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

        Ok(AgentRuntimeOutput {
            final_messages,
            reasoning_trace: reasoning_output.trace,
            suggested_tools: reasoning_output.suggested_tools,
            plan_steps: reasoning_output.plan_steps,
            context_summary: context.context_summary,
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
        let mut input = AssembleInput {
            session_id: task.session_id.clone(),
            messages: Arc::new(task.messages.clone()),
            life_model: Arc::new(life_model.clone()),
            tools_prompt: tools_prompt.to_string(),
            privacy_engine,
            memory_context,
            memory_hits,
            memory_retrieval_time_ms: 0,
        };

        let policy = ContextPolicy::default();
        let (context, _governed) = self.assemble_governed(&mut input, &policy)?;

        Ok(AgentRuntimeOutput {
            final_messages: context.desensitized_messages.to_vec(),
            reasoning_trace: ReasoningTrace::default(),
            suggested_tools: vec![],
            plan_steps: vec![],
            context_summary: context.context_summary,
        })
    }
}

/// Output from AgentRuntime execution.
#[derive(Debug, Clone)]
pub struct AgentRuntimeOutput {
    pub final_messages: Vec<ChatMessage>,
    pub reasoning_trace: ReasoningTrace,
    pub suggested_tools: Vec<String>,
    pub plan_steps: Vec<String>,
    pub context_summary: crate::agent::types::ContextSummary,
}

/// Errors from AgentRuntime.
#[derive(Debug, Clone)]
pub enum AgentRuntimeError {
    ContextAssembly(String),
    StrategyNotFound(String),
    Reasoning(ReasoningError),
}

impl std::fmt::Display for AgentRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRuntimeError::ContextAssembly(e) => write!(f, "Context assembly failed: {}", e),
            AgentRuntimeError::StrategyNotFound(s) => write!(f, "Strategy not found: {}", s),
            AgentRuntimeError::Reasoning(e) => write!(f, "Reasoning failed: {}", e),
        }
    }
}

impl std::error::Error for AgentRuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{AgentTask, AgentTaskKind};
    use crate::layer_router::Layer;
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
            ..Default::default()
        }
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
        );
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

        // L3 task uses layered strategy, which will fail because no API key
        // But context assembly should succeed
        assert!(result.is_err());
        match result.unwrap_err() {
            AgentRuntimeError::Reasoning(_) => {}
            other => panic!("Expected Reasoning error, got: {:?}", other),
        }
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

    // ── P7-3: AgentSpec-driven runtime tests ────────────────────────────

    use crate::agent::types::AgentSpec;

    #[test]
    fn test_execute_task_with_spec_uses_prompt_block_ids() {
        let registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();
        let spec = AgentSpec::default_main_spec();
        // main.default has no prompt_block_ids by default, give it some
        let mut spec = spec;
        spec.prompt_block_ids = vec!["base_system".to_string(), "privacy_rule".to_string()];

        let stack =
            AgentRuntime::prompt_stack_for_spec(&spec, &registry).unwrap();
        assert_eq!(stack.blocks.len(), 2);
        let ids: Vec<&str> = stack.blocks.iter().map(|b| b.id.as_str()).collect();
        assert!(ids.contains(&"base_system"));
        assert!(ids.contains(&"privacy_rule"));
    }

    #[test]
    fn test_unknown_prompt_block_id_fails_before_reasoning() {
        let registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();
        let mut spec = AgentSpec::default_main_spec();
        spec.prompt_block_ids = vec!["nonexistent_block".to_string()];

        let err =
            AgentRuntime::prompt_stack_for_spec(&spec, &registry).unwrap_err();
        assert!(err.contains("unknown prompt block"));
        assert!(err.contains("nonexistent_block"));
    }

    #[test]
    fn test_spec_without_memory_access_excludes_memory() {
        let mut spec = AgentSpec::default_main_spec();
        spec.can_access_memory_evidence = false;

        let policy = AgentRuntime::context_policy_from_spec(&spec);
        assert!(!policy.allow_memory);
    }

    #[test]
    fn test_spec_without_lifemodel_access_excludes_lifemodel_summary() {
        let mut spec = AgentSpec::default_main_spec();
        spec.can_access_lifemodel = false;

        let policy = AgentRuntime::context_policy_from_spec(&spec);
        assert!(!policy.allow_lifemodel_summary);
        assert!(!policy.allow_goals);
        assert!(!policy.allow_state);
    }

    #[test]
    fn test_default_main_spec_preserves_current_behavior() {
        let spec = AgentSpec::default_main_spec();
        let policy = AgentRuntime::context_policy_from_spec(&spec);
        let default_policy = ContextPolicy::default();

        assert_eq!(policy.allow_lifemodel_summary, default_policy.allow_lifemodel_summary);
        assert_eq!(policy.allow_goals, default_policy.allow_goals);
        assert_eq!(policy.allow_state, default_policy.allow_state);
        assert_eq!(policy.allow_memory, default_policy.allow_memory);
        assert_eq!(policy.allow_session_summary, default_policy.allow_session_summary);
        assert_eq!(policy.allow_tool_observations, default_policy.allow_tool_observations);
    }

    // ── P7 stabilization: PromptStack is used by runtime, not only validated ──

    #[tokio::test]
    async fn test_execute_task_with_spec_includes_prompt_stack_in_final_messages_l1() {
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
        let runtime = AgentRuntime::with_config(life_model, scheduler, AgentRuntimeConfig::default());
        let registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();

        let mut task = create_test_task();
        task.layer = crate::layer_router::Layer::L1; // uses DirectReasoner, no API needed

        let spec = AgentSpec::default_main_spec();
        let mut spec_with_blocks = spec.clone();
        spec_with_blocks.prompt_block_ids = vec!["base_system".to_string(), "privacy_rule".to_string()];

        let output = runtime
            .execute_task_with_spec(
                &task,
                &create_test_life_model(),
                "",
                None,
                vec![],
                PrivacyEngine::new(),
                &spec_with_blocks,
                &registry,
            )
            .await
            .unwrap();

        // PromptStack blocks should produce at least one system message
        let system_msgs: Vec<_> = output
            .final_messages
            .iter()
            .filter(|m| m.role == "system")
            .collect();
        assert!(
            !system_msgs.is_empty(),
            "PromptStack with 2 blocks should produce system messages"
        );
        // Verify the system message content is non-empty (blocks were assembled)
        let assembled_content: String = system_msgs.iter().map(|m| m.content.as_str()).collect();
        assert!(!assembled_content.is_empty(), "assembled prompt content should not be empty");
    }

    #[tokio::test]
    async fn test_empty_prompt_block_ids_preserves_previous_behavior() {
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
        let runtime = AgentRuntime::with_config(life_model, scheduler, AgentRuntimeConfig::default());
        let registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();

        let mut task = create_test_task();
        task.layer = crate::layer_router::Layer::L1;

        // main.default has empty prompt_block_ids by default
        let spec = AgentSpec::default_main_spec();

        let output = runtime
            .execute_task_with_spec(
                &task,
                &create_test_life_model(),
                "",
                None,
                vec![],
                PrivacyEngine::new(),
                &spec,
                &registry,
            )
            .await
            .unwrap();

        // No extra system message from AgentSpec when prompt_block_ids is empty.
        // With L1 DirectReasoner (no system_prompt), only desensitized_messages exist.
        assert!(!output.final_messages.is_empty());
        let system_count = output.final_messages.iter().filter(|m| m.role == "system").count();
        assert_eq!(system_count, 0, "no system message should be injected for empty prompt_block_ids");
    }

    #[test]
    fn test_block_trace_does_not_expose_raw_prompt_content() {
        let registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();
        let mut spec = AgentSpec::default_main_spec();
        spec.prompt_block_ids = vec!["base_system".to_string(), "privacy_rule".to_string()];

        let stack = AgentRuntime::prompt_stack_for_spec(&spec, &registry).unwrap();
        let trace = stack.block_trace();
        assert_eq!(trace.len(), 2);

        // Trace entries contain metadata only — no raw prompt content
        for entry in &trace {
            assert!(!entry.id.is_empty());
            assert!(!entry.version.is_empty());
            // BlockTraceEntry has no `content` field — confirmed by type definition
        }

        // Verify a trace entry serializes without raw prompt content
        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("base_system"));
        assert!(json.contains("privacy_rule"));
        assert!(!json.contains("You are OpenLife")); // raw prompt content must NOT appear
    }
}
