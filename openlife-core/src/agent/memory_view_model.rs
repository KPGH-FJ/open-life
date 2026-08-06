use crate::agent::memory_lifecycle::{
    MemoryLifecycleCategory, MemoryLifecycleRecord, MemoryLifecycleStatus,
    MemoryMaterializationStatus,
};
use crate::agent::product_read_model::{
    BackendEntityKind, BackendEntityRef, EvidenceRef, EvidenceSensitivity, EvidenceSource,
};
use crate::agent::review_item::{ReviewItem, ReviewItemDecisionStatus, ReviewItemType};
use crate::memory_gateway::MemoryLane;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MemoryTierSummary {
    pub total: usize,
    pub tier1: usize,
    pub tier2: usize,
    pub tier3: usize,
    pub archived: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLifecycleSummary {
    pub candidate_count: usize,
    pub pending_review_count: usize,
    pub edited_pending_review_count: usize,
    pub accepted_count: usize,
    pub confirmed_count: usize,
    pub pending_materialization_count: usize,
    pub materialized_count: usize,
    pub materialization_failed_count: usize,
    pub rejected_count: usize,
    pub deferred_count: usize,
    pub superseded_count: usize,
    pub rolled_back_count: usize,
    pub expired_count: usize,
    pub archived_count: usize,
    #[serde(default)]
    pub by_status: BTreeMap<String, usize>,
    #[serde(default)]
    pub by_materialization_status: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLaneSummary {
    pub lane: MemoryLane,
    pub label: String,
    pub total_count: usize,
    pub active_count: usize,
    pub candidate_count: usize,
    pub pending_review_count: usize,
    pub confirmed_count: usize,
    pub materialized_count: usize,
    pub rolled_back_count: usize,
    pub archived_count: usize,
    #[serde(default)]
    pub review_item_refs: Vec<BackendEntityRef>,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLifeModelLinkageStatus {
    Partial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLifeModelLinkageSummary {
    pub linked_memory_count: usize,
    pub candidate_memory_count: usize,
    pub materialized_memory_count: usize,
    pub conflict_count: usize,
    pub boundary_memory_count: usize,
    pub linkage_status: MemoryLifeModelLinkageStatus,
    #[serde(default)]
    pub memory_refs: Vec<BackendEntityRef>,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MemoryViewModelSummary {
    pub total_lifecycle_records: usize,
    pub active_memory_count: usize,
    pub review_required_count: usize,
    pub materialized_count: usize,
    pub pending_materialization_count: usize,
    pub failed_materialization_count: usize,
    pub rolled_back_count: usize,
    pub archived_vector_count: usize,
    pub conflict_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier_summary: Option<MemoryTierSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItemView {
    pub memory_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub scope: String,
    pub category: String,
    pub status: String,
    pub materialization_status: String,
    pub recall_state: String,
    pub sensitivity: String,
    pub why_remembered: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_at: Option<String>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes_memory_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_memory_id: Option<String>,
    pub privacy_erased: bool,
    pub can_correct: bool,
    pub can_stop_recall: bool,
    pub can_archive: bool,
    pub can_restore: bool,
    pub can_rollback: bool,
    pub can_privacy_erase: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryViewModel {
    pub summary: MemoryViewModelSummary,
    pub lifecycle_summary: MemoryLifecycleSummary,
    pub lane_summaries: Vec<MemoryLaneSummary>,
    pub recent_memory_refs: Vec<BackendEntityRef>,
    pub review_item_refs: Vec<BackendEntityRef>,
    pub life_model_linkage: MemoryLifeModelLinkageSummary,
    pub items: Vec<MemoryItemView>,
    pub source_refs: Vec<EvidenceRef>,
    pub contract_limitations: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryViewModelBuildInput {
    pub lifecycle_records: Vec<MemoryLifecycleRecord>,
    pub review_items: Vec<ReviewItem>,
    pub tier_summary: Option<MemoryTierSummary>,
    pub source_refs: Vec<EvidenceRef>,
    pub contract_limitations: Vec<String>,
    /// Canonical retrieval disposition keyed by lifecycle memory id. Missing
    /// state means the default active disposition.
    pub retrieval_dispositions: BTreeMap<String, String>,
}

pub fn build_memory_view_model(input: MemoryViewModelBuildInput) -> MemoryViewModel {
    let review_item_refs = memory_review_item_refs(&input.review_items);
    let lifecycle_summary =
        lifecycle_summary(&input.lifecycle_records, &input.retrieval_dispositions);
    let lane_summaries = lane_summaries(
        &input.lifecycle_records,
        &input.review_items,
        &input.retrieval_dispositions,
    );
    let active_memory_count = input
        .lifecycle_records
        .iter()
        .filter(|record| {
            is_active(record)
                && input
                    .retrieval_dispositions
                    .get(&record.memory_id)
                    .map(String::as_str)
                    .is_none_or(|value| value == "active")
        })
        .count();
    let conflict_count = input
        .lifecycle_records
        .iter()
        .map(|record| record.conflict_ids.len())
        .sum();
    let pending_review_item_proposals = input
        .review_items
        .iter()
        .filter(|item| is_memory_review_item(item) && review_item_requires_action(item))
        .map(|item| item.source.proposal_id.clone())
        .collect::<BTreeSet<_>>();
    let pending_lifecycle_proposals = input
        .lifecycle_records
        .iter()
        .filter(|record| lifecycle_requires_review(record.status))
        .map(|record| record.proposal_id.clone())
        .collect::<BTreeSet<_>>();
    let review_required_count = pending_lifecycle_proposals
        .union(&pending_review_item_proposals)
        .count();
    let recent_memory_refs = input
        .lifecycle_records
        .iter()
        .take(8)
        .map(memory_ref)
        .collect::<Vec<_>>();
    let life_model_linkage = life_model_linkage(&input.lifecycle_records);
    let items = input
        .lifecycle_records
        .iter()
        .map(|record| {
            memory_item(
                record,
                input
                    .retrieval_dispositions
                    .get(&record.memory_id)
                    .map(String::as_str),
            )
        })
        .collect();
    let mut contract_limitations = input.contract_limitations;
    if input.tier_summary.is_some() {
        contract_limitations.push(
            "Vector tier counts are supporting storage telemetry; lifecycle materialization status remains the product memory authority.".into(),
        );
    }
    if !input
        .lifecycle_records
        .iter()
        .any(|record| record.category == MemoryLifecycleCategory::Boundary)
    {
        contract_limitations.push(
            "LifeModel linkage remains partial until lifecycle records explicitly encode all LifeModel truth relations.".into(),
        );
    }

    let summary = MemoryViewModelSummary {
        total_lifecycle_records: input.lifecycle_records.len(),
        active_memory_count,
        review_required_count,
        materialized_count: lifecycle_summary.materialized_count,
        pending_materialization_count: lifecycle_summary.pending_materialization_count,
        failed_materialization_count: lifecycle_summary.materialization_failed_count,
        rolled_back_count: lifecycle_summary.rolled_back_count,
        archived_vector_count: input
            .tier_summary
            .as_ref()
            .map(|summary| summary.archived)
            .unwrap_or(0),
        conflict_count,
        tier_summary: input.tier_summary,
    };

    MemoryViewModel {
        summary,
        lifecycle_summary,
        lane_summaries,
        recent_memory_refs,
        review_item_refs,
        life_model_linkage,
        items,
        source_refs: input.source_refs,
        contract_limitations,
    }
}

fn memory_item(
    record: &MemoryLifecycleRecord,
    retrieval_disposition: Option<&str>,
) -> MemoryItemView {
    let privacy_erased = record.content.is_empty()
        && record.status == MemoryLifecycleStatus::RolledBack
        && record.runtime_context_excluded_at.is_some();
    let canonical_active = is_active(record);
    let recall_state = if privacy_erased {
        "erased"
    } else if record.status.is_terminal_historical() {
        "historical"
    } else if canonical_active && retrieval_disposition == Some("paused") {
        "paused"
    } else if canonical_active && retrieval_disposition == Some("archived") {
        "archived"
    } else if canonical_active {
        "active"
    } else {
        "unavailable"
    };
    MemoryItemView {
        memory_id: record.memory_id.clone(),
        content: (!privacy_erased).then(|| record.content.clone()),
        scope: record.scope.to_string(),
        category: record.category.to_string(),
        status: record.status.to_string(),
        materialization_status: record.materialization_status.to_string(),
        recall_state: recall_state.into(),
        sensitivity: record.sensitivity.to_string(),
        why_remembered: if privacy_erased {
            "The original reason and provenance were removed by privacy erasure.".into()
        } else if record.proposal_id.starts_with("explicit_memory:") {
            "The user explicitly asked OpenLife to remember this in a conversation.".into()
        } else if record.supersedes_memory_id.is_some() {
            "The user approved this correction as a replacement for one exact Memory.".into()
        } else {
            "The user approved a reviewed Memory proposal supported by the listed evidence.".into()
        },
        accepted_at: record.accepted_at.map(|time| time.to_rfc3339()),
        evidence_ids: if privacy_erased {
            Vec::new()
        } else {
            record.evidence_ids.clone()
        },
        supersedes_memory_id: record.supersedes_memory_id.clone(),
        replacement_memory_id: record.replacement_memory_id.clone(),
        privacy_erased,
        can_correct: canonical_active
            && !privacy_erased
            && !matches!(retrieval_disposition, Some("paused" | "archived"))
            && record.category != MemoryLifecycleCategory::Correction,
        can_stop_recall: canonical_active
            && retrieval_disposition != Some("paused")
            && retrieval_disposition != Some("archived"),
        can_archive: canonical_active && retrieval_disposition != Some("archived"),
        can_restore: canonical_active
            && matches!(retrieval_disposition, Some("paused" | "archived")),
        can_rollback: canonical_active && !privacy_erased,
        can_privacy_erase: !privacy_erased,
    }
}

fn lifecycle_summary(
    records: &[MemoryLifecycleRecord],
    retrieval_dispositions: &BTreeMap<String, String>,
) -> MemoryLifecycleSummary {
    let mut summary = MemoryLifecycleSummary {
        archived_count: retrieval_dispositions
            .values()
            .filter(|value| value.as_str() == "archived")
            .count(),
        ..MemoryLifecycleSummary::default()
    };
    for record in records {
        increment(&mut summary.by_status, record.status.as_str());
        increment(
            &mut summary.by_materialization_status,
            record.materialization_status.as_str(),
        );
        match record.status {
            MemoryLifecycleStatus::Candidate => summary.candidate_count += 1,
            MemoryLifecycleStatus::PendingReview => summary.pending_review_count += 1,
            MemoryLifecycleStatus::EditedPendingReview => summary.edited_pending_review_count += 1,
            MemoryLifecycleStatus::Accepted => summary.accepted_count += 1,
            MemoryLifecycleStatus::PendingMaterialization => {
                summary.pending_materialization_count += 1
            }
            MemoryLifecycleStatus::Materialized => summary.materialized_count += 1,
            MemoryLifecycleStatus::MaterializationFailed => {
                summary.materialization_failed_count += 1
            }
            MemoryLifecycleStatus::Rejected => summary.rejected_count += 1,
            MemoryLifecycleStatus::Deferred => summary.deferred_count += 1,
            MemoryLifecycleStatus::Superseded => summary.superseded_count += 1,
            MemoryLifecycleStatus::RolledBack => summary.rolled_back_count += 1,
        }
        if matches!(
            record.status,
            MemoryLifecycleStatus::Accepted
                | MemoryLifecycleStatus::PendingMaterialization
                | MemoryLifecycleStatus::Materialized
        ) {
            summary.confirmed_count += 1;
        }
    }
    summary
}

fn lane_summaries(
    records: &[MemoryLifecycleRecord],
    review_items: &[ReviewItem],
    retrieval_dispositions: &BTreeMap<String, String>,
) -> Vec<MemoryLaneSummary> {
    let review_by_proposal = review_items
        .iter()
        .filter(|item| is_memory_review_item(item))
        .map(|item| (item.source.proposal_id.clone(), review_item_ref(item)))
        .collect::<BTreeMap<_, _>>();
    let mut summaries = all_memory_lanes()
        .into_iter()
        .map(|lane| {
            (
                lane,
                MemoryLaneSummary {
                    lane,
                    label: lane_label(lane).into(),
                    total_count: 0,
                    active_count: 0,
                    candidate_count: 0,
                    pending_review_count: 0,
                    confirmed_count: 0,
                    materialized_count: 0,
                    rolled_back_count: 0,
                    archived_count: 0,
                    review_item_refs: Vec::new(),
                    evidence_refs: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for record in records {
        let lane = lane_for_record(record);
        let Some(summary) = summaries.get_mut(&lane) else {
            continue;
        };
        summary.total_count += 1;
        let hidden_from_recall = retrieval_dispositions
            .get(&record.memory_id)
            .is_some_and(|value| value != "active");
        let archived = retrieval_dispositions
            .get(&record.memory_id)
            .is_some_and(|value| value == "archived");
        if is_active(record) && !hidden_from_recall {
            summary.active_count += 1;
        }
        if archived {
            summary.archived_count += 1;
        }
        if lifecycle_requires_review(record.status) {
            summary.pending_review_count += 1;
        }
        match record.status {
            MemoryLifecycleStatus::Candidate => summary.candidate_count += 1,
            MemoryLifecycleStatus::Materialized => summary.materialized_count += 1,
            MemoryLifecycleStatus::RolledBack => summary.rolled_back_count += 1,
            MemoryLifecycleStatus::Accepted | MemoryLifecycleStatus::PendingMaterialization => {}
            _ => {}
        }
        if matches!(
            record.status,
            MemoryLifecycleStatus::Accepted
                | MemoryLifecycleStatus::PendingMaterialization
                | MemoryLifecycleStatus::Materialized
        ) {
            summary.confirmed_count += 1;
        }
        if let Some(review_ref) = review_by_proposal.get(&record.proposal_id) {
            push_unique_ref(&mut summary.review_item_refs, review_ref.clone());
        }
        for evidence_ref in evidence_refs_for_record(record).into_iter().take(3) {
            push_unique_evidence(&mut summary.evidence_refs, evidence_ref);
        }
    }

    summaries.into_values().collect()
}

fn life_model_linkage(records: &[MemoryLifecycleRecord]) -> MemoryLifeModelLinkageSummary {
    let boundary_records = records
        .iter()
        .filter(|record| record.category == MemoryLifecycleCategory::Boundary)
        .collect::<Vec<_>>();
    let materialized_memory_count = records.iter().filter(|record| is_active(record)).count();
    let candidate_memory_count = records
        .iter()
        .filter(|record| lifecycle_requires_review(record.status))
        .count();
    let conflict_count = records
        .iter()
        .map(|record| record.conflict_ids.len())
        .sum::<usize>();
    MemoryLifeModelLinkageSummary {
        linked_memory_count: materialized_memory_count,
        candidate_memory_count,
        materialized_memory_count,
        conflict_count,
        boundary_memory_count: boundary_records.len(),
        linkage_status: MemoryLifeModelLinkageStatus::Partial,
        memory_refs: boundary_records
            .into_iter()
            .take(8)
            .map(memory_ref)
            .collect(),
        evidence_refs: records
            .iter()
            .flat_map(evidence_refs_for_record)
            .take(8)
            .collect(),
    }
}

fn is_active(record: &MemoryLifecycleRecord) -> bool {
    record.status == MemoryLifecycleStatus::Materialized
        && record.materialization_status == MemoryMaterializationStatus::Materialized
        && record.runtime_context_excluded_at.is_none()
}

fn lifecycle_requires_review(status: MemoryLifecycleStatus) -> bool {
    matches!(
        status,
        MemoryLifecycleStatus::Candidate
            | MemoryLifecycleStatus::PendingReview
            | MemoryLifecycleStatus::EditedPendingReview
    )
}

fn is_memory_review_item(item: &ReviewItem) -> bool {
    matches!(
        item.item_type,
        ReviewItemType::MemoryWrite
            | ReviewItemType::MemoryArchive
            | ReviewItemType::LifeModelUpdate
    )
}

fn review_item_requires_action(item: &ReviewItem) -> bool {
    matches!(
        item.status,
        ReviewItemDecisionStatus::Pending
            | ReviewItemDecisionStatus::Edited
            | ReviewItemDecisionStatus::Deferred
            | ReviewItemDecisionStatus::Unknown
    )
}

fn memory_review_item_refs(items: &[ReviewItem]) -> Vec<BackendEntityRef> {
    let mut refs = Vec::new();
    for item in items.iter().filter(|item| is_memory_review_item(item)) {
        push_unique_ref(&mut refs, review_item_ref(item));
    }
    refs
}

fn review_item_ref(item: &ReviewItem) -> BackendEntityRef {
    BackendEntityRef {
        id: item.id.clone(),
        kind: BackendEntityKind::ReviewItem,
        label: format!("Review item {}", item.id),
        href: Some(format!("/mailbox?review={}", item.id)),
    }
}

fn memory_ref(record: &MemoryLifecycleRecord) -> BackendEntityRef {
    BackendEntityRef {
        id: record.memory_id.clone(),
        kind: BackendEntityKind::Memory,
        label: format!("{} · {}", record.category, record.status),
        href: Some(format!("/memory?memory={}", record.memory_id)),
    }
}

fn evidence_refs_for_record(record: &MemoryLifecycleRecord) -> Vec<EvidenceRef> {
    let mut refs = vec![EvidenceRef {
        id: format!("memory_lifecycle:{}", record.memory_id),
        label: format!("Memory lifecycle {}", record.status),
        source: EvidenceSource::Memory,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    }];
    refs.extend(record.evidence_ids.iter().map(|id| EvidenceRef {
        id: format!("evidence:{id}"),
        label: "Memory evidence".into(),
        source: EvidenceSource::Memory,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    }));
    refs
}

fn lane_for_record(record: &MemoryLifecycleRecord) -> MemoryLane {
    match record.category {
        MemoryLifecycleCategory::Preference => MemoryLane::SemanticFactPreference,
        MemoryLifecycleCategory::Fact => MemoryLane::EpisodicLifeEvent,
        MemoryLifecycleCategory::Workflow => MemoryLane::ProceduralRule,
        MemoryLifecycleCategory::Correction => MemoryLane::EvidenceRecord,
        MemoryLifecycleCategory::Boundary => MemoryLane::CanonicalLifeModelTruth,
    }
}

fn all_memory_lanes() -> Vec<MemoryLane> {
    vec![
        MemoryLane::TurnContext,
        MemoryLane::EpisodicLifeEvent,
        MemoryLane::SemanticFactPreference,
        MemoryLane::ProceduralRule,
        MemoryLane::EvidenceRecord,
        MemoryLane::CanonicalLifeModelTruth,
    ]
}

fn lane_label(lane: MemoryLane) -> &'static str {
    match lane {
        MemoryLane::TurnContext => "Turn context",
        MemoryLane::KnowledgeNote => "Knowledge notes",
        MemoryLane::EpisodicLifeEvent => "Episodic life events",
        MemoryLane::SemanticFactPreference => "Semantic facts and preferences",
        MemoryLane::ProceduralRule => "Procedural rules",
        MemoryLane::EvidenceRecord => "Evidence records",
        MemoryLane::CanonicalLifeModelTruth => "Canonical LifeModel truth",
    }
}

fn increment(map: &mut BTreeMap<String, usize>, key: impl Into<String>) {
    *map.entry(key.into()).or_default() += 1;
}

fn push_unique_ref(refs: &mut Vec<BackendEntityRef>, next: BackendEntityRef) {
    if !refs.iter().any(|existing| existing.id == next.id) {
        refs.push(next);
    }
}

fn push_unique_evidence(refs: &mut Vec<EvidenceRef>, next: EvidenceRef) {
    if !refs.iter().any(|existing| existing.id == next.id) {
        refs.push(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::product_read_model::ViewModelWarningSeverity;
    use crate::agent::review_item::{build_review_item, ReviewCenterBuildInput};
    use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
    use chrono::Utc;
    use serde_json::json;

    fn record(
        id: &str,
        proposal_id: &str,
        category: MemoryLifecycleCategory,
        status: MemoryLifecycleStatus,
        materialization_status: MemoryMaterializationStatus,
    ) -> MemoryLifecycleRecord {
        MemoryLifecycleRecord {
            memory_id: id.into(),
            proposal_id: proposal_id.into(),
            source_task_session_id: None,
            source_run_id: None,
            content: "metadata-safe content".into(),
            scope: crate::agent::memory_lifecycle::MemoryLifecycleScope::Global,
            category,
            risk_level: crate::agent::memory_lifecycle::MemoryLifecycleRiskLevel::Low,
            sensitivity: crate::agent::memory_lifecycle::MemoryLifecycleSensitivity::Internal,
            audit_digest: "sha256:test-memory-view-model".into(),
            status,
            materialization_status,
            materialization_error_code: None,
            created_by: "test".into(),
            accepted_by: None,
            accepted_at: Some(Utc::now()),
            materialized_view_id: None,
            materialized_view_version: None,
            evidence_ids: vec!["evidence-1".into()],
            confidence: 0.8,
            conflict_ids: Vec::new(),
            supersedes_memory_id: None,
            replacement_memory_id: None,
            rolled_back_by_event_id: None,
            runtime_context_excluded_at: None,
        }
    }

    fn review_item(id: &str, proposal_id: &str) -> ReviewItem {
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.candidate",
            json!({ "content": "metadata-safe content" }),
            "test memory proposal",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.id = proposal_id.into();
        let mut item = build_review_item(&proposal, &ReviewCenterBuildInput::default());
        item.id = id.into();
        item
    }

    #[test]
    fn accepted_proposal_is_not_materialized_memory_proof() {
        let model = build_memory_view_model(MemoryViewModelBuildInput {
            lifecycle_records: vec![record(
                "memory:1",
                "proposal:1",
                MemoryLifecycleCategory::Preference,
                MemoryLifecycleStatus::Accepted,
                MemoryMaterializationStatus::Pending,
            )],
            ..MemoryViewModelBuildInput::default()
        });

        assert_eq!(model.lifecycle_summary.accepted_count, 1);
        assert_eq!(model.lifecycle_summary.confirmed_count, 1);
        assert_eq!(model.lifecycle_summary.materialized_count, 0);
        assert_eq!(model.summary.active_memory_count, 0);
    }

    #[test]
    fn tier_stats_support_storage_but_do_not_create_active_memory_truth() {
        let model = build_memory_view_model(MemoryViewModelBuildInput {
            tier_summary: Some(MemoryTierSummary {
                total: 12,
                tier1: 4,
                tier2: 6,
                tier3: 2,
                archived: 3,
            }),
            ..MemoryViewModelBuildInput::default()
        });

        assert_eq!(model.summary.active_memory_count, 0);
        assert_eq!(model.summary.materialized_count, 0);
        assert_eq!(model.summary.archived_vector_count, 3);
        assert!(model
            .contract_limitations
            .iter()
            .any(|limitation| limitation.contains("supporting storage telemetry")));
    }

    #[test]
    fn rolled_back_memory_is_not_active() {
        let mut rolled_back = record(
            "memory:1",
            "proposal:1",
            MemoryLifecycleCategory::Fact,
            MemoryLifecycleStatus::RolledBack,
            MemoryMaterializationStatus::NotRequired,
        );
        rolled_back.runtime_context_excluded_at = Some(Utc::now());

        let model = build_memory_view_model(MemoryViewModelBuildInput {
            lifecycle_records: vec![rolled_back],
            ..MemoryViewModelBuildInput::default()
        });

        assert_eq!(model.lifecycle_summary.rolled_back_count, 1);
        assert_eq!(model.summary.active_memory_count, 0);
    }

    #[test]
    fn review_items_are_linked_by_proposal_id_without_claiming_materialization() {
        let model = build_memory_view_model(MemoryViewModelBuildInput {
            lifecycle_records: vec![record(
                "memory:1",
                "proposal:1",
                MemoryLifecycleCategory::Workflow,
                MemoryLifecycleStatus::PendingReview,
                MemoryMaterializationStatus::Pending,
            )],
            review_items: vec![review_item("review:1", "proposal:1")],
            ..MemoryViewModelBuildInput::default()
        });

        let workflow = model
            .lane_summaries
            .iter()
            .find(|summary| summary.lane == MemoryLane::ProceduralRule)
            .expect("workflow lane summary");
        assert_eq!(workflow.review_item_refs[0].id, "review:1");
        assert_eq!(model.summary.review_required_count, 1);
        assert_eq!(model.summary.materialized_count, 0);
    }

    #[test]
    fn materialized_lifecycle_record_counts_as_active_memory() {
        let model = build_memory_view_model(MemoryViewModelBuildInput {
            lifecycle_records: vec![record(
                "memory:1",
                "proposal:1",
                MemoryLifecycleCategory::Preference,
                MemoryLifecycleStatus::Materialized,
                MemoryMaterializationStatus::Materialized,
            )],
            ..MemoryViewModelBuildInput::default()
        });

        assert_eq!(model.lifecycle_summary.materialized_count, 1);
        assert_eq!(model.summary.active_memory_count, 1);
        assert_eq!(model.life_model_linkage.materialized_memory_count, 1);
    }

    #[test]
    fn exported_types_remain_warning_compatible() {
        let _ = ViewModelWarningSeverity::Info;
    }
}
