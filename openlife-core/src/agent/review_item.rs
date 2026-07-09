use crate::agent::product_read_model::{
    BackendEntityKind, BackendEntityRef, EvidenceRef, EvidenceSensitivity, EvidenceSource,
    ProductRiskLevel, ReviewAction, ReviewActionKind, ReviewItemMaterializationStatus,
};
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
pub struct ReviewItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: ReviewItemType,
    pub source: ReviewItemSource,
    pub status: ReviewItemDecisionStatus,
    pub materialization_status: ReviewItemMaterializationStatus,
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

    ReviewCenterViewModel { items, summary }
}

pub fn build_review_item(proposal: &AgentProposal, input: &ReviewCenterBuildInput) -> ReviewItem {
    let item_id = proposal.id.clone();
    let status = ReviewItemDecisionStatus::from(proposal.status);
    let materialization_status = materialization_status_for(proposal, input);
    let evidence_refs = evidence_refs_for(proposal);
    let target_refs = target_refs_for(proposal);
    let task_resume_relation = task_resume_relation_for(proposal, status, materialization_status);
    let allowed_actions = allowed_actions_for(
        proposal,
        &item_id,
        status,
        materialization_status,
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
        allowed_actions,
        risk: risk_for(proposal.risk_level),
        expires_at: proposal.expires_at,
        evidence_refs,
        target_refs,
        task_resume_relation,
    }
}

fn allowed_actions_for(
    proposal: &AgentProposal,
    item_id: &str,
    status: ReviewItemDecisionStatus,
    materialization_status: ReviewItemMaterializationStatus,
    task_resume_relation: Option<&ReviewItemTaskResumeRelation>,
    input: &ReviewCenterBuildInput,
) -> Vec<ReviewAction> {
    let mut actions = Vec::new();
    let approve_blocker = approve_blocker(proposal, input);
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
    if proposal.proposal_type == ProposalType::ExternalWriteAction
        && !is_path_in_safe_paths(external_write_path(proposal), &input.safe_paths)
    {
        return Some("The external write path is outside configured safe paths.".into());
    }
    None
}

fn approve_blocker(proposal: &AgentProposal, input: &ReviewCenterBuildInput) -> Option<String> {
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
    if proposal.proposal_type == ProposalType::ExternalWriteAction
        && !is_path_in_safe_paths(external_write_path(proposal), &input.safe_paths)
    {
        return Some("The external write path is outside configured safe paths.".into());
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

fn external_write_path(proposal: &AgentProposal) -> Option<&str> {
    proposal
        .after
        .get("path")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
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
    match proposal.status {
        ProposalStatus::Accepted => ReviewItemMaterializationStatus::Unknown,
        ProposalStatus::Rejected => ReviewItemMaterializationStatus::NotApplicable,
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
) -> Option<ReviewItemTaskResumeRelation> {
    if !matches!(proposal.source, ProposalSource::ChatConversation) {
        return None;
    }
    let task_session_id = proposal
        .source_detail
        .as_ref()
        .filter(|value| !value.trim().is_empty())?
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
    use serde_json::json;

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

    fn find_action<'a>(item: &'a ReviewItem, kind: ReviewActionKind) -> &'a ReviewAction {
        item.allowed_actions
            .iter()
            .find(|action| action.kind == kind)
            .expect("action exists")
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

        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
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

        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
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
}
