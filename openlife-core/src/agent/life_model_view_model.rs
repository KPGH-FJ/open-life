use crate::agent::product_read_model::{
    BackendEntityKind, BackendEntityRef, DebugAction, DebugActionKind, EvidenceRef,
    EvidenceSensitivity, EvidenceSource, ProductAction, ProductActionKind,
    ReviewItemMaterializationStatus, ViewModelActions, ViewModelEnvelope, ViewModelStatus,
    ViewModelWarning, ViewModelWarningSeverity,
};
use crate::agent::review_item::{ReviewItem, ReviewItemType};
use crate::agent::types::{AgentProposal, ProposalStatus, ProposalType};
use crate::agent::LifeModelLearningCandidate;
use crate::life_model::v2::{
    LegacyLifeModelMigrationPreviewV2, LifeModelDocumentV2, LifeModelHumanProjectionV2,
    LifeModelVersionHistoryEntryV2,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const LIFE_MODEL_TARGET_REF: &str = "lifemodel";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelTruthMode {
    Canonical,
    Candidate,
    PendingReview,
    ManualOverride,
    Unknown,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelOwnerStatus {
    #[serde(rename = "PARTIAL")]
    Partial,
    #[serde(rename = "PHASE_2_REQUIRED")]
    Phase2Required,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelReadiness {
    NotBuilt,
    Limited,
    UsableWithLimits,
    Ready,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelChangeKind {
    Add,
    Update,
    Remove,
    Merge,
    ManualOverride,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelCandidateDecisionStatus {
    Pending,
    Accepted,
    Edited,
    Postponed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelMemoryLinkageStatus {
    Partial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelCanonicalSummary {
    pub life_model_ref: BackendEntityRef,
    pub title: String,
    pub summary: String,
    pub version_label: String,
    pub parent_version: Option<u64>,
    pub document_digest: String,
    pub last_materialized_at: Option<String>,
    pub freshness_status: String,
    pub conflict_status: String,
    pub evidence_refs: Vec<EvidenceRef>,
    pub document: LifeModelDocumentV2,
    pub human_projection: LifeModelHumanProjectionV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelTrustQualityState {
    pub readiness: LifeModelReadiness,
    pub warning_refs: Vec<EvidenceRef>,
    pub owner_status: LifeModelOwnerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelPendingUpdateCounts {
    pub candidate: usize,
    pub pending_review: usize,
    pub approved_not_applied: usize,
    pub failed_materialization: usize,
    pub owner_status: LifeModelOwnerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelCandidateChange {
    pub change_ref: BackendEntityRef,
    pub title: String,
    pub change_kind: LifeModelChangeKind,
    pub affected_dimension_ids: Vec<String>,
    pub review_item_refs: Vec<BackendEntityRef>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub decision_status: LifeModelCandidateDecisionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelMaterializedChange {
    pub change_ref: BackendEntityRef,
    pub title: String,
    pub materialization_status: ReviewItemMaterializationStatus,
    pub materialized_at: Option<String>,
    pub rollback_available: bool,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelManualOverrideState {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    pub draft_ref: Option<BackendEntityRef>,
    pub save_action: Option<ProductAction>,
    pub review_item_refs: Vec<BackendEntityRef>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub owner_status: LifeModelOwnerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelTierSummary {
    pub total: Option<usize>,
    pub tier1: Option<usize>,
    pub tier2: Option<usize>,
    pub tier3: Option<usize>,
    pub archived: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelMemoryLinkageSummary {
    pub linked_memory_count: usize,
    pub candidate_memory_count: usize,
    pub materialized_memory_count: usize,
    pub conflict_count: usize,
    pub memory_refs: Vec<BackendEntityRef>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub linkage_status: LifeModelMemoryLinkageStatus,
    pub tier_summary: LifeModelTierSummary,
    pub owner_status: LifeModelOwnerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelLearningSummary {
    pub available: bool,
    pub active_count: usize,
    pub candidates: Vec<LifeModelLearningCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelViewModel {
    pub truth_mode: LifeModelTruthMode,
    pub canonical_summary: Option<LifeModelCanonicalSummary>,
    pub version_history: Vec<LifeModelVersionHistoryEntryV2>,
    pub legacy_migration_preview: Option<LegacyLifeModelMigrationPreviewV2>,
    pub trust_quality_state: LifeModelTrustQualityState,
    pub pending_update_counts: LifeModelPendingUpdateCounts,
    pub provenance_refs: Vec<EvidenceRef>,
    pub candidate_changes: Vec<LifeModelCandidateChange>,
    pub materialized_changes: Vec<LifeModelMaterializedChange>,
    pub manual_override_state: Option<LifeModelManualOverrideState>,
    pub related_review_item_refs: Vec<BackendEntityRef>,
    pub memory_linkage: LifeModelMemoryLinkageSummary,
    pub learning: LifeModelLearningSummary,
    pub source_refs: Vec<EvidenceRef>,
    pub contract_limitations: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LifeModelProjectionInput {
    pub generated_at: Option<String>,
    pub safe_mode_active: bool,
    pub safe_mode_reason: Option<String>,
    pub life_model_ready: bool,
    pub model_empty: bool,
    pub readiness_issues: Vec<String>,
    pub usage_readiness_issues: Vec<String>,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LifeModelMemoryTierStatsInput {
    pub total: usize,
    pub tier1: usize,
    pub tier2: usize,
    pub tier3: usize,
    pub archived: usize,
}

#[derive(Debug, Clone, Default)]
pub struct LifeModelCanonicalV2Input {
    pub model_id: String,
    pub schema_version: String,
    pub model_version: u64,
    pub parent_version: Option<u64>,
    pub document_digest: String,
    pub summary: String,
    pub item_count: usize,
    pub updated_at: Option<String>,
    pub source_refs: Vec<String>,
    pub document: Option<LifeModelDocumentV2>,
    pub human_projection: LifeModelHumanProjectionV2,
}

#[derive(Debug, Clone, Default)]
pub struct LifeModelViewModelBuildInput {
    pub canonical_v2: Option<LifeModelCanonicalV2Input>,
    pub version_history: Vec<LifeModelVersionHistoryEntryV2>,
    /// A fresh profile with neither legacy YAML nor a persisted v2 head has a
    /// canonical empty owner without creating storage as a read side effect.
    pub fresh_profile_canonical_empty: bool,
    pub legacy_migration_preview: Option<LegacyLifeModelMigrationPreviewV2>,
    pub projection: Option<LifeModelProjectionInput>,
    pub proposals: Vec<AgentProposal>,
    pub review_items: Vec<ReviewItem>,
    pub memory_count: Option<usize>,
    pub tier_stats: Option<LifeModelMemoryTierStatsInput>,
    pub learning_available: bool,
    pub learning_candidates: Vec<LifeModelLearningCandidate>,
    pub now: Option<String>,
    pub stale: bool,
    pub error: Option<String>,
}

pub fn build_life_model_view_model_envelope(
    input: LifeModelViewModelBuildInput,
) -> ViewModelEnvelope<LifeModelViewModel> {
    let now = input.now.clone().or_else(|| {
        input
            .projection
            .as_ref()
            .and_then(|projection| projection.generated_at.clone())
    });

    if let Some(error) = input.error.as_ref() {
        let mut envelope = ViewModelEnvelope::backend_read_model(ViewModelStatus::Error, None);
        envelope.last_updated_at = now;
        envelope.warnings.push(warning(
            "lifemodel.load_error",
            format!(
                "LifeModelViewModel could not load required backend state; no raw LifeModel fallback was used: {error}"
            ),
            ViewModelWarningSeverity::Error,
            Vec::new(),
        ));
        envelope.actions.primary.push(refresh_action(true));
        return envelope;
    }

    let source_refs = collect_source_refs(&input);
    let life_model_proposals = input
        .proposals
        .iter()
        .filter(|proposal| is_life_model_proposal(proposal))
        .cloned()
        .collect::<Vec<_>>();
    let life_model_review_items = input
        .review_items
        .iter()
        .filter(|item| item.item_type == ReviewItemType::LifeModelUpdate)
        .cloned()
        .collect::<Vec<_>>();
    let materialized_proposal_ids = materialized_lifemodel_proposal_ids(&life_model_review_items);
    let failed_materialization_ids = failed_lifemodel_proposal_ids(&life_model_review_items);
    let valid_canonical_version = input
        .canonical_v2
        .as_ref()
        .is_some_and(canonical_v2_input_is_authoritative);
    let canonical_owner = valid_canonical_version || input.fresh_profile_canonical_empty;
    let status = loaded_status(&input, valid_canonical_version);
    let materialized_changes = build_materialized_changes(
        &life_model_proposals,
        &life_model_review_items,
        &materialized_proposal_ids,
    );
    let pending_update_counts = build_pending_update_counts(
        &life_model_proposals,
        &materialized_proposal_ids,
        &failed_materialization_ids,
    );
    let related_review_item_refs = life_model_review_items
        .iter()
        .map(review_item_ref)
        .collect::<Vec<_>>();
    let warnings = build_warnings(
        status,
        &input,
        &source_refs,
        pending_update_counts.approved_not_applied,
    );
    let risky_action_blocker = risky_action_blocker(&input, status);

    let data = LifeModelViewModel {
        truth_mode: derive_truth_mode(status, canonical_owner),
        canonical_summary: build_canonical_summary(&input, valid_canonical_version),
        version_history: if canonical_owner {
            input.version_history.clone()
        } else {
            Vec::new()
        },
        legacy_migration_preview: if canonical_owner {
            None
        } else {
            input.legacy_migration_preview.clone()
        },
        trust_quality_state: build_trust_quality_state(status, &input, &source_refs),
        pending_update_counts,
        provenance_refs: source_refs.clone(),
        candidate_changes: build_candidate_changes(&life_model_proposals),
        materialized_changes,
        manual_override_state: Some(build_manual_override_state(&source_refs)),
        related_review_item_refs,
        memory_linkage: build_memory_linkage(&input, &source_refs),
        learning: LifeModelLearningSummary {
            available: input.learning_available,
            active_count: input.learning_candidates.len(),
            candidates: input.learning_candidates.clone(),
        },
        source_refs: source_refs.clone(),
        contract_limitations: build_contract_limitations(canonical_owner),
    };

    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(data));
    envelope.last_updated_at = now;
    envelope.evidence_refs = source_refs.clone();
    envelope.warnings = warnings;
    envelope.actions = ViewModelActions {
        primary: vec![
            refresh_action(true),
            inspect_evidence_action(status != ViewModelStatus::Stale),
            request_update_action(risky_action_blocker),
        ],
        review: Vec::new(),
        debug_only: if status == ViewModelStatus::Empty {
            Vec::new()
        } else {
            vec![DebugAction {
                id: "lifemodel.inspect_backend_read_model".into(),
                label: "Inspect LifeModel read model".into(),
                kind: DebugActionKind::RawJson,
                enabled: status != ViewModelStatus::Stale,
                developer_only: true,
                target_ref: Some("LifeModelViewModel.backendInputRefs".into()),
            }]
        },
    };
    envelope
}

fn loaded_status(
    input: &LifeModelViewModelBuildInput,
    meaningful_canonical: bool,
) -> ViewModelStatus {
    if input.stale {
        return ViewModelStatus::Stale;
    }
    if meaningful_canonical
        && input
            .canonical_v2
            .as_ref()
            .is_some_and(|canonical| canonical.item_count == 0)
    {
        return ViewModelStatus::Empty;
    }
    if input.legacy_migration_preview.is_some() {
        return ViewModelStatus::Ready;
    }
    if !meaningful_canonical {
        return ViewModelStatus::Empty;
    }
    ViewModelStatus::Ready
}

fn derive_truth_mode(status: ViewModelStatus, meaningful_canonical: bool) -> LifeModelTruthMode {
    if matches!(status, ViewModelStatus::Error) {
        return LifeModelTruthMode::Unavailable;
    }
    if meaningful_canonical {
        return LifeModelTruthMode::Canonical;
    }
    LifeModelTruthMode::Unknown
}

fn build_canonical_summary(
    input: &LifeModelViewModelBuildInput,
    meaningful_canonical: bool,
) -> Option<LifeModelCanonicalSummary> {
    let canonical = input
        .canonical_v2
        .as_ref()
        .filter(|_| meaningful_canonical)?;
    Some(LifeModelCanonicalSummary {
        life_model_ref: BackendEntityRef {
            id: format!(
                "lifemodel-v2:{}:{}",
                canonical.model_id, canonical.model_version
            ),
            kind: BackendEntityKind::LifeModel,
            label: "Canonical LifeModel v2".into(),
            href: None,
        },
        title: "已确认的长期个人模型".into(),
        summary: canonical.summary.clone(),
        version_label: format!(
            "{} · version {}",
            canonical.schema_version, canonical.model_version
        ),
        parent_version: canonical.parent_version,
        document_digest: canonical.document_digest.clone(),
        last_materialized_at: canonical.updated_at.clone(),
        freshness_status: "current".into(),
        conflict_status: "none".into(),
        evidence_refs: canonical
            .source_refs
            .iter()
            .map(|source_ref| EvidenceRef {
                id: format!("lifemodel-v2-source:{source_ref}"),
                label: source_ref.clone(),
                source: EvidenceSource::LifeModel,
                sensitivity: Some(EvidenceSensitivity::LocalPrivate),
            })
            .collect(),
        document: canonical.document.clone()?,
        human_projection: canonical.human_projection.clone(),
    })
}

fn canonical_v2_input_is_authoritative(canonical: &LifeModelCanonicalV2Input) -> bool {
    canonical.item_count == canonical.human_projection.item_count
        && canonical.document.as_ref().is_some_and(|document| {
            document.model_id == canonical.model_id
                && document.total_item_count() == canonical.item_count
                && document
                    .digest()
                    .is_ok_and(|digest| digest == canonical.document_digest)
        })
        && canonical
            .human_projection
            .validate_binding(
                &canonical.model_id,
                canonical.model_version,
                &canonical.document_digest,
            )
            .is_ok()
}

fn build_contract_limitations(authoritative_canonical: bool) -> Vec<String> {
    let mut limitations = vec![
        "Accepted proposal decisions remain approved-not-applied unless the canonical materializer proves an exact committed version.".into(),
        "Memory remains a separate owner and is not copied into the canonical LifeModel document.".into(),
        "Manual overrides are governed and separate from proposal-first review materialization.".into(),
    ];
    if !authoritative_canonical {
        limitations.insert(
            0,
            "No valid structured LifeModel v2 version is available; legacy YAML remains a compatibility owner until governed migration.".into(),
        );
    }
    limitations
}

fn build_trust_quality_state(
    status: ViewModelStatus,
    input: &LifeModelViewModelBuildInput,
    source_refs: &[EvidenceRef],
) -> LifeModelTrustQualityState {
    let readiness = if status == ViewModelStatus::Stale {
        LifeModelReadiness::Stale
    } else if status == ViewModelStatus::Empty {
        LifeModelReadiness::NotBuilt
    } else if input.projection.as_ref().is_some_and(|projection| {
        !projection.life_model_ready
            || !projection.readiness_issues.is_empty()
            || !projection.usage_readiness_issues.is_empty()
    }) {
        LifeModelReadiness::Limited
    } else {
        LifeModelReadiness::UsableWithLimits
    };

    LifeModelTrustQualityState {
        readiness,
        warning_refs: source_refs.to_vec(),
        owner_status: LifeModelOwnerStatus::Partial,
    }
}

fn build_pending_update_counts(
    proposals: &[AgentProposal],
    materialized_proposal_ids: &BTreeSet<String>,
    failed_materialization_ids: &BTreeSet<String>,
) -> LifeModelPendingUpdateCounts {
    let candidate = proposals
        .iter()
        .filter(|proposal| {
            matches!(
                proposal.status,
                ProposalStatus::Pending | ProposalStatus::Edited | ProposalStatus::Postponed
            )
        })
        .count();
    let approved_not_applied = proposals
        .iter()
        .filter(|proposal| proposal.status == ProposalStatus::Accepted)
        .filter(|proposal| !materialized_proposal_ids.contains(&proposal.id))
        .filter(|proposal| !failed_materialization_ids.contains(&proposal.id))
        .count();
    LifeModelPendingUpdateCounts {
        candidate,
        pending_review: candidate,
        approved_not_applied,
        failed_materialization: failed_materialization_ids.len(),
        owner_status: LifeModelOwnerStatus::Partial,
    }
}

fn build_candidate_changes(proposals: &[AgentProposal]) -> Vec<LifeModelCandidateChange> {
    proposals
        .iter()
        .filter(|proposal| {
            matches!(
                proposal.status,
                ProposalStatus::Pending | ProposalStatus::Edited | ProposalStatus::Postponed
            )
        })
        .map(|proposal| LifeModelCandidateChange {
            change_ref: BackendEntityRef {
                id: format!("proposal:{}", proposal.id),
                kind: BackendEntityKind::Proposal,
                label: title_for_proposal(proposal),
                href: None,
            },
            title: title_for_proposal(proposal),
            change_kind: change_kind_from_proposal(proposal),
            affected_dimension_ids: affected_dimension_ids(proposal),
            review_item_refs: vec![review_item_ref_from_proposal(proposal)],
            evidence_refs: vec![evidence_ref_from_proposal(proposal)],
            decision_status: decision_status_from_proposal(proposal.status),
        })
        .collect()
}

fn build_materialized_changes(
    proposals: &[AgentProposal],
    review_items: &[ReviewItem],
    materialized_proposal_ids: &BTreeSet<String>,
) -> Vec<LifeModelMaterializedChange> {
    let mut changes = Vec::new();
    for proposal_id in materialized_proposal_ids {
        let proposal = proposals
            .iter()
            .find(|proposal| &proposal.id == proposal_id);
        let review_item = review_items
            .iter()
            .find(|item| &item.source.proposal_id == proposal_id);
        let mut evidence_refs = Vec::new();
        if let Some(proposal) = proposal {
            evidence_refs.push(evidence_ref_from_proposal(proposal));
        }
        if let Some(item) = review_item {
            evidence_refs.extend(item.evidence_refs.clone());
        }
        changes.push(LifeModelMaterializedChange {
            change_ref: BackendEntityRef {
                id: format!("proposal:{proposal_id}"),
                kind: BackendEntityKind::Proposal,
                label: proposal
                    .map(title_for_proposal)
                    .unwrap_or_else(|| "Materialized LifeModel change".into()),
                href: None,
            },
            title: proposal
                .map(title_for_proposal)
                .unwrap_or_else(|| "Materialized LifeModel change".into()),
            materialization_status: ReviewItemMaterializationStatus::Applied,
            materialized_at: None,
            rollback_available: false,
            evidence_refs: dedupe_evidence_refs(evidence_refs),
        });
    }
    changes
}

fn build_manual_override_state(source_refs: &[EvidenceRef]) -> LifeModelManualOverrideState {
    LifeModelManualOverrideState {
        active: false,
        blocked_reason: Some(
            "Whole-model manual saves are not exposed by the product. LifeModel changes must use the proposal-first review flow.".into(),
        ),
        draft_ref: None,
        save_action: Some(ProductAction {
            id: "lifemodel.manual_override.save".into(),
            label: "Save manual override".into(),
            kind: ProductActionKind::Configure,
            enabled: false,
            disabled_reason: Some(
                "No native-confirmed whole-model editor exists; use reviewable proposals instead."
                    .into(),
            ),
            target_ref: Some(LIFE_MODEL_TARGET_REF.into()),
        }),
        review_item_refs: Vec::new(),
        evidence_refs: source_refs.to_vec(),
        owner_status: LifeModelOwnerStatus::Partial,
    }
}

fn build_memory_linkage(
    input: &LifeModelViewModelBuildInput,
    source_refs: &[EvidenceRef],
) -> LifeModelMemoryLinkageSummary {
    let tier_summary = LifeModelTierSummary {
        total: input.tier_stats.as_ref().map(|stats| stats.total),
        tier1: input.tier_stats.as_ref().map(|stats| stats.tier1),
        tier2: input.tier_stats.as_ref().map(|stats| stats.tier2),
        tier3: input.tier_stats.as_ref().map(|stats| stats.tier3),
        archived: input.tier_stats.as_ref().map(|stats| stats.archived),
    };
    let linked_memory_count = input
        .memory_count
        .or(tier_summary.total)
        .unwrap_or_default();
    let linkage_status = if input.memory_count.is_some() || input.tier_stats.is_some() {
        LifeModelMemoryLinkageStatus::Partial
    } else {
        LifeModelMemoryLinkageStatus::Unknown
    };
    LifeModelMemoryLinkageSummary {
        linked_memory_count,
        candidate_memory_count: 0,
        materialized_memory_count: 0,
        conflict_count: 0,
        memory_refs: Vec::new(),
        evidence_refs: source_refs
            .iter()
            .filter(|evidence| evidence.source == EvidenceSource::Memory)
            .cloned()
            .collect(),
        linkage_status,
        tier_summary,
        owner_status: LifeModelOwnerStatus::Partial,
    }
}

fn build_warnings(
    status: ViewModelStatus,
    input: &LifeModelViewModelBuildInput,
    source_refs: &[EvidenceRef],
    approved_not_applied_count: usize,
) -> Vec<ViewModelWarning> {
    let mut warnings = vec![
        warning(
            "lifemodel.materialization_fail_closed",
            "Accepted LifeModel proposals are decision state only unless an exact canonical version commit proves materialization.",
            ViewModelWarningSeverity::Warning,
            source_refs.to_vec(),
        ),
        warning(
            "lifemodel.memory_linkage_limited",
            "Memory linkage is a compatibility count only; MemoryViewModel remains the separate owner and its contents are not canonical LifeModel truth.",
            ViewModelWarningSeverity::Info,
            source_refs.to_vec(),
        ),
    ];
    if !input.fresh_profile_canonical_empty
        && input
            .canonical_v2
            .as_ref()
            .is_none_or(|canonical| !canonical_v2_input_is_authoritative(canonical))
    {
        warnings.push(warning(
            "lifemodel.canonical_summary_unavailable",
            "No valid structured LifeModel v2 version is available; canonical truth is not inferred from compatibility data.",
            ViewModelWarningSeverity::Info,
            source_refs.to_vec(),
        ));
    }
    if status == ViewModelStatus::Empty {
        let authoritative_empty = input.canonical_v2.as_ref().is_some_and(|canonical| {
            canonical_v2_input_is_authoritative(canonical) && canonical.item_count == 0
        });
        warnings.push(warning(
            "lifemodel.empty",
            if authoritative_empty {
                "The canonical LifeModel v2 owner is intentionally empty; legacy compatibility data is not substituted."
            } else {
                "No confirmed LifeModel content was found; no fake canonical summary was generated."
            },
            ViewModelWarningSeverity::Info,
            source_refs.to_vec(),
        ));
    }
    if status == ViewModelStatus::Stale {
        warnings.push(warning(
            "lifemodel.stale",
            "LifeModelViewModel data is stale; risky actions are disabled until refresh.",
            ViewModelWarningSeverity::Warning,
            source_refs.to_vec(),
        ));
    }
    if input
        .projection
        .as_ref()
        .is_some_and(|projection| projection.safe_mode_active)
    {
        warnings.push(warning(
            "lifemodel.safe_mode",
            "LifeStateProjection reports Safe Mode; risky LifeModel actions are disabled.",
            ViewModelWarningSeverity::Warning,
            source_refs.to_vec(),
        ));
    }
    if approved_not_applied_count > 0 {
        warnings.push(warning(
            "lifemodel.approved_not_applied",
            "One or more accepted LifeModel proposals lack backend materialization proof and remain approved-not-applied.",
            ViewModelWarningSeverity::Warning,
            source_refs.to_vec(),
        ));
    }
    warnings
}

fn risky_action_blocker(
    input: &LifeModelViewModelBuildInput,
    status: ViewModelStatus,
) -> Option<String> {
    if status == ViewModelStatus::Stale {
        return Some("Refresh LifeModel state before using this action.".into());
    }
    if input
        .projection
        .as_ref()
        .is_some_and(|projection| projection.safe_mode_active)
    {
        return Some(
            input
                .projection
                .as_ref()
                .and_then(|projection| projection.safe_mode_reason.clone())
                .unwrap_or_else(|| "LifeStateProjection reports Safe Mode.".into()),
        );
    }
    Some("LifeModel update requests must go through Review Center proposal flow.".into())
}

fn refresh_action(enabled: bool) -> ProductAction {
    ProductAction {
        id: "lifemodel.refresh".into(),
        label: "Refresh LifeModel state".into(),
        kind: ProductActionKind::Refresh,
        enabled,
        disabled_reason: if enabled {
            None
        } else {
            Some("LifeModel state is still loading.".into())
        },
        target_ref: Some(LIFE_MODEL_TARGET_REF.into()),
    }
}

fn inspect_evidence_action(enabled: bool) -> ProductAction {
    ProductAction {
        id: "lifemodel.inspect_evidence".into(),
        label: "Inspect LifeModel evidence".into(),
        kind: ProductActionKind::Inspect,
        enabled,
        disabled_reason: if enabled {
            None
        } else {
            Some("Refresh LifeModel state before inspecting evidence.".into())
        },
        target_ref: Some("lifemodel:evidence".into()),
    }
}

fn request_update_action(disabled_reason: Option<String>) -> ProductAction {
    ProductAction {
        id: "lifemodel.request_update".into(),
        label: "Request LifeModel update".into(),
        kind: ProductActionKind::Start,
        enabled: disabled_reason.is_none(),
        disabled_reason,
        target_ref: Some(LIFE_MODEL_TARGET_REF.into()),
    }
}

fn collect_source_refs(input: &LifeModelViewModelBuildInput) -> Vec<EvidenceRef> {
    let mut refs = Vec::new();
    if let Some(canonical) = input
        .canonical_v2
        .as_ref()
        .filter(|canonical| canonical_v2_input_is_authoritative(canonical))
    {
        refs.push(EvidenceRef {
            id: format!(
                "lifemodel-v2:{}:{}:{}",
                canonical.model_id, canonical.model_version, canonical.document_digest
            ),
            label: format!("Canonical LifeModel v2 version {}", canonical.model_version),
            source: EvidenceSource::LifeModel,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        });
        refs.extend(canonical.source_refs.iter().map(|source_ref| EvidenceRef {
            id: format!("lifemodel-v2-source:{source_ref}"),
            label: source_ref.clone(),
            source: EvidenceSource::LifeModel,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        }));
    }
    if let Some(projection) = &input.projection {
        if projection.source_refs.is_empty() {
            refs.push(EvidenceRef {
                id: "projection:LifeStateProjection".into(),
                label: "LifeStateProjection".into(),
                source: EvidenceSource::BackendReadModel,
                sensitivity: Some(EvidenceSensitivity::LocalPrivate),
            });
        } else {
            refs.extend(
                projection
                    .source_refs
                    .iter()
                    .enumerate()
                    .map(|(index, source_ref)| EvidenceRef {
                        id: format!("projection:{index}:{source_ref}"),
                        label: source_ref.clone(),
                        source: EvidenceSource::BackendReadModel,
                        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
                    }),
            );
        }
    }
    if input.memory_count.is_some() {
        refs.push(EvidenceRef {
            id: "memory:count".into(),
            label: "Memory count".into(),
            source: EvidenceSource::Memory,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        });
    }
    if input.tier_stats.is_some() {
        refs.push(EvidenceRef {
            id: "memory:tier-stats".into(),
            label: "Memory tier stats".into(),
            source: EvidenceSource::Memory,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        });
    }
    refs.extend(
        input
            .proposals
            .iter()
            .filter(|proposal| is_life_model_proposal(proposal))
            .map(evidence_ref_from_proposal),
    );
    dedupe_evidence_refs(refs)
}

fn warning(
    code: impl Into<String>,
    message: impl Into<String>,
    severity: ViewModelWarningSeverity,
    evidence_refs: Vec<EvidenceRef>,
) -> ViewModelWarning {
    ViewModelWarning {
        code: code.into(),
        message: message.into(),
        severity,
        evidence_refs,
    }
}

fn is_life_model_proposal(proposal: &AgentProposal) -> bool {
    proposal.proposal_type == ProposalType::LifeModelUpdate
        || matches!(
            proposal.proposal_type,
            ProposalType::GoalUpdate
                | ProposalType::StateUpdate
                | ProposalType::PreferenceUpdate
                | ProposalType::CapabilityUpdate
        )
        || affected_dimension_ids(proposal)
            .iter()
            .any(|id| id != "unknown")
}

fn affected_dimension_ids(proposal: &AgentProposal) -> Vec<String> {
    let path = proposal
        .affected_path
        .replace(['/', '['], ".")
        .to_ascii_lowercase();
    let mut ids = Vec::new();
    for (prefix, id) in [
        ("identity", "identity"),
        ("goals", "goals"),
        ("capabilities", "capabilities"),
        ("state", "state"),
        ("preferences", "state"),
    ] {
        if path == prefix || path.starts_with(&format!("{prefix}.")) {
            ids.push(id.to_string());
        }
    }
    if ids.is_empty() {
        ids.push("unknown".into());
    }
    ids.sort();
    ids.dedup();
    ids
}

fn materialized_lifemodel_proposal_ids(review_items: &[ReviewItem]) -> BTreeSet<String> {
    review_items
        .iter()
        .filter(|item| item.materialization_status == ReviewItemMaterializationStatus::Applied)
        .map(|item| item.source.proposal_id.clone())
        .collect::<BTreeSet<_>>()
}

fn failed_lifemodel_proposal_ids(review_items: &[ReviewItem]) -> BTreeSet<String> {
    review_items
        .iter()
        .filter(|item| item.materialization_status == ReviewItemMaterializationStatus::Failed)
        .map(|item| item.source.proposal_id.clone())
        .collect()
}

fn evidence_ref_from_proposal(proposal: &AgentProposal) -> EvidenceRef {
    EvidenceRef {
        id: format!("proposal:{}", proposal.id),
        label: "Proposal record".into(),
        source: EvidenceSource::Review,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    }
}

fn review_item_ref(item: &ReviewItem) -> BackendEntityRef {
    BackendEntityRef {
        id: item.id.clone(),
        kind: BackendEntityKind::ReviewItem,
        label: "Review item".into(),
        href: None,
    }
}

fn review_item_ref_from_proposal(proposal: &AgentProposal) -> BackendEntityRef {
    BackendEntityRef {
        id: proposal.id.clone(),
        kind: BackendEntityKind::ReviewItem,
        label: "Review item".into(),
        href: None,
    }
}

fn title_for_proposal(proposal: &AgentProposal) -> String {
    if !proposal.reason.trim().is_empty() {
        proposal.reason.trim().to_string()
    } else if !proposal.affected_path.trim().is_empty() {
        format!("LifeModel change for {}", proposal.affected_path)
    } else {
        format!("LifeModel proposal {}", proposal.id)
    }
}

fn change_kind_from_proposal(proposal: &AgentProposal) -> LifeModelChangeKind {
    if proposal.after.is_null() {
        LifeModelChangeKind::Remove
    } else if proposal.before.is_some() {
        LifeModelChangeKind::Update
    } else {
        LifeModelChangeKind::Add
    }
}

fn decision_status_from_proposal(status: ProposalStatus) -> LifeModelCandidateDecisionStatus {
    match status {
        ProposalStatus::Pending => LifeModelCandidateDecisionStatus::Pending,
        ProposalStatus::Accepted => LifeModelCandidateDecisionStatus::Accepted,
        ProposalStatus::Edited => LifeModelCandidateDecisionStatus::Edited,
        ProposalStatus::Postponed => LifeModelCandidateDecisionStatus::Postponed,
        ProposalStatus::Rejected | ProposalStatus::Expired => {
            LifeModelCandidateDecisionStatus::Unknown
        }
    }
}

fn dedupe_evidence_refs(refs: Vec<EvidenceRef>) -> Vec<EvidenceRef> {
    let mut seen = BTreeSet::new();
    refs.into_iter()
        .filter(|reference| seen.insert(reference.id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::review_item::{build_review_item, ReviewCenterBuildInput};
    use crate::agent::types::{ProposalSource, RiskLevel};
    use crate::life_model::v2::{
        LifeModelDocumentV2, LifeModelStatementV2, LifeModelVersionV2, LIFE_MODEL_V2_SCHEMA_VERSION,
    };
    use serde_json::json;

    fn canonical_input(model_version: u64, statements: &[&str]) -> LifeModelCanonicalV2Input {
        let mut document = LifeModelDocumentV2::empty("primary");
        for (index, statement) in statements.iter().enumerate() {
            document.values.push(LifeModelStatementV2 {
                id: format!("value:{}", index + 1),
                statement: (*statement).into(),
                source_refs: vec![format!("proposal:accepted-{}", index + 1)],
                confirmed_at: "2026-08-08T10:00:00Z".into(),
            });
        }
        let document_digest = document.digest().unwrap();
        let version = LifeModelVersionV2 {
            model_id: "primary".into(),
            schema_version: LIFE_MODEL_V2_SCHEMA_VERSION.into(),
            model_version,
            parent_version: model_version.checked_sub(1).filter(|version| *version > 0),
            parent_digest: model_version
                .checked_sub(1)
                .filter(|version| *version > 0)
                .map(|_| "sha256:parent".into()),
            document_digest: document_digest.clone(),
            version_digest: "sha256:test-version".into(),
            document,
            materialization_id: "proposal:accepted".into(),
            source_refs: vec!["proposal:accepted".into()],
            created_at: "2026-08-08T10:00:00Z".into(),
        };
        LifeModelCanonicalV2Input {
            model_id: version.model_id.clone(),
            schema_version: version.schema_version.clone(),
            model_version,
            parent_version: version.parent_version,
            document_digest,
            summary: version.document.summary(),
            item_count: version.document.total_item_count(),
            updated_at: Some(version.created_at.clone()),
            source_refs: version.source_refs.clone(),
            document: Some(version.document.clone()),
            human_projection: version.human_yaml_projection().unwrap(),
        }
    }

    fn proposal(id: &str, status: ProposalStatus) -> AgentProposal {
        let mut proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            "goals.short_term.0.name",
            json!("Ship backend read models"),
            "Update LifeModel goal",
            0.8,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        proposal.id = id.into();
        proposal.status = status;
        proposal
    }

    #[test]
    fn empty_lifemodel_has_no_fake_canonical_summary() {
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            now: Some("2026-07-09T00:00:00Z".into()),
            ..Default::default()
        });

        assert_eq!(envelope.status, ViewModelStatus::Empty);
        let data = envelope.data.expect("data");
        assert_eq!(data.truth_mode, LifeModelTruthMode::Unknown);
        assert!(data.canonical_summary.is_none());
        assert!(envelope
            .warnings
            .iter()
            .any(|warning| warning.code == "lifemodel.empty"));
    }

    #[test]
    fn fresh_profile_has_a_canonical_empty_owner_without_a_fake_version() {
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            fresh_profile_canonical_empty: true,
            now: Some("2026-08-08T10:00:00Z".into()),
            ..Default::default()
        });

        assert_eq!(envelope.status, ViewModelStatus::Empty);
        let data = envelope.data.expect("fresh profile data");
        assert_eq!(data.truth_mode, LifeModelTruthMode::Canonical);
        assert!(data.canonical_summary.is_none());
        assert!(data.legacy_migration_preview.is_none());
        assert!(envelope
            .warnings
            .iter()
            .all(|warning| warning.code != "lifemodel.canonical_summary_unavailable"));
    }

    #[test]
    fn legacy_migration_preview_is_visible_only_before_meaningful_canonical_v2() {
        let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(
            "identity:\n  name: Test User\nstate:\n  current_focus: Current work\n",
        )
        .expect("preview");
        let compatibility = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            legacy_migration_preview: Some(preview.clone()),
            now: Some("2026-08-08T10:00:00Z".into()),
            ..Default::default()
        });
        assert_eq!(
            compatibility
                .data
                .expect("compatibility data")
                .legacy_migration_preview,
            Some(preview.clone())
        );

        let canonical = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            canonical_v2: Some(canonical_input(1, &["One confirmed item"])),
            legacy_migration_preview: Some(preview),
            now: Some("2026-08-08T10:00:00Z".into()),
            ..Default::default()
        });
        assert!(canonical
            .data
            .expect("canonical data")
            .legacy_migration_preview
            .is_none());
    }

    #[test]
    fn structured_v2_version_is_the_only_canonical_summary_credit() {
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            canonical_v2: Some(canonical_input(
                3,
                &["Autonomy matters.", "Clarity matters."],
            )),
            projection: Some(LifeModelProjectionInput {
                model_empty: true,
                ..Default::default()
            }),
            now: Some("2026-08-08T10:01:00Z".into()),
            ..Default::default()
        });

        assert_eq!(envelope.status, ViewModelStatus::Ready);
        let data = envelope.data.expect("data");
        assert_eq!(data.truth_mode, LifeModelTruthMode::Canonical);
        let summary = data.canonical_summary.expect("canonical summary");
        assert_eq!(summary.version_label, "openlife.lifemodel.v2 · version 3");
        assert!(summary.summary.starts_with("2 confirmed long-term items:"));
        assert_eq!(summary.human_projection.model_version, 3);
        assert!(summary.human_projection.yaml.contains("Autonomy matters."));
        assert!(envelope
            .warnings
            .iter()
            .all(|warning| warning.code != "lifemodel.canonical_summary_unavailable"));
    }

    #[test]
    fn tampered_yaml_projection_cannot_receive_canonical_credit() {
        let mut canonical = canonical_input(2, &["Autonomy matters."]);
        canonical.human_projection.yaml.push_str("\n# drift\n");
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            canonical_v2: Some(canonical),
            now: Some("2026-08-08T10:01:00Z".into()),
            ..Default::default()
        });

        assert_eq!(envelope.status, ViewModelStatus::Empty);
        let data = envelope.data.expect("data");
        assert_eq!(data.truth_mode, LifeModelTruthMode::Unknown);
        assert!(data.canonical_summary.is_none());
        assert!(envelope
            .warnings
            .iter()
            .any(|warning| warning.code == "lifemodel.canonical_summary_unavailable"));
    }

    #[test]
    fn empty_v2_version_is_an_authoritative_empty_canonical_owner() {
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            canonical_v2: Some(canonical_input(1, &[])),
            now: Some("2026-08-08T10:01:00Z".into()),
            ..Default::default()
        });

        assert_eq!(envelope.status, ViewModelStatus::Empty);
        let data = envelope.data.expect("data");
        assert_eq!(data.truth_mode, LifeModelTruthMode::Canonical);
        let summary = data.canonical_summary.expect("canonical empty summary");
        assert_eq!(summary.human_projection.item_count, 0);
    }

    #[test]
    fn accepted_lifemodel_proposal_without_materialization_stays_approved_not_applied() {
        let accepted = proposal("proposal-accepted-1", ProposalStatus::Accepted);
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            proposals: vec![accepted],
            now: Some("2026-07-09T00:00:00Z".into()),
            ..Default::default()
        });

        let data = envelope.data.expect("data");
        assert_eq!(data.pending_update_counts.approved_not_applied, 1);
        assert!(data.materialized_changes.is_empty());
        assert!(envelope
            .warnings
            .iter()
            .any(|warning| warning.code == "lifemodel.approved_not_applied"));
    }

    #[test]
    fn pending_lifemodel_proposal_maps_to_candidate_not_materialization() {
        let pending = proposal("proposal-pending-1", ProposalStatus::Pending);
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            proposals: vec![pending],
            now: Some("2026-07-09T00:00:00Z".into()),
            ..Default::default()
        });

        let data = envelope.data.expect("data");
        assert_eq!(data.pending_update_counts.pending_review, 1);
        assert_eq!(data.candidate_changes.len(), 1);
        assert!(data.materialized_changes.is_empty());
    }

    #[test]
    fn review_item_applied_override_counts_as_materialized() {
        let accepted = proposal("proposal-review-applied", ProposalStatus::Accepted);
        let mut item = build_review_item(&accepted, &ReviewCenterBuildInput::default());
        item.materialization_status = ReviewItemMaterializationStatus::Applied;
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            proposals: vec![accepted],
            review_items: vec![item],
            now: Some("2026-07-09T00:00:00Z".into()),
            ..Default::default()
        });

        let data = envelope.data.expect("data");
        assert_eq!(data.pending_update_counts.approved_not_applied, 0);
        assert_eq!(data.materialized_changes.len(), 1);
        assert_eq!(data.related_review_item_refs.len(), 1);
    }

    #[test]
    fn whole_model_manual_save_is_unavailable_and_points_to_proposals() {
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            now: Some("2026-07-09T00:00:00Z".into()),
            ..Default::default()
        });
        let manual = envelope
            .data
            .expect("data")
            .manual_override_state
            .expect("manual state");

        assert!(!manual.active);
        let blocked_reason = manual.blocked_reason.unwrap();
        assert!(blocked_reason.contains("not exposed"));
        assert!(blocked_reason.contains("proposal-first"));
        assert!(!manual.save_action.expect("save action").enabled);
    }
}
