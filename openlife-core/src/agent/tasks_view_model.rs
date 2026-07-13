use crate::agent::main_chat_agent_v1::{AgentTaskSessionStatus, MainChatAgentStrategy};
use crate::agent::product_read_model::{
    BackendEntityKind, BackendEntityRef, EvidenceRef, EvidenceSensitivity, EvidenceSource,
    ProviderPrivacyBoundarySummary,
};
use crate::agent::types::{AgentRun, AgentRunStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskLifecycleStatus {
    Running,
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
            Self::Running | Self::WaitingPermission | Self::Blocked
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
pub struct TaskViewModelItem {
    pub canonical_task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_session_id: Option<String>,
    #[serde(default)]
    pub related_run_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub title: String,
    pub strategy: String,
    pub lifecycle_status: TaskLifecycleStatus,
    pub terminal_delivery_status: TaskTerminalDeliveryStatus,
    pub final_delivery_evidence_present: bool,
    #[serde(default)]
    pub pending_blockers: Vec<String>,
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
    pub active_count: usize,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTimelineItem {
    pub id: String,
    pub label: String,
    pub status: TaskLifecycleStatus,
    pub evidence_refs: Vec<EvidenceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceViewModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_task_ref: Option<BackendEntityRef>,
    #[serde(default)]
    pub recent_task_refs: Vec<BackendEntityRef>,
    #[serde(default)]
    pub pending_review_item_refs: Vec<BackendEntityRef>,
    #[serde(default)]
    pub timeline: Vec<WorkspaceTimelineItem>,
    pub provider_privacy_boundary_summary: ProviderPrivacyBoundarySummary,
    #[serde(default)]
    pub source_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub contract_limitations: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TaskViewModelTaskInput {
    pub task_session_id: String,
    pub conversation_id: Option<String>,
    pub title: String,
    pub strategy: Option<MainChatAgentStrategy>,
    pub session_status: Option<AgentTaskSessionStatus>,
    pub related_run_ids: Vec<String>,
    pub final_delivery_present: bool,
    pub final_delivery_status: Option<String>,
    pub pending_blockers: Vec<String>,
    pub pending_review_item_refs: Vec<BackendEntityRef>,
    pub allowed_control_ids: Vec<String>,
    pub retry_action_id: Option<String>,
    pub next_recommended_control: Option<String>,
    pub latest_result_preview: Option<String>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct TaskViewModelRunInput {
    pub run: AgentRun,
}

#[derive(Debug, Clone, Default)]
pub struct TasksViewModelBuildInput {
    pub task_inputs: Vec<TaskViewModelTaskInput>,
    pub run_inputs: Vec<TaskViewModelRunInput>,
    pub source_refs: Vec<EvidenceRef>,
    pub contract_limitations: Vec<String>,
}

pub fn build_tasks_view_model(input: TasksViewModelBuildInput) -> TasksViewModel {
    let task_run_ids = input
        .task_inputs
        .iter()
        .flat_map(|task| task.related_run_ids.iter().cloned())
        .collect::<BTreeSet<_>>();

    let mut items = input
        .task_inputs
        .into_iter()
        .map(task_item_from_input)
        .collect::<Vec<_>>();

    for run_input in input.run_inputs {
        if task_run_ids.contains(&run_input.run.id) {
            continue;
        }
        items.push(run_only_item(run_input.run));
    }

    items.sort_by_key(|item| std::cmp::Reverse(item.updated_at));
    let summary = summarize_tasks(&items);

    TasksViewModel {
        items,
        summary,
        source_refs: input.source_refs,
        contract_limitations: input.contract_limitations,
    }
}

pub fn build_workspace_view_model(
    tasks: &TasksViewModel,
    provider_privacy_boundary_summary: ProviderPrivacyBoundarySummary,
    mut contract_limitations: Vec<String>,
) -> WorkspaceViewModel {
    contract_limitations.push(
        "WorkspaceViewModel is a limited R4 baseline; it summarizes backend task/review relations and does not replace Frontend V2."
            .into(),
    );
    let active = tasks
        .items
        .iter()
        .find(|item| item.lifecycle_status.is_active())
        .or_else(|| {
            tasks
                .items
                .iter()
                .find(|item| item.lifecycle_status != TaskLifecycleStatus::Unknown)
        });
    let active_task_ref = active.map(task_ref);
    let recent_task_refs = tasks.items.iter().take(6).map(task_ref).collect::<Vec<_>>();
    let pending_review_item_refs = tasks
        .items
        .iter()
        .flat_map(|item| item.pending_review_item_refs.iter().cloned())
        .map(|item| (item.id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let timeline = tasks
        .items
        .iter()
        .take(8)
        .map(|item| WorkspaceTimelineItem {
            id: item.canonical_task_id.clone(),
            label: item.title.clone(),
            status: item.lifecycle_status,
            evidence_refs: item.evidence_refs.clone(),
            updated_at: item.updated_at,
        })
        .collect();

    WorkspaceViewModel {
        active_task_ref,
        recent_task_refs,
        pending_review_item_refs: pending_review_item_refs.into_values().collect(),
        timeline,
        provider_privacy_boundary_summary,
        source_refs: tasks.source_refs.clone(),
        contract_limitations,
    }
}

fn task_item_from_input(input: TaskViewModelTaskInput) -> TaskViewModelItem {
    let lifecycle_status = lifecycle_status_for_task(&input);
    let terminal_delivery_status = terminal_delivery_status_for_task(&input, lifecycle_status);
    let controls = controls_for_task(&input, lifecycle_status);
    let mut evidence_refs = input.evidence_refs;
    evidence_refs.push(EvidenceRef {
        id: input.task_session_id.clone(),
        label: "Main Chat task session".into(),
        source: EvidenceSource::Task,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    });
    for run_id in &input.related_run_ids {
        evidence_refs.push(EvidenceRef {
            id: run_id.clone(),
            label: "AgentRun".into(),
            source: EvidenceSource::Task,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        });
    }
    evidence_refs.sort_by(|left, right| left.id.cmp(&right.id));
    evidence_refs.dedup_by(|left, right| left.id == right.id && left.source == right.source);

    let latest_result_preview = latest_result_preview_for(
        terminal_delivery_status,
        input.latest_result_preview,
        input.final_delivery_present,
        &input.task_session_id,
    );
    let next_recommended_control = input
        .next_recommended_control
        .filter(|control| !control.trim().is_empty())
        .unwrap_or_else(|| "open_trace".into());

    TaskViewModelItem {
        canonical_task_id: input.task_session_id.clone(),
        task_session_id: Some(input.task_session_id),
        related_run_ids: input.related_run_ids,
        conversation_id: input.conversation_id,
        title: if input.title.trim().is_empty() {
            "Untitled task".into()
        } else {
            input.title
        },
        strategy: input
            .strategy
            .map(|strategy| strategy.as_str().to_string())
            .unwrap_or_else(|| "unknown".into()),
        lifecycle_status,
        terminal_delivery_status,
        final_delivery_evidence_present: input.final_delivery_present,
        pending_blockers: dedup_strings(input.pending_blockers),
        pending_review_item_refs: input.pending_review_item_refs,
        allowed_controls: controls,
        next_recommended_control,
        latest_result_preview,
        evidence_refs,
        updated_at: input.updated_at,
    }
}

fn run_only_item(run: AgentRun) -> TaskViewModelItem {
    let legacy_payload_unverified = run.legacy_payload_unverified;
    let lifecycle_status = if legacy_payload_unverified {
        TaskLifecycleStatus::Unknown
    } else {
        lifecycle_status_for_run(run.status)
    };
    let terminal_delivery_status = match lifecycle_status {
        TaskLifecycleStatus::CompletedNeedsEvidence => {
            TaskTerminalDeliveryStatus::MissingFinalDeliveryEvidence
        }
        TaskLifecycleStatus::Failed => TaskTerminalDeliveryStatus::Failed,
        TaskLifecycleStatus::RemoteUnknown => TaskTerminalDeliveryStatus::Unknown,
        TaskLifecycleStatus::Cancelled => TaskTerminalDeliveryStatus::Cancelled,
        _ => TaskTerminalDeliveryStatus::Unknown,
    };
    let canonical_task_id = if run.task_id.trim().is_empty() {
        format!("run:{}", run.id)
    } else {
        run.task_id.clone()
    };
    let title = if legacy_payload_unverified {
        format!("{} run", run.kind)
    } else {
        run.user_input
            .clone()
            .or_else(|| run.output_preview.clone())
            .unwrap_or_else(|| format!("{} run", run.kind))
    };
    let preview = if legacy_payload_unverified {
        None
    } else {
        run.output_preview.clone().or_else(|| {
            run.error
                .as_ref()
                .map(|error| format!("{}: {}", error.phase, error.message))
        })
    };
    let mut pending_blockers = run.warnings.clone();
    if legacy_payload_unverified {
        pending_blockers.push("legacy_payload_unverified".into());
    }
    let evidence_refs = vec![EvidenceRef {
        id: run.id.clone(),
        label: "AgentRun without task session read model evidence".into(),
        source: EvidenceSource::Task,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    }];
    TaskViewModelItem {
        canonical_task_id: canonical_task_id.clone(),
        task_session_id: None,
        related_run_ids: vec![run.id.clone()],
        conversation_id: run.session_id.clone(),
        title,
        strategy: if legacy_payload_unverified {
            "unknown".into()
        } else {
            run.reasoning_strategy
                .clone()
                .unwrap_or_else(|| run.kind.to_string())
        },
        lifecycle_status,
        terminal_delivery_status,
        final_delivery_evidence_present: false,
        pending_blockers: dedup_strings(pending_blockers),
        pending_review_item_refs: Vec::new(),
        allowed_controls: vec![
            TaskControl::new(
                &canonical_task_id,
                "open_run",
                "Open run",
                TaskControlKind::OpenRun,
            ),
            TaskControl::new(
                &canonical_task_id,
                "view_evidence",
                "View evidence",
                TaskControlKind::ViewEvidence,
            ),
        ],
        next_recommended_control: "open_run".into(),
        latest_result_preview: Some(TaskLatestResultPreview {
            status: terminal_delivery_status,
            label: terminal_label(terminal_delivery_status).into(),
            preview,
            final_delivery_ref: None,
            evidence_refs: evidence_refs.clone(),
        }),
        evidence_refs,
        updated_at: run.finished_at.or(Some(run.started_at)),
    }
}

fn lifecycle_status_for_task(input: &TaskViewModelTaskInput) -> TaskLifecycleStatus {
    let pending_review = !input.pending_review_item_refs.is_empty();
    match input.session_status {
        Some(AgentTaskSessionStatus::Running) => TaskLifecycleStatus::Running,
        Some(AgentTaskSessionStatus::WaitingPermission) => TaskLifecycleStatus::WaitingPermission,
        Some(AgentTaskSessionStatus::Blocked) => TaskLifecycleStatus::Blocked,
        Some(AgentTaskSessionStatus::Failed) => TaskLifecycleStatus::Failed,
        Some(AgentTaskSessionStatus::Cancelled) => TaskLifecycleStatus::Cancelled,
        Some(AgentTaskSessionStatus::Completed) if pending_review => {
            TaskLifecycleStatus::CompletedWithPendingReview
        }
        Some(AgentTaskSessionStatus::Completed) if final_delivery_status_is_complete(input) => {
            TaskLifecycleStatus::Completed
        }
        Some(AgentTaskSessionStatus::Completed)
            if final_delivery_status_is_completed_with_pending_items(input) =>
        {
            TaskLifecycleStatus::CompletedWithPendingReview
        }
        Some(AgentTaskSessionStatus::Completed)
            if input.final_delivery_status.as_deref() == Some("blocked") =>
        {
            TaskLifecycleStatus::Blocked
        }
        Some(AgentTaskSessionStatus::Completed)
            if input.final_delivery_status.as_deref() == Some("failed") =>
        {
            TaskLifecycleStatus::Failed
        }
        Some(AgentTaskSessionStatus::Completed)
            if input.final_delivery_status.as_deref() == Some("cancelled") =>
        {
            TaskLifecycleStatus::Cancelled
        }
        Some(AgentTaskSessionStatus::Completed) => TaskLifecycleStatus::CompletedNeedsEvidence,
        None => TaskLifecycleStatus::Unknown,
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
            .map(|status| matches!(status, "completed" | "delivered"))
            .unwrap_or(false)
}

fn final_delivery_status_is_completed_with_pending_items(input: &TaskViewModelTaskInput) -> bool {
    input.final_delivery_present
        && input.final_delivery_status.as_deref() == Some("completed_with_pending_items")
}

fn lifecycle_status_for_run(status: AgentRunStatus) -> TaskLifecycleStatus {
    match status {
        AgentRunStatus::Running => TaskLifecycleStatus::Running,
        AgentRunStatus::WaitingPermission => TaskLifecycleStatus::WaitingPermission,
        AgentRunStatus::Completed => TaskLifecycleStatus::CompletedNeedsEvidence,
        AgentRunStatus::Failed => TaskLifecycleStatus::Failed,
        AgentRunStatus::RemoteUnknown => TaskLifecycleStatus::RemoteUnknown,
        AgentRunStatus::Cancelled => TaskLifecycleStatus::Cancelled,
    }
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
        &input.task_session_id,
        "open_trace",
        "Open trace",
        TaskControlKind::OpenTrace,
    ));

    for control in control_ids {
        match control {
            "resume" => {
                let resume = TaskControl::new(
                    &input.task_session_id,
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
                    &input.task_session_id,
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
                    &input.task_session_id,
                    "cancel",
                    "Cancel",
                    TaskControlKind::Cancel,
                )
                .requiring_confirmation(),
            ),
            "refresh_context" => controls.push(TaskControl::new(
                &input.task_session_id,
                "refresh_context",
                "Refresh context",
                TaskControlKind::RefreshContext,
            )),
            "open_trace" => {}
            _ => controls.push(
                TaskControl::new(
                    &input.task_session_id,
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
                TaskControlKind::Resume | TaskControlKind::Cancel | TaskControlKind::Retry
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
        *summary
            .by_lifecycle_status
            .entry(item.lifecycle_status.as_str().into())
            .or_insert(0) += 1;
        match item.lifecycle_status {
            TaskLifecycleStatus::Running => summary.active_count += 1,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::main_chat_agent_v1::AgentTaskSessionStatus;
    use crate::agent::product_read_model::ProductRiskLevel;

    #[test]
    fn remote_unknown_agent_run_remains_unknown_in_tasks_projection() {
        let mut run = AgentRun::new_tool_execution_run("a2a.call_agent");
        run.status = AgentRunStatus::RemoteUnknown;
        run.finished_at = Some(Utc::now());
        let model = build_tasks_view_model(TasksViewModelBuildInput {
            run_inputs: vec![TaskViewModelRunInput { run }],
            ..Default::default()
        });

        assert_eq!(model.items.len(), 1);
        assert_eq!(
            model.items[0].lifecycle_status,
            TaskLifecycleStatus::RemoteUnknown
        );
        assert_eq!(
            model.items[0].terminal_delivery_status,
            TaskTerminalDeliveryStatus::Unknown
        );
        assert_eq!(model.summary.failed_count, 0);
        assert_eq!(
            model.summary.by_lifecycle_status.get("remote_unknown"),
            Some(&1)
        );
    }

    #[test]
    fn completed_task_without_final_delivery_fails_closed() {
        let model = build_tasks_view_model(TasksViewModelBuildInput {
            task_inputs: vec![TaskViewModelTaskInput {
                task_session_id: "task-1".into(),
                title: "Needs evidence".into(),
                session_status: Some(AgentTaskSessionStatus::Completed),
                final_delivery_present: false,
                ..Default::default()
            }],
            ..Default::default()
        });

        let item = &model.items[0];
        assert_eq!(
            item.lifecycle_status,
            TaskLifecycleStatus::CompletedNeedsEvidence
        );
        assert_eq!(
            item.terminal_delivery_status,
            TaskTerminalDeliveryStatus::MissingFinalDeliveryEvidence
        );
        assert_eq!(model.summary.completed_count, 0);
        assert_eq!(model.summary.completed_needs_evidence_count, 1);
    }

    #[test]
    fn completed_task_with_final_delivery_missing_status_fails_closed() {
        let model = build_tasks_view_model(TasksViewModelBuildInput {
            task_inputs: vec![TaskViewModelTaskInput {
                task_session_id: "task-missing-status".into(),
                title: "Missing final status".into(),
                session_status: Some(AgentTaskSessionStatus::Completed),
                final_delivery_present: true,
                final_delivery_status: None,
                ..Default::default()
            }],
            ..Default::default()
        });

        let item = &model.items[0];
        assert_eq!(
            item.lifecycle_status,
            TaskLifecycleStatus::CompletedNeedsEvidence
        );
        assert_eq!(
            item.terminal_delivery_status,
            TaskTerminalDeliveryStatus::MissingFinalDeliveryEvidence
        );
        assert_eq!(model.summary.completed_count, 0);
        assert_eq!(model.summary.completed_needs_evidence_count, 1);
    }

    #[test]
    fn completed_task_with_completed_status_is_delivered() {
        let model = build_tasks_view_model(TasksViewModelBuildInput {
            task_inputs: vec![TaskViewModelTaskInput {
                task_session_id: "task-delivered".into(),
                title: "Delivered".into(),
                session_status: Some(AgentTaskSessionStatus::Completed),
                final_delivery_present: true,
                final_delivery_status: Some("completed".into()),
                ..Default::default()
            }],
            ..Default::default()
        });

        let item = &model.items[0];
        assert_eq!(item.lifecycle_status, TaskLifecycleStatus::Completed);
        assert_eq!(
            item.terminal_delivery_status,
            TaskTerminalDeliveryStatus::Delivered
        );
        assert_eq!(model.summary.completed_count, 1);
        assert_eq!(model.summary.completed_needs_evidence_count, 0);
    }

    #[test]
    fn completed_task_with_completed_with_pending_items_is_not_plain_completed() {
        let model = build_tasks_view_model(TasksViewModelBuildInput {
            task_inputs: vec![TaskViewModelTaskInput {
                task_session_id: "task-pending-delivery".into(),
                title: "Pending delivery".into(),
                session_status: Some(AgentTaskSessionStatus::Completed),
                final_delivery_present: true,
                final_delivery_status: Some("completed_with_pending_items".into()),
                ..Default::default()
            }],
            ..Default::default()
        });

        let item = &model.items[0];
        assert_eq!(
            item.lifecycle_status,
            TaskLifecycleStatus::CompletedWithPendingReview
        );
        assert_eq!(
            item.terminal_delivery_status,
            TaskTerminalDeliveryStatus::CompletedWithPendingReview
        );
        assert_eq!(model.summary.completed_count, 0);
        assert_eq!(model.summary.completed_needs_evidence_count, 1);
    }

    #[test]
    fn completed_task_with_pending_review_is_not_plain_completed() {
        let model = build_tasks_view_model(TasksViewModelBuildInput {
            task_inputs: vec![TaskViewModelTaskInput {
                task_session_id: "task-2".into(),
                title: "Pending review".into(),
                session_status: Some(AgentTaskSessionStatus::Completed),
                final_delivery_present: true,
                pending_review_item_refs: vec![BackendEntityRef {
                    id: "review-1".into(),
                    kind: BackendEntityKind::ReviewItem,
                    label: "Review item".into(),
                    href: None,
                }],
                ..Default::default()
            }],
            ..Default::default()
        });

        let item = &model.items[0];
        assert_eq!(
            item.lifecycle_status,
            TaskLifecycleStatus::CompletedWithPendingReview
        );
        assert_eq!(model.summary.completed_count, 0);
        assert_eq!(model.summary.pending_review_count, 1);
    }

    #[test]
    fn request_controls_do_not_claim_completion_after_dispatch() {
        let model = build_tasks_view_model(TasksViewModelBuildInput {
            task_inputs: vec![TaskViewModelTaskInput {
                task_session_id: "task-3".into(),
                title: "Permission".into(),
                session_status: Some(AgentTaskSessionStatus::WaitingPermission),
                allowed_control_ids: vec!["resume".into(), "retry".into(), "cancel".into()],
                retry_action_id: Some("action-1".into()),
                ..Default::default()
            }],
            ..Default::default()
        });

        let controls = &model.items[0].allowed_controls;
        let resume = controls
            .iter()
            .find(|control| control.kind == TaskControlKind::Resume)
            .expect("resume control");
        assert_eq!(resume.effect, TaskControlEffect::TaskResumeRequest);
        assert!(!resume.completion_proof_after_dispatch);
        let retry = controls
            .iter()
            .find(|control| control.kind == TaskControlKind::Retry)
            .expect("retry control");
        assert_eq!(retry.effect, TaskControlEffect::TaskRetryRequest);
        assert_eq!(retry.target_action_id.as_deref(), Some("action-1"));
        let cancel = controls
            .iter()
            .find(|control| control.kind == TaskControlKind::Cancel)
            .expect("cancel control");
        assert_eq!(cancel.effect, TaskControlEffect::TaskCancelRequest);
        assert!(cancel.requires_confirmation);
    }

    #[test]
    fn task_control_effect_invariant_rejects_mismatches() {
        let mut control = TaskControl::new("task-4", "resume", "Resume", TaskControlKind::Resume);
        control.effect = TaskControlEffect::EvidenceOnly;

        let err = control
            .validate()
            .expect_err("resume must be a request control");
        assert_eq!(
            err,
            TaskViewModelContractError::TaskControlEffectMismatch {
                kind: TaskControlKind::Resume,
                expected: TaskControlEffect::TaskResumeRequest,
                actual: TaskControlEffect::EvidenceOnly,
            }
        );
    }

    #[test]
    fn workspace_baseline_uses_backend_task_refs() {
        let tasks = build_tasks_view_model(TasksViewModelBuildInput {
            task_inputs: vec![TaskViewModelTaskInput {
                task_session_id: "task-5".into(),
                title: "Running task".into(),
                session_status: Some(AgentTaskSessionStatus::Running),
                ..Default::default()
            }],
            ..Default::default()
        });
        let workspace =
            build_workspace_view_model(&tasks, ProviderPrivacyBoundarySummary::unknown(), vec![]);

        assert_eq!(
            workspace.active_task_ref.as_ref().map(|r| r.id.as_str()),
            Some("task-5")
        );
        assert_eq!(workspace.timeline[0].status, TaskLifecycleStatus::Running);
        assert_eq!(
            workspace.provider_privacy_boundary_summary.risk,
            ProductRiskLevel::Unknown
        );
    }

    #[test]
    fn unverified_legacy_run_only_item_exposes_unknown_not_persisted_status_or_strategy() {
        let mut run = AgentRun::new_chat_run("legacy-session", "legacy input");
        run.status = AgentRunStatus::Failed;
        run.user_input = None;
        run.output_preview = Some("run_output:bytes=14:hmac-sha256:legacy".into());
        run.error = Some(crate::agent::types::AgentRunError {
            message: "legacy error must not become product truth".into(),
            phase: "provider".into(),
            recoverable: false,
        });
        run.reasoning_strategy = Some("layered".into());
        run.legacy_payload_unverified = true;

        let tasks = build_tasks_view_model(TasksViewModelBuildInput {
            run_inputs: vec![TaskViewModelRunInput { run }],
            ..Default::default()
        });
        let item = &tasks.items[0];

        assert_eq!(item.lifecycle_status, TaskLifecycleStatus::Unknown);
        assert_eq!(
            item.terminal_delivery_status,
            TaskTerminalDeliveryStatus::Unknown
        );
        assert_eq!(item.strategy, "unknown");
        assert_eq!(
            item.latest_result_preview
                .as_ref()
                .map(|preview| preview.status),
            Some(TaskTerminalDeliveryStatus::Unknown)
        );
        assert!(item
            .latest_result_preview
            .as_ref()
            .is_some_and(|preview| preview.preview.is_none()));
        assert!(item
            .pending_blockers
            .iter()
            .any(|warning| warning == "legacy_payload_unverified"));
        assert_eq!(tasks.summary.failed_count, 0);
        assert_eq!(tasks.summary.completed_count, 0);

        let workspace =
            build_workspace_view_model(&tasks, ProviderPrivacyBoundarySummary::unknown(), vec![]);
        assert!(workspace.active_task_ref.is_none());
        assert_eq!(workspace.timeline[0].status, TaskLifecycleStatus::Unknown);
    }
}
