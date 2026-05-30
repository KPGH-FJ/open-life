use crate::agent::hs_selector::RuntimeHSPacket;
use crate::agent::maturation::MaturationProposalCandidate;
use crate::agent::policy_store::{
    ModelRoutePolicy, BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST,
    BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY,
};
use crate::agent::runtime_contract::RuntimeInput;
use crate::agent::types::{ProposalType, RiskLevel};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceSubject {
    RuntimeInput,
    ToolAction,
    MaturationCandidate,
    ModelRoute,
}

impl GovernanceSubject {
    fn as_str(self) -> &'static str {
        match self {
            GovernanceSubject::RuntimeInput => "runtime_input",
            GovernanceSubject::ToolAction => "tool_action",
            GovernanceSubject::MaturationCandidate => "maturation_candidate",
            GovernanceSubject::ModelRoute => "model_route",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceDecisionKind {
    Allow,
    RequireProposal,
    RequireConfirmation,
    RequireLocalOnly,
    Block,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernanceDecision {
    pub subject: GovernanceSubject,
    pub kind: GovernanceDecisionKind,
    pub risk_level: RiskLevel,
    pub reason: String,
    pub metadata_safe_summary: Value,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolGovernanceInput {
    pub tool_name: String,
    pub action_kind: String,
    pub risk_level: RiskLevel,
    pub declared_write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRouteGovernanceInput {
    pub hs_packet: Option<RuntimeHSPacket>,
    pub risk_level: RiskLevel,
    pub local_model_available: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LifeModelGovernor;

impl LifeModelGovernor {
    pub fn govern_maturation_candidate(
        &self,
        candidate: &MaturationProposalCandidate,
    ) -> GovernanceDecision {
        let subject = GovernanceSubject::MaturationCandidate;

        if !candidate.proposal_only {
            return decision(
                subject,
                GovernanceDecisionKind::Block,
                candidate.risk_level,
                "maturation candidate has proposal_only=false; direct LifeModel/Memory writes are blocked",
                maturation_summary(candidate, "proposal_only_false"),
                vec!["candidate must be regenerated as proposal-only before review".into()],
            );
        }

        let kind = if is_high_or_critical(candidate.risk_level)
            && is_lifemodel_update(candidate.proposal_type)
        {
            GovernanceDecisionKind::RequireConfirmation
        } else if requires_proposal_first(candidate.proposal_type) {
            GovernanceDecisionKind::RequireProposal
        } else if is_high_or_critical(candidate.risk_level) {
            GovernanceDecisionKind::RequireConfirmation
        } else {
            GovernanceDecisionKind::Allow
        };

        let reason_code = match kind {
            GovernanceDecisionKind::RequireConfirmation => "high_risk_lifemodel_confirmation",
            GovernanceDecisionKind::RequireProposal => "proposal_first_required",
            GovernanceDecisionKind::Allow => "maturation_candidate_allowed",
            GovernanceDecisionKind::RequireLocalOnly | GovernanceDecisionKind::Block => {
                "maturation_candidate_blocked"
            }
        };

        let reason = match kind {
            GovernanceDecisionKind::RequireConfirmation => {
                "high-risk maturation candidate requires explicit user confirmation before apply"
            }
            GovernanceDecisionKind::RequireProposal => {
                "maturation candidate must enter proposal-first review before any write"
            }
            GovernanceDecisionKind::Allow => {
                "maturation candidate is eligible for proposal drafting"
            }
            GovernanceDecisionKind::RequireLocalOnly | GovernanceDecisionKind::Block => {
                "maturation candidate cannot proceed"
            }
        };

        decision(
            subject,
            kind,
            candidate.risk_level,
            reason,
            maturation_summary(candidate, reason_code),
            Vec::new(),
        )
    }

    pub fn govern_tool_action(&self, input: ToolGovernanceInput) -> GovernanceDecision {
        let action_kind = input.action_kind.trim().to_ascii_lowercase();
        let write_like =
            input.declared_write || is_write_like_action(&input.tool_name, &action_kind);
        let kind = if write_like {
            GovernanceDecisionKind::RequireProposal
        } else {
            GovernanceDecisionKind::Allow
        };
        let reason_code = if write_like {
            BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST
        } else {
            "read_only_tool_allowed"
        };
        let reason = if write_like {
            "external write-like tool action must create a proposal before execution"
        } else {
            "read-only tool action is allowed by governor"
        };

        decision(
            GovernanceSubject::ToolAction,
            kind,
            input.risk_level,
            reason,
            summary(
                GovernanceSubject::ToolAction,
                if write_like {
                    Some(ProposalType::ExternalWriteAction)
                } else {
                    None
                },
                Some(input.tool_name.as_str()),
                input.risk_level,
                None,
                None,
                reason_code,
            ),
            Vec::new(),
        )
    }

    pub fn govern_runtime_input(
        &self,
        input: &RuntimeInput,
        local_model_available: bool,
    ) -> GovernanceDecision {
        if input.hs_packet.is_some() {
            return self.govern_model_route(ModelRouteGovernanceInput {
                hs_packet: input.hs_packet.clone(),
                risk_level: runtime_risk_level(input.hs_packet.as_ref()),
                local_model_available,
            });
        }

        decision(
            GovernanceSubject::RuntimeInput,
            GovernanceDecisionKind::Allow,
            RiskLevel::Low,
            "runtime input has no explicit governed write intent",
            summary(
                GovernanceSubject::RuntimeInput,
                None,
                None,
                RiskLevel::Low,
                None,
                None,
                "no_explicit_write_intent",
            ),
            Vec::new(),
        )
    }

    pub fn govern_model_route(&self, input: ModelRouteGovernanceInput) -> GovernanceDecision {
        let source_run_id = input
            .hs_packet
            .as_ref()
            .and_then(|packet| packet.audit.agent_run_id.as_deref());

        if packet_requires_local_only(input.hs_packet.as_ref()) {
            if !input.local_model_available {
                return decision(
                    GovernanceSubject::ModelRoute,
                    GovernanceDecisionKind::Block,
                    input.risk_level,
                    "fail-closed: local-only policy selected but no local model is available",
                    summary(
                        GovernanceSubject::ModelRoute,
                        None,
                        None,
                        input.risk_level,
                        source_run_id,
                        None,
                        "sensitive_local_only_no_local_model",
                    ),
                    Vec::new(),
                );
            }

            return decision(
                GovernanceSubject::ModelRoute,
                GovernanceDecisionKind::RequireLocalOnly,
                input.risk_level,
                "sensitive runtime policy requires local-only model routing",
                summary(
                    GovernanceSubject::ModelRoute,
                    None,
                    None,
                    input.risk_level,
                    source_run_id,
                    None,
                    "sensitive_local_only",
                ),
                Vec::new(),
            );
        }

        decision(
            GovernanceSubject::ModelRoute,
            GovernanceDecisionKind::Allow,
            input.risk_level,
            "model route is allowed by governor",
            summary(
                GovernanceSubject::ModelRoute,
                None,
                None,
                input.risk_level,
                source_run_id,
                None,
                "model_route_allowed",
            ),
            Vec::new(),
        )
    }
}

fn decision(
    subject: GovernanceSubject,
    kind: GovernanceDecisionKind,
    risk_level: RiskLevel,
    reason: impl Into<String>,
    metadata_safe_summary: Value,
    warnings: Vec<String>,
) -> GovernanceDecision {
    GovernanceDecision {
        subject,
        kind,
        risk_level,
        reason: reason.into(),
        metadata_safe_summary,
        warnings,
    }
}

fn maturation_summary(candidate: &MaturationProposalCandidate, reason_code: &str) -> Value {
    summary(
        GovernanceSubject::MaturationCandidate,
        Some(candidate.proposal_type),
        Some(candidate.affected_path.as_str()),
        candidate.risk_level,
        candidate.source_run_id.as_deref(),
        Some(candidate.source_event_type.as_str()),
        reason_code,
    )
}

fn summary(
    subject: GovernanceSubject,
    proposal_type: Option<ProposalType>,
    affected_path: Option<&str>,
    risk_level: RiskLevel,
    source_run_id: Option<&str>,
    event_type: Option<&str>,
    reason_code: &str,
) -> Value {
    json!({
        "subjectKind": subject.as_str(),
        "proposalType": proposal_type.map(|proposal_type| proposal_type.to_string()),
        "affectedPath": affected_path,
        "riskLevel": risk_level.to_string(),
        "sourceRunId": source_run_id,
        "eventType": event_type,
        "policyReasonCode": reason_code,
    })
}

fn requires_proposal_first(proposal_type: ProposalType) -> bool {
    matches!(
        proposal_type,
        ProposalType::LifeModelUpdate
            | ProposalType::GoalUpdate
            | ProposalType::StateUpdate
            | ProposalType::PreferenceUpdate
            | ProposalType::MemoryWrite
    )
}

fn is_lifemodel_update(proposal_type: ProposalType) -> bool {
    matches!(
        proposal_type,
        ProposalType::LifeModelUpdate
            | ProposalType::GoalUpdate
            | ProposalType::StateUpdate
            | ProposalType::PreferenceUpdate
    )
}

fn is_high_or_critical(risk_level: RiskLevel) -> bool {
    matches!(risk_level, RiskLevel::High | RiskLevel::Critical)
}

fn is_write_like_action(tool_name: &str, action_kind: &str) -> bool {
    if matches!(
        action_kind,
        "catalog" | "manifest" | "tools_prompt" | "available_tools"
    ) {
        return false;
    }

    if matches!(
        action_kind,
        "write"
            | "external_side_effect"
            | "create"
            | "update"
            | "delete"
            | "patch"
            | "send"
            | "post"
            | "put"
            | "mutation"
            | "modify"
            | "archive"
            | "move"
            | "rename"
    ) {
        return true;
    }

    let normalized_tool = tool_name.trim().to_ascii_lowercase();
    [
        ".write", ".create", ".update", ".delete", ".patch", ".send", ".post", ".put", ".modify",
        ".archive", ".move", ".rename", "_write", "_create", "_update", "_delete", "_patch",
        "_send",
    ]
    .iter()
    .any(|needle| normalized_tool.contains(needle))
}

fn packet_requires_local_only(packet: Option<&RuntimeHSPacket>) -> bool {
    packet.is_some_and(|packet| {
        packet
            .audit
            .selected_policy_ids
            .iter()
            .any(|id| id == BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY)
            || packet.selected_policies.iter().any(|policy| {
                policy.policy_id == BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY
                    || policy.route == Some(ModelRoutePolicy::LocalOnly)
            })
    })
}

fn runtime_risk_level(packet: Option<&RuntimeHSPacket>) -> RiskLevel {
    if packet_requires_local_only(packet) {
        RiskLevel::High
    } else {
        RiskLevel::Low
    }
}
