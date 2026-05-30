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
    fn as_str(self) -> &'static str {
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
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct StrategySelector;

impl StrategySelector {
    pub fn select(&self, input: StrategySelectionInput) -> StrategySelection {
        let governor = LifeModelGovernor;
        let governance_decision =
            governor.govern_runtime_input(&input.runtime_input, input.local_model_available);
        let has_hs_packet = input.runtime_input.hs_packet.is_some();
        let task_kind = input.runtime_input.task.kind.to_string();
        let user_text = input.runtime_input.task.user_text.to_ascii_lowercase();
        let intent = StrategyIntent::from_user_text(&user_text);

        let mut warnings = governance_decision.warnings.clone();
        let mut risk_level = governance_decision.risk_level;
        let (kind, reason_code, reason) = if governance_decision.kind
            == GovernanceDecisionKind::Block
        {
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
            warnings,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StrategyIntent {
    planning: bool,
    write_like: bool,
    tool_or_observation: bool,
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
