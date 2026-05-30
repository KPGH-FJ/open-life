use crate::agent::governor::{
    GovernanceDecision, GovernanceDecisionKind, GovernanceSubject, LifeModelGovernor,
    ToolGovernanceInput,
};
use crate::agent::runtime_contract::{RuntimeInput, RuntimeOutput};
use crate::agent::types::RiskLevel;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanGovernanceDecisionSummary {
    pub step_id: String,
    pub subject: String,
    pub decision_kind: GovernanceDecisionKind,
    pub risk_level: RiskLevel,
    pub policy_reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanObservationSummary {
    pub step_id: String,
    pub source: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteReport {
    pub plan_id: String,
    pub source_run_id: Option<String>,
    pub step_count: usize,
    pub executed_read_only_step_count: usize,
    pub blocked_or_proposal_required_step_count: usize,
    pub governance_decisions: Vec<PlanGovernanceDecisionSummary>,
    pub observation_summaries: Vec<PlanObservationSummary>,
    pub warnings: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecutionOutput {
    pub report: PlanExecuteReport,
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
        let source_run_id = source_run_id(&input.runtime_input);
        let plan = self.draft_plan(&input);
        let plan_id = format!("plan-{}", Uuid::new_v4());
        let mut traces = Vec::with_capacity(plan.steps.len());
        let mut governance_decisions = Vec::with_capacity(plan.steps.len());
        let mut observation_summaries = Vec::new();
        let mut warnings = Vec::new();

        if input.max_steps == 0 {
            warnings.push("plan execution skipped because max_steps=0".into());
        }

        for step in &plan.steps {
            let decision = governor.govern_tool_action(step.to_tool_governance_input());
            let status = step_status(step, decision.kind);
            let output_summary = if status == PlanStepStatus::Executed {
                let observation = execute_internal_read_only_step(step);
                let summary = observation.summary.clone();
                observation_summaries.push(observation);
                Some(summary)
            } else {
                Some(metadata_safe_step_summary(step, &decision))
            };

            governance_decisions.push(metadata_safe_governance_summary(step, &decision));
            warnings.extend(decision.warnings.iter().cloned());
            traces.push(PlanStepTrace {
                step_id: step.id.clone(),
                output_summary,
                decision,
                status,
            });
        }

        let report = PlanExecuteReport::new(
            plan_id,
            source_run_id,
            &traces,
            governance_decisions,
            observation_summaries,
            warnings.clone(),
        );

        PlanExecutionOutput {
            report,
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

impl PlanExecuteReport {
    fn new(
        plan_id: String,
        source_run_id: Option<String>,
        traces: &[PlanStepTrace],
        governance_decisions: Vec<PlanGovernanceDecisionSummary>,
        observation_summaries: Vec<PlanObservationSummary>,
        warnings: Vec<String>,
    ) -> Self {
        let step_count = traces.len();
        let executed_read_only_step_count = traces
            .iter()
            .filter(|trace| trace.status == PlanStepStatus::Executed)
            .count();
        let blocked_or_proposal_required_step_count = traces
            .iter()
            .filter(|trace| {
                matches!(
                    trace.status,
                    PlanStepStatus::Blocked | PlanStepStatus::RequiresProposal
                )
            })
            .count();
        let metadata_safe_summary = json!({
            "reportKind": "plan_execute_v1",
            "planId": plan_id,
            "sourceRunId": source_run_id,
            "stepCount": step_count,
            "executedReadOnlyStepCount": executed_read_only_step_count,
            "blockedOrProposalRequiredStepCount": blocked_or_proposal_required_step_count,
            "governanceDecisionCount": governance_decisions.len(),
            "observationSummaryCount": observation_summaries.len(),
            "warningCount": warnings.len(),
        });

        Self {
            plan_id,
            source_run_id,
            step_count,
            executed_read_only_step_count,
            blocked_or_proposal_required_step_count,
            governance_decisions,
            observation_summaries,
            warnings,
            metadata_safe_summary,
        }
    }
}

fn source_run_id(input: &RuntimeInput) -> Option<String> {
    input
        .hs_packet
        .as_ref()
        .and_then(|packet| packet.audit.agent_run_id.clone())
}

fn execute_internal_read_only_step(step: &PlanStep) -> PlanObservationSummary {
    let summary = match step.intent.as_str() {
        "read_only_search" => {
            "read-only context lookup completed; raw query, memory content, and PII omitted"
        }
        "read_only_reasoning" => {
            "read-only internal reasoning completed; raw prompt and memory content omitted"
        }
        _ => "read-only internal step completed; raw inputs and content omitted",
    };

    PlanObservationSummary {
        step_id: step.id.clone(),
        source: "internal_read_only".into(),
        summary: summary.into(),
    }
}

fn metadata_safe_governance_summary(
    step: &PlanStep,
    decision: &GovernanceDecision,
) -> PlanGovernanceDecisionSummary {
    PlanGovernanceDecisionSummary {
        step_id: step.id.clone(),
        subject: governance_subject_kind(decision.subject).into(),
        decision_kind: decision.kind,
        risk_level: decision.risk_level,
        policy_reason_code: policy_reason_code(decision).into(),
    }
}

fn metadata_safe_step_summary(step: &PlanStep, decision: &GovernanceDecision) -> String {
    let reason_code = policy_reason_code(decision);

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

fn policy_reason_code(decision: &GovernanceDecision) -> &str {
    decision
        .metadata_safe_summary
        .get("policyReasonCode")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown")
}

fn governance_subject_kind(subject: GovernanceSubject) -> &'static str {
    match subject {
        GovernanceSubject::RuntimeInput => "runtime_input",
        GovernanceSubject::ToolAction => "tool_action",
        GovernanceSubject::MaturationCandidate => "maturation_candidate",
        GovernanceSubject::ModelRoute => "model_route",
    }
}
