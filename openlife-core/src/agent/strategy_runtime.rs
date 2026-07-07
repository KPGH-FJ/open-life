use crate::agent::governor::LifeModelGovernor;
use crate::agent::plan_execute::{PlanExecuteInput, PlanExecuteService, PlanExecutionOutput};
use crate::agent::runtime::{AgentRuntime, AgentRuntimeError};
use crate::agent::runtime_contract::{RuntimeInput, RuntimeOutput};
use crate::agent::runtime_strategy_contract::{RuntimeStrategyKind, StrategySelection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

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
    Blocked,
}

impl RuntimeStrategyPayloadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RuntimeStrategyPayloadKind::ReAct => "react",
            RuntimeStrategyPayloadKind::PlanExecute => "plan_execute",
            RuntimeStrategyPayloadKind::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStrategySideEffectBudget {
    pub runtime_calls: u32,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub store_writes: u32,
    pub proposal_writes: u32,
    pub memory_writes: u32,
    pub life_model_writes: u32,
    pub mcp_audit_writes: u32,
    pub external_writes: u32,
}

impl RuntimeStrategySideEffectBudget {
    pub fn zero() -> Self {
        Self {
            runtime_calls: 0,
            model_calls: 0,
            tool_calls: 0,
            store_writes: 0,
            proposal_writes: 0,
            memory_writes: 0,
            life_model_writes: 0,
            mcp_audit_writes: 0,
            external_writes: 0,
        }
    }

    pub fn react_preview_budget() -> Self {
        Self {
            runtime_calls: 1,
            model_calls: 1,
            tool_calls: 0,
            ..Self::zero()
        }
    }

    pub fn plan_execute_budget() -> Self {
        Self {
            runtime_calls: 1,
            model_calls: 0,
            tool_calls: 0,
            ..Self::zero()
        }
    }

    fn claims_business_writes_or_external_side_effects(&self) -> bool {
        self.store_writes > 0
            || self.proposal_writes > 0
            || self.memory_writes > 0
            || self.life_model_writes > 0
            || self.mcp_audit_writes > 0
            || self.external_writes > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStrategyDescriptor {
    pub strategy_kind: RuntimeStrategyKind,
    pub metadata_safe_id: String,
    pub metadata_safe_name: String,
    pub payload_kind: RuntimeStrategyPayloadKind,
    pub capability_ids: Vec<String>,
    pub supported_task_categories: Vec<String>,
    pub write_policy: String,
    pub side_effect_budget: RuntimeStrategySideEffectBudget,
    pub proposal_first_required: bool,
    pub metadata_safe_trace_supported: bool,
    pub default_chat_migration_permission: bool,
    pub metadata_safe: bool,
    pub executable: bool,
}

impl RuntimeStrategyDescriptor {
    pub fn executable(
        strategy_kind: RuntimeStrategyKind,
        metadata_safe_id: impl Into<String>,
        metadata_safe_name: impl Into<String>,
        payload_kind: RuntimeStrategyPayloadKind,
    ) -> Self {
        let (
            capability_ids,
            supported_task_categories,
            write_policy,
            side_effect_budget,
            proposal_first_required,
        ) = match strategy_kind {
            RuntimeStrategyKind::ReAct => (
                vec![
                    "conversation.response".into(),
                    "runtime.reason_observe".into(),
                    "metadata_safe_trace".into(),
                ],
                vec!["conversation".into(), "tool_or_observation".into()],
                "write_disabled_in_preview".into(),
                RuntimeStrategySideEffectBudget::react_preview_budget(),
                false,
            ),
            RuntimeStrategyKind::PlanExecute => (
                vec![
                    "planning.plan_execute".into(),
                    "proposal_first_steps".into(),
                    "metadata_safe_trace".into(),
                ],
                vec!["planning".into(), "write_like_governed_planning".into()],
                "proposal_first_for_write_like_steps".into(),
                RuntimeStrategySideEffectBudget::plan_execute_budget(),
                true,
            ),
        };

        Self {
            strategy_kind,
            metadata_safe_id: metadata_safe_id.into(),
            metadata_safe_name: metadata_safe_name.into(),
            payload_kind,
            capability_ids,
            supported_task_categories,
            write_policy,
            side_effect_budget,
            proposal_first_required,
            metadata_safe_trace_supported: true,
            default_chat_migration_permission: false,
            metadata_safe: true,
            executable: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStrategyDeclarativeDescriptor {
    pub strategy_kind: String,
    pub metadata_safe_id: String,
    pub metadata_safe_name: String,
    pub capability_ids: Vec<String>,
    pub supported_task_categories: Vec<String>,
    pub write_policy: String,
    pub side_effect_budget: RuntimeStrategySideEffectBudget,
    pub declarative_only: bool,
    pub executable: bool,
    pub default_chat_migration_permission: bool,
    pub metadata_safe: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStrategyRegistryReadinessReport {
    pub report_kind: String,
    pub ready: bool,
    pub metadata_safe: bool,
    pub executable_strategy_count: usize,
    pub executable_descriptors: Vec<RuntimeStrategyDescriptor>,
    pub future_strategy_descriptors: Vec<RuntimeStrategyDeclarativeDescriptor>,
    pub required_strategy_kinds: Vec<String>,
    pub blocking_reasons: Vec<String>,
    pub default_chat_unchanged: bool,
    pub migration_permission: bool,
    pub no_runtime_model_tool_execution: bool,
    pub no_business_writes: bool,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiStrategyRuntimeMaturityReport {
    pub report_kind: String,
    pub maturity_ready: bool,
    pub registry_readiness: RuntimeStrategyRegistryReadinessReport,
    pub executable_strategies: Vec<RuntimeStrategyDescriptor>,
    pub future_strategy_descriptors: Vec<RuntimeStrategyDeclarativeDescriptor>,
    pub default_chat_unchanged: bool,
    pub migration_permission: bool,
    pub no_runtime_model_tool_execution: bool,
    pub no_business_writes: bool,
    pub status_command_side_effect_budget: RuntimeStrategySideEffectBudget,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe: bool,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStrategyExecutionReport {
    pub report_kind: String,
    pub runtime_kind: String,
    pub selected_strategy_kind: RuntimeStrategyKind,
    pub payload_kind: RuntimeStrategyPayloadKind,
    pub strategy_descriptor_id: String,
    pub strategy_descriptor_name: String,
    pub strategy_capability_ids: Vec<String>,
    pub registry_ready: bool,
    pub selection_reason_code: String,
    pub governance_decision_kind: String,
    pub blocked: bool,
    pub warning_count: usize,
    pub side_effect_budget: RuntimeStrategySideEffectBudget,
    pub default_chat_unchanged: bool,
    pub metadata_safe: bool,
    pub strategy_output_summary: Value,
    #[serde(default)]
    pub registry_blocking_reasons: Vec<String>,
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

    fn descriptor(&self) -> RuntimeStrategyDescriptor {
        RuntimeStrategyDescriptor::executable(
            self.kind(),
            self.metadata_safe_id(),
            self.metadata_safe_name(),
            self.payload_kind(),
        )
    }

    async fn execute(
        &self,
        input: RuntimeStrategyInput,
    ) -> Result<RuntimeStrategyOutput, AgentRuntimeError>;
}

#[derive(Default)]
pub struct RuntimeStrategyRegistry {
    strategies: Vec<Box<dyn RuntimeStrategy>>,
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
        self.strategies.push(strategy);
    }

    pub fn get(&self, kind: RuntimeStrategyKind) -> Option<&dyn RuntimeStrategy> {
        self.strategies
            .iter()
            .find(|strategy| strategy.kind() == kind)
            .map(Box::as_ref)
    }

    pub fn executable_descriptors(&self) -> Vec<RuntimeStrategyDescriptor> {
        self.strategies
            .iter()
            .map(|strategy| strategy.descriptor())
            .collect()
    }

    pub fn future_strategy_descriptors() -> Vec<RuntimeStrategyDeclarativeDescriptor> {
        ["direct", "layered", "workflow", "proactive", "reflective"]
            .into_iter()
            .map(future_strategy_descriptor)
            .collect()
    }

    pub fn readiness_report(&self) -> RuntimeStrategyRegistryReadinessReport {
        self.readiness_report_for_descriptors(self.executable_descriptors())
    }

    pub fn readiness_report_for_descriptors(
        &self,
        executable_descriptors: Vec<RuntimeStrategyDescriptor>,
    ) -> RuntimeStrategyRegistryReadinessReport {
        let mut blocking_reasons = Vec::new();
        let mut seen_kinds = HashSet::new();
        let mut duplicate_kinds = HashSet::new();
        let mut seen_ids = HashSet::new();
        let mut duplicate_ids = HashSet::new();

        for descriptor in &executable_descriptors {
            let kind = descriptor.strategy_kind.as_str();
            if !seen_kinds.insert(kind) {
                duplicate_kinds.insert(kind);
            }
            if !seen_ids.insert(descriptor.metadata_safe_id.as_str()) {
                duplicate_ids.insert(descriptor.metadata_safe_id.as_str());
            }

            if descriptor.payload_kind != expected_payload_kind(descriptor.strategy_kind) {
                blocking_reasons.push(format!(
                    "descriptor_payload_kind_mismatch:{}",
                    descriptor.strategy_kind.as_str()
                ));
            }
            if descriptor
                .side_effect_budget
                .claims_business_writes_or_external_side_effects()
                && !descriptor.proposal_first_required
            {
                blocking_reasons.push(format!(
                    "writes_without_proposal_first:{}",
                    descriptor.strategy_kind.as_str()
                ));
            }
            if descriptor.default_chat_migration_permission {
                blocking_reasons.push(format!(
                    "default_chat_migration_permission_granted:{}",
                    descriptor.strategy_kind.as_str()
                ));
            }
            if !descriptor.metadata_safe
                || !metadata_safe_descriptor_text(
                    &serde_json::to_string(descriptor).unwrap_or_default(),
                )
            {
                blocking_reasons.push(format!(
                    "descriptor_not_metadata_safe:{}",
                    descriptor.strategy_kind.as_str()
                ));
            }
        }

        for kind in duplicate_kinds {
            blocking_reasons.push(format!("duplicate_strategy_kind:{kind}"));
        }
        for id in duplicate_ids {
            blocking_reasons.push(format!("duplicate_strategy_descriptor_id:{id}"));
        }
        for required in [RuntimeStrategyKind::ReAct, RuntimeStrategyKind::PlanExecute] {
            if !executable_descriptors
                .iter()
                .any(|descriptor| descriptor.strategy_kind == required)
            {
                blocking_reasons.push(format!("missing_required_strategy:{}", required.as_str()));
            }
        }

        blocking_reasons.sort();
        blocking_reasons.dedup();
        let ready = blocking_reasons.is_empty();
        let future_strategy_descriptors = Self::future_strategy_descriptors();
        let metadata_safe_summary = json!({
            "reportKind": "runtime_strategy_registry_readiness",
            "ready": ready,
            "executableStrategyCount": executable_descriptors.len(),
            "requiredStrategyKinds": ["react", "plan_execute"],
            "futureStrategyKinds": ["direct", "layered", "workflow", "proactive", "reflective"],
            "blockingReasonCount": blocking_reasons.len(),
            "metadataSafe": true,
            "defaultChatUnchanged": true,
            "migrationPermission": false,
            "runtimeModelToolExecuted": false,
            "businessWrites": false,
        });

        RuntimeStrategyRegistryReadinessReport {
            report_kind: "runtime_strategy_registry_readiness".into(),
            ready,
            metadata_safe: true,
            executable_strategy_count: executable_descriptors.len(),
            executable_descriptors,
            future_strategy_descriptors,
            required_strategy_kinds: vec!["react".into(), "plan_execute".into()],
            blocking_reasons,
            default_chat_unchanged: true,
            migration_permission: false,
            no_runtime_model_tool_execution: true,
            no_business_writes: true,
            metadata_safe_summary,
        }
    }

    pub fn fixed_executable_descriptors() -> Vec<RuntimeStrategyDescriptor> {
        vec![
            RuntimeStrategyDescriptor::executable(
                RuntimeStrategyKind::ReAct,
                "react",
                "ReAct",
                RuntimeStrategyPayloadKind::ReAct,
            ),
            RuntimeStrategyDescriptor::executable(
                RuntimeStrategyKind::PlanExecute,
                "plan_execute",
                "PlanExecute",
                RuntimeStrategyPayloadKind::PlanExecute,
            ),
        ]
    }

    pub fn fixed_readiness_report() -> RuntimeStrategyRegistryReadinessReport {
        RuntimeStrategyRegistry::new()
            .readiness_report_for_descriptors(Self::fixed_executable_descriptors())
    }

    pub fn maturity_report() -> MultiStrategyRuntimeMaturityReport {
        let registry_readiness = Self::fixed_readiness_report();
        let blocking_reasons = registry_readiness.blocking_reasons.clone();
        let maturity_ready = registry_readiness.ready;
        let metadata_safe_summary = json!({
            "reportKind": "multi_strategy_runtime_maturity",
            "maturityReady": maturity_ready,
            "registryReady": registry_readiness.ready,
            "executableStrategies": ["react", "plan_execute"],
            "futureStrategiesDeclarativeOnly": ["direct", "layered", "workflow", "proactive", "reflective"],
            "defaultChatUnchanged": true,
            "migrationPermission": false,
            "runtimeModelToolExecuted": false,
            "businessWrites": false,
            "metadataSafe": true,
        });

        MultiStrategyRuntimeMaturityReport {
            report_kind: "multi_strategy_runtime_maturity".into(),
            maturity_ready,
            executable_strategies: registry_readiness.executable_descriptors.clone(),
            future_strategy_descriptors: registry_readiness.future_strategy_descriptors.clone(),
            registry_readiness,
            default_chat_unchanged: true,
            migration_permission: false,
            no_runtime_model_tool_execution: true,
            no_business_writes: true,
            status_command_side_effect_budget: RuntimeStrategySideEffectBudget::zero(),
            blocking_reasons,
            metadata_safe: true,
            metadata_safe_summary,
        }
    }
}

fn future_strategy_descriptor(kind: &str) -> RuntimeStrategyDeclarativeDescriptor {
    let name = match kind {
        "direct" => "Direct",
        "layered" => "Layered",
        "workflow" => "Workflow",
        "proactive" => "Proactive",
        "reflective" => "Reflective",
        _ => "Future Strategy",
    };

    RuntimeStrategyDeclarativeDescriptor {
        strategy_kind: kind.into(),
        metadata_safe_id: format!("future_{kind}"),
        metadata_safe_name: name.into(),
        capability_ids: vec!["future_strategy_descriptor".into()],
        supported_task_categories: Vec::new(),
        write_policy: "declarative_only_not_executable".into(),
        side_effect_budget: RuntimeStrategySideEffectBudget::zero(),
        declarative_only: true,
        executable: false,
        default_chat_migration_permission: false,
        metadata_safe: true,
    }
}

fn expected_payload_kind(kind: RuntimeStrategyKind) -> RuntimeStrategyPayloadKind {
    match kind {
        RuntimeStrategyKind::ReAct => RuntimeStrategyPayloadKind::ReAct,
        RuntimeStrategyKind::PlanExecute => RuntimeStrategyPayloadKind::PlanExecute,
    }
}

fn metadata_safe_descriptor_text(serialized: &str) -> bool {
    let normalized = serialized.to_ascii_lowercase();
    ![
        "raw prompt",
        "raw assistant",
        "assistant output",
        "tools prompt",
        "tool payload",
        "memory context",
        "lifemodel text",
        "life model text",
        "@example.com",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
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
