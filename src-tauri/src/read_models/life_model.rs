use crate::commands::life_model::{
    get_life_model_current_view_for_model_with_state, LifeModelChangeView, LifeModelCurrentView,
};
use crate::life_state_projection::{get_life_state_projection_with_state, LifeStateProjection};
use crate::memory_gateway;
use crate::state::AppState;
use openlife_core::agent::{
    build_life_model_view_model_envelope, AgentProposal, LifeModelCurrentChangeInput,
    LifeModelCurrentViewInput, LifeModelMemoryTierStatsInput, LifeModelProjectionInput,
    LifeModelViewModel, LifeModelViewModelBuildInput, ReviewItem, ViewModelEnvelope,
    ViewModelWarning, ViewModelWarningSeverity,
};
use std::sync::Arc;
use tauri::State;

use super::review_center::get_review_center_view_model_with_state;

#[tauri::command]
pub async fn get_life_model_view_model(
    state: State<'_, Arc<AppState>>,
) -> Result<ViewModelEnvelope<LifeModelViewModel>, String> {
    get_life_model_view_model_with_state(state.inner()).await
}

pub(crate) async fn get_life_model_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<LifeModelViewModel>, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut warnings = Vec::new();

    let life_model_result = {
        let manager = state.life_model_manager.lock().await;
        manager.load_existing()
    };
    let (life_model, load_error) = match life_model_result {
        Ok(model) => (model, None),
        Err(err) => (None, Some(format!("life_model_load_failed: {err}"))),
    };

    let current_view = match life_model.as_ref() {
        Some(model) => match get_life_model_current_view_for_model_with_state(state, model).await {
            Ok(view) => Some(view.into()),
            Err(err) => {
                warnings.push(warning(
                    "lifemodel_current_view_unavailable",
                    format!("LifeModel current compatibility view could not be loaded: {err}"),
                ));
                None
            }
        },
        None => None,
    };

    let projection = match get_life_state_projection_with_state(state).await {
        Ok(projection) => Some(projection.into()),
        Err(err) => {
            warnings.push(warning(
                "life_state_projection_unavailable",
                format!("LifeStateProjection could not be loaded for LifeModelViewModel: {err}"),
            ));
            None
        }
    };

    let proposals = load_proposals(state, &mut warnings).await;
    let review_items = load_review_items(state, &mut warnings).await;
    let memory_count = match memory_gateway::count_memory_chunks_with_state(state).await {
        Ok(count) => count.try_into().ok(),
        Err(err) => {
            warnings.push(warning(
                "memory_count_unavailable",
                format!("Memory count could not be loaded for LifeModelViewModel: {err}"),
            ));
            None
        }
    };
    let tier_stats = match memory_gateway::get_memory_tier_stats_with_state(state).await {
        Ok(stats) => Some(LifeModelMemoryTierStatsInput {
            total: stats.total.max(0) as usize,
            tier1: stats.tier1.max(0) as usize,
            tier2: stats.tier2.max(0) as usize,
            tier3: stats.tier3.max(0) as usize,
            archived: stats.archived.max(0) as usize,
        }),
        Err(err) => {
            warnings.push(warning(
                "memory_tier_stats_unavailable",
                format!("Memory tier stats could not be loaded for LifeModelViewModel: {err}"),
            ));
            None
        }
    };

    let mut envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
        life_model,
        current_view,
        projection,
        proposals,
        review_items,
        memory_count,
        tier_stats,
        now: Some(now),
        error: load_error,
        ..Default::default()
    });
    envelope.warnings.extend(warnings);
    Ok(envelope)
}

async fn load_proposals(
    state: &Arc<AppState>,
    warnings: &mut Vec<ViewModelWarning>,
) -> Vec<AgentProposal> {
    let Some(store) = state.proposal_store.as_ref() else {
        warnings.push(warning(
            "proposal_store_unavailable",
            "Proposal store is unavailable; LifeModel approved-not-applied counts stay fail-closed.",
        ));
        return Vec::new();
    };
    match store.lock().await.list_all_proposals(200, 0) {
        Ok(proposals) => proposals,
        Err(err) => {
            warnings.push(warning(
                "proposal_store_read_failed",
                format!("Proposal store read failed for LifeModelViewModel: {err}"),
            ));
            Vec::new()
        }
    }
}

async fn load_review_items(
    state: &Arc<AppState>,
    warnings: &mut Vec<ViewModelWarning>,
) -> Vec<ReviewItem> {
    match get_review_center_view_model_with_state(state).await {
        Ok(envelope) => envelope.data.map(|model| model.items).unwrap_or_default(),
        Err(err) => {
            warnings.push(warning(
                "review_center_view_model_unavailable",
                format!("ReviewCenterViewModel could not be loaded for LifeModelViewModel: {err}"),
            ));
            Vec::new()
        }
    }
}

impl From<LifeStateProjection> for LifeModelProjectionInput {
    fn from(value: LifeStateProjection) -> Self {
        Self {
            generated_at: Some(value.generated_at),
            safe_mode_active: value.safe_mode.active,
            safe_mode_reason: Some(value.safe_mode.reason),
            life_model_ready: value.readiness.life_model_ready,
            model_empty: value.readiness.model_empty,
            readiness_issues: value.readiness.readiness_issues,
            usage_readiness_issues: value.readiness.usage_readiness_issues,
            source_refs: value.source_refs,
        }
    }
}

impl From<LifeModelCurrentView> for LifeModelCurrentViewInput {
    fn from(value: LifeModelCurrentView) -> Self {
        Self {
            path: value.path,
            label: value.label,
            value: value.value,
            unavailable_reason: value.unavailable_reason,
            current_value_source: value.current_value_source,
            change: value.change.map(Into::into),
        }
    }
}

impl From<LifeModelChangeView> for LifeModelCurrentChangeInput {
    fn from(value: LifeModelChangeView) -> Self {
        Self {
            path: value.path,
            proposal_id: value.proposal_id,
            proposal_status: value.proposal_status,
            proposal_source: value.proposal_source,
            proposal_run_id: value.proposal_run_id,
            source_excerpt_available: value.source_excerpt.is_some(),
            source_unavailable_reason: value.source_unavailable_reason,
            patch_id: value.patch_id,
            patch_status: value.patch_status,
            patch_path: value.patch_path,
            patch_unavailable_reason: value.patch_unavailable_reason,
            snapshot_versions: value.snapshot_versions,
            snapshot_unavailable_reason: value.snapshot_unavailable_reason,
            current_matches_accepted_after: value.current_matches_accepted_after,
        }
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
