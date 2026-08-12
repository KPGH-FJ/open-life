use crate::{
    artifact_materializer::{
        commit_artifact_move, commit_staged_artifact, confirmed_artifact_receipt,
        confirmed_move_receipt, confirmed_move_receipt_from_paths, inspect_artifact_filesystem,
        inspect_artifact_move, prepare_artifact_materialization,
        prepare_artifact_materialization_for_artifact,
        prepare_artifact_materialization_with_precondition_for_artifact, prepare_artifact_move,
        stage_artifact_bytes, ArtifactFilesystemFailure, ArtifactFilesystemObservation,
        ArtifactMaterializationReceipt, ArtifactTargetPrecondition,
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
    LifeModelLearningReviewDecisionReceipt, MemoryLifecycleRecord, MemoryLifecycleScope,
    MemoryLifecycleStatus, MemoryRollbackReport, ProposalSource, ProposalStatus, ProposalType,
    RiskLevel,
};
use openlife_core::life_model::LifeModel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

#[cfg(test)]
use crate::artifact_materializer::{PreparedArtifactMaterialization, PreparedArtifactMove};

/// Maximum content size for ExternalWriteAction (100 KB)
const EXTERNAL_WRITE_MAX_SIZE: usize = 100 * 1024;
pub(crate) const COMMUNICATION_STYLE_CANONICAL_PATH: &str = "preferences.communication_style";

fn canonical_report_artifact_id(proposal: &AgentProposal) -> Option<&str> {
    if proposal
        .after
        .get("generatedByProvider")
        .and_then(Value::as_bool)
        != Some(true)
        || proposal
            .after
            .get("artifactVersion")
            .and_then(Value::as_u64)
            != Some(1)
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
    canonical_report_artifact_id(proposal)
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
    proposal: &AgentProposal,
) -> Result<Vec<String>, String> {
    let config = state.config.lock().await;
    if proposal.after.get("source").and_then(Value::as_str) != Some("markdown_memory_editor") {
        return Ok(config.system.safe_paths.clone());
    }
    let scope = match proposal.after.get("memoryScope").and_then(Value::as_str) {
        Some("workspace") => crate::markdown_memory::MarkdownMemoryScope::Workspace,
        Some("project") => crate::markdown_memory::MarkdownMemoryScope::Project,
        _ => return Err("Markdown memory proposal scope is missing or invalid".into()),
    };
    let relative = proposal
        .after
        .get("memoryRelativePath")
        .and_then(Value::as_str)
        .ok_or_else(|| "Markdown memory proposal relative path is missing".to_string())?;
    let relative = crate::markdown_memory::validate_markdown_memory_relative_path(relative)?;
    let configured_root = match scope {
        crate::markdown_memory::MarkdownMemoryScope::Workspace => {
            config.system.workspace_memory_root.as_deref()
        }
        crate::markdown_memory::MarkdownMemoryScope::Project => {
            config.system.project_memory_root.as_deref()
        }
    }
    .ok_or_else(|| "Markdown memory proposal root is no longer configured".to_string())?;
    let root = std::path::PathBuf::from(configured_root)
        .canonicalize()
        .map_err(|error| format!("Markdown memory proposal root is unavailable: {error}"))?;
    let expected_source = root.join(&relative);
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
            .map(std::path::PathBuf::from)
            .ok_or_else(|| "Markdown memory move source is missing".to_string())?;
        let target = proposal
            .after
            .get("target_path")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
            .ok_or_else(|| "Markdown memory move target is missing".to_string())?;
        let filename = expected_source
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| "Markdown memory move filename is invalid".to_string())?;
        let expected_target = expected_source.with_file_name(format!("{filename}.disabled.md"));
        if operation != "move" || source != expected_source || target != expected_target {
            return Err("Markdown memory move is not bound to the selected scope root".into());
        }
    } else {
        let target = proposal
            .after
            .get("path")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
            .ok_or_else(|| "Markdown memory write target is missing".to_string())?;
        if target != expected_source {
            return Err("Markdown memory write is not bound to the selected scope root".into());
        }
    }
    Ok(vec![root.to_string_lossy().into_owned()])
}

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
        || response.terminal_owner_transition.is_some()
        || response.memory_gateway.is_some()
        || response.memory_lifecycle.is_some()
        || response.memory_persistence.is_some()
        || response.artifact_materialization.is_some()
        || response.life_model_learning.is_some()
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

#[cfg(test)]
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

fn is_retired_lifemodel_patch_batch(proposal: &AgentProposal) -> bool {
    proposal.proposal_type == ProposalType::LifeModelUpdate
        && proposal.source == ProposalSource::BuilderReview
        && proposal.affected_path == openlife_core::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH
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
    pub agent_runs_reconciled: usize,
    pub agent_run_candidates_examined: usize,
    pub agent_run_cursor_advanced: bool,
    pub agent_run_cursor_wrapped: bool,
    pub artifact_backlog_may_remain: bool,
    pub action_effect_backlog_may_remain: bool,
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
                        return Err("artifact move recovery confirmation CAS lost".into());
                    }
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
        let prepared = match prepare_artifact_materialization_for_artifact(
            &artifact_id_for_proposal(&proposal),
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
            inspect_artifact_move(source, target, &record.content_digest, &safe_paths)?;
        if target_reference_digest != record.target_reference_digest
            || !matches!(observation, ArtifactFilesystemObservation::Confirmed { .. })
        {
            return Err("confirmed artifact move receipt binding mismatch".into());
        }
        let observed = record
            .observed_content_digest
            .filter(|digest| digest == &record.content_digest)
            .ok_or_else(|| "confirmed artifact move observed digest missing".to_string())?;
        return Ok(Some(confirmed_move_receipt_from_paths(
            &proposal.id,
            target,
            target_reference_digest,
            observed,
            record.byte_size,
            record.media_type,
        )));
    }
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
    let safe_paths = artifact_safe_paths_for_proposal(state, proposal).await?;
    let prepared = prepare_artifact_materialization_for_artifact(
        &artifact_id_for_proposal(proposal),
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

async fn project_confirmed_canonical_report_artifact(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    receipt: &ArtifactMaterializationReceipt,
) -> Result<Option<openlife_core::task_runtime::CanonicalArtifactRecord>, String> {
    let Some(expected_artifact_id) = canonical_report_artifact_id(proposal) else {
        return Ok(None);
    };
    if receipt.artifact_id != expected_artifact_id
        || receipt.proposal_id != proposal.id
        || receipt.content_digest != receipt.observed_content_digest
    {
        return Err("canonical report Artifact receipt binding mismatch".into());
    }
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    store
        .lock()
        .await
        .confirm_artifact_materialized(
            &proposal.id,
            &receipt.target_reference,
            &receipt.observed_content_digest,
        )
        .map(Some)
        .map_err(|error| format!("canonical report Artifact confirmation failed: {error}"))
}

async fn project_confirmed_canonical_report_artifact_status(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    receipt: Option<&ArtifactMaterializationReceipt>,
    warnings: &mut Vec<String>,
) -> &'static str {
    let Some(receipt) = receipt else {
        return "not_applicable";
    };
    match project_confirmed_canonical_report_artifact(state, proposal, receipt).await {
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

async fn mark_canonical_report_artifact_effect_failure(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    reason_code: &str,
    effect_unknown: bool,
) {
    if canonical_report_artifact_id(proposal).is_none() {
        return;
    }
    let Some(store) = state.canonical_task_runtime_store.as_ref() else {
        log::warn!(
            "[CanonicalTaskRuntime] report Artifact failure projection unavailable: {}",
            reason_code
        );
        return;
    };
    let result = if effect_unknown {
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
            "[CanonicalTaskRuntime] report Artifact failure projection failed: {}",
            error
        );
    }
}

async fn project_canonical_report_review_rejection(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<(), String> {
    if canonical_report_artifact_id(proposal).is_none() {
        return Ok(());
    }
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    store
        .lock()
        .await
        .mark_artifact_review_rejected(&proposal.id)
        .map(|_| ())
        .map_err(|error| format!("canonical report Review rejection projection failed: {error}"))
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
    let operation = proposal
        .after
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("propose_write");
    if matches!(operation, "move" | "trash" | "restore") {
        return apply_external_move_artifact(state, proposal, claim_id).await;
    }
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
    let safe_paths = match artifact_safe_paths_for_proposal(state, proposal).await {
        Ok(safe_paths) => safe_paths,
        Err(error) => {
            let code = "artifact_scope_binding_failed";
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(error);
        }
    };
    let target_precondition = match reviewed_artifact_target_precondition(&proposal.after) {
        Ok(precondition) => precondition,
        Err(error) => {
            let code = "artifact_target_precondition_missing";
            let _ =
                persist_artifact_failed_before_effect(state, &proposal.id, claim_id, code).await;
            return ArtifactApplyOutcome::FailedBeforeEffect(error);
        }
    };
    let prepared = match prepare_artifact_materialization_with_precondition_for_artifact(
        &artifact_id_for_proposal(proposal),
        &proposal.id,
        claim_id,
        path,
        content,
        &safe_paths,
        target_precondition,
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
            "artifact_move_prepare_receipt_failed",
        )
        .await;
        return ArtifactApplyOutcome::FailedBeforeEffect(detail);
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
    let confirmed = {
        let store = state
            .proposal_store
            .as_ref()
            .expect("ProposalStore checked before artifact move")
            .lock()
            .await;
        store.finish_artifact_confirmed(&proposal.id, claim_id, &observed_digest)
    };
    if !matches!(confirmed, Ok(true)) {
        return ArtifactApplyOutcome::Unknown("artifact_move_confirmed_receipt_unavailable".into());
    }
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
        receipt: Box::new(confirmed_move_receipt(&prepared, observed_digest)),
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
                "description": "Review Phase 4",
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
        "taskSessionId",
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
    require_persistence_write(state)?;
    check_safe_mode(state)?;
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
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
                sync_main_chat_task_blockers_after_review_proposal_accept(state, &accepted).await;
                if let Err(error) = reconcile_agent_runs_for_proposal(state, &accepted).await {
                    warnings.push(format!("AgentRun 投影仍等待 reconciliation: {}", error));
                }
                let learning = reconcile_lifemodel_learning_materialization_response(
                    state,
                    &accepted,
                    &mut warnings,
                )
                .await;
                let canonical_task_runtime_projection_status =
                    project_confirmed_canonical_report_artifact_status(
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
                    project_confirmed_canonical_report_artifact_status(
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
        sync_main_chat_task_blockers_after_review_proposal_accept(state, &proposal).await;
        if let Err(error) = reconcile_agent_runs_for_proposal(state, &proposal).await {
            warnings.push(format!("AgentRun 投影仍等待 reconciliation: {}", error));
        }
        let learning =
            reconcile_lifemodel_learning_materialization_response(state, &proposal, &mut warnings)
                .await;
        let canonical_task_runtime_projection_status =
            project_confirmed_canonical_report_artifact_status(
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
        drop(terminal_owner_fence_guard);
        if let Err(error) = reconcile_agent_runs_for_proposal(state, &proposal).await {
            response["agentRunProjectionStatus"] = "reconciliation_required".into();
            response["warnings"] =
                serde_json::json!([format!("AgentRun 投影仍等待 reconciliation: {error}")]);
        } else {
            response["agentRunProjectionStatus"] = "confirmed".into();
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
                mark_canonical_report_artifact_effect_failure(state, &proposal, &error, false)
                    .await;
                return Err(format!(
                    "Artifact materialization failed before effect: {error}"
                ));
            }
            ArtifactApplyOutcome::Unknown(error) => {
                mark_canonical_report_artifact_effect_failure(state, &proposal, &error, true).await;
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
        project_confirmed_canonical_report_artifact_status(
            state,
            &proposal,
            artifact_materialization.as_ref(),
            &mut warnings,
        )
        .await;
    let learning_materialization =
        reconcile_lifemodel_learning_materialization_response(state, &proposal, &mut warnings)
            .await;
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
        "canonical_task_runtime_projection_status": canonical_task_runtime_projection_status,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcceptProposalAndContinueResponse {
    pub acceptance: AcceptProposalResponse,
    pub task_state: Option<crate::main_chat_task_controls::MainChatAgentTaskState>,
    pub continued_same_run: bool,
}

pub(crate) async fn ensure_proposal_task_owner(
    state: &Arc<AppState>,
    proposal_id: &str,
    task_session_id: &str,
) -> Result<(), String> {
    if task_session_id.trim().is_empty() {
        return Err("accept_proposal_and_continue_task_missing".to_string());
    }
    let origin = {
        let store = state
            .proposal_store
            .as_ref()
            .ok_or_else(proposal_store_missing)?
            .lock()
            .await;
        store
            .terminal_owner_origin_binding(proposal_id)
            .map_err(|error| error.to_string())?
    }
    .ok_or_else(|| "accept_proposal_and_continue_owner_missing".to_string())?;
    if origin.task_session_id() != task_session_id {
        return Err("accept_proposal_and_continue_owner_mismatch".to_string());
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn accept_proposal_and_continue(
    proposal_id: String,
    task_session_id: String,
    window: tauri::WebviewWindow,
    state: State<'_, Arc<AppState>>,
) -> Result<AcceptProposalAndContinueResponse, String> {
    ensure_proposal_task_owner(state.inner(), &proposal_id, &task_session_id).await?;
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
                    "affected_path_digest": affected_path_digest,
                }),
                arguments_summary: "接受当前任务的精确 Review checkpoint 并继续同一任务。",
                scope_summary: "仅恢复这个 proposal 绑定的同一 Task/Run；不扩大权限。",
                challenge_id: None,
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        expected_native_confirmation_digest = Some(snapshot_digest);
    }
    let acceptance_value = accept_proposal_with_state_and_confirmation(
        proposal_id,
        state.inner(),
        expected_native_confirmation_digest.as_deref(),
    )
    .await?;
    let acceptance = typed_accept_proposal_response(acceptance_value)?;
    let current = crate::main_chat_task_controls::load_main_chat_agent_task_state(
        &task_session_id,
        state.inner(),
    )
    .await?;
    let should_resume = acceptance.success && current.can_resume;
    let task_state = if should_resume {
        Some(
            crate::main_chat_task_controls::resume_main_chat_agent_task_with_state(
                &task_session_id,
                state.inner(),
            )
            .await?,
        )
    } else {
        Some(current)
    };
    Ok(AcceptProposalAndContinueResponse {
        acceptance,
        task_state,
        continued_same_run: should_resume,
    })
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
            | ProposalType::ScheduledTask
            | ProposalType::DataExport
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

async fn sync_main_chat_task_after_blocking_review_reject(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<bool, String> {
    let (origin, relation_kind) = {
        let Some(proposal_store) = state.proposal_store.as_ref() else {
            return Ok(false);
        };
        let proposal_store = proposal_store.lock().await;
        (
            proposal_store
                .terminal_owner_origin_binding(&proposal.id)
                .map_err(|error| error.to_string())?,
            proposal_store
                .terminal_relation_projection_proof(&proposal.id)
                .map_err(|error| error.to_string())?
                .map(|proof| proof.relation_kind()),
        )
    };
    if !matches!(
        relation_kind,
        Some(
            openlife_core::agent::ProposalTerminalRelationKind::EffectBlockingPrerequisite
                | openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite
        )
    ) {
        return Ok(false);
    }
    let Some(origin) = origin else {
        return Err("blocking review rejection is missing its terminal-owner origin".into());
    };
    terminal_owner_write_gateway_from_state(state)
        .await?
        .apply_blocking_review_rejection(&proposal.id)
        .await
        .map_err(|error| error.to_string())?;
    let cancelled = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?
        .lock()
        .await
        .load_session(origin.task_session_id())
        .map_err(|error| error.to_string())?;
    if cancelled.is_none_or(|session| {
        session.status
            != openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled
    }) {
        return Err("blocking review rejection cancellation is not yet confirmed".into());
    }
    Ok(true)
}

pub(crate) async fn reject_proposal_with_state(
    proposal_id: String,
    state: &Arc<AppState>,
) -> Result<(), String> {
    require_persistence_write(state)?;
    let mut proposal = get_proposal_with_state(state, &proposal_id).await?;
    if proposal.status == ProposalStatus::Rejected {
        project_canonical_report_review_rejection(state, &proposal).await?;
        crate::life_model_learning::record_lifemodel_learning_review_rejected_with_state(
            state, &proposal,
        )
        .await?;
        let _ = sync_main_chat_task_after_blocking_review_reject(state, &proposal).await?;
        if let Err(error) = reconcile_agent_runs_for_proposal(state, &proposal).await {
            log::warn!(
                "[proposal] AgentRun rejection replay reconciliation pending for {}: {}",
                proposal.id,
                error
            );
        }
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
    project_canonical_report_review_rejection(state, &proposal).await?;
    crate::life_model_learning::record_lifemodel_learning_review_rejected_with_state(
        state, &proposal,
    )
    .await?;
    let _task_cancelled =
        sync_main_chat_task_after_blocking_review_reject(state, &proposal).await?;
    if let Err(error) = reconcile_agent_runs_for_proposal(state, &proposal).await {
        log::warn!(
            "[proposal] AgentRun rejection reconciliation pending for {}: {}",
            proposal.id,
            error
        );
    }
    record_rejected_proactive_reminder_evidence(state, &proposal).await;
    Ok(())
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
    if is_lifemodel_v2_typed_diff(&proposal) || is_legacy_lifemodel_v2_migration(&proposal) {
        return Err(
            "LifeModel v2 Proposal requires its schema-aware editor; generic JSON edit is disabled."
                .into(),
        );
    }
    if is_retired_lifemodel_patch_batch(&proposal) {
        return Err(
            "Legacy Builder batch editing is retired; reject it and create a v2 typed LifeModel proposal."
                .into(),
        );
    }
    if matches!(
        proposal.proposal_type,
        ProposalType::LifeModelUpdate
            | ProposalType::GoalUpdate
            | ProposalType::StateUpdate
            | ProposalType::PreferenceUpdate
            | ProposalType::CapabilityUpdate
    ) {
        return Err(
            "Legacy 4D LifeModel proposal editing is retired; reject it and create a v2 typed LifeModel proposal."
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
    Ok(serde_json::json!({
        "success": true,
        "status": "edited_pending_review",
        "durable_write_executed": false,
    }))
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
        config::AppConfig,
        feedback::FeedbackStore,
        life_model::LifeModelManager,
        mcp::McpRegistry,
        mcp_audit::McpAuditStore,
        memory::MemoryStore,
        scheduler::InferenceScheduler,
        vectors::VectorStore,
        versioning::VersionManager,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    #[test]
    fn confirmed_governed_actions_are_eligible_for_task_blocker_recovery() {
        for proposal_type in [
            ProposalType::ExternalWriteAction,
            ProposalType::ScheduledTask,
            ProposalType::DataExport,
        ] {
            assert!(
                proposal_type_resolves_main_chat_review_blocker(proposal_type),
                "{proposal_type} must recover an exact effect-blocking terminal relation"
            );
        }
        assert!(!proposal_type_resolves_main_chat_review_blocker(
            ProposalType::ToolPermission
        ));
    }

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
            canonical_task_runtime_store: Some(Arc::new(Mutex::new(
                openlife_core::task_runtime::CanonicalTaskRuntimeStore::new_in_memory().unwrap(),
            ))),
            evidence_store: Arc::new(Mutex::new(
                openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
            )),
            life_event_store: Some(Arc::new(Mutex::new(
                openlife_core::agent::LifeEventStore::new_in_memory().unwrap(),
            ))),
            policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
            proposal_store: Some(Arc::new(Mutex::new(
                ProposalStore::new_in_memory().unwrap(),
            ))),
            memory_lifecycle_store: Some(Arc::new(Mutex::new(
                openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap(),
            ))),
            life_model_learning_store: Some(Arc::new(Mutex::new(
                openlife_core::agent::LifeModelLearningStore::new_in_memory().unwrap(),
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
            },
            "lifeModelLearning": {
                "candidateId": "candidate-1",
                "proposalId": "proposal-1",
                "changed": true,
                "status": "materialized",
                "contentScrubbed": false,
                "materializedVersion": 7,
                "materializedDocumentDigest": "sha256:lifemodel-v7",
                "canonicalLifeModelChanged": true
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
        assert_eq!(serialized["lifeModelLearning"]["status"], "materialized");
        assert_eq!(serialized["lifeModelLearning"]["materializedVersion"], 7);
        assert_eq!(
            serialized["lifeModelLearning"]["canonicalLifeModelChanged"],
            true
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

        let deferred_learning_error = typed_accept_proposal_response(serde_json::json!({
            "success": false,
            "status": "deferred",
            "reasonCode": "origin_turn_open",
            "proposalId": "proposal-deferred",
            "dispatchState": "unclaimed",
            "durableWriteExecuted": false,
            "lifeModelLearning": {
                "proposalId": "proposal-deferred",
                "status": "reconciliation_required",
                "canonicalLifeModelChanged": true
            }
        }))
        .expect_err("deferred responses must not carry LifeModel materialization truth");
        assert!(deferred_learning_error.contains("confirmed-effect truth fields"));
    }

    #[test]
    fn accept_proposal_ipc_contract_rejects_unconfirmed_lifemodel_learning_truth() {
        let error = typed_accept_proposal_response(serde_json::json!({
            "success": true,
            "effectStatus": "confirmed",
            "proposalProjectionStatus": "confirmed",
            "proposalId": "proposal-learning",
            "lifeModelLearning": {
                "candidateId": "candidate-learning",
                "proposalId": "proposal-learning",
                "changed": false,
                "status": "proposed",
                "contentScrubbed": false,
                "canonicalLifeModelChanged": false
            }
        }))
        .expect_err("proposed Candidate state must not receive materialization credit");
        assert!(error.contains("lacks confirmed materialization truth"));
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
    async fn reject_legacy_maturation_proposal_does_not_run_retired_outcome_pipeline() {
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
        create_maturation_source_evidence(&state, &proposal).await;

        reject_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let records = proposal_outcome_records(&state, &proposal_id).await;
        assert!(records.is_empty());
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
    async fn legacy_lifemodel_proposal_edit_is_rejected_without_mutation() {
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

        let error = edit_proposal_with_state(id.clone(), serde_json::json!("新焦点"), &state)
            .await
            .unwrap_err();
        assert!(error.contains("Legacy 4D LifeModel proposal editing is retired"));

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
        assert_eq!(stored.status, ProposalStatus::Pending);
        assert_eq!(stored.after, serde_json::json!("旧焦点"));
        assert_eq!(stored.resolved_at, None);
    }

    #[tokio::test]
    async fn legacy_maturation_proposal_edit_is_rejected_without_outcome_or_write() {
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

        let error = edit_proposal_with_state(
            proposal_id.clone(),
            serde_json::json!("RAW_EDITED_PAYLOAD_SECRET"),
            &state,
        )
        .await
        .unwrap_err();
        assert!(error.contains("Legacy 4D LifeModel proposal editing is retired"));

        let records = proposal_outcome_records(&state, &proposal_id).await;
        assert!(records.is_empty());

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_ne!(
            model.preferences.communication_style,
            "RAW_EDITED_PAYLOAD_SECRET"
        );
    }

    fn v2_value_add_diff(
        base: Option<&openlife_core::life_model::v2::LifeModelVersionV2>,
        item_id: &str,
        statement: &str,
    ) -> openlife_core::life_model::v2::LifeModelTypedDiffV2 {
        use openlife_core::life_model::v2::{
            LifeModelDocumentV2, LifeModelItemV2, LifeModelSectionV2, LifeModelStatementV2,
            LifeModelTypedDiffV2, LifeModelTypedOperationV2, LIFE_MODEL_V2_TYPED_DIFF_SCHEMA,
        };

        let item = LifeModelStatementV2 {
            id: item_id.into(),
            statement: statement.into(),
            source_refs: vec!["message:user:v2-review".into()],
            confirmed_at: "2026-08-08T10:00:00Z".into(),
        };
        let mut result = base
            .map(|version| version.document.clone())
            .unwrap_or_else(|| LifeModelDocumentV2::empty("primary"));
        result.values.push(item.clone());
        LifeModelTypedDiffV2 {
            schema_version: LIFE_MODEL_V2_TYPED_DIFF_SCHEMA.into(),
            model_id: "primary".into(),
            base_version: base.map(|version| version.model_version),
            base_document_digest: base.map(|version| version.document_digest.clone()),
            operations: vec![LifeModelTypedOperationV2::Add {
                section: LifeModelSectionV2::Values,
                item: LifeModelItemV2::Statement(item),
            }],
            result_document_digest: result.digest().unwrap(),
        }
    }

    async fn store_v2_diff_proposal(
        state: &Arc<AppState>,
        diff: openlife_core::life_model::v2::LifeModelTypedDiffV2,
    ) -> String {
        let mut proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            openlife_core::life_model::v2::LIFE_MODEL_V2_TYPED_DIFF_PATH,
            serde_json::to_value(&diff).unwrap(),
            "User reviewed an exact LifeModel v2 change.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        proposal.base_hash = diff.base_document_digest.clone();
        let id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        id
    }

    #[tokio::test]
    async fn reviewed_v2_typed_diff_advances_head_only_after_accept_and_stale_stays_pending() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        assert!(state
            .life_model_manager
            .lock()
            .await
            .load_v2_current("primary")
            .unwrap()
            .is_none());

        let initial_id = store_v2_diff_proposal(
            &state,
            v2_value_add_diff(None, "value:autonomy", "Autonomy matters."),
        )
        .await;
        assert!(state
            .life_model_manager
            .lock()
            .await
            .load_v2_current("primary")
            .unwrap()
            .is_none());

        let accepted = accept_proposal_with_state(initial_id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(accepted["success"], true);
        let first = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current("primary")
            .unwrap()
            .unwrap();
        assert_eq!(first.model_version, 1);
        assert_eq!(first.source_refs, vec![format!("proposal:{initial_id}")]);
        let audit = state
            .feedback_store
            .lock()
            .await
            .analytics_details_for_event("lifemodel_v2_gateway_materialized", 5)
            .unwrap();
        let audit = audit
            .iter()
            .find_map(|detail| serde_json::from_str::<serde_json::Value>(detail).ok())
            .expect("v2 materialization audit");
        assert_eq!(audit["proposalId"], initial_id);
        assert_eq!(audit["lane"], "canonical_lifemodel_v2_truth");
        assert_eq!(audit["afterHash"], first.document_digest);
        assert_eq!(audit["containsRawContent"], false);

        let stale_id = store_v2_diff_proposal(
            &state,
            v2_value_add_diff(Some(&first), "value:clarity", "Clarity matters."),
        )
        .await;
        let winner_id = store_v2_diff_proposal(
            &state,
            v2_value_add_diff(Some(&first), "value:care", "Care matters."),
        )
        .await;
        accept_proposal_with_state(winner_id.clone(), &state)
            .await
            .unwrap();
        let winner_replay = accept_proposal_with_state(winner_id, &state).await.unwrap();
        assert_eq!(winner_replay["success"], true);

        let error = accept_proposal_with_state(stale_id.clone(), &state)
            .await
            .unwrap_err();
        assert!(error.contains("lifemodel_v2_typed_diff_stale_base"));
        let current = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current("primary")
            .unwrap()
            .unwrap();
        assert_eq!(current.model_version, 2);
        assert!(current
            .document
            .values
            .iter()
            .any(|item| item.id == "value:care"));
        assert!(!current
            .document
            .values
            .iter()
            .any(|item| item.id == "value:clarity"));
        let stale = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&stale_id)
            .unwrap()
            .unwrap();
        assert_eq!(stale.status, ProposalStatus::Pending);
    }

    async fn draft_test_legacy_migration(state: &Arc<AppState>) -> String {
        use openlife_core::life_model::v2::{
            LegacyLifeModelMigrationDecisionV2, LegacyLifeModelMigrationPreviewV2,
            LegacyLifeModelMigrationSelectionV2,
        };

        let source = state
            .life_model_manager
            .lock()
            .await
            .load_existing_with_source()
            .unwrap()
            .unwrap()
            .1;
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(&source).unwrap();
        let selections = preview
            .candidates
            .iter()
            .map(|candidate| LegacyLifeModelMigrationSelectionV2 {
                candidate_id: candidate.candidate_id.clone(),
                decision: LegacyLifeModelMigrationDecisionV2::Include,
                edited_value: None,
            })
            .collect();
        crate::commands::life_model::draft_legacy_lifemodel_migration_with_state(
            crate::commands::life_model::DraftLegacyLifeModelMigrationRequest {
                source_digest: preview.source_digest,
                selections,
                non_lifemodel_items_acknowledged: true,
            },
            state,
        )
        .await
        .unwrap()
        .proposal_id
    }

    #[tokio::test]
    async fn legacy_migration_draft_has_no_effect_and_accept_atomically_switches_owner() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        {
            let manager = state.life_model_manager.lock().await;
            let mut legacy = manager.load().unwrap();
            legacy.identity.name = "Alice".into();
            manager.save(&legacy).unwrap();
        }

        let proposal_id = draft_test_legacy_migration(&state).await;
        let manager = state.life_model_manager.lock().await;
        assert!(manager.load_v2_current("primary").unwrap().is_none());
        assert!(manager.load_v2_cutover("primary").unwrap().is_none());
        assert!(!manager.v2_store_path().exists());
        drop(manager);

        let accepted = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(accepted["effect_status"], "confirmed");
        let manager = state.life_model_manager.lock().await;
        let version = manager.load_v2_current("primary").unwrap().unwrap();
        let cutover = manager.load_v2_cutover("primary").unwrap().unwrap();
        assert_eq!(version.model_version, 1);
        assert_eq!(cutover.proposal_id, proposal_id);
        assert_eq!(cutover.document_digest, version.document_digest);
        assert!(manager
            .load_active_legacy_runtime_model()
            .unwrap()
            .is_none());
        let backup_dir = manager
            .v2_store_path()
            .parent()
            .unwrap()
            .join("legacy-backups");
        assert_eq!(std::fs::read_dir(backup_dir).unwrap().count(), 1);
        drop(manager);

        let view = crate::read_models::life_model::get_life_model_view_model_with_state(&state)
            .await
            .unwrap();
        let data = view.data.unwrap();
        assert_eq!(
            data.truth_mode,
            openlife_core::agent::LifeModelTruthMode::Canonical
        );
        assert!(data.legacy_migration_preview.is_none());
        assert!(
            crate::commands::life_model::load_legacy_lifemodel_for_test(&state)
                .await
                .unwrap_err()
                .to_string()
                .contains("legacy_lifemodel_read_owner_retired")
        );
    }

    #[tokio::test]
    async fn legacy_source_drift_after_review_fails_before_cutover_and_stays_pending() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        {
            let manager = state.life_model_manager.lock().await;
            let mut legacy = manager.load().unwrap();
            legacy.identity.name = "Alice".into();
            manager.save(&legacy).unwrap();
        }
        let proposal_id = draft_test_legacy_migration(&state).await;
        {
            let manager = state.life_model_manager.lock().await;
            let mut changed = manager.load().unwrap();
            changed.identity.name = "Bob".into();
            manager.save(&changed).unwrap();
        }

        let error = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap_err();
        assert!(error.contains("Patch 应用前校验失败"), "{error}");
        let manager = state.life_model_manager.lock().await;
        assert!(manager.load_v2_current("primary").unwrap().is_none());
        assert!(manager.load_v2_cutover("primary").unwrap().is_none());
        drop(manager);
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

    #[tokio::test]
    async fn v2_typed_diff_proposal_base_mismatch_is_rejected_before_store_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let diff = v2_value_add_diff(None, "value:autonomy", "Autonomy matters.");
        let mut proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            openlife_core::life_model::v2::LIFE_MODEL_V2_TYPED_DIFF_PATH,
            serde_json::to_value(diff).unwrap(),
            "The review snapshot is intentionally inconsistent.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        proposal.base_hash =
            Some("sha256:0000000000000000000000000000000000000000000000000000000000000000".into());
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
        assert!(error.contains("lifemodel_v2_typed_diff_proposal_base_mismatch"));
        assert!(!state
            .life_model_manager
            .lock()
            .await
            .v2_store_path()
            .exists());
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
    async fn accept_action_bound_web_fetch_permission_stays_exact_and_non_reusable() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.builtin.web.fetch",
            serde_json::json!({
                "permission_scope_kind": "action_bound",
                "tool_name": "web.fetch",
                "source": "builtin",
                "risk_level": "medium",
                "action_type": "network",
                "capabilities": ["network"],
                "permission": "allow_once",
                "policy": "allow_once",
                "canonical_scope": {
                    "tool_name": "web.fetch",
                    "source": "builtin",
                    "risk_level": "medium",
                    "action_type": "network",
                    "capabilities": ["network"],
                    "input_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                    "input_length_bytes": 42
                },
                "blocked_action": {
                    "action_type": "web.fetch",
                    "target": "web.fetch",
                    "resolved_target": "web.fetch",
                    "input_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                    "input_length_bytes": 42
                },
                "pending_action_identity": {
                    "taskSessionId": "task-web-fetch",
                    "runId": "run-web-fetch",
                    "queueActionId": "queue-web-fetch",
                    "executorActionId": "executor-web-fetch",
                    "queueActionType": "web.fetch",
                    "executorActionType": "mcp_tool",
                    "requestedTarget": "web.fetch",
                    "resolvedTarget": "web.fetch",
                    "manifestId": "web.fetch",
                    "manifestName": "web.fetch",
                    "manifestSource": "builtin",
                    "manifestContractDigest": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                    "inputHash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                    "inputLengthBytes": 42,
                    "directWritesExecuted": false
                },
                "auto_generated": true,
                "mainChatAgentV1": true,
                "strictManifestIdentity": true,
                "fuzzyNameMatchingUsed": false,
                "directWritesExecuted": false
            }),
            "Allow exactly this pending Main Chat web fetch once.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        );
        let mut forged_manual = proposal.clone();
        forged_manual.source = ProposalSource::Manual;
        let forged_error = validate_proposal_for_acceptance(&forged_manual)
            .expect_err("manual proposal must not impersonate a Main Chat network action");
        assert!(forged_error.contains("Main Chat 的精确产品路径"));

        let mut drifted_identity = proposal.clone();
        drifted_identity.after["pending_action_identity"]["inputHash"] = serde_json::json!(
            "sha256:2222222222222222222222222222222222222222222222222222222222222222"
        );
        let drift_error = validate_proposal_for_acceptance(&drifted_identity)
            .expect_err("changed request identity must not authorize the reviewed web action");
        assert!(drift_error.contains("执行作用域不一致"));

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
            .expect("exact action-bound network tool permission should be accepted");

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
            "web action review must not create a reusable manifest or network-policy grant"
        );
        assert_eq!(permission_store.action_bound_permission_count().unwrap(), 1);
        assert!(permission_store
            .peek_action_bound(&id, &scope)
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn legacy_lifemodel_write_fails_before_effect_and_stays_pending() {
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
        assert!(
            err.contains("Legacy 4D LifeModel writes are retired"),
            "{err}"
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
        assert_eq!(stored.status, ProposalStatus::Pending);
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .dispatch_state(&id)
                .unwrap()
                .as_deref(),
            Some("failed_before_effect")
        );
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
                "expected_target_absent": true,
                "expected_target_digest": null,
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
    async fn accepted_report_proposal_materializes_the_preexisting_canonical_artifact() {
        use sha2::{Digest, Sha256};

        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let safe_path = temp_dir.path().join("safe-report");
        std::fs::create_dir_all(&safe_path).unwrap();
        let safe_path = safe_path.canonicalize().unwrap();
        state.config.lock().await.system.safe_paths =
            vec![safe_path.to_string_lossy().into_owned()];
        let file_path = safe_path.join("report.md");
        let content = "# Canonical report";
        let content_digest = format!("sha256:{:x}", Sha256::digest(content.as_bytes()));
        let prepared = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .prepare_report_artifact(openlife_core::task_runtime::ReportArtifactDraftInput {
                conversation_id: "conversation-report",
                execution_session_id: "execution-session-report",
                run_id: "run-report",
                outcome_digest: &format!(
                    "sha256:{:x}",
                    Sha256::digest(b"canonical report outcome")
                ),
                plan_digest: &format!("sha256:{:x}", Sha256::digest(b"canonical report plan")),
                provider_request_id: "provider-request-report",
                provider_receipt_digest: &format!(
                    "sha256:{:x}",
                    Sha256::digest(b"provider receipt report")
                ),
                tool_observations: &[],
                target_reference: &file_path.to_string_lossy(),
                content_digest: &content_digest,
                media_type: "text/markdown; charset=utf-8",
            })
            .unwrap();
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.{}", file_path.display()),
            serde_json::json!({
                "path": file_path.to_string_lossy(),
                "content": content,
                "content_hash": content_digest,
                "operation": "propose_write",
                "expected_target_absent": true,
                "expected_target_digest": null,
                "artifactId": prepared.artifact_id,
                "canonicalTaskId": prepared.task_id,
                "artifactDraftItemId": prepared.artifact_draft_item_id,
                "artifactVersion": 1,
                "generatedByProvider": true,
            }),
            "Materialize a generated report after Review",
            1.0,
            RiskLevel::High,
            ProposalSource::ChatConversation,
        );
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .bind_report_review(&prepared.artifact_id, &proposal.id)
            .unwrap();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let response = accept_proposal_with_state(proposal.id.clone(), &state)
            .await
            .unwrap();

        assert_eq!(response["effect_status"], "confirmed");
        assert_eq!(
            response["artifactMaterialization"]["artifactId"],
            prepared.artifact_id
        );
        assert_eq!(
            response["canonical_task_runtime_projection_status"],
            "confirmed"
        );
        let artifact = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .load_artifact(&prepared.artifact_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            artifact.status,
            openlife_core::task_runtime::CanonicalArtifactStatus::Materialized
        );
        assert_eq!(
            artifact.materialized_reference.as_deref(),
            Some(file_path.to_string_lossy().as_ref())
        );
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), content);

        let replay = accept_proposal_with_state(proposal.id, &state)
            .await
            .unwrap();
        assert_eq!(replay["effect_status"], "confirmed");
        assert_eq!(
            replay["canonical_task_runtime_projection_status"],
            "confirmed"
        );
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), content);
    }

    #[tokio::test]
    async fn rejected_report_proposal_blocks_the_preexisting_canonical_artifact_task() {
        use sha2::{Digest, Sha256};

        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let file_path = temp_dir.path().join("rejected-report.md");
        let content = "# Rejected canonical report";
        let content_digest = format!("sha256:{:x}", Sha256::digest(content.as_bytes()));
        let prepared = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .prepare_report_artifact(openlife_core::task_runtime::ReportArtifactDraftInput {
                conversation_id: "conversation-rejected-report",
                execution_session_id: "execution-session-rejected-report",
                run_id: "run-rejected-report",
                outcome_digest: &format!(
                    "sha256:{:x}",
                    Sha256::digest(b"rejected canonical report outcome")
                ),
                plan_digest: &format!(
                    "sha256:{:x}",
                    Sha256::digest(b"rejected canonical report plan")
                ),
                provider_request_id: "provider-request-rejected-report",
                provider_receipt_digest: &format!(
                    "sha256:{:x}",
                    Sha256::digest(b"provider receipt rejected report")
                ),
                tool_observations: &[],
                target_reference: &file_path.to_string_lossy(),
                content_digest: &content_digest,
                media_type: "text/markdown; charset=utf-8",
            })
            .unwrap();
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.{}", file_path.display()),
            serde_json::json!({
                "path": file_path.to_string_lossy(),
                "content": content,
                "content_hash": content_digest,
                "operation": "propose_write",
                "expected_target_absent": true,
                "expected_target_digest": null,
                "artifactId": prepared.artifact_id,
                "canonicalTaskId": prepared.task_id,
                "artifactDraftItemId": prepared.artifact_draft_item_id,
                "artifactVersion": 1,
                "generatedByProvider": true,
            }),
            "Reject a generated report in Review",
            1.0,
            RiskLevel::High,
            ProposalSource::ChatConversation,
        );
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .bind_report_review(&prepared.artifact_id, &proposal.id)
            .unwrap();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        reject_proposal_with_state(proposal.id.clone(), &state)
            .await
            .unwrap();

        let store = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await;
        assert_eq!(
            store
                .load_artifact(&prepared.artifact_id)
                .unwrap()
                .unwrap()
                .status,
            openlife_core::task_runtime::CanonicalArtifactStatus::Failed
        );
        assert_eq!(
            store.load_task(&prepared.task_id).unwrap().unwrap().status,
            openlife_core::task_runtime::CanonicalTaskStatus::Blocked
        );
        assert!(store
            .list_items(&prepared.task_id)
            .unwrap()
            .iter()
            .any(|item| {
                item.kind == openlife_core::task_runtime::CanonicalTaskItemKind::ReviewCheckpoint
                    && item.status == openlife_core::task_runtime::CanonicalTaskItemStatus::Blocked
                    && item.summary_code == "report_artifact_review_rejected"
            }));
        assert!(!file_path.exists());
        drop(store);

        reject_proposal_with_state(proposal.id, &state)
            .await
            .unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn partial_report_metadata_cannot_replace_legacy_artifact_identity() {
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            "filesystem.report",
            serde_json::json!({
                "artifactId": "artifact:forged",
                "generatedByProvider": true,
            }),
            "partial metadata must not mint canonical identity",
            1.0,
            RiskLevel::High,
            ProposalSource::ChatConversation,
        );
        assert_eq!(
            artifact_id_for_proposal(&proposal),
            format!("artifact:{}", proposal.id)
        );
        assert!(canonical_report_artifact_id(&proposal).is_none());
    }

    #[tokio::test]
    async fn accept_external_move_and_restore_are_digest_bound_and_receipted() {
        use sha2::{Digest, Sha256};

        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let safe_root = temp_dir.path().join("safe-move");
        std::fs::create_dir_all(&safe_root).unwrap();
        let safe_root = safe_root.canonicalize().unwrap();
        state.config.lock().await.system.safe_paths =
            vec![safe_root.to_string_lossy().into_owned()];

        let source = safe_root.join("source.md");
        let target = safe_root.join("archive.md");
        let content = "# Reviewed artifact move";
        std::fs::write(&source, content).unwrap();
        let digest = format!("sha256:{:x}", Sha256::digest(content.as_bytes()));

        let move_proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.move.{}", source.display()),
            serde_json::json!({
                "operation": "move",
                "source_path": source.to_string_lossy(),
                "target_path": target.to_string_lossy(),
                "source_digest": digest,
            }),
            "Move an explicitly reviewed file",
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
            .create_proposal(&move_proposal)
            .unwrap();

        let moved = accept_proposal_with_state(move_proposal.id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(moved["effect_status"], "confirmed");
        assert_eq!(
            moved["artifactMaterialization"]["targetReference"],
            target.to_string_lossy().as_ref()
        );
        assert!(!source.exists());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), content);

        let restore_proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("filesystem.restore.{}", target.display()),
            serde_json::json!({
                "operation": "restore",
                "source_path": target.to_string_lossy(),
                "target_path": source.to_string_lossy(),
                "source_digest": digest,
            }),
            "Restore the explicitly reviewed file",
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
            .create_proposal(&restore_proposal)
            .unwrap();

        let restored = accept_proposal_with_state(restore_proposal.id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(restored["effect_status"], "confirmed");
        assert!(source.exists());
        assert!(!target.exists());
        assert_eq!(std::fs::read_to_string(&source).unwrap(), content);
    }

    #[tokio::test]
    async fn external_move_refuses_changed_source_after_review() {
        use sha2::{Digest, Sha256};

        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let safe_root = temp_dir.path().join("safe-move-cas");
        std::fs::create_dir_all(&safe_root).unwrap();
        let safe_root = safe_root.canonicalize().unwrap();
        state.config.lock().await.system.safe_paths =
            vec![safe_root.to_string_lossy().into_owned()];
        let source = safe_root.join("source.md");
        let target = safe_root.join("target.md");
        let reviewed_content = "reviewed";
        std::fs::write(&source, reviewed_content).unwrap();
        let digest = format!("sha256:{:x}", Sha256::digest(reviewed_content.as_bytes()));
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            "filesystem.move.cas",
            serde_json::json!({
                "operation": "move",
                "source_path": source.to_string_lossy(),
                "target_path": target.to_string_lossy(),
                "source_digest": digest,
            }),
            "Digest-bound move",
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
        std::fs::write(&source, "changed after review").unwrap();

        let error = accept_proposal_with_state(proposal.id.clone(), &state)
            .await
            .unwrap_err();
        assert!(error.contains("digest") || error.contains("preflight"));
        assert_eq!(
            std::fs::read_to_string(&source).unwrap(),
            "changed after review"
        );
        assert!(!target.exists());
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
                "expected_target_absent": true,
                "expected_target_digest": null,
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

    async fn prepare_move_crash_fixture(
        state: &Arc<AppState>,
        proposal: &AgentProposal,
        safe_paths: &[String],
    ) -> (String, PreparedArtifactMove) {
        let claim_id = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .claim_dispatch(&proposal.id)
            .unwrap()
            .unwrap();
        let prepared = prepare_artifact_move(
            &proposal.id,
            proposal.after["source_path"].as_str().unwrap(),
            proposal.after["target_path"].as_str().unwrap(),
            proposal.after["source_digest"].as_str().unwrap(),
            safe_paths,
        )
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
            serde_json::json!({
                "path": target,
                "content": content,
                "expected_target_absent": true,
                "expected_target_digest": null,
            }),
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
            serde_json::json!({
                "path": target,
                "content": content,
                "expected_target_absent": true,
                "expected_target_digest": null,
            }),
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
    async fn artifact_move_restart_observes_completed_rename_without_redispatch() {
        use sha2::{Digest, Sha256};

        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        let proposals_db = temp_dir.path().join("artifact-move-restart.db");
        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let safe_root = temp_dir.path().join("safe-move-restart");
        std::fs::create_dir_all(&safe_root).unwrap();
        let safe_root = safe_root.canonicalize().unwrap();
        let safe_paths = vec![safe_root.to_string_lossy().into_owned()];
        state.config.lock().await.system.safe_paths = safe_paths.clone();
        let source = safe_root.join("before.md");
        let target = safe_root.join("after.md");
        let content = "move exactly once";
        std::fs::write(&source, content).unwrap();
        let digest = format!("sha256:{:x}", Sha256::digest(content.as_bytes()));
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            "filesystem.move.restart",
            serde_json::json!({
                "operation": "move",
                "source_path": source.to_string_lossy(),
                "target_path": target.to_string_lossy(),
                "source_digest": digest,
            }),
            "Move restart fixture",
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
            prepare_move_crash_fixture(&state, &proposal, &safe_paths).await;
        let observed = commit_artifact_move(&prepared, &safe_paths).unwrap();
        assert_eq!(observed, prepared.content_digest);
        assert!(!source.exists());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), content);

        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let report = reconcile_durable_proposal_projections_with_state(&state, 200)
            .await
            .unwrap();
        assert_eq!(report.artifact_effects_reconciled, 1);
        assert_eq!(report.proposal_projections_repaired, 1);
        assert!(!source.exists());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), content);
        let store = state.proposal_store.as_ref().unwrap().lock().await;
        assert_eq!(
            store.artifact_effect(&proposal.id).unwrap().unwrap().state,
            ArtifactEffectState::Confirmed
        );
        assert_eq!(
            store.get_proposal(&proposal.id).unwrap().unwrap().status,
            ProposalStatus::Accepted
        );
    }

    #[tokio::test]
    async fn startup_seals_claimed_os_handoff_as_unknown_instead_of_retrying() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        let proposals_db = temp_dir.path().join("os-handoff-restart.db");
        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let proposal = AgentProposal::new(
            ProposalType::DataExport,
            "https://example.com/report",
            serde_json::json!({
                "tool": "browser.open",
                "url": "https://example.com/report",
                "content": "Open the reviewed URL in the system browser",
            }),
            "Browser handoff restart fixture",
            1.0,
            RiskLevel::Medium,
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
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .claim_dispatch(&proposal.id)
            .unwrap()
            .is_some());

        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let startup_coordinator =
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap();
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            startup_coordinator.register_read_write(*store);
        }
        Arc::get_mut(&mut state).unwrap().persistence_coordinator = Arc::new(startup_coordinator);
        let report = reconcile_startup_durable_proposal_projections_with_state(&state, 200)
            .await
            .unwrap();
        assert_eq!(report.ambiguous_action_effects_marked_unknown, 1);
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
            Some("unknown")
        );
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .claim_dispatch(&proposal.id)
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn startup_reconciles_claimed_local_scheduled_task_from_canonical_task() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        let proposals_db = temp_dir.path().join("scheduled-task-restart.db");
        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "calendar.events",
            serde_json::json!({
                "tool": "calendar.propose_event",
                "title": "Restart Review",
                "scheduled_at": "2026-08-12T09:00:00+08:00",
                "description": "Prove the local task without redispatching an OS handoff",
            }),
            "Local scheduled task restart fixture",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        let claim_id = {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&proposal).unwrap();
            store
                .claim_dispatch(&proposal.id)
                .unwrap()
                .expect("claim local scheduled task")
        };
        let review_acceptance = {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            openlife_core::agent::ReviewWorkflow::new(&store)
                .claimed_acceptance_snapshot(&proposal.id, &claim_id)
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
        assert_eq!(
            state.scheduled_task_store.list_tasks(None).unwrap().len(),
            1
        );
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
            Some("claimed")
        );

        Arc::get_mut(&mut state).unwrap().proposal_store = Some(Arc::new(Mutex::new(
            ProposalStore::new(&proposals_db).unwrap(),
        )));
        let startup_coordinator =
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap();
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            startup_coordinator.register_read_write(*store);
        }
        Arc::get_mut(&mut state).unwrap().persistence_coordinator = Arc::new(startup_coordinator);

        let report = reconcile_startup_durable_proposal_projections_with_state(&state, 200)
            .await
            .unwrap();

        assert_eq!(report.ambiguous_action_effects_marked_unknown, 0);
        assert_eq!(report.proposal_projections_repaired, 1);
        let store = state.proposal_store.as_ref().unwrap().lock().await;
        assert_eq!(
            store.dispatch_state(&proposal.id).unwrap().as_deref(),
            Some("confirmed")
        );
        assert_eq!(
            store.get_proposal(&proposal.id).unwrap().unwrap().status,
            ProposalStatus::Accepted
        );
        drop(store);
        assert_eq!(
            state.scheduled_task_store.list_tasks(None).unwrap().len(),
            1
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
            serde_json::json!({
                "path": target,
                "content": content,
                "expected_target_absent": true,
                "expected_target_digest": null,
            }),
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
                "expected_target_absent": true,
                "expected_target_digest": null,
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
    async fn accept_calendar_event_creates_local_task_and_ics_projection() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let safe_root = temp_dir.path().join("calendar-projection");
        std::fs::create_dir_all(&safe_root).unwrap();
        let safe_root = safe_root.canonicalize().unwrap();
        state.config.lock().await.system.safe_paths =
            vec![safe_root.to_string_lossy().into_owned()];
        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "calendar.events",
            serde_json::json!({
                "tool": "calendar.propose_event",
                "title": "Planning Review",
                "scheduled_at": "2026-08-12T09:00:00+08:00",
                "description": "Phase 4 review",
            }),
            "Create a reviewed local calendar projection",
            1.0,
            RiskLevel::Medium,
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

        let response = accept_proposal_with_state(proposal.id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(response["effect_status"], "confirmed");
        let tasks = state.scheduled_task_store.list_tasks(None).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].action_type, "calendar.propose_event");
        let ics_path = safe_root.join(calendar_projection_filename(&proposal, "Planning Review"));
        let ics = std::fs::read_to_string(ics_path).unwrap();
        assert!(ics.contains("BEGIN:VEVENT"));
        assert!(ics.contains("SUMMARY:Planning Review"));
        assert!(ics.contains("DTSTART:20260812T010000Z"));
        assert!(ics.contains(&format!("UID:openlife-{}@local", proposal.id)));
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

    #[tokio::test]
    async fn accept_local_utility_runs_only_the_reviewed_bounded_command() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::DataExport,
            "local.utility.whoami",
            serde_json::json!({
                "tool": "local.run_utility",
                "command": "whoami",
                "timeout_ms": 3_000,
                "content": "Run the exact reviewed read-only utility",
            }),
            "Run one reviewed local utility",
            1.0,
            RiskLevel::Medium,
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

        let response = accept_proposal_with_state(proposal.id.clone(), &state)
            .await
            .unwrap();
        assert_eq!(response["effect_status"], "confirmed");
        assert_eq!(
            response["patch_result"]["operation"],
            "local_utility_completed"
        );
        assert!(response["patch_result"]["error"]
            .as_str()
            .is_some_and(|value| value.contains("Reviewed read-only utility completed")));
        let store = state.proposal_store.as_ref().unwrap().lock().await;
        assert_eq!(
            store.dispatch_state(&proposal.id).unwrap().as_deref(),
            Some("confirmed")
        );
        assert_eq!(
            store.get_proposal(&proposal.id).unwrap().unwrap().status,
            ProposalStatus::Accepted
        );
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
    async fn startup_reconciliation_accepts_sealed_action_resume_without_successor_event() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state)
            .unwrap()
            .main_chat_agent_event_store = Some(Arc::new(Mutex::new(
            crate::main_chat_event_stream::MainChatAgentEventStore::new_in_memory().unwrap(),
        )));

        let task_session_id = uuid::Uuid::new_v4().to_string();
        let chat_session_id = uuid::Uuid::new_v4().to_string();
        let user_goal = "read one governed endpoint";
        let session = state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_session_with_id(
                task_session_id.clone(),
                openlife_core::agent::main_chat_agent_v1::AgentTaskSessionDraft {
                    chat_session_id: chat_session_id.clone(),
                    user_goal: user_goal.into(),
                    selected_strategy:
                        openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::ReActToolExecution,
                    current_plan_summary: None,
                    context_snapshot_refs: Vec::new(),
                },
            )
            .unwrap();

        let run_id = task_session_id.clone();
        let canonical_message = state
            .memory_store
            .lock()
            .await
            .save_message_idempotent_with_proof(
                &chat_session_id,
                &openlife_core::llm::ChatMessage {
                    role: "user".into(),
                    content: user_goal.into(),
                },
                &run_id,
            )
            .unwrap();
        {
            let memory_store = state.memory_store.lock().await;
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .bind_canonical_memory_store(&memory_store)
                .unwrap();
            let task_store = state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            task_store
                .bind_canonical_memory_store(&memory_store)
                .unwrap();
            task_store
                .bind_session_canonical_user_message(
                    &task_session_id,
                    &canonical_message.receipt().canonical_ref,
                    user_goal,
                )
                .unwrap();
        }
        let mut run = AgentRun::new_chat_run(&chat_session_id, user_goal);
        run.id = run_id.clone();
        run.task_id = task_session_id.clone();
        run.input_ref = Some(canonical_message.receipt().canonical_ref.clone());
        crate::terminal_owner_write_gateway::create_conversation_bound_agent_run(
            &state,
            &run,
            &canonical_message,
        )
        .await
        .unwrap();

        let admission = {
            let task_store = state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            task_store
                .issue_terminal_owner_epoch_admission(&task_session_id, &run_id, canonical_message)
                .unwrap()
        };
        let epoch = state
            .main_chat_agent_event_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .open_terminal_owner_epoch_from_admission(admission)
            .unwrap();

        let proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            "tool_permission.network_policy.example_com",
            serde_json::json!({
                "permission": "allow_once",
                "tool_name": "web_fetch",
                "source": "network_policy",
                "risk_level": "medium",
                "action_type": "network",
            }),
            "Exact endpoint approval is required before governed replay.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        );
        let execution_registry =
            crate::main_chat_cancellation::MainChatCancellationRegistry::default();
        let execution_registration = execution_registry.register(&task_session_id);
        let execution_epoch = execution_registration.execution_epoch();
        let submission =
            crate::terminal_owner_write_gateway::submit_main_chat_terminal_review_relation(
                &state,
                epoch.review_origin_proof().unwrap(),
                openlife_core::agent::ProposalTerminalRelationKind::ActionResumePrerequisite,
                openlife_core::agent::DurableWriteRequest::from_agent_proposal(
                    openlife_core::agent::DurableWriteSource::MainChat,
                    openlife_core::agent::DurableWriteSubject::ToolPermission,
                    proposal,
                    "Await exact endpoint approval.",
                )
                .with_idempotency_key(format!("startup-action-resume:{task_session_id}")),
                &execution_epoch,
            )
            .await
            .unwrap();
        let proposal_id = submission.review().proposal_id().to_string();

        {
            let task_store = state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            task_store
                .set_pending_blockers(&task_session_id, vec![format!("proposal:{proposal_id}")])
                .unwrap();
            task_store
                .mark_waiting_permission(&task_session_id)
                .unwrap();
        }

        let owner = state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_owner_head(&session.id)
            .unwrap()
            .unwrap();
        {
            let event_store = state
                .main_chat_agent_event_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            event_store
                .begin_terminal_owner_seal(&task_session_id, &run_id, epoch.generation())
                .unwrap();
            event_store
                .append_terminal_final_and_seal(
                    crate::main_chat_event_stream::MainChatTerminalFinalizationInput {
                        task_session_id: task_session_id.clone(),
                        run_id: run_id.clone(),
                        epoch_generation: epoch.generation(),
                        delivery_id: format!("delivery:{task_session_id}"),
                        expected_task_owner_revision: owner.revision(),
                        expected_task_owner_digest: owner.digest().to_string(),
                        status: "waiting_permission".into(),
                    },
                )
                .unwrap();
        }

        let claim_id = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .claim_dispatch(&proposal_id)
            .unwrap()
            .unwrap();
        let acceptance = {
            let proposal_store = state.proposal_store.as_ref().unwrap().lock().await;
            proposal_store
                .mark_effect_confirmed_projection_pending(&proposal_id, &claim_id)
                .unwrap();
            openlife_core::agent::ReviewWorkflow::new(&proposal_store)
                .claimed_acceptance_snapshot(&proposal_id, &claim_id)
                .unwrap()
        };
        terminal_owner_write_gateway_from_state(&state)
            .await
            .unwrap()
            .apply_claimed_review_without_task_transition(acceptance)
            .await
            .unwrap();

        assert!(state
            .main_chat_agent_event_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_immutable_event(
                &task_session_id,
                "terminal_owner.successor_confirmed",
                &format!("successor:{proposal_id}"),
            )
            .unwrap()
            .is_none());

        crate::terminal_owner_write_gateway::update_agent_run_after_review_reconciliation(
            &state,
            &proposal_id,
            &run_id,
        )
        .await
        .expect("foreground action-resume acceptance remains resumable");
        {
            let reconciled_run = state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_run(&run_id)
                .unwrap()
                .unwrap();
            assert_eq!(reconciled_run.status, AgentRunStatus::WaitingPermission);
            assert!(reconciled_run.finished_at.is_none());
        }

        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            let mut misprojected_run = store.get_run(&run_id).unwrap().unwrap();
            misprojected_run.status = AgentRunStatus::Completed;
            misprojected_run.finished_at = Some(chrono::Utc::now());
            store
                .update_run(&misprojected_run)
                .expect("persist pre-fix completed projection for startup recovery");
        }

        let startup_coordinator =
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap();
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            startup_coordinator.register_read_write(*store);
        }
        Arc::get_mut(&mut state).unwrap().persistence_coordinator = Arc::new(startup_coordinator);

        crate::terminal_owner_write_gateway::update_agent_run_after_startup_review_reconciliation(
            &state,
            &proposal_id,
            &run_id,
        )
        .await
        .expect("typed action-resume acceptance does not require a terminal successor event");

        let reconciled_run = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&run_id)
            .unwrap()
            .unwrap();
        assert_eq!(reconciled_run.status, AgentRunStatus::WaitingPermission);
        assert!(reconciled_run.finished_at.is_none());
    }

    #[tokio::test]
    async fn startup_reconciles_review_successor_after_agent_run_projection_crash() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state)
            .unwrap()
            .main_chat_agent_event_store = Some(Arc::new(Mutex::new(
            crate::main_chat_event_stream::MainChatAgentEventStore::new_in_memory().unwrap(),
        )));

        let task_session_id = uuid::Uuid::new_v4().to_string();
        let chat_session_id = uuid::Uuid::new_v4().to_string();
        let user_goal = "remember one reviewed preference";
        state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_session_with_id(
                task_session_id.clone(),
                openlife_core::agent::main_chat_agent_v1::AgentTaskSessionDraft {
                    chat_session_id: chat_session_id.clone(),
                    user_goal: user_goal.into(),
                    selected_strategy:
                        openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::MemoryProposal,
                    current_plan_summary: None,
                    context_snapshot_refs: Vec::new(),
                },
            )
            .unwrap();

        let canonical_message = state
            .memory_store
            .lock()
            .await
            .save_message_idempotent_with_proof(
                &chat_session_id,
                &openlife_core::llm::ChatMessage {
                    role: "user".into(),
                    content: user_goal.into(),
                },
                &task_session_id,
            )
            .unwrap();
        {
            let memory_store = state.memory_store.lock().await;
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .bind_canonical_memory_store(&memory_store)
                .unwrap();
            let task_store = state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            task_store
                .bind_canonical_memory_store(&memory_store)
                .unwrap();
            task_store
                .bind_session_canonical_user_message(
                    &task_session_id,
                    &canonical_message.receipt().canonical_ref,
                    user_goal,
                )
                .unwrap();
        }
        let mut run = AgentRun::new_chat_run(&chat_session_id, user_goal);
        run.id = task_session_id.clone();
        run.task_id = task_session_id.clone();
        run.input_ref = Some(canonical_message.receipt().canonical_ref.clone());
        crate::terminal_owner_write_gateway::create_conversation_bound_agent_run(
            &state,
            &run,
            &canonical_message,
        )
        .await
        .unwrap();

        let admission = state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .issue_terminal_owner_epoch_admission(
                &task_session_id,
                &task_session_id,
                canonical_message,
            )
            .unwrap();
        let epoch = state
            .main_chat_agent_event_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .open_terminal_owner_epoch_from_admission(admission)
            .unwrap();
        let proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.candidates",
            reviewed_memory_after(
                &chat_session_id,
                "Prefer concise progress reports.",
                MemoryCandidateKind::Preference,
                "low",
                "internal",
            ),
            "Remember only after explicit review.",
            1.0,
            RiskLevel::Low,
            ProposalSource::ChatConversation,
        );
        let execution_registry =
            crate::main_chat_cancellation::MainChatCancellationRegistry::default();
        let execution_registration = execution_registry.register(&task_session_id);
        let submission =
            crate::terminal_owner_write_gateway::submit_main_chat_terminal_review_relation(
                &state,
                epoch.review_origin_proof().unwrap(),
                openlife_core::agent::ProposalTerminalRelationKind::EffectBlockingPrerequisite,
                openlife_core::agent::DurableWriteRequest::from_agent_proposal(
                    openlife_core::agent::DurableWriteSource::MainChat,
                    openlife_core::agent::DurableWriteSubject::Memory,
                    proposal,
                    "Await explicit memory review.",
                )
                .with_idempotency_key(format!("startup-review-successor:{task_session_id}")),
                &execution_registration.execution_epoch(),
            )
            .await
            .unwrap();
        let proposal_id = submission.review().proposal_id().to_string();
        {
            let task_store = state
                .main_chat_agent_session_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            task_store
                .set_pending_blockers(&task_session_id, vec![format!("proposal:{proposal_id}")])
                .unwrap();
            task_store
                .mark_waiting_permission(&task_session_id)
                .unwrap();
        }
        let owner = state
            .main_chat_agent_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .canonical_owner_head(&task_session_id)
            .unwrap()
            .unwrap();
        {
            let event_store = state
                .main_chat_agent_event_store
                .as_ref()
                .unwrap()
                .lock()
                .await;
            event_store
                .begin_terminal_owner_seal(&task_session_id, &task_session_id, epoch.generation())
                .unwrap();
            event_store
                .append_terminal_final_and_seal(
                    crate::main_chat_event_stream::MainChatTerminalFinalizationInput {
                        task_session_id: task_session_id.clone(),
                        run_id: task_session_id.clone(),
                        epoch_generation: epoch.generation(),
                        delivery_id: format!("delivery:{task_session_id}"),
                        expected_task_owner_revision: owner.revision(),
                        expected_task_owner_digest: owner.digest().to_string(),
                        status: "completed_with_pending_items".into(),
                    },
                )
                .unwrap();
        }

        let acceptance = accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .expect("review acceptance commits the effect and terminal successor");
        assert_eq!(acceptance["agentRunProjectionStatus"], "confirmed");
        assert_eq!(
            state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_run(&task_session_id)
                .unwrap()
                .unwrap()
                .status,
            AgentRunStatus::Completed
        );

        {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            let mut stale_run = store.get_run(&task_session_id).unwrap().unwrap();
            stale_run.status = AgentRunStatus::WaitingPermission;
            stale_run.finished_at = None;
            store.update_run(&stale_run).unwrap();
        }
        drop(execution_registration);
        let startup_coordinator =
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap();
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            startup_coordinator.register_read_write(*store);
        }
        Arc::get_mut(&mut state).unwrap().persistence_coordinator = Arc::new(startup_coordinator);

        assert_eq!(
            crate::bootstrap::reconcile_startup_orphaned_main_chat_runs(&state)
                .await
                .expect("startup repairs the exact successor-backed AgentRun projection"),
            1
        );
        state.persistence_coordinator.seal();
        state
            .persistence_coordinator
            .require_effects_allowed()
            .expect("exact successor recovery must not leave Safe Mode active");
        let recovered = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(&task_session_id)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.status, AgentRunStatus::Completed);
        assert!(recovered.finished_at.is_some());
    }

    #[tokio::test]
    async fn generic_proposal_edit_rejects_retired_builder_batch() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let batch = serde_json::json!({
            "schemaVersion": "lifemodel_patch_batch_v1",
            "operations": [{
                "candidateId": "candidate-1",
                "path": "goals.short_term",
                "candidate": [{"title": "typed candidate"}]
            }]
        });
        let proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            openlife_core::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH,
            batch.clone(),
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
        assert!(
            error.contains("Legacy Builder batch editing is retired"),
            "{error}"
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
        assert_eq!(stored.after, batch);
    }

    #[tokio::test]
    async fn generic_proposal_edit_rejects_v2_typed_diff_without_schema_aware_editor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            openlife_core::life_model::v2::LIFE_MODEL_V2_TYPED_DIFF_PATH,
            serde_json::json!({"schemaVersion": "lifemodel_typed_diff_v2"}),
            "v2 change awaiting review",
            1.0,
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

        let error = edit_proposal_with_state(
            proposal_id.clone(),
            serde_json::json!({"arbitrary": "generic replacement"}),
            &state,
        )
        .await
        .unwrap_err();

        assert!(error.contains("schema-aware editor"), "{error}");
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
        assert_eq!(
            stored.after,
            serde_json::json!({"schemaVersion": "lifemodel_typed_diff_v2"})
        );
    }

    #[tokio::test]
    async fn legacy_builder_batch_for_statestore_field_fails_before_effect_not_unknown() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let batch = serde_json::json!({
            "schemaVersion": "lifemodel_patch_batch_v1",
            "operations": [{
                "candidateId": "legacy-daily-candidate",
                "path": "goals.daily",
                "candidate": [{"name": "legacy pending task", "done": false}]
            }]
        });
        let proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            openlife_core::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH,
            batch,
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
