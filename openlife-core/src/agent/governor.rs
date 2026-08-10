use crate::agent::policy_store::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST;
use crate::agent::runtime_contract::RuntimeInput;
use crate::agent::types::{ProposalType, RiskLevel};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceSubject {
    RuntimeInput,
    ToolAction,
    ModelRoute,
    MemoryWrite,
    ExternalWrite,
}

impl GovernanceSubject {
    fn as_str(self) -> &'static str {
        match self {
            GovernanceSubject::RuntimeInput => "runtime_input",
            GovernanceSubject::ToolAction => "tool_action",
            GovernanceSubject::ModelRoute => "model_route",
            GovernanceSubject::MemoryWrite => "memory_write",
            GovernanceSubject::ExternalWrite => "external_write",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceDecisionClassification {
    Allow,
    ProposalFirst,
    Confirm,
    LocalOnly,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernorDecisionReport {
    pub report_kind: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub subject: GovernanceSubject,
    pub decision_kind: GovernanceDecisionKind,
    pub classification: GovernanceDecisionClassification,
    pub allowed: bool,
    pub blocked: bool,
    pub requires_confirmation: bool,
    pub requires_proposal: bool,
    pub requires_local_only: bool,
    pub risk_level: RiskLevel,
    pub policy_reason_code: String,
    pub proposal_type: Option<String>,
    pub source_run_id: Option<String>,
    pub selected_policy_ids: Vec<String>,
    pub metadata_safe_summary: Value,
    pub warning_count: usize,
    pub decision_digest: String,
    pub raw_prompt_included: bool,
    pub raw_user_text_included: bool,
    pub raw_assistant_output_included: bool,
    pub raw_memory_included: bool,
    pub raw_life_model_included: bool,
    pub raw_tool_payload_included: bool,
}

impl GovernanceDecision {
    pub fn to_report(&self) -> GovernorDecisionReport {
        let classification = classify_decision_kind(self.kind);
        let policy_reason_code = summary_string(&self.metadata_safe_summary, "policyReasonCode")
            .unwrap_or_else(|| "unknown_policy_reason".into());
        let proposal_type = summary_string(&self.metadata_safe_summary, "proposalType");
        let source_run_id = summary_string(&self.metadata_safe_summary, "sourceRunId");
        let selected_policy_ids =
            summary_string_vec(&self.metadata_safe_summary, "selectedPolicyIds");
        let digest_input = json!({
            "subject": self.subject.as_str(),
            "decisionKind": decision_kind_str(self.kind),
            "classification": classification_str(classification),
            "riskLevel": self.risk_level.to_string(),
            "policyReasonCode": policy_reason_code,
            "proposalType": proposal_type,
            "sourceRunId": source_run_id,
            "selectedPolicyIds": selected_policy_ids,
            "warningCount": self.warnings.len(),
        });
        let decision_digest = digest_value(&digest_input);

        GovernorDecisionReport {
            report_kind: "governor_decision_report".into(),
            metadata_safe: true,
            contains_raw_content: false,
            subject: self.subject,
            decision_kind: self.kind,
            classification,
            allowed: self.kind == GovernanceDecisionKind::Allow,
            blocked: self.kind == GovernanceDecisionKind::Block,
            requires_confirmation: self.kind == GovernanceDecisionKind::RequireConfirmation,
            requires_proposal: self.kind == GovernanceDecisionKind::RequireProposal,
            requires_local_only: self.kind == GovernanceDecisionKind::RequireLocalOnly,
            risk_level: self.risk_level,
            policy_reason_code,
            proposal_type,
            source_run_id,
            selected_policy_ids,
            metadata_safe_summary: self.metadata_safe_summary.clone(),
            warning_count: self.warnings.len(),
            decision_digest,
            raw_prompt_included: false,
            raw_user_text_included: false,
            raw_assistant_output_included: false,
            raw_memory_included: false,
            raw_life_model_included: false,
            raw_tool_payload_included: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolGovernanceInput {
    pub tool_name: String,
    pub action_kind: String,
    pub risk_level: RiskLevel,
    pub declared_write: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryWriteGovernanceInput {
    pub risk_level: RiskLevel,
    pub source_run_id: Option<String>,
    pub proposal_already_created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalWriteGovernanceInput {
    pub tool_name: String,
    pub risk_level: RiskLevel,
    pub source_run_id: Option<String>,
    pub proposal_already_created: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LifeModelGovernor;

impl LifeModelGovernor {
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

    pub fn govern_memory_write(&self, input: MemoryWriteGovernanceInput) -> GovernanceDecision {
        let (kind, reason_code, reason) = if input.proposal_already_created {
            (
                GovernanceDecisionKind::Allow,
                "memory_write_proposal_already_created",
                "memory write is represented by an existing proposal",
            )
        } else {
            (
                GovernanceDecisionKind::RequireProposal,
                "memory_write_proposal_first_required",
                "memory write must create a proposal before persistence",
            )
        };

        decision(
            GovernanceSubject::MemoryWrite,
            kind,
            input.risk_level,
            reason,
            summary(
                GovernanceSubject::MemoryWrite,
                Some(ProposalType::MemoryWrite),
                None,
                input.risk_level,
                input.source_run_id.as_deref(),
                None,
                reason_code,
            ),
            Vec::new(),
        )
    }

    pub fn govern_external_write(&self, input: ExternalWriteGovernanceInput) -> GovernanceDecision {
        let (kind, reason_code, reason) = if input.proposal_already_created {
            (
                GovernanceDecisionKind::Allow,
                "external_write_proposal_already_created",
                "external write is represented by an existing proposal",
            )
        } else {
            (
                GovernanceDecisionKind::RequireProposal,
                BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST,
                "external write-like tool action must create a proposal before execution",
            )
        };

        decision(
            GovernanceSubject::ExternalWrite,
            kind,
            input.risk_level,
            reason,
            summary(
                GovernanceSubject::ExternalWrite,
                Some(ProposalType::ExternalWriteAction),
                Some(input.tool_name.as_str()),
                input.risk_level,
                input.source_run_id.as_deref(),
                None,
                reason_code,
            ),
            Vec::new(),
        )
    }

    pub fn govern_unsupported_tool_source(
        &self,
        tool_name: &str,
        risk_level: RiskLevel,
        source_run_id: Option<&str>,
        reason_code: &str,
    ) -> GovernanceDecision {
        decision(
            GovernanceSubject::ToolAction,
            GovernanceDecisionKind::Block,
            risk_level,
            "tool source has no governed executor and remains disabled/declarative-only",
            summary(
                GovernanceSubject::ToolAction,
                None,
                Some(tool_name),
                risk_level,
                source_run_id,
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
        if input.policy_context.provider_authorization().data_route()
            == crate::llm::ProviderDataRoute::LocalOnly
            && !local_model_available
        {
            return decision(
                GovernanceSubject::ModelRoute,
                GovernanceDecisionKind::Block,
                RiskLevel::High,
                "typed Policy requires local execution but no local model is available",
                summary(
                    GovernanceSubject::ModelRoute,
                    None,
                    None,
                    RiskLevel::High,
                    input.source_run_id.as_deref(),
                    None,
                    "local_only_model_unavailable",
                ),
                Vec::new(),
            );
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

fn classify_decision_kind(kind: GovernanceDecisionKind) -> GovernanceDecisionClassification {
    match kind {
        GovernanceDecisionKind::Allow => GovernanceDecisionClassification::Allow,
        GovernanceDecisionKind::RequireProposal => GovernanceDecisionClassification::ProposalFirst,
        GovernanceDecisionKind::RequireConfirmation => GovernanceDecisionClassification::Confirm,
        GovernanceDecisionKind::RequireLocalOnly => GovernanceDecisionClassification::LocalOnly,
        GovernanceDecisionKind::Block => GovernanceDecisionClassification::Block,
    }
}

fn decision_kind_str(kind: GovernanceDecisionKind) -> &'static str {
    match kind {
        GovernanceDecisionKind::Allow => "allow",
        GovernanceDecisionKind::RequireProposal => "require_proposal",
        GovernanceDecisionKind::RequireConfirmation => "require_confirmation",
        GovernanceDecisionKind::RequireLocalOnly => "require_local_only",
        GovernanceDecisionKind::Block => "block",
    }
}

fn classification_str(classification: GovernanceDecisionClassification) -> &'static str {
    match classification {
        GovernanceDecisionClassification::Allow => "allow",
        GovernanceDecisionClassification::ProposalFirst => "proposal_first",
        GovernanceDecisionClassification::Confirm => "confirm",
        GovernanceDecisionClassification::LocalOnly => "local_only",
        GovernanceDecisionClassification::Block => "block",
    }
}

fn summary_string(summary: &Value, key: &str) -> Option<String> {
    summary
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn summary_string_vec(summary: &Value, key: &str) -> Vec<String> {
    summary
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn digest_value(value: &Value) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    let hash = digest(&SHA256, serialized.as_bytes());
    let hex = hash
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}
