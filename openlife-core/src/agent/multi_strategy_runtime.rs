use crate::agent::governor::{GovernanceDecisionKind, LifeModelGovernor};
use crate::agent::plan_execute::{PlanExecuteInput, PlanExecuteService, PlanExecutionOutput};
use crate::agent::runtime::{AgentRuntime, AgentRuntimeError};
use crate::agent::runtime_contract::{RuntimeInput, RuntimeOutput};
use crate::agent::strategy::{
    RuntimeStrategyKind, StrategySelection, StrategySelectionInput, StrategySelector,
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
    runtime: AgentRuntime,
    selector: StrategySelector,
    governor: LifeModelGovernor,
    plan_execute: PlanExecuteService,
}

impl MultiStrategyRuntime {
    pub fn new(runtime: AgentRuntime) -> Self {
        Self {
            runtime,
            selector: StrategySelector,
            governor: LifeModelGovernor,
            plan_execute: PlanExecuteService,
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

        match selection.kind {
            RuntimeStrategyKind::ReAct => {
                let runtime_output = self
                    .runtime
                    .execute_runtime_input(input.runtime_input)
                    .await?;
                Ok(MultiStrategyRuntimeOutput {
                    selection,
                    payload: MultiStrategyRuntimePayload::ReAct(runtime_output),
                    warnings,
                })
            }
            RuntimeStrategyKind::PlanExecute => {
                let max_steps = input.runtime_input.execution_budget.max_steps as usize;
                let plan_output = self.plan_execute.execute_plan(
                    PlanExecuteInput::from_runtime_input(
                        input.runtime_input,
                        metadata_safe_objective(&selection),
                        max_steps,
                    ),
                    &self.governor,
                );
                warnings.extend(plan_output.warnings.iter().cloned());

                Ok(MultiStrategyRuntimeOutput {
                    selection,
                    payload: MultiStrategyRuntimePayload::PlanExecute(plan_output),
                    warnings,
                })
            }
        }
    }
}

fn metadata_safe_objective(selection: &StrategySelection) -> String {
    let selected_strategy = selection
        .metadata_safe_summary
        .get("selectedStrategyKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let task_kind = selection
        .metadata_safe_summary
        .get("taskKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let reason_code = selection
        .metadata_safe_summary
        .get("reasonCode")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");

    format!(
        "selected_strategy={} task_kind={} reason_code={}",
        selected_strategy, task_kind, reason_code
    )
}
