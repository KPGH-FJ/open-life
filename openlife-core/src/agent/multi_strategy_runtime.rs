use crate::agent::governor::GovernanceDecisionKind;
use crate::agent::plan_execute::PlanExecutionOutput;
use crate::agent::runtime::{AgentRuntime, AgentRuntimeError};
use crate::agent::runtime_contract::{RuntimeInput, RuntimeOutput};
use crate::agent::strategy::{
    RuntimeStrategyKind, StrategySelection, StrategySelectionInput, StrategySelector,
};
use crate::agent::strategy_runtime::{
    PlanExecuteRuntimeStrategy, ReActRuntimeStrategy, RuntimeStrategyDescriptor,
    RuntimeStrategyExecutionReport, RuntimeStrategyInput, RuntimeStrategyPayload,
    RuntimeStrategyPayloadKind, RuntimeStrategyRegistry, RuntimeStrategyRegistryReadinessReport,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
    pub execution_report: RuntimeStrategyExecutionReport,
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
        let registry_report = self.strategies.readiness_report();

        if selection
            .governance_decision
            .as_ref()
            .is_some_and(|decision| decision.kind == GovernanceDecisionKind::Block)
        {
            let descriptor = descriptor_for_selection(&registry_report, selection.kind);
            return Ok(MultiStrategyRuntimeOutput {
                execution_report: execution_report(
                    &selection,
                    &registry_report,
                    descriptor.as_ref(),
                    RuntimeStrategyPayloadKind::Blocked,
                    true,
                    warnings.len(),
                    json!({}),
                ),
                selection,
                payload: MultiStrategyRuntimePayload::Blocked,
                warnings,
            });
        }

        let strategy = self.strategies.get(selection.kind).ok_or_else(|| {
            AgentRuntimeError::StrategyNotFound(format!(
                "runtime_strategy_missing:{}",
                strategy_kind_str(selection.kind)
            ))
        })?;
        let descriptor = strategy.descriptor();
        let strategy_output = strategy
            .execute(RuntimeStrategyInput {
                runtime_input: input.runtime_input,
                selection: selection.clone(),
            })
            .await?;
        warnings.extend(strategy_output.warnings.iter().cloned());
        let payload_kind = strategy.payload_kind();
        let strategy_output_summary = strategy_output.metadata_safe_summary.clone();

        Ok(MultiStrategyRuntimeOutput {
            execution_report: execution_report(
                &selection,
                &registry_report,
                Some(&descriptor),
                payload_kind,
                false,
                warnings.len(),
                strategy_output_summary,
            ),
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

fn descriptor_for_selection(
    registry_report: &RuntimeStrategyRegistryReadinessReport,
    kind: RuntimeStrategyKind,
) -> Option<RuntimeStrategyDescriptor> {
    registry_report
        .executable_descriptors
        .iter()
        .find(|descriptor| descriptor.strategy_kind == kind)
        .cloned()
}

fn execution_report(
    selection: &StrategySelection,
    registry_report: &RuntimeStrategyRegistryReadinessReport,
    descriptor: Option<&RuntimeStrategyDescriptor>,
    payload_kind: RuntimeStrategyPayloadKind,
    blocked: bool,
    warning_count: usize,
    strategy_output_summary: Value,
) -> RuntimeStrategyExecutionReport {
    let governance_decision_kind = selection
        .governance_decision
        .as_ref()
        .map(|decision| governance_decision_kind_str(decision.kind))
        .unwrap_or("unknown");
    let fallback_descriptor = RuntimeStrategyDescriptor::executable(
        selection.kind,
        strategy_kind_str(selection.kind),
        strategy_kind_str(selection.kind),
        payload_kind,
    );
    let descriptor = descriptor.unwrap_or(&fallback_descriptor);

    RuntimeStrategyExecutionReport {
        report_kind: "runtime_strategy_execution_report".into(),
        runtime_kind: "multi_strategy_runtime".into(),
        selected_strategy_kind: selection.kind,
        payload_kind,
        strategy_descriptor_id: descriptor.metadata_safe_id.clone(),
        strategy_descriptor_name: descriptor.metadata_safe_name.clone(),
        strategy_capability_ids: descriptor.capability_ids.clone(),
        registry_ready: registry_report.ready,
        selection_reason_code: selection.report.selection_reason_code.clone(),
        governance_decision_kind: governance_decision_kind.into(),
        blocked,
        warning_count,
        side_effect_budget: if blocked {
            RuntimeStrategyRegistry::maturity_report().status_command_side_effect_budget
        } else {
            descriptor.side_effect_budget.clone()
        },
        default_chat_unchanged: true,
        metadata_safe: true,
        strategy_output_summary,
        registry_blocking_reasons: registry_report.blocking_reasons.clone(),
    }
}

fn governance_decision_kind_str(kind: GovernanceDecisionKind) -> &'static str {
    match kind {
        GovernanceDecisionKind::Allow => "allow",
        GovernanceDecisionKind::RequireProposal => "require_proposal",
        GovernanceDecisionKind::RequireConfirmation => "require_confirmation",
        GovernanceDecisionKind::RequireLocalOnly => "require_local_only",
        GovernanceDecisionKind::Block => "block",
    }
}
