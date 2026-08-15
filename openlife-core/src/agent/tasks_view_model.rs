use crate::agent::product_read_model::{
    BackendEntityKind, BackendEntityRef, EvidenceRef, EvidenceSensitivity, EvidenceSource,
    ProviderPrivacyBoundarySummary,
};
use crate::agent::review_item::{ReviewItem, ReviewItemDecisionStatus};
use crate::task_runtime::{
    CanonicalArtifactStatus, CanonicalTaskItemKind, CanonicalTaskItemStatus,
};
use crate::work_orchestration::{WorkCompletionContract, WorkPlanStepKind, WorkRunBudgetPolicy};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskLifecycleStatus {
    Running,
    WaitingReview,
    WaitingPermission,
    Blocked,
    Failed,
    RemoteUnknown,
    Cancelled,
    Completed,
    CompletedWithPendingReview,
    CompletedNeedsEvidence,
    Unknown,
}

impl TaskLifecycleStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::WaitingReview => "waiting_review",
            Self::WaitingPermission => "waiting_permission",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::RemoteUnknown => "remote_unknown",
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
            Self::CompletedWithPendingReview => "completed_with_pending_review",
            Self::CompletedNeedsEvidence => "completed_needs_evidence",
            Self::Unknown => "unknown",
        }
    }

    fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::CompletedWithPendingReview
                | Self::CompletedNeedsEvidence
                | Self::Failed
                | Self::RemoteUnknown
                | Self::Cancelled
        )
    }

    fn is_active(self) -> bool {
        matches!(
            self,
            Self::Running | Self::WaitingReview | Self::WaitingPermission | Self::Blocked
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskTerminalDeliveryStatus {
    NotTerminal,
    Delivered,
    MissingFinalDeliveryEvidence,
    CompletedWithPendingReview,
    Blocked,
    Failed,
    Cancelled,
    Unknown,
}

impl TaskTerminalDeliveryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotTerminal => "not_terminal",
            Self::Delivered => "delivered",
            Self::MissingFinalDeliveryEvidence => "missing_final_delivery_evidence",
            Self::CompletedWithPendingReview => "completed_with_pending_review",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskControlKind {
    Resume,
    Retry,
    Cancel,
    RefreshContext,
    OpenTrace,
    OpenRun,
    OpenReviewItem,
    ViewEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskControlEffect {
    TaskResumeRequest,
    TaskRetryRequest,
    TaskCancelRequest,
    TaskRefreshRequest,
    NavigationOnly,
    EvidenceOnly,
}

impl TaskControlKind {
    pub fn expected_effect(self) -> TaskControlEffect {
        match self {
            Self::Resume => TaskControlEffect::TaskResumeRequest,
            Self::Retry => TaskControlEffect::TaskRetryRequest,
            Self::Cancel => TaskControlEffect::TaskCancelRequest,
            Self::RefreshContext => TaskControlEffect::TaskRefreshRequest,
            Self::OpenRun | Self::OpenReviewItem => TaskControlEffect::NavigationOnly,
            Self::OpenTrace | Self::ViewEvidence => TaskControlEffect::EvidenceOnly,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskControl {
    pub id: String,
    pub label: String,
    pub kind: TaskControlKind,
    pub effect: TaskControlEffect,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    #[serde(default)]
    pub requires_confirmation: bool,
    pub target_task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_action_id: Option<String>,
    #[serde(default)]
    pub completion_proof_after_dispatch: bool,
}

impl TaskControl {
    pub fn new(
        task_id: impl Into<String>,
        suffix: impl Into<String>,
        label: impl Into<String>,
        kind: TaskControlKind,
    ) -> Self {
        let task_id = task_id.into();
        let suffix = suffix.into();
        Self {
            id: format!("{task_id}:{suffix}"),
            label: label.into(),
            kind,
            effect: kind.expected_effect(),
            enabled: true,
            disabled_reason: None,
            requires_confirmation: false,
            target_task_id: task_id,
            target_action_id: None,
            completion_proof_after_dispatch: false,
        }
    }

    pub fn disabled(mut self, reason: impl Into<String>) -> Self {
        self.enabled = false;
        self.disabled_reason = Some(reason.into());
        self
    }

    pub fn requiring_confirmation(mut self) -> Self {
        self.requires_confirmation = true;
        self
    }

    pub fn with_target_action_id(mut self, action_id: impl Into<String>) -> Self {
        self.target_action_id = Some(action_id.into());
        self
    }

    pub fn validate(&self) -> Result<(), TaskViewModelContractError> {
        let expected = self.kind.expected_effect();
        if self.effect != expected {
            return Err(TaskViewModelContractError::TaskControlEffectMismatch {
                kind: self.kind,
                expected,
                actual: self.effect,
            });
        }
        if self.completion_proof_after_dispatch {
            return Err(TaskViewModelContractError::ControlClaimsCompletionProof {
                id: self.id.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLatestResultPreview {
    pub status: TaskTerminalDeliveryStatus,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_delivery_ref: Option<BackendEntityRef>,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItemViewModel {
    pub id: String,
    pub run_id: String,
    pub sequence: u64,
    pub kind: CanonicalTaskItemKind,
    pub status: CanonicalTaskItemStatus,
    pub summary_code: String,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskWorkPlanStepViewModel {
    pub id: String,
    pub kind: WorkPlanStepKind,
    pub required: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskWorkPlanViewModel {
    pub revision: u64,
    pub steps: Vec<TaskWorkPlanStepViewModel>,
    pub completion: WorkCompletionContract,
    pub budget_policy: WorkRunBudgetPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactViewModel {
    pub artifact_id: String,
    pub version: u64,
    pub status: CanonicalArtifactStatus,
    pub media_type: String,
    pub content_digest: String,
    pub target_reference_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub materialized_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_content_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_ref: Option<BackendEntityRef>,
    pub source_item_ref: BackendEntityRef,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
    pub change: TaskArtifactChangeViewModel,
    pub preview: TaskArtifactPreviewViewModel,
    pub verification: TaskArtifactVerificationViewModel,
    pub undo: TaskArtifactUndoViewModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactUndoViewModel {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_ref: Option<BackendEntityRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskArtifactChangeKind {
    Create,
    Replace,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactChangeViewModel {
    pub kind: TaskArtifactChangeKind,
    pub status: CanonicalArtifactStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_prior_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskArtifactPreviewStatus {
    Available,
    Truncated,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactPreviewViewModel {
    pub status: TaskArtifactPreviewStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskArtifactVerificationStatus {
    Pending,
    Verified,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactVerificationViewModel {
    pub status: TaskArtifactVerificationStatus,
    pub expected_content_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_content_digest: Option<String>,
    pub verification_item_present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskViewModelItem {
    pub canonical_task_id: String,
    #[serde(default)]
    pub related_run_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub title: String,
    pub lifecycle_status: TaskLifecycleStatus,
    pub terminal_delivery_status: TaskTerminalDeliveryStatus,
    pub final_delivery_evidence_present: bool,
    #[serde(default)]
    pub items: Vec<TaskItemViewModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_plan: Option<TaskWorkPlanViewModel>,
    #[serde(default)]
    pub artifacts: Vec<TaskArtifactViewModel>,
    #[serde(default)]
    pub pending_blockers: Vec<String>,
    #[serde(default)]
    pub needs_attention: bool,
    #[serde(default)]
    pub attention_reason_codes: Vec<String>,
    #[serde(default)]
    pub pending_review_item_refs: Vec<BackendEntityRef>,
    #[serde(default)]
    pub allowed_controls: Vec<TaskControl>,
    pub next_recommended_control: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_result_preview: Option<TaskLatestResultPreview>,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TasksViewModelSummary {
    pub total: usize,
    pub needs_attention_count: usize,
    pub active_count: usize,
    pub waiting_review_count: usize,
    pub waiting_permission_count: usize,
    pub blocked_count: usize,
    pub pending_review_count: usize,
    pub completed_count: usize,
    pub completed_needs_evidence_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
    #[serde(default)]
    pub by_lifecycle_status: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TasksViewModel {
    pub items: Vec<TaskViewModelItem>,
    pub summary: TasksViewModelSummary,
    #[serde(default)]
    pub source_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub contract_limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceActivityKind {
    UserInput,
    RouteDecision,
    Plan,
    Action,
    Observation,
    FollowUp,
    PermissionRequest,
    ProposalRequest,
    Error,
    Retry,
    FinalResult,
    Reflection,
    Fallback,
    Blocker,
    DurableLifecycle,
    Unknown,
}

impl WorkspaceActivityKind {
    pub fn from_product_code(value: &str) -> Self {
        match value {
            "user_input" | "instruction" => Self::UserInput,
            "route_decision" => Self::RouteDecision,
            "plan" => Self::Plan,
            "action" | "tool_call" | "provider_generation" => Self::Action,
            "observation" | "verification" => Self::Observation,
            "follow_up" | "steering" => Self::FollowUp,
            "permission_request" => Self::PermissionRequest,
            "proposal_request" | "review_checkpoint" => Self::ProposalRequest,
            "error" => Self::Error,
            "retry" => Self::Retry,
            "final_result" => Self::FinalResult,
            "reflection" => Self::Reflection,
            "fallback" => Self::Fallback,
            "blocker" => Self::Blocker,
            value
                if value.starts_with("turn_")
                    || value.contains("lifecycle")
                    || matches!(value, "artifact_draft" | "artifact_materialized") =>
            {
                Self::DurableLifecycle
            }
            _ => Self::Unknown,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::UserInput => "Request recorded",
            Self::RouteDecision => "Route selected",
            Self::Plan => "Plan updated",
            Self::Action => "Action requested",
            Self::Observation => "Result observed",
            Self::FollowUp => "Follow-up requested",
            Self::PermissionRequest => "Permission required",
            Self::ProposalRequest => "Review required",
            Self::Error => "Execution failed",
            Self::Retry => "Retry requested",
            Self::FinalResult => "Final result recorded",
            Self::Reflection => "Agent reflection recorded",
            Self::Fallback => "Fallback recorded",
            Self::Blocker => "Execution blocked",
            Self::DurableLifecycle => "Durable task state recorded",
            Self::Unknown => "Unclassified activity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceActivityStatus {
    Recorded,
    WaitingDecision,
    Blocked,
    Failed,
    Completed,
    Unknown,
}

impl WorkspaceActivityStatus {
    pub fn from_product_codes(
        kind: WorkspaceActivityKind,
        lifecycle_state: Option<&str>,
        failure_kind: Option<&str>,
    ) -> Self {
        if failure_kind.is_some_and(|failure| failure != "policy_blocker") {
            return Self::Failed;
        }
        match lifecycle_state {
            Some("failed" | "timed_out" | "interrupted") => Self::Failed,
            Some("blocked" | "waiting_permission") => Self::Blocked,
            Some("completed" | "delivered") => Self::Completed,
            Some("unknown") => Self::Unknown,
            _ if matches!(
                kind,
                WorkspaceActivityKind::PermissionRequest | WorkspaceActivityKind::ProposalRequest
            ) =>
            {
                Self::WaitingDecision
            }
            _ if kind == WorkspaceActivityKind::Blocker => Self::Blocked,
            _ if kind == WorkspaceActivityKind::Error => Self::Failed,
            _ if kind == WorkspaceActivityKind::Unknown => Self::Unknown,
            _ => Self::Recorded,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceActivityItem {
    pub id: String,
    pub kind: WorkspaceActivityKind,
    pub label: String,
    pub summary: String,
    pub status: WorkspaceActivityStatus,
    pub evidence_refs: Vec<EvidenceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_at: Option<DateTime<Utc>>,
}

impl WorkspaceActivityItem {
    pub fn from_product_event(
        id: impl Into<String>,
        kind_code: &str,
        summary: impl Into<String>,
        lifecycle_state: Option<&str>,
        failure_kind: Option<&str>,
        evidence_refs: Vec<EvidenceRef>,
        occurred_at: Option<DateTime<Utc>>,
    ) -> Self {
        let kind = WorkspaceActivityKind::from_product_code(kind_code);
        Self {
            id: id.into(),
            kind,
            label: kind.label().into(),
            summary: summary.into(),
            status: WorkspaceActivityStatus::from_product_codes(
                kind,
                lifecycle_state,
                failure_kind,
            ),
            evidence_refs,
            occurred_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_conversation_id: Option<String>,
    #[serde(default)]
    pub tasks: Vec<TaskViewModelItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_task: Option<TaskViewModelItem>,
    #[serde(default)]
    pub recent_task_refs: Vec<BackendEntityRef>,
    #[serde(default)]
    pub pending_review_items: Vec<ReviewItem>,
    #[serde(default)]
    pub activity: Vec<WorkspaceActivityItem>,
    pub provider_privacy_boundary_summary: ProviderPrivacyBoundarySummary,
    pub activity_redaction_state: String,
    #[serde(default)]
    pub source_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub contract_limitations: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskViewModelTaskInput {
    pub task_id: String,
    pub canonical_task_id: Option<String>,
    pub conversation_id: Option<String>,
    pub title: String,
    pub related_run_ids: Vec<String>,
    pub final_delivery_present: bool,
    pub final_delivery_status: Option<String>,
    pub canonical_lifecycle_status: Option<TaskLifecycleStatus>,
    pub canonical_terminal_delivery_status: Option<TaskTerminalDeliveryStatus>,
    pub canonical_final_delivery_evidence_present: Option<bool>,
    pub canonical_items: Vec<TaskItemViewModel>,
    pub work_plan: Option<TaskWorkPlanViewModel>,
    pub canonical_artifacts: Vec<TaskArtifactViewModel>,
    pub pending_blockers: Vec<String>,
    pub needs_attention: bool,
    pub attention_reason_codes: Vec<String>,
    pub pending_review_item_refs: Vec<BackendEntityRef>,
    pub review_projection_authoritative: bool,
    pub allowed_control_ids: Vec<String>,
    pub retry_action_id: Option<String>,
    pub next_recommended_control: Option<String>,
    pub latest_result_preview: Option<String>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct TasksViewModelBuildInput {
    pub task_inputs: Vec<TaskViewModelTaskInput>,
    pub source_refs: Vec<EvidenceRef>,
    pub contract_limitations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceViewModelBuildInput {
    pub tasks: TasksViewModel,
    pub selected_conversation_id: Option<String>,
    pub review_items: Vec<ReviewItem>,
    pub active_task_activity: Vec<WorkspaceActivityItem>,
    pub provider_privacy_boundary_summary: ProviderPrivacyBoundarySummary,
    pub source_refs: Vec<EvidenceRef>,
    pub contract_limitations: Vec<String>,
}

pub fn build_tasks_view_model(input: TasksViewModelBuildInput) -> TasksViewModel {
    let mut items = input
        .task_inputs
        .into_iter()
        .map(task_item_from_input)
        .collect::<Vec<_>>();

    items.sort_by_key(|item| std::cmp::Reverse(item.updated_at));
    let summary = summarize_tasks(&items);

    TasksViewModel {
        items,
        summary,
        source_refs: input.source_refs,
        contract_limitations: input.contract_limitations,
    }
}

pub fn build_workspace_view_model(input: WorkspaceViewModelBuildInput) -> WorkspaceViewModel {
    let selected_conversation_id = input.selected_conversation_id;
    let tasks = input
        .tasks
        .items
        .into_iter()
        .filter(|task| {
            selected_conversation_id
                .as_deref()
                .is_none_or(|conversation_id| {
                    task.conversation_id.as_deref() == Some(conversation_id)
                })
        })
        .collect::<Vec<_>>();
    let active_task = tasks
        .iter()
        .find(|item| item.lifecycle_status.is_active())
        .cloned();
    let recent_task_refs = tasks.iter().take(6).map(task_ref).collect::<Vec<_>>();
    let active_review_ids = active_task
        .as_ref()
        .map(|task| {
            task.pending_review_item_refs
                .iter()
                .map(|item| item.id.as_str())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let mut pending_review_items = input
        .review_items
        .iter()
        .filter(|item| {
            matches!(
                item.status,
                ReviewItemDecisionStatus::Pending
                    | ReviewItemDecisionStatus::Edited
                    | ReviewItemDecisionStatus::Deferred
            ) && active_review_ids.contains(item.id.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    pending_review_items.sort_by(|left, right| left.id.cmp(&right.id));
    let mut source_refs = input.tasks.source_refs.clone();
    source_refs.extend(input.source_refs);
    source_refs.sort_by(|left, right| left.id.cmp(&right.id));
    source_refs.dedup_by(|left, right| left.id == right.id && left.source == right.source);
    let activity = if active_task.is_some() {
        input.active_task_activity
    } else {
        Vec::new()
    };

    WorkspaceViewModel {
        selected_conversation_id,
        tasks,
        active_task,
        recent_task_refs,
        pending_review_items,
        activity,
        provider_privacy_boundary_summary: input.provider_privacy_boundary_summary,
        activity_redaction_state: "metadata_only".into(),
        source_refs,
        contract_limitations: input.contract_limitations,
    }
}

fn task_item_from_input(input: TaskViewModelTaskInput) -> TaskViewModelItem {
    let lifecycle_status = input
        .canonical_lifecycle_status
        .unwrap_or(TaskLifecycleStatus::Unknown);
    let terminal_delivery_status = input
        .canonical_terminal_delivery_status
        .unwrap_or_else(|| terminal_delivery_status_for_task(&input, lifecycle_status));
    let final_delivery_evidence_present = input
        .canonical_final_delivery_evidence_present
        .unwrap_or(input.final_delivery_present);
    let controls = controls_for_task(&input, lifecycle_status);
    let mut evidence_refs = input.evidence_refs;
    evidence_refs.push(EvidenceRef {
        id: input.task_id.clone(),
        label: "Canonical Work Task".into(),
        source: EvidenceSource::Task,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    });
    for run_id in &input.related_run_ids {
        evidence_refs.push(EvidenceRef {
            id: run_id.clone(),
            label: "Canonical Work Run".into(),
            source: EvidenceSource::Task,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        });
    }
    evidence_refs.sort_by(|left, right| left.id.cmp(&right.id));
    evidence_refs.dedup_by(|left, right| left.id == right.id && left.source == right.source);

    let latest_result_preview = latest_result_preview_for(
        terminal_delivery_status,
        input.latest_result_preview,
        final_delivery_evidence_present,
        input.canonical_task_id.as_deref().unwrap_or(&input.task_id),
    );
    let next_recommended_control = input
        .next_recommended_control
        .filter(|control| !control.trim().is_empty())
        .unwrap_or_else(|| "open_trace".into());
    let mut pending_review_item_refs = input.pending_review_item_refs;
    pending_review_item_refs.sort_by(|left, right| left.id.cmp(&right.id));
    pending_review_item_refs.dedup_by(|left, right| left.id == right.id);

    TaskViewModelItem {
        canonical_task_id: input
            .canonical_task_id
            .unwrap_or_else(|| input.task_id.clone()),
        related_run_ids: input.related_run_ids,
        conversation_id: input.conversation_id,
        title: if input.title.trim().is_empty() {
            "Untitled task".into()
        } else {
            input.title
        },
        lifecycle_status,
        terminal_delivery_status,
        final_delivery_evidence_present,
        items: input.canonical_items,
        work_plan: input.work_plan,
        artifacts: input.canonical_artifacts,
        pending_blockers: dedup_strings(input.pending_blockers),
        needs_attention: input.needs_attention,
        attention_reason_codes: dedup_strings(input.attention_reason_codes),
        pending_review_item_refs,
        allowed_controls: controls,
        next_recommended_control,
        latest_result_preview,
        evidence_refs,
        updated_at: input.updated_at,
    }
}

fn terminal_delivery_status_for_task(
    input: &TaskViewModelTaskInput,
    lifecycle_status: TaskLifecycleStatus,
) -> TaskTerminalDeliveryStatus {
    match lifecycle_status {
        TaskLifecycleStatus::CompletedWithPendingReview => {
            TaskTerminalDeliveryStatus::CompletedWithPendingReview
        }
        TaskLifecycleStatus::Completed if final_delivery_status_is_complete(input) => {
            TaskTerminalDeliveryStatus::Delivered
        }
        TaskLifecycleStatus::Completed | TaskLifecycleStatus::CompletedNeedsEvidence => {
            TaskTerminalDeliveryStatus::MissingFinalDeliveryEvidence
        }
        TaskLifecycleStatus::Blocked | TaskLifecycleStatus::WaitingPermission => {
            TaskTerminalDeliveryStatus::Blocked
        }
        TaskLifecycleStatus::WaitingReview => TaskTerminalDeliveryStatus::NotTerminal,
        TaskLifecycleStatus::Failed => TaskTerminalDeliveryStatus::Failed,
        TaskLifecycleStatus::RemoteUnknown => TaskTerminalDeliveryStatus::Unknown,
        TaskLifecycleStatus::Cancelled => TaskTerminalDeliveryStatus::Cancelled,
        TaskLifecycleStatus::Running => TaskTerminalDeliveryStatus::NotTerminal,
        TaskLifecycleStatus::Unknown => TaskTerminalDeliveryStatus::Unknown,
    }
}

fn final_delivery_status_is_complete(input: &TaskViewModelTaskInput) -> bool {
    input.final_delivery_present
        && input
            .final_delivery_status
            .as_deref()
            .map(|status| {
                matches!(status, "completed" | "delivered")
                    || (status == "completed_with_pending_items"
                        && input.review_projection_authoritative
                        && input.pending_review_item_refs.is_empty())
            })
            .unwrap_or(false)
}

fn controls_for_task(
    input: &TaskViewModelTaskInput,
    lifecycle_status: TaskLifecycleStatus,
) -> Vec<TaskControl> {
    let mut controls = Vec::new();
    let control_ids = input
        .allowed_control_ids
        .iter()
        .map(|control| control.as_str())
        .collect::<BTreeSet<_>>();
    let pending_review = !input.pending_review_item_refs.is_empty();
    controls.push(TaskControl::new(
        &input.task_id,
        "open_trace",
        "Open trace",
        TaskControlKind::OpenTrace,
    ));

    for control in control_ids {
        match control {
            "resume" => {
                let resume = TaskControl::new(
                    &input.task_id,
                    "resume",
                    "Resume",
                    TaskControlKind::Resume,
                );
                controls.push(if pending_review {
                    resume.disabled(
                        "Pending review items must be resolved before requesting task resume.",
                    )
                } else {
                    resume
                });
            }
            "retry" => {
                let retry = TaskControl::new(
                    &input.task_id,
                    "retry",
                    "Retry",
                    TaskControlKind::Retry,
                );
                let retry = if let Some(action_id) = &input.retry_action_id {
                    retry.with_target_action_id(action_id)
                } else {
                    retry.disabled("No retryable failed action is available in the backend read model.")
                };
                controls.push(retry);
            }
            "cancel" => controls.push(
                TaskControl::new(
                    &input.task_id,
                    "cancel",
                    "Cancel",
                    TaskControlKind::Cancel,
                )
                .requiring_confirmation(),
            ),
            "refresh_context" => controls.push(TaskControl::new(
                &input.task_id,
                "refresh_context",
                "Refresh context",
                TaskControlKind::RefreshContext,
            )),
            "open_trace" => {}
            _ => controls.push(
                TaskControl::new(
                    &input.task_id,
                    format!("view_evidence:{control}"),
                    control.replace('_', " "),
                    TaskControlKind::ViewEvidence,
                )
                .disabled("This control is exposed as evidence only until a backend action contract exists."),
            ),
        }
    }

    if lifecycle_status.is_terminal() {
        for control in &mut controls {
            if matches!(
                control.kind,
                TaskControlKind::Resume | TaskControlKind::Cancel
            ) {
                control.enabled = false;
                control.disabled_reason =
                    Some("Terminal task state cannot be changed by this request control.".into());
            }
        }
    }

    for control in &controls {
        control
            .validate()
            .expect("task control builder must preserve effect invariants");
    }
    controls
}

fn latest_result_preview_for(
    status: TaskTerminalDeliveryStatus,
    preview: Option<String>,
    final_delivery_present: bool,
    task_id: &str,
) -> Option<TaskLatestResultPreview> {
    let has_preview = preview
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    if !has_preview && status == TaskTerminalDeliveryStatus::NotTerminal {
        return None;
    }
    Some(TaskLatestResultPreview {
        status,
        label: terminal_label(status).into(),
        preview,
        final_delivery_ref: final_delivery_present.then(|| BackendEntityRef {
            id: format!("final_delivery:{task_id}"),
            kind: BackendEntityKind::Evidence,
            label: "Final delivery evidence".into(),
            href: None,
        }),
        evidence_refs: Vec::new(),
    })
}

fn terminal_label(status: TaskTerminalDeliveryStatus) -> &'static str {
    match status {
        TaskTerminalDeliveryStatus::NotTerminal => "not terminal",
        TaskTerminalDeliveryStatus::Delivered => "delivered",
        TaskTerminalDeliveryStatus::MissingFinalDeliveryEvidence => {
            "missing final delivery evidence"
        }
        TaskTerminalDeliveryStatus::CompletedWithPendingReview => "completed with pending review",
        TaskTerminalDeliveryStatus::Blocked => "blocked",
        TaskTerminalDeliveryStatus::Failed => "failed",
        TaskTerminalDeliveryStatus::Cancelled => "cancelled",
        TaskTerminalDeliveryStatus::Unknown => "unknown",
    }
}

fn summarize_tasks(items: &[TaskViewModelItem]) -> TasksViewModelSummary {
    let mut summary = TasksViewModelSummary {
        total: items.len(),
        ..Default::default()
    };
    for item in items {
        if item.needs_attention {
            summary.needs_attention_count += 1;
        }
        *summary
            .by_lifecycle_status
            .entry(item.lifecycle_status.as_str().into())
            .or_insert(0) += 1;
        match item.lifecycle_status {
            TaskLifecycleStatus::Running => summary.active_count += 1,
            TaskLifecycleStatus::WaitingReview => {
                summary.active_count += 1;
                summary.waiting_review_count += 1;
            }
            TaskLifecycleStatus::WaitingPermission => {
                summary.active_count += 1;
                summary.waiting_permission_count += 1;
            }
            TaskLifecycleStatus::Blocked => summary.blocked_count += 1,
            TaskLifecycleStatus::Completed => summary.completed_count += 1,
            TaskLifecycleStatus::CompletedWithPendingReview => {
                summary.completed_needs_evidence_count += 1;
            }
            TaskLifecycleStatus::CompletedNeedsEvidence => {
                summary.completed_needs_evidence_count += 1;
            }
            TaskLifecycleStatus::Failed => summary.failed_count += 1,
            TaskLifecycleStatus::RemoteUnknown => {}
            TaskLifecycleStatus::Cancelled => summary.cancelled_count += 1,
            TaskLifecycleStatus::Unknown => {}
        }
        summary.pending_review_count += item.pending_review_item_refs.len();
    }
    summary
}

fn task_ref(item: &TaskViewModelItem) -> BackendEntityRef {
    BackendEntityRef {
        id: item.canonical_task_id.clone(),
        kind: BackendEntityKind::Task,
        label: item.title.clone(),
        href: item
            .related_run_ids
            .first()
            .map(|run_id| format!("/runs/{run_id}")),
    }
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskViewModelContractError {
    TaskControlEffectMismatch {
        kind: TaskControlKind,
        expected: TaskControlEffect,
        actual: TaskControlEffect,
    },
    ControlClaimsCompletionProof {
        id: String,
    },
}

impl std::fmt::Display for TaskViewModelContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TaskControlEffectMismatch {
                kind,
                expected,
                actual,
            } => write!(
                f,
                "task control {:?} must use effect {:?}, got {:?}",
                kind, expected, actual
            ),
            Self::ControlClaimsCompletionProof { id } => {
                write!(f, "task control {id} must not claim completion proof")
            }
        }
    }
}

impl std::error::Error for TaskViewModelContractError {}
