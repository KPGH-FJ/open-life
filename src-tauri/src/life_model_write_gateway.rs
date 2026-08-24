//! Canonical LifeModel v2 materialization boundary.
//!
//! The shipped product has one LifeModel authority: reviewed typed diffs
//! appended to the canonical v2 store. Historical YAML projections, patch
//! journals, and compatibility writers do not participate in this boundary.

use crate::persistence_coordinator::CanonicalWriteOwner;
use crate::AppState;
use openlife_core::agent::AgentProposal;
use openlife_core::life_model::legacy_migration::{
    LegacyLifeModelMigrationPlanV2, LegacyLifeModelMigrationPreviewV2,
    LIFE_MODEL_V2_LEGACY_MIGRATION_PATH,
};
use openlife_core::life_model::patch::PatchApplyResult;
use openlife_core::life_model::v2::{LifeModelTypedDiffV2, LIFE_MODEL_V2_TYPED_DIFF_PATH};
use std::sync::Arc;

pub(crate) async fn materialize_accepted_legacy_lifemodel_migration_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    plan: &LegacyLifeModelMigrationPlanV2,
) -> Result<PatchApplyResult, String> {
    let failed = |operation: &str, error: String| PatchApplyResult {
        patch_id: proposal.id.clone(),
        success: false,
        path: LIFE_MODEL_V2_LEGACY_MIGRATION_PATH.into(),
        operation: operation.into(),
        error: Some(error),
    };
    if proposal.proposal_type != openlife_core::agent::ProposalType::LifeModelUpdate
        || proposal.affected_path != LIFE_MODEL_V2_LEGACY_MIGRATION_PATH
        || proposal.source != openlife_core::agent::ProposalSource::Manual
        || proposal.source_detail.as_deref() != Some("legacy_lifemodel_migration")
        || proposal.base_hash.is_some()
    {
        return Ok(failed(
            "lifemodel_v2_migration_validation_failed",
            "lifemodel_v2_migration_proposal_identity_mismatch".into(),
        ));
    }
    if let Err(error) = plan.validate_contract() {
        return Ok(failed(
            "lifemodel_v2_migration_validation_failed",
            error.to_string(),
        ));
    }

    let write_admission = state
        .persistence_coordinator
        .admit_canonical_writes(&[CanonicalWriteOwner::LifeModelFileStore])
        .map_err(|error| error.to_string())?;
    let commit_permit = state
        .persistence_coordinator
        .acquire_canonical_commit_permit(&write_admission)
        .await
        .map_err(|error| error.to_string())?;
    let coordinator_guard = state.life_model_write_coordinator.lock().await;
    let manager = state.life_model_manager.lock().await;
    if manager
        .load_v2_current(&plan.model_id)
        .map_err(|error| error.to_string())?
        .is_some()
        || manager
            .load_v2_cutover(&plan.model_id)
            .map_err(|error| error.to_string())?
            .is_some()
    {
        return Ok(failed(
            "lifemodel_v2_migration_commit_conflict",
            "lifemodel_v2_migration_existing_canonical_owner".into(),
        ));
    }
    let source = match manager.load_legacy_source_for_migration() {
        Ok(Some(source)) => source,
        Ok(None) => {
            return Ok(failed(
                "lifemodel_v2_migration_source_changed",
                "legacy_lifemodel_source_missing".into(),
            ))
        }
        Err(error) => {
            return Ok(failed(
                "lifemodel_v2_migration_source_changed",
                error.to_string(),
            ))
        }
    };
    let preview = match LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(&source) {
        Ok(preview) => preview,
        Err(error) => {
            return Ok(failed(
                "lifemodel_v2_migration_source_changed",
                error.to_string(),
            ))
        }
    };
    if let Err(error) = preview.validate_migration_plan(plan) {
        return Ok(failed(
            "lifemodel_v2_migration_source_changed",
            error.to_string(),
        ));
    }
    let backup = match manager.backup_legacy_source_for_migration(&plan.legacy_source_digest) {
        Ok(receipt) => receipt,
        Err(error) => {
            return Ok(failed(
                "lifemodel_v2_migration_backup_failed",
                error.to_string(),
            ))
        }
    };
    let source_after_backup = match manager.load_legacy_source_for_migration() {
        Ok(Some(source)) => source,
        Ok(None) => {
            return Ok(failed(
                "lifemodel_v2_migration_source_changed",
                "legacy_lifemodel_source_missing_after_backup".into(),
            ))
        }
        Err(error) => {
            return Ok(failed(
                "lifemodel_v2_migration_source_changed",
                error.to_string(),
            ))
        }
    };
    let after_backup_preview =
        match LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(&source_after_backup) {
            Ok(preview) => preview,
            Err(error) => {
                return Ok(failed(
                    "lifemodel_v2_migration_source_changed",
                    error.to_string(),
                ))
            }
        };
    if after_backup_preview.source_digest != plan.legacy_source_digest {
        return Ok(failed(
            "lifemodel_v2_migration_source_changed",
            "legacy_lifemodel_source_changed_after_backup".into(),
        ));
    }

    let materialized = match manager.materialize_reviewed_legacy_v2_migration(
        plan,
        &proposal.id,
        &backup.backup_digest,
        &proposal.created_at.to_rfc3339(),
    ) {
        Ok(result) => result,
        Err(error)
            if [
                "lifemodel_v2_migration_existing_canonical_head",
                "lifemodel_v2_migration_cutover_identity_conflict",
            ]
            .iter()
            .any(|code| error.to_string().contains(code)) =>
        {
            return Ok(failed(
                "lifemodel_v2_migration_commit_conflict",
                error.to_string(),
            ));
        }
        Err(error) => return Err(format!("lifemodel_v2_migration_commit_unknown:{error}")),
    };
    drop(manager);
    drop(coordinator_guard);
    drop(commit_permit);

    let detail = serde_json::json!({
        "gateway": "CanonicalLifeModelMigrationMaterializer",
        "proposalId": proposal.id,
        "lane": "canonical_lifemodel_v2_migration",
        "modelVersion": materialized.version.model_version,
        "legacySourceDigest": materialized.cutover.legacy_source_digest,
        "backupDigest": materialized.cutover.backup_digest,
        "documentDigest": materialized.version.document_digest,
        "includedCandidateCount": plan.included_candidate_ids.len(),
        "excludedCandidateCount": plan.excluded_candidate_ids.len(),
        "replayed": materialized.replayed,
        "metadataSafe": true,
        "containsRawContent": false,
    });
    let feedback = state.feedback_store.lock().await;
    if let Err(error) = feedback.log_event(
        "lifemodel_v2_migration_materialized",
        None,
        Some(&detail.to_string()),
    ) {
        log::warn!("[CanonicalLifeModelMigrationMaterializer] audit log failed: {error}");
    }

    Ok(PatchApplyResult {
        patch_id: proposal.id.clone(),
        success: true,
        path: LIFE_MODEL_V2_LEGACY_MIGRATION_PATH.into(),
        operation: if materialized.replayed {
            "lifemodel_v2_migration_replayed".into()
        } else {
            "lifemodel_v2_migration_materialized".into()
        },
        error: None,
    })
}

pub(crate) async fn materialize_accepted_lifemodel_v2_typed_diff_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    diff: &LifeModelTypedDiffV2,
) -> Result<PatchApplyResult, String> {
    let failed = |operation: &str, error: String| PatchApplyResult {
        patch_id: proposal.id.clone(),
        success: false,
        path: LIFE_MODEL_V2_TYPED_DIFF_PATH.into(),
        operation: operation.into(),
        error: Some(error),
    };
    if proposal.proposal_type != openlife_core::agent::ProposalType::LifeModelUpdate
        || proposal.affected_path != LIFE_MODEL_V2_TYPED_DIFF_PATH
    {
        return Ok(failed(
            "lifemodel_v2_typed_diff_validation_failed",
            "lifemodel_v2_typed_diff_proposal_identity_mismatch".into(),
        ));
    }
    if let Err(error) = diff.validate_contract() {
        return Ok(failed(
            "lifemodel_v2_typed_diff_validation_failed",
            error.to_string(),
        ));
    }
    if proposal.base_hash.as_deref() != diff.base_document_digest.as_deref() {
        return Ok(failed(
            "lifemodel_v2_typed_diff_validation_failed",
            "lifemodel_v2_typed_diff_proposal_base_mismatch".into(),
        ));
    }

    let write_admission = state
        .persistence_coordinator
        .admit_canonical_writes(&[CanonicalWriteOwner::LifeModelFileStore])
        .map_err(|error| error.to_string())?;
    let commit_permit = state
        .persistence_coordinator
        .acquire_canonical_commit_permit(&write_admission)
        .await
        .map_err(|error| error.to_string())?;
    let coordinator_guard = state.life_model_write_coordinator.lock().await;
    let manager = state.life_model_manager.lock().await;
    let current = manager
        .load_v2_current(&diff.model_id)
        .map_err(|error| error.to_string())?;
    let current_digest = current
        .as_ref()
        .map(|version| version.document_digest.clone());
    let result_document =
        match diff.apply_to_version_for_review(current.as_ref(), current.is_some()) {
            Ok(document) => document,
            Err(error) => {
                return Ok(failed(
                    "lifemodel_v2_typed_diff_precondition_failed",
                    error.to_string(),
                ))
            }
        };

    let mut additional_source_refs = Vec::new();
    if let Some(learning) =
        openlife_core::agent::review_decision_context::build_review_decision_context(proposal, &[])
            .life_model_learning
    {
        additional_source_refs.push(format!(
            "lifemodel-learning-candidate:{}",
            learning.candidate_id
        ));
        additional_source_refs.extend(
            learning
                .observation_ids
                .into_iter()
                .map(|id| format!("lifemodel-learning-observation:{id}")),
        );
        additional_source_refs.extend(learning.source_refs);
        let Some(store) = state.life_model_learning_store.as_ref() else {
            return Ok(failed(
                "lifemodel_v2_typed_diff_precondition_failed",
                "lifemodel_learning_store_unavailable".into(),
            ));
        };
        let candidate = match store
            .lock()
            .await
            .get_candidate_by_proposal_id(&proposal.id)
        {
            Ok(Some(candidate)) => candidate,
            Ok(None) => {
                return Ok(failed(
                    "lifemodel_v2_typed_diff_precondition_failed",
                    "lifemodel_learning_materialization_candidate_missing".into(),
                ))
            }
            Err(error) => {
                return Ok(failed(
                    "lifemodel_v2_typed_diff_precondition_failed",
                    format!("lifemodel_learning_materialization_candidate_unavailable:{error}"),
                ))
            }
        };
        if candidate.id != learning.candidate_id {
            return Ok(failed(
                "lifemodel_v2_typed_diff_precondition_failed",
                "lifemodel_learning_materialization_candidate_mismatch".into(),
            ));
        }
        if let Some(id) = candidate.observation_ids.last() {
            additional_source_refs.push(format!("lifemodel-learning-observation:{id}"));
        }
        if let Some(source_ref) = candidate.source_refs.last() {
            additional_source_refs.push(source_ref.clone());
        }
    }
    if let Some(detail) = proposal
        .source_detail
        .as_deref()
        .and_then(|detail| detail.strip_prefix("lifemodel_v2_rollback:"))
    {
        let (target_version, target_digest) = detail
            .split_once(':')
            .ok_or_else(|| "lifemodel_v2_rollback_source_detail_invalid".to_string())?;
        let target_version = target_version
            .parse::<u64>()
            .map_err(|_| "lifemodel_v2_rollback_target_version_invalid".to_string())?;
        let target = manager
            .load_v2_version(&diff.model_id, target_version)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "lifemodel_v2_rollback_target_missing".to_string())?;
        if target.document_digest != target_digest
            || target.document_digest != diff.result_document_digest
            || target.document != result_document
        {
            return Ok(failed(
                "lifemodel_v2_typed_diff_precondition_failed",
                "lifemodel_v2_rollback_target_drift".into(),
            ));
        }
        additional_source_refs.push(format!(
            "lifemodel-version:{}:{}:{}",
            diff.model_id, target_version, target.document_digest
        ));
    }

    let materialized = match manager.materialize_reviewed_v2_typed_diff(
        diff,
        &proposal.id,
        &additional_source_refs,
        &proposal.created_at.to_rfc3339(),
    ) {
        Ok(result) => result,
        Err(error)
            if [
                "lifemodel_v2_parent_version_conflict",
                "lifemodel_v2_parent_digest_conflict",
                "lifemodel_v2_materialization_identity_conflict",
            ]
            .iter()
            .any(|code| error.to_string().contains(code)) =>
        {
            return Ok(failed(
                "lifemodel_v2_typed_diff_commit_conflict",
                error.to_string(),
            ));
        }
        Err(error) => return Err(format!("lifemodel_v2_materialization_unknown:{error}")),
    };
    drop(manager);
    drop(coordinator_guard);
    drop(commit_permit);

    record_materialization_audit(
        state,
        proposal,
        diff,
        current_digest.as_deref(),
        materialized.version.model_version,
        &materialized.version.document_digest,
        materialized.replayed,
    )
    .await;

    Ok(PatchApplyResult {
        patch_id: proposal.id.clone(),
        success: true,
        path: LIFE_MODEL_V2_TYPED_DIFF_PATH.into(),
        operation: if materialized.replayed {
            "lifemodel_v2_materialization_replayed".into()
        } else {
            "lifemodel_v2_materialized".into()
        },
        error: None,
    })
}

async fn record_materialization_audit(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    diff: &LifeModelTypedDiffV2,
    current_digest: Option<&str>,
    model_version: u64,
    document_digest: &str,
    replayed: bool,
) {
    let detail = serde_json::json!({
        "gateway": "CanonicalLifeModelMaterializer",
        "proposalId": proposal.id,
        "runId": proposal.run_id,
        "lane": "canonical_lifemodel_v2_truth",
        "modelId": diff.model_id,
        "modelVersion": model_version,
        "reasonCode": if replayed {
            "lifemodel_v2_materialization_replayed"
        } else {
            "lifemodel_v2_materialized"
        },
        "baseHash": diff.base_document_digest,
        "currentHash": current_digest,
        "afterHash": document_digest,
        "metadataSafe": true,
        "containsRawContent": false,
    });
    let feedback = state.feedback_store.lock().await;
    if let Err(error) = feedback.log_event(
        "lifemodel_v2_gateway_materialized",
        None,
        Some(&detail.to_string()),
    ) {
        log::warn!("[CanonicalLifeModelMaterializer] audit log failed: {error}");
    }
}
