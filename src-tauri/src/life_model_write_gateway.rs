use crate::errors::AppError;
use crate::life_model_materializer_guard::{
    ensure_lifemodel_materializer_caller_restriction, LifeModelMaterializerCallerContext,
    LifeModelMaterializerCallerKind, LifeModelMaterializerCallerPurpose,
};
use crate::AppState;
use openlife_core::agent::AgentProposal;
use openlife_core::life_model::patch::{
    ConflictResolution, ConflictType, LifeModelPatch, PatchApplyResult, PatchConflict,
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
    require_persistence_write(state)?;
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
    let request = gateway_request_for_caller(&caller_context, Some(&previous_model), &life_model)
        .map_err(|e| e.to_string())?;
    let decision = LifeModelWriteGateway::decide(request);
    if !decision.allowed {
        return Err(format!(
            "LifeModelWriteGateway blocked persist_life_model: {}",
            decision.reason_code
        ));
    }

    let written =
        write_life_model(state, &previous_model, life_model, create_daily_snapshot).await?;
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

pub(crate) async fn materialize_accepted_lifemodel_proposal_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    patch: LifeModelPatch,
) -> Result<PatchApplyResult, String> {
    require_persistence_write(state)?;
    let _coordinator = state.life_model_write_coordinator.lock().await;
    ensure_no_lifemodel_file_backlog_unlocked(state).await?;
    let before_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };
    let current_model_hash = hash_life_model(&before_model).map_err(|e| e.to_string())?;
    let proposal_base_hash = proposal.base_hash.clone();
    let stale_check = LifeModelWriteGatewayRequest::accepted_proposal(
        proposal.id.clone(),
        proposal.run_id.clone(),
        proposal_evidence_id(proposal),
        proposal_base_hash.clone(),
        Some(current_model_hash.clone()),
        current_model_hash.clone(),
        current_model_hash.clone(),
    );
    let stale_decision = LifeModelWriteGateway::decide(stale_check);
    if stale_decision.status
        == openlife_core::life_model_write_gateway::LifeModelWriteGatewayStatus::StaleConflict
        || !stale_decision.allowed
    {
        record_patch_conflict(state, &patch, proposal).await;
        record_lifemodel_gateway_audit(
            state,
            "lifemodel_gateway_stale_conflict",
            Some(proposal),
            Some(&patch.id),
            None,
            &stale_decision.reason_code,
            stale_decision.conflict_status.as_deref(),
            stale_decision.base_hash.as_deref(),
            stale_decision.current_hash.as_deref(),
            Some(&current_model_hash),
            stale_decision.after_hash.as_deref(),
            &stale_decision.lane,
        )
        .await;
        return Ok(PatchApplyResult {
            patch_id: patch.id,
            success: false,
            path: proposal.affected_path.clone(),
            operation: "lifemodel_gateway_stale_conflict".into(),
            error: Some(stale_decision.reason_code),
        });
    }
    let before_snapshot = {
        let vm = state.version_manager.lock().await;
        vm.ensure_projection_snapshot(
            &before_model,
            &format!("precommit:{}:{}", proposal.id, current_model_hash),
            &format!("patch:{}:before", proposal.id),
            &format!("Snapshot before patch {}", proposal.id),
        )
        .map_err(|e| e.to_string())?
    };

    let mut after_model = before_model.clone();
    let apply_result = match after_model.apply_patch(&patch) {
        Ok(result) => result,
        Err(err) => {
            record_patch_conflict(state, &patch, proposal).await;
            record_lifemodel_gateway_audit(
                state,
                "lifemodel_gateway_conflict",
                Some(proposal),
                Some(&patch.id),
                Some(&before_snapshot.version),
                "accepted_proposal_patch_before_mismatch",
                Some("patch_before_mismatch"),
                proposal_base_hash.as_deref(),
                Some(&current_model_hash),
                Some(&current_model_hash),
                None,
                "canonical_lifemodel_truth",
            )
            .await;
            return Ok(PatchApplyResult {
                patch_id: patch.id,
                success: false,
                path: proposal.affected_path.clone(),
                operation: "lifemodel_patch_conflict".into(),
                error: Some(err.to_string()),
            });
        }
    };

    if !apply_result.success {
        return Ok(apply_result);
    }

    openlife_core::versioning::prepare_model_for_save(Some(&before_model), &mut after_model);
    let after_model_hash = hash_life_model(&after_model).map_err(|e| e.to_string())?;
    let request = LifeModelWriteGatewayRequest::accepted_proposal(
        proposal.id.clone(),
        proposal.run_id.clone(),
        proposal_evidence_id(proposal),
        proposal_base_hash,
        Some(current_model_hash.clone()),
        current_model_hash.clone(),
        after_model_hash.clone(),
    );
    let decision = LifeModelWriteGateway::decide(request);
    if !decision.allowed {
        record_lifemodel_gateway_audit(
            state,
            "lifemodel_gateway_blocked",
            Some(proposal),
            Some(&patch.id),
            Some(&before_snapshot.version),
            &decision.reason_code,
            decision.conflict_status.as_deref(),
            decision.base_hash.as_deref(),
            decision.current_hash.as_deref(),
            decision.before_hash.as_deref(),
            decision.after_hash.as_deref(),
            &decision.lane,
        )
        .await;
        return Ok(PatchApplyResult {
            patch_id: patch.id,
            success: false,
            path: proposal.affected_path.clone(),
            operation: "lifemodel_gateway_blocked".into(),
            error: Some(decision.reason_code),
        });
    }

    let projection_plan = LifeModelProjectionPlan {
        create_daily_snapshot: true,
        create_patch_after_snapshot: true,
        patches: vec![patch.clone()],
    };
    let Some(commit) = write_prepared_life_model_compare_and_swap(
        state,
        &current_model_hash,
        after_model,
        projection_plan,
        FileWriteFaultInjection::None,
        DailySnapshotFaultInjection::None,
    )
    .await?
    else {
        record_patch_conflict(state, &patch, proposal).await;
        record_lifemodel_gateway_audit(
            state,
            "lifemodel_gateway_commit_conflict",
            Some(proposal),
            Some(&patch.id),
            Some(&before_snapshot.version),
            "lifemodel_compare_and_swap_conflict",
            Some("concurrent_write_conflict"),
            decision.base_hash.as_deref(),
            decision.current_hash.as_deref(),
            decision.before_hash.as_deref(),
            decision.after_hash.as_deref(),
            &decision.lane,
        )
        .await;
        return Ok(PatchApplyResult {
            patch_id: patch.id,
            success: false,
            path: proposal.affected_path.clone(),
            operation: "lifemodel_compare_and_swap_conflict".into(),
            error: Some("lifemodel_compare_and_swap_conflict".into()),
        });
    };

    let projection_degraded = commit.projection_degraded;

    record_lifemodel_gateway_audit(
        state,
        "lifemodel_gateway_materialized",
        Some(proposal),
        Some(&patch.id),
        Some(&before_snapshot.version),
        &decision.reason_code,
        None,
        decision.base_hash.as_deref(),
        decision.current_hash.as_deref(),
        decision.before_hash.as_deref(),
        decision.after_hash.as_deref(),
        &decision.lane,
    )
    .await;

    if projection_degraded {
        Ok(PatchApplyResult {
            patch_id: patch.id,
            success: true,
            path: proposal.affected_path.clone(),
            operation: "lifemodel_materialized_projection_degraded".into(),
            error: Some("projection_degraded_after_canonical_commit".into()),
        })
    } else {
        Ok(apply_result)
    }
}

pub(crate) async fn materialize_accepted_lifemodel_patch_batch_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    patches: Vec<LifeModelPatch>,
) -> Result<PatchApplyResult, String> {
    require_persistence_write(state)?;
    let _coordinator = state.life_model_write_coordinator.lock().await;
    ensure_no_lifemodel_file_backlog_unlocked(state).await?;
    let batch_patch_id = format!("batch:{}", proposal.id);
    if patches.is_empty() {
        return Ok(PatchApplyResult {
            patch_id: batch_patch_id,
            success: false,
            path: proposal.affected_path.clone(),
            operation: "lifemodel_patch_batch_validation_failed".into(),
            error: Some("lifemodel_patch_batch_empty".into()),
        });
    }
    if !openlife_core::life_model::patch::detect_conflicts(&patches).is_empty() {
        return Ok(PatchApplyResult {
            patch_id: batch_patch_id,
            success: false,
            path: proposal.affected_path.clone(),
            operation: "lifemodel_patch_batch_validation_failed".into(),
            error: Some("lifemodel_patch_batch_contains_overlapping_paths".into()),
        });
    }

    let before_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|error| error.to_string())?
    };
    let current_model_hash = hash_life_model(&before_model).map_err(|error| error.to_string())?;
    let proposal_base_hash = proposal.base_hash.clone();
    let stale_decision =
        LifeModelWriteGateway::decide(LifeModelWriteGatewayRequest::accepted_proposal(
            proposal.id.clone(),
            proposal.run_id.clone(),
            proposal_evidence_id(proposal),
            proposal_base_hash.clone(),
            Some(current_model_hash.clone()),
            current_model_hash.clone(),
            current_model_hash.clone(),
        ));
    if stale_decision.status
        == openlife_core::life_model_write_gateway::LifeModelWriteGatewayStatus::StaleConflict
        || !stale_decision.allowed
    {
        record_patch_conflict(state, &patches[0], proposal).await;
        record_lifemodel_gateway_audit(
            state,
            "lifemodel_gateway_batch_stale_conflict",
            Some(proposal),
            Some(&batch_patch_id),
            None,
            &stale_decision.reason_code,
            stale_decision.conflict_status.as_deref(),
            stale_decision.base_hash.as_deref(),
            stale_decision.current_hash.as_deref(),
            Some(&current_model_hash),
            stale_decision.after_hash.as_deref(),
            &stale_decision.lane,
        )
        .await;
        return Ok(PatchApplyResult {
            patch_id: batch_patch_id,
            success: false,
            path: proposal.affected_path.clone(),
            operation: "lifemodel_gateway_batch_stale_conflict".into(),
            error: Some(stale_decision.reason_code),
        });
    }

    let before_snapshot = {
        let versions = state.version_manager.lock().await;
        versions
            .ensure_projection_snapshot(
                &before_model,
                &format!("precommit:{}:{}", proposal.id, current_model_hash),
                &format!("patch:{}:before", proposal.id),
                &format!("Snapshot before patch {}", proposal.id),
            )
            .map_err(|error| error.to_string())?
    };
    let mut after_model = before_model.clone();
    for patch in &patches {
        if let Err(error) = after_model.apply_patch(patch) {
            record_patch_conflict(state, patch, proposal).await;
            record_lifemodel_gateway_audit(
                state,
                "lifemodel_gateway_batch_conflict",
                Some(proposal),
                Some(&batch_patch_id),
                Some(&before_snapshot.version),
                "accepted_proposal_batch_patch_before_mismatch",
                Some("patch_before_mismatch"),
                proposal_base_hash.as_deref(),
                Some(&current_model_hash),
                Some(&current_model_hash),
                None,
                "canonical_lifemodel_truth",
            )
            .await;
            return Ok(PatchApplyResult {
                patch_id: batch_patch_id,
                success: false,
                path: proposal.affected_path.clone(),
                operation: "lifemodel_patch_batch_conflict".into(),
                error: Some(error.to_string()),
            });
        }
    }

    openlife_core::versioning::prepare_model_for_save(Some(&before_model), &mut after_model);
    let after_model_hash = hash_life_model(&after_model).map_err(|error| error.to_string())?;
    let decision = LifeModelWriteGateway::decide(LifeModelWriteGatewayRequest::accepted_proposal(
        proposal.id.clone(),
        proposal.run_id.clone(),
        proposal_evidence_id(proposal),
        proposal_base_hash,
        Some(current_model_hash.clone()),
        current_model_hash.clone(),
        after_model_hash,
    ));
    if !decision.allowed {
        return Ok(PatchApplyResult {
            patch_id: batch_patch_id,
            success: false,
            path: proposal.affected_path.clone(),
            operation: "lifemodel_gateway_batch_blocked".into(),
            error: Some(decision.reason_code),
        });
    }

    let projection_plan = LifeModelProjectionPlan {
        create_daily_snapshot: true,
        create_patch_after_snapshot: true,
        patches: patches.clone(),
    };
    let Some(commit) = write_prepared_life_model_compare_and_swap(
        state,
        &current_model_hash,
        after_model,
        projection_plan,
        FileWriteFaultInjection::None,
        DailySnapshotFaultInjection::None,
    )
    .await?
    else {
        record_patch_conflict(state, &patches[0], proposal).await;
        return Ok(PatchApplyResult {
            patch_id: batch_patch_id,
            success: false,
            path: proposal.affected_path.clone(),
            operation: "lifemodel_compare_and_swap_conflict".into(),
            error: Some("lifemodel_compare_and_swap_conflict".into()),
        });
    };

    let projection_degraded = commit.projection_degraded;
    record_lifemodel_gateway_audit(
        state,
        "lifemodel_gateway_batch_materialized",
        Some(proposal),
        Some(&batch_patch_id),
        Some(&before_snapshot.version),
        &decision.reason_code,
        None,
        decision.base_hash.as_deref(),
        decision.current_hash.as_deref(),
        decision.before_hash.as_deref(),
        decision.after_hash.as_deref(),
        &decision.lane,
    )
    .await;

    Ok(PatchApplyResult {
        patch_id: batch_patch_id,
        success: true,
        path: proposal.affected_path.clone(),
        operation: if projection_degraded {
            "lifemodel_patch_batch_projection_degraded".into()
        } else {
            "lifemodel_patch_batch".into()
        },
        error: projection_degraded
            .then(|| "projection_degraded_after_canonical_commit".to_string()),
    })
}

pub(crate) async fn restore_life_model_with_gateway(
    state: &Arc<AppState>,
    restored_model: &LifeModel,
    caller_context: LifeModelMaterializerCallerContext,
    expected_before_hash: Option<&str>,
) -> Result<(), AppError> {
    require_persistence_write(state)
        .map_err(|error| AppError::db_with_hint(error, "read_only_degraded"))?;
    ensure_lifemodel_materializer_caller_restriction(&caller_context, "LifeModelManager::save")
        .map_err(AppError::from)?;
    let _coordinator = state.life_model_write_coordinator.lock().await;
    ensure_no_lifemodel_file_backlog_unlocked(state)
        .await
        .map_err(AppError::from)?;
    let current_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let current_hash = hash_life_model(&current_model).map_err(AppError::from)?;
    if expected_before_hash.is_some_and(|expected| expected != current_hash) {
        return Err(AppError::internal(
            "LifeModel changed after required pre-restore snapshot",
        ));
    }
    let request = LifeModelWriteGatewayRequest {
        intent: LifeModelWriteIntentKind::RestoreImportOverride,
        proposal_id: None,
        run_id: None,
        evidence_id: None,
        base_hash: None,
        current_hash: None,
        before_hash: Some(current_hash),
        after_hash: hash_life_model(restored_model).ok(),
        explicit_manual_override: true,
        risk_acknowledged: true,
    };
    let decision = LifeModelWriteGateway::decide(request);
    if !decision.allowed {
        return Err(AppError::permission(format!(
            "LifeModelWriteGateway blocked restore: {}",
            decision.reason_code
        )));
    }
    write_life_model_without_prepare(state, &current_model, restored_model).await?;
    record_lifemodel_gateway_audit(
        state,
        "lifemodel_gateway_restore",
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
    Ok(())
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
    proposal.base_hash = Some(hash_life_model(&current_model).map_err(|e| e.to_string())?);
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
) -> Result<LifeModel, String> {
    let expected_hash = hash_life_model(previous_model).map_err(|error| error.to_string())?;
    openlife_core::versioning::prepare_model_for_save(Some(previous_model), &mut life_model);
    let plan = LifeModelProjectionPlan {
        create_daily_snapshot,
        ..LifeModelProjectionPlan::default()
    };
    write_prepared_life_model_compare_and_swap(
        state,
        &expected_hash,
        life_model,
        plan,
        FileWriteFaultInjection::None,
        DailySnapshotFaultInjection::None,
    )
    .await?
    .map(|commit| commit.life_model)
    .ok_or_else(|| "LifeModel compare-and-swap conflict".to_string())
}

#[cfg(test)]
async fn write_life_model_compare_and_swap(
    state: &Arc<AppState>,
    expected_hash: &str,
    life_model: LifeModel,
    create_daily_snapshot: bool,
) -> Result<Option<(LifeModel, bool)>, String> {
    let _coordinator = state.life_model_write_coordinator.lock().await;
    ensure_no_lifemodel_file_backlog_unlocked(state).await?;
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
        FileWriteFaultInjection::None,
        snapshot_fault,
    )
    .await?
    .map(|commit| (commit.life_model, commit.projection_degraded)))
}

async fn write_prepared_life_model_compare_and_swap(
    state: &Arc<AppState>,
    expected_hash: &str,
    life_model: LifeModel,
    mut projection_plan: LifeModelProjectionPlan,
    _write_fault: FileWriteFaultInjection,
    snapshot_fault: DailySnapshotFaultInjection,
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
    let hs_compatibility_yaml = {
        let registry_path = state
            .life_model_manager
            .lock()
            .await
            .hs_asset_authority_registry_path();
        let registry = openlife_core::agent::HSAssetAuthorityRegistry::new(registry_path)
            .map_err(|error| error.to_string())?;
        let authority = registry
            .authority(openlife_core::agent::HSAssetCategory::CollaborationGuidance)
            .map_err(|error| error.to_string())?;
        if authority.owner == openlife_core::agent::HSAssetOwner::AcceptedHsStore {
            registry
                .authorize_write(openlife_core::agent::HSAssetWriteRequest {
                    category: openlife_core::agent::HSAssetCategory::CollaborationGuidance,
                    source_owner: openlife_core::agent::HSAssetOwner::AcceptedHsStore,
                    target_owner: openlife_core::agent::HSAssetOwner::LifeModelYaml,
                    kind: openlife_core::agent::HSAssetWriteKind::DerivedCompatibilityProjection,
                })
                .map_err(|error| error.to_string())?;
            let heuristic_store = state.heuristic_store.lock().await;
            let projection = openlife_core::agent::build_collaboration_guidance_projection(
                &life_model,
                &heuristic_store,
            )
            .map_err(|error| error.to_string())?;
            if projection.canonical_digest != projection.compatibility_digest
                || projection.canonical_digest != projection.repeated_materialization_digest
            {
                return Err(
                    "derived collaboration guidance compatibility projection failed digest parity"
                        .into(),
                );
            }
            Some(projection.yaml)
        } else {
            None
        }
    };
    let journal_path = {
        state
            .life_model_manager
            .lock()
            .await
            .mutation_journal_path()
    };
    let journal = FileMutationJournal::new(journal_path).map_err(|error| error.to_string())?;
    let targets = projection_plan.targets();
    let receipt = journal
        .prepare(
            LIFEMODEL_FILE_AGGREGATE_KIND,
            LIFEMODEL_FILE_AGGREGATE_ID,
            "updated",
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
        let save_result = match hs_compatibility_yaml.as_deref() {
            Some(yaml) => manager
                .save_hs_compatibility_view(yaml)
                .map_err(|error| error.to_string()),
            None => manager.save(&life_model).map_err(|error| error.to_string()),
        };
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
) -> Result<(), AppError> {
    let expected_hash = hash_life_model(previous_model).map_err(AppError::from)?;
    write_prepared_life_model_compare_and_swap(
        state,
        &expected_hash,
        life_model.clone(),
        LifeModelProjectionPlan::default(),
        FileWriteFaultInjection::None,
        DailySnapshotFaultInjection::None,
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
            LifeModelMaterializerCallerKind::GovernedManualOverride,
            LifeModelMaterializerCallerPurpose::GovernedManualOverride,
        ) => LifeModelWriteGatewayRequest {
            intent: LifeModelWriteIntentKind::ManualOverride,
            proposal_id: None,
            run_id: None,
            evidence_id: None,
            base_hash: None,
            current_hash: None,
            before_hash,
            after_hash,
            explicit_manual_override: true,
            risk_acknowledged: true,
        },
        (
            LifeModelMaterializerCallerKind::GovernedRestoreImportOperation,
            LifeModelMaterializerCallerPurpose::GovernedRestoreImportOperation,
        ) => LifeModelWriteGatewayRequest {
            intent: LifeModelWriteIntentKind::RestoreImportOverride,
            proposal_id: None,
            run_id: None,
            evidence_id: None,
            base_hash: None,
            current_hash: None,
            before_hash,
            after_hash,
            explicit_manual_override: true,
            risk_acknowledged: true,
        },
        (
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
        ) => LifeModelWriteGatewayRequest {
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
        },
        (
            LifeModelMaterializerCallerKind::AcceptedProposalApply,
            LifeModelMaterializerCallerPurpose::AcceptedProposalApplySourceSpecificPatchMappingComplete,
        ) => {
            return Err(AppError::permission(
                "accepted proposal LifeModel writes must use materialize_accepted_lifemodel_proposal_with_state",
            ))
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

async fn record_patch_conflict(
    state: &Arc<AppState>,
    patch: &LifeModelPatch,
    proposal: &AgentProposal,
) {
    let Some(ref patch_store_arc) = state.patch_store else {
        return;
    };
    let patch_store = patch_store_arc.lock().await;
    let conflict = PatchConflict {
        patch_id_1: patch.id.clone(),
        patch_id_2: format!("current_lifemodel:{}", proposal.id),
        conflict_type: ConflictType::SamePath,
        resolution: Some(ConflictResolution::Manual),
        resolved_at: None,
    };
    if let Err(e) = patch_store.record_conflict(&conflict) {
        log::warn!(
            "[LifeModelWriteGateway] failed to record stale proposal conflict {}: {}",
            proposal.id,
            e
        );
    }
}

#[allow(clippy::too_many_arguments)]
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

fn proposal_evidence_id(proposal: &AgentProposal) -> Option<String> {
    proposal_evidence_id_ref(proposal).map(ToOwned::to_owned)
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

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::types::RiskLevel;
    use openlife_core::life_model::patch::{PatchOp, PatchSource, PatchStatus};

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
    async fn required_pre_change_snapshot_hash_prevents_stale_overwrite() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let baseline = LifeModel::default();
        state
            .life_model_manager
            .lock()
            .await
            .save(&baseline)
            .unwrap();
        let snapshotted_hash = hash_life_model(&baseline).unwrap();
        let mut concurrent = baseline.clone();
        concurrent.identity.name = "concurrent-writer".into();
        state
            .life_model_manager
            .lock()
            .await
            .save(&concurrent)
            .unwrap();
        let mut stale_replacement = baseline;
        stale_replacement.identity.name = "stale-overwrite".into();

        let error = persist_life_model_with_gateway_expected(
            &state,
            stale_replacement,
            false,
            LifeModelMaterializerCallerContext::new(
                "stale-pre-change-test",
                LifeModelMaterializerCallerKind::GovernedManualOverride,
                LifeModelMaterializerCallerPurpose::GovernedManualOverride,
            ),
            Some(&snapshotted_hash),
        )
        .await
        .expect_err("stale pre-change snapshot must block overwrite");
        assert!(error.contains("changed after required pre-change snapshot"));
        assert_eq!(
            state
                .life_model_manager
                .lock()
                .await
                .load()
                .unwrap()
                .identity
                .name,
            "concurrent-writer"
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
            FileWriteFaultInjection::StopAfterStageBeforeCanonical,
            DailySnapshotFaultInjection::None,
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
            FileWriteFaultInjection::StopAfterCanonicalBeforeObserve,
            DailySnapshotFaultInjection::None,
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
