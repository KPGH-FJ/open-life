use crate::{
    artifact_materializer::{
        commit_staged_artifact, confirmed_artifact_receipt, inspect_artifact_filesystem,
        prepare_artifact_materialization, stage_artifact_bytes, ArtifactFilesystemFailure,
        ArtifactFilesystemObservation, ArtifactMaterializationReceipt,
    },
    danger_action_confirmation::{
        require_native_danger_action_confirmation, NativeDangerActionRequest,
    },
    life_model_write_gateway, memory_gateway,
    storage::app_data_dir,
    AppState,
};
use openlife_core::agent::{
    AgentProposal, ArtifactEffectState, MaturationProposalOutcome, MemoryLifecycleRecord,
    MemoryLifecycleScope, MemoryLifecycleStatus, MemoryRollbackReport, ProposalSource,
    ProposalStatus, ProposalType, RiskLevel,
};
use openlife_core::life_model::patch::PatchSource;
use openlife_core::life_model::LifeModel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

#[cfg(test)]
use crate::artifact_materializer::PreparedArtifactMaterialization;

/// Maximum content size for ExternalWriteAction (100 KB)
const EXTERNAL_WRITE_MAX_SIZE: usize = 100 * 1024;
pub(crate) const COMMUNICATION_STYLE_CANONICAL_PATH: &str = "preferences.communication_style";

fn require_persistence_write(state: &Arc<AppState>) -> Result<(), String> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| error.to_string())
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
    #[serde(alias = "proposal_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_owner_transition: Option<Value>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub main_chat_task_sync: Vec<Value>,
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
    #[serde(alias = "blocked_action")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_action: Option<Value>,
    #[serde(alias = "can_continue")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_continue: Option<bool>,
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
        {
            return Err(
                "accept Proposal confirmed response is missing confirmed effect/projection truth"
                    .into(),
            );
        }
        if response.patch_result.is_none() && response.proposal_id.is_none() {
            return Err(
                "accept Proposal confirmed response is missing both patch and terminal-owner identity"
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
        || response.terminal_owner_transition.is_some()
        || response.memory_gateway.is_some()
        || response.memory_lifecycle.is_some()
        || response.memory_persistence.is_some()
        || response.artifact_materialization.is_some()
        || response.blocked_action.is_some()
        || response.can_continue.is_some()
        || !response.main_chat_task_sync.is_empty()
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

pub(crate) fn is_communication_style_lifemodel_path(path: &str) -> bool {
    canonical_lifemodel_path(path) == COMMUNICATION_STYLE_CANONICAL_PATH
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

fn memory_lifecycle_store_missing() -> String {
    "Memory lifecycle store is unavailable. Accepted memory rollback is blocked.".to_string()
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

fn is_builder_lifemodel_patch_batch(proposal: &AgentProposal) -> bool {
    proposal.proposal_type == ProposalType::LifeModelUpdate
        && proposal.source == ProposalSource::BuilderReview
        && proposal.affected_path == openlife_core::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH
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
    pub proposal_projections_repaired: usize,
    pub agent_runs_reconciled: usize,
    pub agent_run_candidates_examined: usize,
    pub agent_run_cursor_advanced: bool,
    pub agent_run_cursor_wrapped: bool,
    pub artifact_backlog_may_remain: bool,
    pub projection_backlog_may_remain: bool,
    pub agent_run_backlog_may_remain: bool,
}

async fn reconcile_agent_runs_for_proposal(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<usize, String> {
    reconcile_agent_runs_for_proposal_with_admission(
        state,
        &proposal.id,
        ProposalReconciliationAdmission::ProductEffects,
    )
    .await
}

async fn reconcile_agent_runs_for_proposal_with_admission(
    state: &Arc<AppState>,
    proposal_id: &str,
    admission: ProposalReconciliationAdmission,
) -> Result<usize, String> {
    let Some(store) = state.agent_run_store.as_ref() else {
        return Err("AgentRun store is unavailable for Proposal reconciliation.".into());
    };
    let store = store.lock().await.clone();
    let linked_runs = crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store
            .list_runs_linked_to_proposal(proposal_id)
            .map_err(|error| error.to_string()),
    )?;
    if linked_runs.is_empty() {
        return Ok(0);
    }

    let mut reconciled = 0_usize;
    for linked_run in linked_runs {
        match admission {
            ProposalReconciliationAdmission::ProductEffects => {
                crate::terminal_owner_write_gateway::update_agent_run_after_review_reconciliation(
                    state,
                    proposal_id,
                    &linked_run.id,
                )
                .await?;
            }
            ProposalReconciliationAdmission::StartupInternal => {
                crate::terminal_owner_write_gateway::update_agent_run_after_startup_review_reconciliation(
                    state,
                    proposal_id,
                    &linked_run.id,
                )
                .await?;
            }
        }
        reconciled += 1;
    }
    Ok(reconciled)
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

async fn reconcile_artifact_effects_with_state(
    state: &Arc<AppState>,
    limit: i64,
) -> Result<(usize, bool), String> {
    let bounded_limit = limit.clamp(1, 200);
    let records = {
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
    let backlog_may_remain = records.len() == bounded_limit as usize;
    let safe_paths = { state.config.lock().await.system.safe_paths.clone() };
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
        let Some(path) = proposal.after.get("path").and_then(Value::as_str) else {
            persist_artifact_unknown(
                state,
                &record.proposal_id,
                &record.dispatch_claim_id,
                "artifact_recovery_path_missing",
            )
            .await?;
            reconciled += 1;
            continue;
        };
        let content = proposal
            .after
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("");
        let prepared = match prepare_artifact_materialization(
            &record.proposal_id,
            &record.dispatch_claim_id,
            path,
            content,
            &safe_paths,
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
                let store = state
                    .proposal_store
                    .as_ref()
                    .ok_or_else(proposal_store_missing)?
                    .lock()
                    .await;
                if !store
                    .finish_artifact_confirmed(
                        &record.proposal_id,
                        &record.dispatch_claim_id,
                        &observed_content_digest,
                    )
                    .map_err(|error| runtime_proposal_store_error(state, error))?
                {
                    return Err("artifact recovery confirmation CAS lost".into());
                }
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
                        let store = state
                            .proposal_store
                            .as_ref()
                            .ok_or_else(proposal_store_missing)?
                            .lock()
                            .await;
                        if !store
                            .finish_artifact_confirmed(
                                &record.proposal_id,
                                &record.dispatch_claim_id,
                                &observed_content_digest,
                            )
                            .map_err(|error| runtime_proposal_store_error(state, error))?
                        {
                            return Err("artifact staged recovery confirmation CAS lost".into());
                        }
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
    let (orphaned_claims_released, orphaned_claim_backlog_may_remain) =
        if matches!(admission, ProposalReconciliationAdmission::StartupInternal) {
            release_startup_artifact_claims_proven_before_effect(state, bounded_limit).await?
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
        artifact_backlog_may_remain,
        projection_backlog_may_remain: confirmed_projection_pending.len() == bounded_limit as usize,
        ..ProposalReconciliationReport::default()
    };
    for (proposal, claim_id) in confirmed_projection_pending {
        let accepted =
            project_confirmed_effect_projection_only(state, &proposal, &claim_id).await?;
        sync_main_chat_task_blockers_after_review_proposal_accept(state, &accepted).await;
        report.agent_runs_reconciled +=
            reconcile_agent_runs_for_proposal_with_admission(state, &accepted.id, admission)
                .await?;
        report.proposal_projections_repaired += 1;
    }

    // A process may stop after the Proposal projection commits but before every linked
    // AgentRun projection is updated. Reconcile only the bounded indexed wait/unknown
    // queue; never scan all historical runs or invoke the effect applicator.
    let reconciliation_page = {
        let Some(store) = state.agent_run_store.as_ref() else {
            return Err("AgentRun store is unavailable for Proposal reconciliation.".into());
        };
        let store = store.lock().await.clone();
        crate::terminal_owner_write_gateway::register_agent_run_store_result(
            state,
            store
                .take_review_reconciliation_page(bounded_limit)
                .map_err(|error| error.to_string()),
        )?
    };
    report.agent_run_backlog_may_remain = reconciliation_page.backlog_may_remain;
    report.agent_run_candidates_examined = reconciliation_page.proposal_ids.len();
    report.agent_run_cursor_advanced = reconciliation_page.cursor_advanced;
    report.agent_run_cursor_wrapped = reconciliation_page.wrapped;
    for proposal_id in reconciliation_page.proposal_ids {
        let (proposal, dispatch_state) = {
            let store = state
                .proposal_store
                .as_ref()
                .ok_or_else(proposal_store_missing)?
                .lock()
                .await;
            let proposal = store
                .get_proposal(&proposal_id)
                .map_err(|error| error.to_string())?;
            let dispatch_state = store
                .dispatch_state(&proposal_id)
                .map_err(|error| error.to_string())?;
            (proposal, dispatch_state)
        };
        if let Some(proposal) = proposal.as_ref() {
            if proposal.status == ProposalStatus::Accepted
                && dispatch_state.as_deref() == Some("confirmed")
            {
                sync_main_chat_task_blockers_after_review_proposal_accept(state, proposal).await;
            }
        }
        // Every durable dispatch state is projected by the canonical gateway.
        // Missing Proposal rows and unknown/new states remain unknown; they are
        // never silently treated as unclaimed or confirmed.
        report.agent_runs_reconciled +=
            reconcile_agent_runs_for_proposal_with_admission(state, &proposal_id, admission)
                .await?;
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
    let record = {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        store
            .artifact_effect(&proposal.id)
            .map_err(|error| runtime_proposal_store_error(state, error))?
    };
    let Some(record) = record.filter(|record| record.state == ArtifactEffectState::Confirmed)
    else {
        return Ok(None);
    };
    let path = proposal
        .after
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "confirmed artifact Proposal lost after.path".to_string())?;
    let content = proposal
        .after
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("");
    let safe_paths = { state.config.lock().await.system.safe_paths.clone() };
    let prepared = prepare_artifact_materialization(
        &proposal.id,
        &record.dispatch_claim_id,
        path,
        content,
        &safe_paths,
    )?;
    if prepared.target_reference_digest != record.target_reference_digest
        || prepared.content_digest != record.content_digest
        || prepared.byte_size != record.byte_size
        || prepared.media_type != record.media_type
    {
        return Err("confirmed artifact receipt binding mismatch".into());
    }
    let observed = record
        .observed_content_digest
        .filter(|digest| digest == &record.content_digest)
        .ok_or_else(|| "confirmed artifact observed digest missing".to_string())?;
    Ok(Some(confirmed_artifact_receipt(&prepared, observed)))
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LifeModelProposalPatchSourceMappingReport {
    proposal_source: ProposalSource,
    patch_source: PatchSource,
    exact_source_mapping: bool,
    metadata_safe_fallback: bool,
    apply_allowed: bool,
    metadata_safe: bool,
    contains_raw_proposal_payload: bool,
    contains_raw_lifemodel_patch_value: bool,
    contains_raw_memory_text: bool,
    contains_raw_chat_text: bool,
    contains_raw_tool_payload: bool,
    default_chat_route_unchanged: bool,
    default_chat_entrypoints_changed: bool,
    default_chat_route: String,
    proposal_first_convergence_complete: bool,
    required_follow_up: String,
    blocking_reasons: Vec<String>,
}

fn lifemodel_patch_source_mapping_for_proposal_source(
    source: ProposalSource,
) -> (PatchSource, bool, Option<&'static str>) {
    match source {
        ProposalSource::BuilderReview => (PatchSource::BuilderReview, true, None),
        ProposalSource::CalibrationRun => (PatchSource::Calibration, true, None),
        ProposalSource::FeedbackEvolution => (PatchSource::Evolution, true, None),
        ProposalSource::Manual => (PatchSource::Manual, true, None),
        ProposalSource::ChatConversation => (PatchSource::ChatConversation, true, None),
        ProposalSource::ProactiveAgent => (PatchSource::ProactiveAgent, true, None),
        ProposalSource::SkillRuntime => (PatchSource::SkillRuntime, true, None),
        ProposalSource::Plugin => (PatchSource::Plugin, true, None),
        ProposalSource::NetworkConsent => (
            PatchSource::Manual,
            false,
            Some("network_consent_is_not_a_lifemodel_patch_source"),
        ),
        ProposalSource::MemoryGovernance => (PatchSource::MemoryGovernance, true, None),
        ProposalSource::PlanningSession => (PatchSource::PlanningSession, true, None),
    }
}

fn evaluate_lifemodel_proposal_patch_source_mapping(
    proposal: &AgentProposal,
) -> LifeModelProposalPatchSourceMappingReport {
    let (patch_source, exact_source_mapping, fallback_reason) =
        lifemodel_patch_source_mapping_for_proposal_source(proposal.source);

    let mut blocking_reasons = Vec::new();
    if let Some(reason) = fallback_reason {
        blocking_reasons.push(format!("{reason}:using_manual_metadata_safe_fallback"));
    }

    let metadata_safe_fallback = !exact_source_mapping;
    let required_follow_up = if metadata_safe_fallback {
        format!(
            "W90+: confirm a dedicated PatchSource variant or accepted Manual fallback policy for {} before proposal-first convergence.",
            proposal.source
        )
    } else {
        "none".to_string()
    };

    LifeModelProposalPatchSourceMappingReport {
        proposal_source: proposal.source,
        patch_source,
        exact_source_mapping,
        metadata_safe_fallback,
        apply_allowed: true,
        metadata_safe: true,
        contains_raw_proposal_payload: false,
        contains_raw_lifemodel_patch_value: false,
        contains_raw_memory_text: false,
        contains_raw_chat_text: false,
        contains_raw_tool_payload: false,
        default_chat_route_unchanged: true,
        default_chat_entrypoints_changed: false,
        default_chat_route: "main_chat_kernel".into(),
        proposal_first_convergence_complete: exact_source_mapping && fallback_reason.is_none(),
        required_follow_up,
        blocking_reasons,
    }
}

pub(crate) fn ensure_lifemodel_proposal_patch_source_mapping(
    proposal: &AgentProposal,
) -> Result<LifeModelProposalPatchSourceMappingReport, String> {
    let report = evaluate_lifemodel_proposal_patch_source_mapping(proposal);
    if !report.apply_allowed {
        return Err(format!(
            "LifeModel proposal PatchSource mapping is unsupported for proposal source {}.",
            proposal.source
        ));
    }
    if !report.metadata_safe
        || report.contains_raw_proposal_payload
        || report.contains_raw_lifemodel_patch_value
        || report.contains_raw_memory_text
        || report.contains_raw_chat_text
        || report.contains_raw_tool_payload
    {
        return Err(format!(
            "LifeModel proposal PatchSource mapping report is not metadata-safe for proposal source {}.",
            proposal.source
        ));
    }
    if proposal.source != ProposalSource::BuilderReview
        && report.patch_source == PatchSource::BuilderReview
    {
        return Err(format!(
            "LifeModel proposal source {} must not be mapped to BuilderReview.",
            proposal.source
        ));
    }
    if !report.exact_source_mapping && !report.metadata_safe_fallback {
        return Err(format!(
            "LifeModel proposal source {} has neither an exact PatchSource mapping nor a metadata-safe fallback.",
            proposal.source
        ));
    }
    Ok(report)
}

pub(crate) fn resolve_lifemodel_patch_source_for_proposal(proposal: &AgentProposal) -> PatchSource {
    evaluate_lifemodel_proposal_patch_source_mapping(proposal).patch_source
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct LifeModelProposalPatchSourceReadinessEntry {
    proposal_source: ProposalSource,
    patch_source: PatchSource,
    exact_source_mapping: bool,
    metadata_safe_fallback: bool,
    unsupported_or_unclassified: bool,
    metadata_safe: bool,
    follow_up: String,
    blocking_reasons: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct LifeModelProposalPatchSourceReadinessReport {
    readiness_ready: bool,
    metadata_safe: bool,
    contains_raw_proposal_payload: bool,
    contains_raw_lifemodel_patch_value: bool,
    contains_raw_memory_text: bool,
    contains_raw_chat_text: bool,
    contains_raw_tool_payload: bool,
    exact_mapping_count: usize,
    metadata_safe_fallback_count: usize,
    unsupported_or_unclassified_count: usize,
    builder_review_only_for_builder_review: bool,
    no_hardcoded_builder_review_in_apply_path: bool,
    apply_path_uses_mapping_ensure: bool,
    apply_path_uses_source_resolver: bool,
    default_chat_route_unchanged: bool,
    proposal_first_convergence_complete: bool,
    blocking_reasons: Vec<String>,
    entries: Vec<LifeModelProposalPatchSourceReadinessEntry>,
}

#[allow(dead_code)]
fn w89_push_unique(blocking_reasons: &mut Vec<String>, reason: impl Into<String>) {
    let reason = reason.into();
    if !blocking_reasons.contains(&reason) {
        blocking_reasons.push(reason);
    }
}

#[allow(dead_code)]
fn lifemodel_proposal_patch_source_readiness_sources() -> [ProposalSource; 10] {
    [
        ProposalSource::BuilderReview,
        ProposalSource::CalibrationRun,
        ProposalSource::FeedbackEvolution,
        ProposalSource::Manual,
        ProposalSource::ChatConversation,
        ProposalSource::ProactiveAgent,
        ProposalSource::SkillRuntime,
        ProposalSource::Plugin,
        ProposalSource::MemoryGovernance,
        ProposalSource::PlanningSession,
    ]
}

#[allow(dead_code)]
fn lifemodel_proposal_patch_source_readiness_entry(
    source: ProposalSource,
) -> LifeModelProposalPatchSourceReadinessEntry {
    let (patch_source, exact_source_mapping, fallback_reason) =
        lifemodel_patch_source_mapping_for_proposal_source(source);
    let metadata_safe_fallback = fallback_reason.is_some();
    let unsupported_or_unclassified = !exact_source_mapping && !metadata_safe_fallback;
    let follow_up = if metadata_safe_fallback {
        format!(
            "W90+: confirm a dedicated PatchSource variant or accepted Manual fallback policy for {} before proposal-first convergence.",
            source
        )
    } else if unsupported_or_unclassified {
        format!(
            "Classify proposal source {} before applying LifeModel proposal patches.",
            source
        )
    } else {
        "none".into()
    };
    let mut blocking_reasons = Vec::new();
    if metadata_safe_fallback {
        w89_push_unique(
            &mut blocking_reasons,
            format!("proposal_patch_source_fallback_strategy_unconfirmed:{source}"),
        );
    }
    if unsupported_or_unclassified {
        w89_push_unique(
            &mut blocking_reasons,
            format!("proposal_patch_source_unclassified:{source}"),
        );
    }
    if source != ProposalSource::BuilderReview && patch_source == PatchSource::BuilderReview {
        w89_push_unique(
            &mut blocking_reasons,
            format!("non_builder_review_source_mapped_to_builder_review:{source}"),
        );
    }

    LifeModelProposalPatchSourceReadinessEntry {
        proposal_source: source,
        patch_source,
        exact_source_mapping,
        metadata_safe_fallback,
        unsupported_or_unclassified,
        metadata_safe: true,
        follow_up,
        blocking_reasons,
    }
}

#[allow(dead_code)]
fn default_chat_bodies_do_not_call_lifemodel_proposal_patch_helpers(
    send_message_body: &str,
    start_stream_message_body: &str,
) -> bool {
    let forbidden_helpers = [
        "LifeModelProposalPatchSourceMappingReport",
        "evaluate_lifemodel_proposal_patch_source_mapping",
        "ensure_lifemodel_proposal_patch_source_mapping",
        "resolve_lifemodel_patch_source_for_proposal",
        "LifeModelProposalPatchSourceReadinessReport",
        "evaluate_lifemodel_proposal_patch_source_readiness",
        "ensure_lifemodel_proposal_patch_source_readiness",
    ];
    forbidden_helpers.iter().all(|helper| {
        !send_message_body.contains(helper) && !start_stream_message_body.contains(helper)
    })
}

#[allow(dead_code)]
fn apply_path_uses_source_resolver_for_lifemodel_patch_from_proposal(
    apply_proposal_to_state_body: &str,
) -> bool {
    let resolver_index =
        apply_proposal_to_state_body.find("resolve_lifemodel_patch_source_for_proposal");
    let from_proposal_index = apply_proposal_to_state_body.find("LifeModelPatch::from_proposal");
    match (resolver_index, from_proposal_index) {
        (Some(resolver_index), Some(from_proposal_index))
            if resolver_index < from_proposal_index =>
        {
            apply_proposal_to_state_body[from_proposal_index..].contains("patch_source,")
        }
        _ => false,
    }
}

#[allow(dead_code)]
fn evaluate_lifemodel_proposal_patch_source_readiness(
    apply_proposal_to_state_body: &str,
    send_message_body: &str,
    start_stream_message_body: &str,
) -> LifeModelProposalPatchSourceReadinessReport {
    let entries: Vec<_> = lifemodel_proposal_patch_source_readiness_sources()
        .into_iter()
        .map(lifemodel_proposal_patch_source_readiness_entry)
        .collect();
    let exact_mapping_count = entries
        .iter()
        .filter(|entry| entry.exact_source_mapping)
        .count();
    let metadata_safe_fallback_count = entries
        .iter()
        .filter(|entry| entry.metadata_safe_fallback)
        .count();
    let unsupported_or_unclassified_count = entries
        .iter()
        .filter(|entry| entry.unsupported_or_unclassified)
        .count();
    let builder_review_only_for_builder_review = entries.iter().all(|entry| {
        entry.patch_source != PatchSource::BuilderReview
            || entry.proposal_source == ProposalSource::BuilderReview
    });
    let no_hardcoded_builder_review_in_apply_path =
        !apply_proposal_to_state_body.contains("PatchSource::BuilderReview");
    let apply_path_uses_mapping_ensure =
        apply_proposal_to_state_body.contains("ensure_lifemodel_proposal_patch_source_mapping");
    let apply_path_uses_source_resolver =
        apply_path_uses_source_resolver_for_lifemodel_patch_from_proposal(
            apply_proposal_to_state_body,
        );
    let default_chat_route_unchanged =
        default_chat_bodies_do_not_call_lifemodel_proposal_patch_helpers(
            send_message_body,
            start_stream_message_body,
        );

    let contains_raw_proposal_payload = false;
    let contains_raw_lifemodel_patch_value = false;
    let contains_raw_memory_text = false;
    let contains_raw_chat_text = false;
    let contains_raw_tool_payload = false;
    let metadata_safe = entries.iter().all(|entry| entry.metadata_safe)
        && !contains_raw_proposal_payload
        && !contains_raw_lifemodel_patch_value
        && !contains_raw_memory_text
        && !contains_raw_chat_text
        && !contains_raw_tool_payload;
    let proposal_first_convergence_complete = true;

    let mut blocking_reasons = Vec::new();
    if metadata_safe_fallback_count > 0 {
        w89_push_unique(
            &mut blocking_reasons,
            "proposal_patch_source_fallback_strategy_unconfirmed",
        );
    }
    if unsupported_or_unclassified_count > 0 {
        w89_push_unique(
            &mut blocking_reasons,
            "proposal_patch_source_unsupported_or_unclassified",
        );
    }
    if !builder_review_only_for_builder_review {
        w89_push_unique(
            &mut blocking_reasons,
            "non_builder_review_source_mapped_to_builder_review",
        );
    }
    if !no_hardcoded_builder_review_in_apply_path {
        w89_push_unique(
            &mut blocking_reasons,
            "apply_proposal_to_state_hardcodes_builder_review",
        );
    }
    if !apply_path_uses_mapping_ensure {
        w89_push_unique(
            &mut blocking_reasons,
            "apply_proposal_to_state_missing_patch_source_mapping_ensure",
        );
    }
    if !apply_path_uses_source_resolver {
        w89_push_unique(
            &mut blocking_reasons,
            "apply_proposal_to_state_missing_patch_source_resolver",
        );
    }
    if !default_chat_route_unchanged {
        w89_push_unique(
            &mut blocking_reasons,
            "default_chat_entrypoints_call_proposal_patch_source_helper",
        );
    }
    if !metadata_safe {
        w89_push_unique(&mut blocking_reasons, "readiness_report_metadata_not_safe");
    }

    let readiness_ready = blocking_reasons.is_empty()
        && metadata_safe
        && builder_review_only_for_builder_review
        && no_hardcoded_builder_review_in_apply_path
        && apply_path_uses_mapping_ensure
        && apply_path_uses_source_resolver
        && default_chat_route_unchanged
        && unsupported_or_unclassified_count == 0
        && proposal_first_convergence_complete;

    LifeModelProposalPatchSourceReadinessReport {
        readiness_ready,
        metadata_safe,
        contains_raw_proposal_payload,
        contains_raw_lifemodel_patch_value,
        contains_raw_memory_text,
        contains_raw_chat_text,
        contains_raw_tool_payload,
        exact_mapping_count,
        metadata_safe_fallback_count,
        unsupported_or_unclassified_count,
        builder_review_only_for_builder_review,
        no_hardcoded_builder_review_in_apply_path,
        apply_path_uses_mapping_ensure,
        apply_path_uses_source_resolver,
        default_chat_route_unchanged,
        proposal_first_convergence_complete,
        blocking_reasons,
        entries,
    }
}

#[allow(dead_code)]
fn ensure_lifemodel_proposal_patch_source_readiness(
    apply_proposal_to_state_body: &str,
    send_message_body: &str,
    start_stream_message_body: &str,
) -> Result<LifeModelProposalPatchSourceReadinessReport, String> {
    let report = evaluate_lifemodel_proposal_patch_source_readiness(
        apply_proposal_to_state_body,
        send_message_body,
        start_stream_message_body,
    );
    if report.readiness_ready {
        Ok(report)
    } else {
        Err(format!(
            "LifeModel proposal PatchSource readiness blocked: {}",
            report.blocking_reasons.join(",")
        ))
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

async fn apply_external_write_artifact(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    claim_id: &str,
) -> ArtifactApplyOutcome {
    let path = match proposal.after.get("path").and_then(Value::as_str) {
        Some(path) => path,
        None => {
            let code = "artifact_path_missing";
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(code.into());
        }
    };
    let content = proposal
        .after
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("");
    if content.len() > EXTERNAL_WRITE_MAX_SIZE {
        let code = "artifact_content_too_large";
        let _ = persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
        return ArtifactApplyOutcome::FailedBeforeEffect(code.into());
    }
    let safe_paths = { state.config.lock().await.system.safe_paths.clone() };
    let prepared = match prepare_artifact_materialization(
        &proposal.id,
        claim_id,
        path,
        content,
        &safe_paths,
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
    let prepared_record = {
        let store = match state.proposal_store.as_ref() {
            Some(store) => store.lock().await,
            None => return ArtifactApplyOutcome::FailedBeforeEffect(proposal_store_missing()),
        };
        store.prepare_artifact_effect(
            &proposal.id,
            claim_id,
            &prepared.target_reference_digest,
            &prepared.content_digest,
            prepared.byte_size,
            &prepared.media_type,
        )
    };
    if let Err(error) = prepared_record {
        let detail = runtime_proposal_store_error(state, error);
        let _ = persist_artifact_failed_before_effect(
            state,
            &proposal.id,
            claim_id,
            "artifact_prepare_receipt_failed",
        )
        .await;
        return ArtifactApplyOutcome::FailedBeforeEffect(detail);
    }

    let stage_prepared = prepared.clone();
    let stage_content = content.to_string();
    let stage_result =
        tokio::task::spawn_blocking(move || stage_artifact_bytes(&stage_prepared, &stage_content))
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
    let staged = {
        let store = state
            .proposal_store
            .as_ref()
            .expect("ProposalStore checked before artifact staging")
            .lock()
            .await;
        store.mark_artifact_staged(&proposal.id, claim_id)
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
    let confirmed = {
        let store = state
            .proposal_store
            .as_ref()
            .expect("ProposalStore checked before artifact confirmation")
            .lock()
            .await;
        store.finish_artifact_confirmed(&proposal.id, claim_id, &observed_digest)
    };
    if !matches!(confirmed, Ok(true)) {
        return ArtifactApplyOutcome::Unknown("artifact_confirmed_receipt_unavailable".into());
    }
    ArtifactApplyOutcome::Confirmed {
        patch_result: patch_result_for_proposal(proposal, true, "artifact_materialized", None),
        receipt: Box::new(confirmed_artifact_receipt(&prepared, observed_digest)),
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

/// Minimal URL-encoding (only encodes space, newline, and special chars).
fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '\n' => "%0A".to_string(),
            '\r' => "%0D".to_string(),
            '&' => "%26".to_string(),
            '=' => "%3D".to_string(),
            '+' => "%2B".to_string(),
            '%' => "%25".to_string(),
            '#' => "%23".to_string(),
            c if c.is_ascii_alphanumeric()
                || c == '-'
                || c == '_'
                || c == '.'
                || c == '!'
                || c == '~'
                || c == '*'
                || c == '\''
                || c == '('
                || c == ')' =>
            {
                c.to_string()
            }
            c => format!("%{:02X}", c as u8),
        })
        .collect()
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

/// Build a minimal ICS (iCalendar) VEVENT string from proposal after data.
fn build_ics_event(after: &Value) -> Result<String, String> {
    let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let uid = uuid::Uuid::new_v4().to_string();
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

#[allow(dead_code)]
fn set_path_value(root: &mut Value, path: &str, value: Value) -> Result<(), String> {
    let mut current = root;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            let object = current
                .as_object_mut()
                .ok_or_else(|| format!("路径 `{}` 的父节点不是对象。", path))?;
            if !object.contains_key(part) {
                return Err(format!("人生模型不包含字段路径 `{}`。", path));
            }
            object.insert(part.to_string(), value);
            return Ok(());
        }

        current = current
            .get_mut(part)
            .ok_or_else(|| format!("人生模型不包含字段路径 `{}`。", path))?;
    }
    Err("Proposal affected_path 不能为空。".to_string())
}

fn json_value_at_dot_path(root: &Value, path: &str) -> Option<Value> {
    let mut current = root;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current.clone())
}

#[allow(dead_code)]
fn apply_life_model_value(
    model: &LifeModel,
    path: &str,
    after: Value,
) -> Result<LifeModel, String> {
    let mut value = serde_json::to_value(model).map_err(|e| e.to_string())?;
    set_path_value(&mut value, path, after)?;
    serde_json::from_value(value).map_err(|e| format!("Proposal 值无法转换为 LifeModel：{}", e))
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
                        return Err(
                            "action_bound ToolPermission 不能声明 network manifest action。"
                                .to_string(),
                        );
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
            let path = after
                .get("path")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty());
            if path.is_none() {
                return Err(
                    "ExternalWriteAction Proposal 缺少 after.path（非空字符串）。".to_string(),
                );
            }
            Ok(())
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
            Ok(())
        }
        ProposalType::PluginPermission
        | ProposalType::ModelPolicyChange
        | ProposalType::ScheduleCheckin
        | ProposalType::Unsupported => {
            // These types are not yet implemented; validation passes but apply will fail
            Ok(())
        }
    }
}

fn validate_proposal_for_acceptance(proposal: &AgentProposal) -> Result<(), String> {
    validate_proposal_payload(proposal.proposal_type, &proposal.after)?;
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
            let canonical_affected_path = canonical_lifemodel_path(&proposal.affected_path);
            let model = {
                let manager = state.life_model_manager.lock().await;
                manager.load().map_err(|e| e.to_string())?
            };
            let source_mapping = ensure_lifemodel_proposal_patch_source_mapping(proposal)?;
            if source_mapping.metadata_safe_fallback {
                log::warn!(
                    "[proposal] W88 PatchSource metadata-safe fallback: proposal_source={}, patch_source={}, follow_up={}, blockers={}",
                    proposal.source,
                    source_mapping.patch_source,
                    source_mapping.required_follow_up,
                    source_mapping.blocking_reasons.join("|")
                );
            }
            let patch_source = resolve_lifemodel_patch_source_for_proposal(proposal);

            if proposal.proposal_type == ProposalType::LifeModelUpdate
                && canonical_affected_path
                    == openlife_core::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH
            {
                if proposal.source != ProposalSource::BuilderReview {
                    return Err(
                        "lifemodel_patch_batch_is_restricted_to_builder_review_source".into(),
                    );
                }
                let batch = serde_json::from_value::<
                    openlife_core::life_model::patch::LifeModelPatchBatchV1,
                >(after)
                .map_err(|_| "invalid_lifemodel_patch_batch_payload".to_string())?;
                batch.validate()?;
                if let Some(operation) = batch.operations.iter().find(|operation| {
                    openlife_core::life_model_write_gateway::life_model_field_authority(
                        &operation.path,
                    ) != openlife_core::life_model_write_gateway::LifeModelFieldAuthority::CanonicalLifeModel
                }) {
                    return Ok(openlife_core::life_model::patch::PatchApplyResult {
                        patch_id: format!("batch:{}", proposal.id),
                        success: false,
                        path: operation.path.clone(),
                        operation: "lifemodel_patch_batch_field_authority_blocked".into(),
                        error: Some("builder_batch_contains_non_lifemodel_owned_field".into()),
                    });
                }
                let builder_risk = match proposal.risk_level {
                    openlife_core::agent::RiskLevel::Low => openlife_core::builder::RiskLevel::Low,
                    openlife_core::agent::RiskLevel::Medium => {
                        openlife_core::builder::RiskLevel::Medium
                    }
                    openlife_core::agent::RiskLevel::High => {
                        openlife_core::builder::RiskLevel::High
                    }
                    openlife_core::agent::RiskLevel::Critical => {
                        openlife_core::builder::RiskLevel::High
                    }
                };
                let signals = batch
                    .operations
                    .iter()
                    .map(|operation| {
                        let dimension = match operation.path.split('.').next() {
                            Some("identity") => openlife_core::builder::BuilderDimension::Identity,
                            Some("goals") => openlife_core::builder::BuilderDimension::Goals,
                            Some("capabilities") => {
                                openlife_core::builder::BuilderDimension::Capabilities
                            }
                            Some("state") | Some("preferences") => {
                                openlife_core::builder::BuilderDimension::State
                            }
                            _ => return Err("invalid_builder_candidate_path".to_string()),
                        };
                        Ok(openlife_core::builder::BuilderSignal {
                            id: operation.candidate_id.clone(),
                            source_step: 0,
                            source_question_id: "builder_review_batch".into(),
                            dimension,
                            affected_path: operation.path.clone(),
                            proposed_value: operation.candidate.clone(),
                            confidence: proposal.confidence,
                            reason: "accepted_builder_review_candidate".into(),
                            risk_level: builder_risk,
                            user_status: openlife_core::builder::SignalUserStatus::Accepted,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let mut preview_model = model.clone();
                let (applied, skipped) =
                    openlife_core::builder::BuilderEngine::apply_signals_to_model(
                        &mut preview_model,
                        &signals,
                    );
                if !skipped.is_empty() || applied.len() != signals.len() {
                    return Err("invalid_builder_candidate_batch".into());
                }
                let before_value = serde_json::to_value(&model)
                    .map_err(|_| "lifemodel_batch_before_serialization_failed".to_string())?;
                let after_value = serde_json::to_value(&preview_model)
                    .map_err(|_| "lifemodel_batch_after_serialization_failed".to_string())?;
                let mut patches = Vec::with_capacity(batch.operations.len());
                for operation in batch.operations {
                    let path = canonical_lifemodel_path(&operation.path);
                    if path != operation.path {
                        return Err("lifemodel_patch_batch_path_must_be_canonical".into());
                    }
                    let path_pointer = openlife_core::life_model::patch::dot_to_pointer(&path);
                    let path_display =
                        openlife_core::life_model::patch::pointer_to_display(&path_pointer, &model);
                    let before = json_value_at_dot_path(&before_value, &path)
                        .ok_or_else(|| "builder_candidate_before_path_missing".to_string())?;
                    let after = json_value_at_dot_path(&after_value, &path)
                        .ok_or_else(|| "builder_candidate_after_path_missing".to_string())?;
                    patches.push(
                        openlife_core::life_model::patch::LifeModelPatch::from_proposal(
                            &proposal.id,
                            &path_pointer,
                            &path_display,
                            openlife_core::life_model::patch::PatchOp::Replace,
                            Some(before),
                            after,
                            &proposal.reason,
                            proposal.confidence,
                            proposal.risk_level,
                            patch_source,
                        ),
                    );
                }
                return life_model_write_gateway::materialize_accepted_lifemodel_patch_batch_with_state(
                    state, proposal, patches,
                )
                .await;
            }

            let path_pointer =
                openlife_core::life_model::patch::dot_to_pointer(&canonical_affected_path);
            let path_display =
                openlife_core::life_model::patch::pointer_to_display(&path_pointer, &model);

            let patch = openlife_core::life_model::patch::LifeModelPatch::from_proposal(
                &proposal.id,
                &path_pointer,
                &path_display,
                openlife_core::life_model::patch::PatchOp::Replace,
                proposal.before.clone(),
                after.clone(),
                &proposal.reason,
                proposal.confidence,
                proposal.risk_level,
                patch_source,
            );

            life_model_write_gateway::materialize_accepted_lifemodel_proposal_with_state(
                state, proposal, patch,
            )
            .await
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
                if !safe_paths.is_empty() {
                    let ics_content = build_ics_event(&after)?;
                    let ics_filename = format!("{}.ics", sanitize_filename(title));
                    let ics_path = std::path::PathBuf::from(&safe_paths[0]).join(&ics_filename);
                    if let Err(e) =
                        openlife_core::atomic_file::write_atomic(&ics_path, ics_content.as_bytes())
                    {
                        log::warn!(
                            "[proposal] Failed to write ICS file '{}': {}",
                            ics_path.display(),
                            e
                        );
                        projection_warning = Some(format!(
                            "projection_degraded: failed to materialize ICS view: {}",
                            e
                        ));
                    }
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

            // email.propose_draft: open system mail client via mailto: URI
            if tool == "email.propose_draft" {
                let to = after.get("to").and_then(Value::as_str).unwrap_or("");
                let subject = after.get("subject").and_then(Value::as_str).unwrap_or("");
                let body = after.get("body").and_then(Value::as_str).unwrap_or(content);
                let mailto = format!(
                    "mailto:{}?subject={}&body={}",
                    to,
                    urlencoding(subject),
                    urlencoding(body)
                );
                match open::that(&mailto) {
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
        ProposalType::PluginPermission
        | ProposalType::ModelPolicyChange
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

pub(crate) async fn get_pending_proposals_with_state(
    limit: i64,
    state: &Arc<AppState>,
) -> Result<Vec<AgentProposal>, String> {
    reconcile_durable_proposal_projections_with_state(state, limit.clamp(1, 200)).await?;
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;
    store
        .list_pending_proposals(limit.clamp(1, 200))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) async fn accept_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    accept_proposal_with_state_and_confirmation(proposal_id, state, None).await
}

async fn accept_proposal_with_state_and_confirmation(
    proposal_id: String,
    state: &Arc<AppState>,
    expected_native_confirmation_digest: Option<&str>,
) -> Result<serde_json::Value, String> {
    require_persistence_write(state)?;
    check_safe_mode(state)?;
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;

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
                sync_main_chat_task_blockers_after_review_proposal_accept(state, &accepted).await;
                if let Err(error) = reconcile_agent_runs_for_proposal(state, &accepted).await {
                    warnings.push(format!("AgentRun 投影仍等待 reconciliation: {}", error));
                }
                Ok(confirmed_effect_reconciliation_response(
                    &accepted,
                    true,
                    warnings,
                    artifact_receipt.clone(),
                ))
            }
            Err(error) => Ok(confirmed_effect_reconciliation_response(
                &proposal,
                false,
                vec![format!(
                    "Effect 已确认，Proposal 投影仍等待 reconciliation；未重放副作用: {}",
                    error
                )],
                artifact_receipt,
            )),
        };
    }
    if proposal.status == ProposalStatus::Accepted && dispatch_state.as_deref() == Some("confirmed")
    {
        let artifact_receipt = confirmed_artifact_receipt_from_store(state, &proposal).await?;
        let mut warnings = vec![
            "Proposal effect was already confirmed; the idempotent retry did not redispatch it."
                .to_string(),
        ];
        sync_main_chat_task_blockers_after_review_proposal_accept(state, &proposal).await;
        if let Err(error) = reconcile_agent_runs_for_proposal(state, &proposal).await {
            warnings.push(format!("AgentRun 投影仍等待 reconciliation: {}", error));
        }
        return Ok(confirmed_effect_reconciliation_response(
            &proposal,
            true,
            warnings,
            artifact_receipt,
        ));
    }
    ensure_pending_or_postponed(&proposal)?;
    validate_proposal_for_acceptance(&proposal)?;
    if is_builder_lifemodel_patch_batch(&proposal) {
        let batch =
            serde_json::from_value::<openlife_core::life_model::patch::LifeModelPatchBatchV1>(
                proposal.after.clone(),
            )
            .map_err(|_| "invalid_lifemodel_patch_batch_payload".to_string())?;
        batch.validate()?;
    }
    if matches!(
        proposal.proposal_type,
        ProposalType::PluginPermission
            | ProposalType::ModelPolicyChange
            | ProposalType::ScheduleCheckin
            | ProposalType::Unsupported
    ) {
        return Err(format!(
            "{} Proposal 尚未接入应用器，已保持 pending。",
            proposal.proposal_type
        ));
    }
    let terminal_owner_origin = {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        store
            .terminal_owner_origin_binding(&proposal_id)
            .map_err(|error| error.to_string())?
    };
    let mut terminal_owner_fence = None;
    if let Some(origin) = terminal_owner_origin.as_ref() {
        let epoch_state = {
            let event_store = state
                .main_chat_agent_event_store
                .as_ref()
                .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?
                .lock()
                .await;
            let epoch = event_store
                .terminal_owner_epoch(origin.task_session_id())
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "terminal_owner_epoch_missing".to_string())?;
            if epoch.run_id() != origin.run_id() {
                return Err("terminal_owner_epoch_run_mismatch".to_string());
            }
            epoch.state()
        };
        if matches!(
            epoch_state,
            crate::main_chat_event_stream::TerminalOwnerSealState::Open
                | crate::main_chat_event_stream::TerminalOwnerSealState::Sealing
        ) {
            let reason_code = match epoch_state {
                crate::main_chat_event_stream::TerminalOwnerSealState::Open => "origin_turn_open",
                crate::main_chat_event_stream::TerminalOwnerSealState::Sealing => {
                    "origin_turn_sealing"
                }
                crate::main_chat_event_stream::TerminalOwnerSealState::Sealed => unreachable!(),
            };
            return Ok(serde_json::json!({
                "success": false,
                "status": "deferred",
                "reasonCode": reason_code,
                "proposalId": proposal_id,
                "dispatchState": "unclaimed",
                "durableWriteExecuted": false,
            }));
        }
        terminal_owner_fence = Some(
            crate::terminal_owner_write_gateway::acquire_terminal_owner_task_fence(
                origin.task_session_id(),
            )
            .await,
        );
        let sealed_epoch = state
            .main_chat_agent_event_store
            .as_ref()
            .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?
            .lock()
            .await
            .terminal_owner_epoch(origin.task_session_id())
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "terminal_owner_epoch_missing".to_string())?;
        if sealed_epoch.run_id() != origin.run_id()
            || sealed_epoch.state() != crate::main_chat_event_stream::TerminalOwnerSealState::Sealed
        {
            return Err("terminal_owner_epoch_changed_before_review_claim".to_string());
        }
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
    let terminal_owner_fence_guard = terminal_owner_fence;
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
    if terminal_owner_origin.is_some() && proposal.proposal_type == ProposalType::MemoryWrite {
        let gateway = terminal_owner_write_gateway_from_state(state).await?;
        let relation_kind = terminal_owner_relation_kind(state, &proposal_id).await?;
        let transition = if matches!(
            relation_kind,
            Some(openlife_core::agent::ProposalTerminalRelationKind::NonBlockingSuccessor)
                | Some(
                    openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite
                )
        ) {
            gateway
                .apply_claimed_review_without_task_transition(review_acceptance)
                .await
                .map_err(|error| error.to_string())?;
            None
        } else {
            Some(
                gateway
                    .apply_claimed_review_acceptance(review_acceptance)
                    .await
                    .map_err(|error| error.to_string())?,
            )
        };
        let mut response = serde_json::json!({
            "success": true,
            "effect_status": "confirmed",
            "proposal_projection_status": "confirmed",
            "proposalId": proposal.id,
        });
        if let Some(transition) = transition {
            response["terminalOwnerTransition"] = serde_json::json!({
                "beforeOwnerRevision": transition.before_owner_revision,
                "afterOwnerRevision": transition.after_owner_revision,
                "beforeOwnerDigest": transition.before_owner_digest,
                "afterOwnerDigest": transition.after_owner_digest,
                "localTransitionReceiptRef": transition.local_transition_receipt_ref,
                "localTransitionReceiptDigest": transition.local_transition_receipt_digest,
                "successorEventId": transition.successor_event_id,
            });
        }
        return Ok(response);
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
                return Err(format!(
                    "Artifact materialization failed before effect: {error}"
                ));
            }
            ArtifactApplyOutcome::Unknown(error) => {
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
    let effect_receipt_persisted = if artifact_materialization.is_some() {
        true
    } else {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        match store.mark_effect_confirmed_projection_pending(&proposal_id, &dispatch_claim_id) {
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
    let mut terminal_owner_transition_response = None;
    let mut main_chat_task_sync = Vec::new();
    let proposal_projected = if effect_receipt_persisted {
        if let Some(origin) = terminal_owner_origin.as_ref() {
            let relation_kind = terminal_owner_relation_kind(state, &proposal_id).await?;
            let gateway = terminal_owner_write_gateway_from_state(state).await?;
            if matches!(
                relation_kind,
                Some(openlife_core::agent::ProposalTerminalRelationKind::NonBlockingSuccessor)
                    | Some(
                        openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite
                    )
            ) {
                match gateway
                    .apply_claimed_review_without_task_transition(review_acceptance)
                    .await
                {
                    Ok(_) => {
                        proposal = get_proposal_with_state(state, &proposal_id).await?;
                        true
                    }
                    Err(error) => {
                        warnings.push(format!(
                            "Effect 已确认，但 typed terminal-owner Proposal 投影失败并等待 reconciliation: {}",
                            error
                        ));
                        false
                    }
                }
            } else {
                match gateway
                    .apply_claimed_review_acceptance(review_acceptance)
                    .await
                {
                    Ok(transition) => {
                        terminal_owner_transition_response = Some(serde_json::json!({
                            "beforeOwnerRevision": transition.before_owner_revision,
                            "afterOwnerRevision": transition.after_owner_revision,
                            "beforeOwnerDigest": transition.before_owner_digest,
                            "afterOwnerDigest": transition.after_owner_digest,
                            "localTransitionReceiptRef": transition.local_transition_receipt_ref,
                            "localTransitionReceiptDigest": transition.local_transition_receipt_digest,
                            "successorEventId": transition.successor_event_id,
                        }));
                        proposal = get_proposal_with_state(state, &proposal_id).await?;
                        if let Some(task_store) = state.main_chat_agent_session_store.as_ref() {
                            if let Ok(Some(session)) = task_store
                                .lock()
                                .await
                                .load_session(origin.task_session_id())
                            {
                                main_chat_task_sync.push(serde_json::json!({
                                "taskSessionId": origin.task_session_id(),
                                "proposalBlockerCleared": true,
                                "remainingBlockerCount": session.pending_blockers.len(),
                                "taskCompletedAfterProposalAccept": session.status
                                    == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed,
                            }));
                            }
                        }
                        true
                    }
                    Err(error) => {
                        warnings.push(format!(
                            "Effect 已确认，但 terminal-owner successor 投影失败并等待 reconciliation: {}",
                            error
                        ));
                        false
                    }
                }
            }
        } else {
            match project_confirmed_effect_projection_only(state, &proposal, &dispatch_claim_id)
                .await
            {
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
        }
    } else {
        false
    };
    let dispatch_projection_confirmed = proposal_projected;
    if proposal_projected {
        record_maturation_proposal_outcome_evidence_with_state(
            state,
            &proposal,
            MaturationProposalOutcome::Accepted,
        )
        .await;
    }
    drop(terminal_owner_fence_guard);
    if let Err(error) = reconcile_agent_runs_for_proposal(state, &proposal).await {
        warnings.push(format!("AgentRun 投影仍等待 reconciliation: {}", error));
    }
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
        "warnings": warnings,
    });
    if let Some(receipt) = artifact_materialization {
        response["artifactMaterialization"] =
            serde_json::to_value(receipt).unwrap_or(serde_json::Value::Null);
    }
    if !main_chat_task_sync.is_empty() {
        response["mainChatTaskSync"] = serde_json::Value::Array(main_chat_task_sync);
    }
    if let Some(transition) = terminal_owner_transition_response {
        response["terminalOwnerTransition"] = transition;
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

fn proposal_type_resolves_main_chat_review_blocker(proposal_type: ProposalType) -> bool {
    matches!(
        proposal_type,
        ProposalType::MemoryWrite
            | ProposalType::MemoryArchive
            | ProposalType::LifeModelUpdate
            | ProposalType::GoalUpdate
            | ProposalType::StateUpdate
            | ProposalType::PreferenceUpdate
            | ProposalType::CapabilityUpdate
            | ProposalType::ExternalWriteAction
    )
}

async fn terminal_owner_write_gateway_from_state(
    state: &Arc<AppState>,
) -> Result<crate::terminal_owner_write_gateway::TerminalOwnerWriteGateway, String> {
    crate::terminal_owner_write_gateway::TerminalOwnerWriteGateway::from_state(state).await
}

async fn terminal_owner_relation_kind(
    state: &Arc<AppState>,
    proposal_id: &str,
) -> Result<Option<openlife_core::agent::ProposalTerminalRelationKind>, String> {
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?
        .lock()
        .await;
    store
        .terminal_relation_projection_proof(proposal_id)
        .map(|proof| proof.map(|proof| proof.relation_kind()))
        .map_err(|error| error.to_string())
}

async fn sync_main_chat_task_blockers_after_review_proposal_accept(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Vec<serde_json::Value> {
    if !proposal_type_resolves_main_chat_review_blocker(proposal.proposal_type) {
        return Vec::new();
    }
    let (origin, relation_kind) = match state.proposal_store.as_ref() {
        Some(store) => {
            let store = store.lock().await;
            (
                store
                    .terminal_owner_origin_binding(&proposal.id)
                    .ok()
                    .flatten(),
                store
                    .terminal_relation_projection_proof(&proposal.id)
                    .ok()
                    .flatten()
                    .map(|proof| proof.relation_kind()),
            )
        }
        None => (None, None),
    };
    if relation_kind.is_some()
        && relation_kind
            != Some(openlife_core::agent::ProposalTerminalRelationKind::EffectBlockingPrerequisite)
    {
        return Vec::new();
    }
    let Some(origin) = origin else {
        return Vec::new();
    };
    let _fence = crate::terminal_owner_write_gateway::acquire_terminal_owner_task_fence(
        origin.task_session_id(),
    )
    .await;
    let materialized_acceptance = {
        let Some(proposal_store) = state.proposal_store.as_ref() else {
            return Vec::new();
        };
        let proposal_store = proposal_store.lock().await;
        match openlife_core::agent::ReviewWorkflow::new(&proposal_store)
            .materialized_acceptance_snapshot(&proposal.id)
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                log::warn!(
                    "[proposal] terminal owner materialized acceptance unavailable for {}: {}",
                    proposal.id,
                    error
                );
                return Vec::new();
            }
        }
    };
    let gateway = match terminal_owner_write_gateway_from_state(state).await {
        Ok(gateway) => gateway,
        Err(error) => {
            log::warn!(
                "[proposal] terminal owner gateway unavailable for {}: {}",
                proposal.id,
                error
            );
            return Vec::new();
        }
    };
    if let Err(error) = gateway
        .apply_materialized_review_successor(materialized_acceptance)
        .await
    {
        log::warn!(
            "[proposal] terminal owner successor reconciliation failed for {}: {}",
            proposal.id,
            error
        );
        return Vec::new();
    }
    let Some(store) = state.main_chat_agent_session_store.as_ref() else {
        return Vec::new();
    };
    let session = match store.lock().await.load_session(origin.task_session_id()) {
        Ok(Some(session)) => session,
        Ok(None) => return Vec::new(),
        Err(error) => {
            log::warn!(
                "[proposal] terminal owner task projection unavailable for {}: {}",
                proposal.id,
                error
            );
            return Vec::new();
        }
    };
    vec![serde_json::json!({
        "taskSessionId": origin.task_session_id(),
        "proposalBlockerCleared": true,
        "remainingBlockerCount": session.pending_blockers.len(),
        "taskCompletedAfterProposalAccept": session.status
            == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed,
    })]
}

pub(crate) async fn reject_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<(), String> {
    require_persistence_write(state)?;
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    ensure_pending_or_postponed(&proposal)?;
    ensure_review_change_precedes_effect_dispatch(state, &proposal_id).await?;
    let expected_status = proposal.status;
    proposal.reject();
    update_review_proposal_before_dispatch_with_state(state, &proposal, expected_status).await?;
    if let Err(error) = reconcile_agent_runs_for_proposal(state, &proposal).await {
        log::warn!(
            "[proposal] AgentRun rejection reconciliation pending for {}: {}",
            proposal.id,
            error
        );
    }
    record_maturation_proposal_outcome_evidence_with_state(
        state,
        &proposal,
        MaturationProposalOutcome::Rejected,
    )
    .await;
    record_rejected_proactive_reminder_evidence(state, &proposal).await;
    Ok(())
}

async fn record_maturation_proposal_outcome_evidence_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    outcome: MaturationProposalOutcome,
) {
    let evidence_store = state.evidence_store.lock().await;
    if let Err(e) = openlife_core::agent::record_maturation_proposal_outcome_evidence(
        &evidence_store,
        proposal,
        outcome,
    ) {
        log::warn!(
            "[LifeModel-Maturation] failed to record proposal outcome evidence for proposal {}: {}",
            proposal.id,
            e
        );
    }
}

async fn record_rejected_proactive_reminder_evidence(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) {
    let evidence_store = state.evidence_store.lock().await;
    if let Err(e) = openlife_core::proactive::ProactiveEngine::default()
        .record_rejected_reminder_proposal(&evidence_store, proposal)
    {
        log::warn!(
            "[LifeModel-HS] failed to record rejected reminder evidence for proposal {}: {}",
            proposal.id,
            e
        );
    }
}

pub(crate) async fn edit_proposal_with_state(
    proposal_id: String,
    new_after: Value,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    require_persistence_write(state)?;
    check_safe_mode(state)?;
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    ensure_pending_or_postponed(&proposal)?;
    ensure_review_change_precedes_effect_dispatch(state, &proposal_id).await?;
    if is_builder_lifemodel_patch_batch(&proposal) {
        return Err(
            "Builder batch Proposal requires a typed Builder editor; generic JSON edit is disabled."
                .into(),
        );
    }
    canonicalize_proposal_affected_path(&mut proposal);
    let expected_status = proposal.status;
    proposal.edit(new_after);
    update_review_proposal_before_dispatch_with_state(state, &proposal, expected_status).await?;
    if let Err(error) = reconcile_agent_runs_for_proposal(state, &proposal).await {
        log::warn!(
            "[proposal] AgentRun edit reconciliation pending for {}: {}",
            proposal.id,
            error
        );
    }
    record_maturation_proposal_outcome_evidence_with_state(
        state,
        &proposal,
        MaturationProposalOutcome::Edited,
    )
    .await;
    Ok(serde_json::json!({
        "success": true,
        "status": "edited_pending_review",
        "durable_write_executed": false,
    }))
}

pub(crate) async fn postpone_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<(), String> {
    require_persistence_write(state)?;
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    ensure_pending_or_postponed(&proposal)?;
    ensure_review_change_precedes_effect_dispatch(state, &proposal_id).await?;
    let expected_status = proposal.status;
    proposal.postpone();
    update_review_proposal_before_dispatch_with_state(state, &proposal, expected_status).await?;
    if let Err(error) = reconcile_agent_runs_for_proposal(state, &proposal).await {
        log::warn!(
            "[proposal] AgentRun postpone reconciliation pending for {}: {}",
            proposal.id,
            error
        );
    }
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

fn parse_memory_lifecycle_scope(scope: Option<String>) -> Option<MemoryLifecycleScope> {
    match scope.as_deref() {
        Some("global") => Some(MemoryLifecycleScope::Global),
        Some("workspace") => Some(MemoryLifecycleScope::Workspace),
        Some("conversation") => Some(MemoryLifecycleScope::Conversation),
        Some("project") => Some(MemoryLifecycleScope::Project),
        _ => None,
    }
}

fn parse_memory_lifecycle_status(status: Option<String>) -> Option<MemoryLifecycleStatus> {
    match status.as_deref() {
        Some("candidate") => Some(MemoryLifecycleStatus::Candidate),
        Some("pending_review") => Some(MemoryLifecycleStatus::PendingReview),
        Some("edited_pending_review") => Some(MemoryLifecycleStatus::EditedPendingReview),
        Some("accepted") => Some(MemoryLifecycleStatus::Accepted),
        Some("pending_materialization") => Some(MemoryLifecycleStatus::PendingMaterialization),
        Some("materialized") => Some(MemoryLifecycleStatus::Materialized),
        Some("materialization_failed") => Some(MemoryLifecycleStatus::MaterializationFailed),
        Some("rejected") => Some(MemoryLifecycleStatus::Rejected),
        Some("deferred") => Some(MemoryLifecycleStatus::Deferred),
        Some("superseded") => Some(MemoryLifecycleStatus::Superseded),
        Some("rolled_back") => Some(MemoryLifecycleStatus::RolledBack),
        _ => None,
    }
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

pub(crate) async fn list_memory_assets_with_state(
    scope: Option<String>,
    status: Option<String>,
    limit: i64,
    offset: i64,
    state: &Arc<AppState>,
) -> Result<Vec<MemoryLifecycleRecord>, String> {
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?;
    let store = lifecycle_store.lock().await;
    store
        .list_records(
            parse_memory_lifecycle_scope(scope),
            parse_memory_lifecycle_status(status),
            limit,
            offset.max(0),
        )
        .map_err(|e| e.to_string())
}

pub(crate) async fn get_memory_asset_with_state(
    memory_id: String,
    state: &Arc<AppState>,
) -> Result<MemoryLifecycleRecord, String> {
    ensure_exact_memory_id(&memory_id)?;
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?;
    let store = lifecycle_store.lock().await;
    store
        .get_record(&memory_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Memory asset not found: {memory_id}"))
}

pub(crate) async fn get_memory_lifecycle_events_with_state(
    memory_id: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    ensure_exact_memory_id(&memory_id)?;
    let lifecycle_store = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(memory_lifecycle_store_missing)?;
    let store = lifecycle_store.lock().await;
    let events = store
        .lifecycle_events(&memory_id)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(events).map_err(|e| e.to_string())
}

pub(crate) async fn rebuild_memory_materialized_view_with_state(
    scope: Option<String>,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, String> {
    let view = memory_gateway::rebuild_materialized_memory_view_with_state(
        parse_memory_lifecycle_scope(scope),
        state,
    )
    .await?;
    serde_json::to_value(view).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_pending_proposals(
    limit: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentProposal>, String> {
    get_pending_proposals_with_state(limit, state.inner()).await
}

#[tauri::command]
pub async fn list_proposals(
    status: Option<String>,
    proposal_type: Option<String>,
    risk_level: Option<String>,
    limit: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<AgentProposal>, String> {
    reconcile_durable_proposal_projections_with_state(state.inner(), limit.clamp(1, 200)).await?;
    let status_filter = status.and_then(|s| match s.as_str() {
        "pending" => Some(ProposalStatus::Pending),
        "accepted" => Some(ProposalStatus::Accepted),
        "rejected" => Some(ProposalStatus::Rejected),
        "edited" => Some(ProposalStatus::Edited),
        "postponed" => Some(ProposalStatus::Postponed),
        "expired" => Some(ProposalStatus::Expired),
        _ => None,
    });

    let type_filter = proposal_type.and_then(|t| match t.as_str() {
        "life_model_update" => Some(ProposalType::LifeModelUpdate),
        "goal_update" => Some(ProposalType::GoalUpdate),
        "state_update" => Some(ProposalType::StateUpdate),
        "preference_update" => Some(ProposalType::PreferenceUpdate),
        "capability_update" => Some(ProposalType::CapabilityUpdate),
        "memory_write" => Some(ProposalType::MemoryWrite),
        "memory_archive" => Some(ProposalType::MemoryArchive),
        "tool_permission" => Some(ProposalType::ToolPermission),
        "plugin_permission" => Some(ProposalType::PluginPermission),
        "scheduled_task" => Some(ProposalType::ScheduledTask),
        "external_write_action" => Some(ProposalType::ExternalWriteAction),
        "model_policy_change" => Some(ProposalType::ModelPolicyChange),
        "data_export" => Some(ProposalType::DataExport),
        "schedule_checkin" => Some(ProposalType::ScheduleCheckin),
        "unsupported" => Some(ProposalType::Unsupported),
        _ => None,
    });

    let risk_filter = risk_level.and_then(|r| match r.as_str() {
        "low" => Some(RiskLevel::Low),
        "medium" => Some(RiskLevel::Medium),
        "high" => Some(RiskLevel::High),
        "critical" => Some(RiskLevel::Critical),
        _ => None,
    });

    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;
    store
        .list_proposals_filtered(status_filter, type_filter, risk_filter, limit.clamp(1, 200))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn batch_accept_low_risk_proposals(
    proposal_ids: Option<Vec<String>>,
    state: State<'_, Arc<AppState>>,
) -> Result<i64, String> {
    require_persistence_write(state.inner())?;
    check_safe_mode(state.inner())?;
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(proposal_store_missing)?;
    let store = store.lock().await;

    // If specific IDs provided, use those; otherwise fall back to all low-risk pending
    let proposals = if let Some(ids) = proposal_ids {
        let mut proposals = Vec::new();
        for id in ids {
            if let Ok(Some(p)) = store.get_proposal(&id) {
                if p.status == ProposalStatus::Pending
                    && p.risk_level == RiskLevel::Low
                    && !proposal_requires_native_confirmation(&p)
                {
                    proposals.push(p);
                }
            }
        }
        proposals
    } else {
        store
            .list_proposals_filtered(
                Some(ProposalStatus::Pending),
                None,
                Some(RiskLevel::Low),
                200,
            )
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|proposal| !proposal_requires_native_confirmation(proposal))
            .collect()
    };
    drop(store);

    let mut accepted_count = 0i64;
    for proposal in proposals {
        match accept_proposal_with_state_and_confirmation(proposal.id.clone(), state.inner(), None)
            .await
        {
            Ok(_) => accepted_count += 1,
            Err(e) => eprintln!("Batch accept failed for proposal {}: {}", proposal.id, e),
        }
    }

    Ok(accepted_count)
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
                | ProposalType::PluginPermission
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
    check_safe_mode(state.inner())?;
    let proposal = get_proposal_with_state(state.inner(), &proposal_id).await?;
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

#[tauri::command]
pub async fn edit_proposal(
    proposal_id: String,
    new_after: Value,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    edit_proposal_with_state(proposal_id, new_after, state.inner()).await
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

#[tauri::command]
pub async fn list_memory_assets(
    scope: Option<String>,
    status: Option<String>,
    limit: i64,
    offset: i64,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<MemoryLifecycleRecord>, String> {
    list_memory_assets_with_state(scope, status, limit, offset, state.inner()).await
}

#[tauri::command]
pub async fn get_memory_asset(
    memory_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<MemoryLifecycleRecord, String> {
    get_memory_asset_with_state(memory_id, state.inner()).await
}

#[tauri::command]
pub async fn get_memory_lifecycle_events(
    memory_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    get_memory_lifecycle_events_with_state(memory_id, state.inner()).await
}

#[tauri::command]
pub async fn rebuild_memory_materialized_view(
    scope: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    rebuild_memory_materialized_view_with_state(scope, state.inner()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{a2a_sidecar::A2ASidecar, HotMemoryCache, PrivacyEngine, SharedHotCache};
    use openlife_core::{
        agent::{
            AgentProposal, AgentRun, AgentRunStatus, AgentRunStore, EvidenceDraft,
            EvidencePrivacyLevel, EvidenceQuery, EvidenceRecord, EvidenceSourceRef,
            EvidenceSourceType, EvidenceType, MemoryCandidateKind, ProposalSource, ProposalStore,
            ProposalType, RiskLevel,
        },
        builder::BuilderSessionStore,
        config::AppConfig,
        feedback::FeedbackStore,
        life_model::{patch::PatchSource, LifeModelManager},
        mcp::McpRegistry,
        mcp_audit::McpAuditStore,
        memory::MemoryStore,
        scheduler::InferenceScheduler,
        vectors::VectorStore,
        versioning::VersionManager,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    fn reviewed_memory_after(
        session_id: &str,
        content: &str,
        candidate_kind: MemoryCandidateKind,
        risk_level: &str,
        sensitivity: &str,
    ) -> Value {
        serde_json::json!({
            "session_id": session_id,
            "content": content,
            "scope": "global",
            "category": openlife_core::agent::memory_lifecycle_category_for_candidate_kind(
                candidate_kind
            ),
            "candidateKind": candidate_kind,
            "riskLevel": risk_level,
            "sensitivity": sensitivity,
            "source": "review_center",
        })
    }

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = AppConfig::default();
        let hot_cache: SharedHotCache =
            Arc::new(tokio::sync::RwLock::new(HotMemoryCache::default()));
        Arc::new(AppState {
            persistence_coordinator: Arc::new(
                crate::persistence_coordinator::PersistenceCoordinator::isolated_evaluation(),
            ),
            governed_data_import_journal: None,
            config: Arc::new(Mutex::new(config.clone())),
            life_model_manager: Arc::new(Mutex::new(LifeModelManager::new(
                temp_dir.path().join("life-model").join("current"),
            ))),
            life_model_write_coordinator: Arc::new(Mutex::new(())),
            memory_store: Arc::new(Mutex::new(MemoryStore::new_in_memory().unwrap())),
            mcp_registry: Arc::new(Mutex::new(McpRegistry::new())),
            scheduler: Arc::new(Mutex::new(InferenceScheduler::new(
                config.local_model.clone(),
                config.prefer_local_model,
                config.llm.provider.clone(),
                config.llm.openai_base.clone(),
                config.llm.openai_key.clone(),
                config.llm.chat_model.clone(),
                config.llm.embedding_model.clone(),
                config.llm.embedding_enabled,
            ))),
            privacy_engine: Arc::new(Mutex::new(PrivacyEngine::new())),
            version_manager: Arc::new(Mutex::new(VersionManager::new(
                temp_dir.path().join("life-model").join("versions"),
            ))),
            feedback_store: Arc::new(Mutex::new(FeedbackStore::new_in_memory().unwrap())),
            vector_store: Arc::new(Mutex::new(VectorStore::new_in_memory().unwrap())),
            vector_persistence_mode: crate::state::VectorPersistenceMode::Enabled,
            builder_session_store: Arc::new(Mutex::new(BuilderSessionStore::new(
                temp_dir.path().join("builder_sessions.json"),
            ))),
            a2a_sidecar: Arc::new(Mutex::new(A2ASidecar::new(
                crate::a2a_server::configured_a2a_port(),
            ))),
            last_snapshot_date: Arc::new(Mutex::new(None)),
            mcp_audit_store: Arc::new(Mutex::new(McpAuditStore::new(
                temp_dir.path().join("mcp_audit.db"),
            ))),
            agent_run_store: Some(Arc::new(Mutex::new(
                AgentRunStore::new_in_memory().unwrap(),
            ))),
            evidence_store: Arc::new(Mutex::new(
                openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
            )),
            life_event_store: Some(Arc::new(Mutex::new(
                openlife_core::agent::LifeEventStore::new_in_memory().unwrap(),
            ))),
            heuristic_store: Arc::new(Mutex::new({
                let store = openlife_core::agent::HeuristicStore::new_in_memory().unwrap();
                store.seed_mvp_heuristics().unwrap();
                store
            })),
            policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
            proposal_store: Some(Arc::new(Mutex::new(
                ProposalStore::new_in_memory().unwrap(),
            ))),
            memory_lifecycle_store: Some(Arc::new(Mutex::new(
                openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap(),
            ))),
            plan_execute_session_store: Some(Arc::new(Mutex::new(
                openlife_core::agent::PlanExecuteSessionStore::new_in_memory().unwrap(),
            ))),
            main_chat_agent_session_store: Some(Arc::new(Mutex::new(
                openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStore::new_in_memory()
                    .unwrap(),
            ))),
            main_chat_action_queue_store: Some(Arc::new(Mutex::new(
                openlife_core::agent::main_chat_agent_v1::ActionQueueStore::new_in_memory()
                    .unwrap(),
            ))),
            main_chat_agent_event_store: None,
            main_chat_selected_skill_ids: Arc::new(Mutex::new(std::collections::HashMap::new())),
            main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
            patch_store: Some(Arc::new(Mutex::new(
                openlife_core::life_model::patch_store::PatchStore::new_in_memory().unwrap(),
            ))),
            rollout_metrics_store: None,
            tool_permission_store: Arc::new(Mutex::new(
                openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            )),
            skill_registry: Arc::new(Mutex::new(openlife_core::skills::SkillRegistry::built_in())),
            plugin_registry: Arc::new(Mutex::new(openlife_core::plugins::PluginRegistry::new(
                temp_dir.path().join("plugins"),
            ))),
            hot_cache,
            startup_warnings: vec![],
            credential_bootstrap_snapshot: Default::default(),
            provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
            scheduled_task_store: Arc::new(
                openlife_core::tasks::TaskStore::new_in_memory().unwrap(),
            ),
            runtime_clock_source: Arc::new(tokio::sync::Mutex::new(
                crate::main_chat_runtime_facts::MainChatRuntimeClockSource::default(),
            )),
            web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            resource_runtime: None,
            state_store: None,
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
        })
    }

    async fn fake_cloud_embedding_endpoint() -> (String, Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cloud_call_count = Arc::new(AtomicUsize::new(0));
        let cloud_call_count_clone = cloud_call_count.clone();

        tokio::spawn(async move {
            loop {
                let accepted =
                    tokio::time::timeout(std::time::Duration::from_millis(750), listener.accept())
                        .await;
                let Ok(Ok((mut socket, _))) = accepted else {
                    break;
                };
                cloud_call_count_clone.fetch_add(1, Ordering::SeqCst);
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 2048];
                let _ = socket.read(&mut buf).await;
                let body = r#"{"data":[{"embedding":[0.1,0.2,0.3]}]}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });

        (format!("http://{}", addr), cloud_call_count)
    }

    async fn configure_cloud_embeddings(state: &Arc<AppState>, openai_base: String) {
        let mut cfg = state.config.lock().await;
        cfg.llm.provider = "openai".to_string();
        cfg.llm.openai_base = openai_base;
        cfg.llm.openai_key = "sk-test".to_string();
        cfg.llm.embedding_model = "text-embedding-3-small".to_string();
        cfg.llm.embedding_enabled = true;
    }

    async fn stamp_lifemodel_base_hash(proposal: &mut AgentProposal, state: &Arc<AppState>) {
        crate::life_model_write_gateway::stamp_lifemodel_proposal_base_hash_with_state(
            state, proposal,
        )
        .await
        .unwrap();
    }

    async fn create_maturation_source_evidence(
        state: &Arc<AppState>,
        proposal: &AgentProposal,
    ) -> String {
        state
            .evidence_store
            .lock()
            .await
            .create_evidence(
                EvidenceDraft::new(
                    EvidenceType::Preference,
                    proposal.affected_path.clone(),
                    proposal.confidence,
                    proposal.risk_level,
                    EvidencePrivacyLevel::Internal,
                )
                .with_summary("maturation candidate source evidence")
                .with_source_ref(EvidenceSourceRef::from_digest(
                    EvidenceSourceType::AgentRun,
                    proposal.run_id.as_deref().unwrap_or("run-tauri-w75"),
                    Some("maturation_candidate"),
                    "candidate-digest-only",
                ))
                .with_linked_proposal(proposal.id.clone())
                .with_linked_agent_run(proposal.run_id.as_deref().unwrap_or("run-tauri-w75")),
            )
            .unwrap()
            .id
    }

    async fn proposal_outcome_records(
        state: &Arc<AppState>,
        proposal_id: &str,
    ) -> Vec<EvidenceRecord> {
        state
            .evidence_store
            .lock()
            .await
            .query(EvidenceQuery {
                evidence_type: Some(EvidenceType::ProposalOutcome),
                linked_proposal_id: Some(proposal_id.to_string()),
                ..Default::default()
            })
            .unwrap()
    }

    fn assert_no_w75_raw_content(serialized: &str) {
        for raw in [
            "RAW_PROMPT_SECRET",
            "RAW_ASSISTANT_OUTPUT_SECRET",
            "RAW_MEMORY_TEXT_SECRET",
            "RAW_TOOL_PAYLOAD_SECRET",
            "RAW_EDITED_PAYLOAD_SECRET",
            "unredacted reviewer note",
        ] {
            assert!(
                !serialized.contains(raw),
                "serialized W75 evidence leaked raw marker {raw}: {serialized}"
            );
        }
    }

    fn test_lifemodel_source_proposal(source: ProposalSource) -> AgentProposal {
        AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("W88 mapped name"),
            "W88 source mapping fixture",
            0.9,
            RiskLevel::Low,
            source,
        )
    }

    fn extract_rust_function_body(source: &str, signature: &str) -> String {
        let start = source
            .find(signature)
            .or_else(|| {
                signature
                    .strip_suffix('(')
                    .and_then(|prefix| source.find(&format!("{prefix}<")))
            })
            .unwrap_or_else(|| panic!("missing function signature {signature}"));
        let body_start = source[start..]
            .find('{')
            .map(|offset| start + offset)
            .unwrap_or_else(|| panic!("missing function body for {signature}"));
        let mut depth = 0usize;
        let mut end = body_start;
        for (offset, ch) in source[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = body_start + offset + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        source[body_start..end].to_string()
    }

    async fn accept_lifemodel_proposal_and_patch_source(
        source: ProposalSource,
    ) -> (PatchSource, serde_json::Value) {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = test_lifemodel_source_proposal(source);
        stamp_lifemodel_base_hash(&mut proposal, &state).await;
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let result = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let patches = state
            .patch_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_patches_by_proposal(&proposal_id)
            .unwrap();
        assert_eq!(patches.len(), 1);
        (patches[0].source, result)
    }

    #[test]
    fn privileged_or_high_risk_proposals_require_native_confirmation() {
        let high_risk = AgentProposal::new(
            ProposalType::GoalUpdate,
            "goals.long_term",
            serde_json::json!([{"description": "bounded test"}]),
            "test",
            0.9,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        assert!(proposal_requires_native_confirmation(&high_risk));

        let privileged_low = AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.test",
            serde_json::json!({
                "permission": "allow_once",
                "tool_name": "test",
                "source": "test",
                "risk_level": "low",
                "action_type": "network"
            }),
            "test",
            0.9,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        assert!(proposal_requires_native_confirmation(&privileged_low));

        let ordinary_medium = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.candidates",
            serde_json::json!({"content": "bounded test"}),
            "test",
            0.9,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        assert!(!proposal_requires_native_confirmation(&ordinary_medium));
    }

    #[test]
    fn native_proposal_confirmation_digest_changes_with_effect_snapshot() {
        let mut proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            "external.write",
            serde_json::json!({"target": "first"}),
            "test",
            0.9,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        let before = proposal_native_confirmation_digest(&proposal);
        proposal.after = serde_json::json!({"target": "second"});
        let after = proposal_native_confirmation_digest(&proposal);
        assert_ne!(before, after);
    }

    #[tokio::test]
    async fn changed_snapshot_after_native_confirmation_fails_before_effect_dispatch() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "tasks.scheduled",
            serde_json::json!({"title": "must not be scheduled"}),
            "test",
            0.9,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let error = accept_proposal_with_state_and_confirmation(
            proposal_id.clone(),
            &state,
            Some("sha256:stale-native-confirmation-snapshot"),
        )
        .await
        .unwrap_err();
        assert!(error.contains("changed after native confirmation"));
        let store = state.proposal_store.as_ref().unwrap().lock().await;
        assert_eq!(
            store.dispatch_state(&proposal_id).unwrap().as_deref(),
            Some("failed_before_effect")
        );
        assert_eq!(
            store.get_proposal(&proposal_id).unwrap().unwrap().status,
            ProposalStatus::Pending
        );
    }

    #[test]
    fn accept_proposal_typed_ipc_response_serializes_the_frontend_contract() {
        let typed = typed_accept_proposal_response(serde_json::json!({
            "success": true,
            "patch_result": {
                "patchId": "patch-1",
                "success": true,
                "path": "identity.name",
                "operation": "replace",
                "error": null
            },
            "effect_status": "confirmed",
            "proposal_projection_status": "reconciliation_required",
            "warnings": ["projection pending"],
            "memoryPersistence": {
                "canonicalCommitted": true,
                "outboxEventId": "event-1",
                "projectionState": "degraded",
                "pending": 0,
                "degraded": 1,
                "applied": 0,
                "reasonCode": "projection_delivery_failed",
                "errorDigest": "sha256:deadbeef"
            },
            "artifactMaterialization": {
                "artifactId": "artifact:proposal-1",
                "proposalId": "proposal-1",
                "targetReference": "/safe/roadshow-summary.md",
                "targetReferenceDigest": "sha256:target",
                "contentDigest": "sha256:content",
                "observedContentDigest": "sha256:content",
                "byteSize": 42,
                "mediaType": "text/markdown; charset=utf-8",
                "status": "confirmed"
            }
        }))
        .unwrap();
        let serialized = serde_json::to_value(typed).unwrap();
        assert!(serialized.get("patchResult").is_some());
        assert!(serialized.get("patch_result").is_none());
        assert_eq!(serialized["effectStatus"], "confirmed");
        assert_eq!(
            serialized["proposalProjectionStatus"],
            "reconciliation_required"
        );
        assert_eq!(serialized["memoryPersistence"]["canonicalCommitted"], true);
        assert_eq!(
            serialized["memoryPersistence"]["projectionState"],
            "degraded"
        );
        assert_eq!(
            serialized["memoryPersistence"]["reasonCode"],
            "projection_delivery_failed"
        );
        assert_eq!(
            serialized["artifactMaterialization"]["targetReference"],
            "/safe/roadshow-summary.md"
        );
        assert_eq!(
            serialized["artifactMaterialization"]["contentDigest"],
            serialized["artifactMaterialization"]["observedContentDigest"]
        );
    }

    #[test]
    fn accept_proposal_ipc_contract_models_terminal_owner_memory_confirmation() {
        let typed = typed_accept_proposal_response(serde_json::json!({
            "success": true,
            "effect_status": "confirmed",
            "proposal_projection_status": "confirmed",
            "proposalId": "proposal-memory-terminal-owner",
            "terminalOwnerTransition": {
                "beforeOwnerRevision": 4,
                "afterOwnerRevision": 5,
                "successorEventId": "successor-1"
            }
        }))
        .expect("terminal-owner Memory acceptance must satisfy the shipped IPC contract");
        let serialized = serde_json::to_value(typed).unwrap();
        assert_eq!(serialized["proposalId"], "proposal-memory-terminal-owner");
        assert_eq!(serialized["effectStatus"], "confirmed");
        assert_eq!(
            serialized["terminalOwnerTransition"]["afterOwnerRevision"],
            5
        );
        assert!(serialized.get("patchResult").is_none());
    }

    #[test]
    fn accept_proposal_ipc_contract_models_deferred_terminal_owner_response() {
        let typed = typed_accept_proposal_response(serde_json::json!({
            "success": false,
            "status": "deferred",
            "reasonCode": "origin_turn_open",
            "proposalId": "proposal-deferred",
            "dispatchState": "unclaimed",
            "durableWriteExecuted": false
        }))
        .expect("a safely deferred acceptance must satisfy the shipped IPC contract");
        let serialized = serde_json::to_value(typed).unwrap();
        assert_eq!(serialized["status"], "deferred");
        assert_eq!(serialized["reasonCode"], "origin_turn_open");
        assert_eq!(serialized["durableWriteExecuted"], false);
        assert!(serialized.get("effectStatus").is_none());
    }

    #[test]
    fn accept_proposal_ipc_contract_rejects_mixed_confirmed_and_deferred_truth() {
        let confirmed_error = typed_accept_proposal_response(serde_json::json!({
            "success": true,
            "effectStatus": "confirmed",
            "proposalProjectionStatus": "confirmed",
            "proposalId": "proposal-confirmed",
            "status": "deferred",
            "reasonCode": "origin_turn_open",
            "dispatchState": "unclaimed",
            "durableWriteExecuted": false
        }))
        .expect_err("confirmed responses must not carry deferred-only truth");
        assert!(confirmed_error.contains("deferred-only truth fields"));

        let deferred_error = typed_accept_proposal_response(serde_json::json!({
            "success": false,
            "status": "deferred",
            "reasonCode": "origin_turn_open",
            "proposalId": "proposal-deferred",
            "dispatchState": "unclaimed",
            "durableWriteExecuted": false,
            "effectStatus": "confirmed",
            "proposalProjectionStatus": "confirmed"
        }))
        .expect_err("deferred responses must not carry confirmed-effect truth");
        assert!(deferred_error.contains("confirmed-effect truth fields"));
    }

    #[test]
    fn accept_proposal_ipc_contract_rejects_unmodeled_truth_fields() {
        let error = typed_accept_proposal_response(serde_json::json!({
            "success": true,
            "patchResult": {
                "patchId": "patch-unknown",
                "success": true,
                "path": "memory.preference",
                "operation": "memory_write_projection_degraded",
                "error": null
            },
            "effectStatus": "confirmed",
            "proposalProjectionStatus": "confirmed",
            "warnings": [],
            "unmodeledProjectionTruth": "must_not_be_silently_dropped"
        }))
        .expect_err("typed IPC must fail closed instead of deleting a new fact");
        assert!(error.contains("unknown field"));
    }

    #[test]
    fn accept_proposal_ipc_contract_rejects_nonconfirmed_artifact_receipt() {
        let error = typed_accept_proposal_response(serde_json::json!({
            "success": true,
            "patchResult": {
                "patchId": "patch-artifact-unknown",
                "success": true,
                "path": "filesystem.safe/artifact.md",
                "operation": "artifact_materialization",
                "error": null
            },
            "effectStatus": "confirmed",
            "proposalProjectionStatus": "confirmed",
            "warnings": [],
            "artifactMaterialization": {
                "artifactId": "artifact:proposal-unknown",
                "proposalId": "proposal-unknown",
                "targetReference": "/safe/artifact.md",
                "targetReferenceDigest": "sha256:target",
                "contentDigest": "sha256:content",
                "observedContentDigest": "sha256:content",
                "byteSize": 42,
                "mediaType": "text/markdown; charset=utf-8",
                "status": "unknown"
            }
        }))
        .expect_err("ArtifactMaterializationReceipt can represent confirmed truth only");
        assert!(error.contains("unknown variant"), "{error}");
    }

    #[test]
    fn external_write_action_old_direct_file_route_stays_absent() {
        let source = include_str!("proposal.rs");
        let retired_writer = ["safe_", "write_", "utf8"].concat();
        assert!(!source.contains(&retired_writer));
        let generic_apply = extract_rust_function_body(source, "async fn apply_proposal_to_state(");
        assert!(generic_apply
            .contains("ExternalWriteAction must execute through ArtifactMaterializer."));
        let review_acceptance = extract_rust_function_body(
            source,
            "async fn accept_proposal_with_state_and_confirmation(",
        );
        assert!(review_acceptance.contains("apply_external_write_artifact(state, &proposal"));
    }

    #[test]
    fn w88_lifemodel_proposal_patch_source_mapping_is_source_specific() {
        for (source, expected) in [
            (ProposalSource::BuilderReview, PatchSource::BuilderReview),
            (ProposalSource::CalibrationRun, PatchSource::Calibration),
            (ProposalSource::FeedbackEvolution, PatchSource::Evolution),
            (ProposalSource::Manual, PatchSource::Manual),
        ] {
            let proposal = test_lifemodel_source_proposal(source);
            let report = evaluate_lifemodel_proposal_patch_source_mapping(&proposal);
            assert_eq!(report.proposal_source, source);
            assert_eq!(report.patch_source, expected);
            assert!(report.exact_source_mapping);
            assert!(!report.metadata_safe_fallback);
            assert!(report.blocking_reasons.is_empty());
            assert_eq!(
                ensure_lifemodel_proposal_patch_source_mapping(&proposal)
                    .unwrap()
                    .patch_source,
                expected
            );
            assert_eq!(
                resolve_lifemodel_patch_source_for_proposal(&proposal),
                expected
            );
        }
    }

    #[test]
    fn w95_lifemodel_proposal_sources_have_exact_patch_source_mappings() {
        for (source, expected) in [
            (
                ProposalSource::ChatConversation,
                PatchSource::ChatConversation,
            ),
            (ProposalSource::ProactiveAgent, PatchSource::ProactiveAgent),
            (ProposalSource::SkillRuntime, PatchSource::SkillRuntime),
            (ProposalSource::Plugin, PatchSource::Plugin),
            (
                ProposalSource::MemoryGovernance,
                PatchSource::MemoryGovernance,
            ),
            (
                ProposalSource::PlanningSession,
                PatchSource::PlanningSession,
            ),
        ] {
            let proposal = test_lifemodel_source_proposal(source);
            let report = evaluate_lifemodel_proposal_patch_source_mapping(&proposal);
            assert_eq!(report.proposal_source, source);
            assert_eq!(report.patch_source, expected);
            assert_ne!(report.patch_source, PatchSource::BuilderReview);
            assert!(report.exact_source_mapping);
            assert!(!report.metadata_safe_fallback);
            assert!(report.apply_allowed);
            assert!(report.metadata_safe);
            assert!(report.default_chat_route_unchanged);
            assert!(report.proposal_first_convergence_complete);
            assert!(report.blocking_reasons.is_empty());
            assert_eq!(report.required_follow_up, "none");
            assert_eq!(
                ensure_lifemodel_proposal_patch_source_mapping(&proposal)
                    .unwrap()
                    .patch_source,
                expected
            );
        }
    }

    #[tokio::test]
    async fn w88_accept_lifemodel_proposal_writes_source_specific_patch_store_source() {
        for (source, expected) in [
            (ProposalSource::BuilderReview, PatchSource::BuilderReview),
            (ProposalSource::CalibrationRun, PatchSource::Calibration),
            (ProposalSource::FeedbackEvolution, PatchSource::Evolution),
            (ProposalSource::Manual, PatchSource::Manual),
        ] {
            let (actual, result) = accept_lifemodel_proposal_and_patch_source(source).await;
            assert_eq!(result["success"], true);
            assert_eq!(actual, expected, "{source} must map to {expected}");
        }
    }

    #[tokio::test]
    async fn w95_non_typical_lifemodel_proposal_sources_write_dedicated_patch_source() {
        for (source, expected) in [
            (
                ProposalSource::ChatConversation,
                PatchSource::ChatConversation,
            ),
            (ProposalSource::ProactiveAgent, PatchSource::ProactiveAgent),
            (ProposalSource::SkillRuntime, PatchSource::SkillRuntime),
            (ProposalSource::Plugin, PatchSource::Plugin),
            (
                ProposalSource::MemoryGovernance,
                PatchSource::MemoryGovernance,
            ),
            (
                ProposalSource::PlanningSession,
                PatchSource::PlanningSession,
            ),
        ] {
            let (actual, result) = accept_lifemodel_proposal_and_patch_source(source).await;
            assert_eq!(result["success"], true);
            assert_ne!(
                actual,
                PatchSource::BuilderReview,
                "{source} must not be masqueraded as BuilderReview"
            );
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn w88_apply_proposal_to_state_no_longer_hardcodes_builder_review_patch_source() {
        let source = std::fs::read_to_string(format!(
            "{}/src/commands/proposal.rs",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("read proposal.rs");
        let body = extract_rust_function_body(&source, "async fn apply_proposal_to_state(");
        assert!(
            !body.contains("PatchSource::BuilderReview"),
            "apply_proposal_to_state must use the W88 source-specific resolver, not a hardcoded BuilderReview PatchSource"
        );
        assert!(body.contains("resolve_lifemodel_patch_source_for_proposal"));
    }

    #[test]
    fn w88_lifemodel_proposal_patch_source_mapping_report_is_metadata_safe() {
        let mut proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            "state.current_focus",
            serde_json::json!({
                "raw": "W88_RAW_LIFEMODEL_PATCH_VALUE_SECRET",
                "memory": "W88_RAW_MEMORY_TEXT_SECRET",
                "chat": "W88_RAW_CHAT_TEXT_SECRET",
                "tool": "W88_RAW_TOOL_PAYLOAD_SECRET"
            }),
            "W88_RAW_PROPOSAL_REASON_SECRET",
            0.8,
            RiskLevel::Low,
            ProposalSource::ChatConversation,
        );
        proposal.before = Some(serde_json::json!("W88_RAW_BEFORE_VALUE_SECRET"));
        proposal.source_detail = Some("W88_RAW_SOURCE_DETAIL_SECRET".into());

        let report = evaluate_lifemodel_proposal_patch_source_mapping(&proposal);
        assert!(report.metadata_safe);
        assert!(!report.contains_raw_proposal_payload);
        assert!(!report.contains_raw_lifemodel_patch_value);
        assert!(!report.contains_raw_memory_text);
        assert!(!report.contains_raw_chat_text);
        assert!(!report.contains_raw_tool_payload);

        let debug_dump = format!("{report:?}");
        for forbidden in [
            "W88_RAW_LIFEMODEL_PATCH_VALUE_SECRET",
            "W88_RAW_MEMORY_TEXT_SECRET",
            "W88_RAW_CHAT_TEXT_SECRET",
            "W88_RAW_TOOL_PAYLOAD_SECRET",
            "W88_RAW_PROPOSAL_REASON_SECRET",
            "W88_RAW_BEFORE_VALUE_SECRET",
            "W88_RAW_SOURCE_DETAIL_SECRET",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "W88 source mapping report leaked raw marker {forbidden}"
            );
        }
    }

    #[test]
    fn w88_lifemodel_proposal_patch_source_mapping_keeps_default_chat_kernel_path() {
        let report = evaluate_lifemodel_proposal_patch_source_mapping(
            &test_lifemodel_source_proposal(ProposalSource::ChatConversation),
        );
        assert!(report.default_chat_route_unchanged);
        assert_eq!(report.default_chat_route, "main_chat_kernel");
        assert!(!report.default_chat_entrypoints_changed);
    }

    fn w89_source_bodies() -> (String, String, String) {
        let proposal_rs_path = format!("{}/src/commands/proposal.rs", env!("CARGO_MANIFEST_DIR"));
        let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
        let proposal_source = std::fs::read_to_string(proposal_rs_path).expect("read proposal.rs");
        let lib_source = std::fs::read_to_string(lib_rs_path).expect("read lib.rs");
        (
            extract_rust_function_body(&proposal_source, "async fn apply_proposal_to_state("),
            extract_rust_function_body(&lib_source, "async fn send_message("),
            extract_rust_function_body(&lib_source, "async fn start_stream_message("),
        )
    }

    fn w89_entry(
        entries: &[LifeModelProposalPatchSourceReadinessEntry],
        source: ProposalSource,
    ) -> &LifeModelProposalPatchSourceReadinessEntry {
        entries
            .iter()
            .find(|entry| entry.proposal_source == source)
            .unwrap_or_else(|| panic!("missing W89 readiness entry for {source}"))
    }

    #[test]
    fn w95_lifemodel_proposal_patch_source_readiness_report_covers_all_exact_sources() {
        let (apply_body, send_body, stream_body) = w89_source_bodies();
        let report = evaluate_lifemodel_proposal_patch_source_readiness(
            &apply_body,
            &send_body,
            &stream_body,
        );

        assert!(report.readiness_ready);
        assert!(report.metadata_safe);
        assert_eq!(report.exact_mapping_count, 10);
        assert_eq!(report.metadata_safe_fallback_count, 0);
        assert_eq!(report.unsupported_or_unclassified_count, 0);
        assert!(report.builder_review_only_for_builder_review);
        assert!(report.no_hardcoded_builder_review_in_apply_path);
        assert!(report.apply_path_uses_mapping_ensure);
        assert!(report.apply_path_uses_source_resolver);
        assert!(report.default_chat_route_unchanged);
        assert!(report.proposal_first_convergence_complete);
        assert_eq!(report.entries.len(), 10);
        assert!(report.blocking_reasons.is_empty());

        for (source, patch_source) in [
            (ProposalSource::BuilderReview, PatchSource::BuilderReview),
            (ProposalSource::CalibrationRun, PatchSource::Calibration),
            (ProposalSource::FeedbackEvolution, PatchSource::Evolution),
            (ProposalSource::Manual, PatchSource::Manual),
            (
                ProposalSource::ChatConversation,
                PatchSource::ChatConversation,
            ),
            (ProposalSource::ProactiveAgent, PatchSource::ProactiveAgent),
            (ProposalSource::SkillRuntime, PatchSource::SkillRuntime),
            (ProposalSource::Plugin, PatchSource::Plugin),
            (
                ProposalSource::MemoryGovernance,
                PatchSource::MemoryGovernance,
            ),
            (
                ProposalSource::PlanningSession,
                PatchSource::PlanningSession,
            ),
        ] {
            let entry = w89_entry(&report.entries, source);
            assert_eq!(entry.patch_source, patch_source);
            assert!(entry.exact_source_mapping);
            assert!(!entry.metadata_safe_fallback);
            assert_eq!(entry.follow_up, "none");
        }
    }

    #[test]
    fn w89_lifemodel_proposal_patch_source_readiness_report_is_metadata_safe() {
        let (apply_body, send_body, stream_body) = w89_source_bodies();
        let report = evaluate_lifemodel_proposal_patch_source_readiness(
            &format!("{apply_body} W89_RAW_PROPOSAL_PAYLOAD_SECRET"),
            &format!("{send_body} W89_RAW_CHAT_TEXT_SECRET"),
            &format!("{stream_body} W89_RAW_TOOL_PAYLOAD_SECRET"),
        );

        assert!(report.metadata_safe);
        assert!(!report.contains_raw_proposal_payload);
        assert!(!report.contains_raw_lifemodel_patch_value);
        assert!(!report.contains_raw_memory_text);
        assert!(!report.contains_raw_chat_text);
        assert!(!report.contains_raw_tool_payload);

        let debug_dump = format!("{report:?}");
        for forbidden in [
            "W89_RAW_PROPOSAL_PAYLOAD_SECRET",
            "W89_RAW_LIFEMODEL_PATCH_VALUE_SECRET",
            "W89_RAW_MEMORY_TEXT_SECRET",
            "W89_RAW_CHAT_TEXT_SECRET",
            "W89_RAW_TOOL_PAYLOAD_SECRET",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "W89 readiness report leaked raw marker {forbidden}"
            );
        }
    }

    #[test]
    fn w89_apply_path_readiness_scanner_proves_mapping_ensure_and_resolver_use() {
        let (apply_body, send_body, stream_body) = w89_source_bodies();
        let report = evaluate_lifemodel_proposal_patch_source_readiness(
            &apply_body,
            &send_body,
            &stream_body,
        );

        assert!(report.no_hardcoded_builder_review_in_apply_path);
        assert!(report.apply_path_uses_mapping_ensure);
        assert!(report.apply_path_uses_source_resolver);
        assert!(apply_body.contains("ensure_lifemodel_proposal_patch_source_mapping(proposal)"));
        assert!(apply_body.contains("resolve_lifemodel_patch_source_for_proposal(proposal)"));
        assert!(apply_body.contains("LifeModelPatch::from_proposal"));
        assert!(!apply_body.contains("PatchSource::BuilderReview"));
    }

    #[test]
    fn w89_default_chat_entrypoints_do_not_call_patch_source_mapping_or_readiness_helpers() {
        let (apply_body, send_body, stream_body) = w89_source_bodies();
        let report = evaluate_lifemodel_proposal_patch_source_readiness(
            &apply_body,
            &send_body,
            &stream_body,
        );

        assert!(report.default_chat_route_unchanged);
        for forbidden in [
            "LifeModelProposalPatchSourceMappingReport",
            "evaluate_lifemodel_proposal_patch_source_mapping",
            "ensure_lifemodel_proposal_patch_source_mapping",
            "resolve_lifemodel_patch_source_for_proposal",
            "LifeModelProposalPatchSourceReadinessReport",
            "evaluate_lifemodel_proposal_patch_source_readiness",
            "ensure_lifemodel_proposal_patch_source_readiness",
        ] {
            assert!(
                !send_body.contains(forbidden),
                "send_message must not call proposal PatchSource helper {forbidden}"
            );
            assert!(
                !stream_body.contains(forbidden),
                "start_stream_message must not call proposal PatchSource helper {forbidden}"
            );
        }
    }

    #[test]
    fn w95_readiness_ensure_passes_after_patch_source_fallback_closure() {
        let (apply_body, send_body, stream_body) = w89_source_bodies();
        let report =
            ensure_lifemodel_proposal_patch_source_readiness(&apply_body, &send_body, &stream_body)
                .expect("W95 closes proposal PatchSource fallback policy");

        assert!(report.readiness_ready);
        assert!(report.proposal_first_convergence_complete);
        assert!(report.blocking_reasons.is_empty());
    }

    #[test]
    fn lifemodel_closed_loop_canonicalizes_communication_style_aliases() {
        for alias in [
            "/preferences/communication_style",
            "preferences.communication_style",
            "preferences.communication",
            "/preferences/communication",
        ] {
            assert_eq!(
                canonical_lifemodel_path(alias),
                COMMUNICATION_STYLE_CANONICAL_PATH
            );
            assert!(is_communication_style_lifemodel_path(alias));
        }
        assert_eq!(canonical_lifemodel_path("identity.name"), "identity.name");
    }

    #[tokio::test]
    async fn proposal_accept_normalizes_communication_style_alias_before_apply() {
        for alias in [
            "/preferences/communication_style",
            "preferences.communication_style",
            "preferences.communication",
            "/preferences/communication",
        ] {
            let temp_dir = tempfile::tempdir().unwrap();
            let state = test_app_state(&temp_dir);
            let mut proposal = AgentProposal::new(
                ProposalType::PreferenceUpdate,
                alias,
                serde_json::json!(format!("accepted via {alias}")),
                "用户确认沟通偏好。",
                0.91,
                RiskLevel::Low,
                ProposalSource::FeedbackEvolution,
            );
            stamp_lifemodel_base_hash(&mut proposal, &state).await;
            let id = proposal.id.clone();
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .create_proposal(&proposal)
                .unwrap();

            let result = accept_proposal_with_state(id.clone(), &state)
                .await
                .unwrap();

            assert_eq!(
                result["patch_result"]["path"],
                serde_json::json!("/preferences/communication_style")
            );
            let model = state.life_model_manager.lock().await.load().unwrap();
            assert_eq!(
                model.preferences.communication_style,
                format!("accepted via {alias}")
            );
            let stored = state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_proposal(&id)
                .unwrap()
                .unwrap();
            assert_eq!(stored.affected_path, COMMUNICATION_STYLE_CANONICAL_PATH);
            let patches = state
                .patch_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_patches_by_proposal(&id)
                .unwrap();
            assert_eq!(patches.len(), 1);
            assert_eq!(patches[0].path_pointer, "/preferences/communication_style");
        }
    }

    #[tokio::test]
    async fn accept_life_model_proposal_updates_model_and_marks_accepted() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("Fujing"),
            "用户确认的新称呼",
            0.9,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        stamp_lifemodel_base_hash(&mut proposal, &state).await;
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(model.identity.name, "Fujing");
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
    }

    #[tokio::test]
    async fn accept_maturation_proposal_records_outcome_evidence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "preferences.communication_style",
            serde_json::json!("W132 accepted communication style"),
            "RAW_PROMPT_SECRET RAW_ASSISTANT_OUTPUT_SECRET unredacted reviewer note",
            0.9,
            RiskLevel::Low,
            ProposalSource::FeedbackEvolution,
        );
        proposal.run_id = Some("run-tauri-w75-accept".into());
        proposal.source_detail = Some("maturation:preference.communication".into());
        stamp_lifemodel_base_hash(&mut proposal, &state).await;
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let source_evidence_id = create_maturation_source_evidence(&state, &proposal).await;

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let records = proposal_outcome_records(&state, &proposal_id).await;
        assert_eq!(records.len(), 1);
        let evidence = &records[0];
        assert_eq!(evidence.run_metadata["outcome"], "accepted");
        assert_eq!(evidence.linked_proposal_ids, vec![proposal_id.clone()]);
        assert_eq!(evidence.linked_agent_run_ids, vec!["run-tauri-w75-accept"]);
        assert_eq!(
            evidence.run_metadata["sourceEvidenceIds"],
            serde_json::json!([source_evidence_id])
        );
        assert_no_w75_raw_content(&serde_json::to_string(evidence).unwrap());

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(
            model.preferences.communication_style,
            "W132 accepted communication style"
        );
    }

    #[tokio::test]
    async fn reject_maturation_proposal_records_negative_outcome_without_applying() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "preferences.communication_style",
            serde_json::json!("W132 rejected communication style should not apply"),
            "RAW_PROMPT_SECRET RAW_ASSISTANT_OUTPUT_SECRET",
            0.86,
            RiskLevel::Low,
            ProposalSource::FeedbackEvolution,
        );
        proposal.run_id = Some("run-tauri-w75-reject".into());
        proposal.source_detail = Some("maturation:preference.communication".into());
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let source_evidence_id = create_maturation_source_evidence(&state, &proposal).await;

        reject_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let records = proposal_outcome_records(&state, &proposal_id).await;
        assert_eq!(records.len(), 1);
        let evidence = &records[0];
        assert_eq!(evidence.run_metadata["outcome"], "rejected");
        assert_eq!(evidence.run_metadata["negative"], true);
        assert_eq!(evidence.run_metadata["opposing"], true);
        assert_eq!(evidence.opposing_refs, vec![source_evidence_id]);
        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_ne!(
            model.preferences.communication_style,
            "W132 rejected communication style should not apply"
        );
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Rejected);
    }

    #[tokio::test]
    async fn edit_life_model_proposal_applies_edited_value() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            "state.current_focus",
            serde_json::json!("旧焦点"),
            "用户状态更新",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let edit_result = edit_proposal_with_state(id.clone(), serde_json::json!("新焦点"), &state)
            .await
            .unwrap();
        assert_eq!(edit_result["status"], "edited_pending_review");
        assert_eq!(edit_result["durable_write_executed"], false);

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_ne!(model.state.current_focus, "新焦点");
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Edited);
        assert_eq!(stored.resolved_at, None);
    }

    #[tokio::test]
    async fn edit_maturation_proposal_records_outcome_without_raw_edited_payload() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "preferences.communication_style",
            serde_json::json!("before edit"),
            "RAW_PROMPT_SECRET RAW_ASSISTANT_OUTPUT_SECRET",
            0.82,
            RiskLevel::Low,
            ProposalSource::FeedbackEvolution,
        );
        proposal.run_id = Some("run-tauri-w75-edit".into());
        proposal.source_detail = Some("maturation:preference.communication".into());
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        create_maturation_source_evidence(&state, &proposal).await;

        let edit_result = edit_proposal_with_state(
            proposal_id.clone(),
            serde_json::json!("RAW_EDITED_PAYLOAD_SECRET"),
            &state,
        )
        .await
        .unwrap();
        assert_eq!(edit_result["status"], "edited_pending_review");

        let records = proposal_outcome_records(&state, &proposal_id).await;
        assert_eq!(records.len(), 1);
        let evidence = &records[0];
        assert_eq!(evidence.run_metadata["outcome"], "edited");
        assert_eq!(evidence.run_metadata["editedPayloadIncluded"], false);
        assert_no_w75_raw_content(&serde_json::to_string(evidence).unwrap());

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_ne!(
            model.preferences.communication_style,
            "RAW_EDITED_PAYLOAD_SECRET"
        );
    }

    #[tokio::test]
    async fn edit_proposal_does_not_write_lifemodel_until_later_accept() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "preferences.communication_style",
            serde_json::json!("original proposed style"),
            "User wants to edit before accepting.",
            0.82,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.before = Some(serde_json::json!(""));
        proposal.run_id = Some("run-edit-then-accept".into());
        stamp_lifemodel_base_hash(&mut proposal, &state).await;
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let edit_result = edit_proposal_with_state(
            proposal_id.clone(),
            serde_json::json!("edited style"),
            &state,
        )
        .await
        .unwrap();
        assert_eq!(edit_result["durable_write_executed"], false);
        let model_after_edit = state.life_model_manager.lock().await.load().unwrap();
        assert_ne!(
            model_after_edit.preferences.communication_style,
            "edited style"
        );

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let model_after_accept = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(
            model_after_accept.preferences.communication_style,
            "edited style"
        );
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
        assert!(stored.resolved_at.is_some());
    }

    #[tokio::test]
    async fn stale_lifemodel_proposal_base_hash_conflicts_without_accepting() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "preferences.communication_style",
            serde_json::json!("proposal style"),
            "This proposal was based on an older model.",
            0.82,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.before = Some(serde_json::json!(""));
        proposal.run_id = Some("run-stale-base".into());
        stamp_lifemodel_base_hash(&mut proposal, &state).await;
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        {
            let manager = state.life_model_manager.lock().await;
            let mut model = manager.load().unwrap();
            model.preferences.communication_style = "changed outside proposal".into();
            manager.save(&model).unwrap();
        }

        let err = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap_err();
        assert!(
            err.contains("accepted_proposal_base_hash_stale"),
            "stale accept must report gateway stale conflict: {err}"
        );
        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(
            model.preferences.communication_style,
            "changed outside proposal"
        );
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Pending);
    }

    #[tokio::test]
    async fn memory_gateway_materializes_food_preference_and_future_rule_lanes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let food = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            reviewed_memory_after(
                "lane-food",
                "午餐吃了沙拉，下午精力不错",
                MemoryCandidateKind::EpisodicLifeEvent,
                "low",
                "internal",
            ),
            "User accepted diet event memory.",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let food_id = food.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&food)
            .unwrap();
        let food_result = accept_proposal_with_state(food_id, &state).await.unwrap();
        assert_eq!(food_result["memoryGateway"]["lane"], "episodic_life_event");
        assert_eq!(
            food_result["memoryGateway"]["status"],
            "local_memory_written"
        );
        assert_eq!(food_result["memoryLifecycle"]["category"], "fact");

        let preference = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            reviewed_memory_after(
                "lane-preference",
                "User prefers concise status updates.",
                MemoryCandidateKind::Preference,
                "low",
                "internal",
            ),
            "User accepted preference memory.",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let preference_id = preference.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&preference)
            .unwrap();
        let preference_result = accept_proposal_with_state(preference_id, &state)
            .await
            .unwrap();
        assert_eq!(
            preference_result["memoryGateway"]["lane"],
            "semantic_fact_preference"
        );
        assert_eq!(
            preference_result["memoryLifecycle"]["category"],
            "preference"
        );

        let future_rule = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.rules.planning",
            reviewed_memory_after(
                "lane-rule",
                "以后做计划时，先安排最难的任务。",
                MemoryCandidateKind::ProceduralRule,
                "medium",
                "internal",
            ),
            "User accepted future planning rule.",
            0.8,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let review_decision = memory_gateway::memory_gateway_decision_for_proposal(
            &future_rule,
            "proposal_review_required",
            Vec::new(),
        );
        assert_eq!(review_decision.lane.as_str(), "procedural_rule");
        assert_eq!(review_decision.status.as_str(), "proposal_required");
        let future_rule_id = future_rule.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&future_rule)
            .unwrap();
        let rule_result = accept_proposal_with_state(future_rule_id, &state)
            .await
            .unwrap();
        assert_eq!(rule_result["memoryGateway"]["lane"], "procedural_rule");
        assert_eq!(rule_result["memoryLifecycle"]["category"], "workflow");
    }

    #[tokio::test]
    async fn accepted_lifemodel_proposal_audit_contains_gateway_hashes_and_lane() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "preferences.communication_style",
            serde_json::json!("audit style"),
            "User accepted communication style update.",
            0.82,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.before = Some(serde_json::json!(""));
        proposal.run_id = Some("run-audit-lifemodel".into());
        proposal.source_detail = Some("evidence:evidence-audit".into());
        stamp_lifemodel_base_hash(&mut proposal, &state).await;
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let details = state
            .feedback_store
            .lock()
            .await
            .analytics_details_for_event("lifemodel_gateway_materialized", 5)
            .unwrap();
        let detail = details
            .iter()
            .find_map(|detail| serde_json::from_str::<serde_json::Value>(detail).ok())
            .expect("lifemodel gateway audit detail");
        assert_eq!(detail["proposalId"], proposal_id);
        assert_eq!(detail["runId"], "run-audit-lifemodel");
        assert_eq!(detail["evidenceId"], "evidence-audit");
        assert_eq!(detail["lane"], "canonical_lifemodel_truth");
        assert!(detail["baseHash"].as_str().is_some_and(|v| !v.is_empty()));
        assert!(detail["currentHash"]
            .as_str()
            .is_some_and(|v| !v.is_empty()));
        assert!(detail["beforeHash"].as_str().is_some_and(|v| !v.is_empty()));
        assert!(detail["afterHash"].as_str().is_some_and(|v| !v.is_empty()));
        assert_eq!(detail["conflictStatus"], serde_json::Value::Null);
        assert_eq!(
            detail["reasonCode"],
            "accepted_proposal_materialization_allowed"
        );
    }

    #[tokio::test]
    async fn high_risk_identity_maturation_proposal_does_not_record_outcome_evidence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("W132 high-risk identity should not record outcome evidence"),
            "RAW_PROMPT_SECRET RAW_ASSISTANT_OUTPUT_SECRET",
            0.84,
            RiskLevel::Low,
            ProposalSource::FeedbackEvolution,
        );
        proposal.run_id = Some("run-tauri-w132-high-risk".into());
        proposal.source_detail = Some("maturation:preference.communication".into());
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        create_maturation_source_evidence(&state, &proposal).await;

        reject_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let records = proposal_outcome_records(&state, &proposal_id).await;
        assert_eq!(records.len(), 0);
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Rejected);
    }

    #[tokio::test]
    async fn unsupported_domain_maturation_proposal_does_not_record_outcome_evidence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::StateUpdate,
            "state.current_focus",
            serde_json::json!("W132 unsupported state update should not record outcome evidence"),
            "RAW_PROMPT_SECRET RAW_ASSISTANT_OUTPUT_SECRET",
            0.8,
            RiskLevel::Low,
            ProposalSource::FeedbackEvolution,
        );
        proposal.run_id = Some("run-tauri-w132-unsupported".into());
        proposal.source_detail = Some("maturation:state.current_focus".into());
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        create_maturation_source_evidence(&state, &proposal).await;

        reject_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let records = proposal_outcome_records(&state, &proposal_id).await;
        assert_eq!(records.len(), 0);
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Rejected);
    }

    #[tokio::test]
    async fn accept_memory_write_proposal_records_memory_without_life_model_patch() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            reviewed_memory_after(
                "proposal-session",
                "用户偏好早上做深度工作",
                MemoryCandidateKind::Preference,
                "low",
                "internal",
            ),
            "用户确认写入长期记忆",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let result = accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(result["success"], true);

        let hits = state
            .memory_store
            .lock()
            .await
            .search_text_memories(Some("proposal-session"), "深度工作", 10)
            .unwrap();
        assert_eq!(hits.len(), 1);

        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
    }

    #[tokio::test]
    async fn malformed_memory_review_contract_fails_before_dispatch_claim() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            serde_json::json!({
                "content": "This payload was never reviewed with typed governance metadata."
            }),
            "Malformed fixture must fail before dispatch.",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let error = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap_err();
        assert!(
            error.contains("reviewed risk level is missing"),
            "the original reviewed-contract boundary must remain observable: {error}"
        );
        let store = state.proposal_store.as_ref().unwrap().lock().await;
        assert_eq!(
            store.dispatch_state(&proposal_id).unwrap().as_deref(),
            Some("unclaimed")
        );
        assert_eq!(
            store.get_proposal(&proposal_id).unwrap().unwrap().status,
            ProposalStatus::Pending
        );
    }

    #[tokio::test]
    async fn accept_memory_write_proposal_returns_lifecycle_materialization_evidence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            reviewed_memory_after(
                "proposal-lifecycle-session",
                "用户偏好 execution-first agents",
                MemoryCandidateKind::Preference,
                "low",
                "internal",
            ),
            "用户确认写入长期记忆",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let result = accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(result["memoryLifecycle"]["proposalId"], id);
        assert!(
            result["memoryLifecycle"]["memoryId"]
                .as_str()
                .is_some_and(|memory_id| !memory_id.trim().is_empty()),
            "accepted memory must expose a concrete lifecycle memory id: {result:?}"
        );
        assert_eq!(result["memoryLifecycle"]["status"], "materialized");
        assert_eq!(
            result["memoryLifecycle"]["materializationStatus"],
            "materialized"
        );
        assert!(
            result["memoryLifecycle"]["materializedViewVersion"]
                .as_i64()
                .is_some_and(|version| version > 0),
            "accepted memory must expose materialized context update evidence: {result:?}"
        );
    }

    #[tokio::test]
    async fn rollback_memory_asset_requires_exact_id_and_updates_lifecycle_context() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            reviewed_memory_after(
                "proposal-rollback-session",
                "User prefers execution-first agents.",
                MemoryCandidateKind::Preference,
                "low",
                "internal",
            ),
            "User confirmed a long-term memory.",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let accepted = accept_proposal_with_state(id, &state).await.unwrap();
        let memory_id = accepted["memoryLifecycle"]["memoryId"]
            .as_str()
            .expect("accepted memory id")
            .to_string();
        let accepted_view_version = accepted["memoryLifecycle"]["materializedViewVersion"]
            .as_i64()
            .expect("accepted materialized view version");

        let ambiguous = rollback_memory_asset_with_state(
            "execution-first agents".into(),
            "not needed".into(),
            &state,
        )
        .await;
        assert!(
            ambiguous
                .unwrap_err()
                .contains("requires an exact accepted memory id"),
            "rollback must not accept text queries"
        );

        let rolled_back = rollback_memory_asset_with_state(
            memory_id.clone(),
            "User requested rollback.".into(),
            &state,
        )
        .await
        .unwrap();

        assert_eq!(rolled_back.record.memory_id, memory_id);
        assert_eq!(rolled_back.record.status, MemoryLifecycleStatus::RolledBack);
        assert_eq!(rolled_back.rollback_event.memory_id, memory_id);
        assert!(
            rolled_back.materialized_view.version > accepted_view_version,
            "rollback must update materialized context version"
        );
        assert!(
            !rolled_back
                .materialized_view
                .active_memory_ids
                .contains(&memory_id),
            "rolled back memory must be excluded from active materialized context"
        );
    }

    #[tokio::test]
    async fn accept_sensitive_memory_write_proposal_does_not_call_cloud_embedding() {
        openlife_core::embedding::clear_embedding_cache();
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let (openai_base, cloud_call_count) = fake_cloud_embedding_endpoint().await;
        configure_cloud_embeddings(&state, openai_base).await;

        let proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.records",
            reviewed_memory_after(
                "proposal-sensitive-session",
                "身份证 11010519491231002X，邮箱 proposal-sensitive@example.com，最近健康诊断和负债压力",
                MemoryCandidateKind::SemanticUserFact,
                "low",
                "sensitive",
            ),
            "用户确认写入长期记忆",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let result = accept_proposal_with_state(id, &state).await.unwrap();

        assert_eq!(result["success"], true);
        assert_eq!(cloud_call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn accept_memory_archive_proposal_uses_stable_canonical_owner() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let profile = openlife_core::embedding::EmbeddingProfile::new(
            openlife_core::embedding::EmbeddingRouteKind::DeterministicHash,
            "openlife-test",
            "proposal-archive-test-v1",
            "builtin:test",
            "proposal-archive-test-artifact-v1",
            4,
        )
        .unwrap();
        let canonical = state
            .memory_store
            .lock()
            .await
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "s1",
                "temporary canonical memory",
                "knowledge_note",
                "manual",
                &[],
                "private",
            )
            .unwrap();
        let owner = openlife_core::vectors::CanonicalVectorOwnerRef::new(
            "knowledge_note",
            &canonical.knowledge_note_id.to_string(),
        )
        .unwrap();
        state
            .vector_store
            .lock()
            .await
            .project_memory_embedding(
                &canonical.canonical_mutation.event_id,
                &owner,
                "s1",
                "temporary canonical memory",
                &[0.1, 0.2, 0.3, 0.4],
                &profile,
            )
            .unwrap();
        let proposal = AgentProposal::new(
            ProposalType::MemoryArchive,
            "memory.retrieval",
            serde_json::json!({
                "owner": {
                    "ownerKind": owner.kind(),
                    "ownerId": owner.id(),
                }
            }),
            "用户确认归档低价值记忆",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        assert!(!state
            .memory_store
            .lock()
            .await
            .is_memory_retrieval_active(&owner)
            .unwrap());
        let archived = state.vector_store.lock().await.export_all_chunks().unwrap();
        assert_eq!(archived.len(), 1);
        assert!(archived[0].archived);
    }

    #[tokio::test]
    async fn accept_memory_archive_without_stable_owner_keeps_pending() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::MemoryArchive,
            "memory.chunks",
            serde_json::json!({ "reason": "missing ids" }),
            "无效归档请求",
            0.5,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let err = accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap_err();
        assert!(err.contains("after.owner") || err.contains("after.owners"));
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Pending);
    }

    #[test]
    fn memory_archive_payload_rejects_derived_vector_row_ids() {
        let error = validate_proposal_payload(
            ProposalType::MemoryArchive,
            &serde_json::json!({ "chunk_ids": [7] }),
        )
        .expect_err("derived vector ids cannot authorize canonical archive");
        assert!(error.contains("derived vector row id"));
    }

    #[tokio::test]
    async fn accept_tool_permission_proposal_records_permission_event() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            "tools.filesystem.write",
            serde_json::json!({
                "permission_scope_kind": "manifest_policy",
                "tool_name": "filesystem.write",
                "source": "builtin",
                "risk_level": "medium",
                "action_type": "write",
                "permission": "allowed"
            }),
            "用户确认工具权限",
            0.7,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
    }

    #[tokio::test]
    async fn main_chat_action_bound_tool_permission_without_exact_scope_stays_pending() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.builtin_echo",
            serde_json::json!({
                "permission_scope_kind": "action_bound",
                "tool_name": "builtin_echo",
                "source": "builtin",
                "risk_level": "low",
                "action_type": "read",
                "permission": "allow_once",
                "mainChatAgentV1": true
            }),
            "Missing action-bound scope must fail closed.",
            0.7,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let error = accept_proposal_with_state(id.clone(), &state)
            .await
            .expect_err("missing exact blocked_action must not materialize permission");
        assert!(error.contains("blocked_action"));
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Pending);
        let permissions = state.tool_permission_store.lock().await;
        assert!(permissions.list().unwrap().is_empty());
        assert_eq!(permissions.action_bound_permission_count().unwrap(), 0);
    }

    #[tokio::test]
    async fn accept_auto_tool_permission_proposal_uses_policy_and_canonical_scope() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.web.search",
            serde_json::json!({
                "permission_scope_kind": "action_bound",
                "tool_name": "web.search",
                "source": "builtin",
                "risk_level": "medium",
                "permission_action": "grant",
                "policy": "allow_once",
                "canonical_scope": {
                    "tool_name": "web.search",
                    "source": "builtin",
                    "risk_level": "medium",
                    "action_type": "read"
                },
                "blocked_action": {
                    "action_type": "mcp_tool",
                    "target": "web.search",
                    "resolved_target": "web.search",
                    "input_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                    "input_length_bytes": 42
                },
                "auto_generated": true,
                "mainChatAgentV1": true,
                "directWritesExecuted": false
            }),
            "用户确认自动生成的工具权限",
            0.7,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);

        let scope = action_bound_tool_permission_scope(&proposal.after).unwrap();
        let permission_store = state.tool_permission_store.lock().await;
        assert!(
            permission_store.list().unwrap().is_empty(),
            "action-bound permission must not become a globally reusable manifest grant"
        );
        let authorization = permission_store
            .peek_action_bound(&id, &scope)
            .unwrap()
            .expect("exact action-bound permission exists");
        assert_eq!(authorization.proposal_id, id);
        assert_eq!(authorization.scope, scope);
    }

    #[tokio::test]
    async fn accept_invalid_life_model_path_keeps_proposal_pending() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            "identity.no_such_field",
            serde_json::json!("bad"),
            "无效字段",
            0.5,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        stamp_lifemodel_base_hash(&mut proposal, &state).await;
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let err = accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap_err();
        assert!(err.contains("Invalid path") || err.contains("no_such_field"));
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Pending);
    }

    #[tokio::test]
    async fn accept_external_write_action_writes_file_to_safe_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let safe_path = temp_dir.path().join("safe");
        std::fs::create_dir_all(&safe_path).unwrap();
        let safe_path_canonical = safe_path.canonicalize().unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.system.safe_paths = vec![safe_path_canonical.to_string_lossy().to_string()];
        }

        let file_path = safe_path_canonical.join("test.txt");
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.{}", file_path.display()),
            serde_json::json!({
                "path": file_path.to_string_lossy().to_string(),
                "content": "Hello from test",
                "content_hash": "",
                "size_bytes": 15,
                "operation": "create",
            }),
            "测试写入文件",
            0.8,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let response = accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(response["effect_status"], "confirmed");
        assert_eq!(
            response["artifactMaterialization"]["contentDigest"],
            response["artifactMaterialization"]["observedContentDigest"]
        );
        assert_eq!(
            response["artifactMaterialization"]["targetReference"],
            file_path.to_string_lossy().as_ref()
        );

        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello from test");
    }

    #[tokio::test]
    async fn proposal_accepts_hs_external_write_payload_and_verifies_hash() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let safe_path = temp_dir.path().join("safe");
        std::fs::create_dir_all(&safe_path).unwrap();
        let safe_path_canonical = safe_path.canonicalize().unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.system.safe_paths = vec![safe_path_canonical.to_string_lossy().to_string()];
        }

        let file_path = safe_path_canonical.join("hs-payload.txt");
        let content = "真实 content 应由 HS ExternalWriteAction payload 写入";
        let content_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            format!("{:x}", hasher.finalize())
        };
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("builtin.{}", file_path.display()),
            serde_json::json!({
                "tool_name": "file.write",
                "tool_id": "file.write",
                "source": "builtin",
                "arguments": {
                    "path": file_path.to_string_lossy().to_string(),
                    "content": content
                },
                "path": file_path.to_string_lossy().to_string(),
                "content": content,
                "content_preview": content,
                "content_hash": content_hash,
                "size_bytes": content.len(),
                "operation": "create",
                "requires_confirmation": true,
                "hs_policy_id": openlife_core::agent::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST,
            }),
            "HS proposal-first 写入文件",
            0.9,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), content);
    }

    async fn stage_artifact_crash_fixture(
        state: &Arc<AppState>,
        proposal: &AgentProposal,
        content: &str,
        safe_paths: &[String],
    ) -> (String, PreparedArtifactMaterialization) {
        let claim_id = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .claim_dispatch(&proposal.id)
            .unwrap()
            .unwrap();
        let path = proposal.after["path"].as_str().unwrap();
        let prepared =
            prepare_artifact_materialization(&proposal.id, &claim_id, path, content, safe_paths)
                .unwrap();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .prepare_artifact_effect(
                &proposal.id,
                &claim_id,
                &prepared.target_reference_digest,
                &prepared.content_digest,
                prepared.byte_size,
                &prepared.media_type,
            )
            .unwrap();
        (claim_id, prepared)
    }

    #[tokio::test]
    async fn artifact_restart_recovers_staged_bytes_without_blind_redispatch() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        let proposals_db = temp_dir.path().join("artifact-stage-restart.db");
        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let safe_root = temp_dir.path().join("safe-stage");
        std::fs::create_dir_all(&safe_root).unwrap();
        let safe_root = safe_root.canonicalize().unwrap();
        let safe_paths = vec![safe_root.to_string_lossy().into_owned()];
        state.config.lock().await.system.safe_paths = safe_paths.clone();
        let target = safe_root.join("roadshow-summary.md");
        let content = "# Roadshow\n\nRestart-safe artifact.";
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.{}", target.display()),
            serde_json::json!({"path": target, "content": content}),
            "Restart recovery fixture",
            1.0,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let (claim_id, prepared) =
            stage_artifact_crash_fixture(&state, &proposal, content, &safe_paths).await;
        stage_artifact_bytes(&prepared, content).unwrap();
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .mark_artifact_staged(&proposal.id, &claim_id)
            .unwrap());
        assert!(!prepared.target_path.exists());

        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let report = reconcile_durable_proposal_projections_with_state(&state, 200)
            .await
            .unwrap();
        assert_eq!(report.artifact_effects_reconciled, 1);
        assert_eq!(report.proposal_projections_repaired, 1);
        assert_eq!(
            std::fs::read_to_string(&prepared.target_path).unwrap(),
            content
        );
        let store = state.proposal_store.as_ref().unwrap().lock().await;
        assert_eq!(
            store.artifact_effect(&proposal.id).unwrap().unwrap().state,
            ArtifactEffectState::Confirmed
        );
        assert_eq!(
            store.dispatch_state(&proposal.id).unwrap().as_deref(),
            Some("confirmed")
        );
        assert_eq!(
            store.get_proposal(&proposal.id).unwrap().unwrap().status,
            ProposalStatus::Accepted
        );
    }

    #[tokio::test]
    async fn artifact_restart_observes_rename_before_receipt_without_rewriting() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        let proposals_db = temp_dir.path().join("artifact-rename-restart.db");
        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let safe_root = temp_dir.path().join("safe-rename");
        std::fs::create_dir_all(&safe_root).unwrap();
        let safe_root = safe_root.canonicalize().unwrap();
        let safe_paths = vec![safe_root.to_string_lossy().into_owned()];
        state.config.lock().await.system.safe_paths = safe_paths.clone();
        let target = safe_root.join("risks.csv");
        let content = "risk,severity\nrestart,high\n";
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.{}", target.display()),
            serde_json::json!({"path": target, "content": content}),
            "Rename crash fixture",
            1.0,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let (claim_id, prepared) =
            stage_artifact_crash_fixture(&state, &proposal, content, &safe_paths).await;
        stage_artifact_bytes(&prepared, content).unwrap();
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .mark_artifact_staged(&proposal.id, &claim_id)
            .unwrap());
        let observed = commit_staged_artifact(&prepared, &safe_paths).unwrap();
        assert_eq!(observed, prepared.content_digest);
        assert!(!prepared.stage_path.exists());

        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let report = reconcile_durable_proposal_projections_with_state(&state, 200)
            .await
            .unwrap();
        assert_eq!(report.artifact_effects_reconciled, 1);
        assert_eq!(report.proposal_projections_repaired, 1);
        assert_eq!(
            std::fs::read_to_string(&prepared.target_path).unwrap(),
            content
        );
    }

    #[tokio::test]
    async fn artifact_restart_proves_prepared_without_bytes_is_retryable() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        let proposals_db = temp_dir.path().join("artifact-prepared-restart.db");
        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let safe_root = temp_dir.path().join("safe-prepared");
        std::fs::create_dir_all(&safe_root).unwrap();
        let safe_root = safe_root.canonicalize().unwrap();
        let safe_paths = vec![safe_root.to_string_lossy().into_owned()];
        state.config.lock().await.system.safe_paths = safe_paths.clone();
        let target = safe_root.join("retry.md");
        let content = "# Retry after proven no effect";
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.{}", target.display()),
            serde_json::json!({"path": target, "content": content}),
            "Prepared crash fixture",
            1.0,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let (_claim_id, prepared) =
            stage_artifact_crash_fixture(&state, &proposal, content, &safe_paths).await;
        assert!(!prepared.stage_path.exists());
        assert!(!prepared.target_path.exists());

        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let report = reconcile_durable_proposal_projections_with_state(&state, 200)
            .await
            .unwrap();
        assert_eq!(report.artifact_effects_reconciled, 1);
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .dispatch_state(&proposal.id)
                .unwrap()
                .as_deref(),
            Some("failed_before_effect")
        );
        let response = accept_proposal_with_state(proposal.id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(response["artifactMaterialization"]["status"], "confirmed");
        assert_eq!(std::fs::read_to_string(target).unwrap(), content);
    }

    #[tokio::test]
    async fn accept_external_write_action_blocks_outside_safe_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let safe_path = temp_dir.path().join("safe");
        std::fs::create_dir_all(&safe_path).unwrap();
        let safe_path_canonical = safe_path.canonicalize().unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.system.safe_paths = vec![safe_path_canonical.to_string_lossy().to_string()];
        }

        let file_path = temp_dir.path().join("unsafe.txt");
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.{}", file_path.display()),
            serde_json::json!({
                "path": file_path.to_string_lossy().to_string(),
                "content": "should not write",
            }),
            "测试安全路径拦截",
            0.8,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let err = accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap_err();
        assert!(err.contains("safe paths"), "{err}");
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn rejecting_proactive_reminder_records_negative_evidence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "proactive.reminder.pending_proposal",
            serde_json::json!({
                "proactive_reminder_category": "pending_proposal",
                "prompt_digest": "digest-only",
            }),
            "raw reminder rejection text should not be stored as evidence",
            0.7,
            RiskLevel::Low,
            ProposalSource::ProactiveAgent,
        );
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        reject_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let records = state
            .evidence_store
            .lock()
            .await
            .query(openlife_core::agent::EvidenceQuery {
                affected_path: Some("proactive.reminder.pending_proposal".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].linked_proposal_ids.contains(&proposal_id));
        let serialized = serde_json::to_string(&records[0]).unwrap();
        assert!(!serialized.contains("raw reminder rejection text"));
    }

    #[tokio::test]
    async fn accept_scheduled_task_returns_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "calendar.event",
            serde_json::json!({
                "title": "Team Meeting",
                "scheduled_at": "2026-05-10T10:00:00Z",
                "description": "Weekly sync",
            }),
            "测试创建计划任务",
            0.8,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
        let tasks = state
            .scheduled_task_store
            .list_tasks(Some("pending"))
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, id);
        assert_eq!(tasks[0].source_proposal_id.as_deref(), Some(id.as_str()));
        assert!(!temp_dir.path().join("scheduled_tasks.json").exists());
    }

    #[tokio::test]
    async fn accepted_exact_scheduled_cloud_route_seals_scoped_single_use_grant() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        {
            let mut config = state.config.lock().await;
            config.llm.openai_key = "test-key-not-persisted".into();
            config.system.network_policy.default_decision = "allow".into();
        }
        let due_at = chrono::Utc::now() + chrono::Duration::hours(1);
        let expires_at = due_at + chrono::Duration::hours(1);
        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "tasks.reviewed_cloud",
            serde_json::json!({
                "title": "Reviewed cloud task",
                "scheduled_at": due_at.to_rfc3339(),
                "description": "Prepare a short review",
                "tool": "scheduled_task",
                "provider_route": {
                    "data_route": "policy_allowed",
                    "provider": "openai",
                    "model": "gpt-4o-mini",
                    "grant_scope": "single_execution",
                    "consent_scope": "scheduled_provider_once",
                    "expires_at": expires_at.to_rfc3339(),
                }
            }),
            "User reviews one exact scheduled cloud execution.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let task = state
            .scheduled_task_store
            .list_tasks(Some("pending"))
            .unwrap()
            .remove(0);
        assert_eq!(task.id, proposal_id);
        assert_eq!(
            task.provider_grant.data_route,
            openlife_core::llm::ProviderDataRoute::PolicyAllowed
        );
        assert_eq!(
            task.provider_grant.grant_scope,
            openlife_core::tasks::ScheduledProviderGrantScope::SingleExecution
        );
        assert!(task.provider_grant.grant_expires_at.is_some());
        assert!(task.provider_grant.review_snapshot_digest.is_some());
        assert!(task.provider_grant.review_dispatch_claim_digest.is_some());
        assert!(!task
            .provider_grant
            .provider_digest
            .as_deref()
            .unwrap()
            .contains("openai"));
    }

    #[tokio::test]
    async fn sensitive_scheduled_cloud_route_fails_before_task_effect() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        {
            let mut config = state.config.lock().await;
            config.llm.openai_key = "test-key-not-persisted".into();
            config.system.network_policy.default_decision = "allow".into();
        }
        let due_at = chrono::Utc::now() + chrono::Duration::hours(1);
        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "tasks.sensitive_cloud",
            serde_json::json!({
                "title": "Sensitive cloud task",
                "scheduled_at": due_at.to_rfc3339(),
                "description": "Summarize my medical diagnosis and health record",
                "tool": "scheduled_task",
                "provider_route": {
                    "data_route": "policy_allowed",
                    "provider": "openai",
                    "model": "gpt-4o-mini",
                    "grant_scope": "single_execution",
                    "consent_scope": "scheduled_provider_once",
                    "expires_at": (due_at + chrono::Duration::hours(1)).to_rfc3339(),
                }
            }),
            "Cloud route must still pass deterministic sensitivity and expiry policy.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let error = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap_err();

        assert!(error.contains("Patch 应用前校验失败"));
        assert!(state
            .scheduled_task_store
            .list_tasks(None)
            .unwrap()
            .is_empty());
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .dispatch_state(&proposal_id)
                .unwrap()
                .as_deref(),
            Some("failed_before_effect")
        );
    }

    #[tokio::test]
    async fn expired_scheduled_cloud_route_fails_before_task_effect() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        {
            let mut config = state.config.lock().await;
            config.llm.openai_key = "test-key-not-persisted".into();
            config.system.network_policy.default_decision = "allow".into();
        }
        let due_at = chrono::Utc::now() + chrono::Duration::hours(1);
        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "tasks.expired_cloud",
            serde_json::json!({
                "title": "Expired cloud grant",
                "scheduled_at": due_at.to_rfc3339(),
                "description": "Prepare a short review",
                "tool": "scheduled_task",
                "provider_route": {
                    "data_route": "policy_allowed",
                    "provider": "openai",
                    "model": "gpt-4o-mini",
                    "grant_scope": "single_execution",
                    "consent_scope": "scheduled_provider_once",
                    "expires_at": (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339(),
                }
            }),
            "Expired cloud authority must fail before task creation.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        assert!(accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap_err()
            .contains("Patch 应用前校验失败"));
        assert!(state
            .scheduled_task_store
            .list_tasks(None)
            .unwrap()
            .is_empty());
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .dispatch_state(&proposal_id)
                .unwrap()
                .as_deref(),
            Some("failed_before_effect")
        );
    }

    #[tokio::test]
    async fn accept_data_export_returns_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let safe_path = temp_dir.path().join("safe");
        std::fs::create_dir_all(&safe_path).unwrap();
        {
            let mut cfg = state.config.lock().await;
            cfg.system.safe_paths = vec![safe_path.to_string_lossy().to_string()];
        }

        let proposal = AgentProposal::new(
            ProposalType::DataExport,
            "export.file",
            serde_json::json!({
                "content": "exported data",
                "filename": "export.txt",
            }),
            "测试数据导出",
            0.8,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        accept_proposal_with_state(id.clone(), &state)
            .await
            .unwrap();

        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Accepted);
    }

    fn scheduled_builder_proposal(title: &str) -> AgentProposal {
        AgentProposal::new(
            ProposalType::ScheduledTask,
            "tasks.scheduled",
            serde_json::json!({
                "title": title,
                "description": "metadata-safe builder proposal reconciliation test",
            }),
            "Builder candidate awaiting review",
            0.9,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        )
    }

    async fn create_waiting_builder_run(
        state: &Arc<AppState>,
        proposal_id: &str,
        session_id: &str,
    ) -> String {
        let mut run = AgentRun::new_builder_run(session_id);
        run.status = AgentRunStatus::WaitingPermission;
        run.add_generated_proposal(proposal_id);
        let run_id = run.id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        run_id
    }

    async fn create_waiting_conversation_run(
        state: &Arc<AppState>,
        proposal_id: &str,
        session_id: &str,
    ) -> String {
        let mut run = AgentRun::new_chat_run(session_id, "conversation awaiting proposal review");
        run.status = AgentRunStatus::WaitingPermission;
        run.add_generated_proposal(proposal_id);
        let run_id = run.id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        run_id
    }

    #[tokio::test]
    async fn generic_proposal_edit_rejects_builder_typed_batch_without_typed_editor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let batch = openlife_core::life_model::patch::LifeModelPatchBatchV1::new(vec![
            openlife_core::life_model::patch::LifeModelPatchBatchOperationV1 {
                candidate_id: "candidate-1".into(),
                path: "goals.short_term".into(),
                candidate: serde_json::json!([{"title": "typed candidate"}]),
            },
        ])
        .unwrap();
        let proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            openlife_core::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH,
            serde_json::to_value(&batch).unwrap(),
            "Builder typed batch awaiting review",
            0.9,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let error = edit_proposal_with_state(
            proposal_id.clone(),
            serde_json::json!({"arbitrary": "generic replacement"}),
            &state,
        )
        .await
        .unwrap_err();
        assert!(error.contains("typed Builder editor"), "{error}");
        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Pending);
        assert_eq!(stored.after, serde_json::to_value(batch).unwrap());
    }

    #[tokio::test]
    async fn legacy_builder_batch_for_statestore_field_fails_before_effect_not_unknown() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let batch = openlife_core::life_model::patch::LifeModelPatchBatchV1::new(vec![
            openlife_core::life_model::patch::LifeModelPatchBatchOperationV1 {
                candidate_id: "legacy-daily-candidate".into(),
                path: "goals.daily".into(),
                candidate: serde_json::json!([{"name": "legacy pending task", "done": false}]),
            },
        ])
        .unwrap();
        let proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            openlife_core::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH,
            serde_json::to_value(batch).unwrap(),
            "persisted before the StateStore ownership cutover",
            0.9,
            RiskLevel::Low,
            ProposalSource::BuilderReview,
        );
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let error = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .expect_err("the retired Builder write must fail before any effect");

        assert!(error.contains("Patch 应用前校验失败"));
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            assert_eq!(
                store.dispatch_state(&proposal_id).unwrap().as_deref(),
                Some("failed_before_effect")
            );
            assert_ne!(
                store.dispatch_state(&proposal_id).unwrap().as_deref(),
                Some("unknown")
            );
        }
        assert!(state
            .life_model_manager
            .lock()
            .await
            .load()
            .unwrap()
            .goals
            .daily
            .is_empty());
    }

    #[tokio::test]
    async fn confirmed_effect_with_failed_proposal_projection_reports_reconciliation_not_failure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        let proposals_db = temp_dir.path().join("projection-failure-proposals.db");
        let proposal_store = ProposalStore::new(&proposals_db).unwrap();
        Arc::get_mut(&mut state).unwrap().proposal_store =
            Some(Arc::new(Mutex::new(proposal_store)));

        let proposal = scheduled_builder_proposal("projection failure task");
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let run_id = create_waiting_builder_run(&state, &proposal_id, "builder-projection").await;

        rusqlite::Connection::open(&proposals_db)
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_accepted_projection
                 BEFORE UPDATE OF status ON proposals
                 WHEN NEW.status = 'accepted'
                 BEGIN
                   SELECT RAISE(FAIL, 'forced proposal projection failure');
                 END;",
            )
            .unwrap();

        let result = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .expect("the effect is confirmed, so a projection failure must not be reported as effect failure");
        assert_eq!(result["success"], true);
        assert_eq!(result["effect_status"], "confirmed");
        assert_eq!(
            result["proposal_projection_status"],
            "reconciliation_required"
        );
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .dispatch_state(&proposal_id)
                .unwrap()
                .as_deref(),
            Some("confirmed_projection_pending")
        );
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .dispatch_error_code(&proposal_id)
                .unwrap()
                .as_deref(),
            Some("proposal_status_projection_pending")
        );
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_proposal(&proposal_id)
                .unwrap()
                .unwrap()
                .status,
            ProposalStatus::Pending
        );
        let projection_pending_run = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run_id)
            .unwrap()
            .unwrap();
        assert_eq!(projection_pending_run.status, AgentRunStatus::Failed);
        assert!(projection_pending_run.error.as_ref().is_some_and(|error| {
            error.phase == "review_projection_pending" && error.recoverable
        }));
        assert_eq!(
            state
                .scheduled_task_store
                .list_tasks(Some("pending"))
                .unwrap()
                .len(),
            1
        );
        rusqlite::Connection::open(&proposals_db)
            .unwrap()
            .execute_batch("DROP TRIGGER fail_accepted_projection;")
            .unwrap();

        let retry = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .expect("retry must reconcile the durable confirmed effect without redispatch");
        assert_eq!(retry["effect_status"], "confirmed");
        assert_eq!(retry["proposal_projection_status"], "confirmed");
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .dispatch_state(&proposal_id)
                .unwrap()
                .as_deref(),
            Some("confirmed")
        );
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_proposal(&proposal_id)
                .unwrap()
                .unwrap()
                .status,
            ProposalStatus::Accepted
        );
        assert_eq!(
            state
                .scheduled_task_store
                .list_tasks(Some("pending"))
                .unwrap()
                .len(),
            1,
            "projection reconciliation must not replay the already-confirmed effect"
        );
        let reconciled_run = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run_id)
            .unwrap()
            .unwrap();
        assert_eq!(reconciled_run.status, AgentRunStatus::Completed);
        assert!(reconciled_run.error.is_none());
    }

    #[tokio::test]
    async fn startup_reconciliation_does_not_treat_legacy_task_strings_as_terminal_authority() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let task_session_id = uuid::Uuid::new_v4().to_string();
        let mut proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            "goals.short_term",
            serde_json::json!({
                "originatingTaskSessionId": task_session_id,
                "description": "confirmed effect awaiting read-model recovery"
            }),
            "Counterfactual crash after effect and Proposal projection.",
            0.9,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        );
        proposal.source_detail = Some(task_session_id.clone());
        let proposal_id = proposal.id.clone();
        {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            store
                .create_session_with_id(
                    task_session_id.clone(),
                    openlife_core::agent::main_chat_agent_v1::AgentTaskSessionDraft {
                        chat_session_id: "artifact-reconciliation-chat".into(),
                        user_goal: "wait for one governed effect".into(),
                        selected_strategy: openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::FileWriteProposal,
                        current_plan_summary: None,
                        context_snapshot_refs: Vec::new(),
                    },
                )
                .unwrap();
            store
                .set_pending_blockers(&task_session_id, vec![format!("proposal:{proposal_id}")])
                .unwrap();
            store.mark_waiting_permission(&task_session_id).unwrap();
        }
        let run_id =
            create_waiting_conversation_run(&state, &proposal_id, "artifact-recovery-chat").await;
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
            let claim_id = store.claim_dispatch(&proposal_id).unwrap().unwrap();
            assert!(store
                .mark_effect_confirmed_projection_pending(&proposal_id, &claim_id)
                .unwrap());
            proposal.accept();
            assert!(store
                .project_confirmed_effect(&proposal, &claim_id)
                .unwrap());
        }

        let report = reconcile_durable_proposal_projections_with_state(&state, 20)
            .await
            .expect("reconcile accepted effect read models");
        assert_eq!(report.agent_run_candidates_examined, 0);
        assert_eq!(report.agent_runs_reconciled, 0);
        let task = state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .load_session(&task_session_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            task.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        );
        assert_eq!(
            task.pending_blockers,
            vec![format!("proposal:{proposal_id}")],
            "source_detail and after payload strings cannot authorize a post-final TaskSession write"
        );
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_run(&run_id)
                .unwrap()
                .unwrap()
                .status,
            AgentRunStatus::WaitingPermission
        );
    }

    #[tokio::test]
    async fn pending_list_reconciles_durable_confirmed_projection_without_replaying_effect() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = scheduled_builder_proposal("recover confirmed projection");
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let run_id =
            create_waiting_builder_run(&state, &proposal_id, "builder-reconcile-on-list").await;

        let claim_id = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .claim_dispatch(&proposal_id)
            .unwrap()
            .unwrap();
        let review_acceptance = {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            openlife_core::agent::ReviewWorkflow::new(&store)
                .claimed_acceptance_snapshot(&proposal_id, &claim_id)
                .unwrap()
        };
        let effect = apply_proposal_to_state(
            &state,
            &proposal,
            proposal.after.clone(),
            Some(&review_acceptance),
        )
        .await
        .unwrap();
        assert!(effect.success);
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .mark_effect_confirmed_projection_pending(&proposal_id, &claim_id)
            .unwrap());

        let report = reconcile_durable_proposal_projections_with_state(&state, 200)
            .await
            .unwrap();
        assert_eq!(report.proposal_projections_repaired, 1);
        assert_eq!(report.agent_runs_reconciled, 1);
        assert!(!report.projection_backlog_may_remain);
        assert!(!report.agent_run_backlog_may_remain);
        let pending = get_pending_proposals_with_state(200, &state).await.unwrap();
        assert!(pending.iter().all(|item| item.id != proposal_id));
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_proposal(&proposal_id)
                .unwrap()
                .unwrap()
                .status,
            ProposalStatus::Accepted
        );
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .dispatch_state(&proposal_id)
                .unwrap()
                .as_deref(),
            Some("confirmed")
        );
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_run(&run_id)
                .unwrap()
                .unwrap()
                .status,
            AgentRunStatus::Completed
        );
        assert_eq!(
            state
                .scheduled_task_store
                .list_tasks(Some("pending"))
                .unwrap()
                .len(),
            1,
            "recovery must project state only and never replay the materialized task"
        );
    }

    #[tokio::test]
    async fn confirmed_projection_pending_cannot_be_rejected_edited_or_postponed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = scheduled_builder_proposal("confirmed review mutation guard");
        let proposal_id = proposal.id.clone();
        let claim_id = {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
            store.claim_dispatch(&proposal_id).unwrap().unwrap()
        };
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .mark_effect_confirmed_projection_pending(&proposal_id, &claim_id)
            .unwrap());

        assert!(reject_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap_err()
            .contains("already confirmed"));
        assert!(edit_proposal_with_state(
            proposal_id.clone(),
            serde_json::json!({"title": "must not overwrite"}),
            &state,
        )
        .await
        .unwrap_err()
        .contains("already confirmed"));
        assert!(postpone_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap_err()
            .contains("already confirmed"));
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_proposal(&proposal_id)
                .unwrap()
                .unwrap()
                .status,
            ProposalStatus::Pending
        );
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .dispatch_state(&proposal_id)
                .unwrap()
                .as_deref(),
            Some("confirmed_projection_pending")
        );
    }

    #[tokio::test]
    async fn builder_proposal_decisions_reconcile_every_linked_waiting_agent_run() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let accepted = scheduled_builder_proposal("accepted builder task");
        let accepted_id = accepted.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&accepted)
            .unwrap();
        let accepted_run_a =
            create_waiting_builder_run(&state, &accepted_id, "builder-accepted-a").await;
        let accepted_run_b =
            create_waiting_builder_run(&state, &accepted_id, "builder-accepted-b").await;
        accept_proposal_with_state(accepted_id, &state)
            .await
            .unwrap();

        let rejected = scheduled_builder_proposal("rejected builder task");
        let rejected_id = rejected.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&rejected)
            .unwrap();
        let rejected_run =
            create_waiting_builder_run(&state, &rejected_id, "builder-rejected").await;
        reject_proposal_with_state(rejected_id, &state)
            .await
            .unwrap();

        let postponed = scheduled_builder_proposal("postponed builder task");
        let postponed_id = postponed.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&postponed)
            .unwrap();
        let postponed_run =
            create_waiting_builder_run(&state, &postponed_id, "builder-postponed").await;
        postpone_proposal_with_state(postponed_id, &state)
            .await
            .unwrap();

        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        assert_eq!(
            store.get_run(&accepted_run_a).unwrap().unwrap().status,
            AgentRunStatus::Completed
        );
        assert_eq!(
            store.get_run(&accepted_run_b).unwrap().unwrap().status,
            AgentRunStatus::Completed
        );
        assert_eq!(
            store.get_run(&rejected_run).unwrap().unwrap().status,
            AgentRunStatus::Cancelled
        );
        assert_eq!(
            store.get_run(&postponed_run).unwrap().unwrap().status,
            AgentRunStatus::WaitingPermission
        );
    }

    #[tokio::test]
    async fn multi_proposal_run_waits_until_every_linked_review_is_terminal() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let first = scheduled_builder_proposal("first linked decision");
        let second = scheduled_builder_proposal("second linked decision");
        let first_id = first.id.clone();
        let second_id = second.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&first).unwrap();
            store.create_proposal(&second).unwrap();
        }
        let mut run = AgentRun::new_builder_run("builder-multi-proposal");
        run.status = AgentRunStatus::WaitingPermission;
        run.add_generated_proposal(&first_id);
        run.add_generated_proposal(&second_id);
        let run_id = run.id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();

        accept_proposal_with_state(first_id, &state).await.unwrap();
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_run(&run_id)
                .unwrap()
                .unwrap()
                .status,
            AgentRunStatus::WaitingPermission,
            "one accepted Proposal must not complete a Run that still has pending reviews"
        );

        accept_proposal_with_state(second_id, &state).await.unwrap();
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_run(&run_id)
                .unwrap()
                .unwrap()
                .status,
            AgentRunStatus::Completed
        );
    }

    #[tokio::test]
    async fn accepted_proposal_linked_after_review_cannot_be_projected_back_to_waiting() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = scheduled_builder_proposal("accepted before AgentRun projection");
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let run = AgentRun::new_builder_run("builder-accept-before-link");
        let run_id = run.id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();

        accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        // This mirrors the old Builder/Calibration producer shape: it retained
        // a pre-review row, then wrote that full stale row after Proposal review.
        crate::terminal_owner_write_gateway::project_agent_run_from_proposal_staging(
            &state,
            &run_id,
            std::slice::from_ref(&proposal_id),
            crate::terminal_owner_write_gateway::AgentRunProposalStagingReceipt {
                kind: crate::terminal_owner_write_gateway::AgentRunProposalStagingKind::Builder,
                requested_count: 1,
                failed_count: 0,
            },
        )
        .await
        .unwrap();

        let canonical = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            canonical.status,
            AgentRunStatus::Completed,
            "link finalization must derive the already-confirmed Proposal instead of trusting a stale WaitingPermission row"
        );
    }

    #[tokio::test]
    async fn mixed_confirmed_and_rejected_review_is_failed_partial_effect_not_cancelled() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let confirmed = scheduled_builder_proposal("confirmed partial effect");
        let rejected = scheduled_builder_proposal("rejected partial effect");
        let confirmed_id = confirmed.id.clone();
        let rejected_id = rejected.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&confirmed).unwrap();
            store.create_proposal(&rejected).unwrap();
        }
        let mut run = AgentRun::new_builder_run("builder-partial-effect");
        run.status = AgentRunStatus::WaitingPermission;
        run.add_generated_proposal(&confirmed_id);
        run.add_generated_proposal(&rejected_id);
        let run_id = run.id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();

        accept_proposal_with_state(confirmed_id, &state)
            .await
            .unwrap();
        reject_proposal_with_state(rejected_id, &state)
            .await
            .unwrap();

        let canonical = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run_id)
            .unwrap()
            .unwrap();
        assert_eq!(canonical.status, AgentRunStatus::Failed);
        assert_eq!(
            canonical.error.as_ref().map(|error| error.phase.as_str()),
            Some("review_partial_effect")
        );
        let count_receipt = canonical.status_updates.last().unwrap();
        assert_eq!(
            count_receipt.phase,
            openlife_core::agent::AgentLoopPhase::Failed
        );
        assert_eq!(count_receipt.step_index, 1, "confirmed effect count");
        assert_eq!(
            count_receipt.tool_call_index,
            Some(1),
            "declined effect count"
        );
    }

    #[tokio::test]
    async fn unknown_dispatch_truth_remains_remote_unknown_without_promoting_legacy_link() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = scheduled_builder_proposal("unknown dispatch result");
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
            let claim_id = store.claim_dispatch(&proposal_id).unwrap().unwrap();
            assert!(store
                .mark_dispatch_unknown(&proposal_id, &claim_id, "test_transport_unknown")
                .unwrap());
        }
        let run_id =
            create_waiting_builder_run(&state, &proposal_id, "builder-unknown-dispatch").await;

        reconcile_agent_runs_for_proposal(&state, &proposal)
            .await
            .unwrap();

        let (first_finished_at, first_revision, first_status_updates) = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            let run = store.get_run(&run_id).unwrap().unwrap();
            (
                run.finished_at,
                store.canonical_revision(&run_id).unwrap(),
                run.status_updates,
            )
        };

        reconcile_agent_runs_for_proposal(&state, &proposal)
            .await
            .unwrap();

        let canonical = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run_id)
            .unwrap()
            .unwrap();
        assert_eq!(canonical.status, AgentRunStatus::RemoteUnknown);
        assert_eq!(
            canonical.finished_at, first_finished_at,
            "re-observing the same unknown receipt must not manufacture a new terminal time"
        );
        assert_eq!(
            serde_json::to_value(&canonical.status_updates).unwrap(),
            serde_json::to_value(&first_status_updates).unwrap(),
            "re-observing the same unknown receipt must be a semantic no-op"
        );
        let count_receipt = canonical.status_updates.last().unwrap();
        assert_eq!(
            count_receipt.phase,
            openlife_core::agent::AgentLoopPhase::Failed
        );
        assert_eq!(count_receipt.step_index, 1, "unknown effect count");
        assert_eq!(
            canonical
                .status_updates
                .iter()
                .filter(|update| {
                    update.phase == openlife_core::agent::AgentLoopPhase::Failed
                        && update.step_index == 1
                        && update.tool_call_index == Some(0)
                })
                .count(),
            1,
            "unchanged unknown truth must be idempotent, not manufacture progress or duplicate receipts"
        );
        drop(canonical);
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .canonical_revision(&run_id)
                .unwrap(),
            first_revision,
            "a semantic no-op must not bump the canonical AgentRun revision"
        );
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_review_reconcilable_linked_proposal_ids(20)
                .unwrap(),
            Vec::<String>::new(),
            "an untyped legacy Proposal link must not be promoted into the typed durable reconciliation queue"
        );
    }

    #[tokio::test]
    async fn dispatch_receipts_project_exact_agent_run_truth() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        for (label, transition, expected_status, expected_phase) in [
            (
                "failed-before-effect",
                "failed_before_effect",
                AgentRunStatus::Failed,
                Some("review_failed_before_effect"),
            ),
            (
                "confirmed-projection-pending",
                "confirmed_projection_pending",
                AgentRunStatus::Failed,
                Some("review_projection_pending"),
            ),
            (
                "claimed-dispatch",
                "claimed",
                AgentRunStatus::RemoteUnknown,
                Some("review_effect_unknown"),
            ),
        ] {
            let proposal = scheduled_builder_proposal(label);
            let proposal_id = proposal.id.clone();
            {
                let store = state.proposal_store.as_ref().unwrap().lock().await;
                store.create_proposal(&proposal).unwrap();
                let claim_id = store.claim_dispatch(&proposal_id).unwrap().unwrap();
                let changed = match transition {
                    "failed_before_effect" => store
                        .mark_dispatch_failed_before_effect(
                            &proposal_id,
                            &claim_id,
                            "test_before_effect",
                        )
                        .unwrap(),
                    "confirmed_projection_pending" => store
                        .mark_effect_confirmed_projection_pending(&proposal_id, &claim_id)
                        .unwrap(),
                    "claimed" => true,
                    _ => unreachable!(),
                };
                assert!(changed);
            }
            let run_id = create_waiting_builder_run(&state, &proposal_id, label).await;
            reconcile_agent_runs_for_proposal(&state, &proposal)
                .await
                .unwrap();
            let canonical = state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_run(&run_id)
                .unwrap()
                .unwrap();
            assert_eq!(
                canonical.status, expected_status,
                "{transition} must preserve its exact execution truth"
            );
            assert_eq!(
                canonical.error.as_ref().map(|error| error.phase.as_str()),
                expected_phase,
                "{transition} must expose a typed recoverable blocker"
            );
        }

        let unclaimed = scheduled_builder_proposal("unclaimed review");
        let unclaimed_id = unclaimed.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&unclaimed)
            .unwrap();
        let run_id = create_waiting_builder_run(&state, &unclaimed_id, "unclaimed-review").await;
        reconcile_agent_runs_for_proposal(&state, &unclaimed)
            .await
            .unwrap();
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_run(&run_id)
                .unwrap()
                .unwrap()
                .status,
            AgentRunStatus::WaitingPermission,
            "only an unclaimed review is truthfully waiting for permission"
        );
    }

    #[tokio::test]
    async fn review_receipt_idempotency_includes_typed_message() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = scheduled_builder_proposal("typed unknown receipt");
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
            let claim_id = store.claim_dispatch(&proposal_id).unwrap().unwrap();
            assert!(store
                .mark_dispatch_unknown(&proposal_id, &claim_id, "typed_unknown")
                .unwrap());
        }
        let mut run = AgentRun::new_builder_run("typed-unknown-receipt");
        run.status = AgentRunStatus::WaitingPermission;
        run.add_generated_proposal(&proposal_id);
        run.status_updates
            .push(openlife_core::agent::AgentLoopStatusUpdate {
                phase: openlife_core::agent::AgentLoopPhase::Failed,
                message: "different_receipt_type_with_same_counts".into(),
                step_index: 1,
                tool_call_index: Some(0),
                timestamp: chrono::Utc::now(),
            });
        let run_id = run.id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();
        let (previous_receipt_count, previous_receipt_message) = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            let canonical = store.get_run(&run_id).unwrap().unwrap();
            (
                canonical.status_updates.len(),
                canonical.status_updates.last().unwrap().message.clone(),
            )
        };

        reconcile_agent_runs_for_proposal(&state, &proposal)
            .await
            .unwrap();

        let canonical = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run_id)
            .unwrap()
            .unwrap();
        assert_eq!(canonical.status_updates.len(), previous_receipt_count + 1);
        let receipt = canonical.status_updates.last().unwrap();
        assert_ne!(receipt.message, previous_receipt_message);
        assert_eq!(receipt.step_index, 1);
        assert_eq!(receipt.tool_call_index, Some(0));
    }

    #[tokio::test]
    async fn partial_staging_failure_with_all_reviews_declined_is_failed_not_cancelled() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = scheduled_builder_proposal("declined after partial staging");
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let mut run = AgentRun::new_builder_run("partial-staging-all-declined");
        run.status = AgentRunStatus::WaitingPermission;
        run.add_generated_proposal(&proposal_id);
        run.error = Some(openlife_core::agent::AgentRunError {
            message: "proposal_staging_partial_or_failed".into(),
            phase: "review_staging_partial".into(),
            recoverable: true,
        });
        let run_id = run.id.clone();
        state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_run(&run)
            .unwrap();

        reject_proposal_with_state(proposal_id, &state)
            .await
            .unwrap();

        let canonical = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run_id)
            .unwrap()
            .unwrap();
        assert_eq!(canonical.status, AgentRunStatus::Failed);
        assert_ne!(canonical.status, AgentRunStatus::Cancelled);
        assert_eq!(
            canonical.error.as_ref().map(|error| error.phase.as_str()),
            Some("review_staging_partial")
        );
    }

    #[tokio::test]
    async fn terminal_proposal_reconciles_linked_non_builder_agent_runs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = scheduled_builder_proposal("conversation-origin scheduled task");
        proposal.source = ProposalSource::ChatConversation;
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        let run_id =
            create_waiting_conversation_run(&state, &proposal_id, "conversation-linked-run").await;

        accept_proposal_with_state(proposal_id, &state)
            .await
            .unwrap();

        let run = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run_id)
            .unwrap()
            .unwrap();
        assert_eq!(run.kind, openlife_core::agent::AgentTaskKind::Conversation);
        assert_eq!(run.status, AgentRunStatus::Completed);
    }

    #[tokio::test]
    async fn expired_proposal_is_truthful_and_cannot_dispatch() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = scheduled_builder_proposal("expired builder task");
        proposal.expires_at = Some(chrono::Utc::now() - chrono::Duration::minutes(1));
        let proposal_id = proposal.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
            assert_eq!(store.cleanup_expired_proposals().unwrap(), 1);
            assert_eq!(
                store.get_proposal(&proposal_id).unwrap().unwrap().status,
                ProposalStatus::Expired
            );
        }

        let error = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap_err();
        assert!(error.contains("已经过期"), "{error}");
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .dispatch_state(&proposal_id)
                .unwrap()
                .as_deref(),
            Some("unclaimed")
        );
        assert!(state
            .scheduled_task_store
            .list_tasks(Some("pending"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn proposal_serializes_for_frontend_contract() {
        let proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("Fujing"),
            "test",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let value = serde_json::to_value(proposal).unwrap();
        assert!(value.get("proposalType").is_some());
        assert_eq!(value.get("proposalType").unwrap(), "goal_update");
        assert_eq!(value.get("riskLevel").unwrap(), "low");
        assert_eq!(value.get("status").unwrap(), "pending");
    }
}
