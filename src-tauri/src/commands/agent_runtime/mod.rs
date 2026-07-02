use crate::main_chat_hs_runtime::build_chat_runtime_hs_packet;
use crate::AppState;
use openlife_core::agent::ReasoningTrace;
use openlife_core::agent::{
    behavior_checks_for_packet, AgentExecutionBudget, AgentRun, AgentRunError, AgentRunStatus,
    AgentRuntime, AgentTask, AgentTaskKind, ContextSummary, ControlledChatPilotEligibilityReport,
    EvidenceDraft, EvidencePrivacyLevel, EvidenceQuery, EvidenceSourceRef, EvidenceSourceType,
    EvidenceType, GovernanceDecisionKind, HSBehaviorCheckSummary, HSSelectionAudit,
    MultiStrategyRuntime, MultiStrategyRuntimeInput, MultiStrategyRuntimeMaturityReport,
    MultiStrategyRuntimeOutput, MultiStrategyRuntimePayload, PlanExecutionOutput, PlanStepStatus,
    ReactBetaExecutionReadinessReport, RedactionLevel, RiskLevel, RuntimeGuidanceConsumptionMode,
    RuntimeInput, RuntimeMigrationGateReport, RuntimeStrategyKind, RuntimeStrategyRegistry,
    RuntimeStrategySideEffectBudget, ToolRegistryBetaReadinessReport,
    DEFAULT_CONTROLLED_CHAT_PILOT_REQUIRED_CLEAN_RUNS,
};
use openlife_core::layer_router::Layer;
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::State;

mod migration_ladder;
mod plan_execute_product;

#[cfg(test)]
pub(crate) use migration_ladder::check_controlled_chat_cutover_candidate_promotion_readiness_with_state;
pub use migration_ladder::*;
#[cfg(test)]
pub(crate) use migration_ladder::{
    check_controlled_chat_cutover_readiness_with_state,
    check_controlled_chat_migration_implementation_gate_with_state,
    check_controlled_chat_pilot_eligibility_with_state,
    check_controlled_pilot_promotion_readiness_with_state, check_runtime_migration_gate_with_state,
    draft_controlled_chat_migration_plan_with_state,
    get_controlled_chat_cutover_candidate_review_summary_with_state,
    get_controlled_chat_migration_review_decision_summary_with_state,
    get_controlled_chat_migration_shadow_review_summary_with_state,
    get_controlled_pilot_promotion_evidence_summary_with_state,
    record_controlled_chat_cutover_candidate_review_decision_with_state,
    record_controlled_chat_migration_review_decision_with_state,
    record_controlled_chat_migration_shadow_review_decision_with_state,
    record_controlled_pilot_promotion_evidence_with_state,
    run_controlled_chat_cutover_candidate_with_state,
    run_controlled_chat_migration_shadow_run_with_state,
};
pub use plan_execute_product::*;

const CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH: &str = "runtime.controlled_pilot.promotion";
const CONTROLLED_PILOT_PROMOTION_BLOCK_PATH: &str = "runtime.controlled_pilot.promotion_block";
const CONTROLLED_CHAT_MIGRATION_REVIEW_DECISION_EVIDENCE_PATH: &str =
    "runtime.controlled_chat.migration_review_decision";
const CONTROLLED_CHAT_MIGRATION_SHADOW_REVIEW_DECISION_EVIDENCE_PATH: &str =
    "runtime.controlled_chat.migration_shadow_review_decision";
const CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH: &str =
    "runtime.controlled_chat.cutover_candidate_review_decision";
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactBetaExecutionStatusReport {
    pub report_kind: String,
    pub readiness: ReactBetaExecutionReadinessReport,
    pub tool_registry_readiness: ToolRegistryBetaReadinessReport,
    pub default_chat_unchanged: bool,
    pub migration_permission: bool,
    pub no_runtime_model_tool_execution: bool,
    pub no_business_writes: bool,
    pub status_command_side_effect_budget: RuntimeStrategySideEffectBudget,
    pub metadata_safe: bool,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentExecutionV1EvalGateReport {
    pub report_kind: String,
    pub runtime_eval: openlife_core::agent::main_chat_agent_v1::MainChatRuntimeEvalReport,
    pub acceptance:
        openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceReport,
    pub live_provider_preflight:
        openlife_core::agent::main_chat_agent_v1::MainChatLiveProviderEvalPreflightReport,
    pub command_surface_gate_executed: bool,
    pub live_provider_attempted: bool,
    pub migration_permission: bool,
    pub metadata_safe: bool,
    pub no_external_provider_invocation: bool,
    pub no_app_store_writes: bool,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentExecutionV1FinalAcceptanceGateCommandReport {
    pub report_kind: String,
    pub report_run_id: String,
    pub final_gate: crate::main_chat_final_gate::MainChatAgentExecutionV1FinalGateReport,
    pub(crate) command_surface_eval:
        crate::main_chat_command_surface_eval::MainChatCommandSurfaceEvalReport,
    pub live_provider_preflight:
        openlife_core::agent::main_chat_agent_v1::MainChatLiveProviderEvalPreflightReport,
    pub command_surface_gate_executed: bool,
    pub live_provider_attempted: bool,
    pub migration_permission: bool,
    pub metadata_safe: bool,
    pub no_external_provider_invocation: bool,
    pub no_app_store_writes: bool,
    pub metadata_safe_summary: Value,
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

fn sha256_metadata_checksum(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
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

pub(super) fn push_unique_string(values: &mut Vec<String>, value: String) {
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

#[tauri::command]
pub async fn get_runtime_strategy_registry_status(
    state: State<'_, Arc<AppState>>,
) -> Result<MultiStrategyRuntimeMaturityReport, String> {
    get_runtime_strategy_registry_status_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_runtime_strategy_registry_status_with_state(
    _state: &Arc<AppState>,
) -> Result<MultiStrategyRuntimeMaturityReport, String> {
    Ok(RuntimeStrategyRegistry::maturity_report())
}

#[tauri::command]
pub async fn get_react_beta_execution_status(
    state: State<'_, Arc<AppState>>,
) -> Result<ReactBetaExecutionStatusReport, String> {
    get_react_beta_execution_status_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_react_beta_execution_status_with_state(
    _state: &Arc<AppState>,
) -> Result<ReactBetaExecutionStatusReport, String> {
    let registry = openlife_core::mcp::McpRegistry::new();
    let tool_registry_readiness =
        openlife_core::agent::evaluate_tool_registry_beta_readiness(&registry);
    let readiness = openlife_core::agent::evaluate_react_beta_execution_readiness();
    let metadata_safe_summary = json!({
        "reportKind": "react_beta_execution_status",
        "readinessReady": readiness.ready,
        "toolRegistryReady": tool_registry_readiness.ready,
        "defaultChatUnchanged": true,
        "migrationPermission": false,
        "runtimeModelToolExecuted": false,
        "businessWrites": false,
        "metadataSafe": true,
    });

    Ok(ReactBetaExecutionStatusReport {
        report_kind: "react_beta_execution_status".into(),
        readiness,
        tool_registry_readiness,
        default_chat_unchanged: true,
        migration_permission: false,
        no_runtime_model_tool_execution: true,
        no_business_writes: true,
        status_command_side_effect_budget: RuntimeStrategySideEffectBudget::zero(),
        metadata_safe: true,
        metadata_safe_summary,
    })
}

#[tauri::command]
pub async fn run_main_chat_agent_execution_v1_eval_gate(
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatAgentExecutionV1EvalGateReport, String> {
    run_main_chat_agent_execution_v1_eval_gate_with_state(&state.inner().clone()).await
}

pub(crate) async fn run_main_chat_agent_execution_v1_eval_gate_with_state(
    state: &Arc<AppState>,
) -> Result<MainChatAgentExecutionV1EvalGateReport, String> {
    let runtime_eval =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let config = state.config.lock().await.clone();
    let scripted_provider_response_present = state
        .scheduler
        .lock()
        .await
        .scripted_generation_response
        .is_some();
    let live_provider_preflight =
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_live_provider_eval_preflight_from_config(
            &config,
            false,
            scripted_provider_response_present,
            false,
        );
    let acceptance =
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_agent_execution_v1_acceptance_gate(
            openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceInput {
                runtime_report: runtime_eval.clone(),
                command_surface:
                    openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence {
                        total_cases: 0,
                        legacy_fallback_count: 0,
                        silent_write_count: 0,
                        send_stream_matrix_coverage: 0.0,
                        kernel_backed_case_count: 0,
                        kernel_direct_answer_case_count: 0,
                        kernel_read_only_tool_case_count: 0,
                        kernel_proposal_write_case_count: 0,
                        kernel_plan_execute_case_count: 0,
                        kernel_blocker_case_count: 0,
                        kernel_hs_context_case_count: 0,
                        kernel_web_tool_case_count: 0,
                        kernel_mcp_tool_case_count: 0,
                        final_completion_ready: false,
                    },
                live_provider:
                    openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceLiveEvidence {
                        generation_eval_executed: false,
                        web_mcp_agent_loop_eval_executed: false,
                        web_agent_loop_eval_executed: false,
                        mcp_agent_loop_eval_executed: false,
                        proposal_permission_eval_executed: false,
                        no_silent_writes: true,
                    },
            },
        );
    let metadata_safe_summary = json!({
        "reportKind": "main_chat_agent_execution_v1_eval_gate",
        "runtimeTotalCases": runtime_eval.total_cases,
        "runtimeFailedCases": runtime_eval.failed_cases,
        "runtimeDeterministicStubCases": runtime_eval.deterministic_stub_case_count,
        "runtimeSilentWriteCount": runtime_eval.silent_write_count,
        "commandSurfaceGateExecuted": false,
        "liveProviderAttempted": false,
        "liveProviderPreflightReady": live_provider_preflight.ready,
        "liveProviderPreflightStatus": live_provider_preflight.status,
        "liveProviderPreflightProvider": live_provider_preflight.provider,
        "liveProviderPreflightBlockers": live_provider_preflight.blockers,
        "liveProviderPreflightRequiredEvidence": live_provider_preflight.required_evidence,
        "liveProviderPreflightInvocationAllowed": live_provider_preflight.live_provider_invocation_allowed,
        "liveProviderPreflightModelInvoked": live_provider_preflight.model_invoked,
        "liveProviderPreflightDirectWritesExecuted": live_provider_preflight.direct_writes_executed,
        "acceptanceReady": acceptance.ready,
        "acceptanceStatus": acceptance.status,
        "blockerCount": acceptance.blockers.len(),
        "metadataSafe": true,
    });

    Ok(MainChatAgentExecutionV1EvalGateReport {
        report_kind: "main_chat_agent_execution_v1_eval_gate".into(),
        runtime_eval,
        acceptance,
        live_provider_preflight,
        command_surface_gate_executed: false,
        live_provider_attempted: false,
        migration_permission: false,
        metadata_safe: true,
        no_external_provider_invocation: true,
        no_app_store_writes: true,
        metadata_safe_summary,
    })
}

#[tauri::command]
pub async fn run_main_chat_agent_productization_v1_gate(
    _state: State<'_, Arc<AppState>>,
) -> Result<
    crate::main_chat_agent_productization_eval::MainChatAgentProductizationV1GateReport,
    String,
> {
    Ok(
        crate::main_chat_agent_productization_eval::run_main_chat_agent_productization_v1_gate_report(
        ),
    )
}

#[tauri::command]
pub async fn run_main_chat_capability_eval_gate(
    _state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_capability_eval::MainChatCapabilityEvalReport, String> {
    Ok(crate::main_chat_capability_eval::run_main_chat_capability_eval_report().await)
}

#[tauri::command]
pub async fn run_main_chat_stage3_execution_ux_report(
    _state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_stage3_execution_ux::MainChatStage3ExecutionUxReport, String> {
    Ok(crate::main_chat_stage3_execution_ux::run_main_chat_stage3_execution_ux_report())
}

#[tauri::command]
pub async fn run_main_chat_external_live_productization_gate(
    state: State<'_, Arc<AppState>>,
) -> Result<
    crate::main_chat_live_productization_eval::MainChatExternalLiveProductizationGateReport,
    String,
> {
    crate::main_chat_live_productization_eval::run_main_chat_external_live_productization_gate_with_state(
        &state.inner().clone(),
        crate::main_chat_live_provider_harness::main_chat_live_provider_eval_opt_in_from_env(),
    )
    .await
}

#[tauri::command]
pub async fn run_main_chat_agent_product_maturity_v2_event_gate(
    _state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_event_stream::MainChatProductMaturityV2EventGateReport, String> {
    Ok(crate::main_chat_event_stream::run_main_chat_agent_product_maturity_v2_event_gate())
}

#[tauri::command]
pub async fn run_main_chat_agent_product_maturity_v2_plan_gate(
    _state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_plan_interaction_eval::MainChatProductMaturityV2PlanGateReport, String>
{
    Ok(
        crate::main_chat_plan_interaction_eval::run_main_chat_agent_product_maturity_v2_plan_gate()
            .await,
    )
}

#[tauri::command]
pub async fn list_main_chat_skills(
    session_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<crate::main_chat_skills_tools::MainChatSkillSummary>, String> {
    crate::main_chat_skills_tools::list_main_chat_skills_with_state(
        &state.inner().clone(),
        session_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn get_main_chat_skill_detail(
    skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_skills_tools::MainChatSkillDetail, String> {
    crate::main_chat_skills_tools::get_main_chat_skill_detail_with_state(
        &state.inner().clone(),
        &skill_id,
    )
    .await
}

#[tauri::command]
pub async fn select_main_chat_skill(
    session_id: String,
    skill_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_skills_tools::MainChatSelectedSkill, String> {
    crate::main_chat_skills_tools::select_main_chat_skill_with_state(
        &state.inner().clone(),
        &session_id,
        &skill_id,
    )
    .await
}

#[tauri::command]
pub async fn clear_main_chat_skill(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_skills_tools::MainChatSelectedSkill, String> {
    crate::main_chat_skills_tools::clear_main_chat_skill_with_state(
        &state.inner().clone(),
        &session_id,
    )
    .await
}

#[tauri::command]
pub async fn list_main_chat_tool_candidates(
    task_session_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_skills_tools::MainChatToolCandidateList, String> {
    crate::main_chat_skills_tools::list_main_chat_tool_candidates_with_state(
        &state.inner().clone(),
        task_session_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn run_main_chat_agent_product_maturity_v2_skills_gate(
    _state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_skills_tools::MainChatProductMaturityV2SkillsGateReport, String> {
    Ok(crate::main_chat_skills_tools::run_main_chat_agent_product_maturity_v2_skills_gate().await)
}

#[tauri::command]
pub async fn run_main_chat_agent_product_maturity_v2_final_readiness_gate(
    state: State<'_, Arc<AppState>>,
) -> Result<
    crate::main_chat_product_maturity_v2_final_readiness::MainChatProductMaturityV2FinalReadinessReport,
    String,
>{
    crate::main_chat_product_maturity_v2_final_readiness::run_main_chat_agent_product_maturity_v2_final_readiness_report(
        &state.inner().clone(),
    )
    .await
}

#[tauri::command]
pub async fn run_main_chat_agent_beta_v1_readiness_gate(
    _state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_agent_beta_v1_readiness::MainChatAgentBetaV1ReadinessReport, String> {
    crate::main_chat_agent_beta_v1_readiness::run_main_chat_agent_beta_v1_readiness_report().await
}

#[tauri::command]
pub async fn run_main_chat_agent_stage2_readiness_gate(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_agent_stage2_readiness::MainChatAgentStage2ReadinessReport, String> {
    let state = state.inner().clone();
    let runtime_handle = tokio::runtime::Handle::current();
    std::thread::Builder::new()
        .name("stage2-readiness-gate".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            runtime_handle.block_on(
                crate::main_chat_agent_stage2_readiness::run_main_chat_agent_stage2_readiness_report(
                    &state,
                ),
            )
        })
        .map_err(|err| format!("spawn stage2 readiness gate worker: {err}"))?
        .join()
        .map_err(|_| "stage2 readiness gate worker panicked".to_string())?
}

#[tauri::command]
pub async fn validate_main_chat_agent_stage2_manual_dogfood_artifact(
) -> Result<crate::main_chat_agent_stage2_readiness::Stage2ManualDogfoodSummary, String> {
    Ok(crate::main_chat_agent_stage2_readiness::validate_stage2_manual_dogfood_artifact())
}

#[tauri::command]
pub async fn run_main_chat_agent_step6_product_acceptance_gate(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::main_chat_step6_product_acceptance::MainChatStep6ProductAcceptanceReport, String>
{
    let state = state.inner().clone();
    let runtime_handle = tokio::runtime::Handle::current();
    std::thread::Builder::new()
        .name("step6-product-acceptance-gate".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            runtime_handle.block_on(
                crate::main_chat_step6_product_acceptance::run_main_chat_step6_product_acceptance_report(
                    &state,
                ),
            )
        })
        .map_err(|err| format!("spawn step6 product acceptance gate worker: {err}"))?
        .join()
        .map_err(|_| "step6 product acceptance gate worker panicked".to_string())?
}

#[tauri::command]
pub async fn prepare_main_chat_step6_live_provider_eval_state(
    state: State<'_, Arc<AppState>>,
) -> Result<
    crate::main_chat_step6_product_acceptance::MainChatStep6LiveProviderEvalStatePrepReport,
    String,
> {
    crate::main_chat_step6_product_acceptance::prepare_main_chat_step6_live_provider_eval_state_with_state(
        &state.inner().clone(),
    )
    .await
}

#[tauri::command]
pub async fn run_main_chat_agent_stage1_dogfood_gate(
    _state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let runtime_handle = tokio::runtime::Handle::current();
    let report = std::thread::Builder::new()
        .name("stage1-dogfood-gate".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            runtime_handle.block_on(
                crate::main_chat_agent_stage1_dogfood::run_main_chat_agent_stage1_dogfood_report(),
            )
        })
        .map_err(|err| format!("spawn stage1 dogfood gate worker: {err}"))?
        .join()
        .map_err(|_| "stage1 dogfood gate worker panicked".to_string())??;
    serde_json::to_value(report).map_err(|err| format!("serialize stage1 dogfood report: {err}"))
}

#[tauri::command]
pub async fn prepare_main_chat_agent_stage1_browser_dogfood_state(
    state: State<'_, Arc<AppState>>,
) -> Result<
    crate::main_chat_agent_stage1_dogfood::MainChatAgentStage1BrowserDogfoodPrepReport,
    String,
> {
    crate::main_chat_agent_stage1_dogfood::prepare_main_chat_agent_stage1_browser_dogfood_state_with_state(
        &state.inner().clone(),
    )
    .await
}

#[tauri::command]
pub async fn set_main_chat_agent_stage1_browser_network_policy(
    enabled: bool,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    if !cfg!(debug_assertions) {
        return Err("stage1_browser_network_policy_toggle_debug_only".into());
    }
    let mut config = state.config.lock().await;
    let previous = config.system.network_policy.enabled;
    config.system.network_policy.enabled = enabled;
    Ok(previous)
}

#[tauri::command]
pub async fn set_main_chat_agent_stage1_browser_web_fixture_output(
    output: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<String>, String> {
    if !cfg!(debug_assertions) {
        return Err("stage1_browser_web_fixture_toggle_debug_only".into());
    }
    let mut fixture = state.web_search_fixture_output.lock().await;
    let previous = fixture.clone();
    *fixture = output;
    Ok(previous)
}

#[tauri::command]
pub async fn set_main_chat_agent_stage1_browser_scripted_response(
    response: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<String>, String> {
    if !cfg!(debug_assertions) {
        return Err("stage1_browser_scripted_response_toggle_debug_only".into());
    }
    let mut scheduler = state.scheduler.lock().await;
    let previous = scheduler.scripted_generation_response.clone();
    scheduler.scripted_generation_response = response;
    Ok(previous)
}

#[tauri::command]
pub async fn run_main_chat_agent_execution_v1_final_acceptance_gate(
    state: State<'_, Arc<AppState>>,
) -> Result<MainChatAgentExecutionV1FinalAcceptanceGateCommandReport, String> {
    run_main_chat_agent_execution_v1_final_acceptance_gate_with_state(&state.inner().clone()).await
}

pub(crate) async fn run_main_chat_agent_execution_v1_final_acceptance_gate_with_state(
    state: &Arc<AppState>,
) -> Result<MainChatAgentExecutionV1FinalAcceptanceGateCommandReport, String> {
    run_main_chat_agent_execution_v1_final_acceptance_gate_with_state_and_live_opt_in(
        state,
        crate::main_chat_live_provider_harness::main_chat_live_provider_eval_opt_in_from_env(),
    )
    .await
}

pub(crate) async fn run_main_chat_agent_execution_v1_final_acceptance_gate_with_state_and_live_opt_in(
    state: &Arc<AppState>,
    explicit_live_eval_requested: bool,
) -> Result<MainChatAgentExecutionV1FinalAcceptanceGateCommandReport, String> {
    let report_run_id = format!("main-chat-final-gate-{}", uuid::Uuid::new_v4());
    let runtime_eval =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let (live_provider_preflight, live_reports) =
        crate::main_chat_live_provider_harness::run_main_chat_live_provider_eval_harness_suite_from_state(
            state,
            explicit_live_eval_requested,
        )
        .await?;
    let command_surface_eval =
        crate::main_chat_command_surface_eval::run_main_chat_command_surface_eval_report().await;
    let command_surface = command_surface_eval.acceptance_evidence();
    let command_surface_total_cases = command_surface.total_cases;
    let command_surface_send_stream_matrix_coverage = command_surface.send_stream_matrix_coverage;
    let external_provider_invoked = live_reports
        .iter()
        .any(|report| report.provider_endpoint_kind == "external_provider" && report.model_invoked);
    let mut final_gate =
        crate::main_chat_final_gate::build_main_chat_agent_execution_v1_final_gate_report(
            runtime_eval,
            command_surface_total_cases,
            command_surface,
            explicit_live_eval_requested,
            live_reports,
        );
    let legacy_fallback_used_count =
        crate::main_chat_runtime_status::main_chat_legacy_fallback_used_count(state).await;
    crate::main_chat_runtime_status::apply_startup_legacy_fallback_blocker(
        &mut final_gate.acceptance,
        legacy_fallback_used_count,
    );
    crate::main_chat_runtime_status::record_main_chat_final_gate_readiness(
        state,
        &final_gate.acceptance,
        report_run_id.clone(),
    )
    .await;
    let no_external_provider_invocation = !external_provider_invoked;
    let metadata_safe_summary = json!({
        "reportKind": "main_chat_agent_execution_v1_final_acceptance_gate",
        "reportRunId": report_run_id.clone(),
        "runtimeTotalCases": final_gate.runtime_total_cases,
        "commandSurfaceTotalCases": final_gate.command_surface_total_cases,
        "commandSurfaceGateExecuted": true,
        "commandSurfaceFailedCases": command_surface_eval.failed_cases,
        "commandSurfaceFailures": command_surface_eval.failures.clone(),
        "commandSurfaceSendStreamMatrixCoverage": command_surface_send_stream_matrix_coverage,
        "commandSurfaceLegacyFallbackCount": command_surface_eval.legacy_fallback_count,
        "commandSurfaceSilentWriteCount": command_surface_eval.silent_write_count,
        "liveProviderAttempted": explicit_live_eval_requested,
        "liveProviderReportCount": final_gate.live_provider_report_count,
        "liveProviderReadyCount": final_gate.live_provider_ready_count,
        "liveProviderScenarioReports": final_gate.live_provider_scenario_reports.clone(),
        "liveProviderPreflightReady": live_provider_preflight.ready,
        "liveProviderPreflightStatus": live_provider_preflight.status,
        "liveProviderPreflightProvider": live_provider_preflight.provider,
        "liveProviderPreflightBlockers": live_provider_preflight.blockers,
        "liveProviderPreflightRequiredEvidence": live_provider_preflight.required_evidence,
        "liveProviderPreflightInvocationAllowed": live_provider_preflight.live_provider_invocation_allowed,
        "liveProviderPreflightModelInvoked": live_provider_preflight.model_invoked,
        "liveProviderPreflightDirectWritesExecuted": live_provider_preflight.direct_writes_executed,
        "acceptanceReady": final_gate.acceptance.ready,
        "acceptanceStatus": final_gate.acceptance.status,
        "blockerCount": final_gate.acceptance.blockers.len(),
        "metadataSafe": true,
    });

    Ok(MainChatAgentExecutionV1FinalAcceptanceGateCommandReport {
        report_kind: "main_chat_agent_execution_v1_final_acceptance_gate".into(),
        report_run_id,
        final_gate,
        command_surface_eval,
        live_provider_preflight,
        command_surface_gate_executed: true,
        live_provider_attempted: explicit_live_eval_requested,
        migration_permission: false,
        metadata_safe: true,
        no_external_provider_invocation,
        no_app_store_writes: true,
        metadata_safe_summary,
    })
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
    let hs_packet = build_chat_runtime_hs_packet(
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
    )
    .with_guidance_consumption_mode(RuntimeGuidanceConsumptionMode::ExplicitRuntime);
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
    let execution_report = &output.execution_report;

    json!({
        "runtimeStrategyTraceKind": "multi_strategy_preview",
        "previewRuntime": "multi_strategy",
        "selectedStrategyKind": strategy_kind,
        "taskKind": task_kind,
        "strategyKind": strategy_kind,
        "payloadKind": payload_kind,
        "strategyDescriptorId": execution_report.strategy_descriptor_id.clone(),
        "strategyCapabilityIds": execution_report.strategy_capability_ids.clone(),
        "governanceDecisionKind": governance_decision_kind,
        "governancePolicyKind": governance_policy_kind,
        "selectionReasonCode": execution_report.selection_reason_code.clone(),
        "reasonCode": reason_code,
        "riskLevel": risk_level,
        "hasHsPacket": has_hs_packet,
        "registryReady": execution_report.registry_ready,
        "warnings": warnings,
        "proposalIds": proposal_ids,
        "planStepCount": plan_step_count,
        "planStepStatuses": plan_step_statuses,
        "blocked": blocked,
        "metadataSafe": true,
        "defaultChatUnchanged": execution_report.default_chat_unchanged,
        "sideEffectBudget": execution_report.side_effect_budget.clone(),
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
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
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

    #[derive(Debug, PartialEq, Eq)]
    struct StatusSideEffectCounts {
        agent_runs: usize,
        proposals: usize,
        evidence: usize,
        memory_messages: usize,
        mcp_audit_logs: usize,
        plan_sessions: usize,
    }

    async fn status_side_effect_counts(state: &Arc<crate::AppState>) -> StatusSideEffectCounts {
        let agent_runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs(100, 0).unwrap().len()
        };
        let proposals = {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.list_all_proposals(100, 0).unwrap().len()
        };
        let evidence = {
            let store = state.evidence_store.lock().await;
            store.query(EvidenceQuery::default()).unwrap().len()
        };
        let memory_messages = {
            let store = state.memory_store.lock().await;
            store.export_all_messages().unwrap().len()
        };
        let mcp_audit_logs = {
            let store = state.mcp_audit_store.lock().await;
            store.list_logs(100).unwrap().len()
        };
        let plan_sessions = {
            let store = state
                .plan_execute_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            store.list_sessions(100).unwrap().len()
        };

        StatusSideEffectCounts {
            agent_runs,
            proposals,
            evidence,
            memory_messages,
            mcp_audit_logs,
            plan_sessions,
        }
    }

    #[tokio::test]
    async fn runtime_strategy_registry_status_command_reports_ready_and_read_only() {
        let state = preview_state().await;
        let before = status_side_effect_counts(&state).await;

        let report = get_runtime_strategy_registry_status_with_state(&state)
            .await
            .unwrap();

        assert!(report.maturity_ready);
        assert!(report.registry_readiness.ready);
        assert!(report.default_chat_unchanged);
        assert!(!report.migration_permission);
        assert!(report.no_runtime_model_tool_execution);
        assert!(report.no_business_writes);
        assert_eq!(report.status_command_side_effect_budget.runtime_calls, 0);
        assert_eq!(report.status_command_side_effect_budget.model_calls, 0);
        assert_eq!(report.status_command_side_effect_budget.tool_calls, 0);
        assert!(report
            .future_strategy_descriptors
            .iter()
            .any(|descriptor| descriptor.strategy_kind == "workflow"
                && descriptor.declarative_only
                && !descriptor.executable));

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("assistant output"));
        assert!(!serialized.contains("LifeModel text"));
        assert!(!serialized.contains("memory context"));
        assert!(!serialized.contains("tool payload"));
        assert!(!serialized.contains("alice@example.com"));

        let after = status_side_effect_counts(&state).await;
        assert_eq!(before, after);
    }

    #[tokio::test]
    async fn react_beta_execution_status_command_reports_ready_read_only_and_not_migration_permission(
    ) {
        let state = preview_state().await;
        let before = status_side_effect_counts(&state).await;

        let report = get_react_beta_execution_status_with_state(&state)
            .await
            .unwrap();

        assert_eq!(report.report_kind, "react_beta_execution_status");
        assert!(
            report.readiness.ready,
            "{:?}",
            report.readiness.blocking_reasons
        );
        assert!(report.tool_registry_readiness.ready);
        assert!(report.default_chat_unchanged);
        assert!(!report.migration_permission);
        assert!(report.no_runtime_model_tool_execution);
        assert!(report.no_business_writes);
        assert!(report.metadata_safe);
        assert_eq!(report.status_command_side_effect_budget.runtime_calls, 0);
        assert_eq!(report.status_command_side_effect_budget.model_calls, 0);
        assert_eq!(report.status_command_side_effect_budget.tool_calls, 0);
        assert_eq!(report.status_command_side_effect_budget.store_writes, 0);

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("raw assistant output"));
        assert!(!serialized.contains("raw tool payload"));
        assert!(!serialized.contains("memory context"));
        assert!(!serialized.contains("LifeModel text"));
        assert!(!serialized.contains("secret@example.com"));

        let after = status_side_effect_counts(&state).await;
        assert_eq!(before, after);
    }

    #[tokio::test]
    async fn main_chat_agent_execution_v1_eval_gate_command_runs_core_runtime_eval_read_only_and_blocked_without_live(
    ) {
        let state = preview_state().await;
        {
            let mut config = state.config.lock().await;
            config.system.network_policy.enabled = false;
        }
        let before = status_side_effect_counts(&state).await;

        let report = run_main_chat_agent_execution_v1_eval_gate_with_state(&state)
            .await
            .unwrap();

        assert_eq!(report.report_kind, "main_chat_agent_execution_v1_eval_gate");
        assert_eq!(report.runtime_eval.total_cases, 100);
        assert_eq!(report.runtime_eval.deterministic_stub_case_count, 0);
        assert_eq!(report.runtime_eval.failed_cases, 0);
        assert!(!report.acceptance.ready);
        assert_eq!(report.acceptance.status, "blocked");
        assert!(!report.command_surface_gate_executed);
        assert!(!report.live_provider_attempted);
        assert!(!report.migration_permission);
        assert!(report.metadata_safe);
        assert!(report.no_external_provider_invocation);
        assert!(report.no_app_store_writes);
        assert!(report
            .acceptance
            .blockers
            .contains(&"command_surface_cases_below_38".to_string()));
        assert!(report
            .acceptance
            .blockers
            .contains(&"live_provider_generation_not_executed".to_string()));
        assert!(!report.live_provider_preflight.ready);
        assert_eq!(report.live_provider_preflight.status, "blocked");
        assert!(
            !report
                .live_provider_preflight
                .live_provider_invocation_allowed
        );
        assert!(!report.live_provider_preflight.model_invoked);
        assert!(!report.live_provider_preflight.direct_writes_executed);
        assert!(report
            .live_provider_preflight
            .blockers
            .contains(&"explicit_live_eval_required".to_string()));
        assert!(report
            .live_provider_preflight
            .blockers
            .contains(&"provider_api_key_missing".to_string()));
        assert!(report
            .live_provider_preflight
            .blockers
            .contains(&"network_disabled".to_string()));
        let live_preflight_blockers = report
            .metadata_safe_summary
            .get("liveProviderPreflightBlockers")
            .and_then(Value::as_array)
            .expect("live provider preflight blockers should be reported");
        assert!(live_preflight_blockers
            .iter()
            .any(|blocker| blocker == "explicit_live_eval_required"));
        assert!(live_preflight_blockers
            .iter()
            .any(|blocker| blocker == "provider_api_key_missing"));
        assert!(live_preflight_blockers
            .iter()
            .any(|blocker| blocker == "network_disabled"));
        assert_eq!(
            report
                .metadata_safe_summary
                .get("liveProviderPreflightModelInvoked")
                .and_then(Value::as_bool),
            Some(false)
        );

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("raw assistant output"));
        assert!(!serialized.contains("raw tool payload"));
        assert!(!serialized.contains("memory context"));
        assert!(!serialized.contains("LifeModel text"));

        let after = status_side_effect_counts(&state).await;
        assert_eq!(before, after);
    }

    #[tokio::test]
    async fn main_chat_agent_execution_v1_final_acceptance_command_uses_real_aggregation_and_fails_closed_without_live(
    ) {
        let state = preview_state().await;
        {
            let mut config = state.config.lock().await;
            config.system.network_policy.enabled = false;
        }
        let before = status_side_effect_counts(&state).await;

        let report = run_main_chat_agent_execution_v1_final_acceptance_gate_with_state(&state)
            .await
            .unwrap();

        assert_eq!(
            report.report_kind,
            "main_chat_agent_execution_v1_final_acceptance_gate"
        );
        assert_eq!(report.final_gate.runtime_total_cases, 100);
        assert_eq!(
            report.final_gate.command_surface_total_cases,
            crate::main_chat_command_surface_eval::MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES.len()
        );
        assert_eq!(
            report.command_surface_eval.total_cases,
            crate::main_chat_command_surface_eval::MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES.len()
        );
        assert_eq!(report.command_surface_eval.failed_cases, 0);
        assert!(report.command_surface_eval.send_coverage > 0.0);
        assert!(report.command_surface_eval.stream_coverage > 0.0);
        assert!(report.command_surface_eval.failures.is_empty());
        assert_eq!(
            report
                .command_surface_eval
                .acceptance_evidence()
                .send_stream_matrix_coverage,
            1.0
        );
        assert!(!report.final_gate.live_provider_attempted);
        assert_eq!(report.final_gate.live_provider_report_count, 0);
        assert_eq!(report.final_gate.live_provider_ready_count, 0);
        assert!(!report.final_gate.acceptance.ready);
        assert_eq!(report.final_gate.acceptance.status, "blocked");
        assert!(report.command_surface_gate_executed);
        assert!(!report.live_provider_attempted);
        assert!(!report.migration_permission);
        assert!(report.metadata_safe);
        assert!(report.no_external_provider_invocation);
        assert!(report.no_app_store_writes);
        assert!(!report
            .final_gate
            .acceptance
            .blockers
            .contains(&"command_surface_cases_below_38".to_string()));
        assert!(!report
            .final_gate
            .acceptance
            .blockers
            .contains(&"command_surface_send_stream_matrix_incomplete".to_string()));
        assert!(report
            .final_gate
            .acceptance
            .blockers
            .contains(&"live_provider_generation_not_executed".to_string()));
        assert!(!report.live_provider_preflight.ready);
        assert_eq!(report.live_provider_preflight.status, "blocked");
        assert!(!report.live_provider_preflight.model_invoked);
        assert!(!report.live_provider_preflight.direct_writes_executed);
        assert!(report
            .live_provider_preflight
            .blockers
            .contains(&"explicit_live_eval_required".to_string()));
        assert!(report
            .live_provider_preflight
            .blockers
            .contains(&"network_disabled".to_string()));

        let serialized = serde_json::to_string(&report).unwrap();
        assert!(!serialized.contains("raw prompt"));
        assert!(!serialized.contains("raw assistant output"));
        assert!(!serialized.contains("raw tool payload"));
        assert!(!serialized.contains("memory context"));
        assert!(!serialized.contains("LifeModel text"));

        let after = status_side_effect_counts(&state).await;
        assert_eq!(before, after);
    }

    #[tokio::test]
    async fn main_chat_agent_execution_v1_final_acceptance_command_attempts_live_when_opted_in_and_blocks_without_credentials(
    ) {
        let state = preview_state().await;
        {
            let mut config = state.config.lock().await;
            config.llm.provider = "openai".into();
            config.llm.openai_base = "https://api.openai.com/v1".into();
            config.llm.chat_model = "gpt-4o-mini".into();
            config.llm.openai_key.clear();
            config.system.network_policy.enabled = true;
        }
        {
            let config = state.config.lock().await.clone();
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = openlife_core::scheduler::InferenceScheduler::new(
                config.local_model.clone(),
                false,
                config.llm.provider.clone(),
                config.llm.openai_base.clone(),
                String::new(),
                config.llm.chat_model.clone(),
                config.llm.embedding_model.clone(),
                false,
            );
        }
        let before = status_side_effect_counts(&state).await;

        let report =
            run_main_chat_agent_execution_v1_final_acceptance_gate_with_state_and_live_opt_in(
                &state, true,
            )
            .await
            .unwrap();

        assert!(report.live_provider_attempted);
        assert!(report.command_surface_gate_executed);
        assert_eq!(report.command_surface_eval.failed_cases, 0);
        assert_eq!(report.final_gate.live_provider_report_count, 4);
        assert_eq!(report.final_gate.live_provider_ready_count, 0);
        assert_eq!(report.final_gate.live_provider_main_chat_invoked_count, 0);
        assert_eq!(report.final_gate.live_provider_model_invoked_count, 0);
        assert!(report.no_external_provider_invocation);
        assert!(!report.live_provider_preflight.ready);
        assert!(report
            .live_provider_preflight
            .blockers
            .contains(&"provider_api_key_missing".to_string()));
        assert!(report
            .final_gate
            .live_provider_blockers
            .contains(&"provider_api_key_missing".to_string()));
        assert!(!report.final_gate.acceptance.ready);
        assert!(report
            .final_gate
            .acceptance
            .blockers
            .contains(&"live_provider_generation_not_executed".to_string()));

        let after = status_side_effect_counts(&state).await;
        assert_eq!(before, after);
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
        assert_eq!(audit["runtimeStrategyTraceKind"], "multi_strategy_preview");
        assert_eq!(audit["selectedStrategyKind"], "planExecute");
        assert_eq!(audit["strategyDescriptorId"], "plan_execute");
        assert_eq!(audit["strategyCapabilityIds"][0], "planning.plan_execute");
        assert_eq!(audit["selectionReasonCode"], "planning_intent_allowed");
        assert_eq!(audit["registryReady"], true);
        assert_eq!(audit["defaultChatUnchanged"], true);
        assert_eq!(audit["sideEffectBudget"]["externalWrites"], 0);
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
