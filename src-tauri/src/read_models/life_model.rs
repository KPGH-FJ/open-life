use crate::life_state_projection::{get_life_state_projection_with_state, LifeStateProjection};
use crate::memory_gateway;
use crate::state::AppState;
use openlife_core::agent::{
    build_life_model_view_model_envelope, AgentProposal, LifeModelCanonicalV2Input,
    LifeModelMemoryTierStatsInput, LifeModelProjectionInput, LifeModelViewModel,
    LifeModelViewModelBuildInput, ReviewItem, ViewModelEnvelope, ViewModelWarning,
    ViewModelWarningSeverity,
};
use openlife_core::life_model::v2::{
    LegacyLifeModelMigrationPreviewV2, LifeModelVersionV2, DEFAULT_LIFE_MODEL_V2_MODEL_ID,
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

    let (canonical_v2_result, cutover_result, history_result) = {
        let manager = state.life_model_manager.lock().await;
        let canonical = manager.load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID);
        let cutover = manager.load_v2_cutover(DEFAULT_LIFE_MODEL_V2_MODEL_ID);
        let history = manager.load_v2_history(DEFAULT_LIFE_MODEL_V2_MODEL_ID, 12);
        (canonical, cutover, history)
    };
    let (canonical_v2, canonical_v2_error) =
        match canonical_v2_result.and_then(|version| version.map(canonical_v2_input).transpose()) {
            Ok(version) => (version, None),
            Err(err) => (
                None,
                Some(format!("lifemodel_v2_canonical_load_failed: {err}")),
            ),
        };
    let cutover_error = match cutover_result {
        Ok(_) => None,
        Err(err) => Some(format!("lifemodel_v2_cutover_load_failed: {err}")),
    };
    let (version_history, history_error) = match history_result {
        Ok(history) => (history, None),
        Err(err) => (
            Vec::new(),
            Some(format!("lifemodel_v2_history_load_failed: {err}")),
        ),
    };
    let canonical_owner = canonical_v2.is_some();
    let (legacy_model_present, legacy_yaml_source, legacy_load_error) = if canonical_owner {
        (false, None, None)
    } else {
        let legacy_result = {
            let manager = state.life_model_manager.lock().await;
            manager.load_existing_with_source()
        };
        match legacy_result {
            Ok(Some((_model, source))) => (true, Some(source), None),
            Ok(None) => (false, None, None),
            Err(err) => (false, None, Some(format!("life_model_load_failed: {err}"))),
        }
    };
    let legacy_migration_preview = if canonical_owner {
        None
    } else {
        match legacy_yaml_source {
            Some(ref source) => match LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(source) {
                Ok(preview) => Some(preview),
                Err(err) => {
                    warnings.push(warning(
                        "lifemodel_legacy_migration_preview_unavailable",
                        format!(
                            "Legacy LifeModel migration preview failed closed without changing data: {err}"
                        ),
                    ));
                    None
                }
            },
            None => None,
        }
    };
    let fresh_profile_canonical_empty = !canonical_owner
        && !legacy_model_present
        && legacy_yaml_source.is_none()
        && legacy_load_error.is_none();
    let load_error = canonical_v2_error
        .or(cutover_error)
        .or(history_error)
        .or(legacy_load_error);

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
        canonical_v2,
        version_history,
        fresh_profile_canonical_empty,
        legacy_migration_preview,
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

fn canonical_v2_input(version: LifeModelVersionV2) -> anyhow::Result<LifeModelCanonicalV2Input> {
    let human_projection = version.human_yaml_projection()?;
    let document = version.document.clone();
    Ok(LifeModelCanonicalV2Input {
        model_id: version.model_id,
        schema_version: version.schema_version,
        model_version: version.model_version,
        parent_version: version.parent_version,
        document_digest: version.document_digest,
        summary: version.document.summary(),
        item_count: version.document.total_item_count(),
        updated_at: Some(version.created_at),
        source_refs: version.source_refs,
        document: Some(document),
        human_projection,
    })
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

fn warning(code: impl Into<String>, message: impl Into<String>) -> ViewModelWarning {
    ViewModelWarning {
        code: code.into(),
        message: message.into(),
        severity: ViewModelWarningSeverity::Warning,
        evidence_refs: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::life_model::v2::{
        LifeModelDocumentV2, LifeModelStatementV2, LIFE_MODEL_V2_SCHEMA_VERSION,
    };

    #[test]
    fn shipped_read_model_preserves_exact_canonical_version_identity() {
        let mut document = LifeModelDocumentV2::empty(DEFAULT_LIFE_MODEL_V2_MODEL_ID);
        document.values.push(LifeModelStatementV2 {
            id: "value:autonomy".into(),
            statement: "User autonomy matters.".into(),
            source_refs: vec!["proposal:accepted-2".into()],
            confirmed_at: "2026-08-08T10:00:00Z".into(),
        });
        let version = LifeModelVersionV2 {
            model_id: DEFAULT_LIFE_MODEL_V2_MODEL_ID.into(),
            schema_version: LIFE_MODEL_V2_SCHEMA_VERSION.into(),
            model_version: 2,
            parent_version: Some(1),
            parent_digest: Some("sha256:parent".into()),
            document_digest: document.digest().unwrap(),
            version_digest: "sha256:test-version".into(),
            document,
            materialization_id: "proposal:accepted-2".into(),
            source_refs: vec!["proposal:accepted-2".into()],
            created_at: "2026-08-08T10:01:00Z".into(),
        };

        let input = canonical_v2_input(version).unwrap();

        assert_eq!(input.model_id, DEFAULT_LIFE_MODEL_V2_MODEL_ID);
        assert_eq!(input.model_version, 2);
        assert_eq!(input.parent_version, Some(1));
        assert_eq!(input.item_count, 1);
        assert_eq!(input.source_refs, vec!["proposal:accepted-2"]);
        assert_eq!(input.human_projection.model_version, 2);
        assert!(input
            .human_projection
            .yaml
            .contains("User autonomy matters."));
    }
}
