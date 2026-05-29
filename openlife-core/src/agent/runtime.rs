use crate::agent::context_assembler::{AssembleInput, CompositeAssembler, ContextAssembler};
use crate::agent::reasoning::{
    DirectReasoner, LayeredReasoner, ReasoningConfig, ReasoningError, ReasoningInput,
    ReasoningStrategy, ReasoningTrace,
};
use crate::agent::types::AgentTask;
use crate::agent::{
    behavior_checks_for_packet, HSBehaviorCheckSummary, HSSelectionAudit, RuntimeHSPacket,
};
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
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
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
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_task_inner(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
        hs_packet: Option<RuntimeHSPacket>,
    ) -> Result<AgentRuntimeOutput, AgentRuntimeError> {
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
        let hs_selection_audit = hs_packet.as_ref().map(|packet| packet.audit.clone());
        let hs_behavior_checks = hs_packet
            .as_ref()
            .map(behavior_checks_for_packet)
            .unwrap_or_default();
        if let Some(prompt) = hs_packet.as_ref().and_then(build_hs_runtime_prompt) {
            final_messages.insert(
                0,
                ChatMessage {
                    role: "system".to_string(),
                    content: prompt,
                },
            );
        }

        Ok(AgentRuntimeOutput {
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
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
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
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn generate_direct_inner(
        &self,
        task: &AgentTask,
        life_model: &LifeModel,
        tools_prompt: &str,
        memory_context: Option<String>,
        memory_hits: Vec<crate::agent::context_assembler::MemoryHit>,
        privacy_engine: crate::privacy::PrivacyEngine,
        hs_packet: Option<RuntimeHSPacket>,
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

        let mut final_messages = context.desensitized_messages.to_vec();
        let hs_selection_audit = hs_packet.as_ref().map(|packet| packet.audit.clone());
        let hs_behavior_checks = hs_packet
            .as_ref()
            .map(behavior_checks_for_packet)
            .unwrap_or_default();
        if let Some(prompt) = hs_packet.as_ref().and_then(build_hs_runtime_prompt) {
            final_messages.insert(
                0,
                ChatMessage {
                    role: "system".to_string(),
                    content: prompt,
                },
            );
        }

        Ok(AgentRuntimeOutput {
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
    pub final_messages: Vec<ChatMessage>,
    pub reasoning_trace: ReasoningTrace,
    pub suggested_tools: Vec<String>,
    pub plan_steps: Vec<String>,
    pub context_summary: crate::agent::types::ContextSummary,
    pub hs_selection_audit: Option<HSSelectionAudit>,
    pub hs_behavior_checks: Vec<HSBehaviorCheckSummary>,
}

fn build_hs_runtime_prompt(packet: &RuntimeHSPacket) -> Option<String> {
    if packet.selected_heuristics.is_empty() {
        return None;
    }
    let guidance = packet
        .selected_heuristics
        .iter()
        .take(4)
        .map(|heuristic| format!("- [{}] {}", heuristic.heuristic_id, heuristic.guidance))
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "Selected personal collaboration guidance for this run:\n{}",
        guidance
    ))
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
}
