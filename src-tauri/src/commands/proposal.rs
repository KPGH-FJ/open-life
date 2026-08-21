use crate::{
    artifact_materializer::{
        artifact_content_digest, commit_artifact_move, commit_staged_artifact,
        confirmed_artifact_receipt, confirmed_move_receipt, confirmed_move_receipt_from_paths,
        inspect_artifact_filesystem, inspect_artifact_move, prepare_artifact_materialization,
        prepare_artifact_materialization_with_precondition_for_artifact_bytes,
        prepare_artifact_move, stage_artifact_bytes, stage_artifact_raw_bytes,
        ArtifactFilesystemFailure, ArtifactFilesystemObservation, ArtifactMaterializationReceipt,
        ArtifactTargetPrecondition,
    },
    danger_action_confirmation::{
        require_native_danger_action_confirmation, NativeDangerActionRequest,
    },
    life_model_write_gateway, memory_gateway,
    storage::app_data_dir,
    AppState,
};
use openlife_core::agent::{
    AgentProposal, ArtifactEffectState, LifeModelLearningCandidateStatus,
    LifeModelLearningReviewDecisionReceipt, MemoryRollbackReport, ProposalSource, ProposalStatus,
    ProposalType, RiskLevel,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

/// Maximum content size for ExternalWriteAction (100 KB)
const EXTERNAL_WRITE_MAX_SIZE: usize = 100 * 1024;
pub(crate) const COMMUNICATION_STYLE_CANONICAL_PATH: &str = "preferences.communication_style";

fn canonical_work_artifact_id(proposal: &AgentProposal) -> Option<&str> {
    let artifact_undo = proposal
        .after
        .get("undoOfArtifactId")
        .and_then(Value::as_str)
        .is_some();
    if (!artifact_undo
        && proposal
            .after
            .get("generatedByProvider")
            .and_then(Value::as_bool)
            != Some(true))
        || proposal
            .after
            .get("artifactVersion")
            .and_then(Value::as_u64)
            .is_none_or(|version| version == 0)
        || proposal
            .after
            .get("canonicalTaskId")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || proposal
            .after
            .get("artifactDraftItemId")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    {
        return None;
    }
    proposal
        .after
        .get("artifactId")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("artifact:") && value.len() <= 512)
}

fn artifact_id_for_proposal(proposal: &AgentProposal) -> String {
    canonical_work_artifact_id(proposal)
        .map(str::to_string)
        .unwrap_or_else(|| format!("artifact:{}", proposal.id))
}

fn reviewed_artifact_target_precondition(
    after: &Value,
) -> Result<ArtifactTargetPrecondition, String> {
    let expected_absent = after
        .get("expected_target_absent")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Artifact Proposal 缺少 expected_target_absent。".to_string())?;
    let expected_digest = after
        .get("expected_target_digest")
        .and_then(Value::as_str)
        .filter(|digest| !digest.trim().is_empty());
    match (expected_absent, expected_digest) {
        (true, None) => Ok(ArtifactTargetPrecondition::Absent),
        (false, Some(digest)) if digest.starts_with("sha256:") => Ok(
            ArtifactTargetPrecondition::ContentDigest(digest.to_string()),
        ),
        _ => Err("Artifact Proposal 必须精确绑定目标不存在或审核时的目标内容摘要。".into()),
    }
}

pub(crate) async fn artifact_safe_paths_for_proposal(
    state: &Arc<AppState>,
    _proposal: &AgentProposal,
) -> Result<Vec<String>, String> {
    let config = state.config.lock().await;
    Ok(config.system.safe_paths.clone())
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
    if canonical_work_artifact_id(proposal).is_some() {
        state
            .persistence_coordinator
            .require_effects_for_stores(&["ProposalStore", "CanonicalTaskRuntimeStore"])
            .map_err(|error| error.to_string())
    } else {
        require_persistence_write(state)?;
        check_safe_mode(state)
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

fn check_safe_mode(state: &Arc<AppState>) -> Result<(), String> {
    if !state.startup_warnings.is_empty() {
        return Err(format!(
            "系统处于 Safe Mode，无法应用 Proposal：{}",
            state.startup_warnings.join("；")
        ));
    }
    Ok(())
}

fn ensure_pending_or_postponed(proposal: &AgentProposal) -> Result<(), String> {
    match proposal.status {
        ProposalStatus::Pending | ProposalStatus::Postponed | ProposalStatus::Edited => Ok(()),
        ProposalStatus::Accepted => Err("该 Proposal 已经被接受，不能重复处理。".to_string()),
        ProposalStatus::Rejected => Err("该 Proposal 已经被拒绝，不能再次处理。".to_string()),
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
            == openlife_core::life_model::v2::LIFE_MODEL_V2_LEGACY_MIGRATION_PATH
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
            | "scheduled_task_review_snapshot_missing"
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
    pub artifact_effects_reconciled: usize,
    pub ambiguous_action_effects_marked_unknown: usize,
    pub proposal_projections_repaired: usize,
    pub artifact_backlog_may_remain: bool,
    pub action_effect_backlog_may_remain: bool,
    pub projection_backlog_may_remain: bool,
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
    state: ArtifactEffectState,
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
                state: match record.state {
                    openlife_core::task_runtime::CanonicalArtifactEffectState::Prepared => {
                        ArtifactEffectState::Prepared
                    }
                    openlife_core::task_runtime::CanonicalArtifactEffectState::Staged => {
                        ArtifactEffectState::Staged
                    }
                    _ => unreachable!("only open canonical effects are listed"),
                },
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let legacy_records = {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        store
            .list_artifact_effects_for_reconciliation(bounded_limit)
            .map_err(|error| runtime_proposal_store_error(state, error))?
    };
    records.extend(
        legacy_records
            .into_iter()
            .map(|record| ArtifactEffectRecoveryRecord {
                proposal_id: record.proposal_id,
                dispatch_claim_id: record.dispatch_claim_id,
                target_reference_digest: record.target_reference_digest,
                content_digest: record.content_digest,
                byte_size: record.byte_size,
                media_type: record.media_type,
                state: record.state,
            }),
    );
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
        let safe_paths = match artifact_safe_paths_for_proposal(state, &proposal).await {
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
            if target_reference_digest != record.target_reference_digest {
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
                    if record.state == ArtifactEffectState::Prepared =>
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
        let resolved = match resolve_artifact_effect_input(state, &proposal).await {
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
                if record.state == ArtifactEffectState::Prepared =>
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

fn claimed_local_scheduled_task_matches_canonical_effect(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<bool, String> {
    if proposal.proposal_type != ProposalType::ScheduledTask
        || parse_reviewed_scheduled_provider_route(&proposal.after)?.is_some()
    {
        return Ok(false);
    }
    let Some(task) = state
        .scheduled_task_store
        .get_task_by_source_proposal_id(&proposal.id)
        .map_err(|error| format!("load claimed scheduled task effect failed: {error}"))?
    else {
        return Ok(false);
    };
    let reviewed_due_at = proposal
        .after
        .get("scheduled_at")
        .or_else(|| proposal.after.get("due_date"))
        .or_else(|| proposal.after.get("date"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let reviewed_due_at = parse_scheduled_at(reviewed_due_at)?.map(|value| value.to_rfc3339());
    let reviewed_title = proposal
        .after
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Untitled Task");
    let reviewed_description = proposal
        .after
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("");
    let reviewed_priority = proposal
        .after
        .get("priority")
        .and_then(Value::as_str)
        .unwrap_or("medium");
    let reviewed_action_type = proposal
        .after
        .get("tool")
        .and_then(Value::as_str)
        .unwrap_or("scheduled_task");

    Ok(task.id == proposal.id
        && task.source_proposal_id.as_deref() == Some(proposal.id.as_str())
        && task.source_run_id == proposal.run_id
        && task.title == reviewed_title
        && task.description == reviewed_description
        && task.due_date == reviewed_due_at
        && task.priority == reviewed_priority
        && task.action_type == reviewed_action_type
        && task.provider_grant.data_route == openlife_core::llm::ProviderDataRoute::LocalOnly)
}

async fn seal_startup_governed_action_claims_as_unknown(
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
            .list_claimed_governed_actions_for_unknown_recovery(bounded_limit as usize)
            .map_err(|error| runtime_proposal_store_error(state, error))?
    };
    let backlog_may_remain = claims.len() == bounded_limit as usize;
    let mut sealed = 0usize;
    for (proposal_id, claim_id) in claims {
        let proposal = {
            let store = state
                .proposal_store
                .as_ref()
                .ok_or_else(proposal_store_missing)?
                .lock()
                .await;
            store
                .get_proposal(&proposal_id)
                .map_err(|error| runtime_proposal_store_error(state, error))?
                .ok_or_else(|| format!("startup_claimed_governed_action_missing:{proposal_id}"))?
        };
        if claimed_local_scheduled_task_matches_canonical_effect(state, &proposal)? {
            let store = state
                .proposal_store
                .as_ref()
                .ok_or_else(proposal_store_missing)?
                .lock()
                .await;
            if !store
                .mark_effect_confirmed_projection_pending(&proposal_id, &claim_id)
                .map_err(|error| runtime_proposal_store_error(state, error))?
            {
                return Err(format!(
                    "startup_scheduled_task_confirmation_cas_lost:{proposal_id}"
                ));
            }
            continue;
        }
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
                "startup_governed_action_effect_unknown",
            )
            .map_err(|error| runtime_proposal_store_error(state, error))?
        {
            sealed += 1;
        }
    }
    Ok((sealed, backlog_may_remain))
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
    let (orphaned_claims_released, orphaned_claim_backlog_may_remain) =
        if matches!(admission, ProposalReconciliationAdmission::StartupInternal) {
            release_startup_artifact_claims_proven_before_effect(state, bounded_limit).await?
        } else {
            (0, false)
        };
    let (ambiguous_action_effects_marked_unknown, action_effect_backlog_may_remain) =
        if matches!(admission, ProposalReconciliationAdmission::StartupInternal) {
            seal_startup_governed_action_claims_as_unknown(state, bounded_limit).await?
        } else {
            (0, false)
        };
    let (reconciled_artifact_effects, artifact_effect_backlog_may_remain) =
        reconcile_artifact_effects_with_state(state, bounded_limit).await?;
    let artifact_effects_reconciled =
        orphaned_claims_released.saturating_add(reconciled_artifact_effects);
    let artifact_backlog_may_remain =
        orphaned_claim_backlog_may_remain || artifact_effect_backlog_may_remain;
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
    let canonical_record = if canonical_work_artifact_id(proposal).is_some() {
        match state.canonical_task_runtime_store.as_ref() {
            Some(store) => store
                .lock()
                .await
                .load_artifact_effect(&proposal.id)
                .map_err(|error| error.to_string())?,
            None => None,
        }
    } else {
        None
    };
    let legacy_record = if canonical_record.is_none() {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        store
            .artifact_effect(&proposal.id)
            .map_err(|error| runtime_proposal_store_error(state, error))?
    } else {
        None
    };
    let effect = if let Some(record) = canonical_record.filter(|record| {
        record.state == openlife_core::task_runtime::CanonicalArtifactEffectState::Confirmed
    }) {
        (
            record.dispatch_claim_id,
            record.target_reference_digest,
            record.content_digest,
            record.byte_size,
            record.media_type,
            record.observed_content_digest,
        )
    } else if let Some(record) =
        legacy_record.filter(|record| record.state == ArtifactEffectState::Confirmed)
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
        let safe_paths = artifact_safe_paths_for_proposal(state, proposal).await?;
        let (target_reference_digest, observation) =
            inspect_artifact_move(source, target, &content_digest, &safe_paths)?;
        if target_reference_digest != expected_target_reference_digest
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
            target_reference_digest,
            observed,
            byte_size,
            media_type,
        );
        receipt.artifact_id = artifact_id_for_proposal(proposal);
        return Ok(Some(receipt));
    }
    let resolved = resolve_artifact_effect_input(state, proposal).await?;
    let path = resolved.path.as_str();
    let content = resolved.content.as_slice();
    let safe_paths = artifact_safe_paths_for_proposal(state, proposal).await?;
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
    if operation == "trash" && proposal.after.get("undoOfArtifactId").is_some() {
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
    if canonical_work_artifact_id(proposal).is_none() {
        return Ok(());
    }
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
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
    if let Some(store) = state.canonical_task_runtime_store.as_ref() {
        let store = store.lock().await;
        if store
            .load_artifact_by_proposal(proposal_id)
            .map_err(|error| error.to_string())?
            .is_some()
        {
            if store
                .load_artifact_effect(proposal_id)
                .map_err(|error| error.to_string())?
                .is_some()
                && !store
                    .finish_artifact_effect_failed_before_effect(proposal_id, claim_id, error_code)
                    .map_err(|error| error.to_string())?
            {
                return Err("canonical_artifact_failed_before_effect_cas_lost".into());
            }
            return Ok(());
        }
    }
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await;
    if store
        .artifact_effect(proposal_id)
        .map_err(|error| runtime_proposal_store_error(state, error))?
        .is_some()
    {
        if !store
            .finish_artifact_failed_before_effect(proposal_id, claim_id, error_code)
            .map_err(|error| runtime_proposal_store_error(state, error))?
        {
            return Err("artifact_failed_before_effect_receipt_cas_lost".into());
        }
    } else if !store
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
    if let Some(store) = state.canonical_task_runtime_store.as_ref() {
        let store = store.lock().await;
        if store
            .load_artifact_by_proposal(proposal_id)
            .map_err(|error| error.to_string())?
            .is_some()
        {
            if !store
                .finish_artifact_effect_unknown(proposal_id, claim_id, error_code)
                .map_err(|error| error.to_string())?
            {
                return Err("canonical_artifact_unknown_receipt_cas_lost".into());
            }
            return Ok(());
        }
    }
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await;
    if !store
        .finish_artifact_unknown(proposal_id, claim_id, error_code)
        .map_err(|error| runtime_proposal_store_error(state, error))?
    {
        return Err("artifact_unknown_receipt_cas_lost".into());
    }
    Ok(())
}

async fn persist_artifact_confirmed(
    state: &Arc<AppState>,
    proposal_id: &str,
    claim_id: &str,
    observed_content_digest: &str,
) -> Result<(), String> {
    if let Some(store) = state.canonical_task_runtime_store.as_ref() {
        let store = store.lock().await;
        if store
            .load_artifact_effect(proposal_id)
            .map_err(|error| error.to_string())?
            .is_some()
        {
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
            return Ok(());
        }
    }
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await;
    if !store
        .finish_artifact_confirmed(proposal_id, claim_id, observed_content_digest)
        .map_err(|error| runtime_proposal_store_error(state, error))?
    {
        return Err("artifact_confirmed_receipt_cas_lost".into());
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
    let Some(artifact_id) = canonical_work_artifact_id(proposal) else {
        let path = proposal
            .after
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| "artifact_path_missing".to_string())?;
        let content = proposal
            .after
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("");
        return Ok(ResolvedArtifactEffectInput {
            artifact_id: artifact_id_for_proposal(proposal),
            path: path.to_string(),
            content: content.as_bytes().to_vec(),
            target_precondition: reviewed_artifact_target_precondition(&proposal.after)?,
        });
    };
    let expected_version = proposal
        .after
        .get("artifactVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| "canonical_artifact_version_missing".to_string())?;
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
    let resolved = match resolve_artifact_effect_input(state, proposal).await {
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
    let safe_paths = match artifact_safe_paths_for_proposal(state, proposal).await {
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
    if let Some(expected_hash) = proposal
        .after
        .get("content_hash")
        .and_then(Value::as_str)
        .filter(|hash| !hash.is_empty())
    {
        let expected_hash = if expected_hash.starts_with("sha256:") {
            expected_hash.to_string()
        } else {
            format!("sha256:{expected_hash}")
        };
        if expected_hash != prepared.content_digest {
            let code = "artifact_content_digest_mismatch";
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(code.into());
        }
    }
    let canonical_effect = canonical_work_artifact_id(proposal).is_some();
    let prepared_record: Result<(), String> = if canonical_effect {
        match state.canonical_task_runtime_store.as_ref() {
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
        }
    } else {
        let store = match state.proposal_store.as_ref() {
            Some(store) => store.lock().await,
            None => return ArtifactApplyOutcome::FailedBeforeEffect(proposal_store_missing()),
        };
        store
            .prepare_artifact_effect(
                &proposal.id,
                claim_id,
                &prepared.target_reference_digest,
                &prepared.content_digest,
                prepared.byte_size,
                &prepared.media_type,
            )
            .map(|_| ())
            .map_err(|error| runtime_proposal_store_error(state, error))
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
    let staged: Result<bool, String> = if canonical_effect {
        match state.canonical_task_runtime_store.as_ref() {
            Some(store) => store
                .lock()
                .await
                .mark_artifact_effect_staged(&proposal.id, claim_id)
                .map_err(|error| error.to_string()),
            None => Err("canonical_task_runtime_store_unavailable".into()),
        }
    } else {
        let store = state
            .proposal_store
            .as_ref()
            .expect("ProposalStore checked before artifact staging")
            .lock()
            .await;
        store
            .mark_artifact_staged(&proposal.id, claim_id)
            .map_err(|error| runtime_proposal_store_error(state, error))
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
        Ok(Err(failure)) => {
            let code = failure.code().to_string();
            let _ = persist_artifact_unknown(state, &proposal.id, claim_id, &code).await;
            return ArtifactApplyOutcome::Unknown(code);
        }
        Err(_) => {
            let code = "artifact_commit_worker_outcome_unknown";
            let _ = persist_artifact_unknown(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::Unknown(code.into());
        }
    };
    let confirmed: Result<bool, String> = if canonical_effect {
        match state.canonical_task_runtime_store.as_ref() {
            Some(store) => store
                .lock()
                .await
                .finish_artifact_effect_confirmed(&proposal.id, claim_id, &observed_digest)
                .map_err(|error| error.to_string()),
            None => Err("canonical_task_runtime_store_unavailable".into()),
        }
    } else {
        let store = state
            .proposal_store
            .as_ref()
            .expect("ProposalStore checked before artifact confirmation")
            .lock()
            .await;
        store
            .finish_artifact_confirmed(&proposal.id, claim_id, &observed_digest)
            .map_err(|error| runtime_proposal_store_error(state, error))
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
    let safe_paths = match artifact_safe_paths_for_proposal(state, proposal).await {
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
    let canonical_effect = canonical_work_artifact_id(proposal).is_some();
    let prepared_record: Result<(), String> = if canonical_effect {
        match state.canonical_task_runtime_store.as_ref() {
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
        }
    } else {
        let store = match state.proposal_store.as_ref() {
            Some(store) => store.lock().await,
            None => return ArtifactApplyOutcome::FailedBeforeEffect(proposal_store_missing()),
        };
        store
            .prepare_artifact_effect(
                &proposal.id,
                claim_id,
                &prepared.target_reference_digest,
                &prepared.content_digest,
                prepared.byte_size,
                &prepared.media_type,
            )
            .map(|_| ())
            .map_err(|error| runtime_proposal_store_error(state, error))
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
    let confirmed: Result<bool, String> = if canonical_effect {
        match state.canonical_task_runtime_store.as_ref() {
            Some(store) => store
                .lock()
                .await
                .finish_artifact_effect_confirmed(&proposal.id, claim_id, &observed_digest)
                .map_err(|error| error.to_string()),
            None => Err("canonical_task_runtime_store_unavailable".into()),
        }
    } else {
        let store = state
            .proposal_store
            .as_ref()
            .expect("ProposalStore checked before artifact move")
            .lock()
            .await;
        store
            .finish_artifact_confirmed(&proposal.id, claim_id, &observed_digest)
            .map_err(|error| runtime_proposal_store_error(state, error))
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

/// Validate that a DataExport filename is a single plain filename.
/// Rejects path traversal, absolute paths, and empty names.
fn validate_export_filename(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." {
        return Err("Filename cannot be empty, '.', or '..'.".to_string());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("Filename cannot contain path separators.".to_string());
    }
    if name.contains("..") {
        return Err("Filename cannot contain parent directory references.".to_string());
    }
    // Ensure it parses as a single normal filename component
    let path = std::path::Path::new(name);
    if path.components().count() != 1 {
        return Err("Filename must be a single component.".to_string());
    }
    if !matches!(
        path.components().next(),
        Some(std::path::Component::Normal(_))
    ) {
        return Err("Filename must be a normal file name.".to_string());
    }
    Ok(())
}

fn urlencoding(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len());
    for byte in s.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(*byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn validate_browser_open_url(raw: &str) -> Result<reqwest::Url, String> {
    match reqwest::Url::parse(raw) {
        Ok(url)
            if matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none() =>
        {
            Ok(url)
        }
        _ => {
            Err("Browser handoff requires a valid http(s) URL without embedded credentials.".into())
        }
    }
}

async fn run_bounded_local_utility(command: &str, timeout_ms: u64) -> Result<String, String> {
    let executable = local_utility_executable(command)
        .ok_or_else(|| "Local utility is not in the exact read-only allowlist.".to_string())?;
    if !(100..=3_000).contains(&timeout_ms) {
        return Err("Local utility timeout must be between 100 and 3000 ms.".into());
    }
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let executable = executable.to_string();
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        use std::process::{Command, Stdio};
        let mut child = Command::new(&executable)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Failed to start local utility: {error}"))?;
        let started = std::time::Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() < timeout => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Local utility timed out and was terminated.".into());
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("Failed to observe local utility: {error}"));
                }
            }
        };
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stdout.take() {
            pipe.read_to_string(&mut stdout)
                .map_err(|error| format!("Failed to read local utility output: {error}"))?;
        }
        if let Some(mut pipe) = child.stderr.take() {
            pipe.read_to_string(&mut stderr)
                .map_err(|error| format!("Failed to read local utility error output: {error}"))?;
        }
        if !status.success() {
            return Err(format!(
                "Local utility exited with {}: {}",
                status,
                stderr.chars().take(500).collect::<String>()
            ));
        }
        Ok(stdout.chars().take(4_000).collect::<String>())
    })
    .await
    .map_err(|_| "Local utility worker outcome is unknown.".to_string())?
}

fn local_utility_executable(command: &str) -> Option<&'static str> {
    match command {
        "date" => Some("/bin/date"),
        "uptime" => Some("/usr/bin/uptime"),
        "uname" => Some("/usr/bin/uname"),
        "whoami" => Some("/usr/bin/whoami"),
        _ => None,
    }
}

#[cfg(test)]
mod browser_handoff_contract_tests {
    use super::{
        run_bounded_local_utility, urlencoding, validate_browser_open_url,
        validate_proposal_payload,
    };
    use openlife_core::agent::ProposalType;

    #[test]
    fn browser_handoff_accepts_only_http_without_embedded_credentials() {
        assert!(validate_browser_open_url("https://example.com/report").is_ok());
        for rejected in [
            "file:///tmp/private",
            "javascript:alert(1)",
            "https://user:secret@example.com/private",
            "not a url",
        ] {
            assert!(validate_browser_open_url(rejected).is_err(), "{rejected}");
        }
    }

    #[test]
    fn mailto_components_are_encoded_as_utf8_bytes() {
        assert_eq!(
            urlencoding("会议 & update"),
            "%E4%BC%9A%E8%AE%AE%20%26%20update"
        );
    }

    #[tokio::test]
    async fn local_utility_is_exact_allowlist_only() {
        let output = run_bounded_local_utility("whoami", 3_000).await.unwrap();
        assert!(!output.trim().is_empty());
        for rejected in ["whoami --help", "sh", "/bin/date", "rm"] {
            assert!(run_bounded_local_utility(rejected, 3_000).await.is_err());
        }
    }

    #[test]
    fn governed_data_export_actions_validate_exact_arguments_before_dispatch() {
        assert!(validate_proposal_payload(
            ProposalType::DataExport,
            &serde_json::json!({
                "tool": "browser.open",
                "url": "https://example.com/report",
                "content": "Open reviewed URL",
            }),
        )
        .is_ok());
        assert!(validate_proposal_payload(
            ProposalType::DataExport,
            &serde_json::json!({
                "tool": "browser.open",
                "url": "file:///tmp/private",
                "content": "Invalid browser target",
            }),
        )
        .is_err());
        assert!(validate_proposal_payload(
            ProposalType::DataExport,
            &serde_json::json!({
                "tool": "email.propose_draft",
                "to": "alice@example.com",
                "subject": "Review",
                "content": "Missing exact body",
            }),
        )
        .is_err());
        for (command, timeout_ms) in [("whoami --help", 3_000), ("whoami", 3_001)] {
            assert!(validate_proposal_payload(
                ProposalType::DataExport,
                &serde_json::json!({
                    "tool": "local.run_utility",
                    "command": command,
                    "timeout_ms": timeout_ms,
                    "content": "Run reviewed utility",
                }),
            )
            .is_err());
        }
    }
}

fn parse_scheduled_at(value: &str) -> Result<Option<chrono::DateTime<chrono::Utc>>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(value) {
        return Ok(Some(parsed.with_timezone(&chrono::Utc)));
    }
    if let Ok(parsed) = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S") {
        return Ok(Some(parsed.and_utc()));
    }
    if let Ok(parsed) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Ok(Some(
            parsed
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| "ScheduledTask 日期超出有效范围。".to_string())?
                .and_utc(),
        ));
    }
    Err(
        "ScheduledTask scheduled_at/date 必须是 RFC3339、YYYY-MM-DDTHH:MM:SS 或 YYYY-MM-DD。"
            .to_string(),
    )
}

fn ics_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace("\r\n", "\\n")
        .replace(['\r', '\n'], "\\n")
}

/// Build a deterministic ICS (iCalendar) VEVENT from the reviewed proposal.
/// Stable UID/DTSTAMP values make an exact acceptance replay byte-identical.
fn build_ics_event(proposal: &AgentProposal, after: &Value) -> Result<String, String> {
    let now = proposal.created_at.format("%Y%m%dT%H%M%SZ").to_string();
    let uid = format!("openlife-{}@local", proposal.id);
    let title = ics_escape(
        after
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled Event"),
    );
    let description = ics_escape(
        after
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    let scheduled_at = after
        .get("scheduled_at")
        .or_else(|| after.get("date"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let scheduled = parse_scheduled_at(scheduled_at)?;
    let dtstart = scheduled
        .map(|value| value.format("%Y%m%dT%H%M%SZ").to_string())
        .unwrap_or_default();
    let dtend = scheduled
        .map(|value| {
            (value + chrono::Duration::hours(1))
                .format("%Y%m%dT%H%M%SZ")
                .to_string()
        })
        .unwrap_or_default();

    Ok(format!(
        "BEGIN:VCALENDAR\r\n\
         VERSION:2.0\r\n\
         PRODID:-//OpenLife//Calendar//EN\r\n\
         BEGIN:VEVENT\r\n\
         DTSTAMP:{now}\r\n\
         UID:{uid}\r\n\
         DTSTART:{dtstart}\r\n\
         DTEND:{dtend}\r\n\
         SUMMARY:{title}\r\n\
         DESCRIPTION:{description}\r\n\
         END:VEVENT\r\n\
         END:VCALENDAR\r\n"
    ))
}

/// Replace path-unsafe characters in a filename.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn calendar_projection_filename(proposal: &AgentProposal, title: &str) -> String {
    let mut title = sanitize_filename(title).trim().to_string();
    if title.is_empty() {
        title = "OpenLife event".into();
    }
    let digest = openlife_core::agent::metadata_safe_text_digest(&proposal.id).1;
    let token = digest
        .strip_prefix("sha256:")
        .unwrap_or(&digest)
        .chars()
        .take(16)
        .collect::<String>();
    format!("{title}-{token}.ics")
}

fn write_calendar_projection_once(
    proposal: &AgentProposal,
    after: &Value,
    safe_paths: &[String],
) -> Result<Option<std::path::PathBuf>, String> {
    if safe_paths.is_empty() {
        return Ok(None);
    }
    let title = after
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Untitled Event");
    let content = build_ics_event(proposal, after)?;
    let filename = calendar_projection_filename(proposal, title);
    let prepared = safe_paths
        .iter()
        .find_map(|safe_path| {
            let requested = std::path::Path::new(safe_path).join(&filename);
            prepare_artifact_materialization(
                &proposal.id,
                "calendar-ics-projection",
                &requested.to_string_lossy(),
                &content,
                safe_paths,
            )
            .ok()
        })
        .ok_or_else(|| "No valid safe path is available for the ICS projection.".to_string())?;

    if prepared.target_path.exists() {
        let existing = std::fs::read(&prepared.target_path).map_err(|error| {
            format!(
                "Failed to inspect existing ICS projection '{}': {error}",
                prepared.target_path.display()
            )
        })?;
        if existing != content.as_bytes() {
            return Err(format!(
                "ICS projection target '{}' already exists with different content.",
                prepared.target_path.display()
            ));
        }
    }

    stage_artifact_bytes(&prepared, &content)
        .map_err(|error| format!("Failed to stage ICS projection: {}", error.code()))?;
    commit_staged_artifact(&prepared, safe_paths)
        .map_err(|error| format!("Failed to commit ICS projection: {}", error.code()))?;
    Ok(Some(prepared.target_path))
}

#[cfg(test)]
mod calendar_projection_tests {
    use super::{build_ics_event, write_calendar_projection_once};
    use crate::artifact_materializer::{prepare_artifact_materialization, stage_artifact_bytes};
    use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
    use serde_json::json;

    fn proposal() -> AgentProposal {
        AgentProposal::new(
            ProposalType::ScheduledTask,
            "calendar.events",
            json!({
                "tool": "calendar.propose_event",
                "title": "Planning Review",
                "scheduled_at": "2026-08-12T09:00:00+08:00",
                "description": "Review current proposal",
            }),
            "reviewed local calendar projection",
            0.9,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        )
    }

    #[test]
    fn calendar_projection_is_deterministic_idempotent_and_never_overwrites_conflict() {
        let directory = tempfile::tempdir().unwrap();
        let proposal = proposal();
        let first_ics = build_ics_event(&proposal, &proposal.after).unwrap();
        let second_ics = build_ics_event(&proposal, &proposal.after).unwrap();
        assert_eq!(first_ics, second_ics);

        let safe_root = directory.path().canonicalize().unwrap();
        let safe_paths = vec![safe_root.to_string_lossy().into_owned()];
        let path = write_calendar_projection_once(&proposal, &proposal.after, &safe_paths)
            .unwrap()
            .expect("configured safe path creates a projection");
        let original = std::fs::read(&path).unwrap();
        assert_eq!(original, first_ics.as_bytes());

        assert_eq!(
            write_calendar_projection_once(&proposal, &proposal.after, &safe_paths)
                .unwrap()
                .as_deref(),
            Some(path.as_path())
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);

        std::fs::write(&path, b"unrelated existing file").unwrap();
        let error =
            write_calendar_projection_once(&proposal, &proposal.after, &safe_paths).unwrap_err();
        assert!(error.contains("already exists with different content"));
        assert_eq!(std::fs::read(&path).unwrap(), b"unrelated existing file");
    }

    #[test]
    fn calendar_projection_commits_and_cleans_a_matching_staged_file() {
        let directory = tempfile::tempdir().unwrap();
        let proposal = proposal();
        let content = build_ics_event(&proposal, &proposal.after).unwrap();
        let safe_root = directory.path().canonicalize().unwrap();
        let safe_paths = vec![safe_root.to_string_lossy().into_owned()];
        let filename = super::calendar_projection_filename(
            &proposal,
            proposal.after["title"].as_str().unwrap(),
        );
        let target = safe_root.join(filename);
        let prepared = prepare_artifact_materialization(
            &proposal.id,
            "calendar-ics-projection",
            &target.to_string_lossy(),
            &content,
            &safe_paths,
        )
        .unwrap();
        stage_artifact_bytes(&prepared, &content).unwrap();
        assert!(prepared.stage_path.exists());

        let path = write_calendar_projection_once(&proposal, &proposal.after, &safe_paths)
            .unwrap()
            .unwrap();

        assert_eq!(path, prepared.target_path);
        assert_eq!(std::fs::read(&path).unwrap(), content.as_bytes());
        assert!(!prepared.stage_path.exists());
    }
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
        ProposalType::LifeModelUpdate
        | ProposalType::GoalUpdate
        | ProposalType::StateUpdate
        | ProposalType::PreferenceUpdate
        | ProposalType::CapabilityUpdate => {
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
                _ => Err(format!(
                    "ExternalWriteAction Proposal 包含不受支持的 operation：{operation}。"
                )),
            }
        }
        ProposalType::ScheduledTask => {
            let title = after
                .get("title")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty());
            if title.is_none() {
                return Err("ScheduledTask Proposal 缺少 after.title（非空字符串）。".to_string());
            }
            if let Some(scheduled_at) = after
                .get("scheduled_at")
                .or_else(|| after.get("due_date"))
                .or_else(|| after.get("date"))
                .and_then(Value::as_str)
            {
                parse_scheduled_at(scheduled_at)?;
            }
            parse_reviewed_scheduled_provider_route(after)?;
            Ok(())
        }
        ProposalType::DataExport => {
            let content = after.get("content").and_then(Value::as_str);
            if content.is_none() {
                return Err("DataExport Proposal 缺少 after.content（字符串）。".to_string());
            }
            match after.get("tool").and_then(Value::as_str) {
                Some("browser.open") => {
                    let url = after
                        .get("url")
                        .and_then(Value::as_str)
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| "browser.open Proposal 缺少精确 after.url。".to_string())?;
                    validate_browser_open_url(url).map(|_| ())
                }
                Some("email.propose_draft") => {
                    for field in ["to", "subject", "body"] {
                        if after
                            .get(field)
                            .and_then(Value::as_str)
                            .is_none_or(|value| value.trim().is_empty())
                        {
                            return Err(format!(
                                "email.propose_draft Proposal 缺少精确 after.{field}。"
                            ));
                        }
                    }
                    Ok(())
                }
                Some("local.run_utility") => {
                    let command = after
                        .get("command")
                        .and_then(Value::as_str)
                        .filter(|value| local_utility_executable(value).is_some())
                        .ok_or_else(|| {
                            "local.run_utility Proposal 必须使用精确只读 allowlist command。"
                                .to_string()
                        })?;
                    debug_assert!(local_utility_executable(command).is_some());
                    let timeout_ms =
                        after
                            .get("timeout_ms")
                            .and_then(Value::as_u64)
                            .ok_or_else(|| {
                                "local.run_utility Proposal 缺少精确 after.timeout_ms。".to_string()
                            })?;
                    if !(100..=3_000).contains(&timeout_ms) {
                        return Err(
                            "local.run_utility Proposal timeout_ms 必须在 100..=3000。".to_string()
                        );
                    }
                    Ok(())
                }
                Some(other) => Err(format!("DataExport Proposal 工具不受支持：{other}。")),
                None => Ok(()),
            }
        }
        ProposalType::ModelPolicyChange
        | ProposalType::ScheduleCheckin
        | ProposalType::Unsupported => {
            // These types are not yet implemented; validation passes but apply will fail
            Ok(())
        }
    }
}

fn validate_proposal_for_acceptance(proposal: &AgentProposal) -> Result<(), String> {
    validate_proposal_payload(proposal.proposal_type, &proposal.after)?;
    if is_lifemodel_v2_typed_diff(proposal) {
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
    if is_legacy_lifemodel_v2_migration(proposal) {
        if proposal.source != ProposalSource::Manual
            || proposal.source_detail.as_deref() != Some("legacy_lifemodel_migration")
        {
            return Err("lifemodel_v2_migration_proposal_source_mismatch".into());
        }
        let plan = serde_json::from_value::<
            openlife_core::life_model::v2::LegacyLifeModelMigrationPlanV2,
        >(proposal.after.clone())
        .map_err(|_| "invalid_lifemodel_v2_migration_payload".to_string())?;
        plan.validate_contract()
            .map_err(|error| error.to_string())?;
        if proposal.base_hash.is_some() {
            return Err("lifemodel_v2_migration_proposal_base_must_be_empty".into());
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

#[derive(Debug, Clone)]
struct ReviewedScheduledProviderRoute {
    provider: String,
    model: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

fn parse_reviewed_scheduled_provider_route(
    after: &Value,
) -> Result<Option<ReviewedScheduledProviderRoute>, String> {
    let Some(route) = after.get("provider_route") else {
        return Ok(None);
    };
    let route = route
        .as_object()
        .ok_or_else(|| "ScheduledTask provider_route 必须是对象。".to_string())?;
    if route.get("data_route").and_then(Value::as_str) != Some("policy_allowed")
        || route.get("grant_scope").and_then(Value::as_str) != Some("single_execution")
        || route.get("consent_scope").and_then(Value::as_str) != Some("scheduled_provider_once")
    {
        return Err(
            "ScheduledTask 云路由必须显式声明 policy_allowed、single_execution 和 scheduled_provider_once。"
                .into(),
        );
    }
    let bounded_target = |name: &str| -> Result<String, String> {
        let value = route
            .get(name)
            .and_then(Value::as_str)
            .filter(|value| {
                !value.trim().is_empty()
                    && value.chars().count() <= 256
                    && !value
                        .chars()
                        .any(|character| character.is_control() || character.is_whitespace())
            })
            .ok_or_else(|| format!("ScheduledTask provider_route.{name} 无效。"))?;
        Ok(value.to_string())
    };
    let expires_at = route
        .get("expires_at")
        .and_then(Value::as_str)
        .ok_or_else(|| "ScheduledTask provider_route.expires_at 缺失。".to_string())?;
    let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at)
        .map_err(|_| "ScheduledTask provider_route.expires_at 必须是 RFC3339。".to_string())?
        .with_timezone(&chrono::Utc);
    if after
        .get("description")
        .and_then(Value::as_str)
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err("ScheduledTask 云路由必须绑定非空 description。".into());
    }
    if after
        .get("scheduled_at")
        .or_else(|| after.get("due_date"))
        .or_else(|| after.get("date"))
        .and_then(Value::as_str)
        .is_none()
    {
        return Err("ScheduledTask 云路由必须绑定 scheduled_at。".into());
    }
    Ok(Some(ReviewedScheduledProviderRoute {
        provider: bounded_target("provider")?,
        model: bounded_target("model")?,
        expires_at,
    }))
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
        ProposalType::LifeModelUpdate
        | ProposalType::GoalUpdate
        | ProposalType::StateUpdate
        | ProposalType::PreferenceUpdate
        | ProposalType::CapabilityUpdate => {
            if is_legacy_lifemodel_v2_migration(proposal) {
                let plan = serde_json::from_value::<
                    openlife_core::life_model::v2::LegacyLifeModelMigrationPlanV2,
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
        ProposalType::ScheduledTask => {
            let Some(review_acceptance) = review_acceptance else {
                return Ok(patch_result_for_proposal(
                    proposal,
                    false,
                    "scheduled_task_review_snapshot_missing",
                    Some("Scheduled task has no exact ReviewWorkflow acceptance snapshot.".into()),
                ));
            };
            let title = after
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Untitled Task");
            let scheduled_at = after
                .get("scheduled_at")
                .or_else(|| after.get("due_date"))
                .or_else(|| after.get("date"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let normalized_scheduled_at =
                parse_scheduled_at(scheduled_at)?.map(|value| value.to_rfc3339());

            let mut task = openlife_core::tasks::ScheduledTask::new(
                title,
                after
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                normalized_scheduled_at.clone(),
                after
                    .get("priority")
                    .and_then(Value::as_str)
                    .unwrap_or("medium"),
            );
            task.id = proposal.id.clone();
            task.source_run_id = proposal.run_id.clone();
            task.source_proposal_id = Some(proposal.id.clone());
            task.action_type = after
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or("scheduled_task")
                .to_string();
            if let Some(route) = parse_reviewed_scheduled_provider_route(&after)? {
                let Some(due_at) = normalized_scheduled_at.as_deref() else {
                    return Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "scheduled_cloud_due_time_missing",
                        Some("Scheduled cloud route requires a due time.".into()),
                    ));
                };
                let due_at = match chrono::DateTime::parse_from_rfc3339(due_at) {
                    Ok(value) => value.with_timezone(&chrono::Utc),
                    Err(_) => {
                        return Ok(patch_result_for_proposal(
                            proposal,
                            false,
                            "scheduled_cloud_due_time_invalid",
                            Some("Scheduled cloud route due time is invalid.".into()),
                        ))
                    }
                };
                let config = state.config.lock().await.clone();
                if route.provider != config.llm.provider
                    || route.model != config.llm.chat_model
                    || config.effective_cloud_api_key().trim().is_empty()
                {
                    return Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "scheduled_cloud_provider_preflight_failed",
                        Some(
                            "Reviewed scheduled provider/model is not the configured credentialed cloud target."
                                .into(),
                        ),
                    ));
                }
                let endpoint = openlife_core::llm::chat_completions_url(
                    &route.provider,
                    &config.effective_openai_base(),
                );
                let capability = format!("provider.{}", route.provider);
                let network_decision =
                    match openlife_core::network_client::resolve_network_policy_decision(
                        &config.system.network_policy,
                        &endpoint,
                        &capability,
                    ) {
                        Ok(decision) => decision,
                        Err(error) => {
                            return Ok(patch_result_for_proposal(
                                proposal,
                                false,
                                "scheduled_cloud_network_policy_invalid",
                                Some(error.to_string()),
                            ))
                        }
                    };
                if network_decision.disposition
                    != openlife_core::network_client::NetworkPolicyDisposition::Allow
                {
                    return Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "scheduled_cloud_network_policy_not_allowed",
                        Some(
                            "Scheduled cloud execution requires an already-allowed exact network policy; Ask or Deny cannot run unattended."
                                .into(),
                        ),
                    ));
                }
                let decision = match openlife_core::agent::main_chat_agent_v1::PolicyRouter
                    .authorize_scheduled_provider_route(
                        review_acceptance,
                        openlife_core::agent::main_chat_agent_v1::ScheduledProviderRouteRequest {
                            task_id: task.id.clone(),
                            description: task.description.clone(),
                            action_type: task.action_type.clone(),
                            due_at,
                            provider: route.provider,
                            model: route.model,
                            requested_data_route:
                                openlife_core::llm::ProviderDataRoute::PolicyAllowed,
                            grant_expires_at: route.expires_at,
                        },
                    ) {
                    Ok(decision) => decision,
                    Err(error) => {
                        return Ok(patch_result_for_proposal(
                            proposal,
                            false,
                            "scheduled_cloud_policy_rejected",
                            Some(error.to_string()),
                        ))
                    }
                };
                if let Err(error) = task.seal_reviewed_cloud_provider_grant(&decision) {
                    return Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "scheduled_cloud_grant_seal_rejected",
                        Some(error.to_string()),
                    ));
                }
            } else {
                task.seal_deterministic_local_provider_grant();
            }

            if let Err(e) = state.scheduled_task_store.create_task_idempotent(&task) {
                return Ok(patch_result_for_proposal(
                    proposal,
                    false,
                    "scheduled_task",
                    Some(format!("Failed to commit scheduled task: {}", e)),
                ));
            }

            // For calendar.propose_event, also write an .ics file if safe_paths allow
            let tool = after.get("tool").and_then(Value::as_str).unwrap_or("");
            let mut projection_warning = None;
            if tool == "calendar.propose_event" {
                let safe_paths = {
                    let cfg = state.config.lock().await;
                    cfg.system.safe_paths.clone()
                };
                if let Err(error) = write_calendar_projection_once(proposal, &after, &safe_paths) {
                    log::warn!("[proposal] Failed to create ICS projection: {error}");
                    projection_warning = Some(format!(
                        "projection_degraded: failed to materialize ICS view: {error}"
                    ));
                }
            }

            Ok(patch_result_for_proposal(
                proposal,
                true,
                if projection_warning.is_some() {
                    "scheduled_task_projection_degraded"
                } else {
                    "scheduled_task"
                },
                projection_warning,
            ))
        }
        ProposalType::DataExport => {
            let content = after.get("content").and_then(Value::as_str).unwrap_or("");
            let filename = after
                .get("filename")
                .and_then(Value::as_str)
                .unwrap_or("export.txt");
            let tool = after.get("tool").and_then(Value::as_str).unwrap_or("");

            if tool == "browser.open" {
                let raw_url = after.get("url").and_then(Value::as_str).unwrap_or("");
                let url = match validate_browser_open_url(raw_url) {
                    Ok(url) => url,
                    Err(error) => {
                        return Ok(patch_result_for_proposal(
                            proposal,
                            false,
                            "browser_open",
                            Some(error),
                        ))
                    }
                };
                match open::that(url.as_str()) {
                    Ok(_) => Ok(patch_result_for_proposal(
                        proposal,
                        true,
                        "browser_handoff_opened",
                        Some("The system accepted the browser handoff; page load and remote outcome remain unverified.".into()),
                    )),
                    Err(error) => Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "browser_open",
                        Some(format!("Failed to open system browser: {error}")),
                    )),
                }
            } else if tool == "local.run_utility" {
                let command = after.get("command").and_then(Value::as_str).unwrap_or("");
                let timeout_ms = after
                    .get("timeout_ms")
                    .and_then(Value::as_u64)
                    .unwrap_or(3_000);
                match run_bounded_local_utility(command, timeout_ms).await {
                    Ok(output) => Ok(patch_result_for_proposal(
                        proposal,
                        true,
                        "local_utility_completed",
                        Some(format!(
                            "Reviewed read-only utility completed. Output: {}",
                            output.trim()
                        )),
                    )),
                    Err(error) => Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "local_utility_failed",
                        Some(error),
                    )),
                }
            // email.propose_draft: open system mail client via mailto: URI
            } else if tool == "email.propose_draft" {
                let to = after.get("to").and_then(Value::as_str).unwrap_or("");
                let subject = after.get("subject").and_then(Value::as_str).unwrap_or("");
                let body = after.get("body").and_then(Value::as_str).unwrap_or(content);
                let mailto = format!(
                    "mailto:{}?subject={}&body={}",
                    urlencoding(to),
                    urlencoding(subject),
                    urlencoding(body)
                );
                match open::that(&mailto) {
                    Ok(_) => Ok(patch_result_for_proposal(
                        proposal,
                        true,
                        "email_draft_handoff_opened",
                        Some("The system accepted the email-draft handoff; OpenLife did not send the message and delivery remains unverified.".into()),
                    )),
                    Err(e) => Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "email_draft_handoff_failed",
                        Some(format!("Failed to open mail client: {}", e)),
                    )),
                }
            } else {
                // Default: write to file
                if let Err(e) = validate_export_filename(filename) {
                    return Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "data_export",
                        Some(e),
                    ));
                }
                let safe_paths = {
                    let cfg = state.config.lock().await;
                    cfg.system.safe_paths.clone()
                };
                let export_dir = if !safe_paths.is_empty() {
                    std::path::PathBuf::from(&safe_paths[0])
                } else {
                    app_data_dir().join("exports")
                };

                if let Err(e) = std::fs::create_dir_all(&export_dir) {
                    return Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "data_export",
                        Some(format!("Failed to create export directory: {}", e)),
                    ));
                }

                let export_path = export_dir.join(filename);
                match openlife_core::atomic_file::write_atomic(&export_path, content.as_bytes()) {
                    Ok(_) => Ok(patch_result_for_proposal(
                        proposal,
                        true,
                        "data_export",
                        None,
                    )),
                    Err(e) => Ok(patch_result_for_proposal(
                        proposal,
                        false,
                        "data_export",
                        Some(format!(
                            "Failed to write export file '{}': {}",
                            export_path.display(),
                            e
                        )),
                    )),
                }
            } // end else (non-email DataExport)
        }
        ProposalType::ModelPolicyChange
        | ProposalType::ScheduleCheckin
        | ProposalType::Unsupported => Err(format!(
            "{} Proposal 尚未接入应用器，已保持 pending。",
            proposal.proposal_type
        )),
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
        let mut response =
            confirmed_effect_reconciliation_response(&proposal, true, warnings, artifact_receipt);
        if let Some(learning) = learning {
            response["lifeModelLearning"] = learning;
        }
        response["canonical_task_runtime_projection_status"] =
            canonical_task_runtime_projection_status.into();
        return Ok(response);
    }
    ensure_pending_or_postponed(&proposal)?;
    validate_proposal_for_acceptance(&proposal)?;
    if matches!(
        proposal.proposal_type,
        ProposalType::ModelPolicyChange | ProposalType::ScheduleCheckin | ProposalType::Unsupported
    ) {
        return Err(format!(
            "{} Proposal 尚未接入应用器，已保持 pending。",
            proposal.proposal_type
        ));
    }
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
                mark_canonical_work_artifact_effect_failure(state, &proposal, &error, false).await;
                return Err(format!(
                    "Artifact materialization failed before effect: {error}"
                ));
            }
            ArtifactApplyOutcome::Unknown(error) => {
                mark_canonical_work_artifact_effect_failure(state, &proposal, &error, true).await;
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
    let canonical_artifact_effect =
        artifact_materialization.is_some() && canonical_work_artifact_id(&proposal).is_some();
    let effect_receipt_persisted = if artifact_materialization.is_some()
        && !canonical_artifact_effect
    {
        // Legacy Artifact effects advance the generic dispatch receipt atomically
        // inside ProposalStore::finish_artifact_confirmed.
        true
    } else {
        // Canonical Artifact effects are owned by CanonicalTaskRuntimeStore. The
        // ProposalStore still owns the Review dispatch checkpoint, so advance only
        // that generic receipt after the canonical effect is durably confirmed.
        match ensure_effect_dispatch_projection_pending(state, &proposal_id, &dispatch_claim_id)
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
        let decision = memory_gateway::memory_gateway_decision_for_proposal(
            &proposal,
            "accepted_proposal_materialization",
            Vec::new(),
        );
        response["memoryGateway"] =
            serde_json::to_value(&decision).unwrap_or(serde_json::Value::Null);
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

pub(crate) async fn reject_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    require_proposal_write_for(state, &proposal)?;
    if proposal.status == ProposalStatus::Rejected {
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
    update_review_proposal_before_dispatch_with_state(state, &proposal, expected_status).await?;
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
    check_safe_mode(state)?;
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
    let allow_empty_result = current.is_some()
        || manager
            .load_v2_cutover(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .map_err(|error| error.to_string())?
            .is_some();
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
    matches!(proposal.risk_level, RiskLevel::High | RiskLevel::Critical)
        || matches!(
            proposal.proposal_type,
            ProposalType::ToolPermission
                | ProposalType::ExternalWriteAction
                | ProposalType::ModelPolicyChange
                | ProposalType::DataExport
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

#[tauri::command]
pub async fn request_artifact_undo(
    artifact_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ArtifactUndoProposalResponse, String> {
    request_artifact_undo_with_state(artifact_id, state.inner()).await
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
    let original_proposal_id = canonical_store
        .lock()
        .await
        .load_task_snapshot(&artifact.task_id)
        .map_err(|error| error.to_string())?
        .and_then(|snapshot| {
            snapshot
                .artifacts
                .into_iter()
                .find(|snapshot| snapshot.artifact.id == artifact.id)
                .and_then(|snapshot| snapshot.review_checkpoint)
                .map(|checkpoint| checkpoint.proposal_id)
        })
        .ok_or_else(|| "canonical_artifact_review_origin_missing".to_string())?;
    let original_proposal = get_proposal_with_state(state, &original_proposal_id).await?;
    if original_proposal
        .after
        .get("expected_target_absent")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("canonical_artifact_undo_unavailable_without_original_bytes".into());
    }
    let safe_paths = artifact_safe_paths_for_proposal(state, &original_proposal).await?;
    let target = crate::artifact_materializer::trash_target_for_source(&source, &safe_paths)?;
    let prepared = crate::artifact_materializer::prepare_artifact_move(
        "artifact-undo-preview",
        &source,
        &target.to_string_lossy(),
        &artifact.content_digest,
        &safe_paths,
    )?;
    let task_snapshot = canonical_store
        .lock()
        .await
        .load_task_snapshot(&artifact.task_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_artifact_task_missing".to_string())?;
    let source_item = task_snapshot
        .items
        .iter()
        .find(|item| item.id == artifact.source_item_id)
        .ok_or_else(|| "canonical_artifact_source_item_missing".to_string())?;
    let mut proposal = AgentProposal::new(
        ProposalType::ExternalWriteAction,
        &format!(
            "filesystem.{}->{}",
            prepared.source_path.display(),
            prepared.target_path.display()
        ),
        serde_json::json!({
            "operation": "trash",
            "source_path": prepared.source_path,
            "target_path": prepared.target_path,
            "source_digest": prepared.content_digest,
            "contentDigest": artifact.content_digest,
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
    canonical_store
        .lock()
        .await
        .bind_artifact_undo(
            &artifact.id,
            review.proposal_id(),
            &source,
            &target.to_string_lossy(),
            &artifact.content_digest,
        )
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
