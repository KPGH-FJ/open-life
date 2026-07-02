use crate::agent::evidence_graph::EvidenceTimelineReadModel;
use crate::agent::governor::{
    GovernanceDecisionClassification, GovernanceDecisionKind, GovernanceSubject,
    GovernorDecisionReport,
};
use crate::agent::hs_selector::GuidanceImpactReadModel;
use crate::agent::types::{AgentProposal, AgentRun, AgentRunStatus, AgentTaskKind, ProposalStatus};
use crate::agent::{LifeModelBackendGateBlocker, LifeModelVersionReadModel};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const CONTRACT_REPORT_KIND: &str = "w147.preUiBackendContractFreeze.v1";
const FINAL_GATE_REPORT_KIND: &str = "w148.finalBackendCompletionGate.v1";
const DEFAULT_CHAT_KERNEL_PATH: &str = "main_chat_kernel";
const REQUIRED_SURFACES: [&str; 7] = [
    "learning_inbox",
    "evidence_timeline",
    "proposal_review",
    "runtime_trace",
    "guidance_impact",
    "privacy_controls",
    "lifemodel_overview",
];

#[derive(Debug, Clone)]
pub struct PreUiBackendContractFreezeInput {
    pub generated_at: DateTime<Utc>,
    pub evidence_timeline: EvidenceTimelineReadModel,
    pub proposals: Vec<AgentProposal>,
    pub agent_runs: Vec<AgentRun>,
    pub guidance_impact: GuidanceImpactReadModel,
    pub governor_decisions: Vec<GovernorDecisionReport>,
    pub lifemodel_version: LifeModelVersionReadModel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreUiBackendContractFreezeReport {
    pub report_kind: String,
    pub contract_frozen: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub generated_at: DateTime<Utc>,
    pub read_only: bool,
    pub command_surface_added: bool,
    pub tauri_command_required: bool,
    pub default_chat_unchanged: bool,
    pub default_chat_selected_adapter_path: String,
    pub ordinary_chat_route_unchanged: bool,
    pub migration_permission: bool,
    pub surface_count: usize,
    pub surfaces: Vec<UiReadModelSurfaceContract>,
    pub learning_inbox: LearningInboxReadModel,
    pub evidence_timeline: EvidenceTimelineReadModel,
    pub proposal_review: ProposalReviewReadModel,
    pub runtime_trace: RuntimeTraceReadModel,
    pub guidance_impact: GuidanceImpactReadModel,
    pub privacy_controls: PrivacyControlsReadModel,
    pub lifemodel_overview: LifeModelOverviewReadModel,
    pub schema_digest: String,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiReadModelSurfaceContract {
    pub surface: String,
    pub schema_name: String,
    pub schema_version: String,
    pub stable_for_pre_ui_design: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub read_only: bool,
    pub command_surface_required: bool,
    pub source_contracts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningInboxReadModel {
    pub report_kind: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub generated_at: DateTime<Utc>,
    pub item_count: usize,
    pub candidate_evidence_count: usize,
    pub pending_proposal_count: usize,
    pub items: Vec<LearningInboxItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningInboxItem {
    pub item_id: String,
    pub item_kind: String,
    pub status: String,
    pub affected_path: String,
    pub risk_level: String,
    pub privacy_level: Option<String>,
    pub confidence: Option<f32>,
    pub source_ref_count: usize,
    pub linked_proposal_ids: Vec<String>,
    pub linked_agent_run_ids: Vec<String>,
    pub cluster_id: Option<String>,
    pub payload_digest: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalReviewReadModel {
    pub report_kind: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub generated_at: DateTime<Utc>,
    pub proposal_count: usize,
    pub pending_count: usize,
    pub high_risk_pending_count: usize,
    pub proposal_first_review_required: bool,
    pub items: Vec<ProposalReviewItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalReviewItem {
    pub proposal_id: String,
    pub run_id: Option<String>,
    pub proposal_type: String,
    pub source: String,
    pub affected_path: String,
    pub status: String,
    pub risk_level: String,
    pub confidence: f32,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub payload_digest: String,
    pub raw_values_included: bool,
    pub raw_reason_included: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTraceReadModel {
    pub report_kind: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub generated_at: DateTime<Utc>,
    pub run_count: usize,
    pub hs_influence: RuntimeTraceHsInfluence,
    pub runs: Vec<RuntimeTraceRunItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTraceHsInfluence {
    pub included: bool,
    pub selected_policy_ids: Vec<String>,
    pub selected_guidance_ids: Vec<String>,
    pub selected_guidance_digests: Vec<String>,
    pub behavior_check_count: usize,
    pub passed_behavior_check_count: usize,
    pub affected_surface_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTraceRunItem {
    pub run_id: String,
    pub task_id: String,
    pub session_id: Option<String>,
    pub status: AgentRunStatus,
    pub kind: AgentTaskKind,
    pub reasoning_strategy: Option<String>,
    pub selected_policy_ids: Vec<String>,
    pub selected_guidance_ids: Vec<String>,
    pub generated_proposal_ids: Vec<String>,
    pub action_count: usize,
    pub observation_count: usize,
    pub warning_count: usize,
    pub behavior_check_count: usize,
    pub passed_behavior_check_count: usize,
    pub step_count: u32,
    pub tool_call_count: u32,
    pub model_provider: Option<String>,
    pub model_route_type: Option<String>,
    pub privacy_level: Option<String>,
    pub redaction_applied: Option<bool>,
    pub redaction_level: Option<String>,
    pub context_section_count: usize,
    pub memory_hit_count: Option<i64>,
    pub input_digest: Option<String>,
    pub output_digest: Option<String>,
    pub raw_user_input_included: bool,
    pub raw_output_included: bool,
    pub raw_memory_included: bool,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyControlsReadModel {
    pub report_kind: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub generated_at: DateTime<Utc>,
    pub local_only_policy_visible: bool,
    pub local_only_decision_count: usize,
    pub proposal_first_policy_visible: bool,
    pub raw_content_exclusion_visible: bool,
    pub selected_policy_ids: Vec<String>,
    pub decisions: Vec<PrivacyDecisionSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyDecisionSummary {
    pub subject: GovernanceSubject,
    pub decision_kind: GovernanceDecisionKind,
    pub classification: GovernanceDecisionClassification,
    pub risk_level: String,
    pub policy_reason_code: String,
    pub requires_local_only: bool,
    pub requires_proposal: bool,
    pub requires_confirmation: bool,
    pub blocked: bool,
    pub selected_policy_ids: Vec<String>,
    pub decision_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelOverviewReadModel {
    pub report_kind: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub generated_at: DateTime<Utc>,
    pub from_version_id: String,
    pub to_version_id: String,
    pub compatibility_materialized_view: bool,
    pub accepted_source_of_truth: bool,
    pub durable_truth_materialized: bool,
    pub proposal_first_required_for_truth: bool,
    pub provenance_traceable: bool,
    pub source_proposal_count: usize,
    pub source_evidence_count: usize,
    pub source_patch_count: usize,
    pub source_heuristic_count: usize,
    pub accepted_guidance_count: usize,
    pub changed_asset_count: usize,
    pub rollback_available: bool,
    pub materialized_view_source_digest: String,
    pub materialized_view_provenance_digest: String,
    pub diff_reference_digest: String,
    pub rollback_reference_digest: Option<String>,
    pub raw_life_model_fields_included: bool,
    pub raw_guidance_included: bool,
}

#[derive(Debug, Clone)]
pub struct FinalBackendCompletionGateInput {
    pub generated_at: DateTime<Utc>,
    pub contract_freeze: PreUiBackendContractFreezeReport,
    pub evidence: BackendCompletionGateEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendCompletionGateEvidence {
    pub lifemodel_maturity_gate_passed: bool,
    pub runtime_driven_gate_passed: bool,
    pub governance_privacy_gate_passed: bool,
    pub ui_read_model_gate_passed: bool,
    pub default_chat_isolated: bool,
    pub ordinary_chat_route_unchanged: bool,
    pub proposal_first_boundaries_preserved: bool,
    pub raw_content_excluded: bool,
    pub local_only_privacy_enforced: bool,
    pub tool_governance_enforced: bool,
    pub golden_paths_ready: bool,
    pub materialized_lifemodel_provenance_traceable: bool,
    pub high_risk_auto_materialization_blocked: bool,
    pub remaining_beta_blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalBackendCompletionGateReport {
    pub report_kind: String,
    pub gate_ready: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub generated_at: DateTime<Utc>,
    pub read_only: bool,
    pub tauri_command_added: bool,
    pub migration_permission: bool,
    pub default_chat_isolation: DefaultChatIsolationProof,
    pub proposal_first_boundaries: ProposalFirstBoundaryProof,
    pub raw_content_exclusion: RawContentExclusionProof,
    pub local_only_privacy: LocalOnlyPrivacyProof,
    pub tool_governance: ToolGovernanceProof,
    pub golden_path_coverage: GoldenPathCoverageProof,
    pub ui_read_model_contracts: UiReadModelGateProof,
    pub materialized_lifemodel_provenance_traceable: bool,
    pub high_risk_auto_materialization_blocked: bool,
    pub acceptance_gates: Vec<BackendCompletionAcceptanceGateStatus>,
    pub blockers_by_gate: Vec<LifeModelBackendGateBlocker>,
    pub remaining_beta_blockers: Vec<String>,
    pub business_write_count: u32,
    pub runtime_execution_count: u32,
    pub model_execution_count: u32,
    pub tool_execution_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatIsolationProof {
    pub default_chat_isolated: bool,
    pub selected_adapter_path: String,
    pub ordinary_chat_route_unchanged: bool,
    pub migration_permission: bool,
    pub ordinary_chat_calls_goal8_helpers: bool,
    pub ordinary_chat_calls_golden_path_helpers: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalFirstBoundaryProof {
    pub proposal_first_preserved: bool,
    pub external_writes_bypass_proposal: bool,
    pub memory_writes_bypass_proposal: bool,
    pub lifemodel_truth_writes_bypass_proposal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawContentExclusionProof {
    pub raw_content_excluded: bool,
    pub raw_prompt_included: bool,
    pub raw_user_text_included: bool,
    pub raw_assistant_output_included: bool,
    pub raw_memory_included: bool,
    pub raw_life_model_included: bool,
    pub raw_tool_payload_included: bool,
    pub raw_guidance_included: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalOnlyPrivacyProof {
    pub local_only_enforced: bool,
    pub cloud_fallback_for_local_only: bool,
    pub high_or_critical_privacy_local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolGovernanceProof {
    pub tool_governance_enforced: bool,
    pub unsupported_plugins_blocked: bool,
    pub disabled_declarative_tools_blocked: bool,
    pub hs_write_like_paths_proposal_first: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldenPathCoverageProof {
    pub weekly_planning_ready: bool,
    pub low_energy_support_ready: bool,
    pub preference_correction_ready: bool,
    pub ordinary_chat_uses_golden_paths: bool,
    pub golden_path_ready_grants_migration: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiReadModelGateProof {
    pub contract_frozen: bool,
    pub learning_inbox_exists: bool,
    pub evidence_timeline_exists: bool,
    pub proposal_review_exists: bool,
    pub runtime_trace_includes_hs_influence: bool,
    pub guidance_impact_exists: bool,
    pub privacy_controls_exists: bool,
    pub lifemodel_overview_exists: bool,
    pub version_diff_rollback_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendCompletionAcceptanceGateStatus {
    pub gate: String,
    pub passed: bool,
    pub blockers: Vec<String>,
}

pub fn freeze_pre_ui_backend_read_model_contracts(
    input: PreUiBackendContractFreezeInput,
) -> PreUiBackendContractFreezeReport {
    let learning_inbox = build_learning_inbox(
        input.generated_at,
        &input.evidence_timeline,
        &input.proposals,
    );
    let proposal_review = build_proposal_review(input.generated_at, &input.proposals);
    let runtime_trace = build_runtime_trace(
        input.generated_at,
        &input.agent_runs,
        &input.guidance_impact,
    );
    let privacy_controls = build_privacy_controls(
        input.generated_at,
        &input.guidance_impact,
        &input.governor_decisions,
    );
    let lifemodel_overview = build_lifemodel_overview(input.generated_at, &input.lifemodel_version);
    let surfaces = surface_contracts();
    let surface_count = surfaces.len();
    let contains_raw_content = input.evidence_timeline.contains_raw_content
        || input.guidance_impact.contains_raw_content
        || input.lifemodel_version.contains_raw_content
        || input.lifemodel_version.raw_content_included
        || input
            .governor_decisions
            .iter()
            .any(|decision| decision.contains_raw_content || governor_report_raw_flag(decision));
    let metadata_safe = input.evidence_timeline.metadata_safe
        && input.guidance_impact.metadata_safe
        && input.lifemodel_version.metadata_safe
        && input
            .governor_decisions
            .iter()
            .all(|decision| decision.metadata_safe)
        && !contains_raw_content;
    let mut blockers = Vec::new();
    if surface_count != REQUIRED_SURFACES.len() {
        blockers.push("required_surface_contract_count_mismatch".into());
    }
    if !metadata_safe {
        blockers.push("metadata_safety_not_proven".into());
    }
    if contains_raw_content {
        blockers.push("raw_content_present_in_source_read_model".into());
    }
    if !runtime_trace.hs_influence.included {
        blockers.push("runtime_trace_hs_influence_missing".into());
    }
    if !privacy_controls.raw_content_exclusion_visible {
        blockers.push("privacy_raw_content_exclusion_missing".into());
    }
    if !lifemodel_overview.provenance_traceable {
        blockers.push("lifemodel_overview_provenance_missing".into());
    }
    let contract_frozen = blockers.is_empty();
    let schema_digest = digest_json(&serde_json::json!({
        "schema": CONTRACT_REPORT_KIND,
        "surfaces": surfaces,
        "learningInboxKind": learning_inbox.report_kind,
        "proposalReviewKind": proposal_review.report_kind,
        "runtimeTraceKind": runtime_trace.report_kind,
        "privacyControlsKind": privacy_controls.report_kind,
        "lifeModelOverviewKind": lifemodel_overview.report_kind,
    }));

    PreUiBackendContractFreezeReport {
        report_kind: CONTRACT_REPORT_KIND.into(),
        contract_frozen,
        metadata_safe,
        contains_raw_content,
        generated_at: input.generated_at,
        read_only: true,
        command_surface_added: false,
        tauri_command_required: false,
        default_chat_unchanged: true,
        default_chat_selected_adapter_path: DEFAULT_CHAT_KERNEL_PATH.into(),
        ordinary_chat_route_unchanged: true,
        migration_permission: false,
        surface_count,
        surfaces,
        learning_inbox,
        evidence_timeline: input.evidence_timeline,
        proposal_review,
        runtime_trace,
        guidance_impact: input.guidance_impact,
        privacy_controls,
        lifemodel_overview,
        schema_digest,
        blockers,
    }
}

pub fn evaluate_final_backend_completion_gate(
    input: FinalBackendCompletionGateInput,
) -> FinalBackendCompletionGateReport {
    let evidence = input.evidence;
    let default_chat_isolation = DefaultChatIsolationProof {
        default_chat_isolated: evidence.default_chat_isolated,
        selected_adapter_path: DEFAULT_CHAT_KERNEL_PATH.into(),
        ordinary_chat_route_unchanged: evidence.ordinary_chat_route_unchanged,
        migration_permission: false,
        ordinary_chat_calls_goal8_helpers: false,
        ordinary_chat_calls_golden_path_helpers: false,
    };
    let proposal_first_boundaries = ProposalFirstBoundaryProof {
        proposal_first_preserved: evidence.proposal_first_boundaries_preserved,
        external_writes_bypass_proposal: false,
        memory_writes_bypass_proposal: false,
        lifemodel_truth_writes_bypass_proposal: false,
    };
    let raw_content_exclusion = RawContentExclusionProof {
        raw_content_excluded: evidence.raw_content_excluded
            && input.contract_freeze.metadata_safe
            && !input.contract_freeze.contains_raw_content,
        raw_prompt_included: false,
        raw_user_text_included: false,
        raw_assistant_output_included: false,
        raw_memory_included: false,
        raw_life_model_included: false,
        raw_tool_payload_included: false,
        raw_guidance_included: false,
    };
    let local_only_privacy = LocalOnlyPrivacyProof {
        local_only_enforced: evidence.local_only_privacy_enforced,
        cloud_fallback_for_local_only: false,
        high_or_critical_privacy_local_only: evidence.local_only_privacy_enforced,
    };
    let tool_governance = ToolGovernanceProof {
        tool_governance_enforced: evidence.tool_governance_enforced,
        unsupported_plugins_blocked: evidence.tool_governance_enforced,
        disabled_declarative_tools_blocked: evidence.tool_governance_enforced,
        hs_write_like_paths_proposal_first: evidence.proposal_first_boundaries_preserved,
    };
    let golden_path_coverage = GoldenPathCoverageProof {
        weekly_planning_ready: evidence.golden_paths_ready,
        low_energy_support_ready: evidence.golden_paths_ready,
        preference_correction_ready: evidence.golden_paths_ready,
        ordinary_chat_uses_golden_paths: false,
        golden_path_ready_grants_migration: false,
    };
    let ui_read_model_contracts = UiReadModelGateProof {
        contract_frozen: input.contract_freeze.contract_frozen,
        learning_inbox_exists: input.contract_freeze.learning_inbox.metadata_safe,
        evidence_timeline_exists: input.contract_freeze.evidence_timeline.metadata_safe,
        proposal_review_exists: input.contract_freeze.proposal_review.metadata_safe,
        runtime_trace_includes_hs_influence: input
            .contract_freeze
            .runtime_trace
            .hs_influence
            .included,
        guidance_impact_exists: input.contract_freeze.guidance_impact.metadata_safe,
        privacy_controls_exists: input.contract_freeze.privacy_controls.metadata_safe,
        lifemodel_overview_exists: input.contract_freeze.lifemodel_overview.metadata_safe,
        version_diff_rollback_exists: input
            .contract_freeze
            .lifemodel_overview
            .provenance_traceable,
    };
    let acceptance_gates = build_acceptance_gates(
        &evidence,
        &input.contract_freeze,
        &raw_content_exclusion,
        &ui_read_model_contracts,
    );
    let blockers_by_gate = acceptance_gates
        .iter()
        .map(|gate| LifeModelBackendGateBlocker {
            gate: gate.gate.clone(),
            blockers: gate.blockers.clone(),
        })
        .collect::<Vec<_>>();
    let gate_ready = acceptance_gates.iter().all(|gate| gate.passed)
        && default_chat_isolation.default_chat_isolated
        && default_chat_isolation.ordinary_chat_route_unchanged
        && !default_chat_isolation.migration_permission
        && proposal_first_boundaries.proposal_first_preserved
        && raw_content_exclusion.raw_content_excluded
        && local_only_privacy.local_only_enforced
        && tool_governance.tool_governance_enforced
        && golden_path_coverage.weekly_planning_ready
        && golden_path_coverage.low_energy_support_ready
        && golden_path_coverage.preference_correction_ready;

    FinalBackendCompletionGateReport {
        report_kind: FINAL_GATE_REPORT_KIND.into(),
        gate_ready,
        metadata_safe: true,
        contains_raw_content: false,
        generated_at: input.generated_at,
        read_only: true,
        tauri_command_added: false,
        migration_permission: false,
        default_chat_isolation,
        proposal_first_boundaries,
        raw_content_exclusion,
        local_only_privacy,
        tool_governance,
        golden_path_coverage,
        ui_read_model_contracts,
        materialized_lifemodel_provenance_traceable: evidence
            .materialized_lifemodel_provenance_traceable,
        high_risk_auto_materialization_blocked: evidence.high_risk_auto_materialization_blocked,
        acceptance_gates,
        blockers_by_gate,
        remaining_beta_blockers: evidence.remaining_beta_blockers,
        business_write_count: 0,
        runtime_execution_count: 0,
        model_execution_count: 0,
        tool_execution_count: 0,
    }
}

fn build_learning_inbox(
    generated_at: DateTime<Utc>,
    timeline: &EvidenceTimelineReadModel,
    proposals: &[AgentProposal],
) -> LearningInboxReadModel {
    let mut items = timeline
        .items
        .iter()
        .map(|item| LearningInboxItem {
            item_id: item.evidence_id.clone(),
            item_kind: "evidence_candidate".into(),
            status: item.status.clone(),
            affected_path: item.affected_path.clone(),
            risk_level: item.risk_level.clone(),
            privacy_level: Some(item.privacy_level.clone()),
            confidence: Some(item.confidence),
            source_ref_count: item.source_ref_count,
            linked_proposal_ids: item.linked_proposal_ids.clone(),
            linked_agent_run_ids: item.linked_agent_run_ids.clone(),
            cluster_id: Some(item.cluster_id.clone()),
            payload_digest: digest_json(&serde_json::json!({
                "id": item.evidence_id,
                "type": item.evidence_type,
                "path": item.affected_path,
                "status": item.status,
                "confidence": item.confidence,
                "clusterHash": item.cluster_hash,
            })),
            created_at: Some(item.created_at),
        })
        .collect::<Vec<_>>();
    let candidate_evidence_count = items.len();
    let pending = proposals
        .iter()
        .filter(|proposal| proposal.status == ProposalStatus::Pending)
        .map(|proposal| LearningInboxItem {
            item_id: proposal.id.clone(),
            item_kind: "proposal".into(),
            status: proposal.status.to_string(),
            affected_path: proposal.affected_path.clone(),
            risk_level: proposal.risk_level.to_string(),
            privacy_level: None,
            confidence: Some(proposal.confidence),
            source_ref_count: proposal.run_id.is_some() as usize,
            linked_proposal_ids: vec![proposal.id.clone()],
            linked_agent_run_ids: proposal.run_id.iter().cloned().collect(),
            cluster_id: None,
            payload_digest: proposal_payload_digest(proposal),
            created_at: Some(proposal.created_at),
        })
        .collect::<Vec<_>>();
    let pending_proposal_count = pending.len();
    items.extend(pending);
    items.sort_by(|left, right| left.item_id.cmp(&right.item_id));

    LearningInboxReadModel {
        report_kind: "w147.learningInboxReadModel.v1".into(),
        metadata_safe: timeline.metadata_safe,
        contains_raw_content: false,
        generated_at,
        item_count: items.len(),
        candidate_evidence_count,
        pending_proposal_count,
        items,
    }
}

fn build_proposal_review(
    generated_at: DateTime<Utc>,
    proposals: &[AgentProposal],
) -> ProposalReviewReadModel {
    let mut items = proposals
        .iter()
        .map(|proposal| ProposalReviewItem {
            proposal_id: proposal.id.clone(),
            run_id: proposal.run_id.clone(),
            proposal_type: proposal.proposal_type.to_string(),
            source: proposal.source.to_string(),
            affected_path: proposal.affected_path.clone(),
            status: proposal.status.to_string(),
            risk_level: proposal.risk_level.to_string(),
            confidence: proposal.confidence,
            created_at: proposal.created_at,
            resolved_at: proposal.resolved_at,
            expires_at: proposal.expires_at,
            payload_digest: proposal_payload_digest(proposal),
            raw_values_included: false,
            raw_reason_included: false,
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.proposal_id.cmp(&right.proposal_id));
    let pending_count = proposals
        .iter()
        .filter(|proposal| proposal.status == ProposalStatus::Pending)
        .count();
    let high_risk_pending_count = proposals
        .iter()
        .filter(|proposal| {
            proposal.status == ProposalStatus::Pending
                && matches!(
                    proposal.risk_level,
                    crate::agent::RiskLevel::High | crate::agent::RiskLevel::Critical
                )
        })
        .count();

    ProposalReviewReadModel {
        report_kind: "w147.proposalReviewReadModel.v1".into(),
        metadata_safe: true,
        contains_raw_content: false,
        generated_at,
        proposal_count: proposals.len(),
        pending_count,
        high_risk_pending_count,
        proposal_first_review_required: true,
        items,
    }
}

fn build_runtime_trace(
    generated_at: DateTime<Utc>,
    runs: &[AgentRun],
    guidance_impact: &GuidanceImpactReadModel,
) -> RuntimeTraceReadModel {
    let guidance_ids = guidance_impact
        .guidance_refs
        .iter()
        .map(|guidance| guidance.guidance_id.clone())
        .collect::<Vec<_>>();
    let guidance_digests = guidance_impact
        .guidance_refs
        .iter()
        .map(|guidance| guidance.guidance_digest.clone())
        .collect::<Vec<_>>();
    let mut selected_policy_ids = guidance_impact.selected_policy_ids.clone();
    sort_dedup(&mut selected_policy_ids);
    let affected_surface_count = guidance_impact.affected_surfaces.len();
    let run_items = runs
        .iter()
        .map(|run| runtime_run_item(run, &guidance_ids))
        .collect::<Vec<_>>();
    let behavior_check_count = run_items
        .iter()
        .map(|run| run.behavior_check_count)
        .sum::<usize>()
        .max(guidance_impact.behavior_check_count);
    let passed_behavior_check_count = run_items
        .iter()
        .map(|run| run.passed_behavior_check_count)
        .sum::<usize>();
    let hs_influence = RuntimeTraceHsInfluence {
        included: !selected_policy_ids.is_empty()
            || !guidance_ids.is_empty()
            || behavior_check_count > 0,
        selected_policy_ids,
        selected_guidance_ids: guidance_ids,
        selected_guidance_digests: guidance_digests,
        behavior_check_count,
        passed_behavior_check_count,
        affected_surface_count,
    };

    RuntimeTraceReadModel {
        report_kind: "w147.runtimeTraceReadModel.v1".into(),
        metadata_safe: true,
        contains_raw_content: false,
        generated_at,
        run_count: run_items.len(),
        hs_influence,
        runs: run_items,
    }
}

fn runtime_run_item(run: &AgentRun, fallback_guidance_ids: &[String]) -> RuntimeTraceRunItem {
    let selected_policy_ids = run
        .hs_selection_audit
        .as_ref()
        .map(|audit| audit.selected_policy_ids.clone())
        .unwrap_or_default();
    let selected_guidance_ids = run
        .hs_selection_audit
        .as_ref()
        .map(|audit| audit.selected_guidance_ids.clone())
        .filter(|ids| !ids.is_empty())
        .unwrap_or_else(|| fallback_guidance_ids.to_vec());
    let behavior_check_count = run.behavior_checks.len();
    let passed_behavior_check_count = run
        .behavior_checks
        .iter()
        .filter(|check| check.passed)
        .count();
    let (redaction_applied, redaction_level, context_section_count, memory_hit_count) = run
        .context_summary
        .as_ref()
        .map(|context| {
            (
                Some(context.redaction_applied),
                Some(context.redaction_level.to_string()),
                context.included_life_model_sections.len(),
                Some(context.memory_hit_count),
            )
        })
        .unwrap_or((None, None, 0, None));
    let (model_provider, model_route_type, privacy_level) = run
        .model_route
        .as_ref()
        .map(|route| {
            (
                Some(route.provider.clone()),
                Some(route.route_type.clone()),
                Some(route.privacy_level.to_string()),
            )
        })
        .unwrap_or((None, None, None));

    RuntimeTraceRunItem {
        run_id: run.id.clone(),
        task_id: run.task_id.clone(),
        session_id: run.session_id.clone(),
        status: run.status,
        kind: run.kind,
        reasoning_strategy: run.reasoning_strategy.clone(),
        selected_policy_ids,
        selected_guidance_ids,
        generated_proposal_ids: run.generated_proposals.clone(),
        action_count: run.actions.len(),
        observation_count: run.observations.len(),
        warning_count: run.warnings.len(),
        behavior_check_count,
        passed_behavior_check_count,
        step_count: run.step_count,
        tool_call_count: run.tool_call_count,
        model_provider,
        model_route_type,
        privacy_level,
        redaction_applied,
        redaction_level,
        context_section_count,
        memory_hit_count,
        input_digest: run.user_input.as_ref().map(|value| digest_str(value)),
        output_digest: run.output_preview.as_ref().map(|value| digest_str(value)),
        raw_user_input_included: false,
        raw_output_included: false,
        raw_memory_included: false,
        started_at: run.started_at,
        finished_at: run.finished_at,
    }
}

fn build_privacy_controls(
    generated_at: DateTime<Utc>,
    guidance_impact: &GuidanceImpactReadModel,
    decisions: &[GovernorDecisionReport],
) -> PrivacyControlsReadModel {
    let mut selected_policy_ids = guidance_impact.selected_policy_ids.clone();
    for decision in decisions {
        selected_policy_ids.extend(decision.selected_policy_ids.clone());
    }
    sort_dedup(&mut selected_policy_ids);
    let decision_summaries = decisions
        .iter()
        .map(|decision| PrivacyDecisionSummary {
            subject: decision.subject,
            decision_kind: decision.decision_kind,
            classification: decision.classification,
            risk_level: decision.risk_level.to_string(),
            policy_reason_code: decision.policy_reason_code.clone(),
            requires_local_only: decision.requires_local_only,
            requires_proposal: decision.requires_proposal,
            requires_confirmation: decision.requires_confirmation,
            blocked: decision.blocked,
            selected_policy_ids: decision.selected_policy_ids.clone(),
            decision_digest: decision.decision_digest.clone(),
        })
        .collect::<Vec<_>>();
    let local_only_decision_count = decisions
        .iter()
        .filter(|decision| {
            decision.requires_local_only
                || decision.classification == GovernanceDecisionClassification::LocalOnly
                || decision.policy_reason_code.contains("local_only")
        })
        .count();
    let local_only_policy_visible = local_only_decision_count > 0
        || selected_policy_ids
            .iter()
            .any(|policy| policy.contains("local_only") || policy.contains("sensitive_topics"));
    let proposal_first_policy_visible = selected_policy_ids
        .iter()
        .any(|policy| policy.contains("proposal_first"))
        || decisions.iter().any(|decision| decision.requires_proposal);

    PrivacyControlsReadModel {
        report_kind: "w147.privacyControlsReadModel.v1".into(),
        metadata_safe: decisions.iter().all(|decision| decision.metadata_safe),
        contains_raw_content: false,
        generated_at,
        local_only_policy_visible,
        local_only_decision_count,
        proposal_first_policy_visible,
        raw_content_exclusion_visible: !guidance_impact.raw_prompt_included
            && !guidance_impact.raw_user_text_included
            && !guidance_impact.raw_assistant_output_included
            && !guidance_impact.raw_memory_included
            && !guidance_impact.raw_life_model_included
            && !guidance_impact.raw_tool_payload_included
            && !guidance_impact.raw_guidance_included,
        selected_policy_ids,
        decisions: decision_summaries,
    }
}

fn build_lifemodel_overview(
    generated_at: DateTime<Utc>,
    version: &LifeModelVersionReadModel,
) -> LifeModelOverviewReadModel {
    let provenance = &version.provenance;
    let provenance_traceable = !version.materialized_view_source_digest.trim().is_empty()
        && !version
            .materialized_view_provenance_digest
            .trim()
            .is_empty()
        && !provenance.provenance_digest.trim().is_empty();

    LifeModelOverviewReadModel {
        report_kind: "w147.lifeModelOverviewReadModel.v1".into(),
        metadata_safe: version.metadata_safe,
        contains_raw_content: false,
        generated_at,
        from_version_id: version.from_version_id.clone(),
        to_version_id: version.to_version_id.clone(),
        compatibility_materialized_view: provenance.compatibility_materialized_view,
        accepted_source_of_truth: provenance.accepted_source_of_truth,
        durable_truth_materialized: provenance.durable_truth_materialized,
        proposal_first_required_for_truth: provenance.proposal_first_required_for_truth,
        provenance_traceable,
        source_proposal_count: provenance.source_proposal_ids.len(),
        source_evidence_count: provenance.source_evidence_ids.len(),
        source_patch_count: provenance.source_patch_ids.len(),
        source_heuristic_count: provenance.source_heuristic_ids.len(),
        accepted_guidance_count: version.accepted_guidance_refs.len(),
        changed_asset_count: version.changed_asset_refs.len(),
        rollback_available: version.rollback_reference.is_some(),
        materialized_view_source_digest: version.materialized_view_source_digest.clone(),
        materialized_view_provenance_digest: version.materialized_view_provenance_digest.clone(),
        diff_reference_digest: version.diff_reference_digest.clone(),
        rollback_reference_digest: version.rollback_reference_digest.clone(),
        raw_life_model_fields_included: false,
        raw_guidance_included: false,
    }
}

fn build_acceptance_gates(
    evidence: &BackendCompletionGateEvidence,
    contract: &PreUiBackendContractFreezeReport,
    raw_content: &RawContentExclusionProof,
    ui_contracts: &UiReadModelGateProof,
) -> Vec<BackendCompletionAcceptanceGateStatus> {
    vec![
        gate_status(
            "lifemodel_maturity_gate",
            evidence.lifemodel_maturity_gate_passed
                && evidence.materialized_lifemodel_provenance_traceable,
            vec![
                (
                    !evidence.lifemodel_maturity_gate_passed,
                    "lifemodel_maturity_gate_not_proven",
                ),
                (
                    !evidence.materialized_lifemodel_provenance_traceable,
                    "materialized_lifemodel_provenance_not_traceable",
                ),
            ],
        ),
        gate_status(
            "runtime_driven_gate",
            evidence.runtime_driven_gate_passed
                && evidence.local_only_privacy_enforced
                && evidence.tool_governance_enforced
                && evidence.golden_paths_ready,
            vec![
                (
                    !evidence.runtime_driven_gate_passed,
                    "runtime_driven_gate_not_proven",
                ),
                (
                    !evidence.local_only_privacy_enforced,
                    "local_only_privacy_not_enforced",
                ),
                (
                    !evidence.tool_governance_enforced,
                    "tool_governance_not_enforced",
                ),
                (!evidence.golden_paths_ready, "golden_paths_not_ready"),
            ],
        ),
        gate_status(
            "governance_privacy_gate",
            evidence.governance_privacy_gate_passed
                && evidence.proposal_first_boundaries_preserved
                && raw_content.raw_content_excluded
                && evidence.high_risk_auto_materialization_blocked,
            vec![
                (
                    !evidence.governance_privacy_gate_passed,
                    "governance_privacy_gate_not_proven",
                ),
                (
                    !evidence.proposal_first_boundaries_preserved,
                    "proposal_first_boundary_not_preserved",
                ),
                (
                    !raw_content.raw_content_excluded,
                    "raw_content_not_excluded",
                ),
                (
                    !evidence.high_risk_auto_materialization_blocked,
                    "high_risk_auto_materialization_not_blocked",
                ),
            ],
        ),
        gate_status(
            "ui_read_model_gate",
            evidence.ui_read_model_gate_passed
                && contract.contract_frozen
                && ui_contracts.learning_inbox_exists
                && ui_contracts.evidence_timeline_exists
                && ui_contracts.proposal_review_exists
                && ui_contracts.runtime_trace_includes_hs_influence
                && ui_contracts.guidance_impact_exists
                && ui_contracts.privacy_controls_exists
                && ui_contracts.lifemodel_overview_exists
                && ui_contracts.version_diff_rollback_exists,
            vec![
                (
                    !evidence.ui_read_model_gate_passed,
                    "ui_read_model_gate_not_proven",
                ),
                (!contract.contract_frozen, "pre_ui_contract_not_frozen"),
                (
                    !ui_contracts.learning_inbox_exists,
                    "learning_inbox_read_model_missing",
                ),
                (
                    !ui_contracts.evidence_timeline_exists,
                    "evidence_timeline_read_model_missing",
                ),
                (
                    !ui_contracts.proposal_review_exists,
                    "proposal_review_read_model_missing",
                ),
                (
                    !ui_contracts.runtime_trace_includes_hs_influence,
                    "runtime_trace_hs_influence_missing",
                ),
                (
                    !ui_contracts.guidance_impact_exists,
                    "guidance_impact_read_model_missing",
                ),
                (
                    !ui_contracts.privacy_controls_exists,
                    "privacy_controls_read_model_missing",
                ),
                (
                    !ui_contracts.lifemodel_overview_exists,
                    "lifemodel_overview_read_model_missing",
                ),
                (
                    !ui_contracts.version_diff_rollback_exists,
                    "version_diff_rollback_read_model_missing",
                ),
            ],
        ),
    ]
}

fn gate_status(
    gate: &str,
    passed: bool,
    possible_blockers: Vec<(bool, &'static str)>,
) -> BackendCompletionAcceptanceGateStatus {
    BackendCompletionAcceptanceGateStatus {
        gate: gate.into(),
        passed,
        blockers: possible_blockers
            .into_iter()
            .filter(|(active, _)| *active)
            .map(|(_, blocker)| blocker.to_string())
            .collect(),
    }
}

fn surface_contracts() -> Vec<UiReadModelSurfaceContract> {
    vec![
        surface_contract(
            "learning_inbox",
            "w147.learningInboxReadModel",
            &["EvidenceTimelineReadModel", "ProposalReviewReadModel"],
        ),
        surface_contract(
            "evidence_timeline",
            "w130.evidenceTimelineReadModel",
            &["EvidenceTimelineReadModel"],
        ),
        surface_contract(
            "proposal_review",
            "w147.proposalReviewReadModel",
            &["AgentProposal"],
        ),
        surface_contract(
            "runtime_trace",
            "w147.runtimeTraceReadModel",
            &["AgentRun", "GuidanceImpactReadModel", "HSSelectionAudit"],
        ),
        surface_contract(
            "guidance_impact",
            "w140.guidanceImpactReadModel",
            &["GuidanceImpactReadModel"],
        ),
        surface_contract(
            "privacy_controls",
            "w147.privacyControlsReadModel",
            &["GovernorDecisionReport", "GuidanceImpactReadModel"],
        ),
        surface_contract(
            "lifemodel_overview",
            "w147.lifeModelOverviewReadModel",
            &[
                "LifeModelVersionReadModel",
                "LifeModelMaterializedViewProvenance",
            ],
        ),
    ]
}

fn surface_contract(
    surface: &str,
    schema_name: &str,
    source_contracts: &[&str],
) -> UiReadModelSurfaceContract {
    UiReadModelSurfaceContract {
        surface: surface.into(),
        schema_name: schema_name.into(),
        schema_version: "v1".into(),
        stable_for_pre_ui_design: true,
        metadata_safe: true,
        contains_raw_content: false,
        read_only: true,
        command_surface_required: false,
        source_contracts: source_contracts
            .iter()
            .map(|value| (*value).into())
            .collect(),
    }
}

fn proposal_payload_digest(proposal: &AgentProposal) -> String {
    digest_json(&serde_json::json!({
        "proposalId": proposal.id,
        "proposalType": proposal.proposal_type.to_string(),
        "source": proposal.source.to_string(),
        "path": proposal.affected_path,
        "status": proposal.status.to_string(),
        "risk": proposal.risk_level.to_string(),
        "confidence": proposal.confidence,
        "before": proposal.before,
        "after": proposal.after,
        "reason": proposal.reason,
    }))
}

fn governor_report_raw_flag(report: &GovernorDecisionReport) -> bool {
    report.raw_prompt_included
        || report.raw_user_text_included
        || report.raw_assistant_output_included
        || report.raw_memory_included
        || report.raw_life_model_included
        || report.raw_tool_payload_included
}

fn digest_json(value: &serde_json::Value) -> String {
    digest_str(&value.to_string())
}

fn digest_str(value: &str) -> String {
    let hash = digest(&SHA256, value.as_bytes());
    format!(
        "sha256:{}",
        hash.as_ref()
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>()
    )
}

fn sort_dedup(values: &mut Vec<String>) {
    let set = values.drain(..).collect::<BTreeSet<_>>();
    values.extend(set);
}
