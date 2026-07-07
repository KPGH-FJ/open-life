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
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(crate) async fn persist_life_model_with_gateway(
    state: &Arc<AppState>,
    life_model: LifeModel,
    create_daily_snapshot: bool,
    caller_context: LifeModelMaterializerCallerContext,
) -> Result<LifeModel, String> {
    ensure_lifemodel_materializer_caller_restriction(&caller_context, "persist_life_model")?;
    let previous_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().ok()
    };
    let request = gateway_request_for_caller(&caller_context, previous_model.as_ref(), &life_model)
        .map_err(|e| e.to_string())?;
    let decision = LifeModelWriteGateway::decide(request);
    if !decision.allowed {
        return Err(format!(
            "LifeModelWriteGateway blocked persist_life_model: {}",
            decision.reason_code
        ));
    }

    let written = write_life_model(
        state,
        previous_model.as_ref(),
        life_model,
        create_daily_snapshot,
    )
    .await?;
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
        vm.snapshot_for_patch(&before_model, &proposal.id, "before")
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

    write_life_model(state, Some(&before_model), after_model.clone(), true).await?;

    {
        let vm = state.version_manager.lock().await;
        vm.snapshot_for_patch(&after_model, &proposal.id, "after")
            .map_err(|e| e.to_string())?;
    }

    if let Some(ref patch_store_arc) = state.patch_store {
        let patch_store = patch_store_arc.lock().await;
        let mut patch_to_save = patch.clone();
        patch_to_save.mark_applied();
        patch_store
            .create_patch(&patch_to_save)
            .map_err(|e| e.to_string())?;
    }

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

    Ok(apply_result)
}

pub(crate) async fn restore_life_model_with_gateway(
    state: &Arc<AppState>,
    restored_model: &LifeModel,
    caller_context: LifeModelMaterializerCallerContext,
) -> Result<(), AppError> {
    ensure_lifemodel_materializer_caller_restriction(&caller_context, "LifeModelManager::save")
        .map_err(AppError::from)?;
    let current_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().ok()
    };
    let request = LifeModelWriteGatewayRequest {
        intent: LifeModelWriteIntentKind::RestoreImportOverride,
        proposal_id: None,
        run_id: None,
        evidence_id: None,
        base_hash: None,
        current_hash: None,
        before_hash: current_model
            .as_ref()
            .and_then(|model| hash_life_model(model).ok()),
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
    write_life_model_without_prepare(state, restored_model).await?;
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
    previous_model: Option<&LifeModel>,
    mut life_model: LifeModel,
    create_daily_snapshot: bool,
) -> Result<LifeModel, String> {
    openlife_core::versioning::prepare_model_for_save(previous_model, &mut life_model);
    {
        let manager = state.life_model_manager.lock().await;
        manager.save(&life_model).map_err(|e| e.to_string())?;
    }
    if create_daily_snapshot {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let should_snapshot = {
            let vm = state.version_manager.lock().await;
            !vm.has_snapshot_tag_on_date("auto:daily-save", &today)
                .map_err(|e| e.to_string())?
        };
        if should_snapshot {
            let vm = state.version_manager.lock().await;
            vm.snapshot(&life_model, "auto:daily-save", "Daily auto snapshot")
                .map_err(|e| e.to_string())?;
            let mut last_snapshot_date = state.last_snapshot_date.lock().await;
            *last_snapshot_date = Some(today);
        }
    }
    Ok(life_model)
}

async fn write_life_model_without_prepare(
    state: &Arc<AppState>,
    life_model: &LifeModel,
) -> Result<(), AppError> {
    let manager = state.life_model_manager.lock().await;
    manager.save(life_model).map_err(AppError::from)
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
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization
            | LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
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

fn hash_life_model(model: &LifeModel) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(model)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}
