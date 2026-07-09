use crate::state::AppState;
use openlife_core::agent::{
    build_review_center_view_model, MemoryLifecycleStatus, MemoryMaterializationStatus,
    ReviewCenterBuildInput, ReviewCenterViewModel, ReviewItemMaterializationStatus,
    ViewModelEnvelope, ViewModelStatus, ViewModelWarning, ViewModelWarningSeverity,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_review_center_view_model(
    state: State<'_, Arc<AppState>>,
) -> Result<ViewModelEnvelope<ReviewCenterViewModel>, String> {
    get_review_center_view_model_with_state(state.inner()).await
}

pub(crate) async fn get_review_center_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<ReviewCenterViewModel>, String> {
    let Some(proposal_store) = state.proposal_store.as_ref() else {
        let mut envelope = ViewModelEnvelope::backend_read_model(ViewModelStatus::Error, None);
        envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
        envelope.warnings.push(warning(
            "proposal_store_unavailable",
            "Proposal store is unavailable; ReviewCenterViewModel cannot determine action eligibility.",
        ));
        return Ok(envelope);
    };

    let proposals = proposal_store
        .lock()
        .await
        .list_all_proposals(100, 0)
        .map_err(|err| format!("failed to load review proposals: {err}"))?;
    let config = state.config.lock().await;
    let safe_paths = config.system.safe_paths.clone();
    drop(config);

    let (materialization_overrides, mut warnings) =
        memory_materialization_overrides(state, &proposals).await;
    let safe_mode_reason = if state.startup_warnings.is_empty() {
        None
    } else {
        Some(format!(
            "Safe Mode is active because startup warnings exist: {}",
            state.startup_warnings.join("; ")
        ))
    };
    let model = build_review_center_view_model(ReviewCenterBuildInput {
        proposals,
        safe_mode_active: !state.startup_warnings.is_empty(),
        safe_mode_reason,
        safe_paths,
        materialization_overrides,
    });

    let status = if model.items.is_empty() {
        ViewModelStatus::Empty
    } else {
        ViewModelStatus::Ready
    };
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.warnings.append(&mut warnings);
    Ok(envelope)
}

async fn memory_materialization_overrides(
    state: &Arc<AppState>,
    proposals: &[openlife_core::agent::AgentProposal],
) -> (
    BTreeMap<String, ReviewItemMaterializationStatus>,
    Vec<ViewModelWarning>,
) {
    let Some(lifecycle_store) = state.memory_lifecycle_store.as_ref() else {
        return (
            BTreeMap::new(),
            vec![warning(
                "memory_lifecycle_store_unavailable",
                "Memory lifecycle proof is unavailable; accepted memory review items stay fail-closed.",
            )],
        );
    };

    let lifecycle_store = lifecycle_store.lock().await;
    let mut overrides = BTreeMap::new();
    let mut warnings = Vec::new();
    for proposal in proposals {
        match lifecycle_store.get_record_by_proposal_id(&proposal.id) {
            Ok(Some(record)) => {
                overrides.insert(
                    proposal.id.clone(),
                    materialization_status_from_memory_lifecycle(
                        record.status,
                        record.materialization_status,
                    ),
                );
            }
            Ok(None) => {}
            Err(err) => warnings.push(warning(
                "memory_lifecycle_lookup_failed",
                format!(
                    "Memory lifecycle proof lookup failed for proposal {}: {err}",
                    proposal.id
                ),
            )),
        }
    }
    (overrides, warnings)
}

fn materialization_status_from_memory_lifecycle(
    status: MemoryLifecycleStatus,
    materialization_status: MemoryMaterializationStatus,
) -> ReviewItemMaterializationStatus {
    if status == MemoryLifecycleStatus::RolledBack {
        return ReviewItemMaterializationStatus::RolledBack;
    }
    if status == MemoryLifecycleStatus::MaterializationFailed {
        return ReviewItemMaterializationStatus::Failed;
    }
    match materialization_status {
        MemoryMaterializationStatus::NotRequired => ReviewItemMaterializationStatus::NotApplicable,
        MemoryMaterializationStatus::Pending => ReviewItemMaterializationStatus::Applying,
        MemoryMaterializationStatus::Materialized => ReviewItemMaterializationStatus::Applied,
        MemoryMaterializationStatus::Failed => ReviewItemMaterializationStatus::Failed,
    }
}

fn warning(code: impl Into<String>, message: impl Into<String>) -> ViewModelWarning {
    ViewModelWarning {
        code: code.into(),
        message: message.into(),
        severity: ViewModelWarningSeverity::Warning,
        evidence_refs: Vec::new(),
    }
}
