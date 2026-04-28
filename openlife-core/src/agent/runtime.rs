use crate::agent::context_assembler::{AssembleInput, CompositeAssembler, ContextAssembler};
use crate::agent::reasoning::{
    DirectReasoner, LayeredReasoner, ReasoningConfig, ReasoningError, ReasoningInput,
    ReasoningStrategy, ReasoningTrace,
};
use crate::agent::types::{AgentTask, AgentTaskKind};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::scheduler::InferenceScheduler;
// use chrono::Utc;
use std::collections::HashMap;
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
    pub fn new(life_model: LifeModel, scheduler: InferenceScheduler, app_config: &crate::config::AppConfig) -> Self {
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
        // 1. Build AssembleInput
        let input = AssembleInput {
            session_id: task.session_id.clone(),
            messages: task.messages.clone(),
            life_model: life_model.clone(),
            tools_prompt: tools_prompt.to_string(),
            privacy_engine,
            memory_context,
            memory_hits,
            memory_retrieval_time_ms: 0,
        };

        // 2. Assemble context
        let context = self.context_assembler.assemble(&input)
            .map_err(|e| AgentRuntimeError::ContextAssembly(e.to_string()))?;

        // 3. Select reasoning strategy based on layer
        let strategy = if task.layer == Layer::L3 {
            self.reasoning_strategies.get("layered")
        } else {
            self.reasoning_strategies.get("direct")
        }.ok_or_else(|| AgentRuntimeError::StrategyNotFound(
            if task.layer == Layer::L3 { "layered" } else { "direct" }.to_string()
        ))?;

        // 4. Build reasoning input
        let reasoning_input = ReasoningInput {
            task_kind: task.kind,
            user_text: task.user_text.clone(),
            session_id: task.session_id.clone(),
        };

        // 5. Execute reasoning
        let run_id = Uuid::new_v4().to_string();
        let reasoning_output = strategy.reason(&reasoning_input, &context, &run_id)
            .await
            .map_err(|e| AgentRuntimeError::Reasoning(e))?;

        // 6. Build final messages with system prompt
        let mut final_messages = context.desensitized_messages.clone();
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
        let input = AssembleInput {
            session_id: task.session_id.clone(),
            messages: task.messages.clone(),
            life_model: life_model.clone(),
            tools_prompt: tools_prompt.to_string(),
            privacy_engine,
            memory_context,
            memory_hits,
            memory_retrieval_time_ms: 0,
        };

        let context = self.context_assembler.assemble(&input)
            .map_err(|e| AgentRuntimeError::ContextAssembly(e.to_string()))?;

        Ok(AgentRuntimeOutput {
            final_messages: context.desensitized_messages.clone(),
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
