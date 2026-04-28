use crate::agent::context_assembler::AssembleOutput;
use crate::agent::reasoning::{
    ReasoningConfig, ReasoningError, ReasoningInput, ReasoningOutput, ReasoningStrategy, ReasoningTrace,
};

/// Direct reasoning strategy: passes context through without layered reasoning.
/// Used as a fallback or for low-latency scenarios.
pub struct DirectReasoner {
    config: ReasoningConfig,
}

impl DirectReasoner {
    pub fn new() -> Self {
        Self {
            config: ReasoningConfig {
                meaning_timeout_ms: 0,
                strategy_timeout_ms: 0,
                generation_timeout_ms: 30000,
                max_retries: 1,
            },
        }
    }

    pub fn with_config(config: ReasoningConfig) -> Self {
        Self { config }
    }
}

impl Default for DirectReasoner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ReasoningStrategy for DirectReasoner {
    fn name(&self) -> &'static str {
        "direct"
    }

    async fn reason(
        &self,
        _input: &ReasoningInput,
        context: &AssembleOutput,
        _run_id: &str,
    ) -> Result<ReasoningOutput, ReasoningError> {
        Ok(ReasoningOutput {
            system_prompt: String::new(),
            trace: ReasoningTrace::default(),
            suggested_tools: vec![],
            plan_steps: vec![],
        })
    }

    fn config(&self) -> ReasoningConfig {
        self.config.clone()
    }
}
