use crate::agent::memory_lifecycle::{
    MemoryLifecycleCategory, MemoryLifecycleRecord, MemoryLifecycleStatus,
    MemoryMaterializationStatus,
};
use crate::agent::product_read_model::{EvidenceRef, EvidenceSensitivity, EvidenceSource};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MemoryViewModelSummary {
    pub total_memory_count: usize,
    pub active_memory_count: usize,
    pub archived_memory_count: usize,
    pub historical_memory_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItemView {
    pub memory_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub scope: String,
    pub category: String,
    pub recall_state: String,
    pub why_remembered: String,
    pub recall_explanation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_at: Option<String>,
    #[serde(default)]
    pub source_refs: Vec<EvidenceRef>,
    pub privacy_erased: bool,
    pub can_correct: bool,
    pub can_archive: bool,
    pub can_restore: bool,
    pub can_rollback: bool,
    pub can_privacy_erase: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryViewModel {
    pub summary: MemoryViewModelSummary,
    pub items: Vec<MemoryItemView>,
    pub source_refs: Vec<EvidenceRef>,
    pub contract_limitations: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryViewModelBuildInput {
    pub lifecycle_records: Vec<MemoryLifecycleRecord>,
    pub source_refs: Vec<EvidenceRef>,
    pub contract_limitations: Vec<String>,
    /// Canonical retrieval disposition keyed by lifecycle memory id. Missing
    /// state means the default active disposition.
    pub retrieval_dispositions: BTreeMap<String, String>,
}

pub fn build_memory_view_model(input: MemoryViewModelBuildInput) -> MemoryViewModel {
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
    let archived_memory_count = input
        .lifecycle_records
        .iter()
        .filter(|record| {
            input
                .retrieval_dispositions
                .get(&record.memory_id)
                .map(String::as_str)
                == Some("archived")
        })
        .count();
    let historical_memory_count = input
        .lifecycle_records
        .iter()
        .filter(|record| record.status.is_terminal_historical())
        .count();
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
    let summary = MemoryViewModelSummary {
        total_memory_count: input.lifecycle_records.len(),
        active_memory_count,
        archived_memory_count,
        historical_memory_count,
    };

    MemoryViewModel {
        summary,
        items,
        source_refs: input.source_refs,
        contract_limitations: input.contract_limitations,
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
        recall_state: recall_state.into(),
        why_remembered: if privacy_erased {
            "The original reason and provenance were removed by privacy erasure.".into()
        } else if record.proposal_id.starts_with("explicit_memory:") {
            "The user explicitly asked OpenLife to remember this in a conversation.".into()
        } else if record.supersedes_memory_id.is_some() {
            "The user approved this correction as a replacement for one exact Memory.".into()
        } else {
            "The user approved a reviewed Memory proposal supported by the listed evidence.".into()
        },
        recall_explanation: match recall_state {
            "active" => "This Memory is eligible only when the current task and exact scope match; hybrid retrieval still reranks it for every turn.".into(),
            "paused" => "This Memory is retained but excluded from normal runtime recall until the user restores it.".into(),
            "archived" => "This Memory is archived and excluded from normal runtime recall until the user restores it.".into(),
            "erased" => "The original content can no longer be recalled because it was privacy-erased.".into(),
            _ => "This historical Memory is not eligible for normal runtime recall.".into(),
        },
        accepted_at: record.accepted_at.map(|time| time.to_rfc3339()),
        source_refs: if privacy_erased {
            vec![memory_lifecycle_evidence_ref(record)]
        } else {
            evidence_refs_for_record(record)
        },
        privacy_erased,
        can_correct: canonical_active
            && !privacy_erased
            && !matches!(retrieval_disposition, Some("paused" | "archived"))
            && record.category != MemoryLifecycleCategory::Correction,
        can_archive: canonical_active && retrieval_disposition != Some("archived"),
        can_restore: canonical_active
            && matches!(retrieval_disposition, Some("paused" | "archived")),
        can_rollback: canonical_active && !privacy_erased,
        can_privacy_erase: !privacy_erased,
    }
}

fn is_active(record: &MemoryLifecycleRecord) -> bool {
    record.status == MemoryLifecycleStatus::Materialized
        && record.materialization_status == MemoryMaterializationStatus::Materialized
        && record.runtime_context_excluded_at.is_none()
}

fn evidence_refs_for_record(record: &MemoryLifecycleRecord) -> Vec<EvidenceRef> {
    let mut refs = vec![memory_lifecycle_evidence_ref(record)];
    refs.extend(record.evidence_ids.iter().map(|id| EvidenceRef {
        id: format!("evidence:{id}"),
        label: "Memory evidence".into(),
        source: EvidenceSource::Memory,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    }));
    refs
}

fn memory_lifecycle_evidence_ref(record: &MemoryLifecycleRecord) -> EvidenceRef {
    EvidenceRef {
        id: format!("memory_lifecycle:{}", record.memory_id),
        label: format!("Memory lifecycle {}", record.status),
        source: EvidenceSource::Memory,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::product_read_model::ViewModelWarningSeverity;
    use chrono::Utc;

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
            source_task_id: None,
            source_run_id: None,
            content: "metadata-safe content".into(),
            scope: crate::agent::memory_lifecycle::MemoryLifecycleScope::Global,
            scope_owner_ref: None,
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

        assert_eq!(model.summary.total_memory_count, 1);
        assert_eq!(model.summary.active_memory_count, 0);
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

        assert_eq!(model.summary.active_memory_count, 0);
        assert_eq!(model.summary.historical_memory_count, 1);
    }

    #[test]
    fn privacy_erased_memory_does_not_expose_prior_evidence_ids() {
        let mut erased = record(
            "memory:1",
            "proposal:1",
            MemoryLifecycleCategory::Fact,
            MemoryLifecycleStatus::RolledBack,
            MemoryMaterializationStatus::NotRequired,
        );
        erased.content.clear();
        erased.runtime_context_excluded_at = Some(Utc::now());

        let model = build_memory_view_model(MemoryViewModelBuildInput {
            lifecycle_records: vec![erased],
            ..MemoryViewModelBuildInput::default()
        });

        assert!(model.items[0].privacy_erased);
        assert_eq!(model.items[0].source_refs.len(), 1);
        assert_eq!(
            model.items[0].source_refs[0].id,
            "memory_lifecycle:memory:1"
        );
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

        assert_eq!(model.summary.total_memory_count, 1);
        assert_eq!(model.summary.active_memory_count, 1);
        assert!(model.items[0].recall_explanation.contains("exact scope"));
        assert_eq!(
            model.items[0].source_refs[0].id,
            "memory_lifecycle:memory:1"
        );
        assert!(model.items[0]
            .source_refs
            .iter()
            .any(|source| source.id == "evidence:evidence-1"));
    }

    #[test]
    fn exported_types_remain_warning_compatible() {
        let _ = ViewModelWarningSeverity::Info;
    }
}
