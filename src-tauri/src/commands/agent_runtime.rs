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
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("promotedMessageHash is required".into());
    }
    if trimmed.len() > 160 || !trimmed.chars().all(is_safe_checksum_char) {
        return Err("promotedMessageHash must be a metadata-safe checksum".into());
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
