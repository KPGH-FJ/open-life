use crate::agent::context_assembler::AssembleOutput;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Input to a reasoning strategy.
#[derive(Debug, Clone)]
pub struct ReasoningInput {
    pub task_kind: crate::agent::types::AgentTaskKind,
    pub user_text: String,
    pub session_id: String,
}

/// Output from a reasoning strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningOutput {
    /// System prompt for the final LLM call
    pub system_prompt: String,
    /// Trace of the reasoning process
    pub trace: ReasoningTrace,
    /// Tools suggested by the strategy
    pub suggested_tools: Vec<String>,
    /// Execution plan steps
    pub plan_steps: Vec<String>,
}

/// Trace of a reasoning process (replaces HermesTrace).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReasoningTrace {
    pub input: Option<String>,
    pub meaning_result: Option<serde_json::Value>,
    pub strategy_result: Option<serde_json::Value>,
    pub generation_result: Option<serde_json::Value>,
    pub output: Option<String>,
    pub errors: Vec<String>,
    #[serde(default)]
    pub tool_plan: Vec<String>,
    pub safety_check_result: Option<serde_json::Value>,
    #[serde(default)]
    pub layer_timings_ms: HashMap<String, u64>,
    #[serde(default)]
    pub stable_steps: Vec<String>,
}

impl ReasoningTrace {
    pub fn set_layer_result(&mut self, phase: ReasoningPhaseKind, value: serde_json::Value) {
        match phase {
            ReasoningPhaseKind::Meaning => self.meaning_result = Some(value),
            ReasoningPhaseKind::Strategy => self.strategy_result = Some(value),
            ReasoningPhaseKind::Generation => self.generation_result = Some(value),
        }
    }

    pub fn set_layer_error(&mut self, phase: ReasoningPhaseKind, error: String) {
        self.errors.push(format!("{:?}: {}", phase, error));
    }

    pub fn to_markdown(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref v) = self.meaning_result {
            if let Some(text) = v.as_str() {
                parts.push(format!("**Meaning**: {}", text));
            } else if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
                parts.push(format!("**Meaning**: {}", text));
            }
        }
        if let Some(ref v) = self.strategy_result {
            if let Some(text) = v.as_str() {
                parts.push(format!("**Strategy**: {}", text));
            } else if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
                parts.push(format!("**Strategy**: {}", text));
            }
        }
        if !self.tool_plan.is_empty() {
            parts.push(format!("**Tools**: {}", self.tool_plan.join(", ")));
        }
        if !self.errors.is_empty() {
            parts.push(format!("**Errors**: {}", self.errors.join("; ")));
        }
        parts.join("\n")
    }
}

/// Error from a reasoning strategy.
#[derive(Debug, Clone)]
pub struct ReasoningError {
    pub phase: String,
    pub message: String,
    pub recoverable: bool,
}

impl std::fmt::Display for ReasoningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {} (recoverable: {})", self.phase, self.message, self.recoverable)
    }
}

impl std::error::Error for ReasoningError {}

/// Configuration for a reasoning strategy.
#[derive(Debug, Clone)]
pub struct ReasoningConfig {
    pub meaning_timeout_ms: u64,
    pub strategy_timeout_ms: u64,
    pub generation_timeout_ms: u64,
    pub max_retries: u32,
}

impl Default for ReasoningConfig {
    fn default() -> Self {
        Self {
            meaning_timeout_ms: 5000,
            strategy_timeout_ms: 15000,
            generation_timeout_ms: 30000,
            max_retries: 1,
        }
    }
}

/// Kind of reasoning phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningPhaseKind {
    Meaning,
    Strategy,
    Generation,
}

/// Trait for reasoning strategies.
#[async_trait::async_trait]
pub trait ReasoningStrategy: Send + Sync {
    /// Strategy name
    fn name(&self) -> &'static str;

    /// Execute reasoning
    async fn reason(
        &self,
        input: &ReasoningInput,
        context: &AssembleOutput,
        run_id: &str,
    ) -> Result<ReasoningOutput, ReasoningError>;

    /// Get configuration
    fn config(&self) -> ReasoningConfig;
}

pub mod direct;
pub mod layered;

pub use direct::DirectReasoner;
pub use layered::LayeredReasoner;
