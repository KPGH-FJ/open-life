use crate::AppState;
use openlife_core::agent::ReasoningTrace;
use openlife_core::agent::{
    behavior_checks_for_packet, AgentExecutionBudget, AgentRun, AgentRunError, AgentRunStatus,
    AgentRuntime, AgentTask, AgentTaskKind, ContextSummary, ControlledChatPilotEligibilityReport,
    EvidenceDraft, EvidencePrivacyLevel, EvidenceQuery, EvidenceSourceRef, EvidenceSourceType,
    EvidenceType, GovernanceDecisionKind, HSBehaviorCheckSummary, HSSelectionAudit,
    MultiStrategyRuntime, MultiStrategyRuntimeInput, MultiStrategyRuntimeOutput,
    MultiStrategyRuntimePayload, PlanExecutionOutput, PlanStepStatus, RedactionLevel, RiskLevel,
    RuntimeInput, RuntimeMigrationGateReport, RuntimeStrategyKind,
    DEFAULT_CONTROLLED_CHAT_PILOT_REQUIRED_CLEAN_RUNS,
};
use openlife_core::layer_router::Layer;
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::State;

const CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH: &str = "runtime.controlled_pilot.promotion";
const CONTROLLED_PILOT_PROMOTION_BLOCK_PATH: &str = "runtime.controlled_pilot.promotion_block";
const CONTROLLED_CHAT_MIGRATION_REVIEW_DECISION_EVIDENCE_PATH: &str =
    "runtime.controlled_chat.migration_review_decision";
const CONTROLLED_CHAT_MIGRATION_SHADOW_REVIEW_DECISION_EVIDENCE_PATH: &str =
    "runtime.controlled_chat.migration_shadow_review_decision";
const CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH: &str =
    "runtime.controlled_chat.cutover_candidate_review_decision";
const DEFAULT_CHAT_ADAPTER_ACTIVATION_REVIEW_DECISION_EVIDENCE_PATH: &str =
    "runtime.default_chat.adapter_activation_review_decision";
const DEFAULT_CHAT_ADAPTER_DRY_RUN_REVIEW_DECISION_EVIDENCE_PATH: &str =
    "runtime.default_chat.adapter_dry_run_review_decision";
const RECENT_PROMOTION_EVIDENCE_LIMIT: usize = 5;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiStrategyAgentPreviewInput {
    pub session_id: String,
    pub user_text: String,
    #[serde(default)]
    pub tools_prompt: String,
    #[serde(default)]
    pub allow_planning: bool,
    #[serde(default)]
    pub local_model_available: bool,
    #[serde(default)]
    pub layer: Option<String>,
    #[serde(default)]
    pub execution_budget: Option<MultiStrategyAgentPreviewExecutionBudgetInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiStrategyAgentPreviewExecutionBudgetInput {
    pub max_steps: Option<u32>,
    pub max_tool_calls: Option<u32>,
    pub timeout_seconds: Option<u64>,
    pub allow_cloud: Option<bool>,
    pub allow_writes: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiStrategyAgentPreviewOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub strategy_kind: String,
    pub payload_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<Value>,
    pub proposal_ids: Vec<String>,
    pub warnings: Vec<String>,
    pub metadata_safe_summary: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub governance_decision_kind: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMigrationGateCheckInput {
    #[serde(default)]
    pub preview_run_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatPilotEligibilityCheckInput {
    #[serde(default)]
    pub required_clean_runs: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledPilotPromotionEvidenceInput {
    pub pilot_run_id: String,
    pub source_session_id: String,
    pub target_session_id: String,
    pub strategy_kind: String,
    pub payload_kind: String,
    #[serde(default)]
    pub governance_decision_kind: Option<String>,
    pub promoted_message_length: usize,
    pub promoted_message_hash: String,
    #[serde(default)]
    pub promoted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledPilotPromotionEvidenceResult {
    pub evidence_id: String,
    pub created: bool,
    pub pilot_run_id: String,
    pub promoted_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledPilotPromotionEvidenceSummary {
    pub promoted_count: usize,
    pub recent_promoted_pilot_run_ids: Vec<String>,
    pub latest_promotion_timestamp: Option<String>,
    pub source_target_mismatch_block_count: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledPilotPromotionReadinessCheckInput {
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledPilotPromotionReadinessReport {
    pub ready: bool,
    pub required_promotions: usize,
    pub promoted_count: usize,
    pub recent_promoted_pilot_run_ids: Vec<String>,
    pub latest_promotion_timestamp: Option<String>,
    pub source_target_mismatch_block_count: usize,
    pub metadata_safe_evidence_ready: bool,
    pub default_chat_unchanged: bool,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationPlanDraftInput {
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationPlanDraft {
    pub draft_ready: bool,
    pub readiness_report: ControlledPilotPromotionReadinessReport,
    pub migration_scope: Vec<String>,
    pub required_preconditions: Vec<String>,
    pub rollback_plan: Vec<String>,
    pub fallback_plan: Vec<String>,
    pub test_plan: Vec<String>,
    pub manual_review_required: bool,
    pub not_automatic_migration: bool,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationReviewDecisionInput {
    pub decision_kind: String,
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub optional_reviewer_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationReviewDecisionResult {
    pub recorded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_id: Option<String>,
    pub decision_kind: String,
    pub draft_ready: bool,
    pub draft_hash: String,
    pub created_at: String,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationReviewLatestDecision {
    pub evidence_id: String,
    pub decision_kind: String,
    pub draft_ready: bool,
    pub draft_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationReviewDecisionSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_decision: Option<ControlledChatMigrationReviewLatestDecision>,
    pub approved_count: usize,
    pub rework_reject_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_timestamp: Option<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationImplementationGateInput {
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationImplementationGateReport {
    pub implementation_eligible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_decision: Option<ControlledChatMigrationReviewLatestDecision>,
    pub readiness_report: ControlledPilotPromotionReadinessReport,
    pub draft_hash_matched: bool,
    pub approved_after_latest_draft: bool,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationShadowRunInput {
    pub session_id: String,
    #[serde(default)]
    pub user_input_checksum: Option<String>,
    #[serde(default)]
    pub bounded_test_prompt_descriptor: Option<String>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationShadowRunOutput {
    pub shadow_run_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_run_id: Option<String>,
    pub implementation_gate_report: ControlledChatMigrationImplementationGateReport,
    pub strategy_kind: String,
    pub payload_kind: String,
    pub metadata_safe_summary: Value,
    pub warnings: Vec<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationShadowReviewDecisionInput {
    pub shadow_run_id: String,
    pub decision_kind: String,
    #[serde(default)]
    pub optional_reviewer_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationShadowReviewDecisionResult {
    pub recorded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_id: Option<String>,
    pub shadow_run_id: String,
    pub decision_kind: String,
    pub readiness_summary_digest: String,
    pub created_at: String,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationShadowReviewLatestDecision {
    pub evidence_id: String,
    pub shadow_run_id: String,
    pub decision_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer_note_checksum: Option<String>,
    pub reviewer_note_length: usize,
    pub reviewer_note_category: String,
    pub readiness_summary_digest: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatMigrationShadowReviewSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_decision: Option<ControlledChatMigrationShadowReviewLatestDecision>,
    pub approved_count: usize,
    pub rework_reject_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_timestamp: Option<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverReadinessInput {
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverReadinessReport {
    pub cutover_planning_eligible: bool,
    pub implementation_gate_report: ControlledChatMigrationImplementationGateReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_shadow_review_decision: Option<ControlledChatMigrationShadowReviewLatestDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_shadow_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness_summary_digest: Option<String>,
    pub default_chat_unchanged: bool,
    pub required_evidence_ready: bool,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverCandidateInput {
    pub session_id: String,
    #[serde(default)]
    pub user_input_checksum: Option<String>,
    #[serde(default)]
    pub bounded_test_prompt_descriptor: Option<String>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverCandidateOutput {
    pub candidate_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_output: Option<String>,
    pub contract_shape: String,
    pub metadata_safe_summary: Value,
    pub warnings: Vec<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverCandidateReviewDecisionInput {
    pub candidate_run_id: String,
    pub decision_kind: String,
    #[serde(default)]
    pub optional_reviewer_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverCandidateReviewDecisionResult {
    pub recorded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_id: Option<String>,
    pub candidate_run_id: String,
    pub decision_kind: String,
    pub contract_shape: String,
    pub candidate_summary_digest: String,
    pub created_at: String,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverCandidateReviewLatestDecision {
    pub evidence_id: String,
    pub candidate_run_id: String,
    pub decision_kind: String,
    pub contract_shape: String,
    pub candidate_summary_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer_note_checksum: Option<String>,
    pub reviewer_note_length: usize,
    pub reviewer_note_category: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverCandidateReviewSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_decision: Option<ControlledChatCutoverCandidateReviewLatestDecision>,
    pub approved_count: usize,
    pub rework_reject_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_timestamp: Option<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverCandidatePromotionReadinessInput {
    #[serde(default)]
    pub required_approved_candidates: Option<usize>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverCandidatePromotionApprovedCandidate {
    pub evidence_id: String,
    pub candidate_run_id: String,
    pub contract_shape: String,
    pub candidate_summary_digest: String,
    pub run_readiness_digest: String,
    pub decision_created_at: String,
    pub ready: bool,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledChatCutoverCandidatePromotionReadinessReport {
    pub ready: bool,
    pub cutover_readiness_eligible: bool,
    pub required_approved_candidates: usize,
    pub approved_candidate_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_decision: Option<ControlledChatCutoverCandidateReviewLatestDecision>,
    pub approved_candidates: Vec<ControlledChatCutoverCandidatePromotionApprovedCandidate>,
    pub default_chat_unchanged: bool,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatRuntimeBoundaryStatus {
    pub current_mode: String,
    pub controlled_candidate_available: bool,
    pub default_chat_unchanged: bool,
    pub candidate_promotion_readiness_required: bool,
    pub automatic_migration_enabled: bool,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterActivationPlanDraftInput {
    #[serde(default)]
    pub required_approved_candidates: Option<usize>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterActivationPlanDraft {
    pub draft_ready: bool,
    pub candidate_promotion_readiness_report:
        ControlledChatCutoverCandidatePromotionReadinessReport,
    pub runtime_boundary_status: DefaultChatRuntimeBoundaryStatus,
    pub activation_scope: Vec<String>,
    pub required_preconditions: Vec<String>,
    pub adapter_contract_checks: Vec<String>,
    pub fallback_plan: Vec<String>,
    pub rollback_plan: Vec<String>,
    pub observability_plan: Vec<String>,
    pub test_plan: Vec<String>,
    pub manual_review_required: bool,
    pub not_automatic_migration: bool,
    pub requires_separate_implementation: bool,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterActivationReviewDecisionInput {
    pub decision_kind: String,
    #[serde(default)]
    pub required_approved_candidates: Option<usize>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub optional_reviewer_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterActivationReviewDecisionResult {
    pub recorded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_id: Option<String>,
    pub decision_kind: String,
    pub draft_ready: bool,
    pub activation_plan_digest: String,
    pub created_at: String,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterActivationReviewLatestDecision {
    pub evidence_id: String,
    pub decision_kind: String,
    pub draft_ready: bool,
    pub activation_plan_digest: String,
    pub candidate_promotion_ready: bool,
    pub current_mode: String,
    pub automatic_migration_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer_note_checksum: Option<String>,
    pub reviewer_note_length: usize,
    pub reviewer_note_category: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterActivationReviewSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_decision: Option<DefaultChatAdapterActivationReviewLatestDecision>,
    pub approved_count: usize,
    pub reject_or_rework_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_timestamp: Option<String>,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterActivationImplementationGateInput {
    #[serde(default)]
    pub required_approved_candidates: Option<usize>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterActivationImplementationGateReport {
    pub implementation_gate_eligible: bool,
    pub draft_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_decision: Option<DefaultChatAdapterActivationReviewLatestDecision>,
    pub current_activation_plan_digest: String,
    pub activation_plan_digest_matched: bool,
    pub default_chat_unchanged: bool,
    pub automatic_migration_enabled: bool,
    pub current_mode: String,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterRoutingStatusInput {
    #[serde(default)]
    pub required_approved_candidates: Option<usize>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterRoutingStatus {
    pub current_mode: String,
    pub adapter_scaffold_present: bool,
    pub controlled_adapter_enabled: bool,
    pub default_send_path: String,
    pub start_stream_path: String,
    pub activation_implementation_gate_eligible: bool,
    pub requires_separate_cutover_implementation: bool,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterContractHarnessInput {
    #[serde(default)]
    pub required_approved_candidates: Option<usize>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterContractCheck {
    pub name: String,
    pub ready: bool,
    pub expected_path: String,
    pub actual_path: String,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterContractHarnessReport {
    pub contract_harness_ready: bool,
    pub contract_shape: String,
    pub adapter_disabled: bool,
    pub activation_implementation_gate_eligible: bool,
    pub routing_status: DefaultChatAdapterRoutingStatus,
    pub send_message_contract: DefaultChatAdapterContractCheck,
    pub stream_message_contract: DefaultChatAdapterContractCheck,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterDryRunInput {
    pub session_id: String,
    pub message: String,
    #[serde(default)]
    pub required_approved_candidates: Option<usize>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterDryRunReport {
    pub dry_run_ready: bool,
    pub blocked: bool,
    pub contract_shape: String,
    pub source_session_id: String,
    pub adapter_path: String,
    pub allow_writes: bool,
    pub max_tool_calls: u32,
    pub default_chat_path_unchanged: bool,
    pub chat_message_saved: bool,
    pub agent_run_recorded: bool,
    pub contract_harness_ready: bool,
    pub input_message_length: usize,
    pub input_message_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_output_preview: Option<String>,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterDryRunReviewDecisionInput {
    pub decision_kind: String,
    pub source_session_id: String,
    pub message: String,
    #[serde(default)]
    pub dry_run_summary_digest: Option<String>,
    #[serde(default)]
    pub required_approved_candidates: Option<usize>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
    #[serde(default)]
    pub optional_reviewer_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterDryRunReviewDecisionResult {
    pub recorded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_id: Option<String>,
    pub decision_kind: String,
    pub source_session_id: String,
    pub contract_shape: String,
    pub dry_run_ready: bool,
    pub dry_run_summary_digest: String,
    pub created_at: String,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterDryRunReviewLatestDecision {
    pub evidence_id: String,
    pub decision_kind: String,
    pub source_session_id: String,
    pub contract_shape: String,
    pub dry_run_ready: bool,
    pub dry_run_summary_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewer_note_checksum: Option<String>,
    pub reviewer_note_length: usize,
    pub reviewer_note_category: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterDryRunReviewSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_decision: Option<DefaultChatAdapterDryRunReviewLatestDecision>,
    pub approved_count: usize,
    pub reject_or_rework_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_timestamp: Option<String>,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterImplementationReadinessInput {
    pub source_session_id: String,
    pub message: String,
    #[serde(default)]
    pub required_approved_candidates: Option<usize>,
    #[serde(default)]
    pub required_promotions: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultChatAdapterImplementationReadinessReport {
    pub implementation_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_dry_run_review_decision: Option<DefaultChatAdapterDryRunReviewLatestDecision>,
    pub activation_implementation_gate_eligible: bool,
    pub contract_harness_ready: bool,
    pub dry_run_ready: bool,
    pub dry_run_review_approved: bool,
    pub dry_run_digest_matched: bool,
    pub default_chat_unchanged: bool,
    pub controlled_adapter_enabled: bool,
    pub automatic_migration_enabled: bool,
    pub default_send_path: String,
    pub start_stream_path: String,
    pub blocking_reasons: Vec<String>,
    pub metadata_safe_summary: Value,
}

#[tauri::command]
pub async fn check_runtime_migration_gate(
    input: RuntimeMigrationGateCheckInput,
    state: State<'_, Arc<AppState>>,
) -> Result<RuntimeMigrationGateReport, String> {
    check_runtime_migration_gate_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn check_runtime_migration_gate_with_state(
    input: RuntimeMigrationGateCheckInput,
    state: &Arc<AppState>,
) -> Result<RuntimeMigrationGateReport, String> {
    let preview_run = find_preview_run_for_gate(input, state).await?;
    Ok(openlife_core::agent::evaluate_runtime_migration_gate(
        openlife_core::agent::RuntimeMigrationGateInput {
            default_chat_uses_multi_strategy: false,
            preview_run: preview_run.as_ref(),
            fallback_available: true,
        },
    ))
}

#[tauri::command]
pub async fn check_controlled_chat_pilot_eligibility(
    input: ControlledChatPilotEligibilityCheckInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatPilotEligibilityReport, String> {
    check_controlled_chat_pilot_eligibility_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn check_controlled_chat_pilot_eligibility_with_state(
    input: ControlledChatPilotEligibilityCheckInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatPilotEligibilityReport, String> {
    let required_clean_runs = input
        .required_clean_runs
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CONTROLLED_CHAT_PILOT_REQUIRED_CLEAN_RUNS);
    let preview_runs =
        find_preview_runs_for_pilot_eligibility(&input, required_clean_runs, state).await?;

    Ok(
        openlife_core::agent::evaluate_controlled_chat_pilot_eligibility(
            openlife_core::agent::ControlledChatPilotEligibilityInput {
                default_chat_uses_multi_strategy: false,
                preview_runs: &preview_runs,
                required_clean_runs,
                fallback_available: true,
            },
        ),
    )
}

#[tauri::command]
pub async fn record_controlled_pilot_promotion_evidence(
    input: ControlledPilotPromotionEvidenceInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledPilotPromotionEvidenceResult, String> {
    record_controlled_pilot_promotion_evidence_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn record_controlled_pilot_promotion_evidence_with_state(
    input: ControlledPilotPromotionEvidenceInput,
    state: &Arc<AppState>,
) -> Result<ControlledPilotPromotionEvidenceResult, String> {
    let evidence = normalize_promotion_evidence_input(input)?;
    let store = state.evidence_store.lock().await;
    let existing = store
        .query(EvidenceQuery {
            affected_path: Some(CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH.into()),
            evidence_type: Some(EvidenceType::RuntimeBehavior),
            linked_agent_run_id: Some(evidence.pilot_run_id.clone()),
            ..EvidenceQuery::default()
        })
        .map_err(|e| format!("failed to query controlled pilot promotion evidence: {e}"))?;

    if let Some(record) = existing.first() {
        let existing_hash = record
            .run_metadata
            .get("promotedMessageHash")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if existing_hash != evidence.promoted_message_hash {
            return Err(
                "promotion evidence already exists for pilotRunId with a different checksum".into(),
            );
        }
        let promoted_at = record
            .run_metadata
            .get("promotedAt")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| record.created_at.to_rfc3339());
        return Ok(ControlledPilotPromotionEvidenceResult {
            evidence_id: record.id.clone(),
            created: false,
            pilot_run_id: evidence.pilot_run_id,
            promoted_at,
        });
    }

    let metadata = json!({
        "evidenceKind": "controlled_pilot_promotion",
        "pilotRunId": evidence.pilot_run_id.clone(),
        "sourceSessionId": evidence.source_session_id.clone(),
        "targetSessionId": evidence.target_session_id.clone(),
        "strategyKind": evidence.strategy_kind.clone(),
        "payloadKind": evidence.payload_kind.clone(),
        "governanceDecisionKind": evidence.governance_decision_kind.clone(),
        "promotedMessageLength": evidence.promoted_message_length,
        "promotedMessageHash": evidence.promoted_message_hash.clone(),
        "promotedAt": evidence.promoted_at.clone(),
        "metadataSafe": true,
        "contentStorage": "checksum_only",
        "toolStorage": "none"
    });
    let draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary("Controlled pilot response promoted to chat history")
    .with_source_ref(EvidenceSourceRef::from_digest(
        EvidenceSourceType::AgentRun,
        &evidence.pilot_run_id,
        Some("controlled_pilot_promotion"),
        &evidence.promoted_message_hash,
    ))
    .with_linked_agent_run(evidence.pilot_run_id.clone());
    let mut draft = draft;
    draft.run_metadata = metadata;

    let record = store
        .create_evidence(draft)
        .map_err(|e| format!("failed to record controlled pilot promotion evidence: {e}"))?;

    Ok(ControlledPilotPromotionEvidenceResult {
        evidence_id: record.id,
        created: true,
        pilot_run_id: evidence.pilot_run_id,
        promoted_at: evidence.promoted_at,
    })
}

#[tauri::command]
pub async fn get_controlled_pilot_promotion_evidence_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledPilotPromotionEvidenceSummary, String> {
    get_controlled_pilot_promotion_evidence_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_controlled_pilot_promotion_evidence_summary_with_state(
    state: &Arc<AppState>,
) -> Result<ControlledPilotPromotionEvidenceSummary, String> {
    let store = state.evidence_store.lock().await;
    let promotions = store
        .query(EvidenceQuery {
            affected_path: Some(CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH.into()),
            evidence_type: Some(EvidenceType::RuntimeBehavior),
            ..EvidenceQuery::default()
        })
        .map_err(|e| format!("failed to read controlled pilot promotion evidence: {e}"))?;
    let mismatch_blocks = store
        .query(EvidenceQuery {
            affected_path: Some(CONTROLLED_PILOT_PROMOTION_BLOCK_PATH.into()),
            evidence_type: Some(EvidenceType::RuntimeBehavior),
            ..EvidenceQuery::default()
        })
        .map_err(|e| format!("failed to read controlled pilot promotion block evidence: {e}"))?;

    let recent_promoted_pilot_run_ids = promotions
        .iter()
        .filter_map(promotion_evidence_pilot_run_id)
        .take(RECENT_PROMOTION_EVIDENCE_LIMIT)
        .collect();
    let latest_promotion_timestamp = promotions.first().map(promotion_evidence_timestamp);

    Ok(ControlledPilotPromotionEvidenceSummary {
        promoted_count: promotions.len(),
        recent_promoted_pilot_run_ids,
        latest_promotion_timestamp,
        source_target_mismatch_block_count: mismatch_blocks.len(),
    })
}

#[tauri::command]
pub async fn check_controlled_pilot_promotion_readiness(
    input: ControlledPilotPromotionReadinessCheckInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledPilotPromotionReadinessReport, String> {
    check_controlled_pilot_promotion_readiness_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn check_controlled_pilot_promotion_readiness_with_state(
    input: ControlledPilotPromotionReadinessCheckInput,
    state: &Arc<AppState>,
) -> Result<ControlledPilotPromotionReadinessReport, String> {
    let required_promotions = input
        .required_promotions
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CONTROLLED_CHAT_PILOT_REQUIRED_CLEAN_RUNS);
    let _session_scope_is_global_for_now = input
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let store = state.evidence_store.lock().await;
    let promotions = store
        .query(EvidenceQuery {
            affected_path: Some(CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH.into()),
            evidence_type: Some(EvidenceType::RuntimeBehavior),
            ..EvidenceQuery::default()
        })
        .map_err(|e| format!("failed to read controlled pilot promotion evidence: {e}"))?;
    let mismatch_blocks = store
        .query(EvidenceQuery {
            affected_path: Some(CONTROLLED_PILOT_PROMOTION_BLOCK_PATH.into()),
            evidence_type: Some(EvidenceType::RuntimeBehavior),
            ..EvidenceQuery::default()
        })
        .map_err(|e| format!("failed to read controlled pilot promotion block evidence: {e}"))?;

    let promoted_count = promotions.len();
    let recent_promoted_pilot_run_ids = promotions
        .iter()
        .filter_map(promotion_evidence_pilot_run_id)
        .take(RECENT_PROMOTION_EVIDENCE_LIMIT)
        .collect();
    let latest_promotion_timestamp = promotions.first().map(promotion_evidence_timestamp);
    let metadata_safe_evidence_ready =
        !promotions.is_empty() && promotions.iter().all(promotion_evidence_is_metadata_safe);
    let default_chat_unchanged = true;

    let mut blocking_reasons = Vec::new();
    if promoted_count < required_promotions {
        push_unique_string(
            &mut blocking_reasons,
            format!(
                "insufficient_promotion_evidence: required {required_promotions} promotions, found {promoted_count}"
            ),
        );
    }
    if !metadata_safe_evidence_ready {
        push_unique_string(
            &mut blocking_reasons,
            "promotion_evidence_not_metadata_safe".to_string(),
        );
    }
    if !mismatch_blocks.is_empty() {
        push_unique_string(
            &mut blocking_reasons,
            "source_target_mismatch_blocks_present".to_string(),
        );
    }

    let ready = default_chat_unchanged
        && promoted_count >= required_promotions
        && metadata_safe_evidence_ready
        && mismatch_blocks.is_empty()
        && blocking_reasons.is_empty();

    Ok(ControlledPilotPromotionReadinessReport {
        ready,
        required_promotions,
        promoted_count,
        recent_promoted_pilot_run_ids,
        latest_promotion_timestamp,
        source_target_mismatch_block_count: mismatch_blocks.len(),
        metadata_safe_evidence_ready,
        default_chat_unchanged,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn draft_controlled_chat_migration_plan(
    input: ControlledChatMigrationPlanDraftInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationPlanDraft, String> {
    draft_controlled_chat_migration_plan_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn draft_controlled_chat_migration_plan_with_state(
    input: ControlledChatMigrationPlanDraftInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationPlanDraft, String> {
    let readiness_report = check_controlled_pilot_promotion_readiness_with_state(
        ControlledPilotPromotionReadinessCheckInput {
            required_promotions: input.required_promotions,
            session_id: input.session_id,
        },
        state,
    )
    .await?;

    let blocking_reasons = readiness_report.blocking_reasons.clone();
    if !readiness_report.ready {
        return Ok(ControlledChatMigrationPlanDraft {
            draft_ready: false,
            readiness_report,
            migration_scope: Vec::new(),
            required_preconditions: Vec::new(),
            rollback_plan: Vec::new(),
            fallback_plan: Vec::new(),
            test_plan: Vec::new(),
            manual_review_required: true,
            not_automatic_migration: true,
            blocking_reasons,
        });
    }

    Ok(ControlledChatMigrationPlanDraft {
        draft_ready: true,
        readiness_report,
        migration_scope: vec![
            "Draft scope is limited to a human-reviewed controlled pilot discussion; default Chat remains unchanged.".into(),
            "No default runtime feature flag is enabled or modified by this draft.".into(),
            "No LifeModel, Memory, Proposal, AgentRun, full tool call data, or promotion evidence write is part of this draft.".into(),
        ],
        required_preconditions: vec![
            "separate human approval is required before any migration implementation work begins.".into(),
            "Readiness pass must be treated only as permission to discuss the next step, not migration permission.".into(),
            "Default Chat send_message and start_stream_message paths must remain on the existing runtime until a later approved change.".into(),
            "Controlled pilot UI must remain explicit, reversible, and write-disabled unless a later review approves otherwise.".into(),
        ],
        rollback_plan: vec![
            "disable the controlled pilot entry and keep default Chat on the existing send path.".into(),
            "Keep existing Chat history and promoted assistant messages as ordinary messages; do not replay pilot output.".into(),
            "Use promotion evidence summaries only for audit review; do not synthesize replacement evidence.".into(),
        ],
        fallback_plan: vec![
            "Use the existing default Chat send path whenever the controlled pilot is unavailable, blocked, or fails.".into(),
            "If migration discussion is rejected, continue collecting reviewed pilot promotion evidence without changing default Chat.".into(),
            "If a future pilot degrades, show blockers and route users back to ordinary Chat without automatic retry or promotion.".into(),
        ],
        test_plan: vec![
            "Verify send_message and start_stream_message do not call the migration draft command.".into(),
            "Verify readiness blocked returns draftReady=false and no executable plan sections.".into(),
            "Verify readiness passed returns scope, preconditions, rollback, fallback, and test plan sections.".into(),
            "Verify the command creates no AgentRun, Proposal, Memory, LifeModel patch, or promotion evidence.".into(),
            "Verify serialized output contains no private transcript text, assistant transcript text, or full tool call data.".into(),
        ],
        manual_review_required: true,
        not_automatic_migration: true,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn record_controlled_chat_migration_review_decision(
    input: ControlledChatMigrationReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationReviewDecisionResult, String> {
    record_controlled_chat_migration_review_decision_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn record_controlled_chat_migration_review_decision_with_state(
    input: ControlledChatMigrationReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationReviewDecisionResult, String> {
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let session_id = normalize_optional_internal_id(input.session_id.as_deref(), "sessionId")?;
    let draft = draft_controlled_chat_migration_plan_with_state(
        ControlledChatMigrationPlanDraftInput {
            required_promotions: input.required_promotions,
            session_id: session_id.clone(),
        },
        state,
    )
    .await?;
    let draft_hash = metadata_hash_for_serializable(&draft)?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let mut blocking_reasons = draft.blocking_reasons.clone();

    if decision_kind == "approve" && !draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "draft_not_ready_for_approval".to_string(),
        );
        return Ok(ControlledChatMigrationReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            decision_kind,
            draft_ready: false,
            draft_hash,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note(input.optional_reviewer_note.as_deref());
    let metadata = json!({
        "evidenceKind": "migration_review_decision",
        "metadataSafe": true,
        "draftReady": draft.draft_ready,
        "decisionKind": decision_kind.clone(),
        "readinessCounts": {
            "requiredPromotions": draft.readiness_report.required_promotions,
            "promotedCount": draft.readiness_report.promoted_count,
            "recentPromotedPilotRunCount": draft.readiness_report.recent_promoted_pilot_run_ids.len(),
            "sourceTargetMismatchBlockCount": draft.readiness_report.source_target_mismatch_block_count,
            "blockingReasonCount": draft.blocking_reasons.len()
        },
        "draftHash": draft_hash.clone(),
        "createdAt": created_at.clone(),
        "sessionId": session_id.as_deref().unwrap_or("global"),
        "reviewerNote": reviewer_note_metadata,
        "blockingReasons": draft.blocking_reasons.clone(),
        "metadataSafeEvidenceReady": draft.readiness_report.metadata_safe_evidence_ready,
        "defaultChatUnchanged": draft.readiness_report.default_chat_unchanged,
        "manualReviewRequired": draft.manual_review_required,
        "notAutomaticMigration": draft.not_automatic_migration,
        "contentStorage": "checksum_only",
        "reviewerNoteStorage": "length_checksum_category_only",
        "toolStorage": "none",
        "transcriptStorage": "none"
    });

    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        CONTROLLED_CHAT_MIGRATION_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary("Controlled chat migration review decision recorded")
    .with_source_ref(EvidenceSourceRef::from_digest(
        EvidenceSourceType::RunMetadata,
        "controlled_chat_migration_plan_draft",
        Some("migration_review_decision"),
        &draft_hash,
    ));
    evidence_draft.run_metadata = metadata;

    let record = {
        let store = state.evidence_store.lock().await;
        store
            .create_evidence(evidence_draft)
            .map_err(|e| format!("failed to record migration review decision evidence: {e}"))?
    };

    Ok(ControlledChatMigrationReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        decision_kind,
        draft_ready: draft.draft_ready,
        draft_hash,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_controlled_chat_migration_review_decision_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationReviewDecisionSummary, String> {
    get_controlled_chat_migration_review_decision_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_controlled_chat_migration_review_decision_summary_with_state(
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationReviewDecisionSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(CONTROLLED_CHAT_MIGRATION_REVIEW_DECISION_EVIDENCE_PATH.into()),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| format!("failed to read migration review decision evidence: {e}"))?
    };
    let records = records
        .into_iter()
        .filter(migration_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| migration_review_decision_kind(record) == Some("approve"))
        .count();
    let rework_reject_count = records
        .iter()
        .filter(|record| {
            matches!(
                migration_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records.first().and_then(migration_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());
    let blocking_reasons = records
        .first()
        .map(migration_review_decision_blocking_reasons)
        .unwrap_or_default();

    Ok(ControlledChatMigrationReviewDecisionSummary {
        latest_decision,
        approved_count,
        rework_reject_count,
        latest_timestamp,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn check_controlled_chat_migration_implementation_gate(
    input: ControlledChatMigrationImplementationGateInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationImplementationGateReport, String> {
    check_controlled_chat_migration_implementation_gate_with_state(input, &state.inner().clone())
        .await
}

pub(crate) async fn check_controlled_chat_migration_implementation_gate_with_state(
    input: ControlledChatMigrationImplementationGateInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationImplementationGateReport, String> {
    let session_id = normalize_optional_internal_id(input.session_id.as_deref(), "sessionId")?;
    let current_draft = draft_controlled_chat_migration_plan_with_state(
        ControlledChatMigrationPlanDraftInput {
            required_promotions: input.required_promotions,
            session_id,
        },
        state,
    )
    .await?;
    let current_draft_hash = metadata_hash_for_serializable(&current_draft)?;
    let readiness_report = current_draft.readiness_report.clone();
    let decision_summary =
        get_controlled_chat_migration_review_decision_summary_with_state(state).await?;
    let latest_decision = decision_summary.latest_decision;
    let draft_hash_matched = latest_decision
        .as_ref()
        .is_some_and(|decision| decision.draft_hash == current_draft_hash);
    let latest_is_approve = latest_decision
        .as_ref()
        .is_some_and(|decision| decision.decision_kind == "approve");
    let approved_after_latest_draft = latest_is_approve && draft_hash_matched;

    let mut blocking_reasons = Vec::new();
    if !readiness_report.ready {
        push_unique_string(
            &mut blocking_reasons,
            "promotion_readiness_currently_blocked".to_string(),
        );
        for reason in &readiness_report.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    if !current_draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "migration_plan_draft_not_ready".to_string(),
        );
    }

    match latest_decision.as_ref() {
        Some(decision) if decision.decision_kind == "approve" => {
            if !decision.draft_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "latest_approval_draft_not_ready".to_string(),
                );
            }
            if !draft_hash_matched {
                push_unique_string(
                    &mut blocking_reasons,
                    "approved_draft_hash_mismatch".to_string(),
                );
            }
        }
        Some(decision) => {
            push_unique_string(
                &mut blocking_reasons,
                format!("latest_review_decision_is_{}", decision.decision_kind),
            );
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "metadata_safe_approve_decision_missing".to_string(),
            );
        }
    }

    let implementation_eligible = readiness_report.ready
        && current_draft.draft_ready
        && latest_is_approve
        && latest_decision
            .as_ref()
            .is_some_and(|decision| decision.draft_ready)
        && draft_hash_matched
        && blocking_reasons.is_empty();

    Ok(ControlledChatMigrationImplementationGateReport {
        implementation_eligible,
        latest_decision,
        readiness_report,
        draft_hash_matched,
        approved_after_latest_draft,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn run_controlled_chat_migration_shadow_run(
    input: ControlledChatMigrationShadowRunInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationShadowRunOutput, String> {
    run_controlled_chat_migration_shadow_run_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn run_controlled_chat_migration_shadow_run_with_state(
    input: ControlledChatMigrationShadowRunInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationShadowRunOutput, String> {
    let normalized = normalize_shadow_run_input(input)?;
    let implementation_gate_report =
        check_controlled_chat_migration_implementation_gate_with_state(
            ControlledChatMigrationImplementationGateInput {
                required_promotions: normalized.required_promotions,
                session_id: Some(normalized.session_id.clone()),
            },
            state,
        )
        .await?;

    if !implementation_gate_report.implementation_eligible {
        let mut blocking_reasons = vec!["implementation_gate_blocked".to_string()];
        for reason in &implementation_gate_report.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
        return Ok(ControlledChatMigrationShadowRunOutput {
            shadow_run_ready: false,
            shadow_run_id: None,
            metadata_safe_summary: shadow_blocked_summary(
                &normalized.descriptor_kind,
                normalized.user_input_checksum.as_deref(),
            ),
            implementation_gate_report,
            strategy_kind: "notRun".into(),
            payload_kind: "notRun".into(),
            warnings: Vec::new(),
            blocking_reasons,
        });
    }

    let mut shadow_run = new_shadow_agent_run(
        &normalized.session_id,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
    );
    let shadow_run_id = shadow_run.id.clone();
    create_shadow_run(state, &shadow_run).await?;

    let runtime_input = MultiStrategyAgentPreviewInput {
        session_id: normalized.session_id.clone(),
        user_text: shadow_prompt_for_descriptor(&normalized.descriptor_kind).into(),
        tools_prompt: "No developer tools catalog supplied for this shadow run.".into(),
        allow_planning: normalized.descriptor_kind == "planning_readiness_probe",
        local_model_available: normalized.descriptor_kind != "sensitive_local_only_probe",
        layer: Some("L2".into()),
        execution_budget: Some(MultiStrategyAgentPreviewExecutionBudgetInput {
            max_steps: Some(3),
            max_tool_calls: Some(0),
            timeout_seconds: Some(30),
            allow_cloud: Some(false),
            allow_writes: Some(false),
        }),
    };

    let execution =
        execute_multi_strategy_agent_preview(runtime_input, state, &shadow_run_id).await;
    let execution = match execution {
        Ok(execution) => execution,
        Err(error) => {
            let safe_error = metadata_safe_shadow_error(&error);
            fail_shadow_run(state, &mut shadow_run, &safe_error).await;
            return Ok(ControlledChatMigrationShadowRunOutput {
                shadow_run_ready: false,
                shadow_run_id: Some(shadow_run_id),
                implementation_gate_report,
                strategy_kind: "notRun".into(),
                payload_kind: "notRun".into(),
                metadata_safe_summary: shadow_failed_summary(
                    &normalized.descriptor_kind,
                    normalized.user_input_checksum.as_deref(),
                    &safe_error,
                ),
                warnings: vec!["shadow runtime failed before readiness comparison".into()],
                blocking_reasons: vec![safe_error],
            });
        }
    };

    let strategy_kind = preview_strategy_kind(execution.output.selection.kind).to_string();
    let payload_kind = preview_payload_kind(&execution.output.payload).to_string();
    let mut warnings = preview_output_warnings(&execution.output, &execution.warnings);
    push_unique_string(
        &mut warnings,
        "shadow runtime forced allowWrites=false".to_string(),
    );
    let metadata_safe_summary = shadow_metadata_safe_summary(
        &execution.output,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
    );
    let audit = shadow_audit_summary(
        &execution.output,
        &warnings,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
    );

    complete_shadow_run(
        state,
        &mut shadow_run,
        ShadowRunCompletion {
            audit,
            warnings: warnings.clone(),
            context_summary: execution.context_summary,
            hs_selection_audit: execution.hs_selection_audit,
            behavior_checks: execution.behavior_checks,
        },
    )
    .await?;

    Ok(ControlledChatMigrationShadowRunOutput {
        shadow_run_ready: true,
        shadow_run_id: Some(shadow_run_id),
        implementation_gate_report,
        strategy_kind,
        payload_kind,
        metadata_safe_summary,
        warnings,
        blocking_reasons: Vec::new(),
    })
}

#[tauri::command]
pub async fn record_controlled_chat_migration_shadow_review_decision(
    input: ControlledChatMigrationShadowReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationShadowReviewDecisionResult, String> {
    record_controlled_chat_migration_shadow_review_decision_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn record_controlled_chat_migration_shadow_review_decision_with_state(
    input: ControlledChatMigrationShadowReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationShadowReviewDecisionResult, String> {
    let shadow_run_id = safe_internal_id(&input.shadow_run_id, "shadowRunId")?;
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let run = load_shadow_review_run(state, &shadow_run_id).await?;
    let readiness = shadow_review_readiness(run.as_ref())?;
    let blocking_reasons = readiness.blocking_reasons.clone();

    if !blocking_reasons.is_empty() {
        return Ok(ControlledChatMigrationShadowReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            shadow_run_id,
            decision_kind,
            readiness_summary_digest: readiness.digest,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        CONTROLLED_CHAT_MIGRATION_SHADOW_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "shadowRunId": shadow_run_id.clone(),
        "decisionKind": decision_kind.clone(),
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "readinessSummaryDigest": readiness.digest.clone(),
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!("failed to record migration shadow review decision evidence: {e}")
        })?
    };

    Ok(ControlledChatMigrationShadowReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        shadow_run_id,
        decision_kind,
        readiness_summary_digest: readiness.digest,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_controlled_chat_migration_shadow_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationShadowReviewSummary, String> {
    get_controlled_chat_migration_shadow_review_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_controlled_chat_migration_shadow_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationShadowReviewSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    CONTROLLED_CHAT_MIGRATION_SHADOW_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| format!("failed to read migration shadow review evidence: {e}"))?
    };
    let records = records
        .into_iter()
        .filter(shadow_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| shadow_review_decision_kind(record) == Some("approve"))
        .count();
    let rework_reject_count = records
        .iter()
        .filter(|record| {
            matches!(
                shadow_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records.first().and_then(shadow_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());

    Ok(ControlledChatMigrationShadowReviewSummary {
        latest_decision,
        approved_count,
        rework_reject_count,
        latest_timestamp,
        blocking_reasons: Vec::new(),
    })
}

#[tauri::command]
pub async fn check_controlled_chat_cutover_readiness(
    input: ControlledChatCutoverReadinessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatCutoverReadinessReport, String> {
    check_controlled_chat_cutover_readiness_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn check_controlled_chat_cutover_readiness_with_state(
    input: ControlledChatCutoverReadinessInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatCutoverReadinessReport, String> {
    let implementation_gate_report =
        check_controlled_chat_migration_implementation_gate_with_state(
            ControlledChatMigrationImplementationGateInput {
                required_promotions: input.required_promotions,
                session_id: input.session_id,
            },
            state,
        )
        .await?;
    let shadow_review_summary =
        get_controlled_chat_migration_shadow_review_summary_with_state(state).await?;
    let latest_shadow_review_decision = shadow_review_summary.latest_decision.clone();
    let default_chat_unchanged = implementation_gate_report
        .readiness_report
        .default_chat_unchanged;

    let mut blocking_reasons = Vec::new();
    if !implementation_gate_report.implementation_eligible {
        push_unique_string(
            &mut blocking_reasons,
            "implementation_gate_not_eligible".into(),
        );
        for reason in &implementation_gate_report.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    if !default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }

    let mut readiness_summary_digest = None;
    let mut verified_shadow_run_id = None;
    let mut shadow_run_ready = false;
    let latest_shadow_decision_kind = latest_shadow_review_decision
        .as_ref()
        .map(|decision| decision.decision_kind.clone())
        .unwrap_or_else(|| "none".into());

    match latest_shadow_review_decision.as_ref() {
        Some(decision) if decision.decision_kind == "approve" => {
            let run = load_shadow_review_run(state, &decision.shadow_run_id).await?;
            let readiness = shadow_review_readiness(run.as_ref())?;
            readiness_summary_digest = Some(readiness.digest.clone());
            for reason in &readiness.blocking_reasons {
                push_unique_string(&mut blocking_reasons, reason.clone());
            }
            if readiness.blocking_reasons.is_empty()
                && readiness.digest != decision.readiness_summary_digest
            {
                push_unique_string(
                    &mut blocking_reasons,
                    "shadow_run_readiness_digest_mismatch".into(),
                );
            }
            shadow_run_ready = readiness.blocking_reasons.is_empty()
                && readiness.digest == decision.readiness_summary_digest;
            if shadow_run_ready {
                verified_shadow_run_id = Some(decision.shadow_run_id.clone());
            }
        }
        Some(decision) => {
            readiness_summary_digest = Some(decision.readiness_summary_digest.clone());
            push_unique_string(
                &mut blocking_reasons,
                format!(
                    "latest_shadow_review_decision_is_{}",
                    decision.decision_kind
                ),
            );
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "shadow_review_approve_missing".into(),
            );
        }
    }

    let required_evidence_ready = implementation_gate_report.implementation_eligible
        && default_chat_unchanged
        && latest_shadow_review_decision
            .as_ref()
            .is_some_and(|decision| decision.decision_kind == "approve")
        && shadow_run_ready;
    let cutover_planning_eligible = required_evidence_ready && blocking_reasons.is_empty();
    let metadata_safe_summary =
        cutover_readiness_metadata_safe_summary(CutoverReadinessMetadataSafeSummaryInput {
            cutover_planning_eligible,
            required_evidence_ready,
            default_chat_unchanged,
            implementation_eligible: implementation_gate_report.implementation_eligible,
            latest_shadow_decision_kind: &latest_shadow_decision_kind,
            shadow_run_ready,
            verified_shadow_run_id: verified_shadow_run_id.as_deref(),
            readiness_summary_digest: readiness_summary_digest.as_deref(),
            shadow_review_summary: &shadow_review_summary,
        });

    Ok(ControlledChatCutoverReadinessReport {
        cutover_planning_eligible,
        implementation_gate_report,
        latest_shadow_review_decision,
        verified_shadow_run_id,
        readiness_summary_digest,
        default_chat_unchanged,
        required_evidence_ready,
        blocking_reasons,
        metadata_safe_summary,
    })
}

#[tauri::command]
pub async fn run_controlled_chat_cutover_candidate(
    input: ControlledChatCutoverCandidateInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatCutoverCandidateOutput, String> {
    run_controlled_chat_cutover_candidate_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn run_controlled_chat_cutover_candidate_with_state(
    input: ControlledChatCutoverCandidateInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatCutoverCandidateOutput, String> {
    let normalized = normalize_cutover_candidate_input(input)?;
    let readiness = check_controlled_chat_cutover_readiness_with_state(
        ControlledChatCutoverReadinessInput {
            required_promotions: normalized.required_promotions,
            session_id: Some(normalized.session_id.clone()),
        },
        state,
    )
    .await?;

    if !readiness.cutover_planning_eligible {
        let mut blocking_reasons = vec!["cutover_readiness_not_eligible".to_string()];
        for reason in &readiness.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
        return Ok(ControlledChatCutoverCandidateOutput {
            candidate_ready: false,
            candidate_run_id: None,
            output_preview: Some("Candidate blocked before runtime".into()),
            user_output: None,
            contract_shape: "blocked".into(),
            metadata_safe_summary: cutover_candidate_blocked_summary(
                &normalized.descriptor_kind,
                normalized.user_input_checksum.as_deref(),
            ),
            warnings: Vec::new(),
            blocking_reasons,
        });
    }

    let mut candidate_run = new_cutover_candidate_agent_run(
        &normalized.session_id,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
    );
    let candidate_run_id = candidate_run.id.clone();
    create_cutover_candidate_run(state, &candidate_run).await?;

    let runtime_input = MultiStrategyAgentPreviewInput {
        session_id: normalized.session_id.clone(),
        user_text: cutover_candidate_prompt_for_descriptor(&normalized.descriptor_kind).into(),
        tools_prompt: "No developer tools catalog supplied for this cutover candidate.".into(),
        allow_planning: false,
        local_model_available: true,
        layer: Some("L2".into()),
        execution_budget: Some(MultiStrategyAgentPreviewExecutionBudgetInput {
            max_steps: Some(2),
            max_tool_calls: Some(0),
            timeout_seconds: Some(30),
            allow_cloud: Some(false),
            allow_writes: Some(false),
        }),
    };

    let execution =
        execute_multi_strategy_agent_preview(runtime_input, state, &candidate_run_id).await;
    let execution = match execution {
        Ok(execution) => execution,
        Err(error) => {
            let safe_error = metadata_safe_cutover_candidate_error(&error);
            fail_cutover_candidate_run(state, &mut candidate_run, &safe_error).await;
            return Ok(ControlledChatCutoverCandidateOutput {
                candidate_ready: false,
                candidate_run_id: Some(candidate_run_id),
                output_preview: Some("Candidate failed before contract validation".into()),
                user_output: None,
                contract_shape: "failed".into(),
                metadata_safe_summary: cutover_candidate_failed_summary(
                    &normalized.descriptor_kind,
                    normalized.user_input_checksum.as_deref(),
                    &safe_error,
                ),
                warnings: vec!["candidate runtime failed before contract validation".into()],
                blocking_reasons: vec![safe_error],
            });
        }
    };

    let contract_shape = cutover_candidate_contract_shape(&execution.output).to_string();
    let candidate_ready = contract_shape == "send_message_compatible";
    let user_output = cutover_candidate_user_output(&execution.output);
    let output_preview = cutover_candidate_output_label(&execution.output);
    let mut warnings = preview_output_warnings(&execution.output, &execution.warnings);
    push_unique_string(
        &mut warnings,
        "candidate runtime forced allowWrites=false".to_string(),
    );
    let blocking_reasons = cutover_candidate_contract_blockers(&execution.output, &contract_shape);
    let output_digest = user_output
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(sha256_metadata_checksum);
    let metadata_safe_summary = cutover_candidate_metadata_safe_summary(
        &execution.output,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
        &contract_shape,
        candidate_ready,
        output_digest.as_deref(),
    );
    let audit = cutover_candidate_audit_summary(
        &execution.output,
        &warnings,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
        &contract_shape,
        candidate_ready,
        output_digest.as_deref(),
    );

    complete_cutover_candidate_run(
        state,
        &mut candidate_run,
        CutoverCandidateRunCompletion {
            audit,
            warnings: warnings.clone(),
            context_summary: execution.context_summary,
            hs_selection_audit: execution.hs_selection_audit,
            behavior_checks: execution.behavior_checks,
        },
    )
    .await?;

    Ok(ControlledChatCutoverCandidateOutput {
        candidate_ready,
        candidate_run_id: Some(candidate_run_id),
        output_preview: Some(output_preview),
        user_output,
        contract_shape,
        metadata_safe_summary,
        warnings,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn record_controlled_chat_cutover_candidate_review_decision(
    input: ControlledChatCutoverCandidateReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatCutoverCandidateReviewDecisionResult, String> {
    record_controlled_chat_cutover_candidate_review_decision_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn record_controlled_chat_cutover_candidate_review_decision_with_state(
    input: ControlledChatCutoverCandidateReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatCutoverCandidateReviewDecisionResult, String> {
    let candidate_run_id = safe_internal_id(&input.candidate_run_id, "candidateRunId")?;
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let run = load_cutover_candidate_review_run(state, &candidate_run_id).await?;
    let readiness = cutover_candidate_review_readiness(run.as_ref())?;
    let mut blocking_reasons = readiness.blocking_reasons.clone();

    if decision_kind == "approve" {
        if readiness.contract_shape != "send_message_compatible" {
            push_unique_string(
                &mut blocking_reasons,
                "candidate_run_contract_shape_not_send_message_compatible".into(),
            );
        }
        if !readiness.candidate_ready {
            push_unique_string(
                &mut blocking_reasons,
                "candidate_run_not_ready_for_approval".into(),
            );
        }
    }

    if !blocking_reasons.is_empty() {
        return Ok(ControlledChatCutoverCandidateReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            candidate_run_id,
            decision_kind,
            contract_shape: readiness.contract_shape,
            candidate_summary_digest: readiness.digest,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "candidateRunId": candidate_run_id.clone(),
        "decisionKind": decision_kind.clone(),
        "contractShape": readiness.contract_shape.clone(),
        "candidateSummaryDigest": readiness.digest.clone(),
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!("failed to record cutover candidate review decision evidence: {e}")
        })?
    };

    Ok(ControlledChatCutoverCandidateReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        candidate_run_id,
        decision_kind,
        contract_shape: readiness.contract_shape,
        candidate_summary_digest: readiness.digest,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_controlled_chat_cutover_candidate_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatCutoverCandidateReviewSummary, String> {
    get_controlled_chat_cutover_candidate_review_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_controlled_chat_cutover_candidate_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<ControlledChatCutoverCandidateReviewSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| format!("failed to read cutover candidate review evidence: {e}"))?
    };
    let records = records
        .into_iter()
        .filter(cutover_candidate_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| cutover_candidate_review_decision_kind(record) == Some("approve"))
        .count();
    let rework_reject_count = records
        .iter()
        .filter(|record| {
            matches!(
                cutover_candidate_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records
        .first()
        .and_then(cutover_candidate_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());

    Ok(ControlledChatCutoverCandidateReviewSummary {
        latest_decision,
        approved_count,
        rework_reject_count,
        latest_timestamp,
        blocking_reasons: Vec::new(),
    })
}

#[tauri::command]
pub async fn check_controlled_chat_cutover_candidate_promotion_readiness(
    input: ControlledChatCutoverCandidatePromotionReadinessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatCutoverCandidatePromotionReadinessReport, String> {
    check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
    input: ControlledChatCutoverCandidatePromotionReadinessInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatCutoverCandidatePromotionReadinessReport, String> {
    let required_approved_candidates = input
        .required_approved_candidates
        .filter(|value| *value > 0)
        .unwrap_or(1);
    let checked_at = chrono::Utc::now().to_rfc3339();
    let cutover_readiness = check_controlled_chat_cutover_readiness_with_state(
        ControlledChatCutoverReadinessInput {
            required_promotions: input.required_promotions,
            session_id: input.session_id,
        },
        state,
    )
    .await?;
    let cutover_readiness_eligible = cutover_readiness.cutover_planning_eligible;
    let default_chat_unchanged = cutover_readiness.default_chat_unchanged;

    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| {
                format!("failed to read cutover candidate promotion readiness evidence: {e}")
            })?
    };
    let records = records
        .into_iter()
        .filter(cutover_candidate_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();
    let latest_decision = records
        .first()
        .and_then(cutover_candidate_review_latest_decision);

    let mut approved_decisions = Vec::new();
    let mut approved_candidate_run_ids = Vec::<String>::new();
    for record in records
        .iter()
        .filter(|record| cutover_candidate_review_decision_kind(record) == Some("approve"))
    {
        let Some(decision) = cutover_candidate_review_latest_decision(record) else {
            continue;
        };
        if approved_candidate_run_ids
            .iter()
            .any(|run_id| run_id == &decision.candidate_run_id)
        {
            continue;
        }
        approved_candidate_run_ids.push(decision.candidate_run_id.clone());
        approved_decisions.push(decision);
    }

    let mut blocking_reasons = Vec::new();
    if !cutover_readiness_eligible {
        push_unique_string(
            &mut blocking_reasons,
            "cutover_readiness_not_eligible".into(),
        );
        for reason in &cutover_readiness.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    if !default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }

    match latest_decision
        .as_ref()
        .map(|decision| decision.decision_kind.as_str())
    {
        Some("reject" | "request_rework") => {
            let decision_kind = latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str())
                .unwrap_or("unknown");
            push_unique_string(
                &mut blocking_reasons,
                format!("latest_candidate_review_decision_is_{decision_kind}"),
            );
        }
        Some("approve") => {}
        Some(other) => {
            push_unique_string(
                &mut blocking_reasons,
                format!("latest_candidate_review_decision_is_{other}"),
            );
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "candidate_review_decision_missing".into(),
            );
        }
    }

    let approved_candidate_count = approved_decisions.len();
    if approved_candidate_count == 0 {
        push_unique_string(
            &mut blocking_reasons,
            "metadata_safe_candidate_approve_evidence_missing".into(),
        );
    }
    if approved_candidate_count < required_approved_candidates {
        push_unique_string(
            &mut blocking_reasons,
            format!(
                "insufficient_approved_candidate_evidence: required {required_approved_candidates}, found {approved_candidate_count}"
            ),
        );
    }

    let mut approved_candidates = Vec::new();
    for decision in approved_decisions {
        let run = load_cutover_candidate_review_run(state, &decision.candidate_run_id).await?;
        let readiness = cutover_candidate_review_readiness(run.as_ref())?;
        let mut candidate_blocking_reasons = readiness.blocking_reasons.clone();
        if readiness.contract_shape != "send_message_compatible" {
            push_unique_string(
                &mut candidate_blocking_reasons,
                "candidate_run_contract_shape_not_send_message_compatible".into(),
            );
        }
        if !readiness.candidate_ready {
            push_unique_string(
                &mut candidate_blocking_reasons,
                "candidate_run_not_ready_for_approval".into(),
            );
        }
        if run.is_some()
            && candidate_blocking_reasons.is_empty()
            && readiness.digest != decision.candidate_summary_digest
        {
            push_unique_string(
                &mut candidate_blocking_reasons,
                "candidate_run_summary_digest_mismatch".into(),
            );
        }

        for reason in &candidate_blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
        let ready = candidate_blocking_reasons.is_empty();
        approved_candidates.push(ControlledChatCutoverCandidatePromotionApprovedCandidate {
            evidence_id: decision.evidence_id,
            candidate_run_id: decision.candidate_run_id,
            contract_shape: readiness.contract_shape,
            candidate_summary_digest: decision.candidate_summary_digest,
            run_readiness_digest: readiness.digest,
            decision_created_at: decision.created_at,
            ready,
            blocking_reasons: candidate_blocking_reasons,
        });
    }

    let ready = cutover_readiness_eligible
        && default_chat_unchanged
        && approved_candidate_count >= required_approved_candidates
        && latest_decision
            .as_ref()
            .is_some_and(|decision| decision.decision_kind == "approve")
        && approved_candidates.iter().all(|candidate| candidate.ready)
        && blocking_reasons.is_empty();
    let metadata_safe_summary = cutover_candidate_promotion_readiness_metadata_safe_summary(
        CutoverCandidatePromotionReadinessMetadataSafeSummaryInput {
            ready,
            cutover_readiness_eligible,
            required_approved_candidates,
            approved_candidate_count,
            latest_decision_kind: latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str())
                .unwrap_or("none"),
            default_chat_unchanged,
            verified_candidate_count: approved_candidates.len(),
            blocking_reason_count: blocking_reasons.len(),
        },
    );

    Ok(ControlledChatCutoverCandidatePromotionReadinessReport {
        ready,
        cutover_readiness_eligible,
        required_approved_candidates,
        approved_candidate_count,
        latest_decision,
        approved_candidates,
        default_chat_unchanged,
        blocking_reasons,
        metadata_safe_summary,
        checked_at,
    })
}

#[tauri::command]
pub async fn get_default_chat_runtime_boundary_status(
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatRuntimeBoundaryStatus, String> {
    get_default_chat_runtime_boundary_status_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_default_chat_runtime_boundary_status_with_state(
    _state: &Arc<AppState>,
) -> Result<DefaultChatRuntimeBoundaryStatus, String> {
    Ok(DefaultChatRuntimeBoundaryStatus {
        current_mode: "legacy_stream".into(),
        controlled_candidate_available: false,
        default_chat_unchanged: true,
        candidate_promotion_readiness_required: true,
        automatic_migration_enabled: false,
        blocking_reasons: Vec::new(),
        metadata_safe_summary: json!({
            "runtimeBoundary": "default_chat",
            "metadataSafe": true,
            "readOnly": true,
            "currentMode": "legacy_stream",
            "controlledCandidateAvailable": false,
            "defaultChatUnchanged": true,
            "candidatePromotionReadinessRequired": true,
            "automaticMigrationEnabled": false,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "none",
            "mcpAuditStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn draft_default_chat_adapter_activation_plan(
    input: DefaultChatAdapterActivationPlanDraftInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterActivationPlanDraft, String> {
    draft_default_chat_adapter_activation_plan_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn draft_default_chat_adapter_activation_plan_with_state(
    input: DefaultChatAdapterActivationPlanDraftInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterActivationPlanDraft, String> {
    let candidate_promotion_readiness_report =
        check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
            ControlledChatCutoverCandidatePromotionReadinessInput {
                required_approved_candidates: input.required_approved_candidates,
                required_promotions: input.required_promotions,
                session_id: input.session_id,
            },
            state,
        )
        .await?;
    let runtime_boundary_status =
        get_default_chat_runtime_boundary_status_with_state(state).await?;

    Ok(draft_default_chat_adapter_activation_plan_from_reports(
        candidate_promotion_readiness_report,
        runtime_boundary_status,
    ))
}

fn draft_default_chat_adapter_activation_plan_from_reports(
    candidate_promotion_readiness_report: ControlledChatCutoverCandidatePromotionReadinessReport,
    runtime_boundary_status: DefaultChatRuntimeBoundaryStatus,
) -> DefaultChatAdapterActivationPlanDraft {
    let mut blocking_reasons = Vec::new();
    if !candidate_promotion_readiness_report.ready {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_promotion_readiness_not_ready".into(),
        );
        for reason in &candidate_promotion_readiness_report.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    if runtime_boundary_status.current_mode != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "default_chat_runtime_boundary_not_legacy_stream".into(),
        );
    }
    if runtime_boundary_status.automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }
    if !runtime_boundary_status.default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }
    if runtime_boundary_status.controlled_candidate_available {
        push_unique_string(
            &mut blocking_reasons,
            "controlled_candidate_available_on_default_path".into(),
        );
    }
    if !runtime_boundary_status.candidate_promotion_readiness_required {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_promotion_readiness_not_required_by_boundary_status".into(),
        );
    }
    for reason in &runtime_boundary_status.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }

    let draft_ready = candidate_promotion_readiness_report.ready
        && runtime_boundary_status.current_mode == "legacy_stream"
        && !runtime_boundary_status.automatic_migration_enabled
        && runtime_boundary_status.default_chat_unchanged
        && !runtime_boundary_status.controlled_candidate_available
        && runtime_boundary_status.candidate_promotion_readiness_required
        && blocking_reasons.is_empty();

    let (
        activation_scope,
        required_preconditions,
        adapter_contract_checks,
        fallback_plan,
        rollback_plan,
        observability_plan,
        test_plan,
    ) = if draft_ready {
        (
            vec![
                "human-review-only draft for a future default Chat controlled adapter activation; default Chat remains on legacy_stream.".into(),
                "Scope is limited to activation boundaries, adapter contract checks, fallback, rollback, observability, and tests.".into(),
                "This draft does not replace default Chat, add an activation flag, run runtime, or create AgentRun/Evidence/Proposal/Memory/LifeModel/MCP audit/chat records.".into(),
            ],
            vec![
                "W33 candidate promotion readiness must remain ready at implementation review time.".into(),
                "W34 default Chat runtime boundary must remain currentMode=legacy_stream with automaticMigrationEnabled=false.".into(),
                "A separate reviewed implementation must explicitly approve any adapter routing work before send_message or start_stream_message changes.".into(),
                "Settings may display this draft only as read-only review material without switch, migrate, or enable controls.".into(),
            ],
            vec![
                "Keep adapter output constrained to the W31/W33 send_message-compatible contract shape before any default path integration.".into(),
                "Preserve send_message and start_stream_message request/response semantics, streaming completion behavior, and error fallback shape.".into(),
                "Require write-disabled, zero-tool, metadata-safe candidate evidence to remain valid before implementation discussion continues.".into(),
                "Reject any adapter path that persists private transcript text, assistant content, full tool data, Proposal, Memory, LifeModel patch, Evidence, MCP audit, or Chat message during draft evaluation.".into(),
            ],
            vec![
                "Keep the existing legacy stream default path as the fallback whenever a future adapter is unavailable, blocked, or fails contract checks.".into(),
                "Do not automatically retry through controlled runtime or promote candidate output into Chat history from this draft.".into(),
                "Surface blockers and keep the user on ordinary Chat until a separate implementation is reviewed.".into(),
            ],
            vec![
                "Rollback must revert only a separate adapter implementation and leave current Chat history as ordinary messages.".into(),
                "Remove any future adapter routing from the default path and return currentMode to legacy_stream.".into(),
                "Do not synthesize replacement evidence, replay candidate output, or patch LifeModel/Memory during rollback.".into(),
            ],
            vec![
                "Track metadata-safe readiness, boundary, fallback, rollback, error, and latency counters without private transcript text or full tool data.".into(),
                "Expose blocking reason counts and latest metadata-safe readiness digests for human review.".into(),
                "Keep observability separate from Chat message persistence, Evidence writes, MCP audit logs, and model/tool runtime payloads.".into(),
            ],
            vec![
                "Verify W33 blocked returns draftReady=false with no activation plan sections.".into(),
                "Verify W34 non-legacy or automatic migration enabled returns draftReady=false with no activation plan sections.".into(),
                "Verify W33 ready plus W34 legacy returns the complete human-review-only activation plan.".into(),
                "Verify command side-effect counts remain unchanged for AgentRun, Proposal, Evidence, LifeModel patch, MCP audit, Memory, and Chat messages.".into(),
                "Verify serialized output is metadata-safe and contains no private transcript text, assistant text, or full tool data.".into(),
                "Verify send_message and start_stream_message do not call this draft command.".into(),
            ],
        )
    } else {
        (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };

    let metadata_safe_summary = json!({
        "activationPlan": "default_chat_adapter_activation",
        "metadataSafe": true,
        "readOnly": true,
        "humanReviewOnly": true,
        "draftReady": draft_ready,
        "manualReviewRequired": true,
        "notAutomaticMigration": true,
        "requiresSeparateImplementation": true,
        "candidatePromotionReady": candidate_promotion_readiness_report.ready,
        "currentMode": runtime_boundary_status.current_mode,
        "automaticMigrationEnabled": runtime_boundary_status.automatic_migration_enabled,
        "defaultChatUnchanged": runtime_boundary_status.default_chat_unchanged,
        "blockingReasonCount": blocking_reasons.len(),
        "activationSectionCount": activation_scope.len(),
        "preconditionSectionCount": required_preconditions.len(),
        "adapterContractCheckCount": adapter_contract_checks.len(),
        "fallbackPlanCount": fallback_plan.len(),
        "rollbackPlanCount": rollback_plan.len(),
        "observabilityPlanCount": observability_plan.len(),
        "testPlanCount": test_plan.len(),
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "read_only",
        "mcpAuditStorage": "none",
        "transcriptStorage": "none",
    });

    DefaultChatAdapterActivationPlanDraft {
        draft_ready,
        candidate_promotion_readiness_report,
        runtime_boundary_status,
        activation_scope,
        required_preconditions,
        adapter_contract_checks,
        fallback_plan,
        rollback_plan,
        observability_plan,
        test_plan,
        manual_review_required: true,
        not_automatic_migration: true,
        requires_separate_implementation: true,
        blocking_reasons,
        metadata_safe_summary,
    }
}

#[tauri::command]
pub async fn record_default_chat_adapter_activation_review_decision(
    input: DefaultChatAdapterActivationReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterActivationReviewDecisionResult, String> {
    record_default_chat_adapter_activation_review_decision_with_state(input, &state.inner().clone())
        .await
}

pub(crate) async fn record_default_chat_adapter_activation_review_decision_with_state(
    input: DefaultChatAdapterActivationReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterActivationReviewDecisionResult, String> {
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let session_id = normalize_optional_internal_id(input.session_id.as_deref(), "sessionId")?;
    let draft = draft_default_chat_adapter_activation_plan_with_state(
        DefaultChatAdapterActivationPlanDraftInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id,
        },
        state,
    )
    .await?;
    let activation_plan_digest = default_chat_adapter_activation_plan_digest(&draft)?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let mut blocking_reasons = draft.blocking_reasons.clone();

    if decision_kind == "approve" && !draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "activation_plan_draft_not_ready_for_approval".into(),
        );
        return Ok(DefaultChatAdapterActivationReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            decision_kind,
            draft_ready: false,
            activation_plan_digest,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        DEFAULT_CHAT_ADAPTER_ACTIVATION_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "evidenceKind": "default_chat_adapter_activation_review_decision",
        "decisionKind": decision_kind.clone(),
        "draftReady": draft.draft_ready,
        "activationPlanDigest": activation_plan_digest.clone(),
        "candidatePromotionReady": draft.candidate_promotion_readiness_report.ready,
        "currentMode": draft.runtime_boundary_status.current_mode,
        "automaticMigrationEnabled": draft.runtime_boundary_status.automatic_migration_enabled,
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!(
                "failed to record default Chat adapter activation review decision evidence: {e}"
            )
        })?
    };

    Ok(DefaultChatAdapterActivationReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        decision_kind,
        draft_ready: draft.draft_ready,
        activation_plan_digest,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_default_chat_adapter_activation_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterActivationReviewSummary, String> {
    get_default_chat_adapter_activation_review_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_default_chat_adapter_activation_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterActivationReviewSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    DEFAULT_CHAT_ADAPTER_ACTIVATION_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| {
                format!("failed to read default Chat adapter activation review evidence: {e}")
            })?
    };
    let records = records
        .into_iter()
        .filter(default_chat_adapter_activation_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_activation_review_decision_kind(record) == Some("approve")
        })
        .count();
    let reject_or_rework_count = records
        .iter()
        .filter(|record| {
            matches!(
                default_chat_adapter_activation_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records
        .first()
        .and_then(default_chat_adapter_activation_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());
    let latest_decision_present = latest_decision.is_some();
    let blocking_reasons = if latest_decision_present {
        Vec::new()
    } else {
        vec!["activation_review_decision_missing".into()]
    };
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterActivationReviewSummary {
        latest_decision,
        approved_count,
        reject_or_rework_count,
        latest_timestamp,
        blocking_reasons,
        metadata_safe_summary: json!({
            "activationReview": "default_chat_adapter_activation",
            "metadataSafe": true,
            "readOnly": true,
            "approvedCount": approved_count,
            "rejectOrReworkCount": reject_or_rework_count,
            "latestDecisionPresent": latest_decision_present,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "transcriptStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn check_default_chat_adapter_activation_implementation_gate(
    input: DefaultChatAdapterActivationImplementationGateInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterActivationImplementationGateReport, String> {
    check_default_chat_adapter_activation_implementation_gate_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn check_default_chat_adapter_activation_implementation_gate_with_state(
    input: DefaultChatAdapterActivationImplementationGateInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterActivationImplementationGateReport, String> {
    let draft = draft_default_chat_adapter_activation_plan_with_state(
        DefaultChatAdapterActivationPlanDraftInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: input.session_id,
        },
        state,
    )
    .await?;
    let current_activation_plan_digest = default_chat_adapter_activation_plan_digest(&draft)?;
    let review_summary =
        get_default_chat_adapter_activation_review_summary_with_state(state).await?;
    let latest_decision = review_summary.latest_decision.clone();
    let mut blocking_reasons = Vec::new();

    if !draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "activation_plan_draft_not_ready".into(),
        );
        for reason in &draft.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }

    let activation_plan_digest_matched = latest_decision
        .as_ref()
        .is_some_and(|decision| decision.activation_plan_digest == current_activation_plan_digest);

    match latest_decision.as_ref() {
        Some(decision) => {
            if decision.decision_kind != "approve" {
                push_unique_string(
                    &mut blocking_reasons,
                    format!(
                        "latest_activation_review_decision_is_{}",
                        decision.decision_kind
                    ),
                );
            }
            if !decision.draft_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "activation_review_draft_not_ready".into(),
                );
            }
            if !activation_plan_digest_matched {
                push_unique_string(
                    &mut blocking_reasons,
                    "activation_plan_digest_mismatch".into(),
                );
            }
            if !decision.candidate_promotion_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "activation_review_candidate_promotion_not_ready".into(),
                );
            }
            if decision.current_mode != "legacy_stream" {
                push_unique_string(
                    &mut blocking_reasons,
                    "activation_review_current_mode_not_legacy_stream".into(),
                );
            }
            if decision.automatic_migration_enabled {
                push_unique_string(
                    &mut blocking_reasons,
                    "activation_review_automatic_migration_enabled".into(),
                );
            }
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "activation_review_decision_missing".into(),
            );
        }
    }

    if draft.runtime_boundary_status.current_mode != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "default_chat_runtime_boundary_not_legacy_stream".into(),
        );
    }
    if draft.runtime_boundary_status.automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }
    if !draft.runtime_boundary_status.default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }

    let implementation_gate_eligible = draft.draft_ready
        && latest_decision.as_ref().is_some_and(|decision| {
            decision.decision_kind == "approve"
                && decision.draft_ready
                && decision.candidate_promotion_ready
                && decision.current_mode == "legacy_stream"
                && !decision.automatic_migration_enabled
        })
        && activation_plan_digest_matched
        && draft.runtime_boundary_status.default_chat_unchanged
        && draft.runtime_boundary_status.current_mode == "legacy_stream"
        && !draft.runtime_boundary_status.automatic_migration_enabled
        && blocking_reasons.is_empty();
    let latest_decision_kind = latest_decision
        .as_ref()
        .map(|decision| decision.decision_kind.clone())
        .unwrap_or_else(|| "none".into());
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterActivationImplementationGateReport {
        implementation_gate_eligible,
        draft_ready: draft.draft_ready,
        latest_decision,
        current_activation_plan_digest,
        activation_plan_digest_matched,
        default_chat_unchanged: draft.runtime_boundary_status.default_chat_unchanged,
        automatic_migration_enabled: draft.runtime_boundary_status.automatic_migration_enabled,
        current_mode: draft.runtime_boundary_status.current_mode.clone(),
        blocking_reasons,
        metadata_safe_summary: json!({
            "activationImplementationGate": "default_chat_adapter_activation",
            "metadataSafe": true,
            "readOnly": true,
            "notAutomaticMigration": true,
            "requiresSeparateImplementation": true,
            "implementationGateEligible": implementation_gate_eligible,
            "draftReady": draft.draft_ready,
            "latestDecisionKind": latest_decision_kind,
            "activationPlanDigestMatched": activation_plan_digest_matched,
            "candidatePromotionReady": draft.candidate_promotion_readiness_report.ready,
            "currentMode": draft.runtime_boundary_status.current_mode,
            "automaticMigrationEnabled": draft.runtime_boundary_status.automatic_migration_enabled,
            "defaultChatUnchanged": draft.runtime_boundary_status.default_chat_unchanged,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "transcriptStorage": "none",
            "agentRunStorage": "none",
            "modelCallStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn get_default_chat_adapter_routing_status(
    input: DefaultChatAdapterRoutingStatusInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterRoutingStatus, String> {
    get_default_chat_adapter_routing_status_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn get_default_chat_adapter_routing_status_with_state(
    input: DefaultChatAdapterRoutingStatusInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterRoutingStatus, String> {
    let activation_gate = check_default_chat_adapter_activation_implementation_gate_with_state(
        DefaultChatAdapterActivationImplementationGateInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: input.session_id,
        },
        state,
    )
    .await?;
    let current_mode = "legacy_stream".to_string();
    let adapter_scaffold_present = true;
    let controlled_adapter_enabled = false;
    let default_send_path = "legacy_stream".to_string();
    let start_stream_path = "legacy_stream".to_string();
    let requires_separate_cutover_implementation = true;
    let mut blocking_reasons = Vec::new();

    if !activation_gate.implementation_gate_eligible {
        push_unique_string(
            &mut blocking_reasons,
            "activation_implementation_gate_not_eligible".into(),
        );
        for reason in &activation_gate.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterRoutingStatus {
        current_mode,
        adapter_scaffold_present,
        controlled_adapter_enabled,
        default_send_path,
        start_stream_path,
        activation_implementation_gate_eligible: activation_gate.implementation_gate_eligible,
        requires_separate_cutover_implementation,
        blocking_reasons,
        metadata_safe_summary: json!({
            "defaultChatAdapterRouting": "disabled_scaffold",
            "metadataSafe": true,
            "readOnly": true,
            "routingMode": "legacy_stream",
            "adapterScaffoldPresent": adapter_scaffold_present,
            "controlledAdapterEnabled": controlled_adapter_enabled,
            "defaultSendPath": "legacy_stream",
            "startStreamPath": "legacy_stream",
            "activationImplementationGateEligible": activation_gate.implementation_gate_eligible,
            "notAutomaticMigration": true,
            "requiresSeparateCutoverImplementation": requires_separate_cutover_implementation,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "transcriptStorage": "none",
            "agentRunStorage": "none",
            "modelCallStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn check_default_chat_adapter_contract_harness(
    input: DefaultChatAdapterContractHarnessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterContractHarnessReport, String> {
    check_default_chat_adapter_contract_harness_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn check_default_chat_adapter_contract_harness_with_state(
    input: DefaultChatAdapterContractHarnessInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterContractHarnessReport, String> {
    let routing_status = get_default_chat_adapter_routing_status_with_state(
        DefaultChatAdapterRoutingStatusInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: input.session_id,
        },
        state,
    )
    .await?;

    let expected_path = "legacy_stream".to_string();
    let mut send_blocking_reasons = Vec::new();
    if routing_status.default_send_path != expected_path {
        push_unique_string(
            &mut send_blocking_reasons,
            "default_send_path_drifted".into(),
        );
    }
    let send_message_contract = DefaultChatAdapterContractCheck {
        name: "send_message".into(),
        ready: send_blocking_reasons.is_empty(),
        expected_path: expected_path.clone(),
        actual_path: routing_status.default_send_path.clone(),
        blocking_reasons: send_blocking_reasons,
    };

    let mut stream_blocking_reasons = Vec::new();
    if routing_status.start_stream_path != expected_path {
        push_unique_string(
            &mut stream_blocking_reasons,
            "start_stream_path_drifted".into(),
        );
    }
    let stream_message_contract = DefaultChatAdapterContractCheck {
        name: "start_stream_message".into(),
        ready: stream_blocking_reasons.is_empty(),
        expected_path,
        actual_path: routing_status.start_stream_path.clone(),
        blocking_reasons: stream_blocking_reasons,
    };

    let mut blocking_reasons = routing_status.blocking_reasons.clone();
    if !routing_status.adapter_scaffold_present {
        push_unique_string(&mut blocking_reasons, "adapter_scaffold_missing".into());
    }
    if routing_status.controlled_adapter_enabled {
        push_unique_string(&mut blocking_reasons, "controlled_adapter_enabled".into());
    }
    if routing_status.current_mode != "legacy_stream" {
        push_unique_string(&mut blocking_reasons, "default_chat_mode_drifted".into());
    }
    for reason in &send_message_contract.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &stream_message_contract.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }

    let adapter_disabled = routing_status.adapter_scaffold_present
        && !routing_status.controlled_adapter_enabled
        && routing_status.current_mode == "legacy_stream"
        && routing_status.default_send_path == "legacy_stream"
        && routing_status.start_stream_path == "legacy_stream";
    let contract_shape = "disabled_adapter_legacy_stream_contract".to_string();
    let activation_implementation_gate_eligible =
        routing_status.activation_implementation_gate_eligible;
    let contract_harness_ready = adapter_disabled
        && activation_implementation_gate_eligible
        && send_message_contract.ready
        && stream_message_contract.ready
        && blocking_reasons.is_empty();
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterContractHarnessReport {
        contract_harness_ready,
        contract_shape: contract_shape.clone(),
        adapter_disabled,
        activation_implementation_gate_eligible,
        routing_status,
        send_message_contract,
        stream_message_contract,
        blocking_reasons,
        metadata_safe_summary: json!({
            "contractHarness": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "contractHarnessReady": contract_harness_ready,
            "contractShape": contract_shape,
            "adapterDisabled": adapter_disabled,
            "activationImplementationGateEligible": activation_implementation_gate_eligible,
            "currentMode": "legacy_stream",
            "defaultSendPath": "legacy_stream",
            "startStreamPath": "legacy_stream",
            "controlledAdapterEnabled": false,
            "notAutomaticMigration": true,
            "requiresSeparateCutoverImplementation": true,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "transcriptStorage": "none",
            "agentRunStorage": "none",
            "modelCallStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn run_default_chat_adapter_dry_run(
    input: DefaultChatAdapterDryRunInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterDryRunReport, String> {
    run_default_chat_adapter_dry_run_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn run_default_chat_adapter_dry_run_with_state(
    input: DefaultChatAdapterDryRunInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterDryRunReport, String> {
    let source_session_id = safe_internal_id(&input.session_id, "sessionId")?;
    let input_message_length = input.message.chars().count();
    let input_message_hash = sha256_hex(&input.message);
    let contract_shape = "default_chat_adapter_dry_run_contract".to_string();
    let allow_writes = false;
    let max_tool_calls = 0;
    let default_chat_path_unchanged = true;
    let chat_message_saved = false;
    let agent_run_recorded = false;

    let contract_harness = check_default_chat_adapter_contract_harness_with_state(
        DefaultChatAdapterContractHarnessInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: Some(source_session_id.clone()),
        },
        state,
    )
    .await?;

    let mut blocking_reasons = contract_harness.blocking_reasons.clone();
    if !contract_harness.contract_harness_ready {
        push_unique_string(&mut blocking_reasons, "contract_harness_not_ready".into());
    }

    let dry_run_ready = contract_harness.contract_harness_ready && blocking_reasons.is_empty();
    let blocked = !dry_run_ready;
    let adapter_path = if dry_run_ready {
        "controlled_adapter_dry_run"
    } else {
        "blocked"
    }
    .to_string();
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterDryRunReport {
        dry_run_ready,
        blocked,
        contract_shape: contract_shape.clone(),
        source_session_id,
        adapter_path: adapter_path.clone(),
        allow_writes,
        max_tool_calls,
        default_chat_path_unchanged,
        chat_message_saved,
        agent_run_recorded,
        contract_harness_ready: contract_harness.contract_harness_ready,
        input_message_length,
        input_message_hash,
        user_output_preview: None,
        blocking_reasons,
        metadata_safe_summary: json!({
            "adapterDryRun": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "dryRunReady": dry_run_ready,
            "blocked": blocked,
            "contractShape": contract_shape,
            "adapterPath": adapter_path,
            "contractHarnessReady": contract_harness.contract_harness_ready,
            "allowWrites": allow_writes,
            "maxToolCalls": max_tool_calls,
            "defaultChatPathUnchanged": default_chat_path_unchanged,
            "chatMessageSaved": chat_message_saved,
            "agentRunRecorded": agent_run_recorded,
            "runtimeCallStorage": "none",
            "modelCallStorage": "none",
            "contentStorage": "length_checksum_only",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "externalWriteStorage": "none",
            "transcriptStorage": "none",
            "notAutomaticMigration": true,
            "defaultSendPath": "legacy_stream",
            "startStreamPath": "legacy_stream",
            "blockingReasonCount": blocking_reason_count,
        }),
    })
}

#[tauri::command]
pub async fn record_default_chat_adapter_dry_run_review_decision(
    input: DefaultChatAdapterDryRunReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterDryRunReviewDecisionResult, String> {
    record_default_chat_adapter_dry_run_review_decision_with_state(input, &state.inner().clone())
        .await
}

pub(crate) async fn record_default_chat_adapter_dry_run_review_decision_with_state(
    input: DefaultChatAdapterDryRunReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterDryRunReviewDecisionResult, String> {
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let expected_digest = input
        .dry_run_summary_digest
        .as_deref()
        .map(|value| safe_checksum_field(value, "dryRunSummaryDigest"))
        .transpose()?;

    let dry_run = run_default_chat_adapter_dry_run_with_state(
        DefaultChatAdapterDryRunInput {
            session_id: source_session_id.clone(),
            message: input.message,
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
        },
        state,
    )
    .await?;
    let dry_run_summary_digest = metadata_hash_for_serializable(&dry_run)?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let mut blocking_reasons = dry_run.blocking_reasons.clone();

    if expected_digest
        .as_ref()
        .is_some_and(|expected| expected != &dry_run_summary_digest)
    {
        push_unique_string(
            &mut blocking_reasons,
            "dry_run_summary_digest_mismatch".into(),
        );
        return Ok(DefaultChatAdapterDryRunReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            decision_kind,
            source_session_id,
            contract_shape: dry_run.contract_shape,
            dry_run_ready: dry_run.dry_run_ready,
            dry_run_summary_digest,
            created_at,
            blocking_reasons,
        });
    }

    if decision_kind == "approve" && !dry_run.dry_run_ready {
        push_unique_string(
            &mut blocking_reasons,
            "dry_run_not_ready_for_approval".into(),
        );
        return Ok(DefaultChatAdapterDryRunReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            decision_kind,
            source_session_id,
            contract_shape: dry_run.contract_shape,
            dry_run_ready: false,
            dry_run_summary_digest,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        DEFAULT_CHAT_ADAPTER_DRY_RUN_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "evidenceKind": "default_chat_adapter_dry_run_review_decision",
        "decisionKind": decision_kind.clone(),
        "sourceSessionId": source_session_id.clone(),
        "contractShape": dry_run.contract_shape.clone(),
        "dryRunReady": dry_run.dry_run_ready,
        "dryRunSummaryDigest": dry_run_summary_digest.clone(),
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!("failed to record default Chat adapter dry-run review decision evidence: {e}")
        })?
    };

    Ok(DefaultChatAdapterDryRunReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        decision_kind,
        source_session_id,
        contract_shape: dry_run.contract_shape,
        dry_run_ready: dry_run.dry_run_ready,
        dry_run_summary_digest,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_default_chat_adapter_dry_run_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterDryRunReviewSummary, String> {
    get_default_chat_adapter_dry_run_review_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_default_chat_adapter_dry_run_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterDryRunReviewSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    DEFAULT_CHAT_ADAPTER_DRY_RUN_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| {
                format!("failed to read default Chat adapter dry-run review evidence: {e}")
            })?
    };
    let records = records
        .into_iter()
        .filter(default_chat_adapter_dry_run_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_dry_run_review_decision_kind(record) == Some("approve")
        })
        .count();
    let reject_or_rework_count = records
        .iter()
        .filter(|record| {
            matches!(
                default_chat_adapter_dry_run_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records
        .first()
        .and_then(default_chat_adapter_dry_run_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());
    let latest_decision_present = latest_decision.is_some();
    let blocking_reasons = if latest_decision_present {
        Vec::new()
    } else {
        vec!["dry_run_review_decision_missing".into()]
    };
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterDryRunReviewSummary {
        latest_decision,
        approved_count,
        reject_or_rework_count,
        latest_timestamp,
        blocking_reasons,
        metadata_safe_summary: json!({
            "dryRunReview": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "approvedCount": approved_count,
            "rejectOrReworkCount": reject_or_rework_count,
            "latestDecisionPresent": latest_decision_present,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "none",
            "reviewerNoteStorage": "length_checksum_category_only",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "agentRunStorage": "none",
            "modelCallStorage": "none",
            "externalWriteStorage": "none",
            "transcriptStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn check_default_chat_adapter_implementation_readiness(
    input: DefaultChatAdapterImplementationReadinessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterImplementationReadinessReport, String> {
    check_default_chat_adapter_implementation_readiness_with_state(input, &state.inner().clone())
        .await
}

pub(crate) async fn check_default_chat_adapter_implementation_readiness_with_state(
    input: DefaultChatAdapterImplementationReadinessInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterImplementationReadinessReport, String> {
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let activation_gate = check_default_chat_adapter_activation_implementation_gate_with_state(
        DefaultChatAdapterActivationImplementationGateInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: Some(source_session_id.clone()),
        },
        state,
    )
    .await?;
    let contract_harness = check_default_chat_adapter_contract_harness_with_state(
        DefaultChatAdapterContractHarnessInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: Some(source_session_id.clone()),
        },
        state,
    )
    .await?;
    let dry_run = run_default_chat_adapter_dry_run_with_state(
        DefaultChatAdapterDryRunInput {
            session_id: source_session_id,
            message: input.message,
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
        },
        state,
    )
    .await?;
    let dry_run_review_summary =
        get_default_chat_adapter_dry_run_review_summary_with_state(state).await?;
    let current_dry_run_digest = metadata_hash_for_serializable(&dry_run)?;
    let latest_dry_run_review_decision = dry_run_review_summary.latest_decision.clone();

    let activation_implementation_gate_eligible = activation_gate.implementation_gate_eligible;
    let contract_harness_ready = contract_harness.contract_harness_ready;
    let dry_run_ready = dry_run.dry_run_ready;
    let dry_run_review_approved = latest_dry_run_review_decision
        .as_ref()
        .is_some_and(|decision| decision.decision_kind == "approve" && decision.dry_run_ready);
    let dry_run_digest_matched = latest_dry_run_review_decision
        .as_ref()
        .is_some_and(|decision| decision.dry_run_summary_digest == current_dry_run_digest);
    let default_send_path = contract_harness.routing_status.default_send_path.clone();
    let start_stream_path = contract_harness.routing_status.start_stream_path.clone();
    let controlled_adapter_enabled = contract_harness.routing_status.controlled_adapter_enabled;
    let automatic_migration_enabled = activation_gate.automatic_migration_enabled;
    let default_chat_unchanged = activation_gate.default_chat_unchanged
        && contract_harness.routing_status.current_mode == "legacy_stream"
        && default_send_path == "legacy_stream"
        && start_stream_path == "legacy_stream";

    let mut blocking_reasons = Vec::new();
    for reason in &activation_gate.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &contract_harness.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &dry_run.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &dry_run_review_summary.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }

    if !activation_implementation_gate_eligible {
        push_unique_string(
            &mut blocking_reasons,
            "activation_implementation_gate_not_eligible".into(),
        );
    }
    if !contract_harness_ready {
        push_unique_string(&mut blocking_reasons, "contract_harness_not_ready".into());
    }
    if !dry_run_ready {
        push_unique_string(&mut blocking_reasons, "dry_run_not_ready".into());
    }
    match latest_dry_run_review_decision.as_ref() {
        Some(decision) => {
            if decision.decision_kind != "approve" {
                push_unique_string(
                    &mut blocking_reasons,
                    "latest_dry_run_review_not_approve".into(),
                );
            }
            if !decision.dry_run_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "approved_dry_run_review_not_ready".into(),
                );
            }
            if decision.decision_kind == "approve" && !dry_run_digest_matched {
                push_unique_string(
                    &mut blocking_reasons,
                    "dry_run_review_digest_mismatch".into(),
                );
            }
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "dry_run_review_approval_missing".into(),
            );
        }
    }
    if !default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }
    if controlled_adapter_enabled {
        push_unique_string(&mut blocking_reasons, "controlled_adapter_enabled".into());
    }
    if automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }
    if default_send_path != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "default_send_path_not_legacy_stream".into(),
        );
    }
    if start_stream_path != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "start_stream_path_not_legacy_stream".into(),
        );
    }

    let implementation_ready = activation_implementation_gate_eligible
        && contract_harness_ready
        && dry_run_ready
        && dry_run_review_approved
        && dry_run_digest_matched
        && default_chat_unchanged
        && !controlled_adapter_enabled
        && !automatic_migration_enabled
        && default_send_path == "legacy_stream"
        && start_stream_path == "legacy_stream"
        && blocking_reasons.is_empty();
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterImplementationReadinessReport {
        implementation_ready,
        latest_dry_run_review_decision,
        activation_implementation_gate_eligible,
        contract_harness_ready,
        dry_run_ready,
        dry_run_review_approved,
        dry_run_digest_matched,
        default_chat_unchanged,
        controlled_adapter_enabled,
        automatic_migration_enabled,
        default_send_path: default_send_path.clone(),
        start_stream_path: start_stream_path.clone(),
        blocking_reasons,
        metadata_safe_summary: json!({
            "implementationReadiness": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "implementationReady": implementation_ready,
            "activationImplementationGateEligible": activation_implementation_gate_eligible,
            "contractHarnessReady": contract_harness_ready,
            "dryRunReady": dry_run_ready,
            "dryRunReviewApproved": dry_run_review_approved,
            "dryRunDigestMatched": dry_run_digest_matched,
            "defaultChatUnchanged": default_chat_unchanged,
            "controlledAdapterEnabled": controlled_adapter_enabled,
            "automaticMigrationEnabled": automatic_migration_enabled,
            "defaultSendPath": default_send_path,
            "startStreamPath": start_stream_path,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "length_checksum_only",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "agentRunStorage": "none",
            "runtimeCallStorage": "none",
            "modelCallStorage": "none",
            "externalWriteStorage": "none",
            "transcriptStorage": "none",
            "notAutomaticMigration": true,
        }),
    })
}

struct NormalizedPromotionEvidenceInput {
    pilot_run_id: String,
    source_session_id: String,
    target_session_id: String,
    strategy_kind: String,
    payload_kind: String,
    governance_decision_kind: String,
    promoted_message_length: usize,
    promoted_message_hash: String,
    promoted_at: String,
}

struct NormalizedShadowRunInput {
    session_id: String,
    user_input_checksum: Option<String>,
    descriptor_kind: String,
    required_promotions: Option<usize>,
}

struct NormalizedCutoverCandidateInput {
    session_id: String,
    user_input_checksum: Option<String>,
    descriptor_kind: String,
    required_promotions: Option<usize>,
}

fn normalize_promotion_evidence_input(
    input: ControlledPilotPromotionEvidenceInput,
) -> Result<NormalizedPromotionEvidenceInput, String> {
    let pilot_run_id = safe_internal_id(&input.pilot_run_id, "pilotRunId")?;
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let target_session_id = safe_internal_id(&input.target_session_id, "targetSessionId")?;
    if source_session_id != target_session_id {
        return Err("sourceSessionId must match targetSessionId for promotion evidence".into());
    }
    let strategy_kind = safe_enum_value(
        &input.strategy_kind,
        "strategyKind",
        &["react", "planExecute"],
    )?;
    let payload_kind = safe_enum_value(
        &input.payload_kind,
        "payloadKind",
        &["react", "planExecute", "blocked"],
    )?;
    let governance_decision_kind = safe_enum_value(
        input
            .governance_decision_kind
            .as_deref()
            .unwrap_or("unknown"),
        "governanceDecisionKind",
        &["allow", "warn", "block", "unknown"],
    )?;
    if input.promoted_message_length == 0 {
        return Err("promotedMessageLength must be greater than zero".into());
    }
    let promoted_message_hash = safe_checksum(&input.promoted_message_hash)?;
    let promoted_at = match input.promoted_at.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => {
            chrono::DateTime::parse_from_rfc3339(value)
                .map_err(|_| "promotedAt must be an RFC3339 timestamp".to_string())?;
            value.to_string()
        }
        _ => chrono::Utc::now().to_rfc3339(),
    };

    Ok(NormalizedPromotionEvidenceInput {
        pilot_run_id,
        source_session_id,
        target_session_id,
        strategy_kind,
        payload_kind,
        governance_decision_kind,
        promoted_message_length: input.promoted_message_length,
        promoted_message_hash,
        promoted_at,
    })
}

fn normalize_shadow_run_input(
    input: ControlledChatMigrationShadowRunInput,
) -> Result<NormalizedShadowRunInput, String> {
    let session_id = safe_internal_id(&input.session_id, "sessionId")?;
    let user_input_checksum = input
        .user_input_checksum
        .as_deref()
        .map(|value| safe_checksum_field(value, "userInputChecksum"))
        .transpose()?;
    let descriptor_kind = match input
        .bounded_test_prompt_descriptor
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => safe_enum_value(
            value,
            "boundedTestPromptDescriptor",
            &[
                "default_readiness_probe",
                "planning_readiness_probe",
                "sensitive_local_only_probe",
            ],
        )?,
        None if user_input_checksum.is_some() => "default_readiness_probe".into(),
        None => {
            return Err(
                "userInputChecksum or boundedTestPromptDescriptor is required for shadow run"
                    .into(),
            )
        }
    };

    Ok(NormalizedShadowRunInput {
        session_id,
        user_input_checksum,
        descriptor_kind,
        required_promotions: input.required_promotions,
    })
}

fn normalize_cutover_candidate_input(
    input: ControlledChatCutoverCandidateInput,
) -> Result<NormalizedCutoverCandidateInput, String> {
    let session_id = safe_internal_id(&input.session_id, "sessionId")?;
    let user_input_checksum = input
        .user_input_checksum
        .as_deref()
        .map(|value| safe_checksum_field(value, "userInputChecksum"))
        .transpose()?;
    let descriptor_kind = match input
        .bounded_test_prompt_descriptor
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => safe_enum_value(
            value,
            "boundedTestPromptDescriptor",
            &["default_contract_probe", "concise_response_probe"],
        )?,
        None => "default_contract_probe".into(),
    };

    Ok(NormalizedCutoverCandidateInput {
        session_id,
        user_input_checksum,
        descriptor_kind,
        required_promotions: input.required_promotions,
    })
}

fn safe_internal_id(value: &str, field: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} is required"));
    }
    if trimmed.len() > 160 || !trimmed.chars().all(is_safe_metadata_token_char) {
        return Err(format!("{field} must be an internal metadata id"));
    }
    Ok(trimmed.to_string())
}

fn normalize_optional_internal_id(
    value: Option<&str>,
    field: &str,
) -> Result<Option<String>, String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| safe_internal_id(value, field))
        .transpose()
}

fn safe_enum_value(value: &str, field: &str, allowed: &[&str]) -> Result<String, String> {
    let trimmed = value.trim();
    if allowed
        .iter()
        .any(|allowed_value| allowed_value == &trimmed)
    {
        Ok(trimmed.to_string())
    } else {
        Err(format!("{field} is not an allowed metadata value"))
    }
}

fn safe_checksum(value: &str) -> Result<String, String> {
    safe_checksum_field(value, "promotedMessageHash")
}

fn safe_checksum_field(value: &str, field: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} is required"));
    }
    if trimmed.len() > 160 || !trimmed.chars().all(is_safe_checksum_char) {
        return Err(format!("{field} must be a metadata-safe checksum"));
    }
    Ok(trimmed.to_string())
}

fn is_safe_metadata_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.')
}

fn is_safe_checksum_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.')
}

fn metadata_hash_for_serializable<T: Serialize>(value: &T) -> Result<String, String> {
    let serialized = serde_json::to_string(value)
        .map_err(|e| format!("failed to serialize metadata-safe draft for hashing: {e}"))?;
    Ok(sha256_metadata_checksum(&serialized))
}

fn default_chat_adapter_activation_plan_digest(
    draft: &DefaultChatAdapterActivationPlanDraft,
) -> Result<String, String> {
    let mut value = serde_json::to_value(draft)
        .map_err(|e| format!("failed to serialize activation plan draft for hashing: {e}"))?;
    if let Some(report) = value
        .get_mut("candidatePromotionReadinessReport")
        .and_then(Value::as_object_mut)
    {
        report.remove("checkedAt");
    }
    metadata_hash_for_serializable(&value)
}

fn sha256_metadata_checksum(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn metadata_safe_reviewer_note(note: Option<&str>) -> Value {
    let note = note.unwrap_or_default();
    let length = note.chars().count();
    let category = match length {
        0 => "none",
        1..=120 => "brief",
        121..=1000 => "standard",
        _ => "extended",
    };
    let checksum = if length == 0 {
        Value::Null
    } else {
        Value::String(sha256_metadata_checksum(note))
    };

    json!({
        "present": length > 0,
        "length": length,
        "checksum": checksum,
        "category": category
    })
}

struct ReviewerNoteMetadataFields {
    checksum: Value,
    length: usize,
    category: String,
}

fn metadata_safe_reviewer_note_fields(note: Option<&str>) -> ReviewerNoteMetadataFields {
    let note = note.unwrap_or_default();
    let length = note.chars().count();
    let category = match length {
        0 => "none",
        1..=120 => "brief",
        121..=1000 => "standard",
        _ => "extended",
    }
    .to_string();
    let checksum = if length == 0 {
        Value::Null
    } else {
        Value::String(sha256_metadata_checksum(note))
    };

    ReviewerNoteMetadataFields {
        checksum,
        length,
        category,
    }
}

fn promotion_evidence_pilot_run_id(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<String> {
    record
        .run_metadata
        .get("pilotRunId")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| record.linked_agent_run_ids.first().cloned())
}

fn promotion_evidence_timestamp(record: &openlife_core::agent::EvidenceRecord) -> String {
    record
        .run_metadata
        .get("promotedAt")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| record.created_at.to_rfc3339())
}

fn promotion_evidence_is_metadata_safe(record: &openlife_core::agent::EvidenceRecord) -> bool {
    if record.affected_path != CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH
        || record.evidence_type != EvidenceType::RuntimeBehavior
    {
        return false;
    }
    let metadata = &record.run_metadata;
    let expected_flags = metadata
        .get("evidenceKind")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "controlled_pilot_promotion")
        && metadata
            .get("metadataSafe")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && metadata
            .get("contentStorage")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "checksum_only")
        && metadata
            .get("toolStorage")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "none");

    expected_flags
        && promotion_evidence_pilot_run_id(record).is_some()
        && metadata_string_is_safe(metadata, "pilotRunId", safe_internal_id)
        && metadata_string_is_safe(metadata, "sourceSessionId", safe_internal_id)
        && metadata_string_is_safe(metadata, "targetSessionId", safe_internal_id)
        && metadata_string_is_safe(metadata, "strategyKind", |value, field| {
            safe_enum_value(value, field, &["react", "planExecute"])
        })
        && metadata_string_is_safe(metadata, "payloadKind", |value, field| {
            safe_enum_value(value, field, &["react", "planExecute", "blocked"])
        })
        && metadata_string_is_safe(metadata, "promotedMessageHash", |value, _field| {
            safe_checksum(value)
        })
        && !contains_unsafe_promotion_metadata(metadata)
}

fn migration_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != CONTROLLED_CHAT_MIGRATION_REVIEW_DECISION_EVIDENCE_PATH
        || record.evidence_type != EvidenceType::RuntimeBehavior
        || !record.linked_agent_run_ids.is_empty()
        || !record.linked_proposal_ids.is_empty()
    {
        return false;
    }
    let metadata = &record.run_metadata;
    metadata
        .get("evidenceKind")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "migration_review_decision")
        && metadata
            .get("metadataSafe")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && metadata
            .get("reviewerNoteStorage")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "length_checksum_category_only")
        && metadata
            .get("toolStorage")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "none")
        && metadata
            .get("transcriptStorage")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "none")
        && metadata_bool_is_present(metadata, "draftReady")
        && metadata_string_is_safe(metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && metadata_string_is_safe(metadata, "draftHash", |value, _field| safe_checksum(value))
        && metadata.get("createdAt").and_then(Value::as_str).is_some()
        && metadata
            .get("readinessCounts")
            .and_then(Value::as_object)
            .is_some()
        && reviewer_note_metadata_is_safe(metadata.get("reviewerNote"))
        && !contains_unsafe_promotion_metadata(metadata)
}

fn metadata_bool_is_present(metadata: &Value, key: &str) -> bool {
    metadata.get(key).and_then(Value::as_bool).is_some()
}

fn reviewer_note_metadata_is_safe(value: Option<&Value>) -> bool {
    let Some(Value::Object(note)) = value else {
        return false;
    };
    let category_is_bounded = note
        .get("category")
        .and_then(Value::as_str)
        .is_some_and(|value| matches!(value, "none" | "brief" | "standard" | "extended"));
    let checksum_is_safe = match note.get("checksum") {
        Some(Value::Null) => true,
        Some(Value::String(value)) => safe_checksum(value).is_ok(),
        _ => false,
    };

    note.get("length").and_then(Value::as_u64).is_some()
        && note.get("present").and_then(Value::as_bool).is_some()
        && category_is_bounded
        && checksum_is_safe
}

fn migration_review_decision_kind(record: &openlife_core::agent::EvidenceRecord) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn migration_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<ControlledChatMigrationReviewLatestDecision> {
    Some(ControlledChatMigrationReviewLatestDecision {
        evidence_id: record.id.clone(),
        decision_kind: migration_review_decision_kind(record)?.to_string(),
        draft_ready: record
            .run_metadata
            .get("draftReady")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        draft_hash: record
            .run_metadata
            .get("draftHash")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        created_at: migration_review_decision_timestamp(record),
    })
}

fn migration_review_decision_timestamp(record: &openlife_core::agent::EvidenceRecord) -> String {
    record
        .run_metadata
        .get("createdAt")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| record.created_at.to_rfc3339())
}

fn migration_review_decision_blocking_reasons(
    record: &openlife_core::agent::EvidenceRecord,
) -> Vec<String> {
    record
        .run_metadata
        .get("blockingReasons")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

struct ShadowReviewReadiness {
    digest: String,
    blocking_reasons: Vec<String>,
}

struct CutoverCandidateReviewReadiness {
    digest: String,
    contract_shape: String,
    candidate_ready: bool,
    blocking_reasons: Vec<String>,
}

async fn load_shadow_review_run(
    state: &Arc<AppState>,
    shadow_run_id: &str,
) -> Result<Option<AgentRun>, String> {
    let Some(store_arc) = state.agent_run_store.as_ref() else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    store
        .get_run(shadow_run_id)
        .map_err(|e| format!("failed to read shadow AgentRun for review: {e}"))
}

fn shadow_review_readiness(run: Option<&AgentRun>) -> Result<ShadowReviewReadiness, String> {
    let Some(run) = run else {
        let summary = json!({
            "runFound": false,
            "metadataSafe": true,
            "sideEffectAuditReady": false,
        });
        return Ok(ShadowReviewReadiness {
            digest: metadata_hash_for_serializable(&summary)?,
            blocking_reasons: vec!["shadow_run_missing".into()],
        });
    };

    let audit = run
        .reasoning_trace
        .as_ref()
        .and_then(|trace| trace.strategy_result.as_ref());
    let metadata_safe = audit_bool(audit, "metadataSafe").unwrap_or(false);
    let allow_writes = shadow_review_allow_writes(audit);
    let storage = |key: &str| audit_string(audit, key).unwrap_or("missing");
    let declared_write_step_count =
        audit_u64_at(audit, &["writeControl", "declaredWriteStepCount"]).unwrap_or_default();
    let proposal_required_step_count =
        audit_u64_at(audit, &["writeControl", "proposalRequiredStepCount"]).unwrap_or_default();
    let proposal_id_count = audit_u64(audit, "proposalIdCount").unwrap_or_default();

    let side_effects_absent = run.user_input.is_none()
        && run.generated_proposals.is_empty()
        && run.actions.is_empty()
        && run.observations.is_empty()
        && run.tool_call_count == 0
        && proposal_id_count == 0
        && declared_write_step_count == 0
        && proposal_required_step_count == 0
        && allow_writes == Some(false);

    let summary = json!({
        "runFound": true,
        "shadowRunId": run.id,
        "reasoningStrategy": run.reasoning_strategy.as_deref().unwrap_or("missing"),
        "status": run.status.to_string(),
        "metadataSafe": metadata_safe,
        "allowWrites": allow_writes,
        "contentStorage": storage("contentStorage"),
        "toolStorage": storage("toolStorage"),
        "chatHistoryStorage": storage("chatHistoryStorage"),
        "proposalStorage": storage("proposalStorage"),
        "lifeModelPatchStorage": storage("lifeModelPatchStorage"),
        "memoryStorage": storage("memoryStorage"),
        "userInputStored": run.user_input.is_some(),
        "generatedProposalCount": run.generated_proposals.len(),
        "actionCount": run.actions.len(),
        "observationCount": run.observations.len(),
        "toolCallCount": run.tool_call_count,
        "proposalIdCount": proposal_id_count,
        "declaredWriteStepCount": declared_write_step_count,
        "proposalRequiredStepCount": proposal_required_step_count,
        "sideEffectsAbsent": side_effects_absent,
    });
    let mut blocking_reasons = Vec::new();

    if run.reasoning_strategy.as_deref() != Some("controlled_migration_shadow_run") {
        push_unique_string(&mut blocking_reasons, "shadow_run_strategy_mismatch".into());
    }
    if run.status != AgentRunStatus::Completed {
        push_unique_string(&mut blocking_reasons, "shadow_run_not_completed".into());
    }
    if audit.is_none() {
        push_unique_string(&mut blocking_reasons, "shadow_run_audit_missing".into());
    }
    if !metadata_safe {
        push_unique_string(&mut blocking_reasons, "shadow_run_metadata_not_safe".into());
    }
    if allow_writes != Some(false) {
        push_unique_string(
            &mut blocking_reasons,
            "shadow_run_allow_writes_not_false".into(),
        );
    }
    for (key, reason) in [
        ("contentStorage", "shadow_run_content_storage_not_none"),
        ("toolStorage", "shadow_run_tool_storage_not_none"),
        (
            "chatHistoryStorage",
            "shadow_run_chat_history_storage_not_none",
        ),
        ("proposalStorage", "shadow_run_proposal_storage_not_none"),
        (
            "lifeModelPatchStorage",
            "shadow_run_life_model_patch_storage_not_none",
        ),
        ("memoryStorage", "shadow_run_memory_storage_not_none"),
    ] {
        if audit_string(audit, key) != Some("none") {
            push_unique_string(&mut blocking_reasons, reason.into());
        }
    }
    if run.user_input.is_some() {
        push_unique_string(
            &mut blocking_reasons,
            "shadow_run_user_input_persisted".into(),
        );
    }
    if !run.generated_proposals.is_empty()
        || proposal_id_count > 0
        || proposal_required_step_count > 0
    {
        push_unique_string(
            &mut blocking_reasons,
            "shadow_run_proposal_side_effects_present".into(),
        );
    }
    if !run.actions.is_empty()
        || !run.observations.is_empty()
        || run.tool_call_count > 0
        || declared_write_step_count > 0
    {
        push_unique_string(
            &mut blocking_reasons,
            "shadow_run_external_write_side_effects_present".into(),
        );
    }

    Ok(ShadowReviewReadiness {
        digest: metadata_hash_for_serializable(&summary)?,
        blocking_reasons,
    })
}

async fn load_cutover_candidate_review_run(
    state: &Arc<AppState>,
    candidate_run_id: &str,
) -> Result<Option<AgentRun>, String> {
    let Some(store_arc) = state.agent_run_store.as_ref() else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    store
        .get_run(candidate_run_id)
        .map_err(|e| format!("failed to read cutover candidate AgentRun for review: {e}"))
}

fn cutover_candidate_review_readiness(
    run: Option<&AgentRun>,
) -> Result<CutoverCandidateReviewReadiness, String> {
    let Some(run) = run else {
        let summary = json!({
            "runFound": false,
            "metadataSafe": true,
            "sideEffectAuditReady": false,
        });
        return Ok(CutoverCandidateReviewReadiness {
            digest: metadata_hash_for_serializable(&summary)?,
            contract_shape: "missing".into(),
            candidate_ready: false,
            blocking_reasons: vec!["candidate_run_missing".into()],
        });
    };

    let audit = run
        .reasoning_trace
        .as_ref()
        .and_then(|trace| trace.strategy_result.as_ref());
    let metadata_safe = audit_bool(audit, "metadataSafe").unwrap_or(false);
    let contract_shape = audit_string(audit, "contractShape")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "missing".into());
    let contract_shape_allowed = matches!(
        contract_shape.as_str(),
        "send_message_compatible" | "blocked" | "failed"
    );
    let candidate_ready = audit_bool(audit, "candidateReady").unwrap_or(false);
    let allow_writes = cutover_candidate_review_allow_writes(audit);
    let max_tool_calls = cutover_candidate_review_max_tool_calls(audit);
    let storage = |key: &str| audit_string(audit, key).unwrap_or("missing");
    let declared_write_step_count =
        audit_u64_at(audit, &["writeControl", "declaredWriteStepCount"]).unwrap_or_default();
    let proposal_required_step_count =
        audit_u64_at(audit, &["writeControl", "proposalRequiredStepCount"]).unwrap_or_default();
    let proposal_id_count = audit_u64(audit, "proposalIdCount").unwrap_or_default();

    let side_effects_absent = run.user_input.is_none()
        && run.generated_proposals.is_empty()
        && run.actions.is_empty()
        && run.observations.is_empty()
        && run.tool_call_count == 0
        && proposal_id_count == 0
        && declared_write_step_count == 0
        && proposal_required_step_count == 0
        && allow_writes == Some(false)
        && max_tool_calls == Some(0);

    let summary = json!({
        "runFound": true,
        "candidateRunId": run.id,
        "reasoningStrategy": run.reasoning_strategy.as_deref().unwrap_or("missing"),
        "status": run.status.to_string(),
        "contractShape": contract_shape.clone(),
        "candidateReady": candidate_ready,
        "metadataSafe": metadata_safe,
        "allowWrites": allow_writes,
        "maxToolCalls": max_tool_calls,
        "contentStorage": storage("contentStorage"),
        "toolStorage": storage("toolStorage"),
        "chatHistoryStorage": storage("chatHistoryStorage"),
        "proposalStorage": storage("proposalStorage"),
        "lifeModelPatchStorage": storage("lifeModelPatchStorage"),
        "memoryStorage": storage("memoryStorage"),
        "evidenceStorage": storage("evidenceStorage"),
        "mcpAuditStorage": storage("mcpAuditStorage"),
        "userInputStored": run.user_input.is_some(),
        "generatedProposalCount": run.generated_proposals.len(),
        "actionCount": run.actions.len(),
        "observationCount": run.observations.len(),
        "toolCallCount": run.tool_call_count,
        "proposalIdCount": proposal_id_count,
        "declaredWriteStepCount": declared_write_step_count,
        "proposalRequiredStepCount": proposal_required_step_count,
        "sideEffectsAbsent": side_effects_absent,
    });
    let mut blocking_reasons = Vec::new();

    if run.reasoning_strategy.as_deref() != Some("controlled_chat_cutover_candidate") {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_strategy_mismatch".into(),
        );
    }
    if run.status != AgentRunStatus::Completed {
        push_unique_string(&mut blocking_reasons, "candidate_run_not_completed".into());
    }
    if audit.is_none() {
        push_unique_string(&mut blocking_reasons, "candidate_run_audit_missing".into());
    }
    if !contract_shape_allowed {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_contract_shape_invalid".into(),
        );
    }
    if !metadata_safe {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_metadata_not_safe".into(),
        );
    }
    if allow_writes != Some(false) {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_allow_writes_not_false".into(),
        );
    }
    if max_tool_calls != Some(0) {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_max_tool_calls_not_zero".into(),
        );
    }
    for (key, reason) in [
        ("contentStorage", "candidate_run_content_storage_not_none"),
        ("toolStorage", "candidate_run_tool_storage_not_none"),
        (
            "chatHistoryStorage",
            "candidate_run_chat_history_storage_not_none",
        ),
        ("proposalStorage", "candidate_run_proposal_storage_not_none"),
        (
            "lifeModelPatchStorage",
            "candidate_run_life_model_patch_storage_not_none",
        ),
        ("memoryStorage", "candidate_run_memory_storage_not_none"),
        ("evidenceStorage", "candidate_run_evidence_storage_not_none"),
        (
            "mcpAuditStorage",
            "candidate_run_mcp_audit_storage_not_none",
        ),
    ] {
        if audit_string(audit, key) != Some("none") {
            push_unique_string(&mut blocking_reasons, reason.into());
        }
    }
    if run.user_input.is_some() {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_user_input_persisted".into(),
        );
    }
    if !run.generated_proposals.is_empty()
        || proposal_id_count > 0
        || proposal_required_step_count > 0
    {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_proposal_side_effects_present".into(),
        );
    }
    if !run.actions.is_empty()
        || !run.observations.is_empty()
        || run.tool_call_count > 0
        || declared_write_step_count > 0
    {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_external_write_side_effects_present".into(),
        );
    }

    Ok(CutoverCandidateReviewReadiness {
        digest: metadata_hash_for_serializable(&summary)?,
        contract_shape,
        candidate_ready,
        blocking_reasons,
    })
}

struct CutoverReadinessMetadataSafeSummaryInput<'a> {
    cutover_planning_eligible: bool,
    required_evidence_ready: bool,
    default_chat_unchanged: bool,
    implementation_eligible: bool,
    latest_shadow_decision_kind: &'a str,
    shadow_run_ready: bool,
    verified_shadow_run_id: Option<&'a str>,
    readiness_summary_digest: Option<&'a str>,
    shadow_review_summary: &'a ControlledChatMigrationShadowReviewSummary,
}

struct CutoverCandidatePromotionReadinessMetadataSafeSummaryInput<'a> {
    ready: bool,
    cutover_readiness_eligible: bool,
    required_approved_candidates: usize,
    approved_candidate_count: usize,
    latest_decision_kind: &'a str,
    default_chat_unchanged: bool,
    verified_candidate_count: usize,
    blocking_reason_count: usize,
}

fn cutover_readiness_metadata_safe_summary(
    input: CutoverReadinessMetadataSafeSummaryInput<'_>,
) -> Value {
    json!({
        "cutoverReadinessGate": "controlled_chat_cutover_planning",
        "metadataSafe": true,
        "planningOnly": true,
        "notAutomaticMigration": true,
        "cutoverPlanningEligible": input.cutover_planning_eligible,
        "requiredEvidenceReady": input.required_evidence_ready,
        "defaultChatUnchanged": input.default_chat_unchanged,
        "implementationEligible": input.implementation_eligible,
        "latestShadowReviewDecisionKind": input.latest_shadow_decision_kind,
        "shadowRunReady": input.shadow_run_ready,
        "verifiedShadowRunId": input.verified_shadow_run_id.unwrap_or("none"),
        "readinessSummaryDigest": input.readiness_summary_digest.unwrap_or("none"),
        "approvedShadowReviewCount": input.shadow_review_summary.approved_count,
        "shadowReviewReworkRejectCount": input.shadow_review_summary.rework_reject_count,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "reviewerNoteStorage": "length_checksum_category_only",
        "transcriptStorage": "none",
    })
}

fn cutover_candidate_promotion_readiness_metadata_safe_summary(
    input: CutoverCandidatePromotionReadinessMetadataSafeSummaryInput<'_>,
) -> Value {
    json!({
        "promotionReadinessGate": "controlled_chat_cutover_candidate",
        "metadataSafe": true,
        "readOnly": true,
        "notAutomaticMigration": true,
        "ready": input.ready,
        "cutoverReadinessEligible": input.cutover_readiness_eligible,
        "requiredApprovedCandidates": input.required_approved_candidates,
        "approvedCandidateCount": input.approved_candidate_count,
        "verifiedCandidateCount": input.verified_candidate_count,
        "latestDecisionKind": input.latest_decision_kind,
        "defaultChatUnchanged": input.default_chat_unchanged,
        "blockingReasonCount": input.blocking_reason_count,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "read_only",
        "mcpAuditStorage": "none",
        "reviewerNoteStorage": "length_checksum_category_only",
        "transcriptStorage": "none",
    })
}

fn cutover_candidate_blocked_summary(
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
) -> Value {
    json!({
        "candidateAdapter": "controlled_chat_cutover_candidate",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "candidateReady": false,
        "contractShape": "blocked",
        "blockedBeforeRuntime": true,
        "allowWrites": false,
        "maxToolCalls": 0,
        "metadataSafe": true,
        "nonDefault": true,
        "defaultChatUnchanged": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "none",
        "mcpAuditStorage": "none",
    })
}

fn cutover_candidate_failed_summary(
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
    safe_error: &str,
) -> Value {
    json!({
        "candidateAdapter": "controlled_chat_cutover_candidate",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "candidateReady": false,
        "contractShape": "failed",
        "candidateErrorCode": cutover_candidate_error_code(safe_error),
        "allowWrites": false,
        "maxToolCalls": 0,
        "metadataSafe": true,
        "nonDefault": true,
        "defaultChatUnchanged": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "none",
        "mcpAuditStorage": "none",
    })
}

fn cutover_candidate_metadata_safe_summary(
    output: &MultiStrategyRuntimeOutput,
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
    contract_shape: &str,
    candidate_ready: bool,
    output_digest: Option<&str>,
) -> Value {
    let metadata = &output.selection.metadata_safe_summary;
    let governance_decision_kind = output
        .selection
        .governance_decision
        .as_ref()
        .map(|decision| preview_governance_decision_kind(decision.kind))
        .unwrap_or("unknown");
    json!({
        "candidateAdapter": "controlled_chat_cutover_candidate",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "contractShape": contract_shape,
        "candidateReady": candidate_ready,
        "strategyKind": preview_strategy_kind(output.selection.kind),
        "payloadKind": preview_payload_kind(&output.payload),
        "governanceDecisionKind": governance_decision_kind,
        "taskKind": metadata.get("taskKind").and_then(Value::as_str).unwrap_or("unknown"),
        "reasonCode": metadata.get("reasonCode").and_then(Value::as_str).unwrap_or("unknown"),
        "riskLevel": metadata.get("riskLevel").and_then(Value::as_str).unwrap_or("unknown"),
        "hasHsPacket": metadata.get("hasHsPacket").and_then(Value::as_bool).unwrap_or(false),
        "planStepCount": preview_plan_step_count(&output.payload),
        "proposalIdCount": preview_proposal_ids(&output.payload).len(),
        "blocked": matches!(output.payload, MultiStrategyRuntimePayload::Blocked),
        "userOutputPresent": cutover_candidate_user_output(output).is_some(),
        "outputDigestPresent": output_digest.is_some(),
        "allowWrites": false,
        "maxToolCalls": 0,
        "metadataSafe": true,
        "nonDefault": true,
        "defaultChatUnchanged": true,
        "proposalApply": false,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "none",
        "mcpAuditStorage": "none",
    })
}

fn cutover_candidate_audit_summary(
    output: &MultiStrategyRuntimeOutput,
    warnings: &[String],
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
    contract_shape: &str,
    candidate_ready: bool,
    output_digest: Option<&str>,
) -> Value {
    let mut write_control = preview_write_control(&output.payload);
    if let Some(map) = write_control.as_object_mut() {
        map.insert("allowWrites".into(), Value::Bool(false));
    }
    let metadata = cutover_candidate_metadata_safe_summary(
        output,
        descriptor_kind,
        user_input_checksum,
        contract_shape,
        candidate_ready,
        output_digest,
    );
    json!({
        "candidateAdapter": "controlled_chat_cutover_candidate",
        "strategyKind": metadata["strategyKind"],
        "payloadKind": metadata["payloadKind"],
        "contractShape": contract_shape,
        "candidateReady": candidate_ready,
        "governanceDecisionKind": metadata["governanceDecisionKind"],
        "taskKind": metadata["taskKind"],
        "reasonCode": metadata["reasonCode"],
        "riskLevel": metadata["riskLevel"],
        "hasHsPacket": metadata["hasHsPacket"],
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "planStepCount": metadata["planStepCount"],
        "planStepStatuses": preview_plan_step_statuses(&output.payload),
        "proposalIdCount": metadata["proposalIdCount"],
        "blocked": metadata["blocked"],
        "userOutputPresent": metadata["userOutputPresent"],
        "outputDigest": output_digest,
        "warnings": warnings,
        "metadataSafe": true,
        "nonDefault": true,
        "defaultChatUnchanged": true,
        "runtimeLimits": {
            "allowWrites": false,
            "maxToolCalls": 0
        },
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "none",
        "mcpAuditStorage": "none",
        "writeControl": write_control,
    })
}

fn shadow_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != CONTROLLED_CHAT_MIGRATION_SHADOW_REVIEW_DECISION_EVIDENCE_PATH
        || record.evidence_type != EvidenceType::RuntimeBehavior
        || record.summary.is_some()
        || !record.source_refs.is_empty()
        || !record.linked_agent_run_ids.is_empty()
        || !record.linked_proposal_ids.is_empty()
    {
        return false;
    }
    let Some(metadata) = record.run_metadata.as_object() else {
        return false;
    };
    let allowed = [
        "shadowRunId",
        "decisionKind",
        "reviewerNoteChecksum",
        "reviewerNoteLength",
        "reviewerNoteCategory",
        "readinessSummaryDigest",
        "createdAt",
    ];
    if metadata.len() != allowed.len()
        || !metadata.keys().all(|key| allowed.contains(&key.as_str()))
    {
        return false;
    }

    metadata_string_is_safe(&record.run_metadata, "shadowRunId", safe_internal_id)
        && metadata_string_is_safe(&record.run_metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && reviewer_note_flat_metadata_is_safe(&record.run_metadata)
        && metadata_string_is_safe(
            &record.run_metadata,
            "readinessSummaryDigest",
            |value, _| safe_checksum(value),
        )
        && record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        && !contains_unsafe_promotion_metadata(&record.run_metadata)
}

fn reviewer_note_flat_metadata_is_safe(metadata: &Value) -> bool {
    let length_is_safe = metadata
        .get("reviewerNoteLength")
        .and_then(Value::as_u64)
        .is_some();
    let category_is_safe = metadata
        .get("reviewerNoteCategory")
        .and_then(Value::as_str)
        .is_some_and(|value| matches!(value, "none" | "brief" | "standard" | "extended"));
    let checksum_is_safe = match metadata.get("reviewerNoteChecksum") {
        Some(Value::Null) => true,
        Some(Value::String(value)) => safe_checksum(value).is_ok(),
        _ => false,
    };

    length_is_safe && category_is_safe && checksum_is_safe
}

fn shadow_review_decision_kind(record: &openlife_core::agent::EvidenceRecord) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn shadow_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<ControlledChatMigrationShadowReviewLatestDecision> {
    Some(ControlledChatMigrationShadowReviewLatestDecision {
        evidence_id: record.id.clone(),
        shadow_run_id: record
            .run_metadata
            .get("shadowRunId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        decision_kind: shadow_review_decision_kind(record)?.to_string(),
        reviewer_note_checksum: record
            .run_metadata
            .get("reviewerNoteChecksum")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        reviewer_note_length: record
            .run_metadata
            .get("reviewerNoteLength")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        reviewer_note_category: record
            .run_metadata
            .get("reviewerNoteCategory")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        readiness_summary_digest: record
            .run_metadata
            .get("readinessSummaryDigest")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        created_at: record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| record.created_at.to_rfc3339()),
    })
}

fn cutover_candidate_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH
        || record.evidence_type != EvidenceType::RuntimeBehavior
        || record.summary.is_some()
        || !record.source_refs.is_empty()
        || !record.linked_agent_run_ids.is_empty()
        || !record.linked_proposal_ids.is_empty()
    {
        return false;
    }
    let Some(metadata) = record.run_metadata.as_object() else {
        return false;
    };
    let allowed = [
        "candidateRunId",
        "decisionKind",
        "contractShape",
        "candidateSummaryDigest",
        "reviewerNoteChecksum",
        "reviewerNoteLength",
        "reviewerNoteCategory",
        "createdAt",
    ];
    if metadata.len() != allowed.len()
        || !metadata.keys().all(|key| allowed.contains(&key.as_str()))
    {
        return false;
    }

    metadata_string_is_safe(&record.run_metadata, "candidateRunId", safe_internal_id)
        && metadata_string_is_safe(&record.run_metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && metadata_string_is_safe(&record.run_metadata, "contractShape", |value, field| {
            safe_enum_value(
                value,
                field,
                &["send_message_compatible", "blocked", "failed"],
            )
        })
        && metadata_string_is_safe(
            &record.run_metadata,
            "candidateSummaryDigest",
            |value, _| safe_checksum(value),
        )
        && reviewer_note_flat_metadata_is_safe(&record.run_metadata)
        && record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        && !contains_unsafe_promotion_metadata(&record.run_metadata)
}

fn cutover_candidate_review_decision_kind(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn cutover_candidate_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<ControlledChatCutoverCandidateReviewLatestDecision> {
    Some(ControlledChatCutoverCandidateReviewLatestDecision {
        evidence_id: record.id.clone(),
        candidate_run_id: record
            .run_metadata
            .get("candidateRunId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        decision_kind: cutover_candidate_review_decision_kind(record)?.to_string(),
        contract_shape: record
            .run_metadata
            .get("contractShape")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        candidate_summary_digest: record
            .run_metadata
            .get("candidateSummaryDigest")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        reviewer_note_checksum: record
            .run_metadata
            .get("reviewerNoteChecksum")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        reviewer_note_length: record
            .run_metadata
            .get("reviewerNoteLength")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        reviewer_note_category: record
            .run_metadata
            .get("reviewerNoteCategory")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        created_at: record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| record.created_at.to_rfc3339()),
    })
}

fn default_chat_adapter_activation_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != DEFAULT_CHAT_ADAPTER_ACTIVATION_REVIEW_DECISION_EVIDENCE_PATH
        || record.evidence_type != EvidenceType::RuntimeBehavior
        || record.summary.is_some()
        || !record.source_refs.is_empty()
        || !record.linked_agent_run_ids.is_empty()
        || !record.linked_proposal_ids.is_empty()
    {
        return false;
    }
    let Some(metadata) = record.run_metadata.as_object() else {
        return false;
    };
    let allowed = [
        "evidenceKind",
        "decisionKind",
        "draftReady",
        "activationPlanDigest",
        "candidatePromotionReady",
        "currentMode",
        "automaticMigrationEnabled",
        "reviewerNoteChecksum",
        "reviewerNoteLength",
        "reviewerNoteCategory",
        "createdAt",
    ];
    if metadata.len() != allowed.len()
        || !metadata.keys().all(|key| allowed.contains(&key.as_str()))
    {
        return false;
    }

    record
        .run_metadata
        .get("evidenceKind")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "default_chat_adapter_activation_review_decision")
        && metadata_string_is_safe(&record.run_metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && record
            .run_metadata
            .get("draftReady")
            .and_then(Value::as_bool)
            .is_some()
        && metadata_string_is_safe(&record.run_metadata, "activationPlanDigest", |value, _| {
            safe_checksum(value)
        })
        && record
            .run_metadata
            .get("candidatePromotionReady")
            .and_then(Value::as_bool)
            .is_some()
        && metadata_string_is_safe(&record.run_metadata, "currentMode", safe_internal_id)
        && record
            .run_metadata
            .get("automaticMigrationEnabled")
            .and_then(Value::as_bool)
            .is_some()
        && reviewer_note_flat_metadata_is_safe(&record.run_metadata)
        && record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        && !contains_unsafe_promotion_metadata(&record.run_metadata)
}

fn default_chat_adapter_activation_review_decision_kind(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn default_chat_adapter_activation_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<DefaultChatAdapterActivationReviewLatestDecision> {
    Some(DefaultChatAdapterActivationReviewLatestDecision {
        evidence_id: record.id.clone(),
        decision_kind: default_chat_adapter_activation_review_decision_kind(record)?.to_string(),
        draft_ready: record
            .run_metadata
            .get("draftReady")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        activation_plan_digest: record
            .run_metadata
            .get("activationPlanDigest")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        candidate_promotion_ready: record
            .run_metadata
            .get("candidatePromotionReady")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        current_mode: record
            .run_metadata
            .get("currentMode")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        automatic_migration_enabled: record
            .run_metadata
            .get("automaticMigrationEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        reviewer_note_checksum: record
            .run_metadata
            .get("reviewerNoteChecksum")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        reviewer_note_length: record
            .run_metadata
            .get("reviewerNoteLength")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        reviewer_note_category: record
            .run_metadata
            .get("reviewerNoteCategory")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        created_at: record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| record.created_at.to_rfc3339()),
    })
}

fn default_chat_adapter_dry_run_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != DEFAULT_CHAT_ADAPTER_DRY_RUN_REVIEW_DECISION_EVIDENCE_PATH
        || record.evidence_type != EvidenceType::RuntimeBehavior
        || record.summary.is_some()
        || !record.source_refs.is_empty()
        || !record.linked_agent_run_ids.is_empty()
        || !record.linked_proposal_ids.is_empty()
    {
        return false;
    }
    let Some(metadata) = record.run_metadata.as_object() else {
        return false;
    };
    let allowed = [
        "evidenceKind",
        "decisionKind",
        "sourceSessionId",
        "contractShape",
        "dryRunReady",
        "dryRunSummaryDigest",
        "reviewerNoteChecksum",
        "reviewerNoteLength",
        "reviewerNoteCategory",
        "createdAt",
    ];
    if metadata.len() != allowed.len()
        || !metadata.keys().all(|key| allowed.contains(&key.as_str()))
    {
        return false;
    }

    record
        .run_metadata
        .get("evidenceKind")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "default_chat_adapter_dry_run_review_decision")
        && metadata_string_is_safe(&record.run_metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && metadata_string_is_safe(&record.run_metadata, "sourceSessionId", safe_internal_id)
        && metadata_string_is_safe(&record.run_metadata, "contractShape", safe_internal_id)
        && record
            .run_metadata
            .get("dryRunReady")
            .and_then(Value::as_bool)
            .is_some()
        && metadata_string_is_safe(&record.run_metadata, "dryRunSummaryDigest", |value, _| {
            safe_checksum_field(value, "dryRunSummaryDigest")
        })
        && reviewer_note_flat_metadata_is_safe(&record.run_metadata)
        && record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        && !contains_unsafe_promotion_metadata(&record.run_metadata)
}

fn default_chat_adapter_dry_run_review_decision_kind(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn default_chat_adapter_dry_run_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<DefaultChatAdapterDryRunReviewLatestDecision> {
    Some(DefaultChatAdapterDryRunReviewLatestDecision {
        evidence_id: record.id.clone(),
        decision_kind: default_chat_adapter_dry_run_review_decision_kind(record)?.to_string(),
        source_session_id: record
            .run_metadata
            .get("sourceSessionId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        contract_shape: record
            .run_metadata
            .get("contractShape")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        dry_run_ready: record
            .run_metadata
            .get("dryRunReady")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        dry_run_summary_digest: record
            .run_metadata
            .get("dryRunSummaryDigest")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        reviewer_note_checksum: record
            .run_metadata
            .get("reviewerNoteChecksum")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        reviewer_note_length: record
            .run_metadata
            .get("reviewerNoteLength")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        reviewer_note_category: record
            .run_metadata
            .get("reviewerNoteCategory")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        created_at: record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| record.created_at.to_rfc3339()),
    })
}

fn shadow_review_allow_writes(audit: Option<&Value>) -> Option<bool> {
    audit_bool_at(audit, &["writeControl", "allowWrites"])
        .or_else(|| audit_bool(audit, "allowWrites"))
}

fn cutover_candidate_review_allow_writes(audit: Option<&Value>) -> Option<bool> {
    audit_bool_at(audit, &["runtimeLimits", "allowWrites"])
        .or_else(|| audit_bool_at(audit, &["writeControl", "allowWrites"]))
        .or_else(|| audit_bool(audit, "allowWrites"))
}

fn cutover_candidate_review_max_tool_calls(audit: Option<&Value>) -> Option<u64> {
    audit_u64_at(audit, &["runtimeLimits", "maxToolCalls"])
        .or_else(|| audit_u64(audit, "maxToolCalls"))
}

fn audit_bool(audit: Option<&Value>, key: &str) -> Option<bool> {
    audit?.get(key).and_then(Value::as_bool)
}

fn audit_string<'a>(audit: Option<&'a Value>, key: &str) -> Option<&'a str> {
    audit?.get(key).and_then(Value::as_str)
}

fn audit_u64(audit: Option<&Value>, key: &str) -> Option<u64> {
    audit?.get(key).and_then(Value::as_u64)
}

fn audit_bool_at(audit: Option<&Value>, path: &[&str]) -> Option<bool> {
    let mut value = audit?;
    for key in path {
        value = value.get(*key)?;
    }
    value.as_bool()
}

fn audit_u64_at(audit: Option<&Value>, path: &[&str]) -> Option<u64> {
    let mut value = audit?;
    for key in path {
        value = value.get(*key)?;
    }
    value.as_u64()
}

fn metadata_string_is_safe(
    metadata: &Value,
    key: &str,
    validator: impl Fn(&str, &str) -> Result<String, String>,
) -> bool {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|value| validator(value, key).is_ok())
}

fn contains_unsafe_promotion_metadata(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            promotion_metadata_key_is_raw_content(key) || contains_unsafe_promotion_metadata(value)
        }),
        Value::Array(items) => items.iter().any(contains_unsafe_promotion_metadata),
        Value::String(text) => looks_like_email_for_metadata(text),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn promotion_metadata_key_is_raw_content(key: &str) -> bool {
    matches!(
        normalize_metadata_key(key).as_str(),
        "rawprompt"
            | "rawpilotresponse"
            | "rawassistantresponse"
            | "rawuserinput"
            | "rawusertext"
            | "userinput"
            | "usertext"
            | "assistantresponse"
            | "pilotresponse"
            | "pilotoutput"
            | "assistantoutput"
            | "rawoutput"
            | "response"
            | "output"
            | "content"
            | "toolpayload"
            | "fulltoolpayload"
            | "toolresult"
            | "messages"
            | "prompt"
    )
}

fn shadow_blocked_summary(descriptor_kind: &str, user_input_checksum: Option<&str>) -> Value {
    json!({
        "shadowRunRuntime": "controlled_chat_migration",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "blockedBeforeRuntime": true,
        "allowWrites": false,
        "metadataSafe": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
    })
}

fn shadow_failed_summary(
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
    safe_error: &str,
) -> Value {
    json!({
        "shadowRunRuntime": "controlled_chat_migration",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "shadowErrorCode": shadow_error_code(safe_error),
        "allowWrites": false,
        "metadataSafe": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
    })
}

fn shadow_metadata_safe_summary(
    output: &MultiStrategyRuntimeOutput,
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
) -> Value {
    let metadata = &output.selection.metadata_safe_summary;
    let governance_decision_kind = output
        .selection
        .governance_decision
        .as_ref()
        .map(|decision| preview_governance_decision_kind(decision.kind))
        .unwrap_or("unknown");
    json!({
        "shadowRunRuntime": "controlled_chat_migration",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "strategyKind": preview_strategy_kind(output.selection.kind),
        "payloadKind": preview_payload_kind(&output.payload),
        "governanceDecisionKind": governance_decision_kind,
        "taskKind": metadata.get("taskKind").and_then(Value::as_str).unwrap_or("unknown"),
        "reasonCode": metadata.get("reasonCode").and_then(Value::as_str).unwrap_or("unknown"),
        "riskLevel": metadata.get("riskLevel").and_then(Value::as_str).unwrap_or("unknown"),
        "hasHsPacket": metadata.get("hasHsPacket").and_then(Value::as_bool).unwrap_or(false),
        "planStepCount": preview_plan_step_count(&output.payload),
        "blocked": matches!(output.payload, MultiStrategyRuntimePayload::Blocked),
        "allowWrites": false,
        "metadataSafe": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
    })
}

fn shadow_audit_summary(
    output: &MultiStrategyRuntimeOutput,
    warnings: &[String],
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
) -> Value {
    let mut write_control = preview_write_control(&output.payload);
    if let Some(map) = write_control.as_object_mut() {
        map.insert("allowWrites".into(), Value::Bool(false));
    }
    let metadata = shadow_metadata_safe_summary(output, descriptor_kind, user_input_checksum);
    json!({
        "shadowRunRuntime": "controlled_chat_migration",
        "strategyKind": metadata["strategyKind"],
        "payloadKind": metadata["payloadKind"],
        "governanceDecisionKind": metadata["governanceDecisionKind"],
        "taskKind": metadata["taskKind"],
        "reasonCode": metadata["reasonCode"],
        "riskLevel": metadata["riskLevel"],
        "hasHsPacket": metadata["hasHsPacket"],
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "planStepCount": metadata["planStepCount"],
        "planStepStatuses": preview_plan_step_statuses(&output.payload),
        "proposalIdCount": preview_proposal_ids(&output.payload).len(),
        "blocked": metadata["blocked"],
        "warnings": warnings,
        "metadataSafe": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "writeControl": write_control,
    })
}

fn shadow_prompt_for_descriptor(descriptor_kind: &str) -> &'static str {
    match descriptor_kind {
        "planning_readiness_probe" => "Plan a controlled migration comparison.",
        "sensitive_local_only_probe" => "Discuss a sensitive local-only readiness check.",
        "default_readiness_probe" => {
            "Compare default chat contract with controlled runtime readiness."
        }
        _ => "Compare default chat contract with controlled runtime readiness.",
    }
}

fn cutover_candidate_prompt_for_descriptor(descriptor_kind: &str) -> &'static str {
    match descriptor_kind {
        "concise_response_probe" => "Provide a concise default Chat compatible response.",
        "default_contract_probe" => {
            "Provide a concise default Chat compatible response for a controlled runtime probe."
        }
        _ => "Provide a concise default Chat compatible response for a controlled runtime probe.",
    }
}

fn cutover_candidate_user_output(output: &MultiStrategyRuntimeOutput) -> Option<String> {
    match &output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output)
            if !runtime_output.user_output.trim().is_empty() =>
        {
            Some(runtime_output.user_output.clone())
        }
        MultiStrategyRuntimePayload::ReAct(_)
        | MultiStrategyRuntimePayload::PlanExecute(_)
        | MultiStrategyRuntimePayload::Blocked => None,
    }
}

fn cutover_candidate_contract_shape(output: &MultiStrategyRuntimeOutput) -> &'static str {
    match &output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output)
            if !runtime_output.user_output.trim().is_empty()
                && runtime_output.proposal_ids.is_empty() =>
        {
            "send_message_compatible"
        }
        MultiStrategyRuntimePayload::Blocked => "blocked",
        MultiStrategyRuntimePayload::ReAct(_) | MultiStrategyRuntimePayload::PlanExecute(_) => {
            "failed"
        }
    }
}

fn cutover_candidate_contract_blockers(
    output: &MultiStrategyRuntimeOutput,
    contract_shape: &str,
) -> Vec<String> {
    let mut blocking_reasons = Vec::new();
    match &output.payload {
        MultiStrategyRuntimePayload::Blocked => {
            push_unique_string(&mut blocking_reasons, "candidate_runtime_blocked".into());
        }
        MultiStrategyRuntimePayload::PlanExecute(_) => {
            push_unique_string(
                &mut blocking_reasons,
                "candidate_runtime_returned_non_chat_payload".into(),
            );
        }
        MultiStrategyRuntimePayload::ReAct(runtime_output) => {
            if runtime_output.user_output.trim().is_empty() {
                push_unique_string(
                    &mut blocking_reasons,
                    "candidate_user_output_missing".into(),
                );
            }
            if !runtime_output.proposal_ids.is_empty() {
                push_unique_string(
                    &mut blocking_reasons,
                    "candidate_proposal_ids_present".into(),
                );
            }
        }
    }

    let write_control = preview_write_control(&output.payload);
    let declared_write_step_count = write_control
        .get("declaredWriteStepCount")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let proposal_required_step_count = write_control
        .get("proposalRequiredStepCount")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if declared_write_step_count > 0 || proposal_required_step_count > 0 {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_write_or_proposal_step_present".into(),
        );
    }
    if contract_shape == "failed" && blocking_reasons.is_empty() {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_contract_shape_failed".into(),
        );
    }

    blocking_reasons
}

fn cutover_candidate_output_label(output: &MultiStrategyRuntimeOutput) -> String {
    format!(
        "Cutover candidate: {} / {}",
        preview_strategy_kind(output.selection.kind),
        preview_payload_kind(&output.payload)
    )
}

fn metadata_safe_shadow_error(error: &str) -> String {
    format!(
        "controlled migration shadow run failed: {}",
        shadow_error_code(error)
    )
}

fn shadow_error_code(error: &str) -> &'static str {
    if error.contains("invalid_shadow_descriptor") {
        "invalid_shadow_descriptor"
    } else if error.contains("failed to load LifeModel") {
        "lifemodel_load_failed"
    } else if error.contains("HS runtime packet build failed") {
        "hs_packet_build_failed"
    } else if error.contains("multi-strategy preview runtime failed") {
        "multi_strategy_runtime_failed"
    } else {
        "shadow_runtime_failed"
    }
}

fn normalize_metadata_key(key: &str) -> String {
    key.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn looks_like_email_for_metadata(text: &str) -> bool {
    text.split_whitespace().any(|token| {
        let token = token.trim_matches(|ch: char| {
            matches!(ch, ',' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']')
        });
        let Some((local, domain)) = token.split_once('@') else {
            return false;
        };
        !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    })
}

fn push_unique_string(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

#[tauri::command]
pub async fn run_multi_strategy_agent_preview(
    input: MultiStrategyAgentPreviewInput,
    state: State<'_, Arc<AppState>>,
) -> Result<MultiStrategyAgentPreviewOutput, String> {
    run_multi_strategy_agent_preview_with_state(input, &state.inner().clone()).await
}

async fn find_preview_run_for_gate(
    input: RuntimeMigrationGateCheckInput,
    state: &Arc<AppState>,
) -> Result<Option<AgentRun>, String> {
    let Some(store_arc) = state.agent_run_store.as_ref() else {
        return Ok(None);
    };
    let store = store_arc.lock().await;

    if let Some(preview_run_id) = input
        .preview_run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return store
            .get_run(preview_run_id)
            .map_err(|e| format!("failed to read preview AgentRun for migration gate: {e}"));
    }

    let runs = if let Some(session_id) = input
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        store
            .list_runs_for_session(session_id, 50)
            .map_err(|e| format!("failed to list preview AgentRuns for migration gate: {e}"))?
    } else {
        store
            .list_runs(50, 0)
            .map_err(|e| format!("failed to list preview AgentRuns for migration gate: {e}"))?
    };

    Ok(runs
        .into_iter()
        .find(|run| run.reasoning_strategy.as_deref() == Some("multi_strategy_preview")))
}

async fn find_preview_runs_for_pilot_eligibility(
    input: &ControlledChatPilotEligibilityCheckInput,
    required_clean_runs: usize,
    state: &Arc<AppState>,
) -> Result<Vec<AgentRun>, String> {
    let Some(store_arc) = state.agent_run_store.as_ref() else {
        return Ok(Vec::new());
    };
    let store = store_arc.lock().await;
    let read_limit = 50_i64.max(required_clean_runs as i64);

    let runs = if let Some(session_id) = input
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        store
            .list_runs_for_session(session_id, read_limit)
            .map_err(|e| format!("failed to list preview AgentRuns for pilot eligibility: {e}"))?
    } else {
        store
            .list_runs(read_limit, 0)
            .map_err(|e| format!("failed to list preview AgentRuns for pilot eligibility: {e}"))?
    };

    Ok(runs
        .into_iter()
        .filter(|run| run.reasoning_strategy.as_deref() == Some("multi_strategy_preview"))
        .take(required_clean_runs)
        .collect())
}

pub(crate) async fn run_multi_strategy_agent_preview_with_state(
    input: MultiStrategyAgentPreviewInput,
    state: &Arc<AppState>,
) -> Result<MultiStrategyAgentPreviewOutput, String> {
    let mut preview_run = new_preview_agent_run(&input.session_id);
    let preview_run_id = preview_run.id.clone();
    create_preview_run(state, &preview_run).await?;

    let result = execute_multi_strategy_agent_preview(input, state, &preview_run_id).await;

    let result = match result {
        Ok(result) => result,
        Err(error) => {
            fail_preview_run(state, &mut preview_run, &error).await;
            return Err(metadata_safe_preview_error(&error));
        }
    };

    let final_warnings = preview_output_warnings(&result.output, &result.warnings);
    let audit = preview_audit_summary(&result.output, &final_warnings);
    let mut output = map_preview_output(result.output, result.warnings);
    output.run_id = Some(preview_run_id);

    complete_preview_run(
        state,
        &mut preview_run,
        PreviewRunCompletion {
            audit,
            warnings: final_warnings,
            proposal_ids: output.proposal_ids.clone(),
            context_summary: result.context_summary,
            hs_selection_audit: result.hs_selection_audit,
            behavior_checks: result.behavior_checks,
        },
    )
    .await?;

    Ok(output)
}

struct PreviewExecutionResult {
    output: MultiStrategyRuntimeOutput,
    warnings: Vec<String>,
    context_summary: ContextSummary,
    hs_selection_audit: Option<HSSelectionAudit>,
    behavior_checks: Vec<HSBehaviorCheckSummary>,
}

struct PreviewRunCompletion {
    audit: Value,
    warnings: Vec<String>,
    proposal_ids: Vec<String>,
    context_summary: ContextSummary,
    hs_selection_audit: Option<HSSelectionAudit>,
    behavior_checks: Vec<HSBehaviorCheckSummary>,
}

struct ShadowRunCompletion {
    audit: Value,
    warnings: Vec<String>,
    context_summary: ContextSummary,
    hs_selection_audit: Option<HSSelectionAudit>,
    behavior_checks: Vec<HSBehaviorCheckSummary>,
}

struct CutoverCandidateRunCompletion {
    audit: Value,
    warnings: Vec<String>,
    context_summary: ContextSummary,
    hs_selection_audit: Option<HSSelectionAudit>,
    behavior_checks: Vec<HSBehaviorCheckSummary>,
}

async fn execute_multi_strategy_agent_preview(
    input: MultiStrategyAgentPreviewInput,
    state: &Arc<AppState>,
    preview_run_id: &str,
) -> Result<PreviewExecutionResult, String> {
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager
            .load()
            .map_err(|e| format!("failed to load LifeModel for preview runtime: {e}"))?
    };
    let scheduler = state.scheduler.lock().await.clone();
    let config = state.config.lock().await.clone();
    let layer = parse_preview_layer(input.layer.as_deref())?;
    let tools_prompt = if input.tools_prompt.trim().is_empty() {
        let registry = state.mcp_registry.lock().await;
        registry.tools_prompt()
    } else {
        input.tools_prompt.clone()
    };
    let (execution_budget, mut adapter_warnings) =
        preview_execution_budget(input.execution_budget.as_ref());
    let life_model_empty = life_model.is_effectively_empty();
    let used_tools_prompt = !tools_prompt.trim().is_empty();

    let task = AgentTask {
        kind: AgentTaskKind::Conversation,
        session_id: input.session_id.clone(),
        user_text: input.user_text.clone(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: input.user_text.clone(),
        }],
        layer,
    };
    let hs_packet = crate::build_chat_runtime_hs_packet(
        state,
        &task,
        &life_model,
        &tools_prompt,
        Some(preview_run_id.to_string()),
    )
    .await?;
    let hs_selection_audit = hs_packet.as_ref().map(|packet| packet.audit.clone());
    let behavior_checks = hs_packet
        .as_ref()
        .map(behavior_checks_for_packet)
        .unwrap_or_default();
    let runtime_input = RuntimeInput::from_agent_task(
        task,
        life_model.clone(),
        None,
        tools_prompt,
        hs_packet,
        execution_budget,
    );
    let runtime = AgentRuntime::new(life_model, scheduler, &config);
    let multi_strategy_runtime = MultiStrategyRuntime::new(runtime);
    let output = multi_strategy_runtime
        .execute(MultiStrategyRuntimeInput {
            runtime_input,
            allow_planning: input.allow_planning,
            local_model_available: input.local_model_available,
        })
        .await
        .map_err(|e| format!("multi-strategy preview runtime failed: {e}"))?;

    adapter_warnings.extend(output.warnings.clone());
    Ok(PreviewExecutionResult {
        output,
        warnings: adapter_warnings,
        context_summary: ContextSummary {
            life_model_empty,
            included_life_model_sections: Vec::new(),
            memory_hit_count: 0,
            memory_sources: Vec::new(),
            used_tools_prompt,
            redaction_applied: true,
            redaction_level: RedactionLevel::Strict,
        },
        hs_selection_audit,
        behavior_checks,
    })
}

fn preview_execution_budget(
    input: Option<&MultiStrategyAgentPreviewExecutionBudgetInput>,
) -> (AgentExecutionBudget, Vec<String>) {
    let mut budget = AgentExecutionBudget::default();
    let mut warnings = Vec::new();

    if let Some(input) = input {
        if let Some(max_steps) = input.max_steps {
            budget.max_steps = max_steps;
        }
        if let Some(max_tool_calls) = input.max_tool_calls {
            budget.max_tool_calls = max_tool_calls;
        }
        if let Some(timeout_seconds) = input.timeout_seconds {
            budget.timeout_seconds = timeout_seconds;
        }
        if let Some(allow_cloud) = input.allow_cloud {
            budget.allow_cloud = allow_cloud;
        }
        if input.allow_writes == Some(true) {
            warnings.push("preview runtime forces allowWrites=false".into());
        }
    }

    budget.allow_writes = false;
    (budget, warnings)
}

fn new_preview_agent_run(session_id: &str) -> AgentRun {
    let mut run = AgentRun::new_chat_run(session_id, "");
    run.user_input = None;
    run.reasoning_strategy = Some("multi_strategy_preview".into());
    run.output_preview = Some("Multi-strategy preview started".into());
    run.context_summary = Some(ContextSummary {
        life_model_empty: false,
        included_life_model_sections: Vec::new(),
        memory_hit_count: 0,
        memory_sources: Vec::new(),
        used_tools_prompt: false,
        redaction_applied: true,
        redaction_level: RedactionLevel::Strict,
    });
    run
}

fn new_shadow_agent_run(
    session_id: &str,
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
) -> AgentRun {
    let mut run = AgentRun::new_chat_run(session_id, "");
    run.user_input = None;
    run.reasoning_strategy = Some("controlled_migration_shadow_run".into());
    run.output_preview = Some("Controlled migration shadow run started".into());
    run.context_summary = Some(ContextSummary {
        life_model_empty: false,
        included_life_model_sections: Vec::new(),
        memory_hit_count: 0,
        memory_sources: Vec::new(),
        used_tools_prompt: false,
        redaction_applied: true,
        redaction_level: RedactionLevel::Strict,
    });
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(json!({
            "shadowRunRuntime": "controlled_chat_migration",
            "descriptorKind": descriptor_kind,
            "userInputChecksumPresent": user_input_checksum.is_some(),
            "status": "started",
            "allowWrites": false,
            "metadataSafe": true,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
        })),
        output: Some("controlled_migration_shadow_run".into()),
        ..ReasoningTrace::default()
    });
    run
}

fn new_cutover_candidate_agent_run(
    session_id: &str,
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
) -> AgentRun {
    let mut run = AgentRun::new_chat_run(session_id, "");
    run.user_input = None;
    run.reasoning_strategy = Some("controlled_chat_cutover_candidate".into());
    run.output_preview = Some("Controlled chat cutover candidate started".into());
    run.context_summary = Some(ContextSummary {
        life_model_empty: false,
        included_life_model_sections: Vec::new(),
        memory_hit_count: 0,
        memory_sources: Vec::new(),
        used_tools_prompt: false,
        redaction_applied: true,
        redaction_level: RedactionLevel::Strict,
    });
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(json!({
            "candidateAdapter": "controlled_chat_cutover_candidate",
            "descriptorKind": descriptor_kind,
            "userInputChecksumPresent": user_input_checksum.is_some(),
            "status": "started",
            "allowWrites": false,
            "maxToolCalls": 0,
            "metadataSafe": true,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
        })),
        output: Some("controlled_chat_cutover_candidate".into()),
        ..ReasoningTrace::default()
    });
    run
}

async fn create_preview_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for preview runtime".to_string())?;
    let store = store_arc.lock().await;
    store
        .create_run(run)
        .map_err(|e| format!("failed to create preview AgentRun: {e}"))
}

async fn create_shadow_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for shadow runtime".to_string())?;
    let store = store_arc.lock().await;
    store
        .create_run(run)
        .map_err(|e| format!("failed to create shadow AgentRun: {e}"))
}

async fn create_cutover_candidate_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for cutover candidate runtime".to_string())?;
    let store = store_arc.lock().await;
    store
        .create_run(run)
        .map_err(|e| format!("failed to create cutover candidate AgentRun: {e}"))
}

async fn complete_preview_run(
    state: &Arc<AppState>,
    run: &mut AgentRun,
    completion: PreviewRunCompletion,
) -> Result<(), String> {
    run.status = AgentRunStatus::Completed;
    run.finished_at = Some(chrono::Utc::now());
    run.generated_proposals = completion.proposal_ids;
    run.warnings = completion.warnings;
    run.hs_selection_audit = completion.hs_selection_audit;
    run.behavior_checks = completion.behavior_checks;
    run.output_preview = Some(preview_output_label(&completion.audit));
    run.step_count = completion
        .audit
        .get("planStepCount")
        .and_then(|value| value.as_u64())
        .unwrap_or_default() as u32;
    run.tool_call_count = 0;
    run.context_summary = Some(completion.context_summary);
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(completion.audit),
        output: Some("multi_strategy_preview".into()),
        stable_steps: vec![
            "strategy_selection".into(),
            "governance_check".into(),
            "preview_payload".into(),
        ],
        ..ReasoningTrace::default()
    });

    update_preview_run(state, run).await
}

async fn complete_shadow_run(
    state: &Arc<AppState>,
    run: &mut AgentRun,
    completion: ShadowRunCompletion,
) -> Result<(), String> {
    run.status = AgentRunStatus::Completed;
    run.finished_at = Some(chrono::Utc::now());
    run.generated_proposals = Vec::new();
    run.warnings = completion.warnings;
    run.hs_selection_audit = completion.hs_selection_audit;
    run.behavior_checks = completion.behavior_checks;
    run.output_preview = Some(shadow_output_label(&completion.audit));
    run.step_count = completion
        .audit
        .get("planStepCount")
        .and_then(|value| value.as_u64())
        .unwrap_or_default() as u32;
    run.tool_call_count = 0;
    run.context_summary = Some(completion.context_summary);
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(completion.audit),
        output: Some("controlled_migration_shadow_run".into()),
        stable_steps: vec![
            "implementation_gate_check".into(),
            "shadow_strategy_selection".into(),
            "write_disabled_runtime_preview".into(),
            "metadata_safe_summary".into(),
        ],
        ..ReasoningTrace::default()
    });

    update_shadow_run(state, run).await
}

async fn complete_cutover_candidate_run(
    state: &Arc<AppState>,
    run: &mut AgentRun,
    completion: CutoverCandidateRunCompletion,
) -> Result<(), String> {
    run.status = AgentRunStatus::Completed;
    run.finished_at = Some(chrono::Utc::now());
    run.generated_proposals = Vec::new();
    run.warnings = completion.warnings;
    run.hs_selection_audit = completion.hs_selection_audit;
    run.behavior_checks = completion.behavior_checks;
    run.output_preview = Some(cutover_candidate_audit_output_label(&completion.audit));
    run.step_count = completion
        .audit
        .get("planStepCount")
        .and_then(|value| value.as_u64())
        .unwrap_or_default() as u32;
    run.tool_call_count = 0;
    run.context_summary = Some(completion.context_summary);
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(completion.audit),
        output: Some("controlled_chat_cutover_candidate".into()),
        stable_steps: vec![
            "cutover_readiness_check".into(),
            "candidate_strategy_selection".into(),
            "send_message_contract_shape_validation".into(),
            "metadata_safe_audit".into(),
        ],
        ..ReasoningTrace::default()
    });

    update_cutover_candidate_run(state, run).await
}

async fn fail_preview_run(state: &Arc<AppState>, run: &mut AgentRun, error: &str) {
    run.fail(AgentRunError {
        message: metadata_safe_preview_error(error),
        phase: "preview_runtime_failed".into(),
        recoverable: false,
    });
    run.user_input = None;
    run.reasoning_strategy = Some("multi_strategy_preview".into());
    let audit = json!({
        "previewRuntime": "multi_strategy",
        "status": "failed",
        "errorCode": preview_error_code(error),
        "metadataSafe": true,
    });
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(audit),
        output: Some("multi_strategy_preview_failed".into()),
        ..ReasoningTrace::default()
    });
    run.output_preview = Some("Multi-strategy preview failed".into());

    if let Err(e) = update_preview_run(state, run).await {
        log::warn!("[AgentRun] failed to update preview run after error: {}", e);
    }
}

async fn fail_shadow_run(state: &Arc<AppState>, run: &mut AgentRun, safe_error: &str) {
    run.fail(AgentRunError {
        message: safe_error.to_string(),
        phase: "controlled_migration_shadow_runtime_failed".into(),
        recoverable: false,
    });
    run.user_input = None;
    run.reasoning_strategy = Some("controlled_migration_shadow_run".into());
    let audit = json!({
        "shadowRunRuntime": "controlled_chat_migration",
        "status": "failed",
        "errorCode": shadow_error_code(safe_error),
        "allowWrites": false,
        "metadataSafe": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
    });
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(audit),
        output: Some("controlled_migration_shadow_run_failed".into()),
        ..ReasoningTrace::default()
    });
    run.output_preview = Some("Controlled migration shadow run failed".into());

    if let Err(e) = update_shadow_run(state, run).await {
        log::warn!("[AgentRun] failed to update shadow run after error: {}", e);
    }
}

async fn fail_cutover_candidate_run(state: &Arc<AppState>, run: &mut AgentRun, safe_error: &str) {
    run.fail(AgentRunError {
        message: safe_error.to_string(),
        phase: "controlled_chat_cutover_candidate_failed".into(),
        recoverable: false,
    });
    run.user_input = None;
    run.reasoning_strategy = Some("controlled_chat_cutover_candidate".into());
    let audit = json!({
        "candidateAdapter": "controlled_chat_cutover_candidate",
        "status": "failed",
        "errorCode": cutover_candidate_error_code(safe_error),
        "contractShape": "failed",
        "allowWrites": false,
        "maxToolCalls": 0,
        "metadataSafe": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
    });
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(audit),
        output: Some("controlled_chat_cutover_candidate_failed".into()),
        ..ReasoningTrace::default()
    });
    run.output_preview = Some("Controlled chat cutover candidate failed".into());

    if let Err(e) = update_cutover_candidate_run(state, run).await {
        log::warn!(
            "[AgentRun] failed to update cutover candidate run after error: {}",
            e
        );
    }
}

async fn update_preview_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for preview runtime".to_string())?;
    let store = store_arc.lock().await;
    store
        .update_run(run)
        .map_err(|e| format!("failed to update preview AgentRun: {e}"))
}

async fn update_shadow_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for shadow runtime".to_string())?;
    let store = store_arc.lock().await;
    store
        .update_run(run)
        .map_err(|e| format!("failed to update shadow AgentRun: {e}"))
}

async fn update_cutover_candidate_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for cutover candidate runtime".to_string())?;
    let store = store_arc.lock().await;
    store
        .update_run(run)
        .map_err(|e| format!("failed to update cutover candidate AgentRun: {e}"))
}

fn metadata_safe_preview_error(error: &str) -> String {
    format!(
        "multi-strategy preview runtime failed: {}",
        preview_error_code(error)
    )
}

fn preview_error_code(error: &str) -> &'static str {
    if error.contains("unsupported preview runtime layer") {
        "invalid_preview_layer"
    } else if error.contains("failed to load LifeModel") {
        "lifemodel_load_failed"
    } else if error.contains("HS runtime packet build failed") {
        "hs_packet_build_failed"
    } else if error.contains("multi-strategy preview runtime failed") {
        "multi_strategy_runtime_failed"
    } else {
        "preview_runtime_failed"
    }
}

fn metadata_safe_cutover_candidate_error(error: &str) -> String {
    format!(
        "controlled chat cutover candidate failed: {}",
        cutover_candidate_error_code(error)
    )
}

fn cutover_candidate_error_code(error: &str) -> &'static str {
    if error.contains("unsupported preview runtime layer") {
        "invalid_candidate_layer"
    } else if error.contains("failed to load LifeModel") {
        "lifemodel_load_failed"
    } else if error.contains("HS runtime packet build failed") {
        "hs_packet_build_failed"
    } else if error.contains("multi-strategy preview runtime failed") {
        "multi_strategy_runtime_failed"
    } else {
        "candidate_runtime_failed"
    }
}

fn shadow_output_label(audit: &Value) -> String {
    let strategy = audit
        .get("strategyKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let payload = audit
        .get("payloadKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    format!("Controlled migration shadow run: {strategy} / {payload}")
}

fn cutover_candidate_audit_output_label(audit: &Value) -> String {
    let strategy = audit
        .get("strategyKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let payload = audit
        .get("payloadKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    format!("Cutover candidate: {strategy} / {payload}")
}

fn preview_output_label(audit: &Value) -> String {
    let strategy = audit
        .get("strategyKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let governance = audit
        .get("governanceDecisionKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    if audit
        .get("blocked")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        format!("Multi-strategy preview blocked: {strategy} / {governance}")
    } else {
        format!("Multi-strategy preview: {strategy} / {governance}")
    }
}

fn map_preview_output(
    output: openlife_core::agent::MultiStrategyRuntimeOutput,
    warnings: Vec<String>,
) -> MultiStrategyAgentPreviewOutput {
    let governance_decision_kind = output
        .selection
        .governance_decision
        .as_ref()
        .map(|decision| preview_governance_decision_kind(decision.kind).to_string());
    let strategy_kind = preview_strategy_kind(output.selection.kind).to_string();
    let metadata_safe_summary = output.selection.metadata_safe_summary.clone();

    match output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output) => MultiStrategyAgentPreviewOutput {
            run_id: runtime_output.run_id,
            strategy_kind,
            payload_kind: "react".into(),
            user_output: Some(runtime_output.user_output),
            plan: None,
            proposal_ids: runtime_output.proposal_ids,
            warnings: merge_warnings(warnings, runtime_output.warnings),
            metadata_safe_summary,
            governance_decision_kind,
        },
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => MultiStrategyAgentPreviewOutput {
            run_id: None,
            strategy_kind,
            payload_kind: "planExecute".into(),
            user_output: None,
            plan: Some(metadata_safe_plan(&plan_output)),
            proposal_ids: Vec::new(),
            warnings,
            metadata_safe_summary,
            governance_decision_kind,
        },
        MultiStrategyRuntimePayload::Blocked => MultiStrategyAgentPreviewOutput {
            run_id: None,
            strategy_kind,
            payload_kind: "blocked".into(),
            user_output: None,
            plan: None,
            proposal_ids: Vec::new(),
            warnings,
            metadata_safe_summary,
            governance_decision_kind,
        },
    }
}

fn metadata_safe_plan(plan_output: &PlanExecutionOutput) -> Value {
    json!({
        "objective": plan_output.plan.objective,
        "steps": plan_output.plan.steps.iter().map(|step| {
            json!({
                "id": step.id,
                "title": step.title,
                "intent": step.intent,
                "toolName": step.tool_name,
                "actionKind": step.action_kind,
                "riskLevel": step.risk_level,
                "declaredWrite": step.declared_write,
            })
        }).collect::<Vec<_>>(),
        "traces": plan_output.traces.iter().map(|trace| {
            let policy_reason_code = trace
                .decision
                .metadata_safe_summary
                .get("policyReasonCode")
                .and_then(|value| value.as_str());
            json!({
                "stepId": trace.step_id,
                "status": trace.status,
                "decisionKind": trace.decision.kind,
                "riskLevel": trace.decision.risk_level,
                "policyReasonCode": policy_reason_code,
            })
        }).collect::<Vec<_>>(),
        "warnings": plan_output.warnings,
    })
}

fn preview_output_warnings(
    output: &MultiStrategyRuntimeOutput,
    adapter_warnings: &[String],
) -> Vec<String> {
    let mut warnings = adapter_warnings.to_vec();
    if let MultiStrategyRuntimePayload::ReAct(runtime_output) = &output.payload {
        warnings.extend(runtime_output.warnings.clone());
    }
    warnings
}

fn preview_audit_summary(output: &MultiStrategyRuntimeOutput, warnings: &[String]) -> Value {
    let strategy_kind = preview_strategy_kind(output.selection.kind);
    let payload_kind = preview_payload_kind(&output.payload);
    let metadata = &output.selection.metadata_safe_summary;
    let task_kind = metadata
        .get("taskKind")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let reason_code = metadata
        .get("reasonCode")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let risk_level = metadata
        .get("riskLevel")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let has_hs_packet = metadata
        .get("hasHsPacket")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let governance_policy_kind = output
        .selection
        .governance_decision
        .as_ref()
        .map(|decision| preview_governance_policy_kind(decision.kind))
        .unwrap_or("unknown");
    let governance_decision_kind = output
        .selection
        .governance_decision
        .as_ref()
        .map(|decision| preview_governance_decision_kind(decision.kind))
        .unwrap_or("unknown");
    let proposal_ids = preview_proposal_ids(&output.payload);
    let inner_run_id = match &output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output) => runtime_output.run_id.clone(),
        MultiStrategyRuntimePayload::PlanExecute(_) | MultiStrategyRuntimePayload::Blocked => None,
    };
    let plan_step_count = preview_plan_step_count(&output.payload);
    let plan_step_statuses = preview_plan_step_statuses(&output.payload);
    let write_control = preview_write_control(&output.payload);
    let blocked = matches!(output.payload, MultiStrategyRuntimePayload::Blocked);

    json!({
        "previewRuntime": "multi_strategy",
        "taskKind": task_kind,
        "strategyKind": strategy_kind,
        "payloadKind": payload_kind,
        "governanceDecisionKind": governance_decision_kind,
        "governancePolicyKind": governance_policy_kind,
        "reasonCode": reason_code,
        "riskLevel": risk_level,
        "hasHsPacket": has_hs_packet,
        "warnings": warnings,
        "proposalIds": proposal_ids,
        "planStepCount": plan_step_count,
        "planStepStatuses": plan_step_statuses,
        "blocked": blocked,
        "metadataSafe": true,
        "innerRunId": inner_run_id,
        "writeControl": write_control,
    })
}

fn preview_payload_kind(payload: &MultiStrategyRuntimePayload) -> &'static str {
    match payload {
        MultiStrategyRuntimePayload::ReAct(_) => "react",
        MultiStrategyRuntimePayload::PlanExecute(_) => "planExecute",
        MultiStrategyRuntimePayload::Blocked => "blocked",
    }
}

fn preview_proposal_ids(payload: &MultiStrategyRuntimePayload) -> Vec<String> {
    match payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output) => runtime_output.proposal_ids.clone(),
        MultiStrategyRuntimePayload::PlanExecute(_) | MultiStrategyRuntimePayload::Blocked => {
            Vec::new()
        }
    }
}

fn preview_plan_step_count(payload: &MultiStrategyRuntimePayload) -> usize {
    match payload {
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => plan_output.plan.steps.len(),
        MultiStrategyRuntimePayload::ReAct(_) | MultiStrategyRuntimePayload::Blocked => 0,
    }
}

fn preview_plan_step_statuses(payload: &MultiStrategyRuntimePayload) -> Vec<String> {
    match payload {
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => plan_output
            .traces
            .iter()
            .map(|trace| preview_plan_step_status(trace.status))
            .collect(),
        MultiStrategyRuntimePayload::ReAct(_) | MultiStrategyRuntimePayload::Blocked => Vec::new(),
    }
}

fn preview_write_control(payload: &MultiStrategyRuntimePayload) -> Value {
    match payload {
        MultiStrategyRuntimePayload::PlanExecute(plan_output) => {
            let declared_write_step_count = plan_output
                .plan
                .steps
                .iter()
                .filter(|step| step.declared_write)
                .count();
            let proposal_required_step_count = plan_output
                .traces
                .iter()
                .filter(|trace| trace.status == PlanStepStatus::RequiresProposal)
                .count();
            let blocked_step_count = plan_output
                .traces
                .iter()
                .filter(|trace| trace.status == PlanStepStatus::Blocked)
                .count();
            json!({
                "declaredWriteStepCount": declared_write_step_count,
                "proposalRequiredStepCount": proposal_required_step_count,
                "blockedStepCount": blocked_step_count,
            })
        }
        MultiStrategyRuntimePayload::ReAct(_) | MultiStrategyRuntimePayload::Blocked => json!({
            "declaredWriteStepCount": 0,
            "proposalRequiredStepCount": 0,
            "blockedStepCount": 0,
        }),
    }
}

fn preview_plan_step_status(status: PlanStepStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

fn merge_warnings(mut left: Vec<String>, right: Vec<String>) -> Vec<String> {
    left.extend(right);
    left
}

fn parse_preview_layer(layer: Option<&str>) -> Result<Layer, String> {
    match layer.map(str::trim).filter(|layer| !layer.is_empty()) {
        None => Ok(Layer::L2),
        Some("L1" | "l1" | "1") => Ok(Layer::L1),
        Some("L2" | "l2" | "2") => Ok(Layer::L2),
        Some("L3" | "l3" | "3") => Ok(Layer::L3),
        Some(other) => Err(format!("unsupported preview runtime layer: {other}")),
    }
}

fn preview_strategy_kind(kind: RuntimeStrategyKind) -> &'static str {
    match kind {
        RuntimeStrategyKind::ReAct => "react",
        RuntimeStrategyKind::PlanExecute => "planExecute",
    }
}

fn preview_governance_decision_kind(kind: GovernanceDecisionKind) -> &'static str {
    match kind {
        GovernanceDecisionKind::Allow => "allow",
        GovernanceDecisionKind::Block => "block",
        GovernanceDecisionKind::RequireProposal
        | GovernanceDecisionKind::RequireConfirmation
        | GovernanceDecisionKind::RequireLocalOnly => "warn",
    }
}

fn preview_governance_policy_kind(kind: GovernanceDecisionKind) -> &'static str {
    match kind {
        GovernanceDecisionKind::Allow => "allow",
        GovernanceDecisionKind::RequireProposal => "require_proposal",
        GovernanceDecisionKind::RequireConfirmation => "require_confirmation",
        GovernanceDecisionKind::RequireLocalOnly => "require_local_only",
        GovernanceDecisionKind::Block => "block",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{
        AgentRun, AgentRunStatus, EvidenceDraft, EvidencePrivacyLevel, EvidenceQuery, EvidenceType,
        ProposalStore, RiskLevel,
    };
    use openlife_core::life_model::LifeModel;

    async fn preview_state() -> std::sync::Arc<crate::AppState> {
        let state = crate::test_utils::test_app_state();
        {
            let manager = state.life_model_manager.lock().await;
            manager.save(&LifeModel::default()).unwrap();
        }
        state
    }

    fn base_input(user_text: &str) -> MultiStrategyAgentPreviewInput {
        MultiStrategyAgentPreviewInput {
            session_id: "session-preview".into(),
            user_text: user_text.into(),
            tools_prompt: "Available tools: memory.search".into(),
            allow_planning: true,
            local_model_available: true,
            layer: None,
            execution_budget: None,
        }
    }

    async fn stored_preview_run(state: &Arc<crate::AppState>, run_id: &str) -> AgentRun {
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        store
            .get_run(run_id)
            .unwrap()
            .unwrap_or_else(|| panic!("missing preview run {run_id}"))
    }

    fn preview_audit(run: &AgentRun) -> &Value {
        run.reasoning_trace
            .as_ref()
            .and_then(|trace| trace.strategy_result.as_ref())
            .expect("preview run should persist metadata-safe audit")
    }

    fn healthy_gate_preview_run(session_id: &str) -> AgentRun {
        let mut run = AgentRun::new_chat_run(session_id, "raw text should be cleared");
        run.status = AgentRunStatus::Completed;
        run.user_input = None;
        run.reasoning_strategy = Some("multi_strategy_preview".into());
        run.output_preview = Some("Multi-strategy preview: react / allow".into());
        run.reasoning_trace = Some(ReasoningTrace {
            strategy_result: Some(json!({
                "previewRuntime": "multi_strategy",
                "strategyKind": "react",
                "payloadKind": "react",
                "governanceDecisionKind": "allow",
                "metadataSafe": true,
                "innerRunId": "inner-react-run",
                "writeControl": {
                    "declaredWriteStepCount": 0,
                    "proposalRequiredStepCount": 0,
                    "blockedStepCount": 0
                }
            })),
            output: Some("multi_strategy_preview".into()),
            ..ReasoningTrace::default()
        });
        run.finished_at = Some(chrono::Utc::now());
        run
    }

    fn healthy_gate_preview_run_with_id(session_id: &str, id: &str, age_seconds: i64) -> AgentRun {
        let mut run = healthy_gate_preview_run(session_id);
        run.id = id.to_string();
        run.started_at = chrono::Utc::now() - chrono::Duration::seconds(age_seconds);
        run.finished_at = Some(run.started_at + chrono::Duration::seconds(1));
        run
    }

    #[tokio::test]
    async fn runtime_migration_gate_command_reads_existing_preview_run_only() {
        let state = preview_state().await;
        let run = healthy_gate_preview_run("session-gate-command");
        let run_id = run.id.clone();
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.create_run(&run).unwrap();
        }
        let before_run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let before_pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();

        let report = check_runtime_migration_gate_with_state(
            RuntimeMigrationGateCheckInput {
                preview_run_id: Some(run_id),
                session_id: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.default_chat_unchanged);
        assert!(report.preview_path_healthy);
        assert!(report.metadata_safe_trace_ready);
        assert!(report.fallback_available);
        assert!(report.no_external_writes);
        assert!(report.proposal_first_preserved);
        assert!(report.blocking_reasons.is_empty());

        let after_run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let after_pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        assert_eq!(before_run_count, after_run_count);
        assert_eq!(
            before_pending_proposals.len(),
            after_pending_proposals.len()
        );
    }

    #[tokio::test]
    async fn controlled_chat_pilot_eligibility_command_reads_existing_preview_runs_only() {
        let state = preview_state().await;
        let runs = vec![
            healthy_gate_preview_run_with_id("session-pilot", "run-preview-clean-3", 0),
            healthy_gate_preview_run_with_id("session-pilot", "run-preview-clean-2", 10),
            healthy_gate_preview_run_with_id("session-pilot", "run-preview-clean-1", 20),
        ];
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            for run in &runs {
                store.create_run(run).unwrap();
            }
        }
        let before_run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let before_pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();

        let report = check_controlled_chat_pilot_eligibility_with_state(
            ControlledChatPilotEligibilityCheckInput::default(),
            &state,
        )
        .await
        .unwrap();

        assert!(report.eligible);
        assert_eq!(report.required_clean_runs, 3);
        assert_eq!(report.clean_run_count, 3);
        assert_eq!(
            report.checked_run_ids,
            vec![
                "run-preview-clean-3",
                "run-preview-clean-2",
                "run-preview-clean-1"
            ]
        );
        assert!(report.blocking_reasons.is_empty());

        let after_run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let after_pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        let stored_runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs_for_session("session-pilot", 10).unwrap()
        };
        assert_eq!(before_run_count, after_run_count);
        assert_eq!(
            before_pending_proposals.len(),
            after_pending_proposals.len()
        );
        assert!(stored_runs.iter().all(|run| run.actions.is_empty()));
        assert!(stored_runs.iter().all(|run| run.observations.is_empty()));
    }

    #[tokio::test]
    async fn controlled_chat_pilot_eligibility_command_blocks_without_enough_preview_evidence() {
        let state = preview_state().await;
        let runs = vec![
            healthy_gate_preview_run_with_id("session-pilot-short", "run-preview-clean-2", 0),
            healthy_gate_preview_run_with_id("session-pilot-short", "run-preview-clean-1", 10),
        ];
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            for run in &runs {
                store.create_run(run).unwrap();
            }
        }

        let report = check_controlled_chat_pilot_eligibility_with_state(
            ControlledChatPilotEligibilityCheckInput {
                required_clean_runs: Some(3),
                session_id: Some("session-pilot-short".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.eligible);
        assert_eq!(report.required_clean_runs, 3);
        assert_eq!(report.clean_run_count, 2);
        assert_eq!(
            report.checked_run_ids,
            vec!["run-preview-clean-2", "run-preview-clean-1"]
        );
        assert!(report
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("insufficient_preview_evidence")));
    }

    #[tokio::test]
    async fn controlled_chat_pilot_eligibility_command_blocks_when_recent_gate_blocks() {
        let state = preview_state().await;
        let mut blocked_run =
            healthy_gate_preview_run_with_id("session-pilot-blocked", "run-preview-blocked-2", 10);
        blocked_run.tool_call_count = 1;
        let runs = vec![
            healthy_gate_preview_run_with_id("session-pilot-blocked", "run-preview-clean-3", 0),
            blocked_run,
            healthy_gate_preview_run_with_id("session-pilot-blocked", "run-preview-clean-1", 20),
        ];
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            for run in &runs {
                store.create_run(run).unwrap();
            }
        }

        let report = check_controlled_chat_pilot_eligibility_with_state(
            ControlledChatPilotEligibilityCheckInput {
                required_clean_runs: Some(3),
                session_id: Some("session-pilot-blocked".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.eligible);
        assert_eq!(report.required_clean_runs, 3);
        assert_eq!(report.clean_run_count, 2);
        assert_eq!(
            report.checked_run_ids,
            vec![
                "run-preview-clean-3",
                "run-preview-blocked-2",
                "run-preview-clean-1"
            ]
        );
        assert!(report
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("run-preview-blocked-2:external_write_risk_detected")));
    }

    #[tokio::test]
    async fn promotion_evidence_command_records_metadata_safe_idempotent_evidence() {
        let state = preview_state().await;
        let raw_pilot_response = "Pilot-only answer with private@example.com";
        let input = ControlledPilotPromotionEvidenceInput {
            pilot_run_id: "run-controlled-pilot-1".into(),
            source_session_id: "session-1".into(),
            target_session_id: "session-1".into(),
            strategy_kind: "react".into(),
            payload_kind: "react".into(),
            governance_decision_kind: Some("allow".into()),
            promoted_message_length: raw_pilot_response.len(),
            promoted_message_hash: "checksum:test-safe-digest".into(),
            promoted_at: Some("2026-05-30T01:02:03Z".into()),
        };

        let first = record_controlled_pilot_promotion_evidence_with_state(input.clone(), &state)
            .await
            .unwrap();
        let second = record_controlled_pilot_promotion_evidence_with_state(input, &state)
            .await
            .unwrap();

        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.evidence_id, second.evidence_id);

        let evidence = {
            let store = state.evidence_store.lock().await;
            store
                .query(EvidenceQuery {
                    affected_path: Some("runtime.controlled_pilot.promotion".into()),
                    evidence_type: Some(EvidenceType::RuntimeBehavior),
                    ..EvidenceQuery::default()
                })
                .unwrap()
        };
        assert_eq!(evidence.len(), 1);
        let record = &evidence[0];
        assert_eq!(record.linked_agent_run_ids, vec!["run-controlled-pilot-1"]);
        assert_eq!(record.run_metadata["pilotRunId"], "run-controlled-pilot-1");
        assert_eq!(record.run_metadata["sourceSessionId"], "session-1");
        assert_eq!(record.run_metadata["targetSessionId"], "session-1");
        assert_eq!(record.run_metadata["strategyKind"], "react");
        assert_eq!(record.run_metadata["payloadKind"], "react");
        assert_eq!(record.run_metadata["governanceDecisionKind"], "allow");
        assert_eq!(
            record.run_metadata["promotedMessageHash"],
            "checksum:test-safe-digest"
        );
        assert_eq!(record.run_metadata["promotedAt"], "2026-05-30T01:02:03Z");

        let serialized = serde_json::to_string(record).unwrap();
        assert!(!serialized.contains(raw_pilot_response));
        assert!(!serialized.contains("private@example.com"));
        assert!(!serialized.contains("rawUserInput"));
        assert!(!serialized.contains("rawAssistantResponse"));
        assert!(!serialized.contains("toolPayload"));
    }

    #[tokio::test]
    async fn promotion_evidence_command_rejects_source_target_mismatch() {
        let state = preview_state().await;
        let err = record_controlled_pilot_promotion_evidence_with_state(
            ControlledPilotPromotionEvidenceInput {
                pilot_run_id: "run-controlled-pilot-1".into(),
                source_session_id: "session-1".into(),
                target_session_id: "session-2".into(),
                strategy_kind: "react".into(),
                payload_kind: "react".into(),
                governance_decision_kind: Some("allow".into()),
                promoted_message_length: 17,
                promoted_message_hash: "checksum:test-safe-digest".into(),
                promoted_at: Some("2026-05-30T01:02:03Z".into()),
            },
            &state,
        )
        .await
        .unwrap_err();

        assert!(err.contains("sourceSessionId must match targetSessionId"));
        let evidence = {
            let store = state.evidence_store.lock().await;
            store
                .query(EvidenceQuery {
                    affected_path: Some("runtime.controlled_pilot.promotion".into()),
                    evidence_type: Some(EvidenceType::RuntimeBehavior),
                    ..EvidenceQuery::default()
                })
                .unwrap()
        };
        assert!(evidence.is_empty());
    }

    #[tokio::test]
    async fn promotion_evidence_summary_returns_read_only_metadata() {
        let state = preview_state().await;
        for (run_id, promoted_at) in [
            ("run-controlled-pilot-1", "2026-05-30T01:02:03Z"),
            ("run-controlled-pilot-2", "2026-05-30T02:03:04Z"),
        ] {
            record_controlled_pilot_promotion_evidence_with_state(
                ControlledPilotPromotionEvidenceInput {
                    pilot_run_id: run_id.into(),
                    source_session_id: "session-1".into(),
                    target_session_id: "session-1".into(),
                    strategy_kind: "react".into(),
                    payload_kind: "react".into(),
                    governance_decision_kind: Some("allow".into()),
                    promoted_message_length: 17,
                    promoted_message_hash: format!("checksum:{run_id}"),
                    promoted_at: Some(promoted_at.into()),
                },
                &state,
            )
            .await
            .unwrap();
        }

        let summary = get_controlled_pilot_promotion_evidence_summary_with_state(&state)
            .await
            .unwrap();

        assert_eq!(summary.promoted_count, 2);
        assert_eq!(
            summary.recent_promoted_pilot_run_ids,
            vec!["run-controlled-pilot-2", "run-controlled-pilot-1"]
        );
        assert_eq!(
            summary.latest_promotion_timestamp.as_deref(),
            Some("2026-05-30T02:03:04Z")
        );
        assert_eq!(summary.source_target_mismatch_block_count, 0);
    }

    #[tokio::test]
    async fn promotion_readiness_blocks_without_enough_evidence_and_is_read_only() {
        let state = preview_state().await;
        record_controlled_pilot_promotion_evidence_with_state(
            ControlledPilotPromotionEvidenceInput {
                pilot_run_id: "run-controlled-pilot-1".into(),
                source_session_id: "session-1".into(),
                target_session_id: "session-1".into(),
                strategy_kind: "react".into(),
                payload_kind: "react".into(),
                governance_decision_kind: Some("allow".into()),
                promoted_message_length: 17,
                promoted_message_hash: "checksum:run-controlled-pilot-1".into(),
                promoted_at: Some("2026-05-30T01:02:03Z".into()),
            },
            &state,
        )
        .await
        .unwrap();
        let before_run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let before_pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        let before_evidence_count = {
            let store = state.evidence_store.lock().await;
            store.query(EvidenceQuery::default()).unwrap().len()
        };

        let report = check_controlled_pilot_promotion_readiness_with_state(
            ControlledPilotPromotionReadinessCheckInput {
                required_promotions: Some(3),
                session_id: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.ready);
        assert_eq!(report.required_promotions, 3);
        assert_eq!(report.promoted_count, 1);
        assert_eq!(
            report.recent_promoted_pilot_run_ids,
            vec!["run-controlled-pilot-1"]
        );
        assert_eq!(
            report.latest_promotion_timestamp.as_deref(),
            Some("2026-05-30T01:02:03Z")
        );
        assert!(report.metadata_safe_evidence_ready);
        assert!(report.default_chat_unchanged);
        assert!(report
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("insufficient_promotion_evidence")));

        let after_run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let after_pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        let after_evidence_count = {
            let store = state.evidence_store.lock().await;
            store.query(EvidenceQuery::default()).unwrap().len()
        };
        assert_eq!(before_run_count, after_run_count);
        assert_eq!(
            before_pending_proposals.len(),
            after_pending_proposals.len()
        );
        assert_eq!(before_evidence_count, after_evidence_count);
    }

    #[tokio::test]
    async fn promotion_readiness_passes_after_required_metadata_safe_promotions() {
        let state = preview_state().await;
        for (run_id, promoted_at) in [
            ("run-controlled-pilot-1", "2026-05-30T01:02:03Z"),
            ("run-controlled-pilot-2", "2026-05-30T02:03:04Z"),
            ("run-controlled-pilot-3", "2026-05-30T03:04:05Z"),
        ] {
            record_controlled_pilot_promotion_evidence_with_state(
                ControlledPilotPromotionEvidenceInput {
                    pilot_run_id: run_id.into(),
                    source_session_id: "session-1".into(),
                    target_session_id: "session-1".into(),
                    strategy_kind: "react".into(),
                    payload_kind: "react".into(),
                    governance_decision_kind: Some("allow".into()),
                    promoted_message_length: 17,
                    promoted_message_hash: format!("checksum:{run_id}"),
                    promoted_at: Some(promoted_at.into()),
                },
                &state,
            )
            .await
            .unwrap();
        }

        let report = check_controlled_pilot_promotion_readiness_with_state(
            ControlledPilotPromotionReadinessCheckInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.ready);
        assert_eq!(report.required_promotions, 3);
        assert_eq!(report.promoted_count, 3);
        assert_eq!(
            report.recent_promoted_pilot_run_ids,
            vec![
                "run-controlled-pilot-3",
                "run-controlled-pilot-2",
                "run-controlled-pilot-1"
            ]
        );
        assert_eq!(
            report.latest_promotion_timestamp.as_deref(),
            Some("2026-05-30T03:04:05Z")
        );
        assert_eq!(report.source_target_mismatch_block_count, 0);
        assert!(report.metadata_safe_evidence_ready);
        assert!(report.default_chat_unchanged);
        assert!(report.blocking_reasons.is_empty());
    }

    #[tokio::test]
    async fn promotion_readiness_blocks_when_source_target_mismatch_blocks_exist() {
        let state = preview_state().await;
        for run_id in [
            "run-controlled-pilot-1",
            "run-controlled-pilot-2",
            "run-controlled-pilot-3",
        ] {
            record_controlled_pilot_promotion_evidence_with_state(
                ControlledPilotPromotionEvidenceInput {
                    pilot_run_id: run_id.into(),
                    source_session_id: "session-1".into(),
                    target_session_id: "session-1".into(),
                    strategy_kind: "react".into(),
                    payload_kind: "react".into(),
                    governance_decision_kind: Some("allow".into()),
                    promoted_message_length: 17,
                    promoted_message_hash: format!("checksum:{run_id}"),
                    promoted_at: Some("2026-05-30T01:02:03Z".into()),
                },
                &state,
            )
            .await
            .unwrap();
        }
        {
            let store = state.evidence_store.lock().await;
            store
                .create_evidence(EvidenceDraft::new(
                    EvidenceType::RuntimeBehavior,
                    CONTROLLED_PILOT_PROMOTION_BLOCK_PATH,
                    1.0,
                    RiskLevel::Low,
                    EvidencePrivacyLevel::Internal,
                ))
                .unwrap();
        }

        let report = check_controlled_pilot_promotion_readiness_with_state(
            ControlledPilotPromotionReadinessCheckInput {
                required_promotions: Some(3),
                session_id: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.ready);
        assert_eq!(report.source_target_mismatch_block_count, 1);
        assert!(report
            .blocking_reasons
            .contains(&"source_target_mismatch_blocks_present".to_string()));
    }

    #[tokio::test]
    async fn promotion_readiness_blocks_non_metadata_safe_promotion_evidence() {
        let state = preview_state().await;
        {
            let store = state.evidence_store.lock().await;
            let mut draft = EvidenceDraft::new(
                EvidenceType::RuntimeBehavior,
                CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH,
                1.0,
                RiskLevel::Low,
                EvidencePrivacyLevel::Internal,
            )
            .with_linked_agent_run("run-controlled-pilot-raw");
            draft.run_metadata = json!({
                "evidenceKind": "controlled_pilot_promotion",
                "pilotRunId": "run-controlled-pilot-raw",
                "sourceSessionId": "session-1",
                "targetSessionId": "session-1",
                "strategyKind": "react",
                "payloadKind": "react",
                "promotedMessageHash": "checksum:raw",
                "metadataSafe": true,
                "contentStorage": "checksum_only",
                "toolStorage": "none",
                "pilotOutput": "raw pilot answer that must not be readiness-safe"
            });
            store.create_evidence(draft).unwrap();
        }

        let report = check_controlled_pilot_promotion_readiness_with_state(
            ControlledPilotPromotionReadinessCheckInput {
                required_promotions: Some(1),
                session_id: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.ready);
        assert!(!report.metadata_safe_evidence_ready);
        assert!(report
            .blocking_reasons
            .contains(&"promotion_evidence_not_metadata_safe".to_string()));
    }

    #[tokio::test]
    async fn migration_plan_draft_blocks_when_promotion_readiness_is_blocked_and_is_read_only() {
        let state = preview_state().await;
        record_controlled_pilot_promotion_evidence_with_state(
            ControlledPilotPromotionEvidenceInput {
                pilot_run_id: "run-controlled-pilot-1".into(),
                source_session_id: "session-1".into(),
                target_session_id: "session-1".into(),
                strategy_kind: "react".into(),
                payload_kind: "react".into(),
                governance_decision_kind: Some("allow".into()),
                promoted_message_length: 17,
                promoted_message_hash: "checksum:run-controlled-pilot-1".into(),
                promoted_at: Some("2026-05-30T01:02:03Z".into()),
            },
            &state,
        )
        .await
        .unwrap();
        let before_run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let before_pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap()
            .len();
        let before_evidence_count = {
            let store = state.evidence_store.lock().await;
            store.query(EvidenceQuery::default()).unwrap().len()
        };
        let before_patch_count = {
            let store = state.patch_store.as_ref().unwrap().lock().await;
            store.patch_count().unwrap()
        };
        let before_model = {
            let manager = state.life_model_manager.lock().await;
            manager.load().unwrap()
        };
        let before_messages = {
            let store = state.memory_store.lock().await;
            store.export_all_messages().unwrap()
        };

        let draft = draft_controlled_chat_migration_plan_with_state(
            ControlledChatMigrationPlanDraftInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!draft.draft_ready);
        assert!(!draft.readiness_report.ready);
        assert!(draft.migration_scope.is_empty());
        assert!(draft.required_preconditions.is_empty());
        assert!(draft.rollback_plan.is_empty());
        assert!(draft.fallback_plan.is_empty());
        assert!(draft.test_plan.is_empty());
        assert!(draft.manual_review_required);
        assert!(draft.not_automatic_migration);
        assert!(draft
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("insufficient_promotion_evidence")));

        let serialized = serde_json::to_string(&draft).unwrap();
        assert!(!serialized.contains("raw user"));
        assert!(!serialized.contains("raw user content"));
        assert!(!serialized.contains("raw assistant"));
        assert!(!serialized.contains("raw assistant output"));
        assert!(!serialized.contains("tool payload"));
        assert!(!serialized.contains("toolPayload"));
        assert!(!serialized.contains("Pilot-only answer"));

        let after_run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let after_pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap()
            .len();
        let after_evidence_count = {
            let store = state.evidence_store.lock().await;
            store.query(EvidenceQuery::default()).unwrap().len()
        };
        let after_patch_count = {
            let store = state.patch_store.as_ref().unwrap().lock().await;
            store.patch_count().unwrap()
        };
        let after_model = {
            let manager = state.life_model_manager.lock().await;
            manager.load().unwrap()
        };
        let after_messages = {
            let store = state.memory_store.lock().await;
            store.export_all_messages().unwrap()
        };

        assert_eq!(before_run_count, after_run_count);
        assert_eq!(before_pending_proposals, after_pending_proposals);
        assert_eq!(before_evidence_count, after_evidence_count);
        assert_eq!(before_patch_count, after_patch_count);
        assert_eq!(before_model.metadata.version, after_model.metadata.version);
        assert_eq!(
            serde_json::to_string(&before_messages).unwrap(),
            serde_json::to_string(&after_messages).unwrap()
        );
    }

    #[tokio::test]
    async fn migration_plan_draft_passes_with_complete_human_review_plan() {
        let state = preview_state().await;
        for (run_id, promoted_at) in [
            ("run-controlled-pilot-1", "2026-05-30T01:02:03Z"),
            ("run-controlled-pilot-2", "2026-05-30T02:03:04Z"),
            ("run-controlled-pilot-3", "2026-05-30T03:04:05Z"),
        ] {
            record_controlled_pilot_promotion_evidence_with_state(
                ControlledPilotPromotionEvidenceInput {
                    pilot_run_id: run_id.into(),
                    source_session_id: "session-1".into(),
                    target_session_id: "session-1".into(),
                    strategy_kind: "react".into(),
                    payload_kind: "react".into(),
                    governance_decision_kind: Some("allow".into()),
                    promoted_message_length: 17,
                    promoted_message_hash: format!("checksum:{run_id}"),
                    promoted_at: Some(promoted_at.into()),
                },
                &state,
            )
            .await
            .unwrap();
        }

        let draft = draft_controlled_chat_migration_plan_with_state(
            ControlledChatMigrationPlanDraftInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(draft.draft_ready);
        assert!(draft.readiness_report.ready);
        assert!(draft.manual_review_required);
        assert!(draft.not_automatic_migration);
        assert!(draft.blocking_reasons.is_empty());
        assert!(!draft.migration_scope.is_empty());
        assert!(!draft.required_preconditions.is_empty());
        assert!(!draft.rollback_plan.is_empty());
        assert!(!draft.fallback_plan.is_empty());
        assert!(!draft.test_plan.is_empty());
        assert!(draft
            .migration_scope
            .iter()
            .any(|item| item.contains("default Chat remains unchanged")));
        assert!(draft
            .required_preconditions
            .iter()
            .any(|item| item.contains("separate human approval")));
        assert!(draft
            .rollback_plan
            .iter()
            .any(|item| item.contains("disable the controlled pilot entry")));
        assert!(draft
            .fallback_plan
            .iter()
            .any(|item| item.contains("existing default Chat send path")));
        assert!(draft
            .test_plan
            .iter()
            .any(|item| item.contains("send_message and start_stream_message")));

        let serialized = serde_json::to_string(&draft).unwrap();
        assert!(!serialized.contains("raw user content"));
        assert!(!serialized.contains("rawUserInput"));
        assert!(!serialized.contains("raw assistant output"));
        assert!(!serialized.contains("rawAssistantResponse"));
        assert!(!serialized.contains("tool payload"));
        assert!(!serialized.contains("toolPayload"));
        assert!(!serialized.contains("Pilot-only answer"));
    }

    async fn seed_ready_migration_review_promotions(state: &Arc<crate::AppState>) {
        for (run_id, promoted_at) in [
            ("run-controlled-pilot-1", "2026-05-30T01:02:03Z"),
            ("run-controlled-pilot-2", "2026-05-30T02:03:04Z"),
            ("run-controlled-pilot-3", "2026-05-30T03:04:05Z"),
        ] {
            record_controlled_pilot_promotion_evidence_with_state(
                ControlledPilotPromotionEvidenceInput {
                    pilot_run_id: run_id.into(),
                    source_session_id: "session-1".into(),
                    target_session_id: "session-1".into(),
                    strategy_kind: "react".into(),
                    payload_kind: "react".into(),
                    governance_decision_kind: Some("allow".into()),
                    promoted_message_length: 17,
                    promoted_message_hash: format!("checksum:{run_id}"),
                    promoted_at: Some(promoted_at.into()),
                },
                state,
            )
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn migration_review_decision_blocks_approve_when_draft_is_not_ready_without_evidence() {
        let state = preview_state().await;
        record_controlled_pilot_promotion_evidence_with_state(
            ControlledPilotPromotionEvidenceInput {
                pilot_run_id: "run-controlled-pilot-1".into(),
                source_session_id: "session-1".into(),
                target_session_id: "session-1".into(),
                strategy_kind: "react".into(),
                payload_kind: "react".into(),
                governance_decision_kind: Some("allow".into()),
                promoted_message_length: 17,
                promoted_message_hash: "checksum:run-controlled-pilot-1".into(),
                promoted_at: Some("2026-05-30T01:02:03Z".into()),
            },
            &state,
        )
        .await
        .unwrap();

        let result = record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some(
                    "Approve this blocked draft? secret@example.com".into(),
                ),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!result.recorded);
        assert!(result.evidence_id.is_none());
        assert_eq!(result.decision_kind, "approve");
        assert!(!result.draft_ready);
        assert!(result
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("insufficient_promotion_evidence")));

        let evidence = {
            let store = state.evidence_store.lock().await;
            store
                .query(EvidenceQuery {
                    affected_path: Some("runtime.controlled_chat.migration_review_decision".into()),
                    evidence_type: Some(EvidenceType::RuntimeBehavior),
                    ..EvidenceQuery::default()
                })
                .unwrap()
        };
        assert!(evidence.is_empty());
    }

    #[tokio::test]
    async fn migration_review_decision_records_metadata_safe_evidence_for_ready_decisions() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        let raw_reviewer_note = "Looks ready for discussion, but never store raw@example.com.";

        for decision_kind in ["approve", "reject", "request_rework"] {
            let result = record_controlled_chat_migration_review_decision_with_state(
                ControlledChatMigrationReviewDecisionInput {
                    decision_kind: decision_kind.into(),
                    required_promotions: Some(3),
                    session_id: Some("session-1".into()),
                    optional_reviewer_note: Some(raw_reviewer_note.into()),
                },
                &state,
            )
            .await
            .unwrap();

            assert!(result.recorded);
            assert!(result.evidence_id.is_some());
            assert!(result.draft_ready);
            assert_eq!(result.decision_kind, decision_kind);
            assert!(result.draft_hash.starts_with("sha256:"));
            assert!(result.blocking_reasons.is_empty());
        }

        let evidence = {
            let store = state.evidence_store.lock().await;
            store
                .query(EvidenceQuery {
                    affected_path: Some("runtime.controlled_chat.migration_review_decision".into()),
                    evidence_type: Some(EvidenceType::RuntimeBehavior),
                    ..EvidenceQuery::default()
                })
                .unwrap()
        };
        assert_eq!(evidence.len(), 3);

        let serialized = serde_json::to_string(&evidence).unwrap();
        assert!(!serialized.contains(raw_reviewer_note));
        assert!(!serialized.contains("raw@example.com"));
        assert!(!serialized.contains("optionalReviewerNote"));
        assert!(!serialized.contains("reviewerNoteRaw"));
        assert!(!serialized.contains("Pilot-only answer"));
        assert!(!serialized.contains("toolPayload"));

        for record in &evidence {
            assert_eq!(record.evidence_type, EvidenceType::RuntimeBehavior);
            assert_eq!(
                record.affected_path,
                "runtime.controlled_chat.migration_review_decision"
            );
            assert!(record.linked_agent_run_ids.is_empty());
            assert!(record.linked_proposal_ids.is_empty());
            assert_eq!(
                record.run_metadata["evidenceKind"],
                "migration_review_decision"
            );
            assert_eq!(record.run_metadata["metadataSafe"], true);
            assert_eq!(record.run_metadata["draftReady"], true);
            assert_eq!(
                record.run_metadata["readinessCounts"]["requiredPromotions"],
                3
            );
            assert_eq!(record.run_metadata["readinessCounts"]["promotedCount"], 3);
            assert!(record.run_metadata["draftHash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:"));
            assert!(record.run_metadata["createdAt"].as_str().is_some());
            assert_eq!(
                record.run_metadata["reviewerNote"]["length"],
                raw_reviewer_note.chars().count()
            );
            assert!(record.run_metadata["reviewerNote"]["checksum"]
                .as_str()
                .unwrap()
                .starts_with("sha256:"));
            assert!(matches!(
                record.run_metadata["reviewerNote"]["category"]
                    .as_str()
                    .unwrap(),
                "brief" | "standard" | "extended" | "none"
            ));
        }
    }

    #[tokio::test]
    async fn migration_review_decision_summary_is_read_only() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "reject".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some("Needs a clearer rollback owner.".into()),
            },
            &state,
        )
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let before_run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let before_pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap()
            .len();
        let before_evidence_count = {
            let store = state.evidence_store.lock().await;
            store.query(EvidenceQuery::default()).unwrap().len()
        };
        let before_patch_count = {
            let store = state.patch_store.as_ref().unwrap().lock().await;
            store.patch_count().unwrap()
        };
        let before_model = {
            let manager = state.life_model_manager.lock().await;
            manager.load().unwrap()
        };
        let before_messages = {
            let store = state.memory_store.lock().await;
            store.export_all_messages().unwrap()
        };

        let summary = get_controlled_chat_migration_review_decision_summary_with_state(&state)
            .await
            .unwrap();

        assert_eq!(summary.approved_count, 1);
        assert_eq!(summary.rework_reject_count, 1);
        assert_eq!(
            summary
                .latest_decision
                .as_ref()
                .map(|item| item.decision_kind.as_str()),
            Some("approve")
        );
        assert!(summary.latest_timestamp.is_some());
        assert!(summary.blocking_reasons.is_empty());

        let after_run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let after_pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap()
            .len();
        let after_evidence_count = {
            let store = state.evidence_store.lock().await;
            store.query(EvidenceQuery::default()).unwrap().len()
        };
        let after_patch_count = {
            let store = state.patch_store.as_ref().unwrap().lock().await;
            store.patch_count().unwrap()
        };
        let after_model = {
            let manager = state.life_model_manager.lock().await;
            manager.load().unwrap()
        };
        let after_messages = {
            let store = state.memory_store.lock().await;
            store.export_all_messages().unwrap()
        };

        assert_eq!(before_run_count, after_run_count);
        assert_eq!(before_pending_proposals, after_pending_proposals);
        assert_eq!(before_evidence_count, after_evidence_count);
        assert_eq!(before_patch_count, after_patch_count);
        assert_eq!(before_model.metadata.version, after_model.metadata.version);
        assert_eq!(
            serde_json::to_string(&before_messages).unwrap(),
            serde_json::to_string(&after_messages).unwrap()
        );
    }

    async fn seed_migration_review_decision_evidence(
        state: &Arc<crate::AppState>,
        decision_kind: &str,
        draft_hash: &str,
    ) {
        let store = state.evidence_store.lock().await;
        let mut draft = EvidenceDraft::new(
            EvidenceType::RuntimeBehavior,
            CONTROLLED_CHAT_MIGRATION_REVIEW_DECISION_EVIDENCE_PATH,
            1.0,
            RiskLevel::Low,
            EvidencePrivacyLevel::Internal,
        )
        .with_summary("Controlled chat migration review decision recorded");
        draft.run_metadata = json!({
            "evidenceKind": "migration_review_decision",
            "metadataSafe": true,
            "draftReady": true,
            "decisionKind": decision_kind,
            "readinessCounts": {
                "requiredPromotions": 3,
                "promotedCount": 3,
                "recentPromotedPilotRunCount": 3,
                "sourceTargetMismatchBlockCount": 0,
                "blockingReasonCount": 0
            },
            "draftHash": draft_hash,
            "createdAt": chrono::Utc::now().to_rfc3339(),
            "sessionId": "session-1",
            "reviewerNote": {
                "present": false,
                "length": 0,
                "checksum": null,
                "category": "none"
            },
            "blockingReasons": [],
            "metadataSafeEvidenceReady": true,
            "defaultChatUnchanged": true,
            "manualReviewRequired": true,
            "notAutomaticMigration": true,
            "contentStorage": "checksum_only",
            "reviewerNoteStorage": "length_checksum_category_only",
            "toolStorage": "none",
            "transcriptStorage": "none"
        });
        store.create_evidence(draft).unwrap();
    }

    struct SideEffectCounts {
        run_count: i64,
        pending_proposal_count: usize,
        evidence_count: usize,
        patch_count: usize,
        mcp_audit_count: usize,
        model_version: String,
        messages_json: String,
    }

    async fn side_effect_counts(state: &Arc<crate::AppState>) -> SideEffectCounts {
        let run_count = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.run_count().unwrap()
        };
        let pending_proposal_count = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap()
            .len();
        let evidence_count = {
            let store = state.evidence_store.lock().await;
            store.query(EvidenceQuery::default()).unwrap().len()
        };
        let patch_count = {
            let store = state.patch_store.as_ref().unwrap().lock().await;
            store.patch_count().unwrap()
        };
        let mcp_audit_count = {
            let store = state.mcp_audit_store.lock().await;
            store.list_logs(100).unwrap().len()
        };
        let model_version = {
            let manager = state.life_model_manager.lock().await;
            manager.load().unwrap().metadata.version
        };
        let messages_json = {
            let store = state.memory_store.lock().await;
            serde_json::to_string(&store.export_all_messages().unwrap()).unwrap()
        };

        SideEffectCounts {
            run_count,
            pending_proposal_count,
            evidence_count,
            patch_count,
            mcp_audit_count,
            model_version,
            messages_json,
        }
    }

    #[tokio::test]
    async fn implementation_gate_blocks_without_approve_evidence() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;

        let report = check_controlled_chat_migration_implementation_gate_with_state(
            ControlledChatMigrationImplementationGateInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.implementation_eligible);
        assert!(report.latest_decision.is_none());
        assert!(report.readiness_report.ready);
        assert!(!report.draft_hash_matched);
        assert!(!report.approved_after_latest_draft);
        assert!(report
            .blocking_reasons
            .contains(&"metadata_safe_approve_decision_missing".to_string()));
    }

    #[tokio::test]
    async fn implementation_gate_blocks_when_latest_decision_is_reject_or_request_rework() {
        for decision_kind in ["reject", "request_rework"] {
            let state = preview_state().await;
            seed_ready_migration_review_promotions(&state).await;
            record_controlled_chat_migration_review_decision_with_state(
                ControlledChatMigrationReviewDecisionInput {
                    decision_kind: "approve".into(),
                    required_promotions: Some(3),
                    session_id: Some("session-1".into()),
                    optional_reviewer_note: None,
                },
                &state,
            )
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            record_controlled_chat_migration_review_decision_with_state(
                ControlledChatMigrationReviewDecisionInput {
                    decision_kind: decision_kind.into(),
                    required_promotions: Some(3),
                    session_id: Some("session-1".into()),
                    optional_reviewer_note: None,
                },
                &state,
            )
            .await
            .unwrap();

            let report = check_controlled_chat_migration_implementation_gate_with_state(
                ControlledChatMigrationImplementationGateInput {
                    required_promotions: Some(3),
                    session_id: Some("session-1".into()),
                },
                &state,
            )
            .await
            .unwrap();

            assert!(!report.implementation_eligible);
            assert_eq!(
                report
                    .latest_decision
                    .as_ref()
                    .map(|decision| decision.decision_kind.as_str()),
                Some(decision_kind)
            );
            assert!(report.draft_hash_matched);
            assert!(!report.approved_after_latest_draft);
            assert!(report
                .blocking_reasons
                .contains(&format!("latest_review_decision_is_{decision_kind}")));
        }
    }

    #[tokio::test]
    async fn implementation_gate_blocks_when_approved_draft_hash_differs_from_current_draft() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        seed_migration_review_decision_evidence(&state, "approve", "sha256:stale-reviewed-draft")
            .await;

        let report = check_controlled_chat_migration_implementation_gate_with_state(
            ControlledChatMigrationImplementationGateInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.implementation_eligible);
        assert_eq!(
            report
                .latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str()),
            Some("approve")
        );
        assert!(report.readiness_report.ready);
        assert!(!report.draft_hash_matched);
        assert!(!report.approved_after_latest_draft);
        assert!(report
            .blocking_reasons
            .contains(&"approved_draft_hash_mismatch".to_string()));
    }

    #[tokio::test]
    async fn implementation_gate_blocks_when_current_readiness_fails() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        {
            let store = state.evidence_store.lock().await;
            store
                .create_evidence(EvidenceDraft::new(
                    EvidenceType::RuntimeBehavior,
                    CONTROLLED_PILOT_PROMOTION_BLOCK_PATH,
                    1.0,
                    RiskLevel::Low,
                    EvidencePrivacyLevel::Internal,
                ))
                .unwrap();
        }

        let report = check_controlled_chat_migration_implementation_gate_with_state(
            ControlledChatMigrationImplementationGateInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.implementation_eligible);
        assert!(!report.readiness_report.ready);
        assert!(report
            .blocking_reasons
            .contains(&"promotion_readiness_currently_blocked".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"source_target_mismatch_blocks_present".to_string()));
    }

    #[tokio::test]
    async fn implementation_gate_is_eligible_with_latest_approve_readiness_pass_and_hash_match() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some("Ready to discuss implementation.".into()),
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let report = check_controlled_chat_migration_implementation_gate_with_state(
            ControlledChatMigrationImplementationGateInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.implementation_eligible);
        assert_eq!(
            report
                .latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str()),
            Some("approve")
        );
        assert!(report.readiness_report.ready);
        assert!(report.draft_hash_matched);
        assert!(report.approved_after_latest_draft);
        assert!(report.blocking_reasons.is_empty());

        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn shadow_run_blocks_when_implementation_gate_is_blocked_without_running_runtime() {
        let state = preview_state().await;
        let before = side_effect_counts(&state).await;

        let output = run_controlled_chat_migration_shadow_run_with_state(
            ControlledChatMigrationShadowRunInput {
                session_id: "session-1".into(),
                user_input_checksum: Some("sha256:raw-user-input-checksum".into()),
                bounded_test_prompt_descriptor: Some("default_readiness_probe".into()),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!output.shadow_run_ready);
        assert!(!output.implementation_gate_report.implementation_eligible);
        assert_eq!(output.strategy_kind, "notRun");
        assert_eq!(output.payload_kind, "notRun");
        assert!(output
            .blocking_reasons
            .contains(&"implementation_gate_blocked".to_string()));
        assert!(output
            .blocking_reasons
            .contains(&"metadata_safe_approve_decision_missing".to_string()));

        let serialized = serde_json::to_string(&output).unwrap();
        assert!(!serialized.contains("raw user"));
        assert!(!serialized.contains("raw assistant"));
        assert!(!serialized.contains("toolPayload"));
        assert!(!serialized.contains("full tool payload"));

        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn shadow_run_executes_when_implementation_gate_is_eligible_with_write_disabled_audit() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some("Ready for a shadow run.".into()),
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let output = run_controlled_chat_migration_shadow_run_with_state(
            ControlledChatMigrationShadowRunInput {
                session_id: "session-1".into(),
                user_input_checksum: Some("sha256:raw-user-input-checksum".into()),
                bounded_test_prompt_descriptor: Some("planning_readiness_probe".into()),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(output.shadow_run_ready);
        assert!(output.implementation_gate_report.implementation_eligible);
        assert_eq!(output.strategy_kind, "planExecute");
        assert_eq!(output.payload_kind, "planExecute");
        assert!(output.blocking_reasons.is_empty());
        assert_eq!(
            output.metadata_safe_summary["descriptorKind"],
            "planning_readiness_probe"
        );
        assert_eq!(output.metadata_safe_summary["allowWrites"], false);
        assert_eq!(output.metadata_safe_summary["metadataSafe"], true);

        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count + 1, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);

        let shadow_runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs_for_session("session-1", 20).unwrap()
        };
        let shadow_run = shadow_runs
            .iter()
            .find(|run| {
                run.reasoning_strategy.as_deref() == Some("controlled_migration_shadow_run")
            })
            .expect("shadow run audit should be persisted separately from preview evidence");
        assert_eq!(shadow_run.status, AgentRunStatus::Completed);
        assert_eq!(shadow_run.user_input, None);
        assert!(shadow_run.actions.is_empty());
        assert!(shadow_run.observations.is_empty());
        assert_eq!(shadow_run.tool_call_count, 0);
        assert!(shadow_run.generated_proposals.is_empty());

        let audit = preview_audit(shadow_run);
        assert_eq!(audit["shadowRunRuntime"], "controlled_chat_migration");
        assert_eq!(audit["metadataSafe"], true);
        assert_eq!(audit["writeControl"]["allowWrites"], false);
        assert_eq!(audit["contentStorage"], "none");
        assert_eq!(audit["toolStorage"], "none");
        assert_eq!(audit["chatHistoryStorage"], "none");

        let serialized_output = serde_json::to_string(&output).unwrap();
        let serialized_run = serde_json::to_string(shadow_run).unwrap();
        for serialized in [serialized_output, serialized_run] {
            assert!(!serialized.contains("raw user"));
            assert!(!serialized.contains("rawUserInput"));
            assert!(!serialized.contains("raw assistant"));
            assert!(!serialized.contains("rawAssistantOutput"));
            assert!(!serialized.contains("toolPayload"));
            assert!(!serialized.contains("full tool payload"));
            assert!(!serialized.contains("Plan a controlled migration comparison."));
            assert!(!serialized.contains("Pilot-only answer"));
        }
    }

    fn completed_shadow_review_run(run_id: &str) -> AgentRun {
        let mut run = AgentRun::new_chat_run("session-1", "raw prompt should not persist");
        run.id = run_id.to_string();
        run.status = AgentRunStatus::Completed;
        run.user_input = None;
        run.reasoning_strategy = Some("controlled_migration_shadow_run".into());
        run.output_preview = Some("Controlled migration shadow run: react / react".into());
        run.generated_proposals = Vec::new();
        run.actions = Vec::new();
        run.observations = Vec::new();
        run.tool_call_count = 0;
        run.finished_at = Some(chrono::Utc::now());
        run.reasoning_trace = Some(ReasoningTrace {
            strategy_result: Some(json!({
                "shadowRunRuntime": "controlled_chat_migration",
                "strategyKind": "react",
                "payloadKind": "react",
                "descriptorKind": "default_readiness_probe",
                "allowWrites": false,
                "metadataSafe": true,
                "contentStorage": "none",
                "toolStorage": "none",
                "chatHistoryStorage": "none",
                "proposalStorage": "none",
                "lifeModelPatchStorage": "none",
                "memoryStorage": "none",
                "proposalIdCount": 0,
                "writeControl": {
                    "allowWrites": false,
                    "declaredWriteStepCount": 0,
                    "proposalRequiredStepCount": 0,
                    "blockedStepCount": 0
                }
            })),
            output: Some("controlled_migration_shadow_run".into()),
            ..ReasoningTrace::default()
        });
        run
    }

    async fn insert_shadow_review_run(state: &Arc<crate::AppState>, run: &AgentRun) {
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        store.create_run(run).unwrap();
    }

    async fn shadow_review_evidence_records(
        state: &Arc<crate::AppState>,
    ) -> Vec<openlife_core::agent::EvidenceRecord> {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    "runtime.controlled_chat.migration_shadow_review_decision".into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .unwrap()
    }

    async fn seed_shadow_review_decision_evidence(
        state: &Arc<crate::AppState>,
        shadow_run_id: &str,
        decision_kind: &str,
    ) {
        let store = state.evidence_store.lock().await;
        let mut draft = EvidenceDraft::new(
            EvidenceType::RuntimeBehavior,
            CONTROLLED_CHAT_MIGRATION_SHADOW_REVIEW_DECISION_EVIDENCE_PATH,
            1.0,
            RiskLevel::Low,
            EvidencePrivacyLevel::Internal,
        );
        draft.run_metadata = json!({
            "shadowRunId": shadow_run_id,
            "decisionKind": decision_kind,
            "reviewerNoteChecksum": null,
            "reviewerNoteLength": 0,
            "reviewerNoteCategory": "none",
            "readinessSummaryDigest": "sha256:seeded-shadow-readiness",
            "createdAt": chrono::Utc::now().to_rfc3339(),
        });
        store.create_evidence(draft).unwrap();
    }

    #[tokio::test]
    async fn shadow_review_invalid_run_is_blocked_without_evidence() {
        let state = preview_state().await;

        let result = record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-missing".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: Some("Do not store reviewer@example.com".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!result.recorded);
        assert!(result.evidence_id.is_none());
        assert_eq!(result.shadow_run_id, "run-shadow-missing");
        assert_eq!(result.decision_kind, "approve");
        assert!(result
            .blocking_reasons
            .contains(&"shadow_run_missing".to_string()));
        assert!(shadow_review_evidence_records(&state).await.is_empty());
    }

    #[tokio::test]
    async fn shadow_review_non_shadow_run_is_blocked_without_evidence() {
        let state = preview_state().await;
        let mut run = completed_shadow_review_run("run-not-shadow");
        run.reasoning_strategy = Some("multi_strategy_preview".into());
        insert_shadow_review_run(&state, &run).await;

        let result = record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-not-shadow".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!result.recorded);
        assert!(result.evidence_id.is_none());
        assert!(result
            .blocking_reasons
            .contains(&"shadow_run_strategy_mismatch".to_string()));
        assert!(shadow_review_evidence_records(&state).await.is_empty());
    }

    #[tokio::test]
    async fn shadow_review_unfinished_run_is_blocked_without_evidence() {
        let state = preview_state().await;
        let mut run = completed_shadow_review_run("run-shadow-running");
        run.status = AgentRunStatus::Running;
        run.finished_at = None;
        insert_shadow_review_run(&state, &run).await;

        let result = record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-running".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!result.recorded);
        assert!(result.evidence_id.is_none());
        assert!(result
            .blocking_reasons
            .contains(&"shadow_run_not_completed".to_string()));
        assert!(shadow_review_evidence_records(&state).await.is_empty());
    }

    #[tokio::test]
    async fn shadow_review_reject_and_rework_require_ready_shadow_run() {
        let state = preview_state().await;
        let mut run = completed_shadow_review_run("run-shadow-not-metadata-safe");
        let strategy_result = run
            .reasoning_trace
            .as_mut()
            .and_then(|trace| trace.strategy_result.as_mut())
            .and_then(Value::as_object_mut)
            .unwrap();
        strategy_result.insert("metadataSafe".into(), json!(false));
        insert_shadow_review_run(&state, &run).await;

        for decision_kind in ["reject", "request_rework"] {
            let result = record_controlled_chat_migration_shadow_review_decision_with_state(
                ControlledChatMigrationShadowReviewDecisionInput {
                    shadow_run_id: "run-shadow-not-metadata-safe".into(),
                    decision_kind: decision_kind.into(),
                    optional_reviewer_note: Some("Do not persist raw review notes.".into()),
                },
                &state,
            )
            .await
            .unwrap();

            assert!(!result.recorded);
            assert!(result.evidence_id.is_none());
            assert!(result
                .blocking_reasons
                .contains(&"shadow_run_metadata_not_safe".to_string()));
        }
        assert!(shadow_review_evidence_records(&state).await.is_empty());
    }

    #[tokio::test]
    async fn shadow_review_approve_records_only_metadata_safe_evidence() {
        let state = preview_state().await;
        let run = completed_shadow_review_run("run-shadow-ready-approve");
        insert_shadow_review_run(&state, &run).await;
        let before = side_effect_counts(&state).await;
        let raw_note = "Approve this shadow run, but never store raw-reviewer@example.com.";

        let result = record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-ready-approve".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: Some(raw_note.into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(result.recorded);
        assert!(result.evidence_id.is_some());
        assert_eq!(result.decision_kind, "approve");
        assert!(result.readiness_summary_digest.starts_with("sha256:"));
        assert!(result.blocking_reasons.is_empty());

        let evidence = shadow_review_evidence_records(&state).await;
        assert_eq!(evidence.len(), 1);
        let record = &evidence[0];
        assert!(record.summary.is_none());
        assert!(record.source_refs.is_empty());
        assert!(record.linked_agent_run_ids.is_empty());
        assert!(record.linked_proposal_ids.is_empty());

        let metadata = record.run_metadata.as_object().unwrap();
        let mut keys = metadata.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "createdAt",
                "decisionKind",
                "readinessSummaryDigest",
                "reviewerNoteCategory",
                "reviewerNoteChecksum",
                "reviewerNoteLength",
                "shadowRunId"
            ]
        );
        assert_eq!(
            record.run_metadata["shadowRunId"],
            "run-shadow-ready-approve"
        );
        assert_eq!(record.run_metadata["decisionKind"], "approve");
        assert_eq!(
            record.run_metadata["reviewerNoteLength"],
            raw_note.chars().count()
        );
        assert!(record.run_metadata["reviewerNoteChecksum"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert_eq!(record.run_metadata["reviewerNoteCategory"], "brief");
        assert!(record.run_metadata["readinessSummaryDigest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(record.run_metadata["createdAt"].as_str().is_some());

        let serialized = serde_json::to_string(record).unwrap();
        assert!(!serialized.contains(raw_note));
        assert!(!serialized.contains("raw-reviewer@example.com"));
        assert!(!serialized.contains("reviewerNoteRaw"));
        assert!(!serialized.contains("shadowPrompt"));
        assert!(!serialized.contains("shadowOutput"));
        assert!(!serialized.contains("toolPayload"));

        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count + 1, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn shadow_review_reject_and_request_rework_can_be_recorded() {
        let state = preview_state().await;

        for decision_kind in ["reject", "request_rework"] {
            let run_id = format!("run-shadow-ready-{decision_kind}");
            let run = completed_shadow_review_run(&run_id);
            insert_shadow_review_run(&state, &run).await;

            let result = record_controlled_chat_migration_shadow_review_decision_with_state(
                ControlledChatMigrationShadowReviewDecisionInput {
                    shadow_run_id: run_id.clone(),
                    decision_kind: decision_kind.into(),
                    optional_reviewer_note: Some("Needs human follow-up.".into()),
                },
                &state,
            )
            .await
            .unwrap();

            assert!(result.recorded);
            assert_eq!(result.decision_kind, decision_kind);
            assert_eq!(result.shadow_run_id, run_id);
        }

        let summary = get_controlled_chat_migration_shadow_review_summary_with_state(&state)
            .await
            .unwrap();
        assert_eq!(summary.approved_count, 0);
        assert_eq!(summary.rework_reject_count, 2);
        assert!(matches!(
            summary
                .latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str()),
            Some("reject" | "request_rework")
        ));
    }

    #[tokio::test]
    async fn shadow_review_summary_is_read_only() {
        let state = preview_state().await;
        let approve_run = completed_shadow_review_run("run-shadow-summary-approve");
        insert_shadow_review_run(&state, &approve_run).await;
        record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-summary-approve".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let rework_run = completed_shadow_review_run("run-shadow-summary-rework");
        insert_shadow_review_run(&state, &rework_run).await;
        record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-summary-rework".into(),
                decision_kind: "request_rework".into(),
                optional_reviewer_note: Some("Needs rework.".into()),
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let summary = get_controlled_chat_migration_shadow_review_summary_with_state(&state)
            .await
            .unwrap();

        assert_eq!(summary.approved_count, 1);
        assert_eq!(summary.rework_reject_count, 1);
        assert_eq!(
            summary
                .latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str()),
            Some("request_rework")
        );
        assert_eq!(
            summary
                .latest_decision
                .as_ref()
                .map(|decision| decision.shadow_run_id.as_str()),
            Some("run-shadow-summary-rework")
        );
        assert!(summary.latest_timestamp.is_some());
        assert!(summary.blocking_reasons.is_empty());

        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn cutover_readiness_blocks_without_implementation_gate_eligibility() {
        let state = preview_state().await;
        let run = completed_shadow_review_run("run-shadow-cutover-implementation-blocked");
        insert_shadow_review_run(&state, &run).await;
        record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-cutover-implementation-blocked".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_controlled_chat_cutover_readiness_with_state(
            ControlledChatCutoverReadinessInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.cutover_planning_eligible);
        assert!(!report.implementation_gate_report.implementation_eligible);
        assert!(report
            .blocking_reasons
            .contains(&"implementation_gate_not_eligible".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"metadata_safe_approve_decision_missing".to_string()));
    }

    #[tokio::test]
    async fn cutover_readiness_blocks_without_approved_shadow_review() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_controlled_chat_cutover_readiness_with_state(
            ControlledChatCutoverReadinessInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.cutover_planning_eligible);
        assert!(report.implementation_gate_report.implementation_eligible);
        assert!(report.latest_shadow_review_decision.is_none());
        assert!(report
            .blocking_reasons
            .contains(&"shadow_review_approve_missing".to_string()));
    }

    #[tokio::test]
    async fn cutover_readiness_blocks_if_approved_shadow_run_missing() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        seed_shadow_review_decision_evidence(&state, "run-shadow-cutover-missing", "approve").await;

        let report = check_controlled_chat_cutover_readiness_with_state(
            ControlledChatCutoverReadinessInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.cutover_planning_eligible);
        assert_eq!(
            report
                .latest_shadow_review_decision
                .as_ref()
                .map(|decision| decision.shadow_run_id.as_str()),
            Some("run-shadow-cutover-missing")
        );
        assert!(report.verified_shadow_run_id.is_none());
        assert!(report
            .blocking_reasons
            .contains(&"shadow_run_missing".to_string()));
    }

    #[tokio::test]
    async fn cutover_readiness_blocks_if_shadow_run_no_longer_metadata_safe_write_disabled_or_side_effect_free(
    ) {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let mut run = completed_shadow_review_run("run-shadow-cutover-drifted");
        insert_shadow_review_run(&state, &run).await;
        record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-cutover-drifted".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let audit = run
            .reasoning_trace
            .as_mut()
            .and_then(|trace| trace.strategy_result.as_mut())
            .and_then(Value::as_object_mut)
            .unwrap();
        audit.insert("metadataSafe".into(), json!(false));
        audit.insert("allowWrites".into(), json!(true));
        audit
            .get_mut("writeControl")
            .and_then(Value::as_object_mut)
            .unwrap()
            .insert("allowWrites".into(), json!(true));
        run.tool_call_count = 1;
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.update_run(&run).unwrap();
        }

        let report = check_controlled_chat_cutover_readiness_with_state(
            ControlledChatCutoverReadinessInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.cutover_planning_eligible);
        assert!(report
            .blocking_reasons
            .contains(&"shadow_run_metadata_not_safe".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"shadow_run_allow_writes_not_false".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"shadow_run_external_write_side_effects_present".to_string()));
    }

    #[tokio::test]
    async fn cutover_readiness_passes_when_w27_w29_and_shadow_run_readiness_are_valid() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let run = completed_shadow_review_run("run-shadow-cutover-ready");
        insert_shadow_review_run(&state, &run).await;
        record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-cutover-ready".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: Some("Ready for cutover planning discussion.".into()),
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_controlled_chat_cutover_readiness_with_state(
            ControlledChatCutoverReadinessInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.cutover_planning_eligible);
        assert!(report.required_evidence_ready);
        assert!(report.default_chat_unchanged);
        assert_eq!(
            report.verified_shadow_run_id.as_deref(),
            Some("run-shadow-cutover-ready")
        );
        assert!(report
            .readiness_summary_digest
            .as_deref()
            .unwrap()
            .starts_with("sha256:"));
        assert!(report.blocking_reasons.is_empty());
        assert_eq!(report.metadata_safe_summary["metadataSafe"], true);
        assert_eq!(report.metadata_safe_summary["planningOnly"], true);
        assert_eq!(report.metadata_safe_summary["contentStorage"], "none");

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("Ready for cutover planning discussion."));
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("raw output"));
        assert!(!serialized.contains("toolPayload"));
    }

    #[tokio::test]
    async fn cutover_readiness_command_is_read_only_by_side_effect_counts() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let run = completed_shadow_review_run("run-shadow-cutover-read-only");
        insert_shadow_review_run(&state, &run).await;
        record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-cutover-read-only".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let report = check_controlled_chat_cutover_readiness_with_state(
            ControlledChatCutoverReadinessInput {
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.cutover_planning_eligible);
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn cutover_candidate_blocks_when_w30_readiness_is_not_eligible_without_runtime() {
        let state = preview_state().await;
        let before = side_effect_counts(&state).await;

        let output = run_controlled_chat_cutover_candidate_with_state(
            ControlledChatCutoverCandidateInput {
                session_id: "session-candidate-blocked".into(),
                bounded_test_prompt_descriptor: Some("default_contract_probe".into()),
                user_input_checksum: Some("sha256:candidate-input".into()),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!output.candidate_ready);
        assert!(output.candidate_run_id.is_none());
        assert_eq!(output.contract_shape, "blocked");
        assert_eq!(
            output.output_preview.as_deref(),
            Some("Candidate blocked before runtime")
        );
        assert!(output.user_output.is_none());
        assert!(output
            .blocking_reasons
            .contains(&"cutover_readiness_not_eligible".to_string()));
        assert_eq!(output.metadata_safe_summary["blockedBeforeRuntime"], true);
        assert_eq!(output.metadata_safe_summary["metadataSafe"], true);
        assert_eq!(output.metadata_safe_summary["allowWrites"], false);
        assert_eq!(output.metadata_safe_summary["maxToolCalls"], 0);

        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn cutover_candidate_runs_only_when_w30_eligible_and_writes_metadata_safe_audit() {
        let state = preview_state().await;
        seed_ready_migration_review_promotions(&state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let run = completed_shadow_review_run("run-shadow-cutover-candidate-ready");
        insert_shadow_review_run(&state, &run).await;
        record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-cutover-candidate-ready".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let output = run_controlled_chat_cutover_candidate_with_state(
            ControlledChatCutoverCandidateInput {
                session_id: "session-candidate-ready".into(),
                bounded_test_prompt_descriptor: Some("default_contract_probe".into()),
                user_input_checksum: Some("sha256:candidate-input".into()),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(output.candidate_ready);
        assert_eq!(output.contract_shape, "send_message_compatible");
        assert!(output.candidate_run_id.is_some());
        assert!(output
            .user_output
            .as_deref()
            .is_some_and(|value| !value.is_empty()));
        assert!(output
            .output_preview
            .as_deref()
            .is_some_and(|value| value.starts_with("Cutover candidate: ")));
        assert!(output.blocking_reasons.is_empty());
        assert_eq!(
            output.metadata_safe_summary["candidateAdapter"],
            "controlled_chat_cutover_candidate"
        );
        assert_eq!(output.metadata_safe_summary["metadataSafe"], true);
        assert_eq!(output.metadata_safe_summary["nonDefault"], true);
        assert_eq!(output.metadata_safe_summary["allowWrites"], false);
        assert_eq!(output.metadata_safe_summary["maxToolCalls"], 0);
        assert_eq!(output.metadata_safe_summary["chatHistoryStorage"], "none");
        assert_eq!(output.metadata_safe_summary["proposalStorage"], "none");
        assert_eq!(output.metadata_safe_summary["memoryStorage"], "none");
        assert!(output
            .warnings
            .contains(&"candidate runtime forced allowWrites=false".to_string()));

        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count + 1, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);

        let candidate_run = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store
                .get_run(output.candidate_run_id.as_deref().unwrap())
                .unwrap()
                .expect("candidate AgentRun audit should be persisted")
        };
        assert_eq!(candidate_run.status, AgentRunStatus::Completed);
        assert_eq!(
            candidate_run.reasoning_strategy.as_deref(),
            Some("controlled_chat_cutover_candidate")
        );
        assert_eq!(candidate_run.user_input, None);
        assert!(candidate_run.actions.is_empty());
        assert!(candidate_run.observations.is_empty());
        assert_eq!(candidate_run.tool_call_count, 0);
        assert!(candidate_run.generated_proposals.is_empty());

        let audit = preview_audit(&candidate_run);
        assert_eq!(
            audit["candidateAdapter"],
            "controlled_chat_cutover_candidate"
        );
        assert_eq!(audit["metadataSafe"], true);
        assert_eq!(audit["writeControl"]["allowWrites"], false);
        assert_eq!(audit["runtimeLimits"]["maxToolCalls"], 0);
        assert_eq!(audit["contentStorage"], "none");
        assert_eq!(audit["toolStorage"], "none");
        assert_eq!(audit["chatHistoryStorage"], "none");
        assert_eq!(audit["proposalStorage"], "none");
        assert_eq!(audit["lifeModelPatchStorage"], "none");
        assert_eq!(audit["memoryStorage"], "none");

        let serialized_output = serde_json::to_string(&output).unwrap();
        let serialized_run = serde_json::to_string(&candidate_run).unwrap();
        assert!(!serialized_run.contains(output.user_output.as_deref().unwrap()));
        for serialized in [serialized_output, serialized_run] {
            assert!(!serialized.contains("raw user"));
            assert!(!serialized.contains("rawUserInput"));
            assert!(!serialized.contains("raw assistant"));
            assert!(!serialized.contains("rawAssistantOutput"));
            assert!(!serialized.contains("toolPayload"));
            assert!(!serialized.contains("full tool payload"));
            assert!(!serialized.contains("Provide a concise default Chat contract response"));
            assert!(!serialized.contains("Candidate-only answer"));
        }
    }

    fn completed_cutover_candidate_review_run(run_id: &str) -> AgentRun {
        let mut run = AgentRun::new_chat_run("session-1", "raw prompt should not persist");
        run.id = run_id.to_string();
        run.status = AgentRunStatus::Completed;
        run.user_input = None;
        run.reasoning_strategy = Some("controlled_chat_cutover_candidate".into());
        run.output_preview = Some("Cutover candidate: react / react".into());
        run.generated_proposals = Vec::new();
        run.actions = Vec::new();
        run.observations = Vec::new();
        run.tool_call_count = 0;
        run.finished_at = Some(chrono::Utc::now());
        run.reasoning_trace = Some(ReasoningTrace {
            strategy_result: Some(json!({
                "candidateAdapter": "controlled_chat_cutover_candidate",
                "strategyKind": "react",
                "payloadKind": "react",
                "contractShape": "send_message_compatible",
                "candidateReady": true,
                "descriptorKind": "default_contract_probe",
                "metadataSafe": true,
                "nonDefault": true,
                "defaultChatUnchanged": true,
                "runtimeLimits": {
                    "allowWrites": false,
                    "maxToolCalls": 0
                },
                "contentStorage": "none",
                "toolStorage": "none",
                "chatHistoryStorage": "none",
                "proposalStorage": "none",
                "lifeModelPatchStorage": "none",
                "memoryStorage": "none",
                "evidenceStorage": "none",
                "mcpAuditStorage": "none",
                "proposalIdCount": 0,
                "writeControl": {
                    "allowWrites": false,
                    "declaredWriteStepCount": 0,
                    "proposalRequiredStepCount": 0,
                    "blockedStepCount": 0
                }
            })),
            output: Some("controlled_chat_cutover_candidate".into()),
            ..ReasoningTrace::default()
        });
        run
    }

    async fn insert_cutover_candidate_review_run(state: &Arc<crate::AppState>, run: &AgentRun) {
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        store.create_run(run).unwrap();
    }

    async fn cutover_candidate_review_evidence_records(
        state: &Arc<crate::AppState>,
    ) -> Vec<openlife_core::agent::EvidenceRecord> {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    "runtime.controlled_chat.cutover_candidate_review_decision".into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .unwrap()
    }

    async fn seed_cutover_candidate_promotion_w30_ready(state: &Arc<crate::AppState>) {
        seed_ready_migration_review_promotions(state).await;
        record_controlled_chat_migration_review_decision_with_state(
            ControlledChatMigrationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            state,
        )
        .await
        .unwrap();
        let run = completed_shadow_review_run("run-shadow-candidate-promotion-ready");
        insert_shadow_review_run(state, &run).await;
        record_controlled_chat_migration_shadow_review_decision_with_state(
            ControlledChatMigrationShadowReviewDecisionInput {
                shadow_run_id: "run-shadow-candidate-promotion-ready".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            state,
        )
        .await
        .unwrap();
    }

    async fn seed_cutover_candidate_review_decision_evidence(
        state: &Arc<crate::AppState>,
        candidate_run_id: &str,
        decision_kind: &str,
        candidate_summary_digest: &str,
    ) {
        let store = state.evidence_store.lock().await;
        let mut draft = EvidenceDraft::new(
            EvidenceType::RuntimeBehavior,
            CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH,
            1.0,
            RiskLevel::Low,
            EvidencePrivacyLevel::Internal,
        );
        draft.run_metadata = json!({
            "candidateRunId": candidate_run_id,
            "decisionKind": decision_kind,
            "contractShape": "send_message_compatible",
            "candidateSummaryDigest": candidate_summary_digest,
            "reviewerNoteChecksum": null,
            "reviewerNoteLength": 0,
            "reviewerNoteCategory": "none",
            "createdAt": chrono::Utc::now().to_rfc3339(),
        });
        store.create_evidence(draft).unwrap();
    }

    #[tokio::test]
    async fn cutover_candidate_review_blocks_missing_candidate_run_without_evidence() {
        let state = preview_state().await;

        let result = record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-missing".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: Some("Do not store reviewer@example.com".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!result.recorded);
        assert!(result.evidence_id.is_none());
        assert_eq!(result.candidate_run_id, "run-candidate-missing");
        assert_eq!(result.decision_kind, "approve");
        assert!(result
            .blocking_reasons
            .contains(&"candidate_run_missing".to_string()));
        assert!(cutover_candidate_review_evidence_records(&state)
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn cutover_candidate_review_blocks_non_candidate_run_without_evidence() {
        let state = preview_state().await;
        let mut run = completed_cutover_candidate_review_run("run-not-candidate");
        run.reasoning_strategy = Some("controlled_migration_shadow_run".into());
        insert_cutover_candidate_review_run(&state, &run).await;

        let result = record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-not-candidate".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!result.recorded);
        assert!(result.evidence_id.is_none());
        assert!(result
            .blocking_reasons
            .contains(&"candidate_run_strategy_mismatch".to_string()));
        assert!(cutover_candidate_review_evidence_records(&state)
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn cutover_candidate_review_blocks_unfinished_candidate_run_without_evidence() {
        let state = preview_state().await;
        let mut run = completed_cutover_candidate_review_run("run-candidate-running");
        run.status = AgentRunStatus::Running;
        run.finished_at = None;
        insert_cutover_candidate_review_run(&state, &run).await;

        let result = record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-running".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!result.recorded);
        assert!(result.evidence_id.is_none());
        assert!(result
            .blocking_reasons
            .contains(&"candidate_run_not_completed".to_string()));
        assert!(cutover_candidate_review_evidence_records(&state)
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn cutover_candidate_review_blocks_approve_when_candidate_is_not_ready_or_not_compatible()
    {
        for (run_id, contract_shape, candidate_ready, expected_reason) in [
            (
                "run-candidate-not-ready",
                "send_message_compatible",
                false,
                "candidate_run_not_ready_for_approval",
            ),
            (
                "run-candidate-failed-shape",
                "failed",
                true,
                "candidate_run_contract_shape_not_send_message_compatible",
            ),
        ] {
            let state = preview_state().await;
            let mut run = completed_cutover_candidate_review_run(run_id);
            let audit = run
                .reasoning_trace
                .as_mut()
                .and_then(|trace| trace.strategy_result.as_mut())
                .and_then(Value::as_object_mut)
                .unwrap();
            audit.insert("contractShape".into(), json!(contract_shape));
            audit.insert("candidateReady".into(), json!(candidate_ready));
            insert_cutover_candidate_review_run(&state, &run).await;

            let result = record_controlled_chat_cutover_candidate_review_decision_with_state(
                ControlledChatCutoverCandidateReviewDecisionInput {
                    candidate_run_id: run_id.into(),
                    decision_kind: "approve".into(),
                    optional_reviewer_note: None,
                },
                &state,
            )
            .await
            .unwrap();

            assert!(!result.recorded);
            assert!(result.evidence_id.is_none());
            assert!(result
                .blocking_reasons
                .contains(&expected_reason.to_string()));
            assert!(cutover_candidate_review_evidence_records(&state)
                .await
                .is_empty());
        }
    }

    #[tokio::test]
    async fn cutover_candidate_review_approve_records_only_metadata_safe_evidence() {
        let state = preview_state().await;
        let run = completed_cutover_candidate_review_run("run-candidate-review-approve");
        insert_cutover_candidate_review_run(&state, &run).await;
        let before = side_effect_counts(&state).await;
        let raw_note = "Approve candidate, but never store raw-reviewer@example.com.";

        let result = record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-review-approve".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: Some(raw_note.into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(result.recorded);
        assert!(result.evidence_id.is_some());
        assert_eq!(result.decision_kind, "approve");
        assert_eq!(result.contract_shape, "send_message_compatible");
        assert!(result.candidate_summary_digest.starts_with("sha256:"));
        assert!(result.blocking_reasons.is_empty());

        let evidence = cutover_candidate_review_evidence_records(&state).await;
        assert_eq!(evidence.len(), 1);
        let record = &evidence[0];
        assert!(record.summary.is_none());
        assert!(record.source_refs.is_empty());
        assert!(record.linked_agent_run_ids.is_empty());
        assert!(record.linked_proposal_ids.is_empty());

        let metadata = record.run_metadata.as_object().unwrap();
        let mut keys = metadata.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "candidateRunId",
                "candidateSummaryDigest",
                "contractShape",
                "createdAt",
                "decisionKind",
                "reviewerNoteCategory",
                "reviewerNoteChecksum",
                "reviewerNoteLength"
            ]
        );
        assert_eq!(
            record.run_metadata["candidateRunId"],
            "run-candidate-review-approve"
        );
        assert_eq!(record.run_metadata["decisionKind"], "approve");
        assert_eq!(
            record.run_metadata["contractShape"],
            "send_message_compatible"
        );
        assert_eq!(
            record.run_metadata["reviewerNoteLength"],
            raw_note.chars().count()
        );
        assert!(record.run_metadata["reviewerNoteChecksum"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert_eq!(record.run_metadata["reviewerNoteCategory"], "brief");
        assert!(record.run_metadata["candidateSummaryDigest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(record.run_metadata["createdAt"].as_str().is_some());

        let serialized = serde_json::to_string(record).unwrap();
        assert!(!serialized.contains(raw_note));
        assert!(!serialized.contains("raw-reviewer@example.com"));
        assert!(!serialized.contains("reviewerNoteRaw"));
        assert!(!serialized.contains("candidate userOutput"));
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("raw assistant output"));
        assert!(!serialized.contains("toolPayload"));

        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count + 1, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn cutover_candidate_review_reject_and_request_rework_can_be_recorded_metadata_safe() {
        let state = preview_state().await;

        for decision_kind in ["reject", "request_rework"] {
            let run_id = format!("run-candidate-review-{decision_kind}");
            let run = completed_cutover_candidate_review_run(&run_id);
            insert_cutover_candidate_review_run(&state, &run).await;

            let result = record_controlled_chat_cutover_candidate_review_decision_with_state(
                ControlledChatCutoverCandidateReviewDecisionInput {
                    candidate_run_id: run_id.clone(),
                    decision_kind: decision_kind.into(),
                    optional_reviewer_note: Some("Needs human follow-up.".into()),
                },
                &state,
            )
            .await
            .unwrap();

            assert!(result.recorded);
            assert_eq!(result.decision_kind, decision_kind);
            assert_eq!(result.candidate_run_id, run_id);
            assert_eq!(result.contract_shape, "send_message_compatible");
        }

        let summary = get_controlled_chat_cutover_candidate_review_summary_with_state(&state)
            .await
            .unwrap();
        assert_eq!(summary.approved_count, 0);
        assert_eq!(summary.rework_reject_count, 2);
        assert!(matches!(
            summary
                .latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str()),
            Some("reject" | "request_rework")
        ));

        for record in cutover_candidate_review_evidence_records(&state).await {
            let serialized = serde_json::to_string(&record).unwrap();
            assert!(!serialized.contains("Needs human follow-up."));
            assert!(!serialized.contains("userOutput"));
            assert!(!serialized.contains("rawPrompt"));
            assert!(!serialized.contains("toolPayload"));
        }
    }

    #[tokio::test]
    async fn cutover_candidate_review_summary_is_read_only() {
        let state = preview_state().await;
        let approve_run = completed_cutover_candidate_review_run("run-candidate-summary-approve");
        insert_cutover_candidate_review_run(&state, &approve_run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-summary-approve".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let rework_run = completed_cutover_candidate_review_run("run-candidate-summary-rework");
        insert_cutover_candidate_review_run(&state, &rework_run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-summary-rework".into(),
                decision_kind: "request_rework".into(),
                optional_reviewer_note: Some("Needs rework.".into()),
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let summary = get_controlled_chat_cutover_candidate_review_summary_with_state(&state)
            .await
            .unwrap();

        assert_eq!(summary.approved_count, 1);
        assert_eq!(summary.rework_reject_count, 1);
        assert_eq!(
            summary
                .latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str()),
            Some("request_rework")
        );
        assert_eq!(
            summary
                .latest_decision
                .as_ref()
                .map(|decision| decision.candidate_run_id.as_str()),
            Some("run-candidate-summary-rework")
        );
        assert_eq!(
            summary
                .latest_decision
                .as_ref()
                .map(|decision| decision.contract_shape.as_str()),
            Some("send_message_compatible")
        );
        assert!(summary.latest_timestamp.is_some());
        assert!(summary.blocking_reasons.is_empty());

        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn cutover_candidate_promotion_readiness_blocks_without_w32_review_evidence() {
        let state = preview_state().await;
        seed_cutover_candidate_promotion_w30_ready(&state).await;

        let report = check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
            ControlledChatCutoverCandidatePromotionReadinessInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.ready);
        assert!(report.cutover_readiness_eligible);
        assert_eq!(report.required_approved_candidates, 1);
        assert_eq!(report.approved_candidate_count, 0);
        assert!(report.latest_decision.is_none());
        assert!(report.approved_candidates.is_empty());
        assert!(report.default_chat_unchanged);
        assert!(report
            .blocking_reasons
            .contains(&"metadata_safe_candidate_approve_evidence_missing".to_string()));
    }

    #[tokio::test]
    async fn cutover_candidate_promotion_readiness_blocks_when_latest_decision_is_reject_or_request_rework(
    ) {
        for decision_kind in ["reject", "request_rework"] {
            let state = preview_state().await;
            seed_cutover_candidate_promotion_w30_ready(&state).await;
            let approve_run = completed_cutover_candidate_review_run(&format!(
                "run-candidate-promotion-approve-{decision_kind}"
            ));
            insert_cutover_candidate_review_run(&state, &approve_run).await;
            record_controlled_chat_cutover_candidate_review_decision_with_state(
                ControlledChatCutoverCandidateReviewDecisionInput {
                    candidate_run_id: approve_run.id.clone(),
                    decision_kind: "approve".into(),
                    optional_reviewer_note: None,
                },
                &state,
            )
            .await
            .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            let latest_run = completed_cutover_candidate_review_run(&format!(
                "run-candidate-promotion-{decision_kind}"
            ));
            insert_cutover_candidate_review_run(&state, &latest_run).await;
            record_controlled_chat_cutover_candidate_review_decision_with_state(
                ControlledChatCutoverCandidateReviewDecisionInput {
                    candidate_run_id: latest_run.id.clone(),
                    decision_kind: decision_kind.into(),
                    optional_reviewer_note: None,
                },
                &state,
            )
            .await
            .unwrap();

            let report = check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
                ControlledChatCutoverCandidatePromotionReadinessInput {
                    required_approved_candidates: Some(1),
                    required_promotions: Some(3),
                    session_id: Some("session-1".into()),
                },
                &state,
            )
            .await
            .unwrap();

            assert!(!report.ready);
            assert_eq!(
                report
                    .latest_decision
                    .as_ref()
                    .map(|decision| decision.decision_kind.as_str()),
                Some(decision_kind)
            );
            assert!(report.blocking_reasons.contains(&format!(
                "latest_candidate_review_decision_is_{decision_kind}"
            )));
        }
    }

    #[tokio::test]
    async fn cutover_candidate_promotion_readiness_blocks_when_approved_candidate_run_missing() {
        let state = preview_state().await;
        seed_cutover_candidate_promotion_w30_ready(&state).await;
        seed_cutover_candidate_review_decision_evidence(
            &state,
            "run-candidate-promotion-missing",
            "approve",
            "sha256:seeded-candidate-summary",
        )
        .await;

        let report = check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
            ControlledChatCutoverCandidatePromotionReadinessInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.ready);
        assert_eq!(report.approved_candidate_count, 1);
        assert_eq!(report.approved_candidates.len(), 1);
        assert_eq!(
            report.approved_candidates[0].candidate_run_id,
            "run-candidate-promotion-missing"
        );
        assert!(!report.approved_candidates[0].ready);
        assert!(report
            .blocking_reasons
            .contains(&"candidate_run_missing".to_string()));
    }

    #[tokio::test]
    async fn cutover_candidate_promotion_readiness_blocks_when_approved_candidate_run_drifted() {
        let state = preview_state().await;
        seed_cutover_candidate_promotion_w30_ready(&state).await;
        let mut run = completed_cutover_candidate_review_run("run-candidate-promotion-drifted");
        insert_cutover_candidate_review_run(&state, &run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-promotion-drifted".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let audit = run
            .reasoning_trace
            .as_mut()
            .and_then(|trace| trace.strategy_result.as_mut())
            .and_then(Value::as_object_mut)
            .unwrap();
        audit.insert("metadataSafe".into(), json!(false));
        audit.insert("candidateReady".into(), json!(false));
        audit
            .get_mut("runtimeLimits")
            .and_then(Value::as_object_mut)
            .unwrap()
            .insert("maxToolCalls".into(), json!(1));
        run.tool_call_count = 1;
        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.update_run(&run).unwrap();
        }

        let report = check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
            ControlledChatCutoverCandidatePromotionReadinessInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.ready);
        assert!(report
            .blocking_reasons
            .contains(&"candidate_run_metadata_not_safe".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"candidate_run_max_tool_calls_not_zero".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"candidate_run_external_write_side_effects_present".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"candidate_run_not_ready_for_approval".to_string()));
    }

    #[tokio::test]
    async fn cutover_candidate_promotion_readiness_ready_with_valid_approved_candidate_evidence() {
        let state = preview_state().await;
        seed_cutover_candidate_promotion_w30_ready(&state).await;
        let run = completed_cutover_candidate_review_run("run-candidate-promotion-ready");
        insert_cutover_candidate_review_run(&state, &run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-promotion-ready".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: Some("Ready for implementation discussion.".into()),
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
            ControlledChatCutoverCandidatePromotionReadinessInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.ready);
        assert!(report.cutover_readiness_eligible);
        assert_eq!(report.required_approved_candidates, 1);
        assert_eq!(report.approved_candidate_count, 1);
        assert_eq!(
            report
                .latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str()),
            Some("approve")
        );
        assert_eq!(report.approved_candidates.len(), 1);
        assert!(report.approved_candidates[0].ready);
        assert_eq!(
            report.approved_candidates[0].candidate_run_id,
            "run-candidate-promotion-ready"
        );
        assert_eq!(
            report.approved_candidates[0].contract_shape,
            "send_message_compatible"
        );
        assert!(report.blocking_reasons.is_empty());
        assert!(report
            .checked_at
            .parse::<chrono::DateTime<chrono::Utc>>()
            .is_ok());
        assert_eq!(report.metadata_safe_summary["metadataSafe"], true);
        assert_eq!(report.metadata_safe_summary["readOnly"], true);
        assert_eq!(report.metadata_safe_summary["notAutomaticMigration"], true);

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("Ready for implementation discussion."));
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("raw output"));
        assert!(!serialized.contains("toolPayload"));
    }

    #[tokio::test]
    async fn cutover_candidate_promotion_readiness_command_is_read_only_by_side_effect_counts() {
        let state = preview_state().await;
        seed_cutover_candidate_promotion_w30_ready(&state).await;
        let run = completed_cutover_candidate_review_run("run-candidate-promotion-read-only");
        insert_cutover_candidate_review_run(&state, &run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-promotion-read-only".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let report = check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
            ControlledChatCutoverCandidatePromotionReadinessInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.ready);
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_runtime_boundary_status_reports_legacy_stream_and_metadata_safe() {
        let state = preview_state().await;

        let report = get_default_chat_runtime_boundary_status_with_state(&state)
            .await
            .unwrap();

        assert_eq!(report.current_mode, "legacy_stream");
        assert!(!report.controlled_candidate_available);
        assert!(report.default_chat_unchanged);
        assert!(report.candidate_promotion_readiness_required);
        assert!(!report.automatic_migration_enabled);
        assert!(report.blocking_reasons.is_empty());
        assert_eq!(
            report.metadata_safe_summary["runtimeBoundary"],
            "default_chat"
        );
        assert_eq!(report.metadata_safe_summary["metadataSafe"], true);
        assert_eq!(report.metadata_safe_summary["readOnly"], true);
        assert_eq!(report.metadata_safe_summary["currentMode"], "legacy_stream");
        assert_eq!(
            report.metadata_safe_summary["automaticMigrationEnabled"],
            false
        );

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("raw output"));
        assert!(!serialized.contains("toolPayload"));
    }

    #[tokio::test]
    async fn default_chat_runtime_boundary_status_is_read_only_by_side_effect_counts() {
        let state = preview_state().await;
        let before = side_effect_counts(&state).await;

        let report = get_default_chat_runtime_boundary_status_with_state(&state)
            .await
            .unwrap();

        assert_eq!(report.current_mode, "legacy_stream");
        assert!(!report.automatic_migration_enabled);
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    async fn seed_ready_default_chat_adapter_activation_plan(
        state: &Arc<crate::AppState>,
        candidate_run_id: &str,
    ) {
        seed_cutover_candidate_promotion_w30_ready(state).await;
        let run = completed_cutover_candidate_review_run(candidate_run_id);
        insert_cutover_candidate_review_run(state, &run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: candidate_run_id.into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            state,
        )
        .await
        .unwrap();
    }

    async fn default_chat_adapter_activation_review_evidence_records(
        state: &Arc<crate::AppState>,
    ) -> Vec<openlife_core::agent::EvidenceRecord> {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    DEFAULT_CHAT_ADAPTER_ACTIVATION_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .unwrap()
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_plan_blocks_when_w33_not_ready_without_sections() {
        let state = preview_state().await;

        let draft = draft_default_chat_adapter_activation_plan_with_state(
            DefaultChatAdapterActivationPlanDraftInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!draft.draft_ready);
        assert!(!draft.candidate_promotion_readiness_report.ready);
        assert_eq!(draft.runtime_boundary_status.current_mode, "legacy_stream");
        assert!(draft.activation_scope.is_empty());
        assert!(draft.required_preconditions.is_empty());
        assert!(draft.adapter_contract_checks.is_empty());
        assert!(draft.fallback_plan.is_empty());
        assert!(draft.rollback_plan.is_empty());
        assert!(draft.observability_plan.is_empty());
        assert!(draft.test_plan.is_empty());
        assert!(draft.manual_review_required);
        assert!(draft.not_automatic_migration);
        assert!(draft.requires_separate_implementation);
        assert!(draft
            .blocking_reasons
            .contains(&"candidate_promotion_readiness_not_ready".to_string()));
        assert!(draft
            .blocking_reasons
            .contains(&"candidate_review_decision_missing".to_string()));
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_plan_blocks_when_w34_boundary_is_not_legacy_or_automatic_disabled(
    ) {
        let state = preview_state().await;
        seed_cutover_candidate_promotion_w30_ready(&state).await;
        let run = completed_cutover_candidate_review_run("run-candidate-activation-boundary");
        insert_cutover_candidate_review_run(&state, &run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-activation-boundary".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let readiness = check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
            ControlledChatCutoverCandidatePromotionReadinessInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();
        assert!(readiness.ready);
        let mut boundary = get_default_chat_runtime_boundary_status_with_state(&state)
            .await
            .unwrap();
        boundary.current_mode = "controlled_adapter".into();
        boundary.automatic_migration_enabled = true;

        let draft = draft_default_chat_adapter_activation_plan_from_reports(readiness, boundary);

        assert!(!draft.draft_ready);
        assert!(draft.activation_scope.is_empty());
        assert!(draft.required_preconditions.is_empty());
        assert!(draft.adapter_contract_checks.is_empty());
        assert!(draft.fallback_plan.is_empty());
        assert!(draft.rollback_plan.is_empty());
        assert!(draft.observability_plan.is_empty());
        assert!(draft.test_plan.is_empty());
        assert!(draft
            .blocking_reasons
            .contains(&"default_chat_runtime_boundary_not_legacy_stream".to_string()));
        assert!(draft
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_plan_ready_with_complete_human_review_plan() {
        let state = preview_state().await;
        seed_cutover_candidate_promotion_w30_ready(&state).await;
        let run = completed_cutover_candidate_review_run("run-candidate-activation-ready");
        insert_cutover_candidate_review_run(&state, &run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-activation-ready".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: Some("Ready for activation planning review.".into()),
            },
            &state,
        )
        .await
        .unwrap();

        let draft = draft_default_chat_adapter_activation_plan_with_state(
            DefaultChatAdapterActivationPlanDraftInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(draft.draft_ready);
        assert!(draft.candidate_promotion_readiness_report.ready);
        assert_eq!(draft.runtime_boundary_status.current_mode, "legacy_stream");
        assert!(!draft.runtime_boundary_status.automatic_migration_enabled);
        assert!(draft.manual_review_required);
        assert!(draft.not_automatic_migration);
        assert!(draft.requires_separate_implementation);
        assert!(draft.blocking_reasons.is_empty());
        assert!(!draft.activation_scope.is_empty());
        assert!(!draft.required_preconditions.is_empty());
        assert!(!draft.adapter_contract_checks.is_empty());
        assert!(!draft.fallback_plan.is_empty());
        assert!(!draft.rollback_plan.is_empty());
        assert!(!draft.observability_plan.is_empty());
        assert!(!draft.test_plan.is_empty());
        assert!(draft
            .activation_scope
            .iter()
            .any(|item| item.contains("human-review-only")));
        assert!(draft
            .required_preconditions
            .iter()
            .any(|item| item.contains("W33 candidate promotion readiness")));
        assert!(draft
            .adapter_contract_checks
            .iter()
            .any(|item| item.contains("send_message-compatible")));
        assert!(draft
            .fallback_plan
            .iter()
            .any(|item| item.contains("legacy stream")));
        assert!(draft
            .rollback_plan
            .iter()
            .any(|item| item.contains("separate adapter implementation")));
        assert!(draft
            .observability_plan
            .iter()
            .any(|item| item.contains("metadata-safe")));
        assert!(draft
            .test_plan
            .iter()
            .any(|item| item.contains("send_message and start_stream_message")));
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_plan_command_is_read_only_by_side_effect_counts() {
        let state = preview_state().await;
        seed_cutover_candidate_promotion_w30_ready(&state).await;
        let run = completed_cutover_candidate_review_run("run-candidate-activation-read-only");
        insert_cutover_candidate_review_run(&state, &run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-activation-read-only".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let draft = draft_default_chat_adapter_activation_plan_with_state(
            DefaultChatAdapterActivationPlanDraftInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(draft.draft_ready);
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_plan_serialized_output_is_metadata_safe() {
        let state = preview_state().await;
        seed_cutover_candidate_promotion_w30_ready(&state).await;
        let run = completed_cutover_candidate_review_run("run-candidate-activation-safe");
        insert_cutover_candidate_review_run(&state, &run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-activation-safe".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: Some(
                    "Human note with secret@example.com must remain checksum-only.".into(),
                ),
            },
            &state,
        )
        .await
        .unwrap();

        let draft = draft_default_chat_adapter_activation_plan_with_state(
            DefaultChatAdapterActivationPlanDraftInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        let serialized = serde_json::to_string(&draft).unwrap();
        assert!(!serialized.contains("secret@example.com"));
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("rawPrompt"));
        assert!(!serialized.contains("raw output"));
        assert!(!serialized.contains("rawOutput"));
        assert!(!serialized.contains("raw assistant output"));
        assert!(!serialized.contains("rawAssistantResponse"));
        assert!(!serialized.contains("tool payload"));
        assert!(!serialized.contains("toolPayload"));
        assert!(!serialized.contains("userOutput"));
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_review_blocks_approve_when_draft_not_ready_without_evidence(
    ) {
        let state = preview_state().await;
        let before = side_effect_counts(&state).await;

        let result = record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some("Approve should not store this blocked note.".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!result.recorded);
        assert!(result.evidence_id.is_none());
        assert!(!result.draft_ready);
        assert!(result
            .blocking_reasons
            .contains(&"activation_plan_draft_not_ready_for_approval".to_string()));
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_review_records_ready_decisions_as_metadata_safe_evidence(
    ) {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-activation-review-ready",
        )
        .await;

        for decision_kind in ["approve", "reject", "request_rework"] {
            let result = record_default_chat_adapter_activation_review_decision_with_state(
                DefaultChatAdapterActivationReviewDecisionInput {
                    decision_kind: decision_kind.into(),
                    required_approved_candidates: Some(1),
                    required_promotions: Some(3),
                    session_id: Some("session-1".into()),
                    optional_reviewer_note: Some("Reviewed activation plan manually.".into()),
                },
                &state,
            )
            .await
            .unwrap();
            assert!(result.recorded);
            assert!(result.evidence_id.is_some());
            assert!(result.draft_ready);
            assert!(result.activation_plan_digest.starts_with("sha256:"));
            assert!(result.blocking_reasons.is_empty());
        }

        let records = default_chat_adapter_activation_review_evidence_records(&state).await;
        assert_eq!(records.len(), 3);
        for record in &records {
            assert!(
                default_chat_adapter_activation_review_decision_evidence_is_metadata_safe(record)
            );
            assert_eq!(
                record.run_metadata["evidenceKind"],
                "default_chat_adapter_activation_review_decision"
            );
            assert_eq!(record.run_metadata["draftReady"], true);
            assert_eq!(record.run_metadata["candidatePromotionReady"], true);
            assert_eq!(record.run_metadata["currentMode"], "legacy_stream");
            assert_eq!(record.run_metadata["automaticMigrationEnabled"], false);
            assert_eq!(record.run_metadata["reviewerNoteCategory"], "brief");
            assert!(record.run_metadata["reviewerNoteChecksum"]
                .as_str()
                .unwrap()
                .starts_with("sha256:"));
            assert_eq!(record.run_metadata.as_object().unwrap().len(), 11);
        }
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_review_summary_is_read_only() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-activation-review-summary",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "request_rework".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some("Needs one more implementation note.".into()),
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let summary = get_default_chat_adapter_activation_review_summary_with_state(&state)
            .await
            .unwrap();

        assert_eq!(summary.approved_count, 1);
        assert_eq!(summary.reject_or_rework_count, 1);
        assert_eq!(
            summary
                .latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str()),
            Some("request_rework")
        );
        assert!(summary.latest_timestamp.is_some());
        assert!(summary.blocking_reasons.is_empty());
        assert_eq!(summary.metadata_safe_summary["readOnly"], true);
        assert_eq!(
            summary.metadata_safe_summary["evidenceStorage"],
            "read_only"
        );
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_review_does_not_store_raw_note_or_outputs() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-activation-review-safe",
        )
        .await;

        let result = record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some(
                    "Reviewer raw note with secret@example.com and candidate output text.".into(),
                ),
            },
            &state,
        )
        .await
        .unwrap();
        let summary = get_default_chat_adapter_activation_review_summary_with_state(&state)
            .await
            .unwrap();
        let records = default_chat_adapter_activation_review_evidence_records(&state).await;

        let serialized = serde_json::to_string(&(result, summary, records)).unwrap();
        assert!(!serialized.contains("secret@example.com"));
        assert!(!serialized.contains("Reviewer raw note"));
        assert!(!serialized.contains("candidate output text"));
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("rawPrompt"));
        assert!(!serialized.contains("raw assistant"));
        assert!(!serialized.contains("rawAssistantOutput"));
        assert!(!serialized.contains("toolPayload"));
        assert!(!serialized.contains("userOutput"));
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_implementation_gate_blocks_without_activation_review()
    {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-activation-implementation-no-review",
        )
        .await;

        let report = check_default_chat_adapter_activation_implementation_gate_with_state(
            DefaultChatAdapterActivationImplementationGateInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.implementation_gate_eligible);
        assert!(report.draft_ready);
        assert!(report.latest_decision.is_none());
        assert!(!report.activation_plan_digest_matched);
        assert!(report
            .blocking_reasons
            .contains(&"activation_review_decision_missing".to_string()));
        assert_eq!(report.metadata_safe_summary["readOnly"], true);
        assert_eq!(report.metadata_safe_summary["notAutomaticMigration"], true);
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_implementation_gate_blocks_latest_reject_or_rework() {
        for decision_kind in ["reject", "request_rework"] {
            let state = preview_state().await;
            seed_ready_default_chat_adapter_activation_plan(
                &state,
                &format!("run-candidate-activation-implementation-{decision_kind}"),
            )
            .await;
            record_default_chat_adapter_activation_review_decision_with_state(
                DefaultChatAdapterActivationReviewDecisionInput {
                    decision_kind: decision_kind.into(),
                    required_approved_candidates: Some(1),
                    required_promotions: Some(3),
                    session_id: Some("session-1".into()),
                    optional_reviewer_note: Some("Human review did not approve.".into()),
                },
                &state,
            )
            .await
            .unwrap();

            let report = check_default_chat_adapter_activation_implementation_gate_with_state(
                DefaultChatAdapterActivationImplementationGateInput {
                    required_approved_candidates: Some(1),
                    required_promotions: Some(3),
                    session_id: Some("session-1".into()),
                },
                &state,
            )
            .await
            .unwrap();

            assert!(!report.implementation_gate_eligible);
            assert_eq!(
                report
                    .latest_decision
                    .as_ref()
                    .map(|decision| decision.decision_kind.as_str()),
                Some(decision_kind)
            );
            assert!(report.blocking_reasons.contains(&format!(
                "latest_activation_review_decision_is_{decision_kind}"
            )));
        }
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_implementation_gate_blocks_when_current_draft_not_ready(
    ) {
        let state = preview_state().await;

        let report = check_default_chat_adapter_activation_implementation_gate_with_state(
            DefaultChatAdapterActivationImplementationGateInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.implementation_gate_eligible);
        assert!(!report.draft_ready);
        assert!(report
            .blocking_reasons
            .contains(&"activation_plan_draft_not_ready".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"candidate_promotion_readiness_not_ready".to_string()));
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_implementation_gate_blocks_digest_mismatch() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-activation-implementation-original",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let second_run =
            completed_cutover_candidate_review_run("run-candidate-activation-implementation-new");
        insert_cutover_candidate_review_run(&state, &second_run).await;
        record_controlled_chat_cutover_candidate_review_decision_with_state(
            ControlledChatCutoverCandidateReviewDecisionInput {
                candidate_run_id: "run-candidate-activation-implementation-new".into(),
                decision_kind: "approve".into(),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_default_chat_adapter_activation_implementation_gate_with_state(
            DefaultChatAdapterActivationImplementationGateInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.implementation_gate_eligible);
        assert!(report.draft_ready);
        assert!(!report.activation_plan_digest_matched);
        assert!(report
            .blocking_reasons
            .contains(&"activation_plan_digest_mismatch".to_string()));
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_implementation_gate_eligible_with_latest_approve_and_matching_digest(
    ) {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-activation-implementation-ready",
        )
        .await;
        let review = record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some("Approved for implementation gate.".into()),
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_default_chat_adapter_activation_implementation_gate_with_state(
            DefaultChatAdapterActivationImplementationGateInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.implementation_gate_eligible);
        assert!(report.draft_ready);
        assert_eq!(
            report.current_activation_plan_digest,
            review.activation_plan_digest
        );
        assert!(report.activation_plan_digest_matched);
        assert!(report.default_chat_unchanged);
        assert_eq!(report.current_mode, "legacy_stream");
        assert!(!report.automatic_migration_enabled);
        assert!(report.blocking_reasons.is_empty());
        assert_eq!(
            report
                .latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str()),
            Some("approve")
        );
        assert_eq!(
            report.metadata_safe_summary["implementationGateEligible"],
            true
        );
        assert_eq!(
            report.metadata_safe_summary["requiresSeparateImplementation"],
            true
        );
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_implementation_gate_is_read_only_by_side_effect_counts(
    ) {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-activation-implementation-read-only",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let report = check_default_chat_adapter_activation_implementation_gate_with_state(
            DefaultChatAdapterActivationImplementationGateInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.implementation_gate_eligible);
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_adapter_activation_implementation_gate_serialized_output_is_metadata_safe(
    ) {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-activation-implementation-safe",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some(
                    "Implementation gate note with secret@example.com and raw candidate output."
                        .into(),
                ),
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_default_chat_adapter_activation_implementation_gate_with_state(
            DefaultChatAdapterActivationImplementationGateInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("secret@example.com"));
        assert!(!serialized.contains("Implementation gate note"));
        assert!(!serialized.contains("raw candidate output"));
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("rawPrompt"));
        assert!(!serialized.contains("raw assistant"));
        assert!(!serialized.contains("rawAssistantOutput"));
        assert!(!serialized.contains("candidate output"));
        assert!(!serialized.contains("toolPayload"));
        assert!(!serialized.contains("userOutput"));
    }

    #[tokio::test]
    async fn default_chat_adapter_routing_status_blocks_when_activation_gate_blocked() {
        let state = preview_state().await;

        let status = get_default_chat_adapter_routing_status_with_state(
            DefaultChatAdapterRoutingStatusInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert_eq!(status.current_mode, "legacy_stream");
        assert!(status.adapter_scaffold_present);
        assert!(!status.controlled_adapter_enabled);
        assert_eq!(status.default_send_path, "legacy_stream");
        assert_eq!(status.start_stream_path, "legacy_stream");
        assert!(!status.activation_implementation_gate_eligible);
        assert!(status.requires_separate_cutover_implementation);
        assert!(status
            .blocking_reasons
            .contains(&"activation_implementation_gate_not_eligible".to_string()));
        assert_eq!(status.metadata_safe_summary["routingMode"], "legacy_stream");
        assert_eq!(
            status.metadata_safe_summary["controlledAdapterEnabled"],
            false
        );
    }

    #[tokio::test]
    async fn default_chat_adapter_routing_status_keeps_legacy_stream_even_when_activation_gate_eligible(
    ) {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-routing-status-ready",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let status = get_default_chat_adapter_routing_status_with_state(
            DefaultChatAdapterRoutingStatusInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert_eq!(status.current_mode, "legacy_stream");
        assert!(status.adapter_scaffold_present);
        assert!(!status.controlled_adapter_enabled);
        assert_eq!(status.default_send_path, "legacy_stream");
        assert_eq!(status.start_stream_path, "legacy_stream");
        assert!(status.activation_implementation_gate_eligible);
        assert!(status.requires_separate_cutover_implementation);
        assert!(status.blocking_reasons.is_empty());
        assert_eq!(status.metadata_safe_summary["readOnly"], true);
        assert_eq!(status.metadata_safe_summary["notAutomaticMigration"], true);
        assert_eq!(
            status.metadata_safe_summary["requiresSeparateCutoverImplementation"],
            true
        );
    }

    #[tokio::test]
    async fn default_chat_adapter_routing_status_is_read_only_by_side_effect_counts() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-routing-status-read-only",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let status = get_default_chat_adapter_routing_status_with_state(
            DefaultChatAdapterRoutingStatusInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(status.activation_implementation_gate_eligible);
        assert!(!status.controlled_adapter_enabled);
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_adapter_routing_status_serialized_output_is_metadata_safe() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-routing-status-safe",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some(
                    "Routing status note with secret@example.com and raw output.".into(),
                ),
            },
            &state,
        )
        .await
        .unwrap();

        let status = get_default_chat_adapter_routing_status_with_state(
            DefaultChatAdapterRoutingStatusInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        let serialized = serde_json::to_string(&status).unwrap();
        assert!(!serialized.contains("secret@example.com"));
        assert!(!serialized.contains("Routing status note"));
        assert!(!serialized.contains("raw output"));
        assert!(!serialized.contains("rawPrompt"));
        assert!(!serialized.contains("rawAssistantOutput"));
        assert!(!serialized.contains("toolPayload"));
        assert!(!serialized.contains("userOutput"));
    }

    #[tokio::test]
    async fn default_chat_adapter_contract_harness_blocks_when_routing_gate_blocked() {
        let state = preview_state().await;

        let report = check_default_chat_adapter_contract_harness_with_state(
            DefaultChatAdapterContractHarnessInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.contract_harness_ready);
        assert_eq!(
            report.contract_shape,
            "disabled_adapter_legacy_stream_contract"
        );
        assert!(report.adapter_disabled);
        assert!(!report.activation_implementation_gate_eligible);
        assert_eq!(report.routing_status.current_mode, "legacy_stream");
        assert!(report
            .blocking_reasons
            .contains(&"activation_implementation_gate_not_eligible".to_string()));
        assert_eq!(
            report.metadata_safe_summary["contractHarness"],
            "default_chat_adapter"
        );
        assert_eq!(report.metadata_safe_summary["readOnly"], true);
    }

    #[tokio::test]
    async fn default_chat_adapter_contract_harness_ready_when_routing_is_eligible_and_disabled() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-contract-harness-ready",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_default_chat_adapter_contract_harness_with_state(
            DefaultChatAdapterContractHarnessInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.contract_harness_ready);
        assert!(report.adapter_disabled);
        assert!(report.activation_implementation_gate_eligible);
        assert!(report.blocking_reasons.is_empty());
        assert_eq!(report.send_message_contract.actual_path, "legacy_stream");
        assert_eq!(report.stream_message_contract.actual_path, "legacy_stream");
        assert!(report.send_message_contract.ready);
        assert!(report.stream_message_contract.ready);
        assert_eq!(report.metadata_safe_summary["adapterDisabled"], true);
        assert_eq!(
            report.metadata_safe_summary["contractShape"],
            "disabled_adapter_legacy_stream_contract"
        );
    }

    #[tokio::test]
    async fn default_chat_adapter_contract_harness_is_read_only_by_side_effect_counts() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-contract-harness-read-only",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let report = check_default_chat_adapter_contract_harness_with_state(
            DefaultChatAdapterContractHarnessInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.contract_harness_ready);
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_adapter_contract_harness_serialized_output_is_metadata_safe() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-contract-harness-safe",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some(
                    "Contract harness note with secret@example.com and raw output.".into(),
                ),
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_default_chat_adapter_contract_harness_with_state(
            DefaultChatAdapterContractHarnessInput {
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
            },
            &state,
        )
        .await
        .unwrap();

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("secret@example.com"));
        assert!(!serialized.contains("Contract harness note"));
        assert!(!serialized.contains("raw output"));
        assert!(!serialized.contains("rawPrompt"));
        assert!(!serialized.contains("rawAssistantOutput"));
        assert!(!serialized.contains("toolPayload"));
        assert!(!serialized.contains("userOutput"));
    }

    #[tokio::test]
    async fn default_chat_adapter_dry_run_blocks_when_contract_harness_blocked() {
        let state = preview_state().await;

        let report = run_default_chat_adapter_dry_run_with_state(
            DefaultChatAdapterDryRunInput {
                session_id: "session-1".into(),
                message: "Should not run dry run while harness is blocked.".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.dry_run_ready);
        assert_eq!(
            report.contract_shape,
            "default_chat_adapter_dry_run_contract"
        );
        assert_eq!(report.source_session_id, "session-1");
        assert_eq!(report.adapter_path, "blocked");
        assert!(!report.allow_writes);
        assert_eq!(report.max_tool_calls, 0);
        assert!(report.default_chat_path_unchanged);
        assert!(!report.chat_message_saved);
        assert!(!report.agent_run_recorded);
        assert!(!report.contract_harness_ready);
        assert!(report
            .blocking_reasons
            .contains(&"contract_harness_not_ready".to_string()));
        assert_eq!(
            report.metadata_safe_summary["adapterDryRun"],
            "default_chat_adapter"
        );
        assert_eq!(report.metadata_safe_summary["dryRunReady"], false);
    }

    #[tokio::test]
    async fn default_chat_adapter_dry_run_returns_contract_shaped_metadata_safe_result() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(&state, "run-candidate-dry-run-ready")
            .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let message = "Private dry-run prompt with secret@example.com".to_string();
        let report = run_default_chat_adapter_dry_run_with_state(
            DefaultChatAdapterDryRunInput {
                session_id: "session-1".into(),
                message: message.clone(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.dry_run_ready);
        assert_eq!(
            report.contract_shape,
            "default_chat_adapter_dry_run_contract"
        );
        assert_eq!(report.source_session_id, "session-1");
        assert_eq!(report.adapter_path, "controlled_adapter_dry_run");
        assert!(!report.allow_writes);
        assert_eq!(report.max_tool_calls, 0);
        assert!(report.default_chat_path_unchanged);
        assert!(!report.chat_message_saved);
        assert!(!report.agent_run_recorded);
        assert!(report.contract_harness_ready);
        assert_eq!(report.input_message_length, message.chars().count());
        assert_eq!(report.input_message_hash.len(), 64);
        assert!(report.user_output_preview.is_none());
        assert!(report.blocking_reasons.is_empty());
        assert_eq!(
            report.metadata_safe_summary["contractShape"],
            "default_chat_adapter_dry_run_contract"
        );
        assert_eq!(report.metadata_safe_summary["allowWrites"], false);
        assert_eq!(report.metadata_safe_summary["maxToolCalls"], 0);
        assert_eq!(
            report.metadata_safe_summary["defaultChatPathUnchanged"],
            true
        );
    }

    #[tokio::test]
    async fn default_chat_adapter_dry_run_has_no_side_effects() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-dry-run-no-side-effects",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let report = run_default_chat_adapter_dry_run_with_state(
            DefaultChatAdapterDryRunInput {
                session_id: "session-1".into(),
                message: "No persistence should happen from this dry run.".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.dry_run_ready);
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_adapter_dry_run_serialized_output_is_metadata_safe() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(&state, "run-candidate-dry-run-safe").await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: Some(
                    "Dry run review note with reviewer-secret@example.com.".into(),
                ),
            },
            &state,
        )
        .await
        .unwrap();

        let report = run_default_chat_adapter_dry_run_with_state(
            DefaultChatAdapterDryRunInput {
                session_id: "session-1".into(),
                message: "Dry run raw prompt with user-secret@example.com and tool payload.".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("user-secret@example.com"));
        assert!(!serialized.contains("reviewer-secret@example.com"));
        assert!(!serialized.contains("Dry run raw prompt"));
        assert!(!serialized.contains("tool payload"));
        assert!(!serialized.contains("rawPrompt"));
        assert!(!serialized.contains("rawAssistantOutput"));
        assert!(!serialized.contains("toolPayload"));
    }

    #[tokio::test]
    async fn default_chat_adapter_dry_run_review_blocks_approve_when_dry_run_not_ready() {
        let state = preview_state().await;
        let before = side_effect_counts(&state).await;

        let result = record_default_chat_adapter_dry_run_review_decision_with_state(
            DefaultChatAdapterDryRunReviewDecisionInput {
                decision_kind: "approve".into(),
                source_session_id: "session-1".into(),
                message: "Dry run review should be blocked.".into(),
                dry_run_summary_digest: None,
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                optional_reviewer_note: Some("Do not store this raw reviewer note.".into()),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!result.recorded);
        assert!(result.evidence_id.is_none());
        assert_eq!(result.decision_kind, "approve");
        assert!(!result.dry_run_ready);
        assert_eq!(
            result.contract_shape,
            "default_chat_adapter_dry_run_contract"
        );
        assert_eq!(result.dry_run_summary_digest.len(), 71);
        assert!(result
            .blocking_reasons
            .contains(&"dry_run_not_ready_for_approval".to_string()));
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_adapter_dry_run_review_records_ready_approve_metadata_safe_evidence() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-dry-run-review-approve",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let result = record_default_chat_adapter_dry_run_review_decision_with_state(
            DefaultChatAdapterDryRunReviewDecisionInput {
                decision_kind: "approve".into(),
                source_session_id: "session-1".into(),
                message: "Dry run review ready probe.".into(),
                dry_run_summary_digest: None,
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                optional_reviewer_note: Some(
                    "Approve note with private-reviewer@example.com".into(),
                ),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(result.recorded);
        assert!(result.evidence_id.is_some());
        assert_eq!(result.decision_kind, "approve");
        assert!(result.dry_run_ready);
        assert_eq!(
            result.contract_shape,
            "default_chat_adapter_dry_run_contract"
        );
        assert!(result.blocking_reasons.is_empty());
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count + 1, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);

        let summary = get_default_chat_adapter_dry_run_review_summary_with_state(&state)
            .await
            .unwrap();
        assert_eq!(summary.approved_count, 1);
        assert_eq!(summary.reject_or_rework_count, 0);
        let latest = summary.latest_decision.unwrap();
        assert_eq!(latest.decision_kind, "approve");
        assert_eq!(latest.source_session_id, "session-1");
        assert!(latest.dry_run_ready);
        assert_eq!(
            latest.contract_shape,
            "default_chat_adapter_dry_run_contract"
        );

        let serialized = {
            let store = state.evidence_store.lock().await;
            let records = store
                .query(EvidenceQuery {
                    affected_path: Some(
                        DEFAULT_CHAT_ADAPTER_DRY_RUN_REVIEW_DECISION_EVIDENCE_PATH.into(),
                    ),
                    evidence_type: Some(EvidenceType::RuntimeBehavior),
                    ..EvidenceQuery::default()
                })
                .unwrap();
            serde_json::to_string(&records).unwrap()
        };
        assert!(!serialized.contains("private-reviewer@example.com"));
        assert!(!serialized.contains("Approve note"));
        assert!(!serialized.contains("rawPrompt"));
        assert!(!serialized.contains("rawAssistantOutput"));
        assert!(!serialized.contains("toolPayload"));
        assert!(!serialized.contains("userOutput"));
    }

    #[tokio::test]
    async fn default_chat_adapter_dry_run_review_records_reject_and_rework_metadata_safe() {
        let state = preview_state().await;

        for decision_kind in ["reject", "request_rework"] {
            let result = record_default_chat_adapter_dry_run_review_decision_with_state(
                DefaultChatAdapterDryRunReviewDecisionInput {
                    decision_kind: decision_kind.into(),
                    source_session_id: "session-1".into(),
                    message: "Blocked dry run can be rejected or marked for rework.".into(),
                    dry_run_summary_digest: None,
                    required_approved_candidates: Some(1),
                    required_promotions: Some(3),
                    optional_reviewer_note: Some("private raw reviewer text".into()),
                },
                &state,
            )
            .await
            .unwrap();

            assert!(result.recorded);
            assert_eq!(result.decision_kind, decision_kind);
            assert!(!result.dry_run_ready);
            assert!(result
                .blocking_reasons
                .contains(&"contract_harness_not_ready".to_string()));
        }

        let summary = get_default_chat_adapter_dry_run_review_summary_with_state(&state)
            .await
            .unwrap();
        assert_eq!(summary.approved_count, 0);
        assert_eq!(summary.reject_or_rework_count, 2);
        assert_eq!(
            summary.latest_decision.unwrap().decision_kind,
            "request_rework"
        );
    }

    #[tokio::test]
    async fn default_chat_adapter_dry_run_review_blocks_digest_mismatch_without_evidence() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-dry-run-review-mismatch",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let result = record_default_chat_adapter_dry_run_review_decision_with_state(
            DefaultChatAdapterDryRunReviewDecisionInput {
                decision_kind: "approve".into(),
                source_session_id: "session-1".into(),
                message: "Dry run review digest mismatch probe.".into(),
                dry_run_summary_digest: Some("sha256:wrongdigest".into()),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!result.recorded);
        assert!(result
            .blocking_reasons
            .contains(&"dry_run_summary_digest_mismatch".to_string()));
        let after = side_effect_counts(&state).await;
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_adapter_dry_run_review_summary_is_read_only() {
        let state = preview_state().await;
        let before = side_effect_counts(&state).await;

        let summary = get_default_chat_adapter_dry_run_review_summary_with_state(&state)
            .await
            .unwrap();

        assert_eq!(summary.approved_count, 0);
        assert_eq!(summary.reject_or_rework_count, 0);
        assert!(summary.latest_decision.is_none());
        assert!(summary
            .blocking_reasons
            .contains(&"dry_run_review_decision_missing".to_string()));
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn default_chat_adapter_implementation_readiness_blocks_without_dry_run_review_approval()
    {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-implementation-readiness-missing-review",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_default_chat_adapter_implementation_readiness_with_state(
            DefaultChatAdapterImplementationReadinessInput {
                source_session_id: "session-1".into(),
                message: "Implementation readiness probe.".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.implementation_ready);
        assert!(report.activation_implementation_gate_eligible);
        assert!(report.contract_harness_ready);
        assert!(report.dry_run_ready);
        assert!(!report.dry_run_review_approved);
        assert!(report.default_chat_unchanged);
        assert!(!report.controlled_adapter_enabled);
        assert!(!report.automatic_migration_enabled);
        assert!(report
            .blocking_reasons
            .contains(&"dry_run_review_approval_missing".to_string()));
        assert_eq!(
            report.metadata_safe_summary["implementationReadiness"],
            "default_chat_adapter"
        );
    }

    #[tokio::test]
    async fn default_chat_adapter_implementation_readiness_blocks_latest_reject_or_rework() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-implementation-readiness-rework",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        record_default_chat_adapter_dry_run_review_decision_with_state(
            DefaultChatAdapterDryRunReviewDecisionInput {
                decision_kind: "request_rework".into(),
                source_session_id: "session-1".into(),
                message: "Implementation readiness probe.".into(),
                dry_run_summary_digest: None,
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_default_chat_adapter_implementation_readiness_with_state(
            DefaultChatAdapterImplementationReadinessInput {
                source_session_id: "session-1".into(),
                message: "Implementation readiness probe.".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.implementation_ready);
        assert_eq!(
            report
                .latest_dry_run_review_decision
                .as_ref()
                .unwrap()
                .decision_kind,
            "request_rework"
        );
        assert!(!report.dry_run_review_approved);
        assert!(report
            .blocking_reasons
            .contains(&"latest_dry_run_review_not_approve".to_string()));
    }

    #[tokio::test]
    async fn default_chat_adapter_implementation_readiness_blocks_digest_mismatch() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-implementation-readiness-digest",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        record_default_chat_adapter_dry_run_review_decision_with_state(
            DefaultChatAdapterDryRunReviewDecisionInput {
                decision_kind: "approve".into(),
                source_session_id: "session-1".into(),
                message: "Approved digest probe.".into(),
                dry_run_summary_digest: None,
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_default_chat_adapter_implementation_readiness_with_state(
            DefaultChatAdapterImplementationReadinessInput {
                source_session_id: "session-1".into(),
                message: "Different readiness probe.".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(!report.implementation_ready);
        assert!(report.dry_run_review_approved);
        assert!(!report.dry_run_digest_matched);
        assert!(report
            .blocking_reasons
            .contains(&"dry_run_review_digest_mismatch".to_string()));
    }

    #[tokio::test]
    async fn default_chat_adapter_implementation_readiness_ready_with_current_approved_review() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-implementation-readiness-ready",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        record_default_chat_adapter_dry_run_review_decision_with_state(
            DefaultChatAdapterDryRunReviewDecisionInput {
                decision_kind: "approve".into(),
                source_session_id: "session-1".into(),
                message: "Implementation readiness probe.".into(),
                dry_run_summary_digest: None,
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                optional_reviewer_note: Some("Do not store raw implementation note.".into()),
            },
            &state,
        )
        .await
        .unwrap();

        let report = check_default_chat_adapter_implementation_readiness_with_state(
            DefaultChatAdapterImplementationReadinessInput {
                source_session_id: "session-1".into(),
                message: "Implementation readiness probe.".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.implementation_ready);
        assert!(report.activation_implementation_gate_eligible);
        assert!(report.contract_harness_ready);
        assert!(report.dry_run_ready);
        assert!(report.dry_run_review_approved);
        assert!(report.dry_run_digest_matched);
        assert!(report.default_chat_unchanged);
        assert!(!report.controlled_adapter_enabled);
        assert!(!report.automatic_migration_enabled);
        assert!(report.blocking_reasons.is_empty());
        assert_eq!(
            report.latest_dry_run_review_decision.unwrap().decision_kind,
            "approve"
        );
        assert_eq!(report.metadata_safe_summary["implementationReady"], true);
    }

    #[tokio::test]
    async fn default_chat_adapter_implementation_readiness_is_read_only_by_side_effect_counts() {
        let state = preview_state().await;
        seed_ready_default_chat_adapter_activation_plan(
            &state,
            "run-candidate-implementation-readiness-read-only",
        )
        .await;
        record_default_chat_adapter_activation_review_decision_with_state(
            DefaultChatAdapterActivationReviewDecisionInput {
                decision_kind: "approve".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                session_id: Some("session-1".into()),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        record_default_chat_adapter_dry_run_review_decision_with_state(
            DefaultChatAdapterDryRunReviewDecisionInput {
                decision_kind: "approve".into(),
                source_session_id: "session-1".into(),
                message: "Implementation readiness probe.".into(),
                dry_run_summary_digest: None,
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
                optional_reviewer_note: None,
            },
            &state,
        )
        .await
        .unwrap();
        let before = side_effect_counts(&state).await;

        let report = check_default_chat_adapter_implementation_readiness_with_state(
            DefaultChatAdapterImplementationReadinessInput {
                source_session_id: "session-1".into(),
                message: "Implementation readiness probe.".into(),
                required_approved_candidates: Some(1),
                required_promotions: Some(3),
            },
            &state,
        )
        .await
        .unwrap();

        assert!(report.implementation_ready);
        let after = side_effect_counts(&state).await;
        assert_eq!(before.run_count, after.run_count);
        assert_eq!(before.pending_proposal_count, after.pending_proposal_count);
        assert_eq!(before.evidence_count, after.evidence_count);
        assert_eq!(before.patch_count, after.patch_count);
        assert_eq!(before.mcp_audit_count, after.mcp_audit_count);
        assert_eq!(before.model_version, after.model_version);
        assert_eq!(before.messages_json, after.messages_json);
    }

    #[tokio::test]
    async fn runtime_migration_gate_command_blocks_without_preview_audit() {
        let state = preview_state().await;

        let report = check_runtime_migration_gate_with_state(
            RuntimeMigrationGateCheckInput::default(),
            &state,
        )
        .await
        .unwrap();

        assert!(report.default_chat_unchanged);
        assert!(!report.preview_path_healthy);
        assert!(!report.metadata_safe_trace_ready);
        assert!(report
            .blocking_reasons
            .contains(&"preview_audit_missing".to_string()));
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_executes_react_path() {
        let state = preview_state().await;

        let output = run_multi_strategy_agent_preview_with_state(
            base_input("What should I focus on today?"),
            &state,
        )
        .await
        .unwrap();

        assert_eq!(output.strategy_kind, "react");
        assert_eq!(output.payload_kind, "react");
        assert!(output.run_id.is_some());
        assert!(output.user_output.is_some());
        assert!(output.proposal_ids.is_empty());
        assert_eq!(output.governance_decision_kind.as_deref(), Some("allow"));

        let run = stored_preview_run(&state, output.run_id.as_deref().unwrap()).await;
        assert_eq!(run.status, AgentRunStatus::Completed);
        assert_eq!(run.user_input, None);
        assert_eq!(
            run.reasoning_strategy.as_deref(),
            Some("multi_strategy_preview")
        );
        let audit = preview_audit(&run);
        assert_eq!(audit["strategyKind"], "react");
        assert_eq!(audit["payloadKind"], "react");
        assert_eq!(audit["blocked"], false);
        assert_eq!(audit["metadataSafe"], true);
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_returns_plan_execute_payload_for_planning_intent() {
        let state = preview_state().await;

        let output = run_multi_strategy_agent_preview_with_state(
            base_input("Plan steps for my afternoon."),
            &state,
        )
        .await
        .unwrap();

        assert_eq!(output.strategy_kind, "planExecute");
        assert_eq!(output.payload_kind, "planExecute");
        assert!(output.run_id.is_some());
        assert!(output.user_output.is_none());
        assert!(output.plan.is_some());
        assert!(output.proposal_ids.is_empty());

        let run = stored_preview_run(&state, output.run_id.as_deref().unwrap()).await;
        assert_eq!(run.status, AgentRunStatus::Completed);
        assert_eq!(run.user_input, None);
        let audit = preview_audit(&run);
        assert_eq!(audit["strategyKind"], "planExecute");
        assert_eq!(audit["payloadKind"], "planExecute");
        assert_eq!(audit["planStepCount"], 1);
        assert_eq!(audit["planStepStatuses"][0], "executed");
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_returns_blocked_for_sensitive_local_only_without_local_model(
    ) {
        let state = preview_state().await;
        let mut input = base_input("Talk through a sensitive health topic about medication.");
        input.local_model_available = false;

        let output = run_multi_strategy_agent_preview_with_state(input, &state)
            .await
            .unwrap();

        assert_eq!(output.payload_kind, "blocked");
        assert!(output.run_id.is_some());
        assert!(output.user_output.is_none());
        assert_eq!(output.governance_decision_kind.as_deref(), Some("block"));

        let run = stored_preview_run(&state, output.run_id.as_deref().unwrap()).await;
        assert_eq!(run.status, AgentRunStatus::Completed);
        assert_eq!(run.user_input, None);
        let audit = preview_audit(&run);
        assert_eq!(audit["payloadKind"], "blocked");
        assert_eq!(audit["governanceDecisionKind"], "block");
        assert_eq!(audit["blocked"], true);
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_does_not_treat_broad_tools_prompt_as_write_intent() {
        let state = preview_state().await;
        let mut input = base_input("What should I focus on today?");
        input.tools_prompt =
            "Available tools: file.write, calendar.create_event, email.send".into();

        let output = run_multi_strategy_agent_preview_with_state(input, &state)
            .await
            .unwrap();

        assert_eq!(output.strategy_kind, "react");
        assert_eq!(output.payload_kind, "react");
        assert!(output.proposal_ids.is_empty());
        assert!(!output
            .metadata_safe_summary
            .to_string()
            .contains("calendar.create_event"));

        let run = stored_preview_run(&state, output.run_id.as_deref().unwrap()).await;
        let persisted = serde_json::to_string(&run).unwrap();
        assert!(!persisted.contains("calendar.create_event"));
        assert!(!persisted.contains("email.send"));
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_output_is_metadata_safe() {
        let state = preview_state().await;
        let mut input =
            base_input("Plan steps for Alice and alice@example.com before sending the full draft.");
        input.tools_prompt = "Available tools: email.send body payload and file.update".into();

        let output = run_multi_strategy_agent_preview_with_state(input, &state)
            .await
            .unwrap();
        let serialized = serde_json::to_string(&output).unwrap();

        assert!(!serialized.contains("Alice"));
        assert!(!serialized.contains("alice@example.com"));
        assert!(!serialized.contains("full draft"));
        assert!(!serialized.contains("email.send"));
        assert!(!serialized.contains("file.update"));

        let run = stored_preview_run(&state, output.run_id.as_deref().unwrap()).await;
        let persisted = serde_json::to_string(&run).unwrap();
        assert!(!persisted.contains("Alice"));
        assert!(!persisted.contains("alice@example.com"));
        assert!(!persisted.contains("full draft"));
        assert!(!persisted.contains("email.send"));
        assert!(!persisted.contains("file.update"));
        assert_eq!(run.user_input, None);
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_persists_failed_run_with_sanitized_error() {
        let state = preview_state().await;
        let mut input = base_input("raw user text for Alice alice@example.com");
        input.layer = Some("not-a-layer".into());

        let err = run_multi_strategy_agent_preview_with_state(input, &state)
            .await
            .unwrap_err();

        assert!(!err.contains("Alice"));
        assert!(!err.contains("alice@example.com"));

        let runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs_for_session("session-preview", 10).unwrap()
        };
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.status, AgentRunStatus::Failed);
        assert_eq!(run.user_input, None);
        let persisted = serde_json::to_string(run).unwrap();
        assert!(!persisted.contains("Alice"));
        assert!(!persisted.contains("alice@example.com"));
        assert!(persisted.contains("preview_runtime_failed"));
    }

    #[tokio::test]
    async fn multi_strategy_preview_command_does_not_write_lifemodel_memory_or_proposals() {
        let state = preview_state().await;
        let proposal_store = ProposalStore::new_in_memory().unwrap();
        assert!(proposal_store
            .list_pending_proposals(10)
            .unwrap()
            .is_empty());

        let before_model = {
            let manager = state.life_model_manager.lock().await;
            manager.load().unwrap()
        };
        let before_messages = {
            let store = state.memory_store.lock().await;
            store.export_all_messages().unwrap()
        };

        let _ = run_multi_strategy_agent_preview_with_state(
            base_input("Create a reminder for tomorrow."),
            &state,
        )
        .await
        .unwrap();

        let after_model = {
            let manager = state.life_model_manager.lock().await;
            manager.load().unwrap()
        };
        let after_messages = {
            let store = state.memory_store.lock().await;
            store.export_all_messages().unwrap()
        };
        let pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();

        assert_eq!(before_model.metadata.version, after_model.metadata.version);
        assert_eq!(
            serde_json::to_string(&before_messages).unwrap(),
            serde_json::to_string(&after_messages).unwrap()
        );
        assert!(pending_proposals.is_empty());
    }
}
