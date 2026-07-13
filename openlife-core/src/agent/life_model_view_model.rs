use crate::agent::product_read_model::{
    BackendEntityKind, BackendEntityRef, DebugAction, DebugActionKind, EvidenceRef,
    EvidenceSensitivity, EvidenceSource, ProductAction, ProductActionKind,
    ReviewItemMaterializationStatus, ViewModelActions, ViewModelEnvelope, ViewModelStatus,
    ViewModelWarning, ViewModelWarningSeverity,
};
use crate::agent::review_item::{ReviewItem, ReviewItemType};
use crate::agent::types::{AgentProposal, ProposalStatus, ProposalType};
use crate::life_model::{LifeModel, Model4DCompletion};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const LIFE_MODEL_TARGET_REF: &str = "lifemodel";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelTruthMode {
    Canonical,
    CurrentCompatibility,
    Candidate,
    PendingReview,
    ManualOverride,
    Unknown,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelDimensionId {
    Identity,
    Goals,
    Capabilities,
    State,
}

impl LifeModelDimensionId {
    fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Goals => "goals",
            Self::Capabilities => "capabilities",
            Self::State => "state",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Identity => "Identity",
            Self::Goals => "Goals",
            Self::Capabilities => "Capabilities",
            Self::State => "State",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelConfidence {
    Low,
    Medium,
    High,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeModelOwnerStatus {
    #[serde(rename = "PARTIAL")]
    Partial,
    #[serde(rename = "PHASE_2_REQUIRED")]
    Phase2Required,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeModelProvenance {
    #[serde(rename = "limited")]
    Limited,
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(rename = "PHASE_2_REQUIRED")]
    Phase2Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelDivergence {
    None,
    Minor,
    Material,
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
    pub last_materialized_at: Option<String>,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelCurrentViewSummary {
    pub current_view_ref: BackendEntityRef,
    pub compatibility_mode: bool,
    pub label: String,
    pub summary: String,
    pub divergence_from_canonical: LifeModelDivergence,
    pub evidence_refs: Vec<EvidenceRef>,
    pub owner_status: LifeModelOwnerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelDimensionSummary {
    pub id: LifeModelDimensionId,
    pub label: String,
    pub summary: String,
    pub confidence: LifeModelConfidence,
    pub stale: bool,
    pub pending_review_item_refs: Vec<BackendEntityRef>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub provenance: LifeModelProvenance,
    pub owner_status: LifeModelOwnerStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelTrustQualityState {
    pub readiness: LifeModelReadiness,
    pub completion_score: Option<u8>,
    pub missing_dimension_count: usize,
    pub stale_dimension_count: usize,
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
pub struct LifeModelViewModel {
    pub truth_mode: LifeModelTruthMode,
    pub canonical_summary: Option<LifeModelCanonicalSummary>,
    pub current_view_summary: Option<LifeModelCurrentViewSummary>,
    pub dimension_summaries: Vec<LifeModelDimensionSummary>,
    pub trust_quality_state: LifeModelTrustQualityState,
    pub pending_update_counts: LifeModelPendingUpdateCounts,
    pub provenance_refs: Vec<EvidenceRef>,
    pub candidate_changes: Vec<LifeModelCandidateChange>,
    pub materialized_changes: Vec<LifeModelMaterializedChange>,
    pub manual_override_state: Option<LifeModelManualOverrideState>,
    pub related_review_item_refs: Vec<BackendEntityRef>,
    pub memory_linkage: LifeModelMemoryLinkageSummary,
    pub source_refs: Vec<EvidenceRef>,
    pub contract_limitations: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LifeModelCurrentChangeInput {
    pub path: String,
    pub proposal_id: String,
    pub proposal_status: String,
    pub proposal_source: String,
    pub proposal_run_id: Option<String>,
    pub source_excerpt_available: bool,
    pub source_unavailable_reason: Option<String>,
    pub patch_id: Option<String>,
    pub patch_status: Option<String>,
    pub patch_path: Option<String>,
    pub patch_unavailable_reason: Option<String>,
    pub snapshot_versions: Vec<String>,
    pub snapshot_unavailable_reason: Option<String>,
    pub current_matches_accepted_after: bool,
}

impl LifeModelCurrentChangeInput {
    pub fn has_materialization_proof(&self) -> bool {
        self.proposal_status == "accepted"
            && self.patch_status.as_deref() == Some("applied")
            && self
                .patch_id
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            && self.patch_unavailable_reason.is_none()
            && self.snapshot_unavailable_reason.is_none()
            && !self.snapshot_versions.is_empty()
            && self.current_matches_accepted_after
    }
}

#[derive(Debug, Clone, Default)]
pub struct LifeModelCurrentViewInput {
    pub path: String,
    pub label: String,
    pub value: Option<String>,
    pub unavailable_reason: Option<String>,
    pub current_value_source: String,
    pub change: Option<LifeModelCurrentChangeInput>,
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
pub struct LifeModelViewModelBuildInput {
    pub life_model: Option<LifeModel>,
    pub current_view: Option<LifeModelCurrentViewInput>,
    pub projection: Option<LifeModelProjectionInput>,
    pub proposals: Vec<AgentProposal>,
    pub review_items: Vec<ReviewItem>,
    pub memory_count: Option<usize>,
    pub tier_stats: Option<LifeModelMemoryTierStatsInput>,
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
    let materialized_proposal_ids =
        materialized_lifemodel_proposal_ids(input.current_view.as_ref(), &life_model_review_items);
    let failed_materialization_ids = failed_lifemodel_proposal_ids(&life_model_review_items);
    let meaningful_life_model = input
        .life_model
        .as_ref()
        .is_some_and(|model| !model.is_effectively_empty());
    let status = loaded_status(&input, meaningful_life_model);
    let current_view_summary =
        build_current_view_summary(&input, &source_refs, meaningful_life_model);
    let dimension_summaries = build_dimension_summaries(
        input.life_model.as_ref(),
        input
            .life_model
            .as_ref()
            .map(LifeModel::calculate_4d_completion),
        &life_model_proposals,
        &source_refs,
        status == ViewModelStatus::Stale,
    );
    let materialized_changes = build_materialized_changes(
        &life_model_proposals,
        input.current_view.as_ref(),
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
        truth_mode: derive_truth_mode(status, current_view_summary.as_ref(), meaningful_life_model),
        canonical_summary: None,
        current_view_summary,
        dimension_summaries: dimension_summaries.clone(),
        trust_quality_state: build_trust_quality_state(
            status,
            &input,
            &dimension_summaries,
            &source_refs,
        ),
        pending_update_counts,
        provenance_refs: source_refs.clone(),
        candidate_changes: build_candidate_changes(&life_model_proposals),
        materialized_changes,
        manual_override_state: Some(build_manual_override_state(&source_refs)),
        related_review_item_refs,
        memory_linkage: build_memory_linkage(&input, &source_refs),
        source_refs: source_refs.clone(),
        contract_limitations: vec![
            "Backend owns this R3 LifeModelViewModel, but canonical truth is not marked ready without materialized provenance.".into(),
            "Accepted proposal decisions remain approved-not-applied unless patch, snapshot, and current-value evidence prove applied.".into(),
            "Memory linkage remains partial until R5 MemoryViewModel owns the Memory/LifeModel relation.".into(),
            "Manual overrides are governed and separate from proposal-first review materialization.".into(),
        ],
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
    meaningful_life_model: bool,
) -> ViewModelStatus {
    if input.stale {
        return ViewModelStatus::Stale;
    }
    if input
        .projection
        .as_ref()
        .is_some_and(|projection| projection.model_empty)
        && !current_view_has_value(input.current_view.as_ref())
    {
        return ViewModelStatus::Empty;
    }
    if !meaningful_life_model && !current_view_has_value(input.current_view.as_ref()) {
        return ViewModelStatus::Empty;
    }
    ViewModelStatus::Ready
}

fn current_view_has_value(current_view: Option<&LifeModelCurrentViewInput>) -> bool {
    current_view
        .and_then(|view| view.value.as_deref())
        .is_some_and(|value| !value.trim().is_empty())
}

fn derive_truth_mode(
    status: ViewModelStatus,
    current_view_summary: Option<&LifeModelCurrentViewSummary>,
    meaningful_life_model: bool,
) -> LifeModelTruthMode {
    if matches!(status, ViewModelStatus::Error) {
        return LifeModelTruthMode::Unavailable;
    }
    if current_view_summary.is_some() || meaningful_life_model {
        return LifeModelTruthMode::CurrentCompatibility;
    }
    LifeModelTruthMode::Unknown
}

fn build_current_view_summary(
    input: &LifeModelViewModelBuildInput,
    source_refs: &[EvidenceRef],
    meaningful_life_model: bool,
) -> Option<LifeModelCurrentViewSummary> {
    if let Some(current_view) = input.current_view.as_ref() {
        if let Some(value) = current_view
            .value
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            return Some(LifeModelCurrentViewSummary {
                current_view_ref: BackendEntityRef {
                    id: format!("lifemodel-current:{}", current_view.path),
                    kind: BackendEntityKind::LifeModel,
                    label: current_view.label.clone(),
                    href: None,
                },
                compatibility_mode: true,
                label: current_view.label.clone(),
                summary: value.to_string(),
                divergence_from_canonical: LifeModelDivergence::Unknown,
                evidence_refs: source_refs.to_vec(),
                owner_status: LifeModelOwnerStatus::Partial,
            });
        }
    }

    if meaningful_life_model {
        let version = input
            .life_model
            .as_ref()
            .map(|model| model.metadata.version.trim())
            .filter(|version| !version.is_empty())
            .unwrap_or("unknown");
        return Some(LifeModelCurrentViewSummary {
            current_view_ref: BackendEntityRef {
                id: format!("lifemodel-current:{version}"),
                kind: BackendEntityKind::LifeModel,
                label: "Existing LifeModel primitive".into(),
                href: None,
            },
            compatibility_mode: true,
            label: "Existing LifeModel primitive".into(),
            summary: "A LifeModel primitive is available, but this read model does not label it canonical truth without materialized provenance.".into(),
            divergence_from_canonical: LifeModelDivergence::Unknown,
            evidence_refs: source_refs.to_vec(),
            owner_status: LifeModelOwnerStatus::Partial,
        });
    }
    None
}

fn build_dimension_summaries(
    life_model: Option<&LifeModel>,
    completion: Option<Model4DCompletion>,
    proposals: &[AgentProposal],
    source_refs: &[EvidenceRef],
    stale: bool,
) -> Vec<LifeModelDimensionSummary> {
    let Some(model) = life_model.filter(|model| !model.is_effectively_empty()) else {
        return Vec::new();
    };

    [
        LifeModelDimensionId::Identity,
        LifeModelDimensionId::Goals,
        LifeModelDimensionId::Capabilities,
        LifeModelDimensionId::State,
    ]
    .into_iter()
    .map(|dimension| {
        let dimension_proposals = proposals
            .iter()
            .filter(|proposal| {
                affected_dimension_ids(proposal)
                    .iter()
                    .any(|id| id == dimension.as_str())
            })
            .collect::<Vec<_>>();
        let mut evidence_refs = source_refs.to_vec();
        evidence_refs.extend(
            dimension_proposals
                .iter()
                .map(|proposal| evidence_ref_from_proposal(proposal)),
        );
        LifeModelDimensionSummary {
            id: dimension,
            label: dimension.label().into(),
            summary: dimension_summary(model, dimension),
            confidence: confidence_for_dimension(completion.as_ref(), dimension),
            stale,
            pending_review_item_refs: dimension_proposals
                .iter()
                .filter(|proposal| {
                    matches!(
                        proposal.status,
                        ProposalStatus::Pending
                            | ProposalStatus::Edited
                            | ProposalStatus::Postponed
                    )
                })
                .map(|proposal| review_item_ref_from_proposal(proposal))
                .collect(),
            evidence_refs: dedupe_evidence_refs(evidence_refs),
            provenance: LifeModelProvenance::Limited,
            owner_status: LifeModelOwnerStatus::Partial,
        }
    })
    .collect()
}

fn dimension_summary(model: &LifeModel, dimension: LifeModelDimensionId) -> String {
    let items = match dimension {
        LifeModelDimensionId::Identity => compact_items([
            nonempty(model.identity.name.as_str()),
            nonempty(model.identity.role_definition.primary_role.as_str()),
            model
                .identity
                .values
                .first()
                .map(|value| value.name.as_str()),
            nonempty(model.identity.mission_statement.as_str()),
        ]),
        LifeModelDimensionId::Goals => compact_items([
            model.goals.daily.first().map(|goal| goal.name.as_str()),
            model
                .goals
                .short_term
                .first()
                .map(|goal| goal.name.as_str()),
            model
                .goals
                .medium_term
                .first()
                .map(|goal| goal.name.as_str()),
            model.goals.long_term.first().map(|goal| goal.name.as_str()),
        ]),
        LifeModelDimensionId::Capabilities => compact_items([
            model
                .capabilities
                .skills
                .first()
                .map(|skill| skill.name.as_str()),
            model
                .capabilities
                .knowledge_domains
                .first()
                .map(|domain| domain.domain.as_str()),
            model
                .capabilities
                .resources
                .first()
                .map(|resource| resource.name.as_str()),
            model
                .capabilities
                .tools
                .first()
                .map(|tool| tool.name.as_str()),
        ]),
        LifeModelDimensionId::State => compact_items([
            nonempty(model.state.current_focus.as_str()),
            model.state.focus_areas.first().map(String::as_str),
            nonempty(model.state.health_status.physical.as_str()),
            nonempty(model.state.emotional_state.current_mood.as_str()),
        ]),
    };
    if items.is_empty() {
        format!("{} has no confirmed summary items.", dimension.label())
    } else {
        items.join(" / ")
    }
}

fn compact_items<const N: usize>(items: [Option<&str>; N]) -> Vec<String> {
    items
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .take(3)
        .map(|value| {
            if value.chars().count() > 80 {
                format!("{}...", value.chars().take(77).collect::<String>())
            } else {
                value.to_string()
            }
        })
        .collect()
}

fn nonempty(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn confidence_for_dimension(
    completion: Option<&Model4DCompletion>,
    dimension: LifeModelDimensionId,
) -> LifeModelConfidence {
    let Some(completion) = completion else {
        return LifeModelConfidence::Unknown;
    };
    let score = match dimension {
        LifeModelDimensionId::Identity => completion.identity,
        LifeModelDimensionId::Goals => completion.goals,
        LifeModelDimensionId::Capabilities => completion.capabilities,
        LifeModelDimensionId::State => completion.state,
    };
    match score {
        75..=100 => LifeModelConfidence::High,
        40..=74 => LifeModelConfidence::Medium,
        1..=39 => LifeModelConfidence::Low,
        _ => LifeModelConfidence::Unknown,
    }
}

fn build_trust_quality_state(
    status: ViewModelStatus,
    input: &LifeModelViewModelBuildInput,
    dimension_summaries: &[LifeModelDimensionSummary],
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
        completion_score: input
            .life_model
            .as_ref()
            .map(LifeModel::calculate_4d_completion)
            .map(|completion| completion.overall),
        missing_dimension_count: 4usize.saturating_sub(dimension_summaries.len()),
        stale_dimension_count: dimension_summaries
            .iter()
            .filter(|dimension| dimension.stale)
            .count(),
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
    current_view: Option<&LifeModelCurrentViewInput>,
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
        if let Some(change) = current_view
            .and_then(|view| view.change.as_ref())
            .filter(|change| &change.proposal_id == proposal_id)
        {
            if let Some(patch_id) = &change.patch_id {
                evidence_refs.push(EvidenceRef {
                    id: format!("lifemodel-patch:{patch_id}"),
                    label: "Applied LifeModel patch".into(),
                    source: EvidenceSource::LifeModel,
                    sensitivity: Some(EvidenceSensitivity::LocalPrivate),
                });
            }
            for version in &change.snapshot_versions {
                evidence_refs.push(EvidenceRef {
                    id: format!("lifemodel-snapshot:{version}"),
                    label: "LifeModel materialization snapshot".into(),
                    source: EvidenceSource::Audit,
                    sensitivity: Some(EvidenceSensitivity::LocalPrivate),
                });
            }
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
            "Manual LifeModel saves require save_life_model with an explicit governed manual override request; this read model does not authorize direct writes.".into(),
        ),
        draft_ref: None,
        save_action: Some(ProductAction {
            id: "lifemodel.manual_override.save".into(),
            label: "Save manual override".into(),
            kind: ProductActionKind::Configure,
            enabled: false,
            disabled_reason: Some(
                "Manual override is governed separately from the proposal-first review flow.".into(),
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
        owner_status: LifeModelOwnerStatus::Phase2Required,
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
            "lifemodel.canonical_summary_unavailable",
            "Canonical LifeModel summary remains unavailable because no refreshed materialized provenance proves canonical truth.",
            ViewModelWarningSeverity::Info,
            source_refs.to_vec(),
        ),
        warning(
            "lifemodel.materialization_fail_closed",
            "Accepted LifeModel proposals are decision state only unless patch, snapshot, and current-value evidence prove materialization.",
            ViewModelWarningSeverity::Warning,
            source_refs.to_vec(),
        ),
        warning(
            "lifemodel.memory_linkage_limited",
            "Memory linkage is a backend-owned partial summary until R5 MemoryViewModel owns memory truth.",
            ViewModelWarningSeverity::Info,
            source_refs.to_vec(),
        ),
    ];
    if status == ViewModelStatus::Empty {
        warnings.push(warning(
            "lifemodel.empty",
            "No confirmed LifeModel content was found; no fake canonical summary was generated.",
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
    if let Some(model) = &input.life_model {
        refs.push(EvidenceRef {
            id: format!(
                "lifemodel:{}",
                empty_to_unknown(model.metadata.version.as_str())
            ),
            label: "LifeModel primitive".into(),
            source: EvidenceSource::LifeModel,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        });
    }
    if let Some(current_view) = &input.current_view {
        refs.push(EvidenceRef {
            id: format!("lifemodel-current:{}", current_view.path),
            label: "LifeModel current compatibility view".into(),
            source: EvidenceSource::LifeModel,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        });
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

fn materialized_lifemodel_proposal_ids(
    current_view: Option<&LifeModelCurrentViewInput>,
    review_items: &[ReviewItem],
) -> BTreeSet<String> {
    let mut ids = review_items
        .iter()
        .filter(|item| item.materialization_status == ReviewItemMaterializationStatus::Applied)
        .map(|item| item.source.proposal_id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(change) = current_view
        .and_then(|view| view.change.as_ref())
        .filter(|change| change.has_materialization_proof())
    {
        ids.insert(change.proposal_id.clone());
    }
    ids
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

fn empty_to_unknown(value: &str) -> &str {
    if value.trim().is_empty() {
        "unknown"
    } else {
        value
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
    use serde_json::json;

    fn model_with_content() -> LifeModel {
        let mut model = LifeModel::default_model();
        model.identity.name = "Test User".into();
        model.goals.short_term.push(crate::life_model::GoalItem {
            name: "Ship backend read models".into(),
            ..Default::default()
        });
        model.capabilities.skills.push(crate::life_model::Skill {
            name: "Rust".into(),
            proficiency: 80,
            description: "Backend development".into(),
        });
        model.state.focus_areas.push("OpenLife".into());
        model
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
            life_model: Some(LifeModel::default_model()),
            now: Some("2026-07-09T00:00:00Z".into()),
            ..Default::default()
        });

        assert_eq!(envelope.status, ViewModelStatus::Empty);
        let data = envelope.data.expect("data");
        assert_eq!(data.truth_mode, LifeModelTruthMode::Unknown);
        assert!(data.canonical_summary.is_none());
        assert!(data.dimension_summaries.is_empty());
        assert!(envelope
            .warnings
            .iter()
            .any(|warning| warning.code == "lifemodel.empty"));
    }

    #[test]
    fn accepted_lifemodel_proposal_without_materialization_stays_approved_not_applied() {
        let accepted = proposal("proposal-accepted-1", ProposalStatus::Accepted);
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            life_model: Some(model_with_content()),
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
    fn applied_materialization_requires_patch_snapshot_and_current_match_evidence() {
        let accepted = proposal("proposal-applied-1", ProposalStatus::Accepted);
        let current_view = LifeModelCurrentViewInput {
            path: "preferences.communication_style".into(),
            label: "Communication style".into(),
            value: Some("Direct and structured".into()),
            current_value_source: "accepted_proposal".into(),
            change: Some(LifeModelCurrentChangeInput {
                proposal_id: accepted.id.clone(),
                proposal_status: "accepted".into(),
                patch_id: Some("patch-1".into()),
                patch_status: Some("applied".into()),
                snapshot_versions: vec!["before-v1".into(), "after-v1".into()],
                current_matches_accepted_after: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            life_model: Some(model_with_content()),
            current_view: Some(current_view),
            proposals: vec![accepted],
            now: Some("2026-07-09T00:00:00Z".into()),
            ..Default::default()
        });

        let data = envelope.data.expect("data");
        assert_eq!(data.pending_update_counts.approved_not_applied, 0);
        assert_eq!(data.materialized_changes.len(), 1);
        assert_eq!(
            data.materialized_changes[0].materialization_status,
            ReviewItemMaterializationStatus::Applied
        );
    }

    #[test]
    fn accepted_current_view_without_snapshot_stays_unapplied() {
        let accepted = proposal("proposal-missing-snapshot", ProposalStatus::Accepted);
        let current_view = LifeModelCurrentViewInput {
            path: "preferences.communication_style".into(),
            label: "Communication style".into(),
            value: Some("Direct and structured".into()),
            change: Some(LifeModelCurrentChangeInput {
                proposal_id: accepted.id.clone(),
                proposal_status: "accepted".into(),
                patch_id: Some("patch-1".into()),
                patch_status: Some("applied".into()),
                snapshot_unavailable_reason: Some("snapshot_missing".into()),
                current_matches_accepted_after: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            life_model: Some(model_with_content()),
            current_view: Some(current_view),
            proposals: vec![accepted],
            now: Some("2026-07-09T00:00:00Z".into()),
            ..Default::default()
        });

        let data = envelope.data.expect("data");
        assert_eq!(data.pending_update_counts.approved_not_applied, 1);
        assert!(data.materialized_changes.is_empty());
    }

    #[test]
    fn pending_lifemodel_proposal_maps_to_candidate_not_materialization() {
        let pending = proposal("proposal-pending-1", ProposalStatus::Pending);
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            life_model: Some(model_with_content()),
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
            life_model: Some(model_with_content()),
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
    fn manual_override_state_is_governed_and_separate() {
        let envelope = build_life_model_view_model_envelope(LifeModelViewModelBuildInput {
            life_model: Some(model_with_content()),
            now: Some("2026-07-09T00:00:00Z".into()),
            ..Default::default()
        });
        let manual = envelope
            .data
            .expect("data")
            .manual_override_state
            .expect("manual state");

        assert!(!manual.active);
        assert!(manual.blocked_reason.unwrap().contains("governed"));
        assert_eq!(manual.save_action.expect("save action").enabled, false);
    }
}
