use crate::{
    artifact_materializer::{
        artifact_content_digest, commit_artifact_move, commit_staged_artifact,
        confirmed_artifact_receipt, confirmed_move_receipt, confirmed_move_receipt_from_paths,
        inspect_artifact_filesystem, inspect_artifact_move,
        prepare_artifact_materialization_with_precondition_for_artifact_bytes,
        prepare_artifact_move, stage_artifact_raw_bytes, ArtifactFilesystemFailure,
        ArtifactFilesystemObservation, ArtifactMaterializationReceipt, ArtifactTargetPrecondition,
    },
    danger_action_confirmation::{
        require_native_danger_action_confirmation, NativeDangerActionRequest,
    },
    life_model_write_gateway, memory_gateway, AppState,
};
use openlife_core::agent::{
    AgentProposal, LifeModelLearningCandidateStatus, LifeModelLearningReviewDecisionReceipt,
    MemoryRollbackReport, ProposalSource, ProposalStatus, ProposalType, RiskLevel,
};
use openlife_core::task_runtime::{CanonicalArtifactEffectState, CanonicalArtifactReviewSubject};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

/// Maximum content size for ExternalWriteAction (100 KB)
const EXTERNAL_WRITE_MAX_SIZE: usize = 100 * 1024;
pub(crate) const COMMUNICATION_STYLE_CANONICAL_PATH: &str = "preferences.communication_style";

fn canonical_work_artifact_id(proposal: &AgentProposal) -> Option<&str> {
    canonical_work_artifact_id_from_after(&proposal.after)
}

fn canonical_work_artifact_id_from_after(after: &Value) -> Option<&str> {
    let artifact_undo = after
        .get("undoOfArtifactId")
        .and_then(Value::as_str)
        .is_some();
    if !artifact_undo {
        let subject =
            serde_json::from_value::<CanonicalArtifactReviewSubject>(after.clone()).ok()?;
        subject.validate().ok()?;
    }
    after
        .get("artifactId")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("artifact:") && value.len() <= 512)
}

fn canonical_artifact_review_subject(
    proposal: &AgentProposal,
) -> Result<CanonicalArtifactReviewSubject, String> {
    let subject = serde_json::from_value::<CanonicalArtifactReviewSubject>(proposal.after.clone())
        .map_err(|_| "canonical_artifact_review_subject_invalid".to_string())?;
    subject.validate().map_err(|error| error.to_string())?;
    if proposal.run_id.as_deref() != Some(subject.source_run_id.as_str())
        || proposal.source_detail.as_deref() != Some(subject.canonical_task_id.as_str())
    {
        return Err("canonical_artifact_review_origin_mismatch".into());
    }
    Ok(subject)
}

fn artifact_id_for_proposal(proposal: &AgentProposal) -> String {
    canonical_work_artifact_id(proposal)
        .map(str::to_string)
        .expect("validated ExternalWriteAction must bind a canonical Artifact")
}

fn reviewed_artifact_target_precondition(
    after: &Value,
) -> Result<ArtifactTargetPrecondition, String> {
    let subject = serde_json::from_value::<CanonicalArtifactReviewSubject>(after.clone())
        .map_err(|_| "canonical_artifact_review_subject_invalid".to_string())?;
    subject.validate().map_err(|error| error.to_string())?;
    let expected_absent = subject.expected_target_absent;
    let expected_digest = subject.expected_target_digest.as_deref();
    match (expected_absent, expected_digest) {
        (true, None) => Ok(ArtifactTargetPrecondition::Absent),
        (false, Some(digest)) if digest.starts_with("sha256:") => Ok(
            ArtifactTargetPrecondition::ContentDigest(digest.to_string()),
        ),
        _ => Err("Artifact Proposal 必须精确绑定目标不存在或审核时的目标内容摘要。".into()),
    }
}

fn require_persistence_write(state: &Arc<AppState>) -> Result<(), String> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| error.to_string())
}

fn require_proposal_write_for(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<(), String> {
    if proposal.proposal_type == ProposalType::ExternalWriteAction {
        if canonical_work_artifact_id(proposal).is_none() {
            return Err("noncanonical_artifact_effect_retired".into());
        }
        state
            .persistence_coordinator
            .require_effects_for_stores(&["ProposalStore", "CanonicalTaskRuntimeStore"])
            .map_err(|error| error.to_string())
    } else {
        require_persistence_write(state)
    }
}

#[derive(Clone, Copy)]
enum ProposalReconciliationAdmission {
    ProductEffects,
    StartupInternal,
}

fn require_proposal_reconciliation_admission(
    state: &Arc<AppState>,
    admission: ProposalReconciliationAdmission,
) -> Result<(), String> {
    match admission {
        ProposalReconciliationAdmission::ProductEffects => require_persistence_write(state),
        ProposalReconciliationAdmission::StartupInternal
            if state
                .persistence_coordinator
                .startup_reconciliation_mutations_safe() =>
        {
            Ok(())
        }
        ProposalReconciliationAdmission::StartupInternal => {
            Err("startup_proposal_reconciliation_mutations_unavailable".into())
        }
    }
}

fn runtime_proposal_store_error(state: &Arc<AppState>, error: impl ToString) -> String {
    let error = error.to_string();
    state
        .persistence_coordinator
        .register_runtime_durable_failure("ProposalStore", &error);
    format!("proposal_store_runtime_degraded:{error}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptProposalResponse {
    pub success: bool,
    #[serde(alias = "patch_result")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_result: Option<openlife_core::life_model::patch::PatchApplyResult>,
    #[serde(alias = "effect_status")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_status: Option<String>,
    #[serde(alias = "proposal_projection_status")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_projection_status: Option<String>,
    /// Projection into the canonical Work Task/Run/Item/Artifact owner is a
    /// separate truth surface from Proposal status. Keep it in the strict IPC
    /// contract so a confirmed effect never becomes an apparent UI failure.
    #[serde(alias = "canonical_task_runtime_projection_status")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_task_runtime_projection_status: Option<String>,
    #[serde(alias = "canonical_tool_review_projection_status")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_tool_review_projection_status: Option<String>,
    #[serde(alias = "proposal_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durable_write_executed: Option<bool>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_gateway: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_lifecycle: Option<Value>,
    /// Canonical Memory commit and its derived projection are separate facts.
    /// Keeping this field in the typed IPC contract prevents serde from
    /// silently dropping a degraded/pending projection while reporting the
    /// already-confirmed effect to the product as fully applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_persistence: Option<MemoryPersistenceResponse>,
    /// Present only for a confirmed filesystem materialization. Proposal
    /// creation and permission wait responses never manufacture this receipt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_materialization: Option<ArtifactMaterializationReceipt>,
    /// Candidate lifecycle reconciliation is part of the confirmed LifeModel
    /// effect. The command must never return an IPC error after the canonical
    /// version was already committed merely because this truth field was not
    /// represented by the typed response contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub life_model_learning: Option<LifeModelLearningAcceptResponse>,
    #[serde(alias = "blocked_action")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_action: Option<Value>,
    #[serde(alias = "can_continue")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_continue: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LifeModelLearningAcceptResponse {
    Materialized(LifeModelLearningReviewDecisionReceipt),
    ReconciliationRequired(LifeModelLearningReconciliationRequiredResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelLearningReconciliationRequiredResponse {
    pub proposal_id: String,
    pub status: LifeModelLearningReconciliationStatus,
    pub canonical_life_model_changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelLearningReconciliationStatus {
    ReconciliationRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoryPersistenceResponse {
    pub canonical_committed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outbox_event_id: Option<String>,
    pub projection_state: openlife_core::persistence_outbox::ProjectionDeliveryState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_digest: Option<String>,
}

fn typed_accept_proposal_response(value: Value) -> Result<AcceptProposalResponse, String> {
    let response: AcceptProposalResponse = serde_json::from_value(value)
        .map_err(|error| format!("accept Proposal response contract mismatch: {error}"))?;
    if response.success {
        if response.effect_status.as_deref() != Some("confirmed")
            || !matches!(
                response.proposal_projection_status.as_deref(),
                Some("confirmed" | "reconciliation_required")
            )
            || matches!(
                response.canonical_task_runtime_projection_status.as_deref(),
                Some(value)
                    if !matches!(value, "confirmed" | "reconciliation_required" | "not_applicable")
            )
            || matches!(
                response.canonical_tool_review_projection_status.as_deref(),
                Some(value)
                    if !matches!(value, "confirmed" | "reconciliation_required" | "not_applicable")
            )
        {
            return Err(
                "accept Proposal confirmed response is missing confirmed effect/projection truth"
                    .into(),
            );
        }
        if response.patch_result.is_none() && response.proposal_id.is_none() {
            return Err(
                "accept Proposal confirmed response is missing both patch and proposal identity"
                    .into(),
            );
        }
        if response.status.is_some()
            || response.reason_code.is_some()
            || response.dispatch_state.is_some()
            || response.durable_write_executed.is_some()
        {
            return Err(
                "accept Proposal confirmed response contains deferred-only truth fields".into(),
            );
        }
        if let Some(learning) = response.life_model_learning.as_ref() {
            match learning {
                LifeModelLearningAcceptResponse::Materialized(receipt)
                    if receipt.status == LifeModelLearningCandidateStatus::Materialized
                        && receipt.materialized_version.is_some()
                        && receipt.materialized_document_digest.is_some()
                        && receipt.canonical_life_model_changed => {}
                LifeModelLearningAcceptResponse::ReconciliationRequired(receipt)
                    if receipt.canonical_life_model_changed => {}
                _ => {
                    return Err(
                        "accept Proposal LifeModel learning response lacks confirmed materialization truth"
                            .into(),
                    )
                }
            }
        }
    } else if response.status.as_deref() != Some("deferred")
        || response.reason_code.as_deref().is_none()
        || response.proposal_id.as_deref().is_none()
        || response.dispatch_state.as_deref().is_none()
        || response.durable_write_executed != Some(false)
    {
        return Err(
            "accept Proposal non-success response is not a complete deferred result".into(),
        );
    } else if response.patch_result.is_some()
        || response.effect_status.is_some()
        || response.proposal_projection_status.is_some()
        || response.canonical_task_runtime_projection_status.is_some()
        || response.canonical_tool_review_projection_status.is_some()
        || response.memory_gateway.is_some()
        || response.memory_lifecycle.is_some()
        || response.memory_persistence.is_some()
        || response.artifact_materialization.is_some()
        || response.life_model_learning.is_some()
        || response.blocked_action.is_some()
        || response.can_continue.is_some()
    {
        return Err(
            "accept Proposal deferred response contains confirmed-effect truth fields".into(),
        );
    }
    Ok(response)
}

#[cfg(test)]
mod accept_response_contract_tests {
    use super::*;

    #[test]
    fn confirmed_artifact_response_keeps_canonical_tool_review_projection_truth() {
        let response = typed_accept_proposal_response(serde_json::json!({
            "success": true,
            "proposal_id": "proposal:test",
            "effect_status": "confirmed",
            "proposal_projection_status": "confirmed",
            "canonical_task_runtime_projection_status": "confirmed",
            "canonical_tool_review_projection_status": "not_applicable",
            "warnings": []
        }))
        .unwrap();

        assert_eq!(
            response.canonical_tool_review_projection_status.as_deref(),
            Some("not_applicable")
        );
    }
}

pub(crate) fn canonical_lifemodel_path(path: &str) -> String {
    let trimmed = path.trim();
    let normalized = if trimmed.starts_with('/') {
        trimmed
            .trim_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(".")
    } else {
        trimmed.trim_matches('.').to_string()
    };
    match normalized.to_ascii_lowercase().as_str() {
        "preferences.communication_style" | "preferences.communication" => {
            COMMUNICATION_STYLE_CANONICAL_PATH.to_string()
        }
        _ => normalized,
    }
}

fn canonicalize_proposal_affected_path(proposal: &mut AgentProposal) {
    let canonical = canonical_lifemodel_path(&proposal.affected_path);
    if canonical != proposal.affected_path {
        proposal.affected_path = canonical;
    }
}

fn proposal_store_missing() -> String {
    "Proposal store is unavailable. Please check Settings > 试用就绪检查.".to_string()
}

fn ensure_pending_or_postponed(proposal: &AgentProposal) -> Result<(), String> {
    match proposal.status {
        ProposalStatus::Pending | ProposalStatus::Postponed | ProposalStatus::Edited => Ok(()),
        ProposalStatus::Accepted => Err("该 Proposal 已经被接受，不能重复处理。".to_string()),
        ProposalStatus::Rejected => Err("该 Proposal 已经被拒绝，不能再次处理。".to_string()),
        ProposalStatus::Cancelled => {
            Err("该 Proposal 已随同一任务的其他决定一起取消。".to_string())
        }
        ProposalStatus::Expired => Err("该 Proposal 已经过期，不能再执行。".to_string()),
    }
}

async fn ensure_review_change_precedes_effect_dispatch(
    state: &Arc<AppState>,
    proposal_id: &str,
) -> Result<(), String> {
    let dispatch_state = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await
        .dispatch_state(proposal_id)
        .map_err(|error| error.to_string())?;
    match dispatch_state.as_deref() {
        Some("unclaimed" | "failed_before_effect") => Ok(()),
        Some("confirmed_projection_pending" | "confirmed") => Err(
            "Proposal effect is already confirmed; review mutation is blocked and projection reconciliation is required."
                .into(),
        ),
        Some("claimed" | "unknown") => Err(
            "Proposal effect state is not safely reversible; review mutation is blocked pending reconciliation."
                .into(),
        ),
        Some(other) => Err(format!(
            "unsupported Proposal dispatch state '{other}'; review mutation failed closed"
        )),
        None => Err("Proposal dispatch receipt is unavailable; review mutation failed closed".into()),
    }
}

fn is_lifemodel_v2_typed_diff(proposal: &AgentProposal) -> bool {
    proposal.proposal_type == ProposalType::LifeModelUpdate
        && proposal.affected_path == openlife_core::life_model::v2::LIFE_MODEL_V2_TYPED_DIFF_PATH
}

fn is_legacy_lifemodel_v2_migration(proposal: &AgentProposal) -> bool {
    proposal.proposal_type == ProposalType::LifeModelUpdate
        && proposal.affected_path
            == openlife_core::life_model::legacy_migration::LIFE_MODEL_V2_LEGACY_MIGRATION_PATH
}

fn dispatch_failure_was_definitely_before_effect(operation: &str) -> bool {
    matches!(
        operation,
        "validation_failed"
            | "lifemodel_gateway_stale_conflict"
            | "lifemodel_patch_conflict"
            | "lifemodel_gateway_blocked"
            | "lifemodel_field_authority_blocked"
            | "lifemodel_patch_batch_validation_failed"
            | "lifemodel_patch_batch_field_authority_blocked"
            | "lifemodel_gateway_batch_stale_conflict"
            | "lifemodel_patch_batch_conflict"
            | "lifemodel_gateway_batch_blocked"
            | "lifemodel_compare_and_swap_conflict"
            | "lifemodel_v2_typed_diff_validation_failed"
            | "lifemodel_v2_typed_diff_precondition_failed"
            | "lifemodel_v2_typed_diff_commit_conflict"
            | "lifemodel_v2_migration_validation_failed"
            | "lifemodel_v2_migration_source_changed"
            | "lifemodel_v2_migration_backup_failed"
            | "lifemodel_v2_migration_commit_conflict"
            | "lifemodel_legacy_write_retired"
            | "lifemodel_legacy_owner_retired"
            | "memory_write_not_committed"
            | "memory_write_duplicate_no_effect"
            | "scheduled_cloud_due_time_missing"
            | "scheduled_cloud_due_time_invalid"
            | "scheduled_cloud_provider_preflight_failed"
            | "scheduled_cloud_network_policy_invalid"
            | "scheduled_cloud_network_policy_not_allowed"
            | "scheduled_cloud_policy_rejected"
            | "scheduled_cloud_grant_seal_rejected"
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProposalReconciliationReport {
    pub artifact_undo_checkpoints_repaired: usize,
    pub artifact_effects_reconciled: usize,
    pub ambiguous_action_effects_marked_unknown: usize,
    pub proposal_projections_repaired: usize,
    pub artifact_backlog_may_remain: bool,
    pub action_effect_backlog_may_remain: bool,
    pub projection_backlog_may_remain: bool,
}

fn required_proposal_after_string<'a>(
    proposal: &'a AgentProposal,
    field: &str,
) -> Result<&'a str, String> {
    proposal
        .after
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("canonical_artifact_undo_{field}_missing"))
}

fn expected_artifact_trash_target(source: &str) -> Result<String, String> {
    let source = std::path::Path::new(source);
    let parent = source
        .parent()
        .ok_or_else(|| "canonical_artifact_undo_trash_parent_missing".to_string())?;
    let filename = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "canonical_artifact_undo_trash_filename_invalid".to_string())?;
    let digest = openlife_core::agent::metadata_safe_text_digest(&source.to_string_lossy()).1;
    let token = digest
        .strip_prefix("sha256:")
        .unwrap_or(&digest)
        .chars()
        .take(16)
        .collect::<String>();
    Ok(parent
        .join(format!(".openlife-trash-{token}-{filename}"))
        .to_string_lossy()
        .into_owned())
}

async fn bind_orphaned_artifact_undo_checkpoint(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<bool, String> {
    if proposal.proposal_type != ProposalType::ExternalWriteAction
        || proposal.after.get("undoOfArtifactId").is_none()
    {
        return Ok(false);
    }
    let artifact_id = required_proposal_after_string(proposal, "undoOfArtifactId")?;
    if required_proposal_after_string(proposal, "artifactId")? != artifact_id {
        return Err("canonical_artifact_undo_identity_mismatch".into());
    }
    let version = proposal
        .after
        .get("artifactVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| "canonical_artifact_undo_artifactVersion_missing".to_string())?;
    let canonical_store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    if let Some(existing) = canonical_store
        .lock()
        .await
        .load_artifact_undo_version(artifact_id, version)
        .map_err(|error| error.to_string())?
    {
        if existing.proposal_id != proposal.id {
            return Err("canonical_artifact_undo_identity_conflict".into());
        }
        return Ok(false);
    }
    let owns_idempotency_key = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await
        .owns_active_review_idempotency_key(&proposal.id, &format!("artifact_undo:{artifact_id}"))
        .map_err(|error| runtime_proposal_store_error(state, error))?;
    if !owns_idempotency_key {
        return Err("canonical_artifact_undo_review_idempotency_mismatch".into());
    }
    let artifact = canonical_store
        .lock()
        .await
        .load_artifact(artifact_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_artifact_missing".to_string())?;
    if artifact.current_version != version
        || artifact.status != openlife_core::task_runtime::CanonicalArtifactStatus::Materialized
    {
        return Err("canonical_artifact_undo_source_not_verified".into());
    }
    let task_snapshot = canonical_store
        .lock()
        .await
        .load_task_snapshot(&artifact.task_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_artifact_task_missing".to_string())?;
    let artifact_snapshot = task_snapshot
        .artifacts
        .iter()
        .find(|snapshot| snapshot.artifact.id == artifact_id)
        .ok_or_else(|| "canonical_artifact_snapshot_missing".to_string())?;
    let direct_origin = proposal.after.get("undoOriginKind").and_then(Value::as_str)
        == Some("direct_materialization");
    let original_proposal_id = artifact_snapshot
        .review_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.proposal_id.as_str());
    if direct_origin {
        if original_proposal_id.is_some()
            || required_proposal_after_string(proposal, "undoOfDirectArtifactVersion")?
                != format!("{}:v{}", artifact.id, artifact.current_version)
            || artifact_snapshot.current_version.expected_target_absent != Some(true)
            || artifact_snapshot
                .current_version
                .expected_target_digest
                .is_some()
            || artifact_snapshot.pre_change_snapshot.is_some()
            || artifact_snapshot
                .current_version
                .observed_content_digest
                .as_deref()
                != Some(artifact.content_digest.as_str())
        {
            return Err("canonical_direct_artifact_undo_origin_invalid".into());
        }
    } else if required_proposal_after_string(proposal, "undoOfProposalId")?
        != original_proposal_id
            .ok_or_else(|| "canonical_artifact_review_origin_missing".to_string())?
    {
        return Err("canonical_artifact_undo_origin_mismatch".into());
    }
    if proposal.source_detail.as_deref() != Some(artifact.task_id.as_str())
        || required_proposal_after_string(proposal, "canonicalTaskId")? != artifact.task_id
        || proposal.run_id.as_deref()
            != Some(required_proposal_after_string(proposal, "sourceRunId")?)
        || proposal
            .after
            .get("directWritesExecuted")
            .and_then(Value::as_bool)
            != Some(false)
        || proposal
            .after
            .get("externalWritesExecuted")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err("canonical_artifact_undo_origin_mismatch".into());
    }
    let source_run_id = task_snapshot
        .items
        .iter()
        .find(|item| item.id == artifact.source_item_id)
        .map(|item| item.run_id.as_str())
        .ok_or_else(|| "canonical_artifact_source_item_missing".to_string())?;
    if required_proposal_after_string(proposal, "artifactDraftItemId")? != artifact.source_item_id
        || required_proposal_after_string(proposal, "sourceRunId")? != source_run_id
    {
        return Err("canonical_artifact_undo_source_item_mismatch".into());
    }
    let original_proposal = match original_proposal_id {
        Some(original_proposal_id) => {
            let original = get_proposal_with_state(state, original_proposal_id).await?;
            if original.status != ProposalStatus::Accepted {
                return Err("canonical_artifact_undo_original_proposal_not_accepted".into());
            }
            Some(original)
        }
        None if direct_origin => None,
        None => return Err("canonical_artifact_review_origin_missing".into()),
    };
    let original_operation = original_proposal
        .as_ref()
        .and_then(|proposal| proposal.after.get("operation"))
        .and_then(Value::as_str)
        .unwrap_or("create");
    let materialized_reference = artifact
        .materialized_reference
        .as_deref()
        .ok_or_else(|| "canonical_artifact_materialized_reference_missing".to_string())?;
    match required_proposal_after_string(proposal, "operation")? {
        "trash" => {
            if original_operation == "move"
                || original_proposal.as_ref().is_some_and(|proposal| {
                    !matches!(
                        reviewed_artifact_target_precondition(&proposal.after),
                        Ok(ArtifactTargetPrecondition::Absent)
                    )
                })
            {
                return Err("canonical_artifact_undo_operation_mismatch".into());
            }
            let source = required_proposal_after_string(proposal, "source_path")?;
            let target = required_proposal_after_string(proposal, "target_path")?;
            let digest = required_proposal_after_string(proposal, "source_digest")?;
            if source != materialized_reference
                || target != expected_artifact_trash_target(source)?
                || digest != artifact.content_digest
                || required_proposal_after_string(proposal, "contentDigest")? != digest
                || proposal.affected_path != format!("filesystem.{source}->{target}")
            {
                return Err("canonical_artifact_undo_projection_identity_mismatch".into());
            }
            canonical_store
                .lock()
                .await
                .bind_artifact_undo(artifact_id, &proposal.id, source, target, digest)
                .map_err(|error| error.to_string())?;
        }
        "restore" => {
            let original_proposal = original_proposal
                .as_ref()
                .ok_or_else(|| "canonical_artifact_undo_operation_mismatch".to_string())?;
            let source = required_proposal_after_string(proposal, "source_path")?;
            let target = required_proposal_after_string(proposal, "target_path")?;
            let digest = required_proposal_after_string(proposal, "source_digest")?;
            if original_operation != "move"
                || source != materialized_reference
                || target != required_proposal_after_string(original_proposal, "source_path")?
                || digest != artifact.content_digest
                || required_proposal_after_string(proposal, "contentDigest")? != digest
                || proposal.affected_path != format!("filesystem.{source}->{target}")
            {
                return Err("canonical_artifact_undo_projection_identity_mismatch".into());
            }
            canonical_store
                .lock()
                .await
                .bind_artifact_rename_undo(artifact_id, &proposal.id, source, target, digest)
                .map_err(|error| error.to_string())?;
        }
        "restore_snapshot" => {
            let original_proposal = original_proposal
                .as_ref()
                .ok_or_else(|| "canonical_artifact_undo_operation_mismatch".to_string())?;
            if original_operation == "move"
                || matches!(
                    reviewed_artifact_target_precondition(&original_proposal.after)?,
                    ArtifactTargetPrecondition::Absent
                )
            {
                return Err("canonical_artifact_undo_operation_mismatch".into());
            }
            let source = required_proposal_after_string(proposal, "snapshot_path")?;
            let target = required_proposal_after_string(proposal, "path")?;
            let restore_digest = required_proposal_after_string(proposal, "restore_digest")?;
            let expected_target_digest =
                required_proposal_after_string(proposal, "expected_target_digest")?;
            if target != materialized_reference
                || expected_target_digest != artifact.content_digest
                || required_proposal_after_string(proposal, "contentDigest")? != restore_digest
                || proposal.affected_path != format!("filesystem.{target}")
            {
                return Err("canonical_artifact_undo_projection_identity_mismatch".into());
            }
            canonical_store
                .lock()
                .await
                .bind_artifact_replacement_undo(
                    artifact_id,
                    &proposal.id,
                    source,
                    target,
                    restore_digest,
                    expected_target_digest,
                )
                .map_err(|error| error.to_string())?;
        }
        _ => return Err("canonical_artifact_undo_operation_invalid".into()),
    }
    Ok(true)
}

async fn reconcile_orphaned_artifact_undo_checkpoints_with_state(
    state: &Arc<AppState>,
    limit: i64,
) -> Result<usize, String> {
    let proposals = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await
        .list_active_review_proposals(limit.clamp(1, 200))
        .map_err(|error| runtime_proposal_store_error(state, error))?;
    let mut repaired = 0usize;
    for proposal in proposals {
        if bind_orphaned_artifact_undo_checkpoint(state, &proposal).await? {
            repaired += 1;
        }
    }
    Ok(repaired)
}

async fn project_confirmed_effect_projection_only(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    claim_id: &str,
) -> Result<AgentProposal, String> {
    let mut accepted = proposal.clone();
    accepted.accept();
    canonicalize_proposal_affected_path(&mut accepted);
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await;
    if store
        .project_confirmed_effect(&accepted, claim_id)
        .map_err(|error| error.to_string())?
    {
        return Ok(accepted);
    }

    // A concurrent reconciler may have won the exact same projection. Treat that as
    // idempotent success only when both the read model and dispatch receipt agree.
    let stored = store
        .get_proposal(&proposal.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "confirmed Proposal projection target disappeared".to_string())?;
    let dispatch_state = store
        .dispatch_state(&proposal.id)
        .map_err(|error| error.to_string())?;
    if stored.status == ProposalStatus::Accepted && dispatch_state.as_deref() == Some("confirmed") {
        Ok(stored)
    } else {
        Err("confirmed effect remains projection_pending; no effect was replayed".into())
    }
}

struct ArtifactEffectRecoveryRecord {
    proposal_id: String,
    dispatch_claim_id: String,
    target_reference_digest: String,
    content_digest: String,
    byte_size: u64,
    media_type: String,
    state: CanonicalArtifactEffectState,
}

async fn artifact_effect_safe_paths(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<Vec<String>, String> {
    let Some(artifact_id) = proposal
        .after
        .get("undoOfArtifactId")
        .and_then(Value::as_str)
    else {
        return crate::canonical_work_runtime::artifact_safe_paths_for_proposal(state, proposal)
            .await;
    };
    let canonical_store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let (task_id, source_run_id) = {
        let store = canonical_store.lock().await;
        let artifact = store
            .load_artifact(artifact_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "canonical_artifact_undo_owner_missing".to_string())?;
        let undo = store
            .load_artifact_undo(artifact_id)
            .map_err(|error| error.to_string())?
            .filter(|undo| undo.proposal_id == proposal.id)
            .ok_or_else(|| "canonical_artifact_undo_checkpoint_missing".to_string())?;
        if undo.artifact_id != artifact.id {
            return Err("canonical_artifact_undo_identity_mismatch".into());
        }
        let task = store
            .load_task_snapshot(&artifact.task_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "canonical_artifact_task_missing".to_string())?;
        let source_run_id = task
            .items
            .iter()
            .find(|item| item.id == artifact.source_item_id)
            .map(|item| item.run_id.clone())
            .ok_or_else(|| "canonical_artifact_source_item_missing".to_string())?;
        (artifact.task_id, source_run_id)
    };
    crate::canonical_work_runtime::artifact_materialized_safe_paths_for_task_run(
        state,
        &task_id,
        &source_run_id,
    )
    .await
}

async fn reconcile_artifact_effects_with_state(
    state: &Arc<AppState>,
    limit: i64,
) -> Result<(usize, bool), String> {
    let bounded_limit = limit.clamp(1, 200);
    let mut records = if let Some(store) = state.canonical_task_runtime_store.as_ref() {
        store
            .lock()
            .await
            .list_artifact_effects_for_reconciliation(bounded_limit as u64)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|record| ArtifactEffectRecoveryRecord {
                proposal_id: record.proposal_id,
                dispatch_claim_id: record.dispatch_claim_id,
                target_reference_digest: record.target_reference_digest,
                content_digest: record.content_digest,
                byte_size: record.byte_size,
                media_type: record.media_type,
                state: record.state,
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    records.sort_by(|left, right| left.proposal_id.cmp(&right.proposal_id));
    records.truncate(bounded_limit as usize);
    let backlog_may_remain = records.len() == bounded_limit as usize;
    let mut reconciled = 0usize;
    for record in records {
        let proposal = {
            let store = state
                .proposal_store
                .as_ref()
                .ok_or_else(proposal_store_missing)?
                .lock()
                .await;
            store
                .get_proposal(&record.proposal_id)
                .map_err(|error| runtime_proposal_store_error(state, error))?
        };
        let Some(proposal) = proposal else {
            return Err("artifact reconciliation proposal disappeared".into());
        };
        if proposal.proposal_type != ProposalType::ExternalWriteAction {
            persist_artifact_unknown(
                state,
                &record.proposal_id,
                &record.dispatch_claim_id,
                "artifact_proposal_type_mismatch",
            )
            .await?;
            reconciled += 1;
            continue;
        }
        let safe_paths = match artifact_effect_safe_paths(state, &proposal).await {
            Ok(safe_paths) => safe_paths,
            Err(_) => {
                persist_artifact_unknown(
                    state,
                    &record.proposal_id,
                    &record.dispatch_claim_id,
                    "artifact_recovery_scope_binding_failed",
                )
                .await?;
                reconciled += 1;
                continue;
            }
        };
        let operation = proposal
            .after
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or("propose_write");
        if matches!(operation, "move" | "trash" | "restore") {
            let Some(source) = proposal.after.get("source_path").and_then(Value::as_str) else {
                persist_artifact_unknown(
                    state,
                    &record.proposal_id,
                    &record.dispatch_claim_id,
                    "artifact_move_recovery_source_missing",
                )
                .await?;
                reconciled += 1;
                continue;
            };
            let Some(target) = proposal.after.get("target_path").and_then(Value::as_str) else {
                persist_artifact_unknown(
                    state,
                    &record.proposal_id,
                    &record.dispatch_claim_id,
                    "artifact_move_recovery_target_missing",
                )
                .await?;
                reconciled += 1;
                continue;
            };
            let inspection =
                inspect_artifact_move(source, target, &record.content_digest, &safe_paths);
            let (target_reference_digest, observation) = match inspection {
                Ok(value) => value,
                Err(_) => {
                    persist_artifact_unknown(
                        state,
                        &record.proposal_id,
                        &record.dispatch_claim_id,
                        "artifact_move_recovery_preflight_failed",
                    )
                    .await?;
                    reconciled += 1;
                    continue;
                }
            };
            let binding_matches = canonical_move_effect_target_digest_matches(
                state,
                &proposal,
                &target_reference_digest,
                &record.target_reference_digest,
            )
            .await
            .unwrap_or(false);
            if !binding_matches {
                persist_artifact_unknown(
                    state,
                    &record.proposal_id,
                    &record.dispatch_claim_id,
                    "artifact_move_recovery_binding_mismatch",
                )
                .await?;
                reconciled += 1;
                continue;
            }
            match observation {
                ArtifactFilesystemObservation::Confirmed {
                    observed_content_digest,
                } => {
                    persist_artifact_confirmed(
                        state,
                        &record.proposal_id,
                        &record.dispatch_claim_id,
                        &observed_content_digest,
                    )
                    .await?;
                }
                ArtifactFilesystemObservation::NoStagedOrFinalBytes
                    if record.state == CanonicalArtifactEffectState::Prepared =>
                {
                    persist_artifact_failed_before_effect(
                        state,
                        &record.proposal_id,
                        &record.dispatch_claim_id,
                        "artifact_move_recovery_proved_no_effect",
                    )
                    .await?;
                }
                ArtifactFilesystemObservation::NoStagedOrFinalBytes
                | ArtifactFilesystemObservation::Staged
                | ArtifactFilesystemObservation::Unknown { .. } => {
                    persist_artifact_unknown(
                        state,
                        &record.proposal_id,
                        &record.dispatch_claim_id,
                        "artifact_move_recovery_state_ambiguous",
                    )
                    .await?;
                }
            }
            reconciled += 1;
            continue;
        }
        let resolved = match if operation == "restore_snapshot"
            && proposal.after.get("undoOfArtifactId").is_some()
        {
            resolve_artifact_undo_restore_input(state, &proposal).await
        } else {
            resolve_artifact_effect_input(state, &proposal).await
        } {
            Ok(resolved) => resolved,
            Err(_) => {
                persist_artifact_unknown(
                    state,
                    &record.proposal_id,
                    &record.dispatch_claim_id,
                    "artifact_recovery_canonical_source_invalid",
                )
                .await?;
                reconciled += 1;
                continue;
            }
        };
        let prepared = match prepare_artifact_materialization_with_precondition_for_artifact_bytes(
            &resolved.artifact_id,
            &record.proposal_id,
            &record.dispatch_claim_id,
            &resolved.path,
            &resolved.content,
            &safe_paths,
            resolved.target_precondition,
        ) {
            Ok(prepared) => prepared,
            Err(_) => {
                persist_artifact_unknown(
                    state,
                    &record.proposal_id,
                    &record.dispatch_claim_id,
                    "artifact_recovery_preflight_failed",
                )
                .await?;
                reconciled += 1;
                continue;
            }
        };
        if prepared.target_reference_digest != record.target_reference_digest
            || prepared.content_digest != record.content_digest
            || prepared.byte_size != record.byte_size
            || prepared.media_type != record.media_type
        {
            persist_artifact_unknown(
                state,
                &record.proposal_id,
                &record.dispatch_claim_id,
                "artifact_recovery_binding_mismatch",
            )
            .await?;
            reconciled += 1;
            continue;
        }
        let inspection_prepared = prepared.clone();
        let observation =
            tokio::task::spawn_blocking(move || inspect_artifact_filesystem(&inspection_prepared))
                .await
                .map_err(|_| "artifact_recovery_inspection_worker_failed".to_string())?;
        match observation {
            ArtifactFilesystemObservation::Confirmed {
                observed_content_digest,
            } => {
                persist_artifact_confirmed(
                    state,
                    &record.proposal_id,
                    &record.dispatch_claim_id,
                    &observed_content_digest,
                )
                .await?;
                reconciled += 1;
            }
            ArtifactFilesystemObservation::Staged => {
                let commit_prepared = prepared.clone();
                let commit_safe_paths = safe_paths.clone();
                match tokio::task::spawn_blocking(move || {
                    commit_staged_artifact(&commit_prepared, &commit_safe_paths)
                })
                .await
                {
                    Ok(Ok(observed_content_digest)) => {
                        persist_artifact_confirmed(
                            state,
                            &record.proposal_id,
                            &record.dispatch_claim_id,
                            &observed_content_digest,
                        )
                        .await?;
                    }
                    Ok(Err(failure)) => {
                        persist_artifact_unknown(
                            state,
                            &record.proposal_id,
                            &record.dispatch_claim_id,
                            failure.code(),
                        )
                        .await?;
                    }
                    Err(_) => {
                        persist_artifact_unknown(
                            state,
                            &record.proposal_id,
                            &record.dispatch_claim_id,
                            "artifact_recovery_commit_worker_unknown",
                        )
                        .await?;
                    }
                }
                reconciled += 1;
            }
            ArtifactFilesystemObservation::NoStagedOrFinalBytes
                if record.state == CanonicalArtifactEffectState::Prepared =>
            {
                persist_artifact_failed_before_effect(
                    state,
                    &record.proposal_id,
                    &record.dispatch_claim_id,
                    "artifact_recovery_proved_no_effect",
                )
                .await?;
                reconciled += 1;
            }
            ArtifactFilesystemObservation::NoStagedOrFinalBytes => {
                persist_artifact_unknown(
                    state,
                    &record.proposal_id,
                    &record.dispatch_claim_id,
                    "artifact_recovery_bytes_missing_after_stage",
                )
                .await?;
            }
            ArtifactFilesystemObservation::Unknown { reason_code } => {
                persist_artifact_unknown(
                    state,
                    &record.proposal_id,
                    &record.dispatch_claim_id,
                    &reason_code,
                )
                .await?;
            }
        }
    }
    Ok((reconciled, backlog_may_remain))
}

async fn release_startup_artifact_claims_proven_before_effect(
    state: &Arc<AppState>,
    limit: i64,
) -> Result<(usize, bool), String> {
    let bounded_limit = limit.clamp(1, 200);
    let claims = {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        store
            .list_claimed_external_writes_without_artifact_intent(bounded_limit as usize)
            .map_err(|error| runtime_proposal_store_error(state, error))?
    };
    let backlog_may_remain = claims.len() == bounded_limit as usize;
    let mut released = 0usize;
    for (proposal_id, claim_id) in claims {
        let canonical_effect = match state.canonical_task_runtime_store.as_ref() {
            Some(store) => store
                .lock()
                .await
                .load_artifact_effect(&proposal_id)
                .map_err(|error| error.to_string())?,
            None => None,
        };
        if let Some(effect) = canonical_effect {
            use openlife_core::task_runtime::CanonicalArtifactEffectState;
            match effect.state {
                CanonicalArtifactEffectState::Prepared | CanonicalArtifactEffectState::Staged => {
                    // The canonical recovery pass below owns inspection of these
                    // in-flight bytes. They are not evidence of a pre-effect claim.
                    continue;
                }
                CanonicalArtifactEffectState::Confirmed => {
                    ensure_effect_dispatch_projection_pending(state, &proposal_id, &claim_id)
                        .await?;
                    continue;
                }
                CanonicalArtifactEffectState::FailedBeforeEffect => {
                    let store = state
                        .proposal_store
                        .as_ref()
                        .ok_or_else(proposal_store_missing)?
                        .lock()
                        .await;
                    if store
                        .mark_dispatch_failed_before_effect(
                            &proposal_id,
                            &claim_id,
                            effect
                                .error_code
                                .as_deref()
                                .unwrap_or("canonical_artifact_failed_before_effect"),
                        )
                        .map_err(|error| runtime_proposal_store_error(state, error))?
                    {
                        released += 1;
                    }
                    continue;
                }
                CanonicalArtifactEffectState::EffectUnknown => {
                    let store = state
                        .proposal_store
                        .as_ref()
                        .ok_or_else(proposal_store_missing)?
                        .lock()
                        .await;
                    if store
                        .mark_dispatch_unknown(
                            &proposal_id,
                            &claim_id,
                            effect
                                .error_code
                                .as_deref()
                                .unwrap_or("canonical_artifact_effect_unknown"),
                        )
                        .map_err(|error| runtime_proposal_store_error(state, error))?
                    {
                        released += 1;
                    }
                    continue;
                }
            }
        }
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        if store
            .mark_dispatch_failed_before_effect(
                &proposal_id,
                &claim_id,
                "startup_artifact_claim_without_prepared_intent",
            )
            .map_err(|error| runtime_proposal_store_error(state, error))?
        {
            released += 1;
        }
    }
    Ok((released, backlog_may_remain))
}

pub(crate) async fn reconcile_durable_proposal_projections_with_state(
    state: &Arc<AppState>,
    limit: i64,
) -> Result<ProposalReconciliationReport, String> {
    reconcile_durable_proposal_projections_inner(
        state,
        limit,
        ProposalReconciliationAdmission::ProductEffects,
    )
    .await
}

pub(crate) async fn reconcile_startup_durable_proposal_projections_with_state(
    state: &Arc<AppState>,
    limit: i64,
) -> Result<ProposalReconciliationReport, String> {
    reconcile_durable_proposal_projections_inner(
        state,
        limit,
        ProposalReconciliationAdmission::StartupInternal,
    )
    .await
}

async fn reconcile_durable_proposal_projections_inner(
    state: &Arc<AppState>,
    limit: i64,
    admission: ProposalReconciliationAdmission,
) -> Result<ProposalReconciliationReport, String> {
    require_proposal_reconciliation_admission(state, admission)?;
    let bounded_limit = limit.clamp(1, 200);
    let artifact_undo_checkpoints_repaired =
        reconcile_orphaned_artifact_undo_checkpoints_with_state(state, bounded_limit).await?;
    let (orphaned_claims_released, orphaned_claim_backlog_may_remain) =
        if matches!(admission, ProposalReconciliationAdmission::StartupInternal) {
            release_startup_artifact_claims_proven_before_effect(state, bounded_limit).await?
        } else {
            (0, false)
        };
    let (reconciled_direct_artifact_effects, direct_artifact_backlog_may_remain) =
        crate::canonical_work_runtime::reconcile_direct_artifact_effects_with_state(
            state,
            bounded_limit as u64,
        )
        .await?;
    let (ambiguous_action_effects_marked_unknown, action_effect_backlog_may_remain) = (0, false);
    let (reconciled_artifact_effects, artifact_effect_backlog_may_remain) =
        reconcile_artifact_effects_with_state(state, bounded_limit).await?;
    let artifact_effects_reconciled = orphaned_claims_released
        .saturating_add(reconciled_artifact_effects)
        .saturating_add(reconciled_direct_artifact_effects);
    let artifact_backlog_may_remain = orphaned_claim_backlog_may_remain
        || artifact_effect_backlog_may_remain
        || direct_artifact_backlog_may_remain;
    let confirmed_projection_pending = {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        store
            .list_confirmed_projection_pending(bounded_limit)
            .map_err(|error| runtime_proposal_store_error(state, error))?
    };

    let mut report = ProposalReconciliationReport {
        artifact_undo_checkpoints_repaired,
        artifact_effects_reconciled,
        ambiguous_action_effects_marked_unknown,
        action_effect_backlog_may_remain,
        artifact_backlog_may_remain,
        projection_backlog_may_remain: confirmed_projection_pending.len() == bounded_limit as usize,
        ..ProposalReconciliationReport::default()
    };
    for (proposal, claim_id) in confirmed_projection_pending {
        let artifact_receipt = confirmed_artifact_receipt_from_store(state, &proposal).await?;
        let mut canonical_warnings = Vec::new();
        let canonical_projection = project_confirmed_canonical_work_artifact_status(
            state,
            &proposal,
            artifact_receipt.as_ref(),
            &mut canonical_warnings,
        )
        .await;
        if canonical_projection == "reconciliation_required" {
            return Err(canonical_warnings
                .into_iter()
                .next()
                .unwrap_or_else(|| "canonical Work Artifact reconciliation failed".into()));
        }
        let _accepted =
            project_confirmed_effect_projection_only(state, &proposal, &claim_id).await?;
        report.proposal_projections_repaired += 1;
    }
    Ok(report)
}

async fn confirmed_artifact_receipt_from_store(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<Option<ArtifactMaterializationReceipt>, String> {
    if proposal.proposal_type != ProposalType::ExternalWriteAction {
        return Ok(None);
    }
    if canonical_work_artifact_id(proposal).is_none() {
        return Err("noncanonical_artifact_effect_retired".into());
    }
    let canonical_record = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?
        .lock()
        .await
        .load_artifact_effect(&proposal.id)
        .map_err(|error| error.to_string())?;
    let effect = if let Some(record) =
        canonical_record.filter(|record| record.state == CanonicalArtifactEffectState::Confirmed)
    {
        (
            record.dispatch_claim_id,
            record.target_reference_digest,
            record.content_digest,
            record.byte_size,
            record.media_type,
            record.observed_content_digest,
        )
    } else {
        return Ok(None);
    };
    let (
        dispatch_claim_id,
        expected_target_reference_digest,
        content_digest,
        byte_size,
        media_type,
        observed_content_digest,
    ) = effect;
    let operation = proposal
        .after
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("propose_write");
    if matches!(operation, "move" | "trash" | "restore") {
        let source = proposal
            .after
            .get("source_path")
            .and_then(Value::as_str)
            .ok_or_else(|| "confirmed artifact move lost source_path".to_string())?;
        let target = proposal
            .after
            .get("target_path")
            .and_then(Value::as_str)
            .ok_or_else(|| "confirmed artifact move lost target_path".to_string())?;
        let safe_paths = artifact_effect_safe_paths(state, proposal).await?;
        let (move_reference_digest, observation) =
            inspect_artifact_move(source, target, &content_digest, &safe_paths)?;
        let expected_effect_digest_matches = canonical_move_effect_target_digest_matches(
            state,
            proposal,
            &move_reference_digest,
            &expected_target_reference_digest,
        )
        .await?;
        if !expected_effect_digest_matches
            || !matches!(observation, ArtifactFilesystemObservation::Confirmed { .. })
        {
            return Err("confirmed artifact move receipt binding mismatch".into());
        }
        let observed = observed_content_digest
            .filter(|digest| digest == &content_digest)
            .ok_or_else(|| "confirmed artifact move observed digest missing".to_string())?;
        let mut receipt = confirmed_move_receipt_from_paths(
            &proposal.id,
            target,
            move_reference_digest,
            observed,
            byte_size,
            media_type,
        );
        receipt.artifact_id = artifact_id_for_proposal(proposal);
        return Ok(Some(receipt));
    }
    let resolved =
        if operation == "restore_snapshot" && proposal.after.get("undoOfArtifactId").is_some() {
            resolve_artifact_undo_restore_input(state, proposal).await?
        } else {
            resolve_artifact_effect_input(state, proposal).await?
        };
    let path = resolved.path.as_str();
    let content = resolved.content.as_slice();
    let safe_paths = artifact_effect_safe_paths(state, proposal).await?;
    let prepared = prepare_artifact_materialization_with_precondition_for_artifact_bytes(
        &resolved.artifact_id,
        &proposal.id,
        &dispatch_claim_id,
        path,
        content,
        &safe_paths,
        resolved.target_precondition,
    )?;
    if prepared.target_reference_digest != expected_target_reference_digest
        || prepared.content_digest != content_digest
        || prepared.byte_size != byte_size
        || prepared.media_type != media_type
    {
        return Err("confirmed artifact receipt binding mismatch".into());
    }
    let observed = observed_content_digest
        .filter(|digest| digest == &content_digest)
        .ok_or_else(|| "confirmed artifact observed digest missing".to_string())?;
    Ok(Some(confirmed_artifact_receipt(&prepared, observed)))
}

async fn canonical_move_effect_target_digest_matches(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    move_reference_digest: &str,
    expected_effect_digest: &str,
) -> Result<bool, String> {
    if proposal.after.get("undoOfArtifactId").is_some() {
        return Ok(move_reference_digest == expected_effect_digest);
    }
    let artifact_id = artifact_id_for_proposal(proposal);
    state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?
        .lock()
        .await
        .load_artifact(&artifact_id)
        .map_err(|error| error.to_string())
        .map(|artifact| {
            artifact
                .is_some_and(|artifact| artifact.target_reference_digest == expected_effect_digest)
        })
}

async fn project_confirmed_canonical_work_artifact(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    receipt: &ArtifactMaterializationReceipt,
) -> Result<Option<openlife_core::task_runtime::CanonicalArtifactRecord>, String> {
    let Some(expected_artifact_id) = canonical_work_artifact_id(proposal) else {
        return Ok(None);
    };
    if receipt.artifact_id != expected_artifact_id
        || receipt.proposal_id != proposal.id
        || receipt.content_digest != receipt.observed_content_digest
    {
        return Err("canonical Work Artifact receipt binding mismatch".into());
    }
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let operation = proposal
        .after
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("propose_write");
    if proposal.after.get("undoOfArtifactId").is_some()
        && matches!(operation, "trash" | "restore" | "restore_snapshot")
    {
        let store = store.lock().await;
        store
            .confirm_artifact_undone(
                &proposal.id,
                &receipt.target_reference,
                &receipt.observed_content_digest,
            )
            .map_err(|error| {
                format!("canonical Work Artifact Undo confirmation failed: {error}")
            })?;
        store
            .load_artifact(expected_artifact_id)
            .map_err(|error| format!("canonical Work Artifact reload failed: {error}"))
    } else {
        store
            .lock()
            .await
            .confirm_artifact_materialized(
                &proposal.id,
                &receipt.target_reference,
                &receipt.observed_content_digest,
            )
            .map(Some)
            .map_err(|error| format!("canonical Work Artifact confirmation failed: {error}"))
    }
}

async fn project_confirmed_canonical_work_artifact_status(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    receipt: Option<&ArtifactMaterializationReceipt>,
    warnings: &mut Vec<String>,
) -> &'static str {
    let Some(receipt) = receipt else {
        return "not_applicable";
    };
    match project_confirmed_canonical_work_artifact(state, proposal, receipt).await {
        Ok(Some(_)) => "confirmed",
        Ok(None) => "not_applicable",
        Err(error) => {
            warnings.push(format!(
                "Artifact 已确认，但 canonical Task Runtime 投影仍等待 reconciliation: {error}"
            ));
            "reconciliation_required"
        }
    }
}

async fn mark_canonical_work_artifact_effect_failure(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    reason_code: &str,
    effect_unknown: bool,
) {
    if canonical_work_artifact_id(proposal).is_none() {
        return;
    }
    let Some(store) = state.canonical_task_runtime_store.as_ref() else {
        log::warn!(
            "[CanonicalTaskRuntime] Work Artifact failure projection unavailable: {}",
            reason_code
        );
        return;
    };
    let artifact_undo = proposal.after.get("undoOfArtifactId").is_some();
    let result = if artifact_undo {
        store
            .lock()
            .await
            .mark_artifact_undo_terminal(
                &proposal.id,
                if effect_unknown {
                    "effect_unknown"
                } else {
                    "failed"
                },
                reason_code,
            )
            .map(|_| ())
    } else if effect_unknown {
        store
            .lock()
            .await
            .mark_artifact_effect_unknown(&proposal.id, reason_code)
    } else {
        store
            .lock()
            .await
            .mark_artifact_failed_before_effect(&proposal.id, reason_code)
    };
    if let Err(error) = result {
        log::warn!(
            "[CanonicalTaskRuntime] Work Artifact failure projection failed: {}",
            error
        );
    }
}

async fn project_canonical_work_review_rejection(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<(), String> {
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    if canonical_work_tool_review_identity(proposal).is_some() {
        let projected = store
            .lock()
            .await
            .mark_tool_review_rejected(&proposal.id)
            .map_err(|error| format!("canonical Work tool Review rejection failed: {error}"))?;
        if projected.is_some() {
            let registry = state
                .main_chat_runtime_state
                .lock()
                .await
                .work_review_decision_registry
                .clone();
            let _ = registry.resolve(&proposal.id, crate::state::WorkReviewDecision::Rejected)?;
        }
        return Ok(());
    }
    if canonical_work_artifact_id(proposal).is_none() {
        return Ok(());
    }
    if proposal.after.get("undoOfArtifactId").is_some() {
        return store
            .lock()
            .await
            .mark_artifact_undo_terminal(&proposal.id, "failed", "artifact_undo_review_rejected")
            .map(|_| ())
            .map_err(|error| format!("canonical Work Undo rejection projection failed: {error}"));
    }
    store
        .lock()
        .await
        .mark_artifact_review_rejected(&proposal.id)
        .map(|_| ())
        .map_err(|error| format!("canonical Work Review rejection projection failed: {error}"))
}

fn canonical_work_tool_review_identity(proposal: &AgentProposal) -> Option<(&str, &str)> {
    if proposal.proposal_type != ProposalType::ToolPermission
        || proposal.source != ProposalSource::ChatConversation
    {
        return None;
    }
    let pending = proposal.after.get("pending_action_identity")?;
    let task_id = pending.get("taskId").and_then(Value::as_str)?;
    let run_id = pending.get("runId").and_then(Value::as_str)?;
    (proposal.source_detail.as_deref() == Some(task_id)
        && proposal.run_id.as_deref() == Some(run_id))
    .then_some((task_id, run_id))
}

async fn require_active_canonical_work_tool_review(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<(), String> {
    let Some((task_id, run_id)) = canonical_work_tool_review_identity(proposal) else {
        return Ok(());
    };
    let scope_digest = canonical_work_tool_review_scope_digest(&proposal.after)?;
    let checkpoint = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?
        .lock()
        .await
        .load_tool_review_checkpoint(&proposal.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_work_tool_review_checkpoint_missing".to_string())?;
    if checkpoint.task_id != task_id
        || checkpoint.run_id != run_id
        || checkpoint.scope_digest != scope_digest
        || checkpoint.status != "waiting"
    {
        return Err("canonical_work_tool_review_checkpoint_not_active".into());
    }
    Ok(())
}

async fn project_canonical_work_tool_review_acceptance(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<bool, String> {
    if canonical_work_tool_review_identity(proposal).is_none() {
        return Ok(false);
    }
    let scope_digest = canonical_work_tool_review_scope_digest(&proposal.after)?;
    let registry = state
        .main_chat_runtime_state
        .lock()
        .await
        .work_review_decision_registry
        .clone();
    let live_continuation_available = registry.has_waiter(&proposal.id)?;
    let projected = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?
        .lock()
        .await
        .mark_tool_review_accepted(&proposal.id, &scope_digest, live_continuation_available)
        .map_err(|error| format!("canonical Work tool Review acceptance failed: {error}"))?;
    if projected.is_some() {
        if live_continuation_available
            && !registry.resolve(&proposal.id, crate::state::WorkReviewDecision::Accepted)?
        {
            state
                .canonical_task_runtime_store
                .as_ref()
                .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?
                .lock()
                .await
                .terminalize_general_run(
                    proposal
                        .source_detail
                        .as_deref()
                        .ok_or_else(|| "canonical_work_tool_review_task_missing".to_string())?,
                    proposal
                        .run_id
                        .as_deref()
                        .ok_or_else(|| "canonical_work_tool_review_run_missing".to_string())?,
                    openlife_core::task_runtime::CanonicalTaskStatus::Interrupted,
                )
                .map_err(|error| {
                    format!("canonical Work tool Review wake race reconciliation failed: {error}")
                })?;
        }
        return Ok(true);
    }
    Ok(false)
}

fn canonical_work_tool_review_scope_digest(after: &Value) -> Result<String, String> {
    match tool_permission_scope_kind(after)? {
        ToolPermissionScopeKind::ActionBound => {
            Ok(action_bound_tool_permission_scope(after)?.binding_digest())
        }
        ToolPermissionScopeKind::NetworkPolicy => {
            let digest = after
                .get("canonical_scope")
                .and_then(|scope| scope.get("scope_digest"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    "canonical Work network Review missing canonical scope digest".to_string()
                })?;
            let Some(hex) = digest.strip_prefix("sha256:") else {
                return Err("canonical Work network Review scope digest is invalid".into());
            };
            if hex.len() != 64
                || !hex
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err("canonical Work network Review scope digest is invalid".into());
            }
            Ok(digest.to_string())
        }
        ToolPermissionScopeKind::ManifestPolicy => {
            Err("canonical Work Review cannot bind a reusable manifest policy".into())
        }
    }
}

fn confirmed_effect_reconciliation_response(
    proposal: &AgentProposal,
    projection_confirmed: bool,
    warnings: Vec<String>,
    artifact_materialization: Option<ArtifactMaterializationReceipt>,
) -> Value {
    let mut response = serde_json::json!({
        "success": true,
        "patch_result": patch_result_for_proposal(
            proposal,
            true,
            if projection_confirmed {
                "confirmed_effect_projection_reconciled"
            } else {
                "confirmed_effect_projection_pending"
            },
            None,
        ),
        "effect_status": "confirmed",
        "proposal_projection_status": if projection_confirmed {
            "confirmed"
        } else {
            "reconciliation_required"
        },
        "warnings": warnings,
    });
    if let Some(receipt) = artifact_materialization {
        response["artifactMaterialization"] =
            serde_json::to_value(receipt).unwrap_or(serde_json::Value::Null);
    }
    response
}

fn patch_result_for_proposal(
    proposal: &AgentProposal,
    success: bool,
    operation: &str,
    error: Option<String>,
) -> openlife_core::life_model::patch::PatchApplyResult {
    openlife_core::life_model::patch::PatchApplyResult {
        patch_id: proposal.id.clone(),
        success,
        path: proposal.affected_path.clone(),
        operation: operation.to_string(),
        error,
    }
}

enum ArtifactApplyOutcome {
    Confirmed {
        patch_result: openlife_core::life_model::patch::PatchApplyResult,
        receipt: Box<ArtifactMaterializationReceipt>,
    },
    FailedBeforeEffect(String),
    Unknown(String),
}

async fn persist_artifact_failed_before_effect(
    state: &Arc<AppState>,
    proposal_id: &str,
    claim_id: &str,
    error_code: &str,
) -> Result<(), String> {
    let canonical_store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let store = canonical_store.lock().await;
    let artifact_exists = store
        .load_artifact_by_proposal(proposal_id)
        .map_err(|error| error.to_string())?
        .is_some();
    if !artifact_exists {
        return Err("noncanonical_artifact_effect_retired".into());
    }
    let effect_exists = store
        .load_artifact_effect(proposal_id)
        .map_err(|error| error.to_string())?
        .is_some();
    if effect_exists {
        if !store
            .finish_artifact_effect_failed_before_effect(proposal_id, claim_id, error_code)
            .map_err(|error| error.to_string())?
        {
            return Err("canonical_artifact_failed_before_effect_cas_lost".into());
        }
        return Ok(());
    }
    drop(store);
    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await;
    if !proposal_store
        .mark_dispatch_failed_before_effect(proposal_id, claim_id, error_code)
        .map_err(|error| runtime_proposal_store_error(state, error))?
    {
        return Err("artifact_preflight_failure_receipt_cas_lost".into());
    }
    Ok(())
}

async fn persist_artifact_unknown(
    state: &Arc<AppState>,
    proposal_id: &str,
    claim_id: &str,
    error_code: &str,
) -> Result<(), String> {
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?
        .lock()
        .await;
    if !store
        .finish_artifact_effect_unknown(proposal_id, claim_id, error_code)
        .map_err(|error| error.to_string())?
    {
        return Err("canonical_artifact_unknown_receipt_cas_lost".into());
    }
    Ok(())
}

async fn persist_artifact_confirmed(
    state: &Arc<AppState>,
    proposal_id: &str,
    claim_id: &str,
    observed_content_digest: &str,
) -> Result<(), String> {
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?
        .lock()
        .await;
    if !store
        .finish_artifact_effect_confirmed(proposal_id, claim_id, observed_content_digest)
        .map_err(|error| error.to_string())?
    {
        return Err("canonical_artifact_confirmed_receipt_cas_lost".into());
    }
    drop(store);
    if !ensure_effect_dispatch_projection_pending(state, proposal_id, claim_id).await? {
        return Err("canonical_artifact_dispatch_projection_receipt_cas_lost".into());
    }
    Ok(())
}

async fn ensure_effect_dispatch_projection_pending(
    state: &Arc<AppState>,
    proposal_id: &str,
    claim_id: &str,
) -> Result<bool, String> {
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await;
    let dispatch_state = store
        .dispatch_state(proposal_id)
        .map_err(|error| runtime_proposal_store_error(state, error))?;
    match dispatch_state.as_deref() {
        Some("confirmed_projection_pending" | "confirmed") => Ok(true),
        Some("claimed") => store
            .mark_effect_confirmed_projection_pending(proposal_id, claim_id)
            .map_err(|error| runtime_proposal_store_error(state, error)),
        _ => Ok(false),
    }
}

struct ResolvedArtifactEffectInput {
    artifact_id: String,
    path: String,
    content: Vec<u8>,
    target_precondition: ArtifactTargetPrecondition,
}

async fn resolve_artifact_effect_input(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<ResolvedArtifactEffectInput, String> {
    let subject = canonical_artifact_review_subject(proposal)?;
    let artifact_id = subject.artifact_id.as_str();
    let expected_version = subject.artifact_version;
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let store = store.lock().await;
    let artifact = store
        .load_artifact_by_proposal(&proposal.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_artifact_review_checkpoint_missing".to_string())?;
    if artifact.id != artifact_id || artifact.current_version != expected_version {
        return Err("canonical_artifact_review_version_mismatch".into());
    }
    let version = store
        .load_artifact_version(artifact_id, expected_version)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_artifact_version_missing".to_string())?;
    let path = version
        .target_reference
        .ok_or_else(|| "canonical_artifact_target_reference_missing".to_string())?;
    let draft_reference = version
        .draft_reference
        .ok_or_else(|| "canonical_artifact_draft_reference_missing".to_string())?;
    let target_precondition = match (
        version.expected_target_absent,
        version.expected_target_digest,
    ) {
        (Some(true), None) => ArtifactTargetPrecondition::Absent,
        (Some(false), Some(digest)) => ArtifactTargetPrecondition::ContentDigest(digest),
        _ => return Err("canonical_artifact_target_precondition_missing".into()),
    };
    drop(store);
    let bytes = std::fs::read(&draft_reference)
        .map_err(|_| "canonical_artifact_draft_read_failed".to_string())?;
    if bytes.len() > EXTERNAL_WRITE_MAX_SIZE {
        return Err("artifact_content_too_large".into());
    }
    if artifact_content_digest(&bytes) != artifact.content_digest {
        return Err("canonical_artifact_draft_digest_mismatch".into());
    }
    Ok(ResolvedArtifactEffectInput {
        artifact_id: artifact_id.to_string(),
        path,
        content: bytes,
        target_precondition,
    })
}

async fn resolve_artifact_undo_restore_input(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<ResolvedArtifactEffectInput, String> {
    let artifact_id = proposal
        .after
        .get("artifactId")
        .and_then(Value::as_str)
        .ok_or_else(|| "canonical_artifact_undo_artifact_missing".to_string())?;
    let version = proposal
        .after
        .get("artifactVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| "canonical_artifact_undo_version_missing".to_string())?;
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let undo = store
        .lock()
        .await
        .load_artifact_undo_version(artifact_id, version)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_artifact_undo_missing".to_string())?;
    if undo.proposal_id != proposal.id
        || undo.operation
            != openlife_core::task_runtime::CanonicalArtifactUndoOperation::RestoreReplaced
        || proposal.after.get("snapshot_path").and_then(Value::as_str)
            != Some(undo.source_reference.as_str())
        || proposal.after.get("path").and_then(Value::as_str)
            != Some(undo.target_reference.as_str())
        || proposal.after.get("restore_digest").and_then(Value::as_str)
            != Some(undo.content_digest.as_str())
    {
        return Err("canonical_artifact_replacement_undo_identity_mismatch".into());
    }
    let expected_target_digest = undo
        .expected_target_digest
        .ok_or_else(|| "canonical_artifact_replacement_undo_precondition_missing".to_string())?;
    let content = std::fs::read(&undo.source_reference)
        .map_err(|_| "canonical_artifact_pre_change_snapshot_unavailable".to_string())?;
    if content.len() > EXTERNAL_WRITE_MAX_SIZE
        || artifact_content_digest(&content) != undo.content_digest
    {
        return Err("canonical_artifact_pre_change_snapshot_changed".into());
    }
    Ok(ResolvedArtifactEffectInput {
        artifact_id: artifact_id.to_string(),
        path: undo.target_reference,
        content,
        target_precondition: ArtifactTargetPrecondition::ContentDigest(expected_target_digest),
    })
}

async fn apply_external_write_artifact(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    claim_id: &str,
) -> ArtifactApplyOutcome {
    let operation = proposal
        .after
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("propose_write");
    if matches!(operation, "move" | "trash" | "restore") {
        return apply_external_move_artifact(state, proposal, claim_id).await;
    }
    let resolved = match if operation == "restore_snapshot"
        && proposal.after.get("undoOfArtifactId").is_some()
    {
        resolve_artifact_undo_restore_input(state, proposal).await
    } else {
        resolve_artifact_effect_input(state, proposal).await
    } {
        Ok(resolved) => resolved,
        Err(error) => {
            let code = "artifact_canonical_source_invalid";
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(error);
        }
    };
    let path = resolved.path.as_str();
    let content = resolved.content.as_slice();
    if content.len() > EXTERNAL_WRITE_MAX_SIZE {
        let code = "artifact_content_too_large";
        let _ = persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
        return ArtifactApplyOutcome::FailedBeforeEffect(code.into());
    }
    let safe_paths = match artifact_effect_safe_paths(state, proposal).await {
        Ok(safe_paths) => safe_paths,
        Err(error) => {
            let code = "artifact_scope_binding_failed";
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(error);
        }
    };
    let prepared = match prepare_artifact_materialization_with_precondition_for_artifact_bytes(
        &resolved.artifact_id,
        &proposal.id,
        claim_id,
        path,
        content,
        &safe_paths,
        resolved.target_precondition,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            let code = "artifact_preflight_failed";
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(error);
        }
    };
    let expected_hash = if operation == "restore_snapshot"
        && proposal.after.get("undoOfArtifactId").is_some()
    {
        proposal
            .after
            .get("restore_digest")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    } else {
        match canonical_artifact_review_subject(proposal) {
            Ok(subject) => subject.content_digest,
            Err(error) => {
                let code = "artifact_review_subject_invalid";
                let _ = persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code)
                    .await;
                return ArtifactApplyOutcome::FailedBeforeEffect(error);
            }
        }
    };
    if expected_hash != prepared.content_digest {
        let code = "artifact_content_digest_mismatch";
        let _ = persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
        return ArtifactApplyOutcome::FailedBeforeEffect(code.into());
    }
    let prepared_record = match state.canonical_task_runtime_store.as_ref() {
        Some(store) => match store.lock().await.prepare_artifact_effect(
            &proposal.id,
            claim_id,
            &prepared.target_reference_digest,
            &prepared.content_digest,
            prepared.byte_size,
            &prepared.media_type,
        ) {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err("canonical_artifact_effect_owner_missing".into()),
            Err(error) => Err(error.to_string()),
        },
        None => Err("canonical_task_runtime_store_unavailable".into()),
    };
    if let Err(error) = prepared_record {
        let _ = persist_artifact_failed_before_effect(
            state,
            &proposal.id,
            claim_id,
            "artifact_prepare_receipt_failed",
        )
        .await;
        return ArtifactApplyOutcome::FailedBeforeEffect(error);
    }

    let stage_prepared = prepared.clone();
    let stage_content = content.to_vec();
    let stage_result = tokio::task::spawn_blocking(move || {
        stage_artifact_raw_bytes(&stage_prepared, &stage_content)
    })
    .await;
    match stage_result {
        Ok(Ok(())) => {}
        Ok(Err(ArtifactFilesystemFailure::FailedBeforeEffect(code))) => {
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, &code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(code);
        }
        Ok(Err(ArtifactFilesystemFailure::Unknown(code))) => {
            let _ = persist_artifact_unknown(state, &proposal.id, claim_id, &code).await;
            return ArtifactApplyOutcome::Unknown(code);
        }
        Err(_) => {
            let code = "artifact_stage_worker_outcome_unknown";
            let _ = persist_artifact_unknown(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::Unknown(code.into());
        }
    }
    let staged = match state.canonical_task_runtime_store.as_ref() {
        Some(store) => store
            .lock()
            .await
            .mark_artifact_effect_staged(&proposal.id, claim_id)
            .map_err(|error| error.to_string()),
        None => Err("canonical_task_runtime_store_unavailable".into()),
    };
    if !matches!(staged, Ok(true)) {
        let code = "artifact_staged_receipt_unconfirmed";
        let _ = persist_artifact_unknown(state, &proposal.id, claim_id, code).await;
        return ArtifactApplyOutcome::Unknown(code.into());
    }

    let commit_prepared = prepared.clone();
    let commit_safe_paths = safe_paths.clone();
    let commit_result = tokio::task::spawn_blocking(move || {
        commit_staged_artifact(&commit_prepared, &commit_safe_paths)
    })
    .await;
    let observed_digest = match commit_result {
        Ok(Ok(digest)) => digest,
        Ok(Err(ArtifactFilesystemFailure::FailedBeforeEffect(code))) => {
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, &code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(code);
        }
        Ok(Err(ArtifactFilesystemFailure::Unknown(code))) => {
            let _ = persist_artifact_unknown(state, &proposal.id, claim_id, &code).await;
            return ArtifactApplyOutcome::Unknown(code);
        }
        Err(_) => {
            let code = "artifact_commit_worker_outcome_unknown";
            let _ = persist_artifact_unknown(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::Unknown(code.into());
        }
    };
    let confirmed = match state.canonical_task_runtime_store.as_ref() {
        Some(store) => store
            .lock()
            .await
            .finish_artifact_effect_confirmed(&proposal.id, claim_id, &observed_digest)
            .map_err(|error| error.to_string()),
        None => Err("canonical_task_runtime_store_unavailable".into()),
    };
    if !matches!(confirmed, Ok(true)) {
        return ArtifactApplyOutcome::Unknown("artifact_confirmed_receipt_unavailable".into());
    }
    ArtifactApplyOutcome::Confirmed {
        patch_result: patch_result_for_proposal(proposal, true, "artifact_materialized", None),
        receipt: Box::new(confirmed_artifact_receipt(&prepared, observed_digest)),
    }
}

async fn apply_external_move_artifact(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    claim_id: &str,
) -> ArtifactApplyOutcome {
    let source = match proposal.after.get("source_path").and_then(Value::as_str) {
        Some(source) => source,
        None => {
            let code = "artifact_move_source_missing";
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(code.into());
        }
    };
    let target = match proposal.after.get("target_path").and_then(Value::as_str) {
        Some(target) => target,
        None => {
            let code = "artifact_move_target_missing";
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(code.into());
        }
    };
    let expected_digest = proposal
        .after
        .get("source_digest")
        .and_then(Value::as_str)
        .unwrap_or("");
    if expected_digest.is_empty() {
        let code = "artifact_move_digest_missing";
        let _ = persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
        return ArtifactApplyOutcome::FailedBeforeEffect(code.into());
    }
    let safe_paths = match artifact_effect_safe_paths(state, proposal).await {
        Ok(safe_paths) => safe_paths,
        Err(error) => {
            let code = "artifact_move_scope_binding_failed";
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(error);
        }
    };
    let prepared =
        match prepare_artifact_move(&proposal.id, source, target, expected_digest, &safe_paths) {
            Ok(prepared) => prepared,
            Err(error) => {
                let code = "artifact_move_preflight_failed";
                let _ = persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code)
                    .await;
                return ArtifactApplyOutcome::FailedBeforeEffect(error);
            }
        };
    let expected_effect_target_digest = if proposal.after.get("undoOfArtifactId").is_some() {
        prepared.target_reference_digest.clone()
    } else {
        let artifact_id = artifact_id_for_proposal(proposal);
        match state.canonical_task_runtime_store.as_ref() {
            Some(store) => match store.lock().await.load_artifact(&artifact_id) {
                Ok(Some(artifact)) => artifact.target_reference_digest,
                Ok(None) => {
                    return ArtifactApplyOutcome::FailedBeforeEffect(
                        "canonical_artifact_move_owner_missing".into(),
                    )
                }
                Err(error) => return ArtifactApplyOutcome::FailedBeforeEffect(error.to_string()),
            },
            None => {
                return ArtifactApplyOutcome::FailedBeforeEffect(
                    "canonical_task_runtime_store_unavailable".into(),
                )
            }
        }
    };
    let prepared_record = match state.canonical_task_runtime_store.as_ref() {
        Some(store) => match store.lock().await.prepare_artifact_effect(
            &proposal.id,
            claim_id,
            &expected_effect_target_digest,
            &prepared.content_digest,
            prepared.byte_size,
            &prepared.media_type,
        ) {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err("canonical_artifact_effect_owner_missing".into()),
            Err(error) => Err(error.to_string()),
        },
        None => Err("canonical_task_runtime_store_unavailable".into()),
    };
    if let Err(error) = prepared_record {
        let _ = persist_artifact_failed_before_effect(
            state,
            &proposal.id,
            claim_id,
            "artifact_move_prepare_receipt_failed",
        )
        .await;
        return ArtifactApplyOutcome::FailedBeforeEffect(error);
    }
    let move_prepared = prepared.clone();
    let move_safe_paths = safe_paths.clone();
    let move_result =
        tokio::task::spawn_blocking(move || commit_artifact_move(&move_prepared, &move_safe_paths))
            .await;
    let observed_digest = match move_result {
        Ok(Ok(digest)) => digest,
        Ok(Err(ArtifactFilesystemFailure::FailedBeforeEffect(code))) => {
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, &code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(code);
        }
        Ok(Err(ArtifactFilesystemFailure::Unknown(code))) => {
            let _ = persist_artifact_unknown(state, &proposal.id, claim_id, &code).await;
            return ArtifactApplyOutcome::Unknown(code);
        }
        Err(_) => {
            let code = "artifact_move_worker_outcome_unknown";
            let _ = persist_artifact_unknown(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::Unknown(code.into());
        }
    };
    let confirmed = match state.canonical_task_runtime_store.as_ref() {
        Some(store) => store
            .lock()
            .await
            .finish_artifact_effect_confirmed(&proposal.id, claim_id, &observed_digest)
            .map_err(|error| error.to_string()),
        None => Err("canonical_task_runtime_store_unavailable".into()),
    };
    if !matches!(confirmed, Ok(true)) {
        return ArtifactApplyOutcome::Unknown("artifact_move_confirmed_receipt_unavailable".into());
    }
    let mut receipt = confirmed_move_receipt(&prepared, observed_digest);
    receipt.artifact_id = artifact_id_for_proposal(proposal);
    ArtifactApplyOutcome::Confirmed {
        patch_result: patch_result_for_proposal(
            proposal,
            true,
            &format!(
                "artifact_{}_materialized",
                proposal.after["operation"].as_str().unwrap_or("move")
            ),
            None,
        ),
        receipt: Box::new(receipt),
    }
}

pub(crate) fn memory_session_id(after: &Value) -> String {
    after
        .get("session_id")
        .or_else(|| after.get("sessionId"))
        .and_then(Value::as_str)
        .unwrap_or("proposal")
        .to_string()
}

pub(crate) fn memory_source(after: &Value) -> String {
    after
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("proposal")
        .to_string()
}

pub(crate) fn memory_content(after: &Value) -> Result<String, String> {
    if let Some(content) = after.get("content").and_then(Value::as_str) {
        let content = content.trim();
        if !content.is_empty() {
            return Ok(content.to_string());
        }
    }
    if let Some(content) = after.as_str() {
        let content = content.trim();
        if !content.is_empty() {
            return Ok(content.to_string());
        }
    }
    Err("MemoryWrite Proposal 缺少 after.content。".to_string())
}

pub(crate) fn memory_archive_owners(
    after: &Value,
) -> Result<Vec<memory_gateway::CanonicalMemoryOwnerInput>, String> {
    if after.get("chunk_ids").is_some()
        || after.get("chunkIds").is_some()
        || after.get("ids").is_some()
        || after.as_i64().is_some()
        || after.as_array().is_some()
    {
        return Err(
            "MemoryArchive Proposal 不再接受 derived vector row id；必须提供 after.owner 的 stable canonical owner。"
                .to_string(),
        );
    }
    let owner = after.get("owner");
    let owners = after.get("owners");
    if owner.is_some() == owners.is_some() {
        return Err(
            "MemoryArchive Proposal 必须且只能提供 after.owner 或 after.owners。".to_string(),
        );
    }
    let values = if let Some(owner) = owner {
        vec![owner.clone()]
    } else {
        owners
            .and_then(Value::as_array)
            .filter(|owners| !owners.is_empty() && owners.len() <= 200)
            .cloned()
            .ok_or_else(|| {
                "MemoryArchive Proposal after.owners 必须包含 1..=200 个 owner。".to_string()
            })?
    };
    let parsed = values
        .into_iter()
        .map(|value| {
            let owner: memory_gateway::CanonicalMemoryOwnerInput = serde_json::from_value(value)
                .map_err(|_| {
                    "MemoryArchive Proposal owner 必须只包含 ownerKind 和 ownerId。".to_string()
                })?;
            owner.owner().map_err(|error| error.to_string())?;
            Ok(owner)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let unique = parsed
        .iter()
        .map(|owner| format!("{}:{}", owner.owner_kind, owner.owner_id))
        .collect::<std::collections::HashSet<_>>();
    if unique.len() != parsed.len() {
        return Err("MemoryArchive Proposal after.owners 包含重复 owner。".to_string());
    }
    let lifecycle_owned = parsed
        .iter()
        .filter(|owner| owner.owner_kind == "memory_lifecycle")
        .count();
    if lifecycle_owned != 0 && lifecycle_owned != parsed.len() {
        return Err(
            "MemoryArchive Proposal 不能在一个原子批次中混合 lifecycle 与 MemoryStore owner。"
                .to_string(),
        );
    }
    Ok(parsed)
}

pub(crate) fn validate_proposal_payload(
    proposal_type: ProposalType,
    after: &Value,
) -> Result<(), String> {
    match proposal_type {
        ProposalType::LifeModelUpdate => {
            // LifeModel proposals require after to be a non-null value
            if after.is_null() {
                return Err("LifeModel Proposal 的 after 值不能为 null。".to_string());
            }
            Ok(())
        }
        ProposalType::MemoryWrite => {
            let content = after
                .get("content")
                .and_then(Value::as_str)
                .or_else(|| after.as_str());
            match content {
                Some(c) if !c.trim().is_empty() => Ok(()),
                _ => Err("MemoryWrite Proposal 缺少 after.content（非空字符串）。".to_string()),
            }
        }
        ProposalType::MemoryArchive => memory_archive_owners(after).map(|_| ()),
        ProposalType::ToolPermission => {
            let scope_kind = tool_permission_scope_kind(after)?;
            for (field, aliases) in [
                ("tool_name", &["tool_name", "toolName", "name"][..]),
                ("source", &["source"][..]),
                ("risk_level", &["risk_level", "riskLevel"][..]),
                ("action_type", &["action_type", "actionType"][..]),
            ] {
                let value = aliases
                    .iter()
                    .find_map(|alias| tool_permission_scope_field(after, alias));
                if value.is_none() || value.is_some_and(|value| value.trim().is_empty()) {
                    return Err(format!(
                        "ToolPermission Proposal 缺少精确 after.{field}（非空字符串）。"
                    ));
                }
            }
            let (policy, _) = resolve_tool_permission_policy(after)?;
            let manifest_action_type = tool_permission_scope_field(after, "action_type")
                .or_else(|| tool_permission_scope_field(after, "actionType"))
                .expect("validated action_type");
            match scope_kind {
                ToolPermissionScopeKind::ActionBound => {
                    if manifest_action_type == "network" {
                        // This grants one exact Main Chat tool invocation, not destination
                        // access. The executor evaluates network_policy independently before
                        // dispatch and does not consume this grant when that policy blocks.
                        validate_main_chat_action_bound_network_payload(after)?;
                    }
                    if policy != openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce {
                        return Err("Action-bound ToolPermission 必须使用 allow_once。".to_string());
                    }
                    action_bound_tool_permission_scope(after)?;
                }
                ToolPermissionScopeKind::ManifestPolicy => {
                    if manifest_action_type == "network" {
                        return Err(
                            "network ToolPermission 必须使用 network_policy scope。".to_string()
                        );
                    }
                    if policy == openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce {
                        return Err(
                            "manifest_policy ToolPermission 不能使用一次性隐式作用域。".to_string()
                        );
                    }
                    if after
                        .get("permission")
                        .or_else(|| after.get("policy"))
                        .is_none()
                    {
                        return Err(
                            "manifest_policy ToolPermission 必须显式声明 permission/policy。"
                                .to_string(),
                        );
                    }
                }
                ToolPermissionScopeKind::NetworkPolicy => {
                    let decision_id = after
                        .get("canonical_scope")
                        .or_else(|| after.get("canonicalScope"))
                        .and_then(|scope| {
                            scope
                                .get("network_policy_decision_id")
                                .or_else(|| scope.get("networkPolicyDecisionId"))
                        })
                        .and_then(Value::as_str)
                        .filter(|value| !value.trim().is_empty());
                    if manifest_action_type != "network" || decision_id.is_none() {
                        return Err(
                            "network_policy ToolPermission 必须绑定 network action 与精确 decision id。"
                                .to_string(),
                        );
                    }
                    if policy != openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce {
                        return Err(
                            "network_policy ToolPermission 必须使用 allow_once。".to_string()
                        );
                    }
                }
            }
            Ok(())
        }
        ProposalType::ExternalWriteAction => {
            if canonical_work_artifact_id_from_after(after).is_none() {
                return Err("noncanonical_artifact_effect_retired".into());
            }
            if after.get("undoOfArtifactId").is_none() {
                let subject =
                    serde_json::from_value::<CanonicalArtifactReviewSubject>(after.clone())
                        .map_err(|_| "canonical_artifact_review_subject_invalid".to_string())?;
                subject.validate().map_err(|error| error.to_string())?;
                reviewed_artifact_target_precondition(after)?;
                return Ok(());
            }
            let operation = after
                .get("operation")
                .and_then(Value::as_str)
                .unwrap_or("propose_write");
            match operation {
                "move" | "trash" | "restore" => {
                    for field in ["source_path", "target_path", "source_digest"] {
                        if after
                            .get(field)
                            .and_then(Value::as_str)
                            .is_none_or(|value| value.trim().is_empty())
                        {
                            return Err(format!(
                                "ExternalWriteAction {operation} Proposal 缺少 after.{field}（非空字符串）。"
                            ));
                        }
                    }
                    Ok(())
                }
                "create" | "overwrite" | "propose_write" => {
                    let path = after
                        .get("path")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty());
                    if path.is_none() {
                        return Err(
                            "ExternalWriteAction Proposal 缺少 after.path（非空字符串）。"
                                .to_string(),
                        );
                    }
                    reviewed_artifact_target_precondition(after)?;
                    Ok(())
                }
                "restore_snapshot" => {
                    for field in [
                        "snapshot_path",
                        "path",
                        "restore_digest",
                        "expected_target_digest",
                    ] {
                        if after
                            .get(field)
                            .and_then(Value::as_str)
                            .is_none_or(|value| value.trim().is_empty())
                        {
                            return Err(format!(
                                "ExternalWriteAction restore_snapshot Proposal 缺少 after.{field}（非空字符串）。"
                            ));
                        }
                    }
                    Ok(())
                }
                _ => Err(format!(
                    "ExternalWriteAction Proposal 包含不受支持的 operation：{operation}。"
                )),
            }
        }
    }
}

fn validate_proposal_for_acceptance(proposal: &AgentProposal) -> Result<(), String> {
    validate_proposal_payload(proposal.proposal_type, &proposal.after)?;
    if is_legacy_lifemodel_v2_migration(proposal) {
        if proposal.source != ProposalSource::Manual
            || proposal.source_detail.as_deref() != Some("legacy_lifemodel_migration")
            || proposal.base_hash.is_some()
        {
            return Err("lifemodel_v2_migration_proposal_source_mismatch".into());
        }
        let plan = serde_json::from_value::<
            openlife_core::life_model::legacy_migration::LegacyLifeModelMigrationPlanV2,
        >(proposal.after.clone())
        .map_err(|_| "invalid_lifemodel_v2_migration_payload".to_string())?;
        plan.validate_contract()
            .map_err(|error| error.to_string())?;
    } else if is_lifemodel_v2_typed_diff(proposal) {
        let diff = serde_json::from_value::<openlife_core::life_model::v2::LifeModelTypedDiffV2>(
            proposal.after.clone(),
        )
        .map_err(|_| "invalid_lifemodel_v2_typed_diff_payload".to_string())?;
        diff.validate_contract()
            .map_err(|error| error.to_string())?;
        if proposal.base_hash.as_deref() != diff.base_document_digest.as_deref() {
            return Err("lifemodel_v2_typed_diff_proposal_base_mismatch".into());
        }
    }
    if proposal.proposal_type == ProposalType::ToolPermission
        && tool_permission_scope_kind(&proposal.after)? == ToolPermissionScopeKind::ActionBound
        && tool_permission_scope_field(&proposal.after, "action_type") == Some("network")
    {
        let scope = action_bound_tool_permission_scope(&proposal.after)?;
        if proposal.source != ProposalSource::ChatConversation
            || proposal.affected_path
                != format!("tool_permission.{}.{}", scope.source, scope.tool_name)
        {
            return Err(
                "action-bound network ToolPermission 必须来自 Main Chat 的精确产品路径。".into(),
            );
        }
    }
    if proposal.proposal_type == ProposalType::MemoryWrite {
        let content = memory_content(&proposal.after)?;
        openlife_core::agent::MemoryLifecycleAcceptanceInput::from_memory_proposal(
            proposal, content,
        )
        .map_err(|error| format!("MemoryWrite Proposal 审阅契约无效：{error}"))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolPermissionScopeKind {
    ActionBound,
    ManifestPolicy,
    NetworkPolicy,
}

fn tool_permission_scope_kind(after: &Value) -> Result<ToolPermissionScopeKind, String> {
    let label = after
        .get("permission_scope_kind")
        .or_else(|| after.get("permissionScopeKind"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "ToolPermission Proposal 必须显式声明 permission_scope_kind。".to_string()
        })?;
    match label {
        "action_bound" => Ok(ToolPermissionScopeKind::ActionBound),
        "manifest_policy" => Ok(ToolPermissionScopeKind::ManifestPolicy),
        "network_policy" => Ok(ToolPermissionScopeKind::NetworkPolicy),
        _ => Err(format!(
            "ToolPermission Proposal 的 permission_scope_kind '{}' 无效。",
            label
        )),
    }
}

fn resolve_tool_permission_policy(
    after: &Value,
) -> Result<
    (
        openlife_core::tool_permissions::ToolPermissionPolicy,
        String,
    ),
    String,
> {
    let policy_label = after
        .get("permission")
        .or_else(|| after.get("policy"))
        .or_else(|| after.get("level"))
        .and_then(Value::as_str);
    let label = if let Some(label) = policy_label {
        label
    } else {
        match after
            .get("permission_action")
            .and_then(Value::as_str)
            .unwrap_or("grant")
        {
            "grant" => "allow_until_revoked",
            "deny" => "deny",
            other => {
                return Err(format!(
                    "ToolPermission Proposal 的 permission_action 值 '{}' 无效。有效值: grant, deny",
                    other
                ));
            }
        }
    };
    let policy = match label {
        "allowed" | "allow" => {
            openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked
        }
        "deny" => openlife_core::tool_permissions::ToolPermissionPolicy::Deny,
        "ask_every_time" => openlife_core::tool_permissions::ToolPermissionPolicy::AskEveryTime,
        "allow_once" => openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
        "allow_until_revoked" => {
            openlife_core::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked
        }
        other => {
            return Err(format!(
                "ToolPermission Proposal 的 permission 值 '{}' 无效。有效值: allow, allowed, deny, ask_every_time, allow_once, allow_until_revoked",
                other
            ));
        }
    };
    Ok((policy, label.to_string()))
}

fn tool_permission_scope_field<'a>(after: &'a Value, field: &str) -> Option<&'a str> {
    after
        .get(field)
        .or_else(|| {
            after
                .get("canonical_scope")
                .and_then(|scope| scope.get(field))
        })
        .and_then(Value::as_str)
}

fn action_bound_tool_permission_scope(
    after: &Value,
) -> Result<openlife_core::tool_permissions::ActionBoundToolPermissionScope, String> {
    openlife_core::tool_permissions::ActionBoundToolPermissionScope::from_proposal_after(after)
        .map_err(|error| error.to_string())
}

fn validate_main_chat_action_bound_network_payload(after: &Value) -> Result<(), String> {
    let scope = action_bound_tool_permission_scope(after)?;
    if !matches!(scope.tool_name.as_str(), "web.fetch" | "web.search")
        || scope.source != "builtin"
        || !after
            .get("capabilities")
            .or_else(|| {
                after
                    .get("canonical_scope")
                    .and_then(|canonical| canonical.get("capabilities"))
            })
            .and_then(Value::as_array)
            .is_some_and(|capabilities| capabilities.iter().any(|value| value == "network"))
    {
        return Err(
            "action-bound network ToolPermission 仅允许内置 web.fetch/web.search 的精确 Main Chat 动作。"
                .into(),
        );
    }
    for (field, expected) in [
        ("auto_generated", true),
        ("mainChatAgentV1", true),
        ("strictManifestIdentity", true),
        ("fuzzyNameMatchingUsed", false),
        ("directWritesExecuted", false),
    ] {
        if after.get(field).and_then(Value::as_bool) != Some(expected) {
            return Err(format!(
                "action-bound network ToolPermission 缺少严格 Main Chat 标记 after.{field}。"
            ));
        }
    }
    let identity = after
        .get("pending_action_identity")
        .or_else(|| after.get("pendingActionIdentity"))
        .ok_or_else(|| {
            "action-bound network ToolPermission 缺少 pending_action_identity。".to_string()
        })?;
    let identity_string = |field: &str| {
        identity
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
    };
    for field in [
        "taskId",
        "runId",
        "queueActionId",
        "executorActionId",
        "manifestContractDigest",
    ] {
        if identity_string(field).is_none() {
            return Err(format!(
                "action-bound network ToolPermission 缺少精确 pending_action_identity.{field}。"
            ));
        }
    }
    if identity_string("queueActionType") != Some(scope.queue_action_type.as_str())
        || identity_string("executorActionType") != Some("mcp_tool")
        || identity_string("requestedTarget") != Some(scope.requested_target.as_str())
        || identity_string("resolvedTarget") != Some(scope.resolved_target.as_str())
        || identity_string("manifestId") != Some(scope.tool_name.as_str())
        || identity_string("manifestName") != Some(scope.tool_name.as_str())
        || identity_string("manifestSource") != Some(scope.source.as_str())
        || identity_string("inputHash") != Some(scope.input_hash.as_str())
        || identity.get("inputLengthBytes").and_then(Value::as_u64)
            != Some(scope.input_length_bytes)
        || identity
            .get("directWritesExecuted")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(
            "action-bound network ToolPermission 的 pending action identity 与精确执行作用域不一致。"
                .into(),
        );
    }
    Ok(())
}

async fn apply_proposal_to_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    after: Value,
    review_acceptance: Option<
        &openlife_core::agent::review_workflow::ClaimedReviewAcceptanceSnapshot,
    >,
) -> Result<openlife_core::life_model::patch::PatchApplyResult, String> {
    // Validate payload schema before applying
    if let Err(e) = validate_proposal_payload(proposal.proposal_type, &after) {
        return Ok(openlife_core::life_model::patch::PatchApplyResult {
            patch_id: proposal.id.clone(),
            success: false,
            path: proposal.affected_path.clone(),
            operation: "validation_failed".to_string(),
            error: Some(e),
        });
    }

    match proposal.proposal_type {
        ProposalType::LifeModelUpdate => {
            if is_legacy_lifemodel_v2_migration(proposal) {
                let plan = serde_json::from_value::<
                    openlife_core::life_model::legacy_migration::LegacyLifeModelMigrationPlanV2,
                >(after)
                .map_err(|_| "invalid_lifemodel_v2_migration_payload".to_string())?;
                return life_model_write_gateway::materialize_accepted_legacy_lifemodel_migration_with_state(
                    state, proposal, &plan,
                )
                .await;
            }
            if is_lifemodel_v2_typed_diff(proposal) {
                let diff = serde_json::from_value::<
                    openlife_core::life_model::v2::LifeModelTypedDiffV2,
                >(after)
                .map_err(|_| "invalid_lifemodel_v2_typed_diff_payload".to_string())?;
                return life_model_write_gateway::materialize_accepted_lifemodel_v2_typed_diff_with_state(
                    state, proposal, &diff,
                )
                .await;
            }
            Ok(openlife_core::life_model::patch::PatchApplyResult {
                patch_id: proposal.id.clone(),
                success: false,
                path: proposal.affected_path.clone(),
                operation: "lifemodel_legacy_write_retired".into(),
                error: Some(
                    "Legacy 4D LifeModel writes are retired. Recreate this change through the v2 typed Review flow."
                        .into(),
                ),
            })
        }
        ProposalType::MemoryWrite | ProposalType::MemoryArchive => match proposal.proposal_type {
            ProposalType::MemoryWrite => {
                let content = memory_content(&after)?;
                let session_id = memory_session_id(&after);
                let original_source = memory_source(&after);
                memory_gateway::materialize_memory_proposal_with_state(
                    state,
                    proposal,
                    content,
                    session_id,
                    original_source,
                )
                .await
            }
            ProposalType::MemoryArchive => {
                let owners = memory_archive_owners(&after)?;
                memory_gateway::archive_memory_for_proposal_with_state(state, proposal, &owners)
                    .await
            }
            _ => unreachable!(),
        },
        ProposalType::ToolPermission => {
            let scope_kind = tool_permission_scope_kind(&after)?;
            let tool_name = tool_permission_scope_field(&after, "tool_name")
                .or_else(|| tool_permission_scope_field(&after, "toolName"))
                .or_else(|| tool_permission_scope_field(&after, "name"))
                .ok_or_else(|| "ToolPermission Proposal 缺少 after.tool_name。".to_string())?;
            let (policy, permission) = resolve_tool_permission_policy(&after)?;
            let source = tool_permission_scope_field(&after, "source")
                .ok_or_else(|| "ToolPermission Proposal 缺少精确 after.source。".to_string())?;
            let risk_level = tool_permission_scope_field(&after, "risk_level")
                .or_else(|| tool_permission_scope_field(&after, "riskLevel"))
                .ok_or_else(|| "ToolPermission Proposal 缺少精确 after.risk_level。".to_string())?;
            let action_type = tool_permission_scope_field(&after, "action_type")
                .or_else(|| tool_permission_scope_field(&after, "actionType"))
                .ok_or_else(|| {
                    "ToolPermission Proposal 缺少精确 after.action_type。".to_string()
                })?;
            if scope_kind == ToolPermissionScopeKind::ManifestPolicy
                && proposal.source != openlife_core::agent::ProposalSource::Manual
            {
                return Err(
                    "manifest_policy ToolPermission 只能由显式 Manual review source 创建。"
                        .to_string(),
                );
            }
            let action_bound_scope = if scope_kind == ToolPermissionScopeKind::ActionBound {
                Some(action_bound_tool_permission_scope(&after)?)
            } else {
                None
            };
            let permission_id = {
                let permission_store = state.tool_permission_store.lock().await;
                if let Some(scope) = action_bound_scope.as_ref() {
                    permission_store
                        .grant_action_bound(&proposal.id, scope)
                        .map(|authorization| authorization.permission_id)
                        .map_err(|e| e.to_string())?
                } else if scope_kind == ToolPermissionScopeKind::NetworkPolicy {
                    let review_acceptance = review_acceptance.ok_or_else(|| {
                        "network_policy ToolPermission 缺少不可序列化的 ReviewWorkflow acceptance proof。"
                            .to_string()
                    })?;
                    permission_store
                        .grant_reviewed_network_once(
                            review_acceptance,
                            tool_name,
                            source,
                            risk_level,
                            action_type,
                        )
                        .map(|record| record.id)
                        .map_err(|e| e.to_string())?
                } else {
                    permission_store
                        .grant(tool_name, source, risk_level, action_type, policy, None)
                        .map(|record| record.id)
                        .map_err(|e| e.to_string())?
                }
            };
            {
                let feedback = state.feedback_store.lock().await;
                let detail = serde_json::json!({
                    "proposal_id": proposal.id,
                    "tool_name": tool_name,
                    "permission": permission,
                    "permission_id": permission_id,
                    "permission_scope_kind": match scope_kind {
                        ToolPermissionScopeKind::ActionBound => "action_bound",
                        ToolPermissionScopeKind::ManifestPolicy => "manifest_policy",
                        ToolPermissionScopeKind::NetworkPolicy => "network_policy",
                    },
                    "source_detail": proposal.source_detail,
                });
                let detail_text = detail.to_string();
                feedback
                    .log_event(
                        "tool_permission_accepted",
                        proposal.run_id.as_deref(),
                        Some(&detail_text),
                    )
                    .map_err(|e| e.to_string())?;
            }
            // Check for blocked_action payload from auto-generated proposals
            // so the frontend can offer a "continue" or replay option.
            let blocked_action = after.get("blocked_action").cloned();
            Ok(patch_result_for_proposal(
                proposal,
                true,
                "tool_permission",
                blocked_action.map(|ba| format!("__blocked_action__:{ba}")),
            ))
        }
        ProposalType::ExternalWriteAction => {
            Err("ExternalWriteAction must execute through ArtifactMaterializer.".into())
        }
    }
}

async fn get_proposal_with_state(
    state: &Arc<AppState>,
    proposal_id: &str,
) -> Result<AgentProposal, String> {
    state
        .persistence_coordinator
        .require_trusted_read("ProposalStore")
        .map_err(|error| error.to_string())?;
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;
    store
        .get_proposal(proposal_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Proposal 不存在：{}", proposal_id))
}

async fn update_review_proposal_before_dispatch_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    expected_status: ProposalStatus,
) -> Result<(), String> {
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;
    if store
        .update_review_before_dispatch(proposal, expected_status)
        .map_err(|error| runtime_proposal_store_error(state, error))?
    {
        Ok(())
    } else {
        let current_status = store
            .get_proposal(&proposal.id)
            .map_err(|error| error.to_string())?
            .map(|current| current.status.to_string())
            .unwrap_or_else(|| "missing".into());
        let dispatch_state = store
            .dispatch_state(&proposal.id)
            .map_err(|error| error.to_string())?
            .unwrap_or_else(|| "missing".into());
        Err(format!(
            "Proposal review compare-and-swap conflict: current_status={current_status}, dispatch_state={dispatch_state}"
        ))
    }
}

#[cfg(test)]
pub(crate) async fn accept_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    accept_proposal_with_state_and_confirmation(proposal_id, state, None).await
}

async fn reconcile_lifemodel_learning_materialization_response(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    warnings: &mut Vec<String>,
) -> Option<serde_json::Value> {
    match crate::life_model_learning::reconcile_lifemodel_learning_materialization_with_state(
        state, proposal,
    )
    .await
    {
        Ok(Some(receipt)) => serde_json::to_value(receipt).ok(),
        Ok(None) => None,
        Err(error) => {
            warnings.push(format!(
                "LifeModel 已由 gateway 处理，但学习候选状态尚待 reconciliation: {error}"
            ));
            Some(serde_json::json!({
                "proposalId": proposal.id,
                "status": "reconciliation_required",
                "canonicalLifeModelChanged": true,
            }))
        }
    }
}

pub(crate) async fn accept_proposal_with_state_and_confirmation(
    proposal_id: String,
    state: &Arc<AppState>,
    expected_native_confirmation_digest: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    require_proposal_write_for(state, &proposal)?;
    crate::life_model_learning::reconcile_lifemodel_learning_review_edit_with_state(
        state, &proposal,
    )
    .await?;

    let (confirmed_projection_claim, dispatch_state) = {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        (
            store
                .confirmed_projection_claim_id(&proposal_id)
                .map_err(|error| error.to_string())?,
            store
                .dispatch_state(&proposal_id)
                .map_err(|error| error.to_string())?,
        )
    };
    if let Some(claim_id) = confirmed_projection_claim {
        let artifact_receipt = confirmed_artifact_receipt_from_store(state, &proposal).await?;
        return match project_confirmed_effect_projection_only(state, &proposal, &claim_id).await {
            Ok(accepted) => {
                let mut warnings = vec![
                    "Recovered the durable confirmed effect projection without redispatching the effect."
                        .to_string(),
                ];
                let learning = reconcile_lifemodel_learning_materialization_response(
                    state,
                    &accepted,
                    &mut warnings,
                )
                .await;
                let canonical_task_runtime_projection_status =
                    project_confirmed_canonical_work_artifact_status(
                        state,
                        &accepted,
                        artifact_receipt.as_ref(),
                        &mut warnings,
                    )
                    .await;
                let canonical_tool_review_projection_status =
                    match project_canonical_work_tool_review_acceptance(state, &accepted).await {
                        Ok(true) => "confirmed",
                        Ok(false) => "not_applicable",
                        Err(error) => {
                            warnings.push(format!(
                                "Tool permission 已确认，但 canonical Work 等待点仍需 reconciliation: {error}"
                            ));
                            "reconciliation_required"
                        }
                    };
                let mut response = confirmed_effect_reconciliation_response(
                    &accepted,
                    true,
                    warnings,
                    artifact_receipt.clone(),
                );
                if let Some(learning) = learning {
                    response["lifeModelLearning"] = learning;
                }
                response["canonical_task_runtime_projection_status"] =
                    canonical_task_runtime_projection_status.into();
                response["canonical_tool_review_projection_status"] =
                    canonical_tool_review_projection_status.into();
                Ok(response)
            }
            Err(error) => {
                let mut warnings = vec![format!(
                    "Effect 已确认，Proposal 投影仍等待 reconciliation；未重放副作用: {}",
                    error
                )];
                let canonical_task_runtime_projection_status =
                    project_confirmed_canonical_work_artifact_status(
                        state,
                        &proposal,
                        artifact_receipt.as_ref(),
                        &mut warnings,
                    )
                    .await;
                let mut response = confirmed_effect_reconciliation_response(
                    &proposal,
                    false,
                    warnings,
                    artifact_receipt,
                );
                response["canonical_task_runtime_projection_status"] =
                    canonical_task_runtime_projection_status.into();
                Ok(response)
            }
        };
    }
    if proposal.status == ProposalStatus::Accepted && dispatch_state.as_deref() == Some("confirmed")
    {
        let artifact_receipt = confirmed_artifact_receipt_from_store(state, &proposal).await?;
        let mut warnings = vec![
            "Proposal effect was already confirmed; the idempotent retry did not redispatch it."
                .to_string(),
        ];
        let learning =
            reconcile_lifemodel_learning_materialization_response(state, &proposal, &mut warnings)
                .await;
        let canonical_task_runtime_projection_status =
            project_confirmed_canonical_work_artifact_status(
                state,
                &proposal,
                artifact_receipt.as_ref(),
                &mut warnings,
            )
            .await;
        let canonical_tool_review_projection_status =
            match project_canonical_work_tool_review_acceptance(state, &proposal).await {
                Ok(true) => "confirmed",
                Ok(false) => "not_applicable",
                Err(error) => {
                    warnings.push(format!(
                        "Tool permission 已确认，但 canonical Work 等待点仍需 reconciliation: {error}"
                    ));
                    "reconciliation_required"
                }
            };
        let mut response =
            confirmed_effect_reconciliation_response(&proposal, true, warnings, artifact_receipt);
        if let Some(learning) = learning {
            response["lifeModelLearning"] = learning;
        }
        response["canonical_task_runtime_projection_status"] =
            canonical_task_runtime_projection_status.into();
        response["canonical_tool_review_projection_status"] =
            canonical_tool_review_projection_status.into();
        return Ok(response);
    }
    ensure_pending_or_postponed(&proposal)?;
    validate_proposal_for_acceptance(&proposal)?;
    require_active_canonical_work_tool_review(state, &proposal).await?;
    let dispatch_claim_id = {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        store
            .claim_dispatch(&proposal_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                "该 Proposal 已由另一个请求领取执行；请先检查执行结果，禁止重复副作用。".to_string()
            })?
    };
    if let Some(expected_digest) = expected_native_confirmation_digest {
        // The native grant is bound to the exact Proposal snapshot. Reload only
        // after winning the dispatch claim: edits that raced before the claim are
        // now visible, while edits racing after the claim fail their own CAS.
        let claimed_proposal = get_proposal_with_state(state, &proposal_id).await?;
        let current_digest = proposal_native_confirmation_digest(&claimed_proposal);
        if current_digest != expected_digest {
            if let Some(store) = state.proposal_store.as_ref() {
                let store = store.lock().await;
                let _ = store.mark_dispatch_failed_before_effect(
                    &proposal_id,
                    &dispatch_claim_id,
                    "native_confirmation_snapshot_changed",
                );
            }
            return Err(
                "Proposal changed after native confirmation; no effect was dispatched. Review and confirm the new snapshot."
                    .to_string(),
            );
        }
        proposal = claimed_proposal;
        validate_proposal_for_acceptance(&proposal)?;
    }
    let review_acceptance_result = {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        openlife_core::agent::ReviewWorkflow::new(&store)
            .claimed_acceptance_snapshot(&proposal_id, &dispatch_claim_id)
            .map_err(|error| error.to_string())
    };
    let review_acceptance = match review_acceptance_result {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if let Some(store) = state.proposal_store.as_ref() {
                let store = store.lock().await;
                let _ = store.mark_dispatch_failed_before_effect(
                    &proposal_id,
                    &dispatch_claim_id,
                    "review_acceptance_snapshot_unavailable",
                );
            }
            return Err(format!(
                "Review acceptance snapshot could not be proven before effect: {error}"
            ));
        }
    };
    if canonical_work_artifact_id(&proposal).is_some() {
        let request_digest = openlife_core::agent::metadata_safe_value_digest(&serde_json::json!({
            "proposalId": proposal.id,
            "dispatchClaimId": dispatch_claim_id,
            "artifactId": canonical_work_artifact_id(&proposal),
            "operation": proposal.after.get("operation"),
        }))
        .1;
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let attempt_result = state
            .canonical_task_runtime_store
            .as_ref()
            .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?
            .lock()
            .await;
        let attempt_result = if proposal.after.get("undoOfArtifactId").is_some() {
            attempt_result.begin_artifact_undo_attempt(&proposal.id, &attempt_id, &request_digest)
        } else {
            attempt_result.begin_artifact_materialization_attempt(
                &proposal.id,
                &attempt_id,
                &request_digest,
            )
        };
        match attempt_result {
            Ok(Some(_)) => {}
            Ok(None) => {
                if let Some(store) = state.proposal_store.as_ref() {
                    let store = store.lock().await;
                    let _ = store.mark_dispatch_failed_before_effect(
                        &proposal_id,
                        &dispatch_claim_id,
                        "canonical_materializer_owner_missing",
                    );
                }
                return Err(
                    "canonical Work materialization owner is missing; no effect was dispatched"
                        .into(),
                );
            }
            Err(error) => {
                if let Some(store) = state.proposal_store.as_ref() {
                    let store = store.lock().await;
                    let _ = store.mark_dispatch_failed_before_effect(
                        &proposal_id,
                        &dispatch_claim_id,
                        "canonical_materializer_attempt_admission_failed",
                    );
                }
                return Err(format!(
                    "canonical Work materialization attempt admission failed: {error}"
                ));
            }
        }
    }
    let (result, artifact_materialization) = if proposal.proposal_type
        == ProposalType::ExternalWriteAction
    {
        match apply_external_write_artifact(state, &proposal, &dispatch_claim_id).await {
            ArtifactApplyOutcome::Confirmed {
                patch_result,
                receipt,
            } => (patch_result, Some(*receipt)),
            ArtifactApplyOutcome::FailedBeforeEffect(error) => {
                let reason_code = artifact_failure_projection_reason_code(&error, false);
                mark_canonical_work_artifact_effect_failure(state, &proposal, reason_code, false)
                    .await;
                return Err(format!(
                    "Artifact materialization failed before effect: {error}"
                ));
            }
            ArtifactApplyOutcome::Unknown(error) => {
                let reason_code = artifact_failure_projection_reason_code(&error, true);
                mark_canonical_work_artifact_effect_failure(state, &proposal, reason_code, true)
                    .await;
                return Err(format!(
                        "Artifact materialization state is unknown; automatic redispatch is forbidden: {error}"
                    ));
            }
        }
    } else {
        let result = match apply_proposal_to_state(
            state,
            &proposal,
            proposal.after.clone(),
            Some(&review_acceptance),
        )
        .await
        {
            Ok(result) => result,
            Err(error) => {
                if let Some(store) = state.proposal_store.as_ref() {
                    let store = store.lock().await;
                    let _ = store.mark_dispatch_unknown(
                        &proposal_id,
                        &dispatch_claim_id,
                        "proposal_apply_effect_unknown",
                    );
                }
                return Err(format!(
                    "Proposal 执行状态无法确认，已禁止自动重试并等待 reconciliation：{}",
                    error
                ));
            }
        };
        (result, None)
    };
    if !result.success {
        if let Some(store) = state.proposal_store.as_ref() {
            let store = store.lock().await;
            if dispatch_failure_was_definitely_before_effect(&result.operation) {
                let _ = store.mark_dispatch_failed_before_effect(
                    &proposal_id,
                    &dispatch_claim_id,
                    &result.operation,
                );
            } else {
                let _ = store.mark_dispatch_unknown(
                    &proposal_id,
                    &dispatch_claim_id,
                    "proposal_apply_effect_unknown",
                );
            }
        }
        let detail = result.error.clone().unwrap_or_default();
        return if dispatch_failure_was_definitely_before_effect(&result.operation) {
            Err(format!("Patch 应用前校验失败: {}", detail))
        } else {
            Err(format!(
                "Patch 未确认完成，实际副作用状态为 unknown，已禁止自动重试: {}",
                detail
            ))
        };
    }
    let mut warnings = Vec::new();
    let canonical_task_runtime_projection_status =
        project_confirmed_canonical_work_artifact_status(
            state,
            &proposal,
            artifact_materialization.as_ref(),
            &mut warnings,
        )
        .await;
    let learning_materialization =
        reconcile_lifemodel_learning_materialization_response(state, &proposal, &mut warnings)
            .await;
    // Canonical domain stores own their effect receipts. ProposalStore owns only
    // the Review dispatch checkpoint and projection lifecycle.
    let effect_receipt_persisted = match ensure_effect_dispatch_projection_pending(
        state,
        &proposal_id,
        &dispatch_claim_id,
    )
    .await
    {
        Ok(true) => true,
        Ok(false) => {
            warnings.push(
                    "Effect 已确认，但 dispatch receipt claim 已变化；禁止重复执行并等待 reconciliation。"
                        .to_string(),
                );
            false
        }
        Err(error) => {
            warnings.push(format!(
                "Effect 已确认，但 dispatch receipt 持久化失败并等待 reconciliation: {}",
                error
            ));
            false
        }
    };
    proposal.accept();
    canonicalize_proposal_affected_path(&mut proposal);
    let proposal_projected = if effect_receipt_persisted {
        match project_confirmed_effect_projection_only(state, &proposal, &dispatch_claim_id).await {
            Ok(projected) => {
                proposal = projected;
                true
            }
            Err(error) => {
                warnings.push(format!(
                    "Effect 已确认，但 Proposal status 投影失败并等待 reconciliation: {}",
                    error
                ));
                false
            }
        }
    } else {
        false
    };
    let dispatch_projection_confirmed = proposal_projected;
    let canonical_tool_review_projection_status = if proposal_projected {
        match project_canonical_work_tool_review_acceptance(state, &proposal).await {
            Ok(true) => "confirmed",
            Ok(false) => "not_applicable",
            Err(error) => {
                warnings.push(format!(
                    "Tool permission 已确认，但 canonical Work 等待点仍需 reconciliation: {error}"
                ));
                "reconciliation_required"
            }
        }
    } else if canonical_work_tool_review_identity(&proposal).is_some() {
        "reconciliation_required"
    } else {
        "not_applicable"
    };
    // Check for blocked_action in the patch result error field
    let blocked_action_info = if let Some(ref err) = result.error {
        if err.starts_with("__blocked_action__:") {
            err.strip_prefix("__blocked_action__:")
                .map(|s| s.to_string())
        } else {
            None
        }
    } else {
        None
    };
    let mut response = serde_json::json!({
        "success": true,
        "patch_result": result,
        "effect_status": "confirmed",
        "proposal_projection_status": if proposal_projected && dispatch_projection_confirmed {
            "confirmed"
        } else {
            "reconciliation_required"
        },
        "canonical_task_runtime_projection_status": canonical_task_runtime_projection_status,
        "canonical_tool_review_projection_status": canonical_tool_review_projection_status,
        "warnings": warnings,
    });
    if let Some(receipt) = artifact_materialization {
        response["artifactMaterialization"] =
            serde_json::to_value(receipt).unwrap_or(serde_json::Value::Null);
    }
    if let Some(learning) = learning_materialization {
        response["lifeModelLearning"] = learning;
    }
    if proposal.proposal_type == ProposalType::MemoryWrite {
        if let Some(lifecycle_store) = state.memory_lifecycle_store.as_ref() {
            let store = lifecycle_store.lock().await;
            if let Ok(Some(record)) = store.get_record_by_proposal_id(&proposal.id) {
                response["memoryLifecycle"] =
                    serde_json::to_value(&record).unwrap_or(serde_json::Value::Null);
                response["memoryPersistence"] =
                    match store.latest_projection_event_id(&record.memory_id) {
                        Ok(Some(event_id)) => match store.projection_summary(&event_id) {
                            Ok(summary) => serde_json::json!({
                                "canonicalCommitted": true,
                                "outboxEventId": event_id,
                                "projectionState": summary.state(),
                                "pending": summary.pending,
                                "degraded": summary.degraded,
                                "applied": summary.applied,
                            }),
                            Err(error) => serde_json::json!({
                                "canonicalCommitted": true,
                                "projectionState": "degraded",
                                "reasonCode": "projection_summary_unavailable",
                                "errorDigest": openlife_core::persistence_outbox::metadata_digest(
                                    &error.to_string()
                                ),
                            }),
                        },
                        _ => serde_json::json!({
                            "canonicalCommitted": true,
                            "projectionState": "degraded",
                            "reasonCode": "canonical_outbox_reference_missing",
                        }),
                    };
            }
        }
    }
    if let Some(blocked) = blocked_action_info {
        if let Ok(parsed) = serde_json::from_str::<Value>(&blocked) {
            response["blocked_action"] = parsed;
            response["can_continue"] = serde_json::Value::Bool(true);
        }
    }
    Ok(response)
}

fn artifact_failure_projection_reason_code(error: &str, effect_unknown: bool) -> &'static str {
    for code in [
        "artifact_target_precondition_changed",
        "artifact_target_symbolic_link_forbidden",
        "artifact_target_outside_safe_paths",
        "artifact_source_digest_changed",
    ] {
        if error.contains(code) {
            return code;
        }
    }
    if effect_unknown {
        "artifact_effect_unknown"
    } else {
        "artifact_materialization_failed_before_effect"
    }
}

pub(crate) async fn reject_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    require_proposal_write_for(state, &proposal)?;
    if proposal.status == ProposalStatus::Rejected {
        if canonical_work_artifact_id(&proposal).is_some()
            && proposal.after.get("undoOfArtifactId").is_none()
        {
            let store = state
                .proposal_store
                .as_ref()
                .ok_or_else(proposal_store_missing)?
                .lock()
                .await;
            store
                .reject_review_and_cancel_active_siblings(&proposal, ProposalStatus::Rejected)
                .map_err(|error| runtime_proposal_store_error(state, error))?
                .ok_or_else(|| "Proposal review compare-and-swap conflict".to_string())?;
        }
        project_canonical_work_review_rejection(state, &proposal).await?;
        crate::life_model_learning::record_lifemodel_learning_review_rejected_with_state(
            state, &proposal,
        )
        .await?;
        return Ok(());
    }
    ensure_pending_or_postponed(&proposal)?;
    crate::life_model_learning::reconcile_lifemodel_learning_review_edit_with_state(
        state, &proposal,
    )
    .await?;
    ensure_review_change_precedes_effect_dispatch(state, &proposal_id).await?;
    let expected_status = proposal.status;
    proposal.reject();
    if canonical_work_artifact_id(&proposal).is_some()
        && proposal.after.get("undoOfArtifactId").is_none()
    {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        store
            .reject_review_and_cancel_active_siblings(&proposal, expected_status)
            .map_err(|error| runtime_proposal_store_error(state, error))?
            .ok_or_else(|| "Proposal review compare-and-swap conflict".to_string())?;
    } else {
        update_review_proposal_before_dispatch_with_state(state, &proposal, expected_status)
            .await?;
    }
    project_canonical_work_review_rejection(state, &proposal).await?;
    crate::life_model_learning::record_lifemodel_learning_review_rejected_with_state(
        state, &proposal,
    )
    .await?;
    Ok(())
}

pub(crate) async fn edit_lifemodel_learning_proposal_with_state(
    proposal_id: String,
    statement: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    require_persistence_write(state)?;
    let statement = statement.trim();
    if statement.is_empty() || statement.chars().count() > 500 {
        return Err("LifeModel learning statement must contain 1 to 500 characters.".into());
    }
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    ensure_pending_or_postponed(&proposal)?;
    crate::life_model_learning::reconcile_lifemodel_learning_review_edit_with_state(
        state, &proposal,
    )
    .await?;
    ensure_review_change_precedes_effect_dispatch(state, &proposal_id).await?;
    if !proposal
        .source_detail
        .as_deref()
        .is_some_and(|detail| detail.starts_with("lifemodel_learning:"))
        || !openlife_core::agent::review_decision_context::is_lifemodel_learning_review(&proposal)
    {
        return Err("Only a validated LifeModel learning proposal supports this editor.".into());
    }
    let original: openlife_core::life_model::v2::LifeModelTypedDiffV2 =
        serde_json::from_value(proposal.after.clone())
            .map_err(|_| "LifeModel learning typed diff is invalid.".to_string())?;
    let operation = original
        .operations
        .first()
        .cloned()
        .ok_or_else(|| "LifeModel learning typed operation is missing.".to_string())?;
    let (section, mut item) = match operation {
        openlife_core::life_model::v2::LifeModelTypedOperationV2::Add {
            section,
            item: openlife_core::life_model::v2::LifeModelItemV2::Statement(item),
        } if original.operations.len() == 1 => (section, item),
        _ => return Err("LifeModel learning editor only supports one statement add.".into()),
    };
    item.statement = statement.to_string();
    let manager = state.life_model_manager.lock().await;
    let current = manager
        .load_v2_current(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
        .map_err(|error| error.to_string())?;
    if current.as_ref().map(|version| version.model_version) != original.base_version
        || current
            .as_ref()
            .map(|version| version.document_digest.as_str())
            != original.base_document_digest.as_deref()
    {
        return Err(
            "LifeModel learning proposal base is stale; create a fresh review item.".into(),
        );
    }
    let allow_empty_result = current.is_some();
    let revised = openlife_core::life_model::v2::LifeModelTypedDiffV2::from_operations_for_review(
        openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID,
        current.as_ref(),
        vec![
            openlife_core::life_model::v2::LifeModelTypedOperationV2::Add {
                section,
                item: openlife_core::life_model::v2::LifeModelItemV2::Statement(item),
            },
        ],
        allow_empty_result,
    )
    .map_err(|error| error.to_string())?;
    drop(manager);
    let before = proposal
        .before
        .as_mut()
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "LifeModel learning review metadata is missing.".to_string())?;
    before.insert(
        "proposedValue".into(),
        serde_json::to_value(
            openlife_core::life_model::v2::LifeModelUserValueV2::Statement {
                statement: statement.to_string(),
            },
        )
        .map_err(|error| error.to_string())?,
    );
    before.insert("editedByUser".into(), Value::Bool(true));
    let expected_status = proposal.status;
    proposal.edit(serde_json::to_value(&revised).map_err(|error| error.to_string())?);
    update_review_proposal_before_dispatch_with_state(state, &proposal, expected_status).await?;
    let learning = crate::life_model_learning::record_lifemodel_learning_review_edit_with_state(
        state, &proposal, statement,
    )
    .await?
    .ok_or_else(|| "LifeModel learning review context disappeared after edit.".to_string())?;
    Ok(serde_json::json!({
        "proposalId": proposal.id,
        "status": "edited_pending_review",
        "resultDocumentDigest": revised.result_document_digest,
        "durableWriteExecuted": false,
        "learning": learning,
    }))
}

pub(crate) async fn postpone_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<(), String> {
    require_persistence_write(state)?;
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    ensure_pending_or_postponed(&proposal)?;
    crate::life_model_learning::reconcile_lifemodel_learning_review_edit_with_state(
        state, &proposal,
    )
    .await?;
    ensure_review_change_precedes_effect_dispatch(state, &proposal_id).await?;
    let expected_status = proposal.status;
    proposal.postpone();
    update_review_proposal_before_dispatch_with_state(state, &proposal, expected_status).await?;
    Ok(())
}

fn ensure_exact_memory_id(memory_id: &str) -> Result<(), String> {
    let trimmed = memory_id.trim();
    if trimmed != memory_id
        || trimmed.is_empty()
        || !trimmed.starts_with("memory:")
        || trimmed
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(
            "rollback_memory_asset requires an exact accepted memory id, not a text query."
                .to_string(),
        );
    }
    Ok(())
}

pub(crate) async fn rollback_memory_asset_with_state(
    memory_id: String,
    reason: String,
    state: &Arc<AppState>,
) -> Result<MemoryRollbackReport, String> {
    ensure_exact_memory_id(&memory_id)?;
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("rollback_memory_asset requires a rollback reason.".into());
    }
    memory_gateway::rollback_memory_asset_with_state(memory_id, reason.to_string(), state).await
}

fn proposal_native_confirmation_digest(proposal: &AgentProposal) -> String {
    let (_, digest) =
        openlife_core::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
            "proposal_id": proposal.id,
            "run_id": proposal.run_id,
            "proposal_type": proposal.proposal_type,
            "source": proposal.source,
            "source_detail": proposal.source_detail,
            "risk_level": proposal.risk_level,
            "affected_path": proposal.affected_path,
            "base_hash": proposal.base_hash,
            "before": proposal.before,
            "after": proposal.after,
            "reason": proposal.reason,
            "confidence_bits": proposal.confidence.to_bits(),
            "status": proposal.status,
            "created_at": proposal.created_at,
            "resolved_at": proposal.resolved_at,
            "expires_at": proposal.expires_at,
        }));
    digest
}

fn proposal_requires_native_confirmation(proposal: &AgentProposal) -> bool {
    if proposal.proposal_type == ProposalType::ExternalWriteAction
        && canonical_work_artifact_id(proposal).is_some()
    {
        // The canonical ReviewItem is already an exact, user-visible,
        // digest-bound just-in-time confirmation. A second native prompt adds
        // no authority and creates approval fatigue.
        return false;
    }
    matches!(proposal.risk_level, RiskLevel::High | RiskLevel::Critical)
        || matches!(
            proposal.proposal_type,
            ProposalType::ToolPermission | ProposalType::ExternalWriteAction
        )
}

async fn proposal_may_dispatch_effect(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<bool, String> {
    if !matches!(
        proposal.status,
        ProposalStatus::Pending | ProposalStatus::Postponed | ProposalStatus::Edited
    ) {
        return Ok(false);
    }
    let dispatch_state = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await
        .dispatch_state(&proposal.id)
        .map_err(|error| error.to_string())?;
    Ok(matches!(
        dispatch_state.as_deref(),
        None | Some("unclaimed" | "failed_before_effect")
    ))
}

#[tauri::command]
pub async fn accept_proposal(
    proposal_id: String,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<AcceptProposalResponse, String> {
    let proposal = get_proposal_with_state(state.inner(), &proposal_id).await?;
    require_proposal_write_for(state.inner(), &proposal)?;
    let mut expected_native_confirmation_digest = None;
    if proposal_requires_native_confirmation(&proposal)
        && proposal_may_dispatch_effect(state.inner(), &proposal).await?
    {
        ensure_pending_or_postponed(&proposal)?;
        validate_proposal_for_acceptance(&proposal)?;
        let snapshot_digest = proposal_native_confirmation_digest(&proposal);
        let affected_path_digest = openlife_core::agent::metadata_safe::metadata_safe_value_digest(
            &serde_json::json!({ "affected_path": proposal.affected_path }),
        )
        .1;
        require_native_danger_action_confirmation(
            &window,
            NativeDangerActionRequest {
                action_type: "proposal_accept",
                target_ids_for_new_challenge: std::slice::from_ref(&proposal_id),
                requested_target: Some(proposal_id.as_str()),
                affected_count: 1,
                arguments: &serde_json::json!({
                    "proposal_snapshot_digest": snapshot_digest.clone(),
                    "proposal_type": proposal.proposal_type,
                    "risk_level": proposal.risk_level,
                    "affected_path_digest": affected_path_digest.clone(),
                }),
                arguments_summary: &format!(
                    "接受 {} / {} Proposal；affected path 仅以 digest 展示：{}",
                    proposal.proposal_type, proposal.risk_level, affected_path_digest
                ),
                scope_summary: "执行高风险 Proposal 的已审核 canonical 或 external effect。",
                challenge_id: None,
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        expected_native_confirmation_digest = Some(snapshot_digest);
    }
    let response = accept_proposal_with_state_and_confirmation(
        proposal_id,
        state.inner(),
        expected_native_confirmation_digest.as_deref(),
    )
    .await?;
    typed_accept_proposal_response(response)
}

#[tauri::command]
pub async fn reject_proposal(
    proposal_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    reject_proposal_with_state(proposal_id, state.inner()).await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactUndoProposalResponse {
    pub artifact_id: String,
    pub proposal_id: String,
    pub status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactUndoProposalFailure {
    pub artifact_id: String,
    pub reason_code: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactUndoProposalResponse {
    pub task_id: String,
    pub status: &'static str,
    pub proposals: Vec<ArtifactUndoProposalResponse>,
    pub failures: Vec<ArtifactUndoProposalFailure>,
}

enum PreparedArtifactUndo {
    TrashCreated {
        source: String,
        target: String,
        content_digest: String,
    },
    RestoreReplaced {
        snapshot: String,
        target: String,
        restore_digest: String,
        expected_target_digest: String,
    },
    RestoreMoved {
        source: String,
        target: String,
        content_digest: String,
    },
}

fn artifact_undo_request_failure_reason_code(error: &str) -> &'static str {
    match error {
        "artifact_undo_source_changed" => "artifact_undo_source_changed",
        "canonical_artifact_pre_change_snapshot_unavailable"
        | "canonical_artifact_pre_change_snapshot_changed" => {
            "artifact_undo_original_bytes_unavailable"
        }
        "canonical_artifact_undo_unavailable_without_original_bytes" => {
            "artifact_undo_original_bytes_unavailable"
        }
        _ if error.contains("target already exists") => "artifact_undo_target_conflict",
        _ if error.contains("source digest does not match")
            || error.contains("Failed to resolve move source") =>
        {
            "artifact_undo_source_changed"
        }
        _ => "artifact_undo_request_failed",
    }
}

#[tauri::command]
pub async fn request_artifact_undo(
    artifact_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ArtifactUndoProposalResponse, String> {
    request_artifact_undo_with_state(artifact_id, state.inner()).await
}

#[tauri::command]
pub async fn request_task_artifact_undo(
    task_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<TaskArtifactUndoProposalResponse, String> {
    request_task_artifact_undo_with_state(task_id, state.inner()).await
}

pub(crate) async fn request_task_artifact_undo_with_state(
    task_id: String,
    state: &Arc<AppState>,
) -> Result<TaskArtifactUndoProposalResponse, String> {
    require_persistence_write(state)?;
    let canonical_store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let snapshot = canonical_store
        .lock()
        .await
        .load_task_snapshot(&task_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_task_missing".to_string())?;
    let mut artifact_ids = Vec::new();
    for artifact in snapshot.artifacts {
        if artifact.artifact.status
            != openlife_core::task_runtime::CanonicalArtifactStatus::Materialized
        {
            continue;
        }
        let undo = canonical_store
            .lock()
            .await
            .load_artifact_undo(&artifact.artifact.id)
            .map_err(|error| error.to_string())?;
        if undo.is_none() {
            artifact_ids.push(artifact.artifact.id);
        }
    }
    if artifact_ids.len() < 2 {
        return Err(
            "canonical_task_artifact_batch_undo_requires_multiple_available_artifacts".into(),
        );
    }

    let mut proposals = Vec::with_capacity(artifact_ids.len());
    let mut failures = Vec::new();
    for artifact_id in artifact_ids {
        match request_artifact_undo_with_state(artifact_id.clone(), state).await {
            Ok(proposal) => proposals.push(proposal),
            Err(error) => failures.push(ArtifactUndoProposalFailure {
                artifact_id,
                reason_code: artifact_undo_request_failure_reason_code(&error),
            }),
        }
    }
    if proposals.is_empty() {
        return Err("canonical_task_artifact_batch_undo_no_proposals_created".into());
    }
    Ok(TaskArtifactUndoProposalResponse {
        task_id,
        status: if failures.is_empty() {
            "waiting_review"
        } else {
            "partial_waiting_review"
        },
        proposals,
        failures,
    })
}

pub(crate) async fn request_artifact_undo_with_state(
    artifact_id: String,
    state: &Arc<AppState>,
) -> Result<ArtifactUndoProposalResponse, String> {
    require_persistence_write(state)?;
    let canonical_store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let artifact = canonical_store
        .lock()
        .await
        .load_artifact(&artifact_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_artifact_missing".to_string())?;
    if artifact.status != openlife_core::task_runtime::CanonicalArtifactStatus::Materialized {
        return Err("canonical_artifact_undo_requires_materialized_artifact".into());
    }
    let source = artifact
        .materialized_reference
        .clone()
        .ok_or_else(|| "canonical_artifact_materialized_reference_missing".to_string())?;
    let task_snapshot = canonical_store
        .lock()
        .await
        .load_task_snapshot(&artifact.task_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_artifact_task_missing".to_string())?;
    let artifact_snapshot = task_snapshot
        .artifacts
        .iter()
        .find(|snapshot| snapshot.artifact.id == artifact.id)
        .ok_or_else(|| "canonical_artifact_snapshot_missing".to_string())?;
    let source_item = task_snapshot
        .items
        .iter()
        .find(|item| item.id == artifact.source_item_id)
        .ok_or_else(|| "canonical_artifact_source_item_missing".to_string())?;
    let original_proposal_id = artifact_snapshot
        .review_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.proposal_id.clone());
    let (original_proposal, original_operation, original_target_absent, safe_paths) =
        if let Some(original_proposal_id) = original_proposal_id.as_deref() {
            let original_proposal = get_proposal_with_state(state, original_proposal_id).await?;
            let target_absent = matches!(
                reviewed_artifact_target_precondition(&original_proposal.after)?,
                ArtifactTargetPrecondition::Absent
            );
            let operation = original_proposal
                .after
                .get("operation")
                .and_then(Value::as_str)
                .unwrap_or("create")
                .to_string();
            let safe_paths =
                crate::canonical_work_runtime::artifact_materialized_safe_paths_for_task_run(
                    state,
                    &artifact.task_id,
                    &source_item.run_id,
                )
                .await?;
            (
                Some(original_proposal),
                operation,
                target_absent,
                safe_paths,
            )
        } else {
            if artifact_snapshot.current_version.expected_target_absent != Some(true)
                || artifact_snapshot
                    .current_version
                    .expected_target_digest
                    .is_some()
                || artifact_snapshot.pre_change_snapshot.is_some()
                || artifact_snapshot
                    .current_version
                    .observed_content_digest
                    .as_deref()
                    != Some(artifact.content_digest.as_str())
            {
                return Err("canonical_direct_artifact_undo_origin_invalid".into());
            }
            let safe_paths =
                crate::canonical_work_runtime::artifact_materialized_safe_paths_for_task_run(
                    state,
                    &artifact.task_id,
                    &source_item.run_id,
                )
                .await?;
            (None, "create".to_string(), true, safe_paths)
        };
    let prepared_undo = if original_operation == "move" {
        let target = original_proposal
            .as_ref()
            .ok_or_else(|| "canonical_artifact_rename_source_missing".to_string())?
            .after
            .get("source_path")
            .and_then(Value::as_str)
            .filter(|target| !target.trim().is_empty())
            .ok_or_else(|| "canonical_artifact_rename_source_missing".to_string())?
            .to_string();
        crate::artifact_materializer::prepare_artifact_move(
            "artifact-undo-preview",
            &source,
            &target,
            &artifact.content_digest,
            &safe_paths,
        )?;
        PreparedArtifactUndo::RestoreMoved {
            source: source.clone(),
            target,
            content_digest: artifact.content_digest.clone(),
        }
    } else if original_target_absent {
        let target = crate::artifact_materializer::trash_target_for_source(&source, &safe_paths)?;
        crate::artifact_materializer::prepare_artifact_move(
            "artifact-undo-preview",
            &source,
            &target.to_string_lossy(),
            &artifact.content_digest,
            &safe_paths,
        )?;
        PreparedArtifactUndo::TrashCreated {
            source: source.clone(),
            target: target.to_string_lossy().into_owned(),
            content_digest: artifact.content_digest.clone(),
        }
    } else {
        let pre_change = canonical_store
            .lock()
            .await
            .load_artifact_pre_change_snapshot(&artifact.id, artifact.current_version)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                "canonical_artifact_undo_unavailable_without_original_bytes".to_string()
            })?;
        let bytes = std::fs::read(&pre_change.snapshot_reference)
            .map_err(|_| "canonical_artifact_pre_change_snapshot_unavailable".to_string())?;
        if artifact_content_digest(&bytes) != pre_change.content_digest {
            return Err("canonical_artifact_pre_change_snapshot_changed".into());
        }
        if crate::artifact_materializer::capture_artifact_target_precondition(&source, &safe_paths)?
            != ArtifactTargetPrecondition::ContentDigest(artifact.content_digest.clone())
        {
            return Err("artifact_undo_source_changed".into());
        }
        crate::artifact_materializer::prepare_artifact_materialization_with_precondition_for_artifact_bytes(
            &artifact.id,
            "artifact-undo-preview",
            "artifact-undo-preview",
            &source,
            &bytes,
            &safe_paths,
            ArtifactTargetPrecondition::ContentDigest(artifact.content_digest.clone()),
        )?;
        PreparedArtifactUndo::RestoreReplaced {
            snapshot: pre_change.snapshot_reference,
            target: source.clone(),
            restore_digest: pre_change.content_digest,
            expected_target_digest: artifact.content_digest.clone(),
        }
    };
    let mut after = match &prepared_undo {
        PreparedArtifactUndo::TrashCreated {
            source,
            target,
            content_digest,
        } => serde_json::json!({
            "operation": "trash",
            "source_path": source,
            "target_path": target,
            "source_digest": content_digest,
            "contentDigest": content_digest,
            "canonicalTaskId": artifact.task_id,
            "artifactDraftItemId": artifact.source_item_id,
            "artifactId": artifact.id,
            "artifactVersion": artifact.current_version,
            "undoOfArtifactId": artifact.id,
            "undoOfProposalId": original_proposal_id,
            "sourceRunId": source_item.run_id,
            "directWritesExecuted": false,
            "externalWritesExecuted": false,
        }),
        PreparedArtifactUndo::RestoreMoved {
            source,
            target,
            content_digest,
        } => serde_json::json!({
            "operation": "restore",
            "source_path": source,
            "target_path": target,
            "source_digest": content_digest,
            "contentDigest": content_digest,
            "canonicalTaskId": artifact.task_id,
            "artifactDraftItemId": artifact.source_item_id,
            "artifactId": artifact.id,
            "artifactVersion": artifact.current_version,
            "undoOfArtifactId": artifact.id,
            "undoOfProposalId": original_proposal_id,
            "sourceRunId": source_item.run_id,
            "directWritesExecuted": false,
            "externalWritesExecuted": false,
        }),
        PreparedArtifactUndo::RestoreReplaced {
            snapshot,
            target,
            restore_digest,
            expected_target_digest,
        } => serde_json::json!({
            "operation": "restore_snapshot",
            "snapshot_path": snapshot,
            "path": target,
            "restore_digest": restore_digest,
            "expected_target_digest": expected_target_digest,
            "contentDigest": restore_digest,
            "canonicalTaskId": artifact.task_id,
            "artifactDraftItemId": artifact.source_item_id,
            "artifactId": artifact.id,
            "artifactVersion": artifact.current_version,
            "undoOfArtifactId": artifact.id,
            "undoOfProposalId": original_proposal_id,
            "sourceRunId": source_item.run_id,
            "directWritesExecuted": false,
            "externalWritesExecuted": false,
        }),
    };
    if original_proposal_id.is_none() {
        let object = after
            .as_object_mut()
            .ok_or_else(|| "canonical_artifact_undo_payload_invalid".to_string())?;
        object.remove("undoOfProposalId");
        object.insert(
            "undoOriginKind".into(),
            Value::String("direct_materialization".into()),
        );
        object.insert(
            "undoOfDirectArtifactVersion".into(),
            Value::String(format!("{}:v{}", artifact.id, artifact.current_version)),
        );
    }
    let affected_path = match &prepared_undo {
        PreparedArtifactUndo::TrashCreated { source, target, .. } => {
            format!("filesystem.{source}->{target}")
        }
        PreparedArtifactUndo::RestoreReplaced { target, .. } => {
            format!("filesystem.{target}")
        }
        PreparedArtifactUndo::RestoreMoved { source, target, .. } => {
            format!("filesystem.{source}->{target}")
        }
    };
    let mut proposal = AgentProposal::new(
        ProposalType::ExternalWriteAction,
        &affected_path,
        after,
        "User requested a governed Undo for a verified OpenLife-created Artifact.",
        1.0,
        RiskLevel::High,
        openlife_core::agent::ProposalSource::Manual,
    );
    proposal.run_id = Some(source_item.run_id.clone());
    proposal.source_detail = Some(artifact.task_id.clone());
    let request = openlife_core::agent::DurableWriteRequest::from_agent_proposal(
        openlife_core::agent::DurableWriteSource::ManualOverride,
        openlife_core::agent::DurableWriteSubject::ExternalWrite,
        proposal,
        "Artifact Undo is pending Review Center approval; no file was moved.",
    )
    .with_idempotency_key(format!("artifact_undo:{}", artifact.id));
    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await;
    let review = openlife_core::agent::ReviewWorkflow::new(&proposal_store)
        .submit(request)
        .map_err(|error| error.to_string())?;
    drop(proposal_store);
    match prepared_undo {
        PreparedArtifactUndo::TrashCreated {
            source,
            target,
            content_digest,
        } => canonical_store.lock().await.bind_artifact_undo(
            &artifact.id,
            review.proposal_id(),
            &source,
            &target,
            &content_digest,
        ),
        PreparedArtifactUndo::RestoreReplaced {
            snapshot,
            target,
            restore_digest,
            expected_target_digest,
        } => canonical_store.lock().await.bind_artifact_replacement_undo(
            &artifact.id,
            review.proposal_id(),
            &snapshot,
            &target,
            &restore_digest,
            &expected_target_digest,
        ),
        PreparedArtifactUndo::RestoreMoved {
            source,
            target,
            content_digest,
        } => canonical_store.lock().await.bind_artifact_rename_undo(
            &artifact.id,
            review.proposal_id(),
            &source,
            &target,
            &content_digest,
        ),
    }
    .map_err(|error| error.to_string())?;
    Ok(ArtifactUndoProposalResponse {
        artifact_id: artifact.id,
        proposal_id: review.proposal_id().to_string(),
        status: "waiting_review",
    })
}

#[tauri::command]
pub async fn postpone_proposal(
    proposal_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    postpone_proposal_with_state(proposal_id, state.inner()).await
}

#[tauri::command]
pub async fn rollback_memory_asset(
    memory_id: String,
    reason: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MemoryRollbackReport, String> {
    rollback_memory_asset_with_state(memory_id, reason, state.inner()).await
}
