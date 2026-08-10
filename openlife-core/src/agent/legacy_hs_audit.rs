//! Read-only compatibility shapes for historical AgentRun receipts.
//!
//! Current runtime never constructs a selection packet from these values and
//! they carry no provider, tool, or durable-write authority. They remain only
//! so existing `hs_selection_audit_json` rows can be decoded, minimized, and
//! shown as historical metadata until the AgentRun retention boundary removes
//! those rows.

use crate::agent::{EvidencePrivacyLevel, RiskLevel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HSBehaviorCheckSummary {
    pub id: String,
    pub label: String,
    pub passed: bool,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoricalHeuristicLifecycleStatus {
    Candidate,
    Trial,
    Active,
    Weakened,
    Archived,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoricalHsAssetKind {
    Policy,
    Heuristic,
    Evidence,
    State,
    LifeModelCompat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoricalHsExclusionReason {
    InactiveLifecycle,
    TaskMismatch,
    TriggerMismatch,
    PolicyConflict,
    OverBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalHsAssetExclusion {
    pub asset_id: String,
    pub asset_kind: HistoricalHsAssetKind,
    pub reason: HistoricalHsExclusionReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalGuidancePolicyBoundarySummary {
    pub hard_policy_boundary: bool,
    pub route_policy_relaxed: bool,
    pub tool_policy_relaxed: bool,
    pub proposal_first_preserved: bool,
    pub privacy_constraint_count: usize,
    pub model_constraint_count: usize,
    pub tool_constraint_count: usize,
    pub constraint_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalSelectedGuidanceRef {
    pub guidance_id: String,
    pub guidance_digest: String,
    pub guidance_type: String,
    pub lifecycle_status: HistoricalHeuristicLifecycleStatus,
    pub domain: String,
    pub trigger_digest: String,
    pub selected_reason: String,
    pub impact_kind: String,
    pub impact_summary: String,
    pub risk_level: RiskLevel,
    pub privacy_level: EvidencePrivacyLevel,
    pub source_proposal_id: Option<String>,
    pub source_evidence_count: usize,
    pub source_lineage_digest: String,
    pub policy_boundary: HistoricalGuidancePolicyBoundarySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HSSelectionAudit {
    pub agent_task_id: Option<String>,
    pub agent_run_id: Option<String>,
    pub input_digest: String,
    pub selected_policy_ids: Vec<String>,
    pub selected_heuristic_ids: Vec<String>,
    #[serde(default)]
    pub selected_guidance_ids: Vec<String>,
    #[serde(default)]
    pub selected_guidance_refs: Vec<HistoricalSelectedGuidanceRef>,
    pub excluded_assets: Vec<HistoricalHsAssetExclusion>,
    pub estimated_tokens: usize,
    pub token_budget: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historical_full_audit_shape_round_trips_without_runtime_authority() {
        let value = serde_json::json!({
            "agentTaskId": "task-old",
            "agentRunId": "run-old",
            "inputDigest": "sha256:old",
            "selectedPolicyIds": ["policy.sensitive_topics.local_only"],
            "selectedHeuristicIds": ["heuristic.old"],
            "selectedGuidanceIds": ["guidance.old"],
            "selectedGuidanceRefs": [{
                "guidanceId": "guidance.old",
                "guidanceDigest": "sha256:guidance",
                "guidanceType": "accepted_guidance",
                "lifecycleStatus": "active",
                "domain": "planning",
                "triggerDigest": "sha256:trigger",
                "selectedReason": "historical_match",
                "impactKind": "plan_shape",
                "impactSummary": "Historical metadata only.",
                "riskLevel": "low",
                "privacyLevel": "internal",
                "sourceProposalId": "proposal-old",
                "sourceEvidenceCount": 1,
                "sourceLineageDigest": "sha256:lineage",
                "policyBoundary": {
                    "hardPolicyBoundary": true,
                    "routePolicyRelaxed": false,
                    "toolPolicyRelaxed": false,
                    "proposalFirstPreserved": true,
                    "privacyConstraintCount": 1,
                    "modelConstraintCount": 1,
                    "toolConstraintCount": 1,
                    "constraintDigest": "sha256:constraint"
                }
            }],
            "excludedAssets": [{
                "assetId": "heuristic.excluded",
                "assetKind": "heuristic",
                "reason": "policy_conflict"
            }],
            "estimatedTokens": 8,
            "tokenBudget": 128
        });

        let decoded: HSSelectionAudit = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(decoded.selected_guidance_refs.len(), 1);
        assert_eq!(decoded.excluded_assets.len(), 1);
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    }
}
