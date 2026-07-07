use crate::agent::governor::{GovernanceDecision, GovernanceDecisionKind, LifeModelGovernor};
use crate::agent::runtime_contract::RuntimeInput;
use crate::agent::types::RiskLevel;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStrategyKind {
    ReAct,
    PlanExecute,
}

impl RuntimeStrategyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RuntimeStrategyKind::ReAct => "react",
            RuntimeStrategyKind::PlanExecute => "plan_execute",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StrategySelectionInput {
    pub runtime_input: RuntimeInput,
    pub allow_planning: bool,
    pub local_model_available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategySelection {
    pub kind: RuntimeStrategyKind,
    pub reason: String,
    #[serde(default)]
    pub governance_decision: Option<GovernanceDecision>,
    pub metadata_safe_summary: Value,
    pub report: StrategySelectionReport,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyCandidateEvaluation {
    pub strategy_kind: RuntimeStrategyKind,
    pub supported: bool,
    pub reason_code: String,
    pub governance_decision_kind: String,
    pub risk_level: String,
    pub planning_allowed: bool,
    pub local_model_available: bool,
    pub has_hs_packet: bool,
    pub blocked: bool,
    pub fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategySelectionReport {
    pub report_kind: String,
    pub selected_strategy_kind: RuntimeStrategyKind,
    pub selection_reason_code: String,
    pub governance_decision_kind: String,
    pub risk_level: String,
    pub planning_allowed: bool,
    pub local_model_available: bool,
    pub has_hs_packet: bool,
    pub blocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_kind: Option<RuntimeStrategyKind>,
    pub candidates: Vec<StrategyCandidateEvaluation>,
    pub metadata_safe: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl Default for StrategySelectionReport {
    fn default() -> Self {
        Self {
            report_kind: "strategy_selection_report".into(),
            selected_strategy_kind: RuntimeStrategyKind::ReAct,
            selection_reason_code: "default_react".into(),
            governance_decision_kind: "allow".into(),
            risk_level: "low".into(),
            planning_allowed: false,
            local_model_available: false,
            has_hs_packet: false,
            blocked: false,
            fallback_kind: None,
            candidates: Vec::new(),
            metadata_safe: true,
            warnings: Vec::new(),
        }
    }
}

pub(crate) fn select_historical_runtime_strategy(
    input: StrategySelectionInput,
) -> StrategySelection {
    let governor = LifeModelGovernor;
    let governance_decision =
        governor.govern_runtime_input(&input.runtime_input, input.local_model_available);
    let has_hs_packet = input.runtime_input.hs_packet.is_some();
    let task_kind = input.runtime_input.task.kind.to_string();
    let user_text = input.runtime_input.task.user_text.to_ascii_lowercase();
    let intent = StrategyIntent::from_user_text(&user_text);

    let mut warnings = governance_decision.warnings.clone();
    let mut risk_level = governance_decision.risk_level;
    let (kind, reason_code, reason) = if governance_decision.kind == GovernanceDecisionKind::Block {
        warnings.push("strategy selection blocked by governor".into());
        (
            choose_candidate_kind(intent, input.allow_planning),
            "governance_blocked",
            format!(
                "strategy selection blocked by governor: {}",
                governance_decision.reason
            ),
        )
    } else if intent.planning && input.allow_planning {
        (
            RuntimeStrategyKind::PlanExecute,
            "planning_intent_allowed",
            "planning intent selected PlanExecute strategy".into(),
        )
    } else if intent.planning && !input.allow_planning {
        warnings.push("planning disabled; falling back to ReAct strategy".into());
        (
            RuntimeStrategyKind::ReAct,
            "planning_disabled_fallback",
            "planning intent was detected but planning is disabled".into(),
        )
    } else if intent.write_like && input.allow_planning {
        risk_level = max_risk(risk_level, RiskLevel::Medium);
        (
            RuntimeStrategyKind::PlanExecute,
            "write_like_intent",
            "write-like intent selected PlanExecute strategy for governed step planning".into(),
        )
    } else if intent.write_like {
        risk_level = max_risk(risk_level, RiskLevel::Medium);
        warnings.push("planning disabled; falling back to ReAct strategy".into());
        (
            RuntimeStrategyKind::ReAct,
            "planning_disabled_fallback",
            "write-like intent was detected but planning is disabled".into(),
        )
    } else if intent.tool_or_observation {
        (
            RuntimeStrategyKind::ReAct,
            "tool_observation_react",
            "tool or observation intent selected ReAct strategy".into(),
        )
    } else {
        (
            RuntimeStrategyKind::ReAct,
            "default_react",
            "simple chat selected ReAct strategy".into(),
        )
    };

    let governance_decision_kind = governance_decision_kind_str(governance_decision.kind);
    let report = selection_report(SelectionReportContext {
        selected_kind: kind,
        intent,
        planning_allowed: input.allow_planning,
        local_model_available: input.local_model_available,
        has_hs_packet,
        risk_level,
        governance_decision_kind: governance_decision.kind,
        reason_code,
        warnings: &warnings,
    });

    StrategySelection {
        kind,
        reason,
        governance_decision: Some(governance_decision.clone()),
        metadata_safe_summary: selection_summary(
            kind,
            task_kind,
            risk_level,
            has_hs_packet,
            governance_decision.kind,
            reason_code,
        ),
        report: StrategySelectionReport {
            governance_decision_kind: governance_decision_kind.into(),
            ..report
        },
        warnings,
    }
}

#[derive(Debug, Clone, Copy)]
struct StrategyIntent {
    planning: bool,
    write_like: bool,
    tool_or_observation: bool,
}

struct SelectionReportContext<'a> {
    selected_kind: RuntimeStrategyKind,
    intent: StrategyIntent,
    planning_allowed: bool,
    local_model_available: bool,
    has_hs_packet: bool,
    risk_level: RiskLevel,
    governance_decision_kind: GovernanceDecisionKind,
    reason_code: &'a str,
    warnings: &'a [String],
}

impl StrategyIntent {
    fn from_user_text(lowercase_text: &str) -> Self {
        Self {
            planning: contains_any(lowercase_text, &["plan", "steps", "计划", "分步骤", "安排"]),
            write_like: contains_any(
                lowercase_text,
                &[
                    "write", "create", "update", "send", "schedule", "写入", "创建", "更新",
                    "发送", "安排",
                ],
            ),
            tool_or_observation: contains_any(
                lowercase_text,
                &["search", "tool", "observe", "检索", "查找"],
            ),
        }
    }
}

fn choose_candidate_kind(intent: StrategyIntent, allow_planning: bool) -> RuntimeStrategyKind {
    if allow_planning && (intent.planning || intent.write_like) {
        RuntimeStrategyKind::PlanExecute
    } else {
        RuntimeStrategyKind::ReAct
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn selection_summary(
    kind: RuntimeStrategyKind,
    task_kind: String,
    risk_level: RiskLevel,
    has_hs_packet: bool,
    governance_decision_kind: GovernanceDecisionKind,
    reason_code: &str,
) -> Value {
    json!({
        "selectedStrategyKind": kind.as_str(),
        "taskKind": task_kind,
        "riskLevel": risk_level.to_string(),
        "hasHsPacket": has_hs_packet,
        "governanceDecisionKind": governance_decision_kind_str(governance_decision_kind),
        "reasonCode": reason_code,
    })
}

fn selection_report(context: SelectionReportContext<'_>) -> StrategySelectionReport {
    let SelectionReportContext {
        selected_kind,
        intent,
        planning_allowed,
        local_model_available,
        has_hs_packet,
        risk_level,
        governance_decision_kind,
        reason_code,
        warnings,
    } = context;
    let blocked = governance_decision_kind == GovernanceDecisionKind::Block;
    let fallback_kind = if reason_code == "planning_disabled_fallback" {
        Some(RuntimeStrategyKind::PlanExecute)
    } else {
        None
    };
    let governance_decision_kind_text = governance_decision_kind_str(governance_decision_kind);
    let risk_level_text = risk_level.to_string();

    StrategySelectionReport {
        report_kind: "strategy_selection_report".into(),
        selected_strategy_kind: selected_kind,
        selection_reason_code: reason_code.into(),
        governance_decision_kind: governance_decision_kind_text.into(),
        risk_level: risk_level_text.clone(),
        planning_allowed,
        local_model_available,
        has_hs_packet,
        blocked,
        fallback_kind,
        candidates: vec![
            react_candidate(
                selected_kind,
                intent,
                planning_allowed,
                local_model_available,
                has_hs_packet,
                blocked,
                reason_code,
                governance_decision_kind_text,
                &risk_level_text,
            ),
            plan_execute_candidate(
                selected_kind,
                intent,
                planning_allowed,
                local_model_available,
                has_hs_packet,
                blocked,
                reason_code,
                governance_decision_kind_text,
                &risk_level_text,
            ),
        ],
        metadata_safe: true,
        warnings: warnings.to_vec(),
    }
}

#[allow(clippy::too_many_arguments)]
fn react_candidate(
    selected_kind: RuntimeStrategyKind,
    intent: StrategyIntent,
    planning_allowed: bool,
    local_model_available: bool,
    has_hs_packet: bool,
    blocked: bool,
    selected_reason_code: &str,
    governance_decision_kind: &str,
    risk_level: &str,
) -> StrategyCandidateEvaluation {
    let supported = selected_kind == RuntimeStrategyKind::ReAct && !blocked;
    let reason_code = if blocked {
        "governance_blocked"
    } else if selected_kind == RuntimeStrategyKind::ReAct {
        selected_reason_code
    } else if intent.planning || intent.write_like {
        "plan_execute_preferred"
    } else {
        "not_selected"
    };

    StrategyCandidateEvaluation {
        strategy_kind: RuntimeStrategyKind::ReAct,
        supported,
        reason_code: reason_code.into(),
        governance_decision_kind: governance_decision_kind.into(),
        risk_level: risk_level.into(),
        planning_allowed,
        local_model_available,
        has_hs_packet,
        blocked,
        fallback: selected_reason_code == "planning_disabled_fallback",
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_execute_candidate(
    selected_kind: RuntimeStrategyKind,
    intent: StrategyIntent,
    planning_allowed: bool,
    local_model_available: bool,
    has_hs_packet: bool,
    blocked: bool,
    selected_reason_code: &str,
    governance_decision_kind: &str,
    risk_level: &str,
) -> StrategyCandidateEvaluation {
    let supported = selected_kind == RuntimeStrategyKind::PlanExecute && !blocked;
    let reason_code = if blocked {
        "governance_blocked"
    } else if selected_kind == RuntimeStrategyKind::PlanExecute {
        selected_reason_code
    } else if intent.planning || intent.write_like {
        if planning_allowed {
            "not_selected"
        } else {
            "planning_disabled"
        }
    } else {
        "no_planning_or_write_intent"
    };

    StrategyCandidateEvaluation {
        strategy_kind: RuntimeStrategyKind::PlanExecute,
        supported,
        reason_code: reason_code.into(),
        governance_decision_kind: governance_decision_kind.into(),
        risk_level: risk_level.into(),
        planning_allowed,
        local_model_available,
        has_hs_packet,
        blocked,
        fallback: false,
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

fn max_risk(left: RiskLevel, right: RiskLevel) -> RiskLevel {
    if risk_rank(left) >= risk_rank(right) {
        left
    } else {
        right
    }
}

fn risk_rank(risk_level: RiskLevel) -> u8 {
    match risk_level {
        RiskLevel::Low => 0,
        RiskLevel::Medium => 1,
        RiskLevel::High => 2,
        RiskLevel::Critical => 3,
    }
}
