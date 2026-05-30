use crate::agent::governor::GovernanceDecisionKind;
use crate::agent::plan_execute::PlanExecutionOutput;
use crate::agent::runtime::{AgentRuntime, AgentRuntimeError};
use crate::agent::runtime_contract::{RuntimeInput, RuntimeOutput};
use crate::agent::strategy::{
    RuntimeStrategyKind, StrategySelection, StrategySelectionInput, StrategySelector,
};
use crate::agent::strategy_runtime::{
    PlanExecuteRuntimeStrategy, ReActRuntimeStrategy, RuntimeStrategyInput, RuntimeStrategyPayload,
    RuntimeStrategyRegistry,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct MultiStrategyRuntimeInput {
    pub runtime_input: RuntimeInput,
    pub allow_planning: bool,
    pub local_model_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "output")]
pub enum MultiStrategyRuntimePayload {
    ReAct(RuntimeOutput),
    PlanExecute(PlanExecutionOutput),
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiStrategyRuntimeOutput {
    pub selection: StrategySelection,
    pub payload: MultiStrategyRuntimePayload,
    #[serde(default)]
    pub warnings: Vec<String>,
}

pub struct MultiStrategyRuntime {
    selector: StrategySelector,
    strategies: RuntimeStrategyRegistry,
}

impl MultiStrategyRuntime {
    pub fn new(runtime: AgentRuntime) -> Self {
        let strategies = RuntimeStrategyRegistry::new()
            .with_strategy(Box::new(ReActRuntimeStrategy::new(runtime)))
            .with_strategy(Box::new(PlanExecuteRuntimeStrategy::default()));

        Self::with_strategy_registry(StrategySelector, strategies)
    }

    pub(crate) fn with_strategy_registry(
        selector: StrategySelector,
        strategies: RuntimeStrategyRegistry,
    ) -> Self {
        Self {
            selector,
            strategies,
        }
    }

    pub async fn execute(
        &self,
        input: MultiStrategyRuntimeInput,
    ) -> Result<MultiStrategyRuntimeOutput, AgentRuntimeError> {
        let selection = self.selector.select(StrategySelectionInput {
            runtime_input: input.runtime_input.clone(),
            allow_planning: input.allow_planning,
            local_model_available: input.local_model_available,
        });
        let mut warnings = selection.warnings.clone();

        if selection
            .governance_decision
            .as_ref()
            .is_some_and(|decision| decision.kind == GovernanceDecisionKind::Block)
        {
            return Ok(MultiStrategyRuntimeOutput {
                selection,
                payload: MultiStrategyRuntimePayload::Blocked,
                warnings,
            });
        }

        let strategy = self.strategies.get(selection.kind).ok_or_else(|| {
            AgentRuntimeError::StrategyNotFound(format!(
                "runtime strategy {}",
                strategy_kind_str(selection.kind)
            ))
        })?;
        let strategy_output = strategy
            .execute(RuntimeStrategyInput {
                runtime_input: input.runtime_input,
                selection: selection.clone(),
            })
            .await?;
        warnings.extend(strategy_output.warnings.iter().cloned());

        Ok(MultiStrategyRuntimeOutput {
            selection,
            payload: MultiStrategyRuntimePayload::from(strategy_output.payload),
            warnings,
        })
    }
}

impl From<RuntimeStrategyPayload> for MultiStrategyRuntimePayload {
    fn from(payload: RuntimeStrategyPayload) -> Self {
        match payload {
            RuntimeStrategyPayload::ReAct(output) => MultiStrategyRuntimePayload::ReAct(output),
            RuntimeStrategyPayload::PlanExecute(output) => {
                MultiStrategyRuntimePayload::PlanExecute(output)
            }
        }
    }
}

fn strategy_kind_str(kind: RuntimeStrategyKind) -> &'static str {
    match kind {
        RuntimeStrategyKind::ReAct => "react",
        RuntimeStrategyKind::PlanExecute => "plan_execute",
    }
}
