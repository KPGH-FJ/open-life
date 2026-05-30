use crate::agent::governor::{
    GovernanceDecision, GovernanceDecisionKind, LifeModelGovernor, ToolGovernanceInput,
};
use crate::agent::runtime_contract::{RuntimeInput, RuntimeOutput};
use crate::agent::types::RiskLevel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct PlanExecuteInput {
    pub runtime_input: RuntimeInput,
    pub objective: String,
    pub max_steps: usize,
}

impl PlanExecuteInput {
    pub fn from_runtime_input(
        runtime_input: RuntimeInput,
        objective: impl Into<String>,
        max_steps: usize,
    ) -> Self {
        Self {
            runtime_input,
            objective: objective.into(),
            max_steps,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDraft {
    pub objective: String,
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub id: String,
    pub title: String,
    pub intent: String,
    pub tool_name: Option<String>,
    pub action_kind: String,
    pub risk_level: RiskLevel,
    pub declared_write: bool,
}

impl PlanStep {
    pub fn to_tool_governance_input(&self) -> ToolGovernanceInput {
        ToolGovernanceInput {
            tool_name: self
                .tool_name
                .clone()
                .unwrap_or_else(|| "runtime.reasoning".into()),
            action_kind: self.action_kind.clone(),
            risk_level: self.risk_level,
            declared_write: self.declared_write,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStepTrace {
    pub step_id: String,
    pub decision: GovernanceDecision,
    pub status: PlanStepStatus,
    pub output_summary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    Planned,
    Skipped,
    Blocked,
    RequiresProposal,
    RequiresConfirmation,
    Executed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecutionOutput {
    pub plan: PlanDraft,
    pub traces: Vec<PlanStepTrace>,
    pub runtime_outputs: Vec<RuntimeOutput>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PlanExecuteService;

impl PlanExecuteService {
    pub fn draft_plan(&self, input: &PlanExecuteInput) -> PlanDraft {
        let mut steps = Vec::new();
        let user_text = input.runtime_input.task.user_text.to_ascii_lowercase();

        if contains_search_intent(&user_text) {
            push_step(
                &mut steps,
                input.max_steps,
                PlanStepSpec {
                    title: "Read relevant context",
                    intent: "read_only_search",
                    tool_name: Some("memory.search"),
                    action_kind: "search",
                    risk_level: RiskLevel::Low,
                    declared_write: false,
                },
            );
        }

        if let Some(action_kind) = write_action_kind(&user_text) {
            push_step(
                &mut steps,
                input.max_steps,
                PlanStepSpec {
                    title: "Prepare write proposal",
                    intent: "write_like_external_action",
                    tool_name: Some("external.write_proposal"),
                    action_kind,
                    risk_level: RiskLevel::Medium,
                    declared_write: true,
                },
            );
        }

        if steps.is_empty() && input.max_steps > 0 {
            push_step(
                &mut steps,
                input.max_steps,
                PlanStepSpec {
                    title: "Reason about objective",
                    intent: "read_only_reasoning",
                    tool_name: None,
                    action_kind: "reason",
                    risk_level: RiskLevel::Low,
                    declared_write: false,
                },
            );
        }

        PlanDraft {
            objective: input.objective.clone(),
            steps,
        }
    }

    pub fn execute_plan(
        &self,
        input: PlanExecuteInput,
        governor: &LifeModelGovernor,
    ) -> PlanExecutionOutput {
        let plan = self.draft_plan(&input);
        let mut traces = Vec::with_capacity(plan.steps.len());
        let mut warnings = Vec::new();

        if input.max_steps == 0 {
            warnings.push("plan execution skipped because max_steps=0".into());
        }

        for step in &plan.steps {
            let decision = governor.govern_tool_action(step.to_tool_governance_input());
            let status = step_status(step, decision.kind);
            traces.push(PlanStepTrace {
                step_id: step.id.clone(),
                output_summary: Some(metadata_safe_step_summary(step, &decision)),
                decision,
                status,
            });
        }

        PlanExecutionOutput {
            plan,
            traces,
            runtime_outputs: Vec::new(),
            warnings,
        }
    }
}

struct PlanStepSpec {
    title: &'static str,
    intent: &'static str,
    tool_name: Option<&'static str>,
    action_kind: &'static str,
    risk_level: RiskLevel,
    declared_write: bool,
}

fn push_step(steps: &mut Vec<PlanStep>, max_steps: usize, spec: PlanStepSpec) {
    if steps.len() >= max_steps {
        return;
    }

    steps.push(PlanStep {
        id: format!("step-{}", steps.len() + 1),
        title: spec.title.into(),
        intent: spec.intent.into(),
        tool_name: spec.tool_name.map(String::from),
        action_kind: spec.action_kind.into(),
        risk_level: spec.risk_level,
        declared_write: spec.declared_write,
    });
}

fn contains_search_intent(lowercase_text: &str) -> bool {
    ["search", "查找", "检索"]
        .iter()
        .any(|needle| lowercase_text.contains(needle))
}

fn write_action_kind(lowercase_text: &str) -> Option<&'static str> {
    [
        ("write", "write"),
        ("create", "create"),
        ("update", "update"),
        ("send", "send"),
        ("schedule", "schedule"),
        ("写入", "write"),
        ("创建", "create"),
        ("更新", "update"),
        ("发送", "send"),
        ("安排", "schedule"),
    ]
    .iter()
    .find_map(|(needle, action_kind)| lowercase_text.contains(needle).then_some(*action_kind))
}

fn step_status(step: &PlanStep, kind: GovernanceDecisionKind) -> PlanStepStatus {
    match kind {
        GovernanceDecisionKind::Allow => {
            if !step.declared_write && step.risk_level == RiskLevel::Low {
                PlanStepStatus::Executed
            } else {
                PlanStepStatus::Planned
            }
        }
        GovernanceDecisionKind::RequireProposal => PlanStepStatus::RequiresProposal,
        GovernanceDecisionKind::RequireConfirmation => PlanStepStatus::RequiresConfirmation,
        GovernanceDecisionKind::RequireLocalOnly => PlanStepStatus::RequiresConfirmation,
        GovernanceDecisionKind::Block => PlanStepStatus::Blocked,
    }
}

fn metadata_safe_step_summary(step: &PlanStep, decision: &GovernanceDecision) -> String {
    let reason_code = decision
        .metadata_safe_summary
        .get("policyReasonCode")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");

    format!(
        "step_id={} action_kind={} risk_level={} decision={:?} policy_reason_code={} tool_name={}",
        step.id,
        step.action_kind,
        step.risk_level,
        decision.kind,
        reason_code,
        step.tool_name.as_deref().unwrap_or("runtime.reasoning")
    )
}
