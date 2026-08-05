use crate::agent::product_read_model::{
    BackendEntityKind, BackendEntityRef, EvidenceRef, EvidenceSensitivity, EvidenceSource,
    ProductRiskLevel, ReviewAction, ReviewActionKind, ReviewItemMaterializationStatus,
};
use crate::agent::review_decision_context::{build_review_decision_context, ReviewDecisionContext};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalStatus, ProposalType, RiskLevel};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewItemType {
    GoalUpdate,
    StateUpdate,
    PreferenceUpdate,
    CapabilityUpdate,
    MemoryWrite,
    MemoryArchive,
    ToolPermission,
    PluginPermission,
    ScheduledTask,
    ExternalWriteAction,
    ModelPolicyChange,
    DataExport,
    ScheduleCheckin,
    LifeModelUpdate,
    Unsupported,
}

impl From<ProposalType> for ReviewItemType {
    fn from(value: ProposalType) -> Self {
        match value {
            ProposalType::GoalUpdate => Self::GoalUpdate,
            ProposalType::StateUpdate => Self::StateUpdate,
            ProposalType::PreferenceUpdate => Self::PreferenceUpdate,
            ProposalType::CapabilityUpdate => Self::CapabilityUpdate,
            ProposalType::MemoryWrite => Self::MemoryWrite,
            ProposalType::MemoryArchive => Self::MemoryArchive,
            ProposalType::ToolPermission => Self::ToolPermission,
            ProposalType::PluginPermission => Self::PluginPermission,
            ProposalType::ScheduledTask => Self::ScheduledTask,
            ProposalType::ExternalWriteAction => Self::ExternalWriteAction,
            ProposalType::ModelPolicyChange => Self::ModelPolicyChange,
            ProposalType::DataExport => Self::DataExport,
            ProposalType::ScheduleCheckin => Self::ScheduleCheckin,
            ProposalType::Unsupported => Self::Unsupported,
            ProposalType::LifeModelUpdate => Self::LifeModelUpdate,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewItemDecisionStatus {
    Pending,
    Approved,
    Rejected,
    Edited,
    Deferred,
    Unknown,
}

impl ReviewItemDecisionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Edited => "edited",
            Self::Deferred => "deferred",
            Self::Unknown => "unknown",
        }
    }
}

impl From<ProposalStatus> for ReviewItemDecisionStatus {
    fn from(value: ProposalStatus) -> Self {
        match value {
            ProposalStatus::Pending => Self::Pending,
            ProposalStatus::Accepted => Self::Approved,
            ProposalStatus::Rejected => Self::Rejected,
            ProposalStatus::Edited => Self::Edited,
            ProposalStatus::Postponed => Self::Deferred,
            ProposalStatus::Expired => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewItemSourceKind {
    Proposal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItemSource {
    pub kind: ReviewItemSourceKind,
    pub proposal_id: String,
    pub proposal_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItemTaskResumeRelation {
    pub task_session_id: String,
    #[serde(default)]
    pub resume_requires_materialization: bool,
    pub can_request_resume: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_action_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItemArtifactEvidence {
    pub state: String,
    pub target_reference_digest: String,
    pub content_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_content_digest: Option<String>,
    pub byte_size: u64,
    pub media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: ReviewItemType,
    pub source: ReviewItemSource,
    pub status: ReviewItemDecisionStatus,
    pub materialization_status: ReviewItemMaterializationStatus,
    pub decision_context: ReviewDecisionContext,
    pub allowed_actions: Vec<ReviewAction>,
    pub risk: ProductRiskLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub target_refs: Vec<BackendEntityRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_resume_relation: Option<ReviewItemTaskResumeRelation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_evidence: Option<ReviewItemArtifactEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewBatchDomain {
    Memory,
    LifeModel,
    ToolPermission,
    ExternalAction,
    Other,
}

impl ReviewBatchDomain {
    fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::LifeModel => "life_model",
            Self::ToolPermission => "tool_permission",
            Self::ExternalAction => "external_action",
            Self::Other => "other",
        }
    }
}

/// A presentation-only grouping of independently authoritative ReviewItems.
///
/// A batch has no approve/reject action and cannot authorize effects. Each
/// child Proposal retains its own decision, dispatch claim and materialization
/// receipt; this projection only prevents one Main Chat session from appearing
/// as an unstructured wall of cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBatch {
    pub id: String,
    pub domain: ReviewBatchDomain,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub item_ids: Vec<String>,
    pub action_required_count: usize,
    pub highest_risk: ProductRiskLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCenterSummary {
    pub total: usize,
    pub action_required_count: usize,
    pub blocked_action_count: usize,
    #[serde(default)]
    pub by_status: BTreeMap<String, usize>,
    #[serde(default)]
    pub by_risk: BTreeMap<String, usize>,
    #[serde(default)]
    pub by_materialization_status: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCenterViewModel {
    #[serde(default)]
    pub batches: Vec<ReviewBatch>,
    pub items: Vec<ReviewItem>,
    pub summary: ReviewCenterSummary,
}

#[derive(Debug, Clone, Default)]
pub struct ReviewCenterBuildInput {
    pub proposals: Vec<AgentProposal>,
    pub safe_mode_active: bool,
    pub safe_mode_reason: Option<String>,
    pub safe_paths: Vec<String>,
    pub materialization_overrides: BTreeMap<String, ReviewItemMaterializationStatus>,
    pub terminal_owner_task_session_ids: BTreeMap<String, String>,
    pub artifact_evidence: BTreeMap<String, ReviewItemArtifactEvidence>,
}

pub fn build_review_center_view_model(input: ReviewCenterBuildInput) -> ReviewCenterViewModel {
    let mut items = Vec::new();
    let mut summary = ReviewCenterSummary::default();

    for proposal in &input.proposals {
        let item = build_review_item(proposal, &input);
        summary.total += 1;
        increment(&mut summary.by_status, item.status.as_str());
        increment(&mut summary.by_risk, risk_key(item.risk));
        increment(
            &mut summary.by_materialization_status,
            materialization_key(item.materialization_status),
        );
        if item.allowed_actions.iter().any(is_enabled_decision_action) {
            summary.action_required_count += 1;
        }
        summary.blocked_action_count += item
            .allowed_actions
            .iter()
            .filter(|action| !action.enabled && action.disabled_reason.is_some())
            .count();
        items.push(item);
    }

    let batches = build_review_batches(&input.proposals, &items, &input);
    ReviewCenterViewModel {
        batches,
        items,
        summary,
    }
}

fn build_review_batches(
    proposals: &[AgentProposal],
    items: &[ReviewItem],
    input: &ReviewCenterBuildInput,
) -> Vec<ReviewBatch> {
    let mut groups: BTreeMap<(ReviewBatchDomain, String), ReviewBatch> = BTreeMap::new();
    for (proposal, item) in proposals.iter().zip(items) {
        let domain = review_batch_domain(proposal.proposal_type);
        let session_id = review_batch_session_id(proposal, input);
        let grouping_owner = session_id
            .clone()
            .unwrap_or_else(|| format!("proposal:{}", proposal.id));
        let (_, owner_digest) = crate::agent::metadata_safe::metadata_safe_text_digest(&format!(
            "{}:{}",
            domain.as_str(),
            grouping_owner
        ));
        let key = (domain, grouping_owner);
        let batch = groups.entry(key).or_insert_with(|| ReviewBatch {
            id: format!("review_batch:{}:{owner_digest}", domain.as_str()),
            domain,
            session_id,
            item_ids: Vec::new(),
            action_required_count: 0,
            highest_risk: item.risk,
        });
        batch.item_ids.push(item.id.clone());
        if item.allowed_actions.iter().any(is_enabled_decision_action) {
            batch.action_required_count += 1;
        }
        if product_risk_rank(item.risk) > product_risk_rank(batch.highest_risk) {
            batch.highest_risk = item.risk;
        }
    }
    groups.into_values().collect()
}

fn review_batch_domain(proposal_type: ProposalType) -> ReviewBatchDomain {
    match proposal_type {
        ProposalType::MemoryWrite | ProposalType::MemoryArchive => ReviewBatchDomain::Memory,
        ProposalType::GoalUpdate
        | ProposalType::StateUpdate
        | ProposalType::PreferenceUpdate
        | ProposalType::CapabilityUpdate
        | ProposalType::ModelPolicyChange
        | ProposalType::LifeModelUpdate => ReviewBatchDomain::LifeModel,
        ProposalType::ToolPermission | ProposalType::PluginPermission => {
            ReviewBatchDomain::ToolPermission
        }
        ProposalType::ScheduledTask
        | ProposalType::ExternalWriteAction
        | ProposalType::DataExport
        | ProposalType::ScheduleCheckin => ReviewBatchDomain::ExternalAction,
        ProposalType::Unsupported => ReviewBatchDomain::Other,
    }
}

fn review_batch_session_id(
    proposal: &AgentProposal,
    input: &ReviewCenterBuildInput,
) -> Option<String> {
    input
        .terminal_owner_task_session_ids
        .get(&proposal.id)
        .cloned()
}

fn product_risk_rank(risk: ProductRiskLevel) -> u8 {
    match risk {
        ProductRiskLevel::None => 0,
        ProductRiskLevel::Low => 1,
        ProductRiskLevel::Medium => 2,
        ProductRiskLevel::High => 3,
        ProductRiskLevel::Critical => 4,
        ProductRiskLevel::Unknown => 5,
    }
}

pub fn build_review_item(proposal: &AgentProposal, input: &ReviewCenterBuildInput) -> ReviewItem {
    let item_id = proposal.id.clone();
    let status = ReviewItemDecisionStatus::from(proposal.status);
    let materialization_status = materialization_status_for(proposal, input);
    let evidence_refs = evidence_refs_for(proposal);
    let decision_context = build_review_decision_context(proposal, &evidence_refs);
    let target_refs = target_refs_for(proposal);
    let task_resume_relation =
        task_resume_relation_for(proposal, status, materialization_status, input);
    let allowed_actions = allowed_actions_for(
        proposal,
        &item_id,
        status,
        materialization_status,
        &decision_context,
        task_resume_relation.as_ref(),
        input,
    );

    ReviewItem {
        id: item_id,
        item_type: ReviewItemType::from(proposal.proposal_type),
        source: ReviewItemSource {
            kind: ReviewItemSourceKind::Proposal,
            proposal_id: proposal.id.clone(),
            proposal_source: proposal.source.to_string(),
            source_detail: proposal.source_detail.clone(),
            run_id: proposal.run_id.clone(),
        },
        status,
        materialization_status,
        decision_context,
        allowed_actions,
        risk: risk_for(proposal.risk_level),
        expires_at: proposal.expires_at,
        evidence_refs,
        target_refs,
        task_resume_relation,
        artifact_evidence: input.artifact_evidence.get(&proposal.id).cloned(),
    }
}

fn allowed_actions_for(
    proposal: &AgentProposal,
    item_id: &str,
    status: ReviewItemDecisionStatus,
    materialization_status: ReviewItemMaterializationStatus,
    decision_context: &ReviewDecisionContext,
    task_resume_relation: Option<&ReviewItemTaskResumeRelation>,
    input: &ReviewCenterBuildInput,
) -> Vec<ReviewAction> {
    let mut actions = Vec::new();
    let approve_blocker = approve_blocker(proposal, decision_context, input);
    actions.push(
        action(item_id, "approve", "Approve", ReviewActionKind::Approve)
            .with_expected_materialization_status(ReviewItemMaterializationStatus::Unknown)
            .requiring_confirmation()
            .maybe_disabled(approve_blocker),
    );
    actions.push(
        action(item_id, "reject", "Reject", ReviewActionKind::Reject)
            .maybe_disabled(review_decision_blocker(status)),
    );
    actions.push(
        action(item_id, "later", "Later", ReviewActionKind::Later)
            .maybe_disabled(review_decision_blocker(status)),
    );
    actions.push(
        action(item_id, "edit", "Edit", ReviewActionKind::Edit)
            .maybe_disabled(edit_blocker(proposal, status, input)),
    );

    if status == ReviewItemDecisionStatus::Approved
        && !matches!(
            materialization_status,
            ReviewItemMaterializationStatus::Applied
                | ReviewItemMaterializationStatus::NotApplicable
        )
    {
        actions.push(
            action(item_id, "apply", "Apply", ReviewActionKind::Apply)
                .requiring_confirmation()
                .disabled(
                    "No backend materialization request command is available for this review item.",
                ),
        );
    }

    if let Some(relation) = task_resume_relation {
        let resume = action(item_id, "resume", "Resume task", ReviewActionKind::Resume);
        actions.push(if relation.can_request_resume {
            resume
        } else {
            resume.disabled(
                relation
                    .blocked_reason
                    .clone()
                    .unwrap_or_else(|| "Approve before requesting task resume.".into()),
            )
        });
    }

    actions.push(action(
        item_id,
        "view_evidence",
        "View evidence",
        ReviewActionKind::ViewEvidence,
    ));

    for action in &actions {
        action
            .validate()
            .expect("review action builder must preserve kind/effect invariant");
    }

    actions
}

trait MaybeDisabled {
    fn maybe_disabled(self, reason: Option<String>) -> Self;
}

impl MaybeDisabled for ReviewAction {
    fn maybe_disabled(self, reason: Option<String>) -> Self {
        match reason {
            Some(reason) => self.disabled(reason),
            None => self,
        }
    }
}

fn action(item_id: &str, suffix: &str, label: &str, kind: ReviewActionKind) -> ReviewAction {
    ReviewAction::new(
        format!("{item_id}:{suffix}"),
        label,
        kind,
        item_id.to_string(),
    )
}

fn is_enabled_decision_action(action: &ReviewAction) -> bool {
    action.enabled
        && matches!(
            action.kind,
            ReviewActionKind::Approve
                | ReviewActionKind::Reject
                | ReviewActionKind::Edit
                | ReviewActionKind::Later
                | ReviewActionKind::Revoke
        )
}

fn review_decision_blocker(status: ReviewItemDecisionStatus) -> Option<String> {
    if is_reviewable_status(status) {
        None
    } else {
        Some("Only pending, edited, or deferred review items can receive a review decision.".into())
    }
}

fn edit_blocker(
    proposal: &AgentProposal,
    status: ReviewItemDecisionStatus,
    input: &ReviewCenterBuildInput,
) -> Option<String> {
    if input.safe_mode_active {
        return Some(
            input
                .safe_mode_reason
                .clone()
                .unwrap_or_else(|| "Safe Mode disables approve and edit actions.".into()),
        );
    }
    if let Some(blocker) = review_decision_blocker(status) {
        return Some(blocker);
    }
    if is_unsupported_type(proposal.proposal_type) {
        return Some("This review item type has no backend edit/apply pathway yet.".into());
    }
    if is_builder_lifemodel_patch_batch(proposal) {
        return Some(
            "Builder batch review requires a typed Builder editor; generic edit is unavailable."
                .into(),
        );
    }
    if proposal.proposal_type == ProposalType::ExternalWriteAction
        && !external_write_paths_are_safe(proposal, &input.safe_paths)
    {
        return Some("The external write path is outside configured safe paths.".into());
    }
    if proposal.proposal_type == ProposalType::ExternalWriteAction
        && !external_write_target_precondition_is_complete(proposal)
    {
        return Some(
            "The reviewed file target state is incomplete; create a fresh proposal before editing."
                .into(),
        );
    }
    None
}

fn is_builder_lifemodel_patch_batch(proposal: &AgentProposal) -> bool {
    proposal.proposal_type == ProposalType::LifeModelUpdate
        && proposal.source == ProposalSource::BuilderReview
        && proposal.affected_path == crate::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH
}

fn approve_blocker(
    proposal: &AgentProposal,
    decision_context: &ReviewDecisionContext,
    input: &ReviewCenterBuildInput,
) -> Option<String> {
    if input.safe_mode_active {
        return Some(
            input
                .safe_mode_reason
                .clone()
                .unwrap_or_else(|| "Safe Mode disables approve and edit actions.".into()),
        );
    }
    if let Some(blocker) = review_decision_blocker(ReviewItemDecisionStatus::from(proposal.status))
    {
        return Some(blocker);
    }
    if is_unsupported_type(proposal.proposal_type) {
        return Some("This review item type has no backend apply pathway yet.".into());
    }
    if proposal.proposal_type == ProposalType::ToolPermission
        && !decision_context
            .permission
            .as_ref()
            .is_some_and(|permission| permission.is_ready())
    {
        return Some(
            "Exact permission scope is incomplete; approval stays disabled until the backend can explain the action, target, duration, and transmission boundary."
                .into(),
        );
    }
    if proposal.proposal_type == ProposalType::ExternalWriteAction
        && !external_write_paths_are_safe(proposal, &input.safe_paths)
    {
        return Some("The external write path is outside configured safe paths.".into());
    }
    if proposal.proposal_type == ProposalType::ExternalWriteAction
        && !external_write_target_precondition_is_complete(proposal)
    {
        return Some(
            "The reviewed file target state is incomplete; create a fresh proposal before approval."
                .into(),
        );
    }
    None
}

fn is_reviewable_status(status: ReviewItemDecisionStatus) -> bool {
    matches!(
        status,
        ReviewItemDecisionStatus::Pending
            | ReviewItemDecisionStatus::Edited
            | ReviewItemDecisionStatus::Deferred
    )
}

fn is_unsupported_type(proposal_type: ProposalType) -> bool {
    matches!(
        proposal_type,
        ProposalType::PluginPermission
            | ProposalType::ModelPolicyChange
            | ProposalType::ScheduleCheckin
            | ProposalType::Unsupported
    )
}

fn external_write_paths(proposal: &AgentProposal) -> Vec<&str> {
    let operation = proposal
        .after
        .get("operation")
        .and_then(|value| value.as_str());
    if matches!(operation, Some("move" | "trash" | "restore")) {
        return ["source_path", "target_path"]
            .into_iter()
            .filter_map(|field| proposal.after.get(field).and_then(|value| value.as_str()))
            .filter(|value| !value.trim().is_empty())
            .collect();
    }
    proposal
        .after
        .get("path")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .into_iter()
        .collect()
}

fn external_write_paths_are_safe(proposal: &AgentProposal, safe_paths: &[String]) -> bool {
    let paths = external_write_paths(proposal);
    !paths.is_empty()
        && paths
            .into_iter()
            .all(|path| is_path_in_safe_paths(Some(path), safe_paths))
}

fn external_write_target_precondition_is_complete(proposal: &AgentProposal) -> bool {
    let operation = proposal
        .after
        .get("operation")
        .and_then(|value| value.as_str())
        .unwrap_or("propose_write");
    if matches!(operation, "move" | "trash" | "restore") {
        return true;
    }
    let Some(expected_absent) = proposal
        .after
        .get("expected_target_absent")
        .and_then(|value| value.as_bool())
    else {
        return false;
    };
    let expected_digest = proposal
        .after
        .get("expected_target_digest")
        .and_then(|value| value.as_str())
        .filter(|value| value.starts_with("sha256:") && value.len() > "sha256:".len());
    if expected_absent {
        expected_digest.is_none()
    } else {
        expected_digest.is_some()
    }
}

fn is_path_in_safe_paths(path: Option<&str>, safe_paths: &[String]) -> bool {
    let Some(path) = path else {
        return false;
    };
    let normalized = normalize_path(path);
    safe_paths.iter().any(|safe_path| {
        let safe = normalize_path(safe_path);
        normalized == safe || normalized.starts_with(&format!("{safe}/"))
    })
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn materialization_status_for(
    proposal: &AgentProposal,
    input: &ReviewCenterBuildInput,
) -> ReviewItemMaterializationStatus {
    if let Some(status) = input.materialization_overrides.get(&proposal.id) {
        return *status;
    }
    if let Some(evidence) = input.artifact_evidence.get(&proposal.id) {
        return match evidence.state.as_str() {
            "prepared" | "staged" => ReviewItemMaterializationStatus::Applying,
            "confirmed"
                if evidence.observed_content_digest.as_deref()
                    == Some(evidence.content_digest.as_str()) =>
            {
                ReviewItemMaterializationStatus::Applied
            }
            "failed_before_effect" => ReviewItemMaterializationStatus::Failed,
            "unknown" | "confirmed" => ReviewItemMaterializationStatus::Unknown,
            _ => ReviewItemMaterializationStatus::Unknown,
        };
    }
    match proposal.status {
        ProposalStatus::Accepted => ReviewItemMaterializationStatus::Unknown,
        ProposalStatus::Rejected | ProposalStatus::Expired => {
            ReviewItemMaterializationStatus::NotApplicable
        }
        ProposalStatus::Pending | ProposalStatus::Edited | ProposalStatus::Postponed => {
            ReviewItemMaterializationStatus::NotStarted
        }
    }
}

fn evidence_refs_for(proposal: &AgentProposal) -> Vec<EvidenceRef> {
    let mut refs = vec![EvidenceRef {
        id: format!("proposal:{}", proposal.id),
        label: "Proposal record".into(),
        source: EvidenceSource::Review,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    }];
    if let Some(run_id) = &proposal.run_id {
        refs.push(EvidenceRef {
            id: format!("run:{run_id}"),
            label: "Agent run".into(),
            source: EvidenceSource::Task,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        });
    }
    refs
}

fn target_refs_for(proposal: &AgentProposal) -> Vec<BackendEntityRef> {
    let mut refs = vec![BackendEntityRef {
        id: proposal.id.clone(),
        kind: BackendEntityKind::Proposal,
        label: "Proposal".into(),
        href: None,
    }];
    refs.push(BackendEntityRef {
        id: proposal.affected_path.clone(),
        kind: target_kind_for(proposal.proposal_type),
        label: target_label_for(proposal.proposal_type).into(),
        href: None,
    });
    if let Some(run_id) = &proposal.run_id {
        refs.push(BackendEntityRef {
            id: run_id.clone(),
            kind: BackendEntityKind::Run,
            label: "Agent run".into(),
            href: None,
        });
    }
    refs
}

fn target_kind_for(proposal_type: ProposalType) -> BackendEntityKind {
    match proposal_type {
        ProposalType::MemoryWrite | ProposalType::MemoryArchive => BackendEntityKind::Memory,
        ProposalType::ToolPermission => BackendEntityKind::ToolPermission,
        ProposalType::ExternalWriteAction | ProposalType::DataExport => {
            BackendEntityKind::ExternalResource
        }
        ProposalType::ScheduledTask | ProposalType::ScheduleCheckin => BackendEntityKind::Schedule,
        ProposalType::ModelPolicyChange => BackendEntityKind::Policy,
        _ => BackendEntityKind::LifeModel,
    }
}

fn target_label_for(proposal_type: ProposalType) -> &'static str {
    match proposal_type {
        ProposalType::MemoryWrite | ProposalType::MemoryArchive => "Memory",
        ProposalType::ToolPermission => "Tool permission",
        ProposalType::ExternalWriteAction => "External write",
        ProposalType::DataExport => "Data export",
        ProposalType::ScheduledTask | ProposalType::ScheduleCheckin => "Schedule",
        ProposalType::ModelPolicyChange => "Model policy",
        _ => "LifeModel",
    }
}

fn task_resume_relation_for(
    proposal: &AgentProposal,
    status: ReviewItemDecisionStatus,
    materialization_status: ReviewItemMaterializationStatus,
    input: &ReviewCenterBuildInput,
) -> Option<ReviewItemTaskResumeRelation> {
    let task_session_id = input
        .terminal_owner_task_session_ids
        .get(&proposal.id)?
        .clone();
    let resume_requires_materialization = resume_requires_materialization(proposal.proposal_type);
    let materialization_allows_resume = !resume_requires_materialization
        || matches!(
            materialization_status,
            ReviewItemMaterializationStatus::Applied
                | ReviewItemMaterializationStatus::NotApplicable
        );
    let can_request_resume =
        status == ReviewItemDecisionStatus::Approved && materialization_allows_resume;
    let resume_action_id = Some(format!("{}:resume", proposal.id));
    let blocked_reason = if status != ReviewItemDecisionStatus::Approved {
        Some("Approve before requesting task resume.".into())
    } else if resume_requires_materialization && !materialization_allows_resume {
        Some(match materialization_status {
            ReviewItemMaterializationStatus::Unknown => {
                "Materialization evidence is unknown; cannot request task resume yet.".into()
            }
            ReviewItemMaterializationStatus::Failed => {
                "Materialization failed; cannot request task resume yet.".into()
            }
            ReviewItemMaterializationStatus::RolledBack => {
                "Materialization was rolled back; cannot request task resume yet.".into()
            }
            ReviewItemMaterializationStatus::Applying => {
                "Materialization is still applying; cannot request task resume yet.".into()
            }
            ReviewItemMaterializationStatus::NotStarted => {
                "Materialization has not started; cannot request task resume yet.".into()
            }
            ReviewItemMaterializationStatus::Applied
            | ReviewItemMaterializationStatus::NotApplicable => unreachable!(
                "allowed materialization states are handled before constructing blocker"
            ),
        })
    } else {
        None
    };
    Some(ReviewItemTaskResumeRelation {
        task_session_id,
        resume_requires_materialization,
        can_request_resume,
        resume_action_id,
        blocked_reason,
    })
}

fn resume_requires_materialization(proposal_type: ProposalType) -> bool {
    !matches!(proposal_type, ProposalType::ToolPermission)
}

fn risk_for(risk: RiskLevel) -> ProductRiskLevel {
    match risk {
        RiskLevel::Low => ProductRiskLevel::Low,
        RiskLevel::Medium => ProductRiskLevel::Medium,
        RiskLevel::High => ProductRiskLevel::High,
        RiskLevel::Critical => ProductRiskLevel::Critical,
    }
}

fn risk_key(risk: ProductRiskLevel) -> &'static str {
    match risk {
        ProductRiskLevel::None => "none",
        ProductRiskLevel::Low => "low",
        ProductRiskLevel::Medium => "medium",
        ProductRiskLevel::High => "high",
        ProductRiskLevel::Critical => "critical",
        ProductRiskLevel::Unknown => "unknown",
    }
}

fn materialization_key(status: ReviewItemMaterializationStatus) -> &'static str {
    match status {
        ReviewItemMaterializationStatus::NotApplicable => "not_applicable",
        ReviewItemMaterializationStatus::NotStarted => "not_started",
        ReviewItemMaterializationStatus::Applying => "applying",
        ReviewItemMaterializationStatus::Applied => "applied",
        ReviewItemMaterializationStatus::Failed => "failed",
        ReviewItemMaterializationStatus::RolledBack => "rolled_back",
        ReviewItemMaterializationStatus::Unknown => "unknown",
    }
}

fn increment(map: &mut BTreeMap<String, usize>, key: &str) {
    *map.entry(key.to_string()).or_insert(0) += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{ProviderPrivacyBoundarySummary, WorkspaceActivityItem};
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ReviewItemContractGolden {
        schema_version: String,
        review_item: ReviewItem,
        workspace_activity: WorkspaceActivityItem,
        provider_boundary: ProviderPrivacyBoundarySummary,
    }

    fn proposal(proposal_type: ProposalType) -> AgentProposal {
        AgentProposal::new(
            proposal_type,
            "goals.0.title",
            json!("new value"),
            "test proposal",
            0.8,
            RiskLevel::Medium,
            ProposalSource::Manual,
        )
    }

    fn find_action(item: &ReviewItem, kind: ReviewActionKind) -> &ReviewAction {
        item.allowed_actions
            .iter()
            .find(|action| action.kind == kind)
            .expect("action exists")
    }

    #[test]
    fn review_item_contract_golden_round_trips_and_preserves_action_invariants() {
        let raw = include_str!("../../../test-fixtures/contracts/review-item-contract-golden.json");
        let parsed: serde_json::Value = serde_json::from_str(raw).expect("parse golden JSON");
        let golden: ReviewItemContractGolden =
            serde_json::from_value(parsed.clone()).expect("deserialize Rust contract");

        assert_eq!(golden.schema_version, "openlife.phase4a-contract.v1");
        assert!(golden
            .review_item
            .allowed_actions
            .iter()
            .all(|action| action.validate().is_ok()));
        assert!(golden
            .review_item
            .decision_context
            .permission
            .as_ref()
            .is_some_and(|permission| permission.is_ready()));
        assert_eq!(
            serde_json::to_value(golden).expect("serialize Rust contract"),
            parsed
        );
    }

    #[test]
    fn review_item_external_write_outside_safe_paths_disables_approve_and_edit() {
        let mut proposal = proposal(ProposalType::ExternalWriteAction);
        proposal.after = json!({ "path": "/outside/private.txt" });
        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            safe_paths: vec!["/allowed".into()],
            ..Default::default()
        });

        let item = &model.items[0];
        let approve = find_action(item, ReviewActionKind::Approve);
        let edit = find_action(item, ReviewActionKind::Edit);
        let reject = find_action(item, ReviewActionKind::Reject);

        assert!(!approve.enabled);
        assert_eq!(approve.effect, approve.kind.expected_effect());
        assert!(approve
            .disabled_reason
            .as_deref()
            .unwrap()
            .contains("outside configured safe paths"));
        assert!(!edit.enabled);
        assert!(reject.enabled);
        assert_eq!(model.summary.blocked_action_count, 2);
    }

    #[test]
    fn review_item_file_trash_inside_safe_path_enables_approval() {
        let mut proposal = proposal(ProposalType::ExternalWriteAction);
        proposal.after = json!({
            "operation": "trash",
            "source_path": "/allowed/report.txt",
            "target_path": "/allowed/.openlife-trash-report.txt",
            "source_digest": "sha256:source"
        });
        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            safe_paths: vec!["/allowed".into()],
            ..Default::default()
        });

        let item = &model.items[0];
        assert!(find_action(item, ReviewActionKind::Approve).enabled);
        assert!(find_action(item, ReviewActionKind::Edit).enabled);
    }

    #[test]
    fn review_item_file_write_without_reviewed_target_state_stays_blocked() {
        let mut proposal = proposal(ProposalType::ExternalWriteAction);
        proposal.after = json!({
            "operation": "propose_write",
            "path": "/allowed/report.txt",
            "content": "report"
        });
        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            safe_paths: vec!["/allowed".into()],
            ..Default::default()
        });

        let item = &model.items[0];
        let approve = find_action(item, ReviewActionKind::Approve);
        assert!(!approve.enabled);
        assert!(approve
            .disabled_reason
            .as_deref()
            .unwrap()
            .contains("target state is incomplete"));
        assert!(find_action(item, ReviewActionKind::Reject).enabled);
    }

    #[test]
    fn review_item_file_move_with_unsafe_target_stays_blocked() {
        let mut proposal = proposal(ProposalType::ExternalWriteAction);
        proposal.after = json!({
            "operation": "move",
            "source_path": "/allowed/report.txt",
            "target_path": "/outside/report.txt",
            "source_digest": "sha256:source"
        });
        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            safe_paths: vec!["/allowed".into()],
            ..Default::default()
        });

        let item = &model.items[0];
        assert!(!find_action(item, ReviewActionKind::Approve).enabled);
        assert!(!find_action(item, ReviewActionKind::Edit).enabled);
        assert!(find_action(item, ReviewActionKind::Reject).enabled);
    }

    #[test]
    fn artifact_receipt_projects_applied_only_when_observed_digest_matches() {
        let mut proposal = proposal(ProposalType::ExternalWriteAction);
        proposal.affected_path = "/safe/report.md".into();
        proposal.after = json!({ "path": "/safe/report.md", "content": "report" });
        let evidence = ReviewItemArtifactEvidence {
            state: "confirmed".into(),
            target_reference_digest: "sha256:target".into(),
            content_digest: "sha256:content".into(),
            observed_content_digest: Some("sha256:content".into()),
            byte_size: 6,
            media_type: "text/markdown; charset=utf-8".into(),
            error_code: None,
        };
        let input = ReviewCenterBuildInput {
            proposals: vec![proposal.clone()],
            safe_paths: vec!["/safe".into()],
            artifact_evidence: BTreeMap::from([(proposal.id.clone(), evidence.clone())]),
            ..Default::default()
        };

        let item = build_review_item(&proposal, &input);
        assert_eq!(
            item.materialization_status,
            ReviewItemMaterializationStatus::Applied
        );
        assert_eq!(item.artifact_evidence, Some(evidence));

        let mismatched = ReviewCenterBuildInput {
            artifact_evidence: BTreeMap::from([(
                proposal.id.clone(),
                ReviewItemArtifactEvidence {
                    observed_content_digest: Some("sha256:other".into()),
                    ..item.artifact_evidence.unwrap()
                },
            )]),
            ..input
        };
        assert_eq!(
            build_review_item(&proposal, &mismatched).materialization_status,
            ReviewItemMaterializationStatus::Unknown
        );
    }

    #[test]
    fn review_item_incomplete_tool_permission_disables_approve_but_keeps_reject_available() {
        let proposal = proposal(ProposalType::ToolPermission);
        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            ..Default::default()
        });
        let item = &model.items[0];
        let approve = find_action(item, ReviewActionKind::Approve);

        assert!(!approve.enabled);
        assert!(approve
            .disabled_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("Exact permission scope is incomplete")));
        assert!(find_action(item, ReviewActionKind::Reject).enabled);
        assert!(!item
            .decision_context
            .permission
            .as_ref()
            .expect("permission context")
            .is_ready());
    }

    #[test]
    fn builder_batch_read_model_disables_the_unimplemented_generic_editor() {
        let mut proposal = proposal(ProposalType::LifeModelUpdate);
        proposal.source = ProposalSource::BuilderReview;
        proposal.affected_path = crate::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH.into();
        proposal.after = json!({
            "schemaVersion": crate::life_model::patch::LIFEMODEL_PATCH_BATCH_SCHEMA_V1,
            "operations": [{
                "candidateId": "candidate-1",
                "path": "identity.name",
                "candidate": "Alex"
            }]
        });

        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            ..Default::default()
        });
        let edit = find_action(&model.items[0], ReviewActionKind::Edit);
        assert!(!edit.enabled);
        assert!(edit
            .disabled_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("typed Builder editor")));
        assert!(find_action(&model.items[0], ReviewActionKind::Approve).enabled);
    }

    #[test]
    fn accepted_proposal_without_materialization_evidence_is_unknown_not_applied() {
        let mut proposal = proposal(ProposalType::GoalUpdate);
        proposal.status = ProposalStatus::Accepted;

        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            ..Default::default()
        });

        let item = &model.items[0];
        assert_eq!(item.status, ReviewItemDecisionStatus::Approved);
        assert_eq!(
            item.materialization_status,
            ReviewItemMaterializationStatus::Unknown
        );
        assert_eq!(model.summary.by_materialization_status.get("applied"), None);
        assert_eq!(
            model.summary.by_materialization_status.get("unknown"),
            Some(&1)
        );
    }

    #[test]
    fn materialization_override_is_required_before_item_reports_applied() {
        let mut proposal = proposal(ProposalType::MemoryWrite);
        proposal.status = ProposalStatus::Accepted;
        let mut overrides = BTreeMap::new();
        overrides.insert(
            proposal.id.clone(),
            ReviewItemMaterializationStatus::Applied,
        );

        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            materialization_overrides: overrides,
            ..Default::default()
        });

        assert_eq!(
            model.items[0].materialization_status,
            ReviewItemMaterializationStatus::Applied
        );
        assert_eq!(
            model.summary.by_materialization_status.get("applied"),
            Some(&1)
        );
    }

    #[test]
    fn accepted_durable_proposal_with_unknown_materialization_cannot_request_resume() {
        let mut proposal = proposal(ProposalType::MemoryWrite);
        proposal.source = ProposalSource::ChatConversation;
        proposal.source_detail = Some("task-session-1".into());
        proposal.status = ProposalStatus::Accepted;
        let proposal_id = proposal.id.clone();

        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            terminal_owner_task_session_ids: BTreeMap::from([(
                proposal_id,
                "task-session-1".into(),
            )]),
            ..Default::default()
        });

        let item = &model.items[0];
        let resume = find_action(item, ReviewActionKind::Resume);
        assert!(!resume.enabled);
        assert_eq!(resume.effect, resume.kind.expected_effect());
        assert_eq!(
            item.task_resume_relation
                .as_ref()
                .map(|relation| relation.can_request_resume),
            Some(false)
        );
        assert_eq!(
            item.task_resume_relation
                .as_ref()
                .map(|relation| relation.resume_requires_materialization),
            Some(true)
        );
        assert_eq!(
            item.task_resume_relation
                .as_ref()
                .and_then(|relation| relation.blocked_reason.as_deref()),
            Some("Materialization evidence is unknown; cannot request task resume yet.")
        );
        assert_eq!(
            item.materialization_status,
            ReviewItemMaterializationStatus::Unknown
        );
    }

    #[test]
    fn applied_durable_proposal_can_request_resume_as_request_not_completion_proof() {
        let mut proposal = proposal(ProposalType::MemoryWrite);
        proposal.source = ProposalSource::ChatConversation;
        proposal.source_detail = Some("task-session-1".into());
        proposal.status = ProposalStatus::Accepted;
        let proposal_id = proposal.id.clone();
        let mut overrides = BTreeMap::new();
        overrides.insert(
            proposal.id.clone(),
            ReviewItemMaterializationStatus::Applied,
        );

        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            materialization_overrides: overrides,
            terminal_owner_task_session_ids: BTreeMap::from([(
                proposal_id,
                "task-session-1".into(),
            )]),
            ..Default::default()
        });

        let item = &model.items[0];
        let resume = find_action(item, ReviewActionKind::Resume);
        assert!(resume.enabled);
        assert_eq!(resume.effect, resume.kind.expected_effect());
        assert_eq!(
            item.task_resume_relation
                .as_ref()
                .map(|relation| relation.can_request_resume),
            Some(true)
        );
        assert_eq!(
            item.task_resume_relation
                .as_ref()
                .map(|relation| relation.resume_requires_materialization),
            Some(true)
        );
    }

    #[test]
    fn accepted_tool_permission_resume_declares_materialization_not_required() {
        let mut proposal = proposal(ProposalType::ToolPermission);
        proposal.source = ProposalSource::ChatConversation;
        proposal.source_detail = Some("task-session-1".into());
        proposal.status = ProposalStatus::Accepted;
        let proposal_id = proposal.id.clone();

        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            terminal_owner_task_session_ids: BTreeMap::from([(
                proposal_id,
                "task-session-1".into(),
            )]),
            ..Default::default()
        });

        let item = &model.items[0];
        let resume = find_action(item, ReviewActionKind::Resume);
        assert!(resume.enabled);
        assert_eq!(resume.effect, resume.kind.expected_effect());
        assert_eq!(
            item.task_resume_relation
                .as_ref()
                .map(|relation| relation.resume_requires_materialization),
            Some(false)
        );
        assert_eq!(
            item.materialization_status,
            ReviewItemMaterializationStatus::Unknown
        );
    }

    #[test]
    fn review_batches_group_same_session_by_domain_without_batch_authorization() {
        let mut memory_one = proposal(ProposalType::MemoryWrite);
        memory_one.source = ProposalSource::MemoryGovernance;
        memory_one.source_detail =
            Some("main_chat_agent_task_session:task-1;candidate:first".into());
        memory_one.risk_level = RiskLevel::Medium;
        let mut memory_two = proposal(ProposalType::MemoryWrite);
        memory_two.source = ProposalSource::MemoryGovernance;
        memory_two.source_detail =
            Some("main_chat_agent_task_session:task-1;candidate:second".into());
        memory_two.risk_level = RiskLevel::High;
        let mut life_model = proposal(ProposalType::LifeModelUpdate);
        life_model.source = ProposalSource::MemoryGovernance;
        life_model.after = json!({ "originatingTaskSessionId": "task-1" });

        let memory_ids = vec![memory_one.id.clone(), memory_two.id.clone()];
        let terminal_owner_task_session_ids = [
            memory_one.id.clone(),
            memory_two.id.clone(),
            life_model.id.clone(),
        ]
        .into_iter()
        .map(|proposal_id| (proposal_id, "task-1".into()))
        .collect();
        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![memory_one, memory_two, life_model],
            terminal_owner_task_session_ids,
            ..Default::default()
        });

        assert_eq!(model.items.len(), 3);
        assert_eq!(model.batches.len(), 2);
        let memory_batch = model
            .batches
            .iter()
            .find(|batch| batch.domain == ReviewBatchDomain::Memory)
            .expect("Memory batch");
        assert_eq!(memory_batch.session_id.as_deref(), Some("task-1"));
        assert_eq!(memory_batch.item_ids, memory_ids);
        assert_eq!(memory_batch.action_required_count, 2);
        assert_eq!(memory_batch.highest_risk, ProductRiskLevel::High);
        assert!(memory_batch.id.starts_with("review_batch:memory:sha256:"));
        assert!(model.items[..2].iter().all(|item| {
            item.task_resume_relation
                .as_ref()
                .is_some_and(|relation| relation.task_session_id == "task-1")
        }));
        assert!(
            serde_json::to_value(memory_batch)
                .unwrap()
                .get("allowedActions")
                .is_none(),
            "ReviewBatch must not become a second authorization surface"
        );
    }

    #[test]
    fn forged_source_detail_and_payload_cannot_create_task_relation_or_batch_owner() {
        let mut proposal = proposal(ProposalType::MemoryWrite);
        proposal.source = ProposalSource::ChatConversation;
        proposal.source_detail = Some("forged-task".into());
        proposal.after = json!({
            "sourceTaskSessionId": "forged-task",
            "taskSessionId": "forged-task",
            "session_id": "forged-task"
        });
        proposal.status = ProposalStatus::Accepted;

        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            ..Default::default()
        });

        assert!(
            model.items[0].task_resume_relation.is_none(),
            "descriptive Proposal fields cannot mint a task resume authority"
        );
        assert_eq!(model.batches.len(), 1);
        assert!(
            model.batches[0].session_id.is_none(),
            "descriptive Proposal fields cannot mint a ReviewBatch task owner"
        );
    }
}
