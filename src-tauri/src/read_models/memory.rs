use crate::memory_gateway;
use crate::state::AppState;
use openlife_core::agent::{
    build_memory_view_model, EvidenceRef, EvidenceSensitivity, EvidenceSource, MemoryTierSummary,
    MemoryViewModel, MemoryViewModelBuildInput, ReviewItem, ViewModelEnvelope, ViewModelStatus,
    ViewModelWarning, ViewModelWarningSeverity,
};
use openlife_core::vectors::TierStats;
use std::sync::Arc;
use tauri::State;

use super::review_center::get_review_center_view_model_with_state;

#[tauri::command]
pub async fn get_memory_view_model(
    state: State<'_, Arc<AppState>>,
) -> Result<ViewModelEnvelope<MemoryViewModel>, String> {
    get_memory_view_model_with_state(state.inner()).await
}

pub(crate) async fn get_memory_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<MemoryViewModel>, String> {
    let mut warnings = Vec::new();
    let (lifecycle_available, lifecycle_records) =
        load_lifecycle_records(state, &mut warnings).await;
    let tier_summary = load_tier_summary(state, &mut warnings).await;
    let review_items = load_review_items(state, &mut warnings).await;

    let model = build_memory_view_model(MemoryViewModelBuildInput {
        lifecycle_records,
        review_items,
        tier_summary,
        source_refs: vec![
            source_ref("memory_lifecycle_store", "Memory lifecycle store"),
            source_ref("review_center_view_model", "ReviewCenterViewModel"),
            source_ref("vector_store_tier_stats", "Vector access-tier telemetry"),
            source_ref(
                "memory_store_retrieval_state",
                "Canonical Memory retrieval state",
            ),
        ],
        contract_limitations: vec![
            "Accepted proposal decisions remain decision state until lifecycle materialization evidence proves applied.".into(),
            "Archived count comes from proven canonical Memory retrieval state; vector archive flags are projection telemetry only.".into(),
        ],
    });

    let status = if !lifecycle_available {
        ViewModelStatus::Error
    } else if model.summary.total_lifecycle_records == 0 && model.review_item_refs.is_empty() {
        ViewModelStatus::Empty
    } else {
        ViewModelStatus::Ready
    };
    let evidence_refs = model.source_refs.clone();
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.evidence_refs = evidence_refs;
    envelope.warnings = warnings;
    Ok(envelope)
}

async fn load_lifecycle_records(
    state: &Arc<AppState>,
    warnings: &mut Vec<ViewModelWarning>,
) -> (bool, Vec<openlife_core::agent::MemoryLifecycleRecord>) {
    let Some(store) = state.memory_lifecycle_store.as_ref() else {
        warnings.push(warning(
            "memory_lifecycle_store_unavailable",
            "MemoryViewModel cannot prove durable memory materialization without MemoryLifecycleStore.",
        ));
        return (false, Vec::new());
    };

    match store.lock().await.list_records(None, None, 200, 0) {
        Ok(records) => (true, records),
        Err(err) => {
            warnings.push(warning(
                "memory_lifecycle_records_unavailable",
                format!("MemoryViewModel could not load lifecycle records: {err}"),
            ));
            (false, Vec::new())
        }
    }
}

async fn load_tier_summary(
    state: &Arc<AppState>,
    warnings: &mut Vec<ViewModelWarning>,
) -> Option<MemoryTierSummary> {
    match memory_gateway::get_memory_tier_stats_with_state(state).await {
        Ok(stats) => Some(tier_summary(stats)),
        Err(err) => {
            warnings.push(warning(
                "memory_tier_stats_unavailable",
                format!("MemoryViewModel could not load vector tier telemetry: {err}"),
            ));
            None
        }
    }
}

async fn load_review_items(
    state: &Arc<AppState>,
    warnings: &mut Vec<ViewModelWarning>,
) -> Vec<ReviewItem> {
    match get_review_center_view_model_with_state(state).await {
        Ok(envelope) => {
            warnings.extend(envelope.warnings);
            envelope.data.map(|model| model.items).unwrap_or_default()
        }
        Err(err) => {
            warnings.push(warning(
                "review_center_view_model_unavailable",
                format!("MemoryViewModel could not load ReviewCenterViewModel refs: {err}"),
            ));
            Vec::new()
        }
    }
}

fn tier_summary(stats: TierStats) -> MemoryTierSummary {
    MemoryTierSummary {
        total: stats.total.max(0) as usize,
        tier1: stats.tier1.max(0) as usize,
        tier2: stats.tier2.max(0) as usize,
        tier3: stats.tier3.max(0) as usize,
        archived: stats.archived.max(0) as usize,
    }
}

fn source_ref(id: impl Into<String>, label: impl Into<String>) -> EvidenceRef {
    EvidenceRef {
        id: id.into(),
        label: label.into(),
        source: EvidenceSource::BackendReadModel,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
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
