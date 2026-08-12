use crate::main_chat_task_controls::{
    get_main_chat_agent_task_detail_with_state, list_main_chat_agent_tasks_with_state,
    MainChatAgentTaskFilter, TaskDetail,
};
use crate::state::AppState;
use openlife_core::agent::{
    build_tasks_view_model, build_workspace_view_model, AgentRun, BackendEntityKind,
    BackendEntityRef, EvidenceRef, EvidenceSensitivity, EvidenceSource,
    ProviderPrivacyBoundarySummary, ReviewItem, ReviewItemDecisionStatus, TaskArtifactViewModel,
    TaskItemViewModel, TaskLifecycleStatus, TaskTerminalDeliveryStatus, TaskViewModelRunInput,
    TaskViewModelTaskInput, TasksViewModel, TasksViewModelBuildInput, ViewModelEnvelope,
    ViewModelStatus, ViewModelWarning, ViewModelWarningSeverity, WorkspaceActivityItem,
    WorkspaceViewModel, WorkspaceViewModelBuildInput,
};
use openlife_core::task_runtime::{
    CanonicalArtifactSnapshot, CanonicalArtifactStatus, CanonicalTaskSnapshot, CanonicalTaskStatus,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tauri::State;

use super::provider_privacy::get_provider_privacy_boundary_summary_with_state;
use super::review_center::get_review_center_view_model_with_state;

#[tauri::command]
pub async fn get_tasks_view_model(
    state: State<'_, Arc<AppState>>,
) -> Result<ViewModelEnvelope<TasksViewModel>, String> {
    get_tasks_view_model_with_state(state.inner()).await
}

#[tauri::command]
pub async fn get_workspace_view_model(
    state: State<'_, Arc<AppState>>,
) -> Result<ViewModelEnvelope<WorkspaceViewModel>, String> {
    get_workspace_view_model_with_state(state.inner()).await
}

pub(crate) async fn get_tasks_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<TasksViewModel>, String> {
    Ok(load_tasks_read_model_snapshot(state).await?.envelope)
}

struct TasksReadModelSnapshot {
    envelope: ViewModelEnvelope<TasksViewModel>,
    review_items: Vec<ReviewItem>,
    activity_by_task: BTreeMap<String, Vec<WorkspaceActivityItem>>,
}

async fn load_tasks_read_model_snapshot(
    state: &Arc<AppState>,
) -> Result<TasksReadModelSnapshot, String> {
    let mut warnings = Vec::new();
    let (review_items, review_projection_authoritative) =
        load_review_items(state, &mut warnings).await;
    let mut loaded_tasks = load_task_inputs(
        state,
        &review_items,
        review_projection_authoritative,
        &mut warnings,
    )
    .await;
    overlay_canonical_report_tasks(state, &mut loaded_tasks, &mut warnings).await;
    let run_inputs = load_run_inputs(state, &mut warnings).await;
    let model = build_tasks_view_model(TasksViewModelBuildInput {
        task_inputs: loaded_tasks.task_inputs,
        run_inputs,
        source_refs: vec![
            source_ref("main_chat_task_controls", "Main Chat task controls"),
            source_ref("agent_run_store", "AgentRun store"),
            source_ref("review_center_view_model", "ReviewCenterViewModel"),
            source_ref(
                "canonical_task_runtime_store",
                "Canonical report Task Runtime store",
            ),
        ],
        contract_limitations: vec![
            "Resume, retry, cancel, and refresh controls are request eligibility only; completion requires a refreshed backend read model.".into(),
            "Run-only rows without task-session final delivery evidence are not treated as completed task proof.".into(),
            "Migrated report lifecycle and delivery proof come from canonical Task and ArtifactVersion state; compatibility TaskSession completion cannot override them.".into(),
        ],
    });
    let canonical_runtime_degraded = warnings.iter().any(|warning| {
        matches!(
            warning.code.as_str(),
            "canonical_task_runtime_store_unavailable" | "canonical_task_runtime_read_failed"
        )
    });
    let status = if model.items.is_empty() {
        if warnings.is_empty() {
            ViewModelStatus::Empty
        } else {
            ViewModelStatus::Error
        }
    } else if canonical_runtime_degraded {
        ViewModelStatus::Stale
    } else {
        ViewModelStatus::Ready
    };
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.warnings = warnings;
    Ok(TasksReadModelSnapshot {
        envelope,
        review_items,
        activity_by_task: loaded_tasks.activity_by_task,
    })
}

pub(crate) async fn get_workspace_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<WorkspaceViewModel>, String> {
    let mut snapshot = load_tasks_read_model_snapshot(state).await?;
    let tasks_status = snapshot.envelope.status;
    let tasks = snapshot
        .envelope
        .data
        .take()
        .ok_or_else(|| "TasksViewModel data unavailable for WorkspaceViewModel".to_string())?;
    let active_task_id = tasks.items.iter().find_map(|item| {
        matches!(
            item.lifecycle_status,
            TaskLifecycleStatus::Running
                | TaskLifecycleStatus::WaitingReview
                | TaskLifecycleStatus::WaitingPermission
                | TaskLifecycleStatus::Blocked
        )
        .then_some(item.canonical_task_id.clone())
    });
    let active_task_activity = active_task_id
        .as_ref()
        .and_then(|task_id| snapshot.activity_by_task.remove(task_id))
        .unwrap_or_default();
    let provider_envelope = get_provider_privacy_boundary_summary_with_state(state).await?;
    let provider_status = provider_envelope.status;
    let provider_summary = provider_envelope
        .data
        .clone()
        .unwrap_or_else(ProviderPrivacyBoundarySummary::unknown);
    let model = build_workspace_view_model(WorkspaceViewModelBuildInput {
        tasks,
        review_items: snapshot.review_items,
        active_task_activity,
        provider_privacy_boundary_summary: provider_summary,
        source_refs: vec![source_ref(
            "main_chat_task_evidence_view",
            "Metadata-safe Main Chat task activity",
        )],
        contract_limitations: vec![
            "Task controls and review actions are requests only; completion requires a refreshed backend read model.".into(),
            "Workspace activity is metadata-only. Resource, Web, and artifact bodies remain behind their typed evidence owners.".into(),
            "activeTask is the global active task and can belong to a conversation other than the one currently selected in the Workspace.".into(),
        ],
    });
    let status = workspace_composition_status(
        tasks_status,
        provider_status,
        !model.recent_task_refs.is_empty(),
    );
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.warnings = snapshot.envelope.warnings;
    envelope.warnings.extend(provider_envelope.warnings);
    Ok(envelope)
}

fn workspace_composition_status(
    tasks_status: ViewModelStatus,
    provider_status: ViewModelStatus,
    has_task_data: bool,
) -> ViewModelStatus {
    if matches!(tasks_status, ViewModelStatus::Error)
        || matches!(provider_status, ViewModelStatus::Error)
    {
        return ViewModelStatus::Error;
    }
    if matches!(tasks_status, ViewModelStatus::Loading)
        || matches!(provider_status, ViewModelStatus::Loading)
    {
        return ViewModelStatus::Loading;
    }
    if matches!(tasks_status, ViewModelStatus::Stale)
        || matches!(provider_status, ViewModelStatus::Stale)
        || (has_task_data && matches!(provider_status, ViewModelStatus::Empty))
    {
        return ViewModelStatus::Stale;
    }
    if !has_task_data || matches!(tasks_status, ViewModelStatus::Empty) {
        return ViewModelStatus::Empty;
    }
    ViewModelStatus::Ready
}

#[derive(Default)]
struct LoadedTaskInputs {
    task_inputs: Vec<TaskViewModelTaskInput>,
    activity_by_task: BTreeMap<String, Vec<WorkspaceActivityItem>>,
}

async fn overlay_canonical_report_tasks(
    state: &Arc<AppState>,
    loaded: &mut LoadedTaskInputs,
    warnings: &mut Vec<ViewModelWarning>,
) {
    let Some(store) = state.canonical_task_runtime_store.as_ref() else {
        warnings.push(warning(
            "canonical_task_runtime_store_unavailable",
            "TasksViewModel cannot prove migrated report Task, Item, or Artifact truth because CanonicalTaskRuntimeStore is unavailable.",
        ));
        return;
    };
    let snapshots = match store.lock().await.list_task_snapshots(100) {
        Ok(snapshots) => snapshots,
        Err(error) => {
            warnings.push(warning(
                "canonical_task_runtime_read_failed",
                format!("TasksViewModel could not load canonical report Task snapshots: {error}"),
            ));
            return;
        }
    };
    for snapshot in snapshots {
        overlay_canonical_report_task(loaded, snapshot);
    }
}

fn overlay_canonical_report_task(loaded: &mut LoadedTaskInputs, snapshot: CanonicalTaskSnapshot) {
    let run_ids = snapshot
        .runs
        .iter()
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
    let execution_session_ids = snapshot
        .runs
        .iter()
        .map(|run| run.execution_session_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let existing_index = loaded.task_inputs.iter().position(|input| {
        execution_session_ids.contains(input.task_session_id.as_str())
            || input
                .related_run_ids
                .iter()
                .any(|run_id| run_ids.contains(run_id))
    });
    let task_session_id = existing_index
        .and_then(|index| loaded.task_inputs.get(index))
        .map(|input| input.task_session_id.clone())
        .or_else(|| {
            snapshot
                .runs
                .last()
                .map(|run| run.execution_session_id.clone())
        })
        .unwrap_or_else(|| snapshot.task.id.clone());
    let (lifecycle_status, terminal_status, delivery_proven) =
        canonical_report_delivery_status(&snapshot);
    let canonical_items = snapshot
        .items
        .iter()
        .map(|item| TaskItemViewModel {
            id: item.id.clone(),
            run_id: item.run_id.clone(),
            sequence: item.sequence,
            kind: item.kind,
            status: item.status,
            summary_code: item.summary_code.clone(),
            evidence_refs: vec![EvidenceRef {
                id: item.id.clone(),
                label: "Canonical Task Item".into(),
                source: EvidenceSource::Task,
                sensitivity: Some(EvidenceSensitivity::LocalPrivate),
            }],
        })
        .collect::<Vec<_>>();
    let canonical_artifacts = snapshot
        .artifacts
        .iter()
        .map(canonical_artifact_view)
        .collect::<Vec<_>>();
    let pending_review_item_refs = snapshot
        .artifacts
        .iter()
        .filter(|artifact| artifact.artifact.status == CanonicalArtifactStatus::WaitingReview)
        .filter_map(|artifact| artifact.artifact.proposal_id.as_ref())
        .map(|proposal_id| BackendEntityRef {
            id: proposal_id.clone(),
            kind: BackendEntityKind::ReviewItem,
            label: "Report Artifact review".into(),
            href: None,
        })
        .collect::<Vec<_>>();
    let blockers = snapshot
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.status,
                openlife_core::task_runtime::CanonicalTaskItemStatus::Blocked
                    | openlife_core::task_runtime::CanonicalTaskItemStatus::Failed
                    | openlife_core::task_runtime::CanonicalTaskItemStatus::EffectUnknown
            )
        })
        .map(|item| item.summary_code.clone())
        .collect::<Vec<_>>();
    let preview = Some(canonical_report_preview(&snapshot));
    let canonical_evidence = vec![
        source_ref(snapshot.task.id.clone(), "Canonical report Task snapshot"),
        source_ref(
            format!("{}:artifacts", snapshot.task.id),
            "Canonical report ArtifactVersion snapshot",
        ),
    ];

    if let Some(index) = existing_index {
        let input = &mut loaded.task_inputs[index];
        let old_activity_key = input.task_session_id.clone();
        input.canonical_task_id = Some(snapshot.task.id.clone());
        input.conversation_id = Some(snapshot.task.conversation_id.clone());
        input.related_run_ids = run_ids;
        input.canonical_lifecycle_status = Some(lifecycle_status);
        input.canonical_terminal_delivery_status = Some(terminal_status);
        input.canonical_final_delivery_evidence_present = Some(delivery_proven);
        input.canonical_items = canonical_items;
        input.canonical_artifacts = canonical_artifacts;
        input
            .pending_review_item_refs
            .extend(pending_review_item_refs);
        input.pending_blockers.extend(blockers);
        input.latest_result_preview = preview;
        input.evidence_refs.extend(canonical_evidence);
        input.updated_at = Some(snapshot.task.updated_at);
        if let Some(activity) = loaded.activity_by_task.remove(&old_activity_key) {
            loaded
                .activity_by_task
                .insert(snapshot.task.id.clone(), activity);
        }
    } else {
        loaded.task_inputs.push(TaskViewModelTaskInput {
            task_session_id,
            canonical_task_id: Some(snapshot.task.id.clone()),
            conversation_id: Some(snapshot.task.conversation_id),
            title: "Generated report".into(),
            strategy: None,
            session_status: None,
            related_run_ids: run_ids,
            final_delivery_present: false,
            final_delivery_status: None,
            canonical_lifecycle_status: Some(lifecycle_status),
            canonical_terminal_delivery_status: Some(terminal_status),
            canonical_final_delivery_evidence_present: Some(delivery_proven),
            canonical_items,
            canonical_artifacts,
            pending_blockers: blockers,
            pending_review_item_refs,
            review_projection_authoritative: true,
            allowed_control_ids: Vec::new(),
            retry_action_id: None,
            next_recommended_control: Some("open_trace".into()),
            latest_result_preview: preview,
            evidence_refs: canonical_evidence,
            updated_at: Some(snapshot.task.updated_at),
        });
    }
}

fn canonical_report_delivery_status(
    snapshot: &CanonicalTaskSnapshot,
) -> (TaskLifecycleStatus, TaskTerminalDeliveryStatus, bool) {
    let delivery_proven = !snapshot.artifacts.is_empty()
        && snapshot.artifacts.iter().all(|artifact| {
            artifact.artifact.status == CanonicalArtifactStatus::Materialized
                && artifact.artifact.content_digest
                    == artifact
                        .current_version
                        .observed_content_digest
                        .as_deref()
                        .unwrap_or("")
                && artifact.artifact.materialized_reference.is_some()
                && artifact.artifact.materialized_reference
                    == artifact.current_version.materialized_reference
        });
    match snapshot.task.status {
        CanonicalTaskStatus::Running => (
            TaskLifecycleStatus::Running,
            TaskTerminalDeliveryStatus::NotTerminal,
            false,
        ),
        CanonicalTaskStatus::WaitingReview => (
            TaskLifecycleStatus::WaitingReview,
            TaskTerminalDeliveryStatus::NotTerminal,
            false,
        ),
        CanonicalTaskStatus::Completed if delivery_proven => (
            TaskLifecycleStatus::Completed,
            TaskTerminalDeliveryStatus::Delivered,
            true,
        ),
        CanonicalTaskStatus::Completed => (
            TaskLifecycleStatus::CompletedNeedsEvidence,
            TaskTerminalDeliveryStatus::MissingFinalDeliveryEvidence,
            false,
        ),
        CanonicalTaskStatus::Blocked => (
            TaskLifecycleStatus::Blocked,
            TaskTerminalDeliveryStatus::Blocked,
            false,
        ),
        CanonicalTaskStatus::Failed => (
            TaskLifecycleStatus::Failed,
            TaskTerminalDeliveryStatus::Failed,
            false,
        ),
        CanonicalTaskStatus::EffectUnknown => (
            TaskLifecycleStatus::RemoteUnknown,
            TaskTerminalDeliveryStatus::Unknown,
            false,
        ),
    }
}

fn canonical_artifact_view(snapshot: &CanonicalArtifactSnapshot) -> TaskArtifactViewModel {
    let proposal_ref = snapshot
        .artifact
        .proposal_id
        .as_ref()
        .map(|proposal_id| BackendEntityRef {
            id: proposal_id.clone(),
            kind: BackendEntityKind::ReviewItem,
            label: "Artifact Review checkpoint".into(),
            href: None,
        });
    TaskArtifactViewModel {
        artifact_id: snapshot.artifact.id.clone(),
        version: snapshot.current_version.version,
        status: snapshot.artifact.status,
        media_type: snapshot.artifact.media_type.clone(),
        content_digest: snapshot.artifact.content_digest.clone(),
        target_reference_digest: snapshot.artifact.target_reference_digest.clone(),
        materialized_reference: snapshot.current_version.materialized_reference.clone(),
        observed_content_digest: snapshot.current_version.observed_content_digest.clone(),
        proposal_ref,
        source_item_ref: BackendEntityRef {
            id: snapshot.artifact.source_item_id.clone(),
            kind: BackendEntityKind::Evidence,
            label: "ArtifactDraft Item".into(),
            href: None,
        },
        evidence_refs: vec![EvidenceRef {
            id: snapshot.artifact.id.clone(),
            label: "Canonical ArtifactVersion".into(),
            source: EvidenceSource::Task,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        }],
    }
}

fn canonical_report_preview(snapshot: &CanonicalTaskSnapshot) -> String {
    let total = snapshot.artifacts.len();
    let materialized = snapshot
        .artifacts
        .iter()
        .filter(|artifact| artifact.artifact.status == CanonicalArtifactStatus::Materialized)
        .count();
    match snapshot.task.status {
        CanonicalTaskStatus::WaitingReview => {
            format!("{total} report artifact(s) are waiting for Review.")
        }
        CanonicalTaskStatus::Completed => {
            format!("{materialized} of {total} report artifact(s) are materialized and verified.")
        }
        CanonicalTaskStatus::Blocked => "The report is blocked by a Review decision.".into(),
        CanonicalTaskStatus::EffectUnknown => {
            "The report materialization result is unknown and was not replayed.".into()
        }
        CanonicalTaskStatus::Failed => "The report task failed before verified delivery.".into(),
        CanonicalTaskStatus::Running => {
            format!("The report task has prepared {total} artifact draft(s).")
        }
    }
}

async fn load_task_inputs(
    state: &Arc<AppState>,
    review_items: &[ReviewItem],
    review_projection_authoritative: bool,
    warnings: &mut Vec<ViewModelWarning>,
) -> LoadedTaskInputs {
    let summaries = match list_main_chat_agent_tasks_with_state(
        Some(MainChatAgentTaskFilter {
            statuses: Vec::new(),
            conversation_id: None,
            include_terminal: true,
            include_stale: true,
        }),
        Some(100),
        Some(0),
        state,
    )
    .await
    {
        Ok(summaries) => summaries,
        Err(err) => {
            warnings.push(warning(
                "main_chat_task_summaries_unavailable",
                format!("TasksViewModel could not load Main Chat task summaries: {err}"),
            ));
            return LoadedTaskInputs::default();
        }
    };

    let mut inputs = Vec::new();
    let mut activity_by_task = BTreeMap::new();
    for summary in summaries {
        let detail =
            match get_main_chat_agent_task_detail_with_state(&summary.task_session_id, state).await
            {
                Ok(detail) => detail,
                Err(err) => {
                    warnings.push(warning(
                        "main_chat_task_detail_unavailable",
                        format!(
                            "TasksViewModel could not load task detail for {}: {err}",
                            summary.task_session_id
                        ),
                    ));
                    continue;
                }
            };
        let related_run_ids = related_run_ids_for(&summary.run_id, &detail);
        let pending_review_item_refs = review_refs_for_task(review_items, &summary.task_session_id);
        let final_delivery_status = detail
            .final_delivery
            .as_ref()
            .and_then(|delivery| delivery.get("status"))
            .and_then(|status| status.as_str())
            .map(str::to_string);
        let retry_action_id = detail.retry_target_action_id.clone();
        let mut pending_blockers = detail.blockers.clone();
        pending_blockers.extend(detail.task_session.pending_blockers.clone());
        pending_blockers.extend(detail.continuity_diagnostics.reason_codes.clone());
        activity_by_task.insert(
            summary.task_session_id.clone(),
            workspace_activity_for_task(&summary.task_session_id, &detail),
        );
        inputs.push(TaskViewModelTaskInput {
            task_session_id: summary.task_session_id,
            canonical_task_id: None,
            conversation_id: Some(summary.conversation_id),
            title: summary.title,
            strategy: Some(detail.task_session.selected_strategy),
            session_status: Some(detail.task_session.status),
            related_run_ids,
            final_delivery_present: detail.final_delivery.is_some(),
            final_delivery_status,
            canonical_lifecycle_status: None,
            canonical_terminal_delivery_status: None,
            canonical_final_delivery_evidence_present: None,
            canonical_items: Vec::new(),
            canonical_artifacts: Vec::new(),
            pending_blockers,
            pending_review_item_refs,
            review_projection_authoritative,
            allowed_control_ids: detail.allowed_controls,
            retry_action_id,
            next_recommended_control: Some(detail.next_recommended_control),
            latest_result_preview: Some(summary.last_observation_preview),
            evidence_refs: vec![source_ref("main_chat_task_detail", "Main Chat task detail")],
            updated_at: Some(detail.task_session.updated_at),
        });
    }
    LoadedTaskInputs {
        task_inputs: inputs,
        activity_by_task,
    }
}

fn workspace_activity_for_task(
    task_session_id: &str,
    detail: &TaskDetail,
) -> Vec<WorkspaceActivityItem> {
    detail
        .evidence_view
        .event_timeline
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let event_id = if event.id.trim().is_empty() || event.id == "unknown" {
                format!("{task_session_id}:activity:{index}")
            } else {
                event.id.clone()
            };
            let evidence_id = event
                .source_ref
                .clone()
                .filter(|source| !source.trim().is_empty() && source != "unknown")
                .unwrap_or_else(|| event_id.clone());
            WorkspaceActivityItem::from_product_event(
                event_id,
                &event.kind,
                event.summary.clone(),
                event.normalized_lifecycle_state.as_deref(),
                event.failure_kind.as_deref(),
                vec![EvidenceRef {
                    id: evidence_id,
                    label: "Task activity evidence".into(),
                    source: EvidenceSource::Task,
                    sensitivity: Some(EvidenceSensitivity::LocalPrivate),
                }],
                event.created_at,
            )
        })
        .collect()
}

async fn load_run_inputs(
    state: &Arc<AppState>,
    warnings: &mut Vec<ViewModelWarning>,
) -> Vec<TaskViewModelRunInput> {
    let Some(store) = state.agent_run_store.as_ref() else {
        warnings.push(warning(
            "agent_run_store_unavailable",
            "TasksViewModel could not load run-only rows because AgentRun store is unavailable.",
        ));
        return Vec::new();
    };
    let store = store.lock().await;
    let runs = crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store.list_runs(100, 0).map_err(|error| error.to_string()),
    );
    drop(store);
    match runs {
        Ok(runs) => runs
            .into_iter()
            .map(|run| TaskViewModelRunInput { run })
            .collect(),
        Err(err) => {
            warnings.push(warning(
                "agent_run_store_read_failed",
                format!("TasksViewModel could not load AgentRuns: {err}"),
            ));
            Vec::new()
        }
    }
}

async fn load_review_items(
    state: &Arc<AppState>,
    warnings: &mut Vec<ViewModelWarning>,
) -> (Vec<ReviewItem>, bool) {
    match get_review_center_view_model_with_state(state).await {
        Ok(envelope)
            if matches!(
                envelope.status,
                ViewModelStatus::Ready | ViewModelStatus::Empty
            ) =>
        {
            match envelope.data {
                Some(model) => (model.items, true),
                None => {
                    warnings.push(warning(
                        "review_center_view_model_data_missing",
                        "TasksViewModel could not prove review-item absence because ReviewCenterViewModel returned no data.",
                    ));
                    (Vec::new(), false)
                }
            }
        }
        Ok(envelope) => {
            warnings.push(warning(
                "review_center_view_model_not_authoritative",
                format!(
                    "TasksViewModel could not prove review-item absence because ReviewCenterViewModel status is {:?}.",
                    envelope.status
                ),
            ));
            (Vec::new(), false)
        }
        Err(err) => {
            warnings.push(warning(
                "review_center_view_model_unavailable",
                format!("TasksViewModel could not load ReviewCenterViewModel: {err}"),
            ));
            (Vec::new(), false)
        }
    }
}

fn related_run_ids_for(summary_run_id: &str, detail: &TaskDetail) -> Vec<String> {
    let mut ids = Vec::new();
    if summary_run_id != "unknown" && !summary_run_id.trim().is_empty() {
        ids.push(summary_run_id.to_string());
    }
    if let Some(run_id) = detail.evidence_view.run_id.as_ref() {
        if !run_id.trim().is_empty() && run_id != "unknown" {
            ids.push(run_id.clone());
        }
    }
    ids.sort();
    ids.dedup();
    ids
}

fn review_refs_for_task(
    review_items: &[ReviewItem],
    task_session_id: &str,
) -> Vec<BackendEntityRef> {
    let mut refs = Vec::new();
    for item in review_items {
        if item.status != ReviewItemDecisionStatus::Pending {
            continue;
        }
        if item
            .task_resume_relation
            .as_ref()
            .is_some_and(|relation| relation.task_session_id == task_session_id)
        {
            refs.push(BackendEntityRef {
                id: item.id.clone(),
                kind: BackendEntityKind::ReviewItem,
                label: format!("{:?}", item.item_type),
                href: None,
            });
        }
    }
    refs.sort_by(|left, right| left.id.cmp(&right.id));
    refs.dedup_by(|left, right| left.id == right.id);
    refs
}

#[allow(dead_code)]
fn _agent_run_ref(run: &AgentRun) -> EvidenceRef {
    EvidenceRef {
        id: run.id.clone(),
        label: "AgentRun".into(),
        source: EvidenceSource::Task,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
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
    use super::{
        get_tasks_view_model_with_state, load_run_inputs, review_refs_for_task,
        workspace_composition_status,
    };
    use openlife_core::agent::{
        build_review_center_view_model, AgentProposal, ProposalSource, ProposalType,
        ReviewCenterBuildInput, RiskLevel, ViewModelStatus,
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn install_release_like_persistence_coordinator(state: &mut Arc<crate::AppState>) {
        let coordinator = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        coordinator.seal();
        Arc::get_mut(state)
            .expect("test state has one outer owner")
            .persistence_coordinator = coordinator;
    }

    fn pending_chat_proposal() -> AgentProposal {
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.preference",
            json!({ "value": "concise" }),
            "Remember an explicit preference.",
            0.9,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        );
        proposal.run_id = Some("forged-run".into());
        proposal.source_detail = Some("forged-task".into());
        proposal
    }

    #[test]
    fn task_review_refs_ignore_descriptive_source_and_run_fields() {
        let proposal = pending_chat_proposal();
        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            ..Default::default()
        });

        assert!(
            review_refs_for_task(&model.items, "forged-task").is_empty(),
            "TasksViewModel cannot infer review ownership from source_detail or run_id"
        );
    }

    #[test]
    fn task_review_refs_accept_only_canonical_terminal_origin_projection() {
        let proposal = pending_chat_proposal();
        let proposal_id = proposal.id.clone();
        let model = build_review_center_view_model(ReviewCenterBuildInput {
            proposals: vec![proposal],
            terminal_owner_task_session_ids: BTreeMap::from([(
                proposal_id.clone(),
                "canonical-task".into(),
            )]),
            ..Default::default()
        });

        let refs = review_refs_for_task(&model.items, "canonical-task");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id, proposal_id);
        assert!(review_refs_for_task(&model.items, "forged-task").is_empty());
    }

    #[test]
    fn workspace_composition_preserves_upstream_failure_states() {
        assert_eq!(
            workspace_composition_status(ViewModelStatus::Error, ViewModelStatus::Ready, false,),
            ViewModelStatus::Error
        );
        assert_eq!(
            workspace_composition_status(ViewModelStatus::Ready, ViewModelStatus::Stale, true,),
            ViewModelStatus::Stale
        );
        assert_eq!(
            workspace_composition_status(ViewModelStatus::Ready, ViewModelStatus::Loading, true,),
            ViewModelStatus::Loading
        );
    }

    #[test]
    fn workspace_composition_fails_closed_when_provider_summary_is_absent() {
        assert_eq!(
            workspace_composition_status(ViewModelStatus::Ready, ViewModelStatus::Empty, true,),
            ViewModelStatus::Stale
        );
        assert_eq!(
            workspace_composition_status(ViewModelStatus::Ready, ViewModelStatus::Ready, true,),
            ViewModelStatus::Ready
        );
    }

    #[tokio::test]
    async fn tasks_run_read_failure_is_unknown_and_degrades_before_future_effects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks-agent-run-read-failure.db");
        let store = openlife_core::agent::AgentRunStore::new(&path).unwrap();
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(store)));
        install_release_like_persistence_coordinator(&mut state);
        let fault = rusqlite::Connection::open(&path).unwrap();
        fault.execute_batch("DROP TABLE agent_runs;").unwrap();
        drop(fault);

        let mut warnings = Vec::new();
        let inputs = load_run_inputs(&state, &mut warnings).await;
        assert!(inputs.is_empty());
        assert!(warnings
            .iter()
            .any(|warning| warning.code == "agent_run_store_read_failed"));
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        );
        assert!(state
            .persistence_coordinator
            .admit_agent_run_write()
            .is_err());
    }

    #[tokio::test]
    async fn tasks_view_model_fails_closed_when_canonical_report_store_is_unavailable() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .canonical_task_runtime_store = None;

        let envelope = get_tasks_view_model_with_state(&state).await.unwrap();

        assert_eq!(envelope.status, ViewModelStatus::Error);
        assert!(envelope
            .warnings
            .iter()
            .any(|warning| { warning.code == "canonical_task_runtime_store_unavailable" }));
    }

    #[tokio::test]
    async fn tasks_view_model_projects_canonical_report_items_and_artifact_delivery() {
        use sha2::{Digest, Sha256};

        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let content = "# Canonical report";
        let content_digest = format!("sha256:{:x}", Sha256::digest(content.as_bytes()));
        let prepared = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .prepare_report_artifact(openlife_core::task_runtime::ReportArtifactDraftInput {
                conversation_id: "conversation-report-view",
                execution_session_id: "execution-report-view",
                run_id: "run-report-view",
                outcome_digest: &format!("sha256:{:x}", Sha256::digest(b"report view outcome")),
                target_reference: "/tmp/openlife/report-view.md",
                content_digest: &content_digest,
                media_type: "text/markdown; charset=utf-8",
            })
            .unwrap();
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .bind_report_review(&prepared.artifact_id, "proposal-report-view")
            .unwrap();

        let waiting = get_tasks_view_model_with_state(&state).await.unwrap();
        assert_eq!(waiting.status, ViewModelStatus::Ready);
        let waiting = waiting.data.unwrap();
        assert_eq!(waiting.items.len(), 1);
        let task = &waiting.items[0];
        assert_eq!(task.canonical_task_id, prepared.task_id);
        assert_eq!(
            task.task_session_id.as_deref(),
            Some("execution-report-view")
        );
        assert_eq!(
            task.lifecycle_status,
            openlife_core::agent::TaskLifecycleStatus::WaitingReview
        );
        assert_eq!(task.items.len(), 2);
        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(
            task.artifacts[0].status,
            openlife_core::task_runtime::CanonicalArtifactStatus::WaitingReview
        );
        assert!(!task.final_delivery_evidence_present);
        assert!(task
            .latest_result_preview
            .as_ref()
            .is_some_and(|preview| preview.final_delivery_ref.is_none()));

        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .confirm_artifact_materialized(
                "proposal-report-view",
                "/tmp/openlife/report-view.md",
                &content_digest,
            )
            .unwrap();
        let completed = get_tasks_view_model_with_state(&state).await.unwrap();
        let completed = completed.data.unwrap();
        assert_eq!(completed.items.len(), 1);
        let task = &completed.items[0];
        assert_eq!(
            task.lifecycle_status,
            openlife_core::agent::TaskLifecycleStatus::Completed
        );
        assert_eq!(
            task.terminal_delivery_status,
            openlife_core::agent::TaskTerminalDeliveryStatus::Delivered
        );
        assert!(task.final_delivery_evidence_present);
        assert!(task
            .latest_result_preview
            .as_ref()
            .is_some_and(|preview| preview.final_delivery_ref.is_some()));
        assert_eq!(task.items.len(), 3);
        assert_eq!(
            task.artifacts[0].materialized_reference.as_deref(),
            Some("/tmp/openlife/report-view.md")
        );
    }
}
