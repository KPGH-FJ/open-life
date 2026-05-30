use crate::agent::governor::LifeModelGovernor;
use crate::agent::plan_execute::{PlanExecuteInput, PlanExecuteService, PlanExecutionOutput};
use crate::agent::runtime::{AgentRuntime, AgentRuntimeError};
use crate::agent::runtime_contract::{RuntimeInput, RuntimeOutput};
use crate::agent::strategy::{RuntimeStrategyKind, StrategySelection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RuntimeStrategyInput {
    pub runtime_input: RuntimeInput,
    pub selection: StrategySelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStrategyPayloadKind {
    ReAct,
    PlanExecute,
}

impl RuntimeStrategyPayloadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RuntimeStrategyPayloadKind::ReAct => "react",
            RuntimeStrategyPayloadKind::PlanExecute => "plan_execute",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "output")]
pub enum RuntimeStrategyPayload {
    ReAct(RuntimeOutput),
    PlanExecute(PlanExecutionOutput),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStrategyOutput {
    pub payload: RuntimeStrategyPayload,
    pub metadata_safe_summary: Value,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[async_trait::async_trait]
pub trait RuntimeStrategy: Send + Sync {
    fn kind(&self) -> RuntimeStrategyKind;
    fn metadata_safe_id(&self) -> &'static str;
    fn metadata_safe_name(&self) -> &'static str;
    fn payload_kind(&self) -> RuntimeStrategyPayloadKind;

    async fn execute(
        &self,
        input: RuntimeStrategyInput,
    ) -> Result<RuntimeStrategyOutput, AgentRuntimeError>;
}

#[derive(Default)]
pub struct RuntimeStrategyRegistry {
    strategies: HashMap<RuntimeStrategyKind, Box<dyn RuntimeStrategy>>,
}

impl RuntimeStrategyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_strategy(mut self, strategy: Box<dyn RuntimeStrategy>) -> Self {
        self.register(strategy);
        self
    }

    pub fn register(&mut self, strategy: Box<dyn RuntimeStrategy>) {
        self.strategies.insert(strategy.kind(), strategy);
    }

    pub fn get(&self, kind: RuntimeStrategyKind) -> Option<&dyn RuntimeStrategy> {
        self.strategies.get(&kind).map(Box::as_ref)
    }
}

pub struct ReActRuntimeStrategy {
    runtime: AgentRuntime,
}

impl ReActRuntimeStrategy {
    pub fn new(runtime: AgentRuntime) -> Self {
        Self { runtime }
    }

    pub fn map_output(&self, output: RuntimeOutput) -> RuntimeStrategyPayload {
        RuntimeStrategyPayload::ReAct(output)
    }
}

#[async_trait::async_trait]
impl RuntimeStrategy for ReActRuntimeStrategy {
    fn kind(&self) -> RuntimeStrategyKind {
        RuntimeStrategyKind::ReAct
    }

    fn metadata_safe_id(&self) -> &'static str {
        "react"
    }

    fn metadata_safe_name(&self) -> &'static str {
        "ReAct"
    }

    fn payload_kind(&self) -> RuntimeStrategyPayloadKind {
        RuntimeStrategyPayloadKind::ReAct
    }

    async fn execute(
        &self,
        input: RuntimeStrategyInput,
    ) -> Result<RuntimeStrategyOutput, AgentRuntimeError> {
        let runtime_output = self
            .runtime
            .execute_runtime_input(input.runtime_input)
            .await?;

        Ok(RuntimeStrategyOutput {
            payload: self.map_output(runtime_output),
            metadata_safe_summary: runtime_strategy_summary(self),
            warnings: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlanExecuteRuntimeStrategy {
    service: PlanExecuteService,
    governor: LifeModelGovernor,
}

impl PlanExecuteRuntimeStrategy {
    pub fn new(service: PlanExecuteService, governor: LifeModelGovernor) -> Self {
        Self { service, governor }
    }

    pub fn map_output(&self, output: PlanExecutionOutput) -> RuntimeStrategyPayload {
        RuntimeStrategyPayload::PlanExecute(output)
    }
}

#[async_trait::async_trait]
impl RuntimeStrategy for PlanExecuteRuntimeStrategy {
    fn kind(&self) -> RuntimeStrategyKind {
        RuntimeStrategyKind::PlanExecute
    }

    fn metadata_safe_id(&self) -> &'static str {
        "plan_execute"
    }

    fn metadata_safe_name(&self) -> &'static str {
        "PlanExecute"
    }

    fn payload_kind(&self) -> RuntimeStrategyPayloadKind {
        RuntimeStrategyPayloadKind::PlanExecute
    }

    async fn execute(
        &self,
        input: RuntimeStrategyInput,
    ) -> Result<RuntimeStrategyOutput, AgentRuntimeError> {
        let max_steps = input.runtime_input.execution_budget.max_steps as usize;
        let plan_output = self.service.execute_plan(
            PlanExecuteInput::from_runtime_input(
                input.runtime_input,
                metadata_safe_objective(&input.selection),
                max_steps,
            ),
            &self.governor,
        );
        let warnings = plan_output.warnings.clone();

        Ok(RuntimeStrategyOutput {
            payload: self.map_output(plan_output),
            metadata_safe_summary: runtime_strategy_summary(self),
            warnings,
        })
    }
}

fn runtime_strategy_summary(strategy: &dyn RuntimeStrategy) -> Value {
    json!({
        "strategyId": strategy.metadata_safe_id(),
        "strategyName": strategy.metadata_safe_name(),
        "strategyKind": strategy_kind_str(strategy.kind()),
        "payloadKind": strategy.payload_kind().as_str(),
    })
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

fn strategy_kind_str(kind: RuntimeStrategyKind) -> &'static str {
    match kind {
        RuntimeStrategyKind::ReAct => "react",
        RuntimeStrategyKind::PlanExecute => "plan_execute",
    }
}
