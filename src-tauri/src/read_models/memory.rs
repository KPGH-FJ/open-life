use crate::memory_gateway;
use crate::state::AppState;
use openlife_core::agent::{
    build_memory_view_model, EvidenceRef, EvidenceSensitivity, EvidenceSource, MemoryTierSummary,
    MemoryViewModel, MemoryViewModelBuildInput, ReviewItem, ViewModelEnvelope, ViewModelStatus,
    ViewModelWarning, ViewModelWarningSeverity,
};
use openlife_core::vectors::TierStats;
use std::{collections::BTreeMap, sync::Arc};
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
    let (lifecycle_available, lifecycle_records, retrieval_dispositions) =
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
        retrieval_dispositions,
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
) -> (
    bool,
    Vec<openlife_core::agent::MemoryLifecycleRecord>,
    BTreeMap<String, String>,
) {
    let Some(store) = state.memory_lifecycle_store.as_ref() else {
        warnings.push(warning(
            "memory_lifecycle_store_unavailable",
            "MemoryViewModel cannot prove durable memory materialization without MemoryLifecycleStore.",
        ));
        return (false, Vec::new(), BTreeMap::new());
    };

    let store = store.lock().await;
    match store.list_records(None, None, 200, 0) {
        Ok(records) => {
            let mut dispositions = BTreeMap::new();
            for record in &records {
                match store.memory_retrieval_state(&record.memory_id) {
                    Ok(Some(state)) => {
                        dispositions
                            .insert(record.memory_id.clone(), state.disposition.as_str().into());
                    }
                    Ok(None) => {}
                    Err(err) => warnings.push(warning(
                        "memory_retrieval_state_unavailable",
                        format!(
                            "Memory recall state could not be loaded for {}: {err}",
                            record.memory_id
                        ),
                    )),
                }
            }
            (true, records, dispositions)
        }
        Err(err) => {
            warnings.push(warning(
                "memory_lifecycle_records_unavailable",
                format!("MemoryViewModel could not load lifecycle records: {err}"),
            ));
            (false, Vec::new(), BTreeMap::new())
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

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::{
        agent::{
            AgentProposal, MemoryLifecycleAcceptanceInput, ProposalSource, ProposalType, RiskLevel,
        },
        memory::MemoryRetrievalDisposition,
    };

    fn memory_proposal(id: &str, content: &str) -> AgentProposal {
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.project",
            serde_json::json!({
                "content": content,
                "scope": "project",
                "category": "preference",
                "candidateKind": "preference",
                "riskLevel": "low",
                "sensitivity": "internal"
            }),
            "reviewed test memory",
            1.0,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.id = id.into();
        proposal
    }

    #[tokio::test]
    async fn product_items_distinguish_archived_from_privacy_erased_memory() {
        let state = crate::test_utils::test_app_state();
        let proposal = memory_proposal("proposal:memory-view", "A readable project preference");
        let input = MemoryLifecycleAcceptanceInput::from_memory_proposal(
            &proposal,
            "A readable project preference".into(),
        )
        .unwrap();
        let memory_id = {
            let store = state.memory_lifecycle_store.as_ref().unwrap().lock().await;
            let accepted = store.accept_memory_proposal(input).unwrap();
            store
                .set_memory_retrieval_disposition(
                    &accepted.record.memory_id,
                    MemoryRetrievalDisposition::Archived,
                    "test_user_archive",
                )
                .unwrap();
            accepted.record.memory_id
        };

        let archived = get_memory_view_model_with_state(&state).await.unwrap();
        let archived_item = archived
            .data
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.memory_id == memory_id)
            .unwrap();
        assert_eq!(archived_item.recall_state, "archived");
        assert!(archived_item.can_restore);
        assert_eq!(
            archived_item.content.as_deref(),
            Some("A readable project preference")
        );

        state
            .memory_lifecycle_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .privacy_erase_memory_asset(&memory_id)
            .unwrap();
        let erased = get_memory_view_model_with_state(&state).await.unwrap();
        let erased_item = erased
            .data
            .unwrap()
            .items
            .into_iter()
            .find(|item| item.memory_id == memory_id)
            .unwrap();
        assert_eq!(erased_item.recall_state, "erased");
        assert!(erased_item.privacy_erased);
        assert!(erased_item.content.is_none());
        assert!(erased_item.evidence_ids.is_empty());
        assert!(!erased_item.can_privacy_erase);
    }
}
