use crate::errors::AppError;
use crate::life_model_materializer_guard::{
    ensure_lifemodel_materializer_caller_restriction, LifeModelMaterializerCallerContext,
    LifeModelMaterializerCallerKind, LifeModelMaterializerCallerPurpose,
    STATE_STORE_DAILY_TASK_COMPATIBILITY_MATERIALIZER_ID,
};
use crate::persistence_coordinator::{CanonicalCommitPermit, CanonicalWriteOwner};
use crate::AppState;
use openlife_core::agent::AgentProposal;
use openlife_core::life_model::patch::{LifeModelPatch, PatchApplyResult};
use openlife_core::life_model::v2::{
    LegacyLifeModelMigrationPlanV2, LegacyLifeModelMigrationPreviewV2, LifeModelTypedDiffV2,
    DEFAULT_LIFE_MODEL_V2_MODEL_ID, LIFE_MODEL_V2_LEGACY_MIGRATION_PATH,
    LIFE_MODEL_V2_TYPED_DIFF_PATH,
};
use openlife_core::life_model::LifeModel;
use openlife_core::life_model_write_gateway::{
    LifeModelWriteGateway, LifeModelWriteGatewayRequest, LifeModelWriteIntentKind,
};
use openlife_core::persistence_outbox::{
    FileMutationJournal, FileMutationState, FileProjectionDelivery,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[cfg(test)]
fn require_persistence_write(state: &Arc<AppState>) -> Result<(), String> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| error.to_string())
}

const LIFEMODEL_FILE_AGGREGATE_KIND: &str = "lifemodel_yaml";
const LIFEMODEL_FILE_AGGREGATE_ID: &str = "current";
const PROJECTION_DAILY_SNAPSHOT: &str = "daily_snapshot";
const PROJECTION_PATCH_AFTER_SNAPSHOT: &str = "patch_after_snapshot";
const PROJECTION_PATCH_STORE: &str = "patch_store";
const LIFEMODEL_FILE_RECONCILIATION_BATCH: usize = 256;

fn serialized_field_changed<T: serde::Serialize>(before: &T, after: &T) -> Result<bool, String> {
    let before = serde_json::to_value(before)
        .map_err(|error| format!("lifemodel_field_authority_before_encode_failed:{error}"))?;
    let after = serde_json::to_value(after)
        .map_err(|error| format!("lifemodel_field_authority_after_encode_failed:{error}"))?;
    Ok(before != after)
}

fn validate_lifemodel_field_authority(
    before: &LifeModel,
    after: &LifeModel,
    allow_statestore_compatibility_projection: bool,
) -> Result<(), String> {
    if !allow_statestore_compatibility_projection
        && serialized_field_changed(&before.goals.daily, &after.goals.daily)?
    {
        return Err("statestore_owned_path_changed:goals.daily".into());
    }
    if serialized_field_changed(&before.state.alerts, &after.state.alerts)?
        && !(allow_statestore_compatibility_projection && after.state.alerts.is_empty())
    {
        return Err("derived_projection_path_changed:state.alerts".into());
    }
    if allow_statestore_compatibility_projection {
        let mut before_without_projection = before.clone();
        let mut after_without_projection = after.clone();
        before_without_projection.goals.daily.clear();
        after_without_projection.goals.daily.clear();
        before_without_projection.state.alerts.clear();
        after_without_projection.state.alerts.clear();
        if serialized_field_changed(&before_without_projection, &after_without_projection)? {
            return Err("source_compatibility_changed_canonical_lifemodel_field".into());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct LifeModelProjectionPlan {
    create_daily_snapshot: bool,
    create_patch_after_snapshot: bool,
    patches: Vec<LifeModelPatch>,
}

impl LifeModelProjectionPlan {
    fn targets(&self) -> Vec<&'static str> {
        let mut targets = Vec::with_capacity(3);
        if self.create_daily_snapshot {
            targets.push(PROJECTION_DAILY_SNAPSHOT);
        }
        if self.create_patch_after_snapshot {
            targets.push(PROJECTION_PATCH_AFTER_SNAPSHOT);
        }
        if !self.patches.is_empty() {
            targets.push(PROJECTION_PATCH_STORE);
        }
        targets
    }
}

#[derive(Debug, Clone)]
struct LifeModelFileCommit {
    life_model: LifeModel,
    projection_degraded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifeModelFilePostCommitPolicy {
    ReconcileProjections,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LifeModelFileReconciliationReport {
    pub applied: usize,
    pub not_committed: usize,
    pub degraded: usize,
    pub backlog_may_remain: bool,
}

#[cfg(test)]
pub(crate) async fn reconcile_lifemodel_file_mutations_with_state(
    state: &Arc<AppState>,
) -> Result<LifeModelFileReconciliationReport, String> {
    require_persistence_write(state)?;
    reconcile_lifemodel_file_mutations_admitted(state).await
}

pub(crate) async fn reconcile_startup_lifemodel_file_mutations_with_state(
    state: &Arc<AppState>,
) -> Result<LifeModelFileReconciliationReport, String> {
    if !state
        .persistence_coordinator
        .startup_reconciliation_mutations_safe()
    {
        return Err("startup_lifemodel_reconciliation_mutations_unavailable".into());
    }
    reconcile_lifemodel_file_mutations_admitted(state).await
}

async fn reconcile_lifemodel_file_mutations_admitted(
    state: &Arc<AppState>,
) -> Result<LifeModelFileReconciliationReport, String> {
    let _coordinator = state.life_model_write_coordinator.lock().await;
    let result =
        reconcile_lifemodel_file_mutations_unlocked(state, DailySnapshotFaultInjection::None, true)
            .await;
    if let Err(error) = &result {
        state.persistence_coordinator.register_unavailable(
            "LifeModelFileJournal",
            "runtime_outbox_reconciliation_failure",
            error,
        );
    }
    result
}

async fn reconcile_lifemodel_file_mutations_unlocked(
    state: &Arc<AppState>,
    snapshot_fault: DailySnapshotFaultInjection,
    reconcile_mutation_state: bool,
) -> Result<LifeModelFileReconciliationReport, String> {
    let (canonical, journal_path) = {
        let manager = state.life_model_manager.lock().await;
        (
            manager.load().map_err(|error| error.to_string())?,
            manager.mutation_journal_path(),
        )
    };
    let canonical_digest = hash_life_model(&canonical).map_err(|error| error.to_string())?;
    let journal = FileMutationJournal::new(journal_path).map_err(|error| error.to_string())?;
    let reconciled = if reconcile_mutation_state {
        journal
            .reconcile_prepared(&canonical_digest)
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let mut report = LifeModelFileReconciliationReport::default();
    for (_, state) in &reconciled {
        match state {
            FileMutationState::NotCommitted => report.not_committed += 1,
            FileMutationState::Degraded | FileMutationState::Prepared => report.degraded += 1,
            FileMutationState::Committed => {}
        }
    }

    let open_patch_operations = if let Some(patch_store) = state.patch_store.as_ref() {
        patch_store
            .lock()
            .await
            .list_open_materialization_operations(LIFEMODEL_FILE_RECONCILIATION_BATCH)
            .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    for operation in open_patch_operations {
        match journal.operation_state(&operation.operation_id) {
            Ok(FileMutationState::NotCommitted) => {
                let patch_store = state.patch_store.as_ref().ok_or_else(|| {
                    "LifeModel patch store unavailable during recovery".to_string()
                })?;
                patch_store
                    .lock()
                    .await
                    .discard_not_committed_materialization_operation(&operation.operation_id)
                    .map_err(|error| error.to_string())?;
            }
            Ok(FileMutationState::Committed) => {}
            Ok(FileMutationState::Prepared | FileMutationState::Degraded) => {
                report.degraded += 1;
            }
            Err(error) => {
                return Err(format!(
                    "LifeModel patch stage references missing journal operation {}: {error}",
                    operation.operation_id
                ));
            }
        }
    }

    let deliveries = journal
        .list_replayable_deliveries(LIFEMODEL_FILE_RECONCILIATION_BATCH)
        .map_err(|error| error.to_string())?;
    for delivery in deliveries {
        if delivery.after_digest != canonical_digest {
            journal
                .mark_projection_degraded(
                    &delivery.operation_id,
                    &delivery.projection_target,
                    "canonical digest no longer matches projection source",
                )
                .map_err(|error| error.to_string())?;
            report.degraded += 1;
            continue;
        }
        match dispatch_lifemodel_file_projection(state, &canonical, &delivery, snapshot_fault).await
        {
            Ok(()) => {
                journal
                    .mark_projection_applied(&delivery.operation_id, &delivery.projection_target)
                    .map_err(|error| error.to_string())?;
                report.applied += 1;
            }
            Err(error) => {
                journal
                    .mark_projection_degraded(
                        &delivery.operation_id,
                        &delivery.projection_target,
                        &error,
                    )
                    .map_err(|journal_error| journal_error.to_string())?;
                report.degraded += 1;
            }
        }
    }

    let journal_backlog = !journal
        .list_replayable_deliveries(1)
        .map_err(|error| error.to_string())?
        .is_empty();
    let patch_backlog = if let Some(store) = state.patch_store.as_ref() {
        !store
            .lock()
            .await
            .list_open_materialization_operations(1)
            .map_err(|error| error.to_string())?
            .is_empty()
    } else {
        false
    };
    let unresolved_mutation = journal
        .unresolved_operation(LIFEMODEL_FILE_AGGREGATE_KIND, LIFEMODEL_FILE_AGGREGATE_ID)
        .map_err(|error| error.to_string())?;
    if unresolved_mutation.as_ref().is_some_and(|(_, state)| {
        matches!(
            state,
            FileMutationState::Prepared | FileMutationState::Degraded
        )
    }) {
        report.degraded += 1;
    }
    report.backlog_may_remain = journal_backlog || patch_backlog || unresolved_mutation.is_some();
    Ok(report)
}

async fn dispatch_lifemodel_file_projection(
    state: &Arc<AppState>,
    canonical: &LifeModel,
    delivery: &FileProjectionDelivery,
    _snapshot_fault: DailySnapshotFaultInjection,
) -> Result<(), String> {
    match delivery.projection_target.as_str() {
        PROJECTION_PATCH_STORE => {
            let patch_store = state
                .patch_store
                .as_ref()
                .ok_or_else(|| "LifeModel patch store unavailable".to_string())?;
            patch_store
                .lock()
                .await
                .apply_materialization_operation(&delivery.operation_id)
                .map_err(|error| error.to_string())?;
            Ok(())
        }
        PROJECTION_PATCH_AFTER_SNAPSHOT => {
            let patch_store = state
                .patch_store
                .as_ref()
                .ok_or_else(|| "LifeModel patch store unavailable".to_string())?;
            let proposal_id = patch_store
                .lock()
                .await
                .materialization_operation_proposal_id(&delivery.operation_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    "LifeModel patch projection proposal reference missing".to_string()
                })?;
            let projection_key =
                format!("{}:{}", delivery.operation_id, delivery.projection_target);
            state
                .version_manager
                .lock()
                .await
                .ensure_projection_snapshot(
                    canonical,
                    &projection_key,
                    &format!("patch:{proposal_id}:after"),
                    &format!("Snapshot after patch {proposal_id}"),
                )
                .map_err(|error| error.to_string())?;
            Ok(())
        }
        PROJECTION_DAILY_SNAPSHOT => {
            #[cfg(test)]
            if _snapshot_fault == DailySnapshotFaultInjection::LookupFailure {
                return Err("injected_daily_snapshot_lookup_failure".into());
            }
            let projection_key =
                format!("{}:{}", delivery.operation_id, delivery.projection_target);
            state
                .version_manager
                .lock()
                .await
                .ensure_projection_snapshot(
                    canonical,
                    &projection_key,
                    "auto:daily-save",
                    "Daily auto snapshot",
                )
                .map_err(|error| error.to_string())?;
            *state.last_snapshot_date.lock().await =
                Some(chrono::Local::now().format("%Y-%m-%d").to_string());
            Ok(())
        }
        other => Err(format!(
            "unsupported LifeModel file projection target: {other}"
        )),
    }
}

async fn ensure_no_lifemodel_file_backlog_unlocked(state: &Arc<AppState>) -> Result<(), String> {
    let report = reconcile_lifemodel_file_mutations_unlocked(
        state,
        DailySnapshotFaultInjection::None,
        false,
    )
    .await?;
    if report.degraded > 0 || report.backlog_may_remain {
        return Err(format!(
            "LifeModel canonical write blocked by degraded projection recovery: degraded={}, backlog={}",
            report.degraded, report.backlog_may_remain
        ));
    }
    Ok(())
}

pub(crate) async fn persist_life_model_with_gateway_expected(
    state: &Arc<AppState>,
    life_model: LifeModel,
    create_daily_snapshot: bool,
    caller_context: LifeModelMaterializerCallerContext,
    expected_before_hash: Option<&str>,
) -> Result<LifeModel, String> {
    let write_admission = state
        .persistence_coordinator
        .admit_canonical_writes(&[CanonicalWriteOwner::LifeModelFileStore])
        .map_err(|error| error.to_string())?;
    let commit_permit = state
        .persistence_coordinator
        .acquire_canonical_commit_permit(&write_admission)
        .await
        .map_err(|error| error.to_string())?;
    ensure_lifemodel_materializer_caller_restriction(&caller_context, "persist_life_model")?;
    let _coordinator = state.life_model_write_coordinator.lock().await;
    ensure_no_lifemodel_file_backlog_unlocked(state).await?;
    let previous_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|error| error.to_string())?
    };
    let previous_hash = hash_life_model(&previous_model).map_err(|error| error.to_string())?;
    if expected_before_hash.is_some_and(|expected| expected != previous_hash) {
        return Err("LifeModel changed after required pre-change snapshot".into());
    }
    let source_compatibility_projection = matches!(
        (caller_context.kind, caller_context.purpose),
        (
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
        )
    ) && caller_context.stable_id
        == STATE_STORE_DAILY_TASK_COMPATIBILITY_MATERIALIZER_ID;
    validate_lifemodel_field_authority(
        &previous_model,
        &life_model,
        source_compatibility_projection,
    )?;
    let request = gateway_request_for_caller(&caller_context, Some(&previous_model), &life_model)
        .map_err(|e| e.to_string())?;
    let decision = LifeModelWriteGateway::decide(request);
    if !decision.allowed {
        return Err(format!(
            "LifeModelWriteGateway blocked persist_life_model: {}",
            decision.reason_code
        ));
    }

    let written = if source_compatibility_projection {
        // A compatibility view must be byte-for-byte scoped to its owned
        // projection fields. The general save preparation mutates LifeModel
        // metadata, so it is deliberately bypassed for this exact lane.
        write_life_model_without_prepare(
            state,
            &previous_model,
            &life_model,
            commit_permit,
            LifeModelFilePostCommitPolicy::ReconcileProjections,
        )
        .await
        .map_err(|error| error.to_string())?;
        life_model
    } else {
        write_life_model(
            state,
            &previous_model,
            life_model,
            create_daily_snapshot,
            commit_permit,
            LifeModelFilePostCommitPolicy::ReconcileProjections,
        )
        .await?
    };
    record_lifemodel_gateway_audit(
        state,
        "lifemodel_gateway_persist",
        None,
        None,
        None,
        &decision.reason_code,
        None,
        decision.base_hash.as_deref(),
        decision.current_hash.as_deref(),
        decision.before_hash.as_deref(),
        decision.after_hash.as_deref(),
        &decision.lane,
    )
    .await;
    Ok(written)
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
    let allow_empty_result = current.is_some()
        || manager
            .load_v2_cutover(&diff.model_id)
            .map_err(|error| error.to_string())?
            .is_some();
    let result_document =
        match diff.apply_to_version_for_review(current.as_ref(), allow_empty_result) {
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

    let created_at = proposal.created_at.to_rfc3339();
    let materialized = match manager.materialize_reviewed_v2_typed_diff(
        diff,
        &proposal.id,
        &additional_source_refs,
        &created_at,
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

    let snapshot_version = materialized.version.model_version.to_string();
    record_lifemodel_gateway_audit(
        state,
        "lifemodel_v2_gateway_materialized",
        Some(proposal),
        Some(&proposal.id),
        Some(&snapshot_version),
        if materialized.replayed {
            "lifemodel_v2_materialization_replayed"
        } else {
            "lifemodel_v2_materialized"
        },
        None,
        diff.base_document_digest.as_deref(),
        current_digest.as_deref(),
        current_digest.as_deref(),
        Some(&materialized.version.document_digest),
        "canonical_lifemodel_v2_truth",
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
        || proposal.base_hash.is_some()
        || proposal.source != openlife_core::agent::ProposalSource::Manual
        || proposal.source_detail.as_deref() != Some("legacy_lifemodel_migration")
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
    if plan.model_id != DEFAULT_LIFE_MODEL_V2_MODEL_ID {
        return Ok(failed(
            "lifemodel_v2_migration_validation_failed",
            "lifemodel_v2_migration_model_id_mismatch".into(),
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
        .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
        .map_err(|error| error.to_string())?
        .is_some()
        || manager
            .load_v2_cutover(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .map_err(|error| error.to_string())?
            .is_some()
    {
        return Ok(failed(
            "lifemodel_v2_migration_commit_conflict",
            "lifemodel_v2_migration_existing_canonical_owner".into(),
        ));
    }

    let Some((_, source)) = manager
        .load_existing_with_source()
        .map_err(|error| error.to_string())?
    else {
        return Ok(failed(
            "lifemodel_v2_migration_source_changed",
            "legacy_lifemodel_source_missing".into(),
        ));
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

    let backup = match manager.prepare_legacy_v2_backup(&plan.legacy_source_digest) {
        Ok(backup) => backup,
        Err(error) => {
            return Ok(failed(
                "lifemodel_v2_migration_backup_failed",
                error.to_string(),
            ))
        }
    };
    if let Err(error) = manager.verify_legacy_source_digest(&plan.legacy_source_digest) {
        return Ok(failed(
            "lifemodel_v2_migration_source_changed",
            error.to_string(),
        ));
    }

    let cutover_at = proposal.created_at.to_rfc3339();
    let materialized = match manager.materialize_reviewed_legacy_v2_migration(
        plan,
        &proposal.id,
        &backup.backup_digest,
        &cutover_at,
    ) {
        Ok(result) => result,
        Err(error)
            if [
                "lifemodel_v2_migration_existing_canonical_head",
                "lifemodel_v2_migration_cutover_identity_conflict",
                "lifemodel_v2_materialization_identity_conflict",
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

    let version = materialized.version.model_version.to_string();
    record_lifemodel_gateway_audit(
        state,
        "lifemodel_v2_migration_materialized",
        Some(proposal),
        Some(&proposal.id),
        Some(&version),
        if materialized.replayed {
            "lifemodel_v2_migration_replayed"
        } else {
            "lifemodel_v2_migration_cutover_committed"
        },
        None,
        Some(&plan.legacy_source_digest),
        Some(&plan.legacy_source_digest),
        Some(&backup.backup_digest),
        Some(&materialized.version.document_digest),
        "canonical_lifemodel_v2_migration",
    )
    .await;

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

pub(crate) async fn stamp_lifemodel_proposal_base_hash_with_state(
    state: &Arc<AppState>,
    proposal: &mut AgentProposal,
) -> Result<(), String> {
    if !lifemodel_proposal_type(proposal) || proposal.base_hash.is_some() {
        return Ok(());
    }
    let current_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    proposal.base_hash =
        Some(hash_canonical_lifemodel_semantics(&current_model).map_err(|e| e.to_string())?);
    Ok(())
}

fn lifemodel_proposal_type(proposal: &AgentProposal) -> bool {
    matches!(
        proposal.proposal_type,
        openlife_core::agent::ProposalType::LifeModelUpdate
            | openlife_core::agent::ProposalType::GoalUpdate
            | openlife_core::agent::ProposalType::StateUpdate
            | openlife_core::agent::ProposalType::PreferenceUpdate
            | openlife_core::agent::ProposalType::CapabilityUpdate
    )
}

async fn write_life_model(
    state: &Arc<AppState>,
    previous_model: &LifeModel,
    mut life_model: LifeModel,
    create_daily_snapshot: bool,
    commit_permit: CanonicalCommitPermit<'_>,
    post_commit_policy: LifeModelFilePostCommitPolicy,
) -> Result<LifeModel, String> {
    let expected_hash = hash_life_model(previous_model).map_err(|error| error.to_string())?;
    openlife_core::versioning::prepare_model_for_save(Some(previous_model), &mut life_model);
    let plan = LifeModelProjectionPlan {
        create_daily_snapshot,
        ..LifeModelProjectionPlan::default()
    };
    let commit = write_prepared_life_model_compare_and_swap(
        state,
        &expected_hash,
        life_model,
        plan,
        commit_permit,
        FileWriteFaultInjection::None,
        DailySnapshotFaultInjection::None,
        post_commit_policy,
    )
    .await?
    .ok_or_else(|| "LifeModel compare-and-swap conflict".to_string())?;
    if commit.projection_degraded {
        log::warn!(
            "[LifeModelWriteGateway] canonical LifeModel commit succeeded with degraded projections"
        );
    }
    Ok(commit.life_model)
}

#[cfg(test)]
async fn write_life_model_compare_and_swap(
    state: &Arc<AppState>,
    expected_hash: &str,
    life_model: LifeModel,
    create_daily_snapshot: bool,
) -> Result<Option<(LifeModel, bool)>, String> {
    write_life_model_compare_and_swap_with_snapshot_fault(
        state,
        expected_hash,
        life_model,
        create_daily_snapshot,
        DailySnapshotFaultInjection::None,
    )
    .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DailySnapshotFaultInjection {
    None,
    #[cfg(test)]
    LookupFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileWriteFaultInjection {
    None,
    #[cfg(test)]
    StopAfterStageBeforeCanonical,
    #[cfg(test)]
    StopAfterCanonicalBeforeObserve,
}

#[cfg(test)]
async fn write_life_model_compare_and_swap_with_snapshot_fault(
    state: &Arc<AppState>,
    expected_hash: &str,
    mut life_model: LifeModel,
    create_daily_snapshot: bool,
    snapshot_fault: DailySnapshotFaultInjection,
) -> Result<Option<(LifeModel, bool)>, String> {
    let write_admission = state
        .persistence_coordinator
        .admit_canonical_writes(&[CanonicalWriteOwner::LifeModelFileStore])
        .map_err(|error| error.to_string())?;
    let commit_permit = state
        .persistence_coordinator
        .acquire_canonical_commit_permit(&write_admission)
        .await
        .map_err(|error| error.to_string())?;
    let _coordinator = state.life_model_write_coordinator.lock().await;
    ensure_no_lifemodel_file_backlog_unlocked(state).await?;
    let current = {
        let manager = state.life_model_manager.lock().await;
        let current = manager.load().map_err(|error| error.to_string())?;
        let current_hash = hash_life_model(&current).map_err(|error| error.to_string())?;
        if current_hash != expected_hash {
            return Ok(None);
        }
        current
    };
    openlife_core::versioning::prepare_model_for_save(Some(&current), &mut life_model);
    let plan = LifeModelProjectionPlan {
        create_daily_snapshot,
        ..LifeModelProjectionPlan::default()
    };
    Ok(write_prepared_life_model_compare_and_swap(
        state,
        expected_hash,
        life_model,
        plan,
        commit_permit,
        FileWriteFaultInjection::None,
        snapshot_fault,
        LifeModelFilePostCommitPolicy::ReconcileProjections,
    )
    .await?
    .map(|commit| (commit.life_model, commit.projection_degraded)))
}

// The file CAS binds the expected hash, prepared model, admission, journal,
// and post-commit policy as separate authority-bearing inputs.
#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
async fn write_prepared_life_model_compare_and_swap(
    state: &Arc<AppState>,
    expected_hash: &str,
    life_model: LifeModel,
    mut projection_plan: LifeModelProjectionPlan,
    commit_permit: CanonicalCommitPermit<'_>,
    _write_fault: FileWriteFaultInjection,
    snapshot_fault: DailySnapshotFaultInjection,
    _post_commit_policy: LifeModelFilePostCommitPolicy,
) -> Result<Option<LifeModelFileCommit>, String> {
    let after_digest = hash_life_model(&life_model).map_err(|error| error.to_string())?;
    if after_digest == expected_hash {
        return Ok(Some(LifeModelFileCommit {
            life_model,
            projection_degraded: false,
        }));
    }
    if projection_plan.create_daily_snapshot {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        match state
            .version_manager
            .lock()
            .await
            .has_snapshot_tag_on_date("auto:daily-save", &today)
        {
            Ok(true) => projection_plan.create_daily_snapshot = false,
            Ok(false) => {}
            Err(error) => {
                log::warn!(
                    "[LifeModelWriteGateway] daily snapshot lookup degraded before canonical commit; durable projection retains the failure: {}",
                    error
                );
            }
        }
    }
    let journal_path = {
        state
            .life_model_manager
            .lock()
            .await
            .mutation_journal_path()
    };
    let journal = FileMutationJournal::new(journal_path).map_err(|error| error.to_string())?;
    let targets = projection_plan.targets();
    let mutation_kind = "updated";
    let receipt = journal
        .prepare(
            LIFEMODEL_FILE_AGGREGATE_KIND,
            LIFEMODEL_FILE_AGGREGATE_ID,
            mutation_kind,
            expected_hash,
            &after_digest,
            &targets,
        )
        .map_err(|error| error.to_string())?;

    if !projection_plan.patches.is_empty() {
        let patch_store = state.patch_store.as_ref().ok_or_else(|| {
            "LifeModel patch store unavailable before canonical commit".to_string()
        })?;
        if let Err(error) = patch_store.lock().await.stage_materialization_patches(
            &receipt.operation_id,
            expected_hash,
            &after_digest,
            &projection_plan.patches,
        ) {
            let _ = journal.mark_not_committed(&receipt.operation_id);
            return Err(format!(
                "LifeModel patch staging failed before canonical commit: {error}"
            ));
        }
    }

    #[cfg(test)]
    if _write_fault == FileWriteFaultInjection::StopAfterStageBeforeCanonical {
        return Err("injected_stop_after_lifemodel_stage".into());
    }

    let (save_result, observed_digest) = {
        let manager = state.life_model_manager.lock().await;
        let current = manager.load().map_err(|error| error.to_string())?;
        let current_hash = hash_life_model(&current).map_err(|error| error.to_string())?;
        if current_hash != expected_hash {
            drop(manager);
            journal
                .mark_not_committed(&receipt.operation_id)
                .map_err(|error| error.to_string())?;
            if let Some(patch_store) = state.patch_store.as_ref() {
                patch_store
                    .lock()
                    .await
                    .discard_not_committed_materialization_operation(&receipt.operation_id)
                    .map_err(|error| error.to_string())?;
            }
            return Ok(None);
        }
        let save_result = manager.save(&life_model).map_err(|error| error.to_string());
        let observed = manager.load().map_err(|error| error.to_string())?;
        let observed_digest = hash_life_model(&observed).map_err(|error| error.to_string())?;
        (save_result, observed_digest)
    };

    #[cfg(test)]
    if save_result.is_ok()
        && _write_fault == FileWriteFaultInjection::StopAfterCanonicalBeforeObserve
    {
        return Err("injected_stop_after_lifemodel_canonical_rename".into());
    }

    if let Err(error) = save_result {
        state.persistence_coordinator.register_unavailable(
            "LifeModelFileStore",
            "runtime_canonical_write_failure",
            &error,
        );
        if observed_digest == *expected_hash {
            journal
                .observe_canonical_digest(&receipt.operation_id, &observed_digest)
                .map_err(|journal_error| journal_error.to_string())?;
            if let Some(patch_store) = state.patch_store.as_ref() {
                patch_store
                    .lock()
                    .await
                    .discard_not_committed_materialization_operation(&receipt.operation_id)
                    .map_err(|stage_error| stage_error.to_string())?;
            }
        } else {
            journal
                .mark_degraded(&receipt.operation_id, &error)
                .map_err(|journal_error| journal_error.to_string())?;
        }
        return Err(format!(
            "LifeModel canonical write durability is not confirmed: {error}"
        ));
    }

    match journal
        .observe_canonical_digest(&receipt.operation_id, &observed_digest)
        .map_err(|error| error.to_string())?
    {
        FileMutationState::Committed => {}
        FileMutationState::NotCommitted => {
            if let Some(patch_store) = state.patch_store.as_ref() {
                patch_store
                    .lock()
                    .await
                    .discard_not_committed_materialization_operation(&receipt.operation_id)
                    .map_err(|error| error.to_string())?;
            }
            return Ok(None);
        }
        FileMutationState::Prepared | FileMutationState::Degraded => {
            return Err("LifeModel canonical digest diverged after atomic write".into());
        }
    }

    drop(commit_permit);

    let projection_degraded = match reconcile_lifemodel_file_mutations_unlocked(
        state,
        snapshot_fault,
        false,
    )
    .await
    {
        Ok(report) => report.degraded > 0 || report.backlog_may_remain,
        Err(error) => {
            log::warn!(
                "[LifeModelWriteGateway] canonical commit succeeded but projection reconciliation failed: {}",
                error
            );
            true
        }
    };
    Ok(Some(LifeModelFileCommit {
        life_model,
        projection_degraded,
    }))
}

async fn write_life_model_without_prepare(
    state: &Arc<AppState>,
    previous_model: &LifeModel,
    life_model: &LifeModel,
    commit_permit: CanonicalCommitPermit<'_>,
    post_commit_policy: LifeModelFilePostCommitPolicy,
) -> Result<(), AppError> {
    let expected_hash = hash_life_model(previous_model).map_err(AppError::from)?;
    write_prepared_life_model_compare_and_swap(
        state,
        &expected_hash,
        life_model.clone(),
        LifeModelProjectionPlan::default(),
        commit_permit,
        FileWriteFaultInjection::None,
        DailySnapshotFaultInjection::None,
        post_commit_policy,
    )
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::internal("LifeModel compare-and-swap conflict"))?;
    Ok(())
}

fn gateway_request_for_caller(
    caller_context: &LifeModelMaterializerCallerContext,
    previous_model: Option<&LifeModel>,
    after: &LifeModel,
) -> Result<LifeModelWriteGatewayRequest, AppError> {
    let before_hash = previous_model.and_then(|model| hash_life_model(model).ok());
    let after_hash = hash_life_model(after).ok();
    let request = match (caller_context.kind, caller_context.purpose) {
        (
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
        ) => {
            if caller_context.stable_id != STATE_STORE_DAILY_TASK_COMPATIBILITY_MATERIALIZER_ID {
                return Err(AppError::permission(
                    "source-data compatibility LifeModel writes require the StateStore daily-task projector identity",
                ));
            }
            LifeModelWriteGatewayRequest {
                intent: LifeModelWriteIntentKind::SourceDataCompatibility,
                proposal_id: None,
                run_id: None,
                evidence_id: None,
                base_hash: None,
                current_hash: None,
                before_hash,
                after_hash,
                explicit_manual_override: false,
                risk_acknowledged: false,
            }
        }
        _ => {
            return Err(AppError::permission(format!(
                "LifeModelWriteGateway does not allow caller {} to persist LifeModel",
                caller_context.stable_id
            )))
        }
    };
    Ok(request)
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
async fn record_lifemodel_gateway_audit(
    state: &Arc<AppState>,
    event_name: &str,
    proposal: Option<&AgentProposal>,
    patch_id: Option<&str>,
    snapshot_version: Option<&str>,
    reason_code: &str,
    conflict_status: Option<&str>,
    base_hash: Option<&str>,
    current_hash: Option<&str>,
    before_hash: Option<&str>,
    after_hash: Option<&str>,
    lane: &str,
) {
    let detail = serde_json::json!({
        "gateway": "LifeModelWriteGateway",
        "proposalId": proposal.map(|proposal| proposal.id.as_str()),
        "runId": proposal.and_then(|proposal| proposal.run_id.as_deref()),
        "evidenceId": proposal.and_then(proposal_evidence_id_ref),
        "lane": lane,
        "patchId": patch_id,
        "snapshotVersion": snapshot_version,
        "reasonCode": reason_code,
        "conflictStatus": conflict_status,
        "baseHash": base_hash,
        "currentHash": current_hash,
        "beforeHash": before_hash,
        "afterHash": after_hash,
        "metadataSafe": true,
        "containsRawContent": false,
    });
    let detail_text = detail.to_string();
    let feedback = state.feedback_store.lock().await;
    if let Err(e) = feedback.log_event(event_name, None, Some(&detail_text)) {
        log::warn!("[LifeModelWriteGateway] audit log failed: {}", e);
    }
}

fn proposal_evidence_id_ref(proposal: &AgentProposal) -> Option<&str> {
    proposal
        .after
        .get("evidence_id")
        .or_else(|| proposal.after.get("evidenceId"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            proposal
                .source_detail
                .as_deref()
                .and_then(|detail| detail.strip_prefix("evidence:"))
        })
}

pub(crate) fn hash_life_model(model: &LifeModel) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(model)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

pub(crate) fn hash_canonical_lifemodel_semantics(
    model: &LifeModel,
) -> Result<String, serde_json::Error> {
    let mut canonical = model.clone();
    canonical.metadata = Default::default();
    canonical.goals.daily.clear();
    canonical.state.alerts.clear();
    let bytes = serde_json::to_vec(&canonical)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!(
        "sha256-lifemodel-semantic-v1:{:x}",
        hasher.finalize()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::types::RiskLevel;
    use openlife_core::life_model::patch::{PatchOp, PatchSource, PatchStatus};

    async fn isolated_lifemodel_commit_permit<'state>(
        state: &'state Arc<AppState>,
    ) -> CanonicalCommitPermit<'state> {
        let admission = state
            .persistence_coordinator
            .admit_canonical_writes(&[CanonicalWriteOwner::LifeModelFileStore])
            .expect("isolated evaluation must admit a bounded LifeModel write");
        state
            .persistence_coordinator
            .acquire_canonical_commit_permit(&admission)
            .await
            .expect("isolated evaluation admission must become a commit permit")
    }

    fn prepared_name_patch(
        baseline: &LifeModel,
        proposal_id: &str,
        next_name: &str,
    ) -> (LifeModel, LifeModelPatch) {
        let patch = LifeModelPatch::from_proposal(
            proposal_id,
            "/identity/name",
            "Identity > Name",
            PatchOp::Replace,
            Some(serde_json::json!(baseline.identity.name.clone())),
            serde_json::json!(next_name),
            "journal recovery test patch",
            0.9,
            RiskLevel::Medium,
            PatchSource::Manual,
        );
        let mut after = baseline.clone();
        after.apply_patch(&patch).unwrap();
        openlife_core::versioning::prepare_model_for_save(Some(baseline), &mut after);
        (after, patch)
    }

    async fn assert_compare_and_swap_committed_with_degraded_projection(
        state: &Arc<AppState>,
        expected_hash: &str,
        next: LifeModel,
        fault: DailySnapshotFaultInjection,
    ) {
        let expected_name = next.identity.name.clone();
        let result = write_life_model_compare_and_swap_with_snapshot_fault(
            state,
            expected_hash,
            next,
            true,
            fault,
        )
        .await;

        let (written, projection_degraded) = result
            .expect("projection failure must not be reported as canonical failure")
            .expect("the canonical compare-and-swap must commit");
        assert!(projection_degraded);
        assert_eq!(written.identity.name, expected_name);

        let canonical = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(canonical.identity.name, expected_name);
    }

    #[tokio::test]
    async fn lifemodel_compare_and_swap_allows_one_writer_for_one_baseline() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let baseline = LifeModel::default();
        {
            let manager = state.life_model_manager.lock().await;
            manager.save(&baseline).unwrap();
        }
        let expected_hash = hash_life_model(&baseline).unwrap();
        let mut first = baseline.clone();
        first.identity.name = "first-writer".into();
        let mut second = baseline;
        second.identity.name = "second-writer".into();

        let (first_result, second_result) = tokio::join!(
            write_life_model_compare_and_swap(&state, &expected_hash, first, false),
            write_life_model_compare_and_swap(&state, &expected_hash, second, false),
        );
        let committed = [first_result.unwrap(), second_result.unwrap()]
            .into_iter()
            .filter(Option::is_some)
            .count();
        assert_eq!(committed, 1);
    }

    #[tokio::test]
    async fn stale_normal_admission_cannot_enter_lifemodel_commit_after_degradation() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let baseline = LifeModel::default();
        state
            .life_model_manager
            .lock()
            .await
            .save(&baseline)
            .unwrap();
        let baseline_hash = hash_life_model(&baseline).unwrap();

        let stale_admission = state
            .persistence_coordinator
            .admit_canonical_writes(&[CanonicalWriteOwner::LifeModelFileStore])
            .expect("healthy runtime must issue a normal admission");
        state
            .persistence_coordinator
            .degrade_globally("test_lifemodel_commit_fence");

        let error = match state
            .persistence_coordinator
            .acquire_canonical_commit_permit(&stale_admission)
            .await
        {
            Ok(_) => panic!("an admission minted before degradation entered the owner lock"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            crate::persistence_coordinator::PersistenceGateError::AdmissionInvalidated { .. }
        ));
        assert_eq!(
            hash_life_model(&state.life_model_manager.lock().await.load().unwrap()).unwrap(),
            baseline_hash,
            "a rejected late admission must leave canonical LifeModel unchanged"
        );
    }

    #[test]
    fn maximum_patch_batch_uses_semantic_targets_not_one_target_per_patch() {
        let patches = (0..64)
            .map(|index| {
                LifeModelPatch::from_proposal(
                    "proposal-max-batch",
                    &format!("/goals/short_term/{index}/name"),
                    &format!("Goals > Short Term > {index} > Name"),
                    PatchOp::Replace,
                    Some(serde_json::json!("before")),
                    serde_json::json!("after"),
                    "batch target bound",
                    0.9,
                    RiskLevel::Medium,
                    PatchSource::Manual,
                )
            })
            .collect::<Vec<_>>();
        let plan = LifeModelProjectionPlan {
            create_daily_snapshot: true,
            create_patch_after_snapshot: true,
            patches,
        };

        assert_eq!(
            plan.targets(),
            vec![
                PROJECTION_DAILY_SNAPSHOT,
                PROJECTION_PATCH_AFTER_SNAPSHOT,
                PROJECTION_PATCH_STORE
            ]
        );
    }

    #[tokio::test]
    async fn exact_statestore_projector_can_update_only_the_daily_compatibility_view() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let baseline = LifeModel::default();
        state
            .life_model_manager
            .lock()
            .await
            .save(&baseline)
            .unwrap();
        let baseline_hash = hash_life_model(&baseline).unwrap();
        let mut projection = baseline.clone();
        projection
            .goals
            .daily
            .push(openlife_core::life_model::DailyGoal {
                name: "canonical projection".into(),
                operation_id: Some(uuid::Uuid::new_v4().to_string()),
                operation_digest: Some("state-asset-v1:test".into()),
                ..Default::default()
            });

        let written = persist_life_model_with_gateway_expected(
            &state,
            projection,
            false,
            LifeModelMaterializerCallerContext::new(
                STATE_STORE_DAILY_TASK_COMPATIBILITY_MATERIALIZER_ID,
                LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
                LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
            ),
            Some(&baseline_hash),
        )
        .await
        .expect("the exact StateStore projector must retain its compatibility lane");

        assert_eq!(written.goals.daily.len(), 1);
        assert_eq!(written.goals.daily[0].name, "canonical projection");
        let mut written_without_projection = written;
        written_without_projection.goals.daily.clear();
        assert_eq!(
            serde_json::to_value(written_without_projection).unwrap(),
            serde_json::to_value(baseline).unwrap(),
            "the compatibility lane must not bump metadata or mutate canonical fields"
        );
    }

    #[tokio::test]
    async fn source_compatibility_identity_cannot_be_forged_to_change_canonical_fields() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let baseline = LifeModel::default();
        state
            .life_model_manager
            .lock()
            .await
            .save(&baseline)
            .unwrap();
        let baseline_hash = hash_life_model(&baseline).unwrap();
        let mut replacement = baseline;
        replacement.identity.name = "forged compatibility writer".into();

        let error = persist_life_model_with_gateway_expected(
            &state,
            replacement,
            false,
            LifeModelMaterializerCallerContext::new(
                "forged_source_compatibility_writer",
                LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
                LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
            ),
            Some(&baseline_hash),
        )
        .await
        .expect_err("compatibility privilege must be bound to the exact projector");

        assert!(error.contains("StateStore daily-task projector identity"));
        assert!(state
            .life_model_manager
            .lock()
            .await
            .load()
            .unwrap()
            .identity
            .name
            .is_empty());
    }

    #[test]
    fn proposal_semantic_hash_ignores_metadata_and_derived_views_only() {
        let baseline = LifeModel::default_model();
        let expected = hash_canonical_lifemodel_semantics(&baseline).unwrap();
        let mut projected = baseline.clone();
        projected.metadata.version = "999.0.0".into();
        projected.metadata.updated_at = "2099-01-01T00:00:00Z".into();
        projected.goals.daily.push(Default::default());
        projected.state.alerts.push(Default::default());
        assert_eq!(
            hash_canonical_lifemodel_semantics(&projected).unwrap(),
            expected
        );

        projected.identity.name = "semantic change".into();
        assert_ne!(
            hash_canonical_lifemodel_semantics(&projected).unwrap(),
            expected
        );
    }

    #[tokio::test]
    async fn canonical_commit_survives_daily_snapshot_lookup_failure() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let baseline = LifeModel::default();
        {
            let manager = state.life_model_manager.lock().await;
            manager.save(&baseline).unwrap();
        }
        let expected_hash = hash_life_model(&baseline).unwrap();
        let mut next = baseline;
        next.identity.name = "lookup-failure-still-committed".into();

        assert_compare_and_swap_committed_with_degraded_projection(
            &state,
            &expected_hash,
            next,
            DailySnapshotFaultInjection::LookupFailure,
        )
        .await;
    }

    #[tokio::test]
    async fn canonical_commit_survives_real_daily_snapshot_write_failure() {
        let original_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let fault_root = tempfile::tempdir().unwrap();
        let versions_path = fault_root.path().join("versions-is-a-file");
        std::fs::write(&versions_path, b"not a directory").unwrap();

        let mut state_with_fault = (*original_state).clone();
        state_with_fault.version_manager = Arc::new(tokio::sync::Mutex::new(
            openlife_core::versioning::VersionManager::new(&versions_path),
        ));
        let state = Arc::new(state_with_fault);

        let baseline = LifeModel::default();
        {
            let manager = state.life_model_manager.lock().await;
            manager.save(&baseline).unwrap();
        }
        let expected_hash = hash_life_model(&baseline).unwrap();
        let mut next = baseline;
        next.identity.name = "snapshot-write-failure-still-committed".into();

        assert_compare_and_swap_committed_with_degraded_projection(
            &state,
            &expected_hash,
            next,
            DailySnapshotFaultInjection::None,
        )
        .await;
    }

    #[tokio::test]
    async fn crash_after_patch_stage_before_rename_recovers_as_not_committed() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let baseline = LifeModel::default();
        {
            state
                .life_model_manager
                .lock()
                .await
                .save(&baseline)
                .unwrap();
        }
        let expected_hash = hash_life_model(&baseline).unwrap();
        let (after, patch) = prepared_name_patch(
            &baseline,
            "proposal-before-rename",
            "RAW_NAME_MUST_NOT_ENTER_JOURNAL",
        );

        let result = write_prepared_life_model_compare_and_swap(
            &state,
            &expected_hash,
            after,
            LifeModelProjectionPlan {
                create_daily_snapshot: false,
                create_patch_after_snapshot: true,
                patches: vec![patch.clone()],
            },
            isolated_lifemodel_commit_permit(&state).await,
            FileWriteFaultInjection::StopAfterStageBeforeCanonical,
            DailySnapshotFaultInjection::None,
            LifeModelFilePostCommitPolicy::ReconcileProjections,
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            hash_life_model(&state.life_model_manager.lock().await.load().unwrap()).unwrap(),
            expected_hash
        );
        assert_eq!(
            state
                .patch_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_patch(&patch.id)
                .unwrap()
                .unwrap()
                .status,
            PatchStatus::Pending
        );

        let preflight_error = ensure_no_lifemodel_file_backlog_unlocked(&state)
            .await
            .expect_err("runtime preflight must not steal a prepared mutation");
        assert!(preflight_error.contains("blocked by degraded projection recovery"));
        assert_eq!(
            state
                .patch_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_patch(&patch.id)
                .unwrap()
                .unwrap()
                .status,
            PatchStatus::Pending
        );

        let report = reconcile_lifemodel_file_mutations_with_state(&state)
            .await
            .unwrap();
        assert!(report.not_committed >= 1);
        assert_eq!(report.degraded, 0);
        assert!(!report.backlog_may_remain);
        assert!(state
            .patch_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_patch(&patch.id)
            .unwrap()
            .is_none());

        let journal_path = state
            .life_model_manager
            .lock()
            .await
            .mutation_journal_path();
        for suffix in ["", "-wal", "-shm"] {
            let path = std::path::PathBuf::from(format!("{}{suffix}", journal_path.display()));
            if path.exists() {
                let bytes = std::fs::read(path).unwrap();
                assert!(
                    !String::from_utf8_lossy(&bytes).contains("RAW_NAME_MUST_NOT_ENTER_JOURNAL")
                );
            }
        }
    }

    #[tokio::test]
    async fn crash_after_rename_before_observe_replays_patch_and_snapshot_once() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let baseline = LifeModel::default();
        state
            .life_model_manager
            .lock()
            .await
            .save(&baseline)
            .unwrap();
        let expected_hash = hash_life_model(&baseline).unwrap();
        let (after, patch) =
            prepared_name_patch(&baseline, "proposal-after-rename", "committed-after-rename");
        let expected_after_hash = hash_life_model(&after).unwrap();

        let result = write_prepared_life_model_compare_and_swap(
            &state,
            &expected_hash,
            after,
            LifeModelProjectionPlan {
                create_daily_snapshot: false,
                create_patch_after_snapshot: true,
                patches: vec![patch.clone()],
            },
            isolated_lifemodel_commit_permit(&state).await,
            FileWriteFaultInjection::StopAfterCanonicalBeforeObserve,
            DailySnapshotFaultInjection::None,
            LifeModelFilePostCommitPolicy::ReconcileProjections,
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            hash_life_model(&state.life_model_manager.lock().await.load().unwrap()).unwrap(),
            expected_after_hash
        );
        assert_eq!(
            state
                .patch_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_patch(&patch.id)
                .unwrap()
                .unwrap()
                .status,
            PatchStatus::Pending
        );

        let first = reconcile_lifemodel_file_mutations_with_state(&state)
            .await
            .unwrap();
        assert_eq!(first.degraded, 0);
        assert!(!first.backlog_may_remain);
        assert_eq!(
            state
                .patch_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_patch(&patch.id)
                .unwrap()
                .unwrap()
                .status,
            PatchStatus::Applied
        );
        let snapshot_count = state
            .version_manager
            .lock()
            .await
            .get_patch_snapshots("proposal-after-rename")
            .unwrap()
            .len();
        assert_eq!(snapshot_count, 1);

        let second = reconcile_lifemodel_file_mutations_with_state(&state)
            .await
            .unwrap();
        assert_eq!(second.applied, 0);
        assert_eq!(second.degraded, 0);
        assert!(!second.backlog_may_remain);
        assert_eq!(
            state
                .version_manager
                .lock()
                .await
                .get_patch_snapshots("proposal-after-rename")
                .unwrap()
                .len(),
            snapshot_count
        );
        assert_eq!(
            state
                .patch_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .patch_count()
                .unwrap(),
            1
        );
    }
}
