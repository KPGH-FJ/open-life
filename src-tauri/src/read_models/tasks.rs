use crate::{artifact_materializer::managed_artifact_root, state::AppState};
use openlife_core::agent::{
    build_tasks_view_model, build_workspace_view_model, BackendEntityKind, BackendEntityRef,
    EvidenceRef, EvidenceSensitivity, EvidenceSource, ProviderPrivacyBoundarySummary,
    ReviewCenterViewModel, TaskArtifactChangeKind, TaskArtifactChangeViewModel,
    TaskArtifactPreviewStatus, TaskArtifactPreviewViewModel, TaskArtifactUndoViewModel,
    TaskArtifactVerificationStatus, TaskArtifactVerificationViewModel, TaskArtifactViewModel,
    TaskCompletionDisposition, TaskItemViewModel, TaskLifecycleStatus, TaskTerminalDeliveryStatus,
    TaskViewModelTaskInput, TaskWorkPlanStepViewModel, TaskWorkPlanViewModel, TasksViewModel,
    TasksViewModelBuildInput, ViewModelEnvelope, ViewModelStatus, ViewModelWarning,
    ViewModelWarningSeverity, WorkspaceActivityItem, WorkspaceViewModel,
    WorkspaceViewModelBuildInput,
};
use openlife_core::conversation::ConversationItemKind;
use openlife_core::task_runtime::{
    CanonicalArtifactSnapshot, CanonicalArtifactStatus, CanonicalTaskSnapshot, CanonicalTaskStatus,
    CanonicalWorkPlanRecord,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

use super::provider_privacy::get_provider_privacy_boundary_summary_with_state;
use super::review_center::get_review_center_view_model_with_state;

const TASK_ARTIFACT_PREVIEW_MAX_CHARS: usize = 12_000;
const TASK_ARTIFACT_READ_MAX_BYTES: u64 = 100 * 1024;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchViewModel {
    pub captured_at: String,
    pub workspace: ViewModelEnvelope<WorkspaceViewModel>,
    pub tasks: ViewModelEnvelope<TasksViewModel>,
    pub review: ViewModelEnvelope<ReviewCenterViewModel>,
    pub provider_boundary: ViewModelEnvelope<ProviderPrivacyBoundarySummary>,
}

#[tauri::command]
pub async fn get_workbench_view_model(
    state: State<'_, Arc<AppState>>,
    conversation_id: Option<String>,
) -> Result<WorkbenchViewModel, String> {
    get_workbench_view_model_with_state(
        state.inner(),
        conversation_id.as_deref().filter(|value| !value.is_empty()),
    )
    .await
}

#[cfg(test)]
pub(crate) async fn get_tasks_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<TasksViewModel>, String> {
    Ok(load_tasks_read_model_snapshot(state).await?.envelope)
}

struct TasksReadModelSnapshot {
    envelope: ViewModelEnvelope<TasksViewModel>,
    review_envelope: ViewModelEnvelope<ReviewCenterViewModel>,
    activity_by_task: BTreeMap<String, Vec<WorkspaceActivityItem>>,
}

async fn load_tasks_read_model_snapshot(
    state: &Arc<AppState>,
) -> Result<TasksReadModelSnapshot, String> {
    let mut warnings = Vec::new();
    let review_envelope = load_review_envelope(state).await;
    let review_projection_authoritative =
        review_projection_is_authoritative(&review_envelope, &mut warnings);
    let loaded_tasks =
        load_canonical_task_inputs(state, review_projection_authoritative, &mut warnings).await;
    let model = build_tasks_view_model(TasksViewModelBuildInput {
        task_inputs: loaded_tasks.task_inputs,
        source_refs: vec![
            source_ref(
                "canonical_task_runtime_store",
                "Canonical Task, Run, Item, Attempt, FinalResult, and Artifact store",
            ),
            source_ref("review_center_view_model", "ReviewCenterViewModel"),
        ],
        contract_limitations: vec![
            "Task completion comes only from canonical FinalResult or verified Artifact delivery; Conversation text alone is not completion proof.".into(),
            "Review actions are requests only; materialization requires a refreshed canonical Task snapshot.".into(),
        ],
    });
    let canonical_runtime_degraded = warnings.iter().any(|warning| {
        matches!(
            warning.code.as_str(),
            "canonical_task_runtime_store_unavailable" | "canonical_task_runtime_read_failed"
        )
    });
    let status = if canonical_runtime_degraded {
        if model.items.is_empty() {
            ViewModelStatus::Error
        } else {
            ViewModelStatus::Stale
        }
    } else if model.items.is_empty() {
        ViewModelStatus::Empty
    } else if !warnings.is_empty() {
        ViewModelStatus::Stale
    } else {
        ViewModelStatus::Ready
    };
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.warnings = warnings;
    Ok(TasksReadModelSnapshot {
        envelope,
        review_envelope,
        activity_by_task: loaded_tasks.activity_by_task,
    })
}

pub(crate) async fn get_workbench_view_model_with_state(
    state: &Arc<AppState>,
    requested_conversation_id: Option<&str>,
) -> Result<WorkbenchViewModel, String> {
    let captured_at = chrono::Utc::now().to_rfc3339();
    let snapshot = load_tasks_read_model_snapshot(state).await?;
    let tasks = snapshot.envelope.clone();
    let review = snapshot.review_envelope.clone();
    let provider_boundary = load_provider_boundary_envelope(state).await;
    let workspace = compose_workspace_envelope(
        snapshot,
        provider_boundary.clone(),
        requested_conversation_id,
    );
    Ok(WorkbenchViewModel {
        captured_at,
        workspace,
        tasks,
        review,
        provider_boundary,
    })
}

async fn load_provider_boundary_envelope(
    state: &Arc<AppState>,
) -> ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
    match get_provider_privacy_boundary_summary_with_state(state).await {
        Ok(envelope) => envelope,
        Err(error) => error_envelope(
            "provider_privacy_boundary_unavailable",
            format!("Provider privacy boundary could not be loaded: {error}"),
        ),
    }
}

fn compose_workspace_envelope(
    mut snapshot: TasksReadModelSnapshot,
    provider_envelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>,
    conversation_id: Option<&str>,
) -> ViewModelEnvelope<WorkspaceViewModel> {
    let tasks_status = snapshot.envelope.status;
    let Some(tasks) = snapshot.envelope.data.take() else {
        return error_envelope(
            "tasks_view_model_data_missing",
            "TasksViewModel data unavailable for WorkspaceViewModel",
        );
    };
    let has_task_data = !tasks.items.is_empty();
    let active_task_id = tasks.items.iter().find_map(|item| {
        let in_selected_conversation = conversation_id
            .is_none_or(|selected| item.conversation_id.as_deref() == Some(selected));
        (in_selected_conversation
            && matches!(
                item.lifecycle_status,
                TaskLifecycleStatus::Running
                    | TaskLifecycleStatus::WaitingReview
                    | TaskLifecycleStatus::WaitingPermission
                    | TaskLifecycleStatus::Blocked
            ))
        .then_some(item.canonical_task_id.clone())
    });
    let active_task_activity = active_task_id
        .as_ref()
        .and_then(|task_id| snapshot.activity_by_task.remove(task_id))
        .unwrap_or_default();
    let provider_status = provider_envelope.status;
    let model = build_workspace_view_model(WorkspaceViewModelBuildInput {
        tasks,
        selected_conversation_id: conversation_id.map(str::to_owned),
        active_task_activity,
        source_refs: vec![source_ref(
            "main_chat_task_evidence_view",
            "Metadata-safe Main Chat task activity",
        )],
        contract_limitations: vec![
            "Task controls and review actions are requests only; completion requires a refreshed backend read model.".into(),
            "Workspace activity is metadata-only. Resource, Web, and artifact bodies remain behind their typed evidence owners.".into(),
            "When selectedConversationId is present, activity is restricted to that exact Conversation; Task and Review entities remain in their single Workbench lanes.".into(),
        ],
    });
    let status = workspace_composition_status(tasks_status, provider_status, has_task_data);
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.warnings = snapshot.envelope.warnings;
    envelope.warnings.extend(provider_envelope.warnings);
    envelope
}

fn error_envelope<T>(code: &str, message: impl Into<String>) -> ViewModelEnvelope<T> {
    let mut envelope = ViewModelEnvelope::backend_read_model(ViewModelStatus::Error, None);
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.warnings.push(warning(code, message));
    envelope
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

async fn load_canonical_task_inputs(
    state: &Arc<AppState>,
    review_projection_authoritative: bool,
    warnings: &mut Vec<ViewModelWarning>,
) -> LoadedTaskInputs {
    let Some(store) = state.canonical_task_runtime_store.as_ref() else {
        warnings.push(warning(
            "canonical_task_runtime_store_unavailable",
            "TasksViewModel cannot prove Task, Run, Item, Attempt, FinalResult, or Artifact truth because CanonicalTaskRuntimeStore is unavailable.",
        ));
        return LoadedTaskInputs::default();
    };
    let snapshots_with_plans = {
        let store = store.lock().await;
        let snapshots = match store.list_task_snapshots(100) {
            Ok(snapshots) => snapshots,
            Err(error) => {
                warnings.push(warning(
                    "canonical_task_runtime_read_failed",
                    format!("TasksViewModel could not load canonical Task snapshots: {error}"),
                ));
                return LoadedTaskInputs::default();
            }
        };
        snapshots
            .into_iter()
            .map(|snapshot| {
                let plan = match snapshot.runs.last() {
                    Some(run) => store.load_work_plan(&run.run_id),
                    None => Ok(None),
                };
                (snapshot, plan)
            })
            .collect::<Vec<_>>()
    };
    let mut loaded = LoadedTaskInputs::default();
    for (snapshot, plan) in snapshots_with_plans {
        let plan = match plan {
            Ok(plan) => plan,
            Err(error) => {
                warnings.push(warning(
                    "canonical_work_plan_read_failed",
                    format!(
                        "TasksViewModel could not load the canonical Work plan for task {}: {error}",
                        snapshot.task.id
                    ),
                ));
                None
            }
        };
        let (input, activity) =
            canonical_task_input(state, review_projection_authoritative, snapshot, plan).await;
        loaded.activity_by_task.insert(
            input
                .canonical_task_id
                .clone()
                .unwrap_or_else(|| input.task_id.clone()),
            activity,
        );
        loaded.task_inputs.push(input);
    }
    loaded
}

async fn canonical_task_input(
    state: &Arc<AppState>,
    review_projection_authoritative: bool,
    snapshot: CanonicalTaskSnapshot,
    work_plan: Option<CanonicalWorkPlanRecord>,
) -> (TaskViewModelTaskInput, Vec<WorkspaceActivityItem>) {
    let run_ids = snapshot
        .runs
        .iter()
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
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
    let mut canonical_artifacts = Vec::with_capacity(snapshot.artifacts.len());
    for artifact in &snapshot.artifacts {
        canonical_artifacts.push(canonical_artifact_view(state, &snapshot, artifact).await);
    }
    let (lifecycle_status, terminal_status, delivery_proven) = if !snapshot.artifacts.is_empty() {
        canonical_artifact_delivery_status(&snapshot, &canonical_artifacts)
    } else {
        canonical_general_delivery_status(&snapshot)
    };
    let pending_review_item_refs = snapshot
        .artifacts
        .iter()
        .filter(|artifact| artifact.artifact.status == CanonicalArtifactStatus::WaitingReview)
        .filter_map(|artifact| artifact.review_checkpoint.as_ref())
        .filter(|checkpoint| checkpoint.status == "waiting")
        .map(|checkpoint| &checkpoint.proposal_id)
        .map(|proposal_id| BackendEntityRef {
            id: proposal_id.clone(),
            kind: BackendEntityKind::ReviewItem,
            label: "Artifact review".into(),
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
    let preview = canonical_result_preview(state, &snapshot, &canonical_artifacts)
        .await
        .or_else(|| canonical_task_preview(&snapshot));
    let canonical_evidence = vec![
        source_ref(snapshot.task.id.clone(), "Canonical Task snapshot"),
        source_ref(
            format!("{}:attempts", snapshot.task.id),
            "Canonical ItemAttempt snapshot",
        ),
    ];
    let activity = snapshot
        .items
        .iter()
        .map(|item| {
            WorkspaceActivityItem::from_product_event(
                item.id.clone(),
                item.kind.as_str(),
                canonical_item_activity_summary(item.kind, &item.summary_code),
                Some(item.status.as_str()),
                None,
                vec![EvidenceRef {
                    id: item.id.clone(),
                    label: "Canonical Task Item".into(),
                    source: EvidenceSource::Task,
                    sensitivity: Some(EvidenceSensitivity::LocalPrivate),
                }],
                Some(item.updated_at),
            )
        })
        .collect();
    let title = canonical_task_title(state, &snapshot).await;
    let attention_reason_codes = snapshot
        .attention
        .iter()
        .filter(|attention| attention.resolved_at.is_none())
        .map(|attention| attention.reason_code.clone())
        .collect::<Vec<_>>();
    let needs_attention = !attention_reason_codes.is_empty();
    let retry_scope_stale = snapshot.attention.iter().any(|attention| {
        attention.resolved_at.is_none()
            && attention.kind == openlife_core::task_runtime::CanonicalAttentionKind::ScopeStale
    });
    let completion_disposition = snapshot.final_result.as_ref().map(|result| {
        if result.summary_code.ends_with("_with_disclosed_limitations") {
            TaskCompletionDisposition::CompleteWithDisclosedLimitations
        } else {
            TaskCompletionDisposition::Complete
        }
    });
    (
        TaskViewModelTaskInput {
            task_id: snapshot.task.id.clone(),
            canonical_task_id: Some(snapshot.task.id.clone()),
            conversation_id: Some(snapshot.task.conversation_id),
            title,
            related_run_ids: run_ids,
            final_delivery_present: false,
            final_delivery_status: None,
            canonical_lifecycle_status: Some(lifecycle_status),
            canonical_terminal_delivery_status: Some(terminal_status),
            canonical_final_delivery_evidence_present: Some(delivery_proven),
            completion_disposition,
            canonical_items,
            work_plan: work_plan.map(|record| TaskWorkPlanViewModel {
                revision: record.plan_revision,
                steps: record
                    .plan
                    .steps
                    .into_iter()
                    .map(|step| TaskWorkPlanStepViewModel {
                        id: step.id,
                        kind: step.kind,
                        required: step.required,
                        depends_on: step.depends_on,
                    })
                    .collect(),
                completion: record.plan.completion,
                budget_policy: record.budget_policy,
            }),
            canonical_artifacts,
            pending_blockers: blockers,
            needs_attention,
            attention_reason_codes,
            pending_review_item_refs,
            review_projection_authoritative,
            allowed_control_ids: match snapshot.task.status {
                CanonicalTaskStatus::Running => vec!["cancel".into()],
                CanonicalTaskStatus::Failed
                | CanonicalTaskStatus::Blocked
                | CanonicalTaskStatus::Cancelled
                | CanonicalTaskStatus::Interrupted
                    if !retry_scope_stale =>
                {
                    vec!["retry".into()]
                }
                _ => Vec::new(),
            },
            retry_action_id: snapshot.runs.last().map(|run| run.run_id.clone()),
            next_recommended_control: Some("open_trace".into()),
            latest_result_preview: preview,
            evidence_refs: canonical_evidence,
            updated_at: Some(snapshot.task.updated_at),
        },
        activity,
    )
}

async fn canonical_task_title(state: &Arc<AppState>, snapshot: &CanonicalTaskSnapshot) -> String {
    let fallback = "Work";
    let Some(store) = state.conversation_store.as_ref() else {
        return fallback.into();
    };
    let Ok(items) = store
        .lock()
        .await
        .list_items(&snapshot.task.conversation_id, 200)
    else {
        return fallback.into();
    };
    items
        .into_iter()
        .find(|item| {
            item.kind == ConversationItemKind::UserMessage
                && item.content_digest == snapshot.task.initial_outcome_digest
        })
        .map(|item| bounded_task_title(&item.content))
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| fallback.into())
}

fn bounded_task_title(goal: &str) -> String {
    const MAX_TITLE_CHARS: usize = 120;
    let normalized = goal.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let title = chars.by_ref().take(MAX_TITLE_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{title}…")
    } else {
        title
    }
}

fn canonical_item_activity_summary(
    kind: openlife_core::task_runtime::CanonicalTaskItemKind,
    summary_code: &str,
) -> String {
    let tool_name = summary_code.split_once(':').map(|(_, tool)| tool);
    match (kind, tool_name) {
        (openlife_core::task_runtime::CanonicalTaskItemKind::ToolCall, Some(tool)) => {
            format!("正在使用 {}。", canonical_tool_label(tool))
        }
        (openlife_core::task_runtime::CanonicalTaskItemKind::Observation, Some(tool)) => {
            format!("已取得 {} 的可核对结果。", canonical_tool_label(tool))
        }
        (openlife_core::task_runtime::CanonicalTaskItemKind::ProviderGeneration, _) => {
            "模型正在根据已授权的上下文生成结果。".into()
        }
        (openlife_core::task_runtime::CanonicalTaskItemKind::Instruction, _) => {
            "用户任务已经绑定到本次执行。".into()
        }
        (openlife_core::task_runtime::CanonicalTaskItemKind::Plan, _) => {
            "任务执行计划已经记录。".into()
        }
        (openlife_core::task_runtime::CanonicalTaskItemKind::FinalResult, _) => {
            "最终结果已经记录并可供核对。".into()
        }
        _ => summary_code.replace('_', " "),
    }
}

fn canonical_tool_label(tool: &str) -> &str {
    match tool {
        "document.read" => "本地文档读取",
        "web.search" => "Web 搜索",
        "web.fetch" => "网页读取",
        "mcp.read_only" => "MCP 只读工具",
        other => other,
    }
}

fn canonical_general_delivery_status(
    snapshot: &CanonicalTaskSnapshot,
) -> (TaskLifecycleStatus, TaskTerminalDeliveryStatus, bool) {
    let delivered = snapshot.final_result.as_ref().is_some_and(|result| {
        snapshot.items.iter().any(|item| {
            item.id == result.item_id
                && item.run_id == result.run_id
                && item.kind == openlife_core::task_runtime::CanonicalTaskItemKind::FinalResult
                && item.status == openlife_core::task_runtime::CanonicalTaskItemStatus::Completed
                && item.payload_digest == result.result_digest
        })
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
        CanonicalTaskStatus::Completed if delivered => (
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
        CanonicalTaskStatus::Cancelled => (
            TaskLifecycleStatus::Cancelled,
            TaskTerminalDeliveryStatus::Cancelled,
            false,
        ),
        CanonicalTaskStatus::Interrupted => (
            TaskLifecycleStatus::Interrupted,
            TaskTerminalDeliveryStatus::NotTerminal,
            false,
        ),
        CanonicalTaskStatus::EffectUnknown => (
            TaskLifecycleStatus::RemoteUnknown,
            TaskTerminalDeliveryStatus::Unknown,
            false,
        ),
    }
}

async fn canonical_result_preview(
    state: &Arc<AppState>,
    snapshot: &CanonicalTaskSnapshot,
    artifact_views: &[TaskArtifactViewModel],
) -> Option<String> {
    let result = snapshot.final_result.as_ref()?;
    if !artifact_views.is_empty() {
        let undone = snapshot
            .artifacts
            .iter()
            .filter(|artifact| {
                artifact
                    .undo
                    .as_ref()
                    .is_some_and(|undo| undo.status == "undone")
            })
            .count();
        if undone > 0 {
            return Some(format!(
                "任务已经完成并通过核验；{undone} 个产物随后已按用户请求撤销。"
            ));
        }
        let verified_previews = artifact_views
            .iter()
            .filter(|artifact| {
                artifact.verification.status == TaskArtifactVerificationStatus::Verified
            })
            .filter_map(|artifact| artifact.preview.content.as_deref())
            .collect::<Vec<_>>();
        if !verified_previews.is_empty() {
            return Some(
                verified_previews
                    .join("\n\n")
                    .chars()
                    .take(TASK_ARTIFACT_PREVIEW_MAX_CHARS)
                    .collect(),
            );
        }
    }
    let store = state.conversation_store.as_ref()?;
    store
        .lock()
        .await
        .list_items(&snapshot.task.conversation_id, 200)
        .ok()?
        .into_iter()
        .find(|item| {
            item.id == result.conversation_item_id && item.content_digest == result.result_digest
        })
        .map(|item| item.content.chars().take(800).collect())
}

fn canonical_artifact_delivery_status(
    snapshot: &CanonicalTaskSnapshot,
    artifact_views: &[TaskArtifactViewModel],
) -> (TaskLifecycleStatus, TaskTerminalDeliveryStatus, bool) {
    let final_result_present = snapshot.final_result.as_ref().is_some_and(|result| {
        let expected_id =
            openlife_core::task_runtime::final_result_item_id(&snapshot.task.id, &result.run_id);
        result.item_id == expected_id
            && snapshot.items.iter().any(|item| {
                item.id == result.item_id
                    && item.run_id == result.run_id
                    && item.kind == openlife_core::task_runtime::CanonicalTaskItemKind::FinalResult
                    && item.status
                        == openlife_core::task_runtime::CanonicalTaskItemStatus::Completed
            })
    });
    let delivery_proven = !snapshot.artifacts.is_empty()
        && artifact_views.len() == snapshot.artifacts.len()
        && snapshot
            .artifacts
            .iter()
            .zip(artifact_views.iter())
            .all(|(artifact, artifact_view)| {
            let observed_digest = artifact
                .current_version
                .observed_content_digest
                .as_deref()
                .unwrap_or("");
            let expected_verification_id = openlife_core::task_runtime::artifact_verification_item_id(
                &artifact.artifact.id,
                artifact.current_version.version,
                observed_digest,
            );
            let governed_undo_proven = artifact
                .undo
                .as_ref()
                .is_some_and(|undo| undo.status == "undone")
                && snapshot.items.iter().any(|item| {
                    item.kind
                        == openlife_core::task_runtime::CanonicalTaskItemKind::ReviewCheckpoint
                        && item.status
                            == openlife_core::task_runtime::CanonicalTaskItemStatus::Completed
                        && item.summary_code == "artifact_undo_confirmed"
                        && snapshot.attempts.iter().any(|attempt| {
                            attempt.item_id == item.id
                                && attempt.executor_kind == "materializer"
                                && attempt.status
                                    == openlife_core::task_runtime::CanonicalTaskItemStatus::Completed
                                && attempt.receipt_digest.is_some()
                        })
                });
            artifact.artifact.status == CanonicalArtifactStatus::Materialized
                && artifact.artifact.content_digest == observed_digest
                && artifact.artifact.materialized_reference.is_some()
                && artifact.artifact.materialized_reference
                    == artifact.current_version.materialized_reference
                && snapshot.items.iter().any(|item| {
                    item.id == expected_verification_id
                        && item.kind
                            == openlife_core::task_runtime::CanonicalTaskItemKind::Verification
                        && item.status
                            == openlife_core::task_runtime::CanonicalTaskItemStatus::Completed
                })
                && (artifact_view.verification.status
                    == TaskArtifactVerificationStatus::Verified
                    || governed_undo_proven)
        })
        && final_result_present;
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
        CanonicalTaskStatus::Cancelled => (
            TaskLifecycleStatus::Cancelled,
            TaskTerminalDeliveryStatus::Cancelled,
            false,
        ),
        CanonicalTaskStatus::Interrupted => (
            TaskLifecycleStatus::Interrupted,
            TaskTerminalDeliveryStatus::NotTerminal,
            false,
        ),
        CanonicalTaskStatus::EffectUnknown => (
            TaskLifecycleStatus::RemoteUnknown,
            TaskTerminalDeliveryStatus::Unknown,
            false,
        ),
    }
}

async fn canonical_artifact_view(
    state: &Arc<AppState>,
    task: &CanonicalTaskSnapshot,
    snapshot: &CanonicalArtifactSnapshot,
) -> TaskArtifactViewModel {
    let proposal_ref = snapshot
        .review_checkpoint
        .as_ref()
        .map(|checkpoint| BackendEntityRef {
            id: checkpoint.proposal_id.clone(),
            kind: BackendEntityKind::ReviewItem,
            label: "Artifact Review checkpoint".into(),
            href: None,
        });
    let verification_item_present = snapshot
        .current_version
        .observed_content_digest
        .as_deref()
        .is_some_and(|observed| {
            let expected = openlife_core::task_runtime::artifact_verification_item_id(
                &snapshot.artifact.id,
                snapshot.current_version.version,
                observed,
            );
            task.items.iter().any(|item| {
                item.id == expected
                    && item.kind == openlife_core::task_runtime::CanonicalTaskItemKind::Verification
                    && item.status
                        == openlife_core::task_runtime::CanonicalTaskItemStatus::Completed
            })
        });
    let presentation =
        artifact_presentation(state, snapshot, task, verification_item_present).await;
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
        undo: artifact_undo_view(snapshot, &presentation.change, &presentation.verification),
        change: presentation.change,
        preview: presentation.preview,
        verification: presentation.verification,
    }
}

fn artifact_undo_view(
    snapshot: &CanonicalArtifactSnapshot,
    change: &TaskArtifactChangeViewModel,
    verification: &TaskArtifactVerificationViewModel,
) -> TaskArtifactUndoViewModel {
    if let Some(undo) = snapshot.undo.as_ref() {
        return TaskArtifactUndoViewModel {
            available: false,
            status: Some(undo.status.clone()),
            proposal_ref: Some(BackendEntityRef {
                id: undo.proposal_id.clone(),
                kind: BackendEntityKind::ReviewItem,
                label: "Artifact Undo Review checkpoint".into(),
                href: None,
            }),
            reason_code: (undo.status != "undone")
                .then(|| "artifact_undo_pending_or_failed".into()),
        };
    }
    let available = snapshot.artifact.status == CanonicalArtifactStatus::Materialized
        && verification.status == TaskArtifactVerificationStatus::Verified
        && change.kind == TaskArtifactChangeKind::Create;
    TaskArtifactUndoViewModel {
        available,
        status: None,
        proposal_ref: None,
        reason_code: (!available).then(|| {
            if change.kind == TaskArtifactChangeKind::Replace {
                "artifact_undo_unavailable_without_original_bytes".into()
            } else if verification.status != TaskArtifactVerificationStatus::Verified {
                "artifact_undo_requires_verified_materialization".into()
            } else {
                "artifact_undo_unavailable".into()
            }
        }),
    }
}

struct ArtifactPresentation {
    change: TaskArtifactChangeViewModel,
    preview: TaskArtifactPreviewViewModel,
    verification: TaskArtifactVerificationViewModel,
}

async fn artifact_presentation(
    state: &Arc<AppState>,
    snapshot: &CanonicalArtifactSnapshot,
    task: &CanonicalTaskSnapshot,
    verification_item_present: bool,
) -> ArtifactPresentation {
    let unavailable = |reason: &str| TaskArtifactPreviewViewModel {
        status: TaskArtifactPreviewStatus::Unavailable,
        content: None,
        reason_code: Some(reason.to_string()),
    };
    let mut change = TaskArtifactChangeViewModel {
        kind: TaskArtifactChangeKind::Unknown,
        status: snapshot.artifact.status,
        target_reference: snapshot.current_version.materialized_reference.clone(),
        expected_prior_digest: None,
    };
    let mut preview = unavailable("artifact_preview_source_unavailable");
    let mut verification = TaskArtifactVerificationViewModel {
        status: match snapshot.artifact.status {
            CanonicalArtifactStatus::Draft | CanonicalArtifactStatus::WaitingReview => {
                TaskArtifactVerificationStatus::Pending
            }
            CanonicalArtifactStatus::Materialized | CanonicalArtifactStatus::Failed => {
                TaskArtifactVerificationStatus::Failed
            }
            CanonicalArtifactStatus::EffectUnknown => TaskArtifactVerificationStatus::Unknown,
        },
        expected_content_digest: snapshot.artifact.content_digest.clone(),
        observed_content_digest: snapshot.current_version.observed_content_digest.clone(),
        verification_item_present,
        reason_code: None,
    };

    if let (Some(target), Some(expected_absent)) = (
        snapshot.current_version.target_reference.as_ref(),
        snapshot.current_version.expected_target_absent,
    ) {
        change.kind = if expected_absent {
            TaskArtifactChangeKind::Create
        } else {
            TaskArtifactChangeKind::Replace
        };
        change.target_reference = Some(target.clone());
        change.expected_prior_digest = snapshot.current_version.expected_target_digest.clone();
    }

    if matches!(
        snapshot.artifact.status,
        CanonicalArtifactStatus::Draft | CanonicalArtifactStatus::WaitingReview
    ) {
        if let Some(draft_reference) = snapshot.current_version.draft_reference.as_deref() {
            match read_canonical_artifact_draft(
                state,
                draft_reference,
                &snapshot.artifact.content_digest,
                &snapshot.artifact.media_type,
            )
            .await
            {
                Ok(content) => preview = bounded_artifact_preview(&content),
                Err(reason) => preview = unavailable(&reason),
            }
        }
        verification.reason_code = Some("artifact_waiting_materialization".to_string());
        return ArtifactPresentation {
            change,
            preview,
            verification,
        };
    }

    if snapshot.artifact.status == CanonicalArtifactStatus::Materialized {
        let Some(path) = snapshot.current_version.materialized_reference.as_deref() else {
            verification.reason_code = Some("artifact_materialized_reference_missing".to_string());
            return ArtifactPresentation {
                change,
                preview,
                verification,
            };
        };
        let safe_paths = match canonical_artifact_safe_paths(state, snapshot, task).await {
            Ok(paths) => paths,
            Err(reason) => {
                verification.reason_code = Some(reason);
                return ArtifactPresentation {
                    change,
                    preview,
                    verification,
                };
            }
        };
        match read_verified_artifact(path, &safe_paths, &snapshot.artifact.media_type) {
            Ok((digest, content)) => {
                verification.observed_content_digest = Some(digest.clone());
                if digest == snapshot.artifact.content_digest
                    && snapshot.current_version.observed_content_digest.as_deref()
                        == Some(digest.as_str())
                    && verification_item_present
                {
                    verification.status = TaskArtifactVerificationStatus::Verified;
                    preview = bounded_artifact_preview(&content);
                } else {
                    verification.reason_code = Some("artifact_content_digest_drift".to_string());
                }
            }
            Err(reason) => verification.reason_code = Some(reason),
        }
    } else {
        verification.reason_code = Some(
            match snapshot.artifact.status {
                CanonicalArtifactStatus::EffectUnknown => "artifact_effect_unknown",
                CanonicalArtifactStatus::Failed => "artifact_delivery_failed",
                CanonicalArtifactStatus::Draft | CanonicalArtifactStatus::WaitingReview => {
                    "artifact_waiting_materialization"
                }
                CanonicalArtifactStatus::Materialized => unreachable!(),
            }
            .to_string(),
        );
    }
    ArtifactPresentation {
        change,
        preview,
        verification,
    }
}

async fn canonical_artifact_safe_paths(
    state: &Arc<AppState>,
    artifact: &CanonicalArtifactSnapshot,
    task: &CanonicalTaskSnapshot,
) -> Result<Vec<String>, String> {
    let source_run_id = task
        .items
        .iter()
        .find(|item| item.id == artifact.artifact.source_item_id)
        .map(|item| item.run_id.as_str())
        .ok_or_else(|| "artifact_source_run_missing".to_string())?;
    let run = task
        .runs
        .iter()
        .find(|run| run.run_id == source_run_id)
        .ok_or_else(|| "artifact_source_run_missing".to_string())?;
    let root = match run.project_id.as_deref() {
        Some(project_id) => {
            let conversation_store = state
                .conversation_store
                .as_ref()
                .ok_or_else(|| "conversation_store_unavailable".to_string())?;
            let project = conversation_store
                .lock()
                .await
                .get_project(project_id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "artifact_project_missing".to_string())?;
            let scope_digest =
                openlife_core::conversation::ConversationStore::project_scope_digest(&project);
            if run.project_revision != Some(project.revision)
                || run.scope_digest.as_deref() != Some(scope_digest.as_str())
            {
                return Err("artifact_project_scope_stale".into());
            }
            match project.workspace_root {
                Some(root) => PathBuf::from(root),
                None => task_managed_artifact_root(state, &task.task.conversation_id).await?,
            }
        }
        None => task_managed_artifact_root(state, &task.task.conversation_id).await?,
    };
    let canonical = root
        .canonicalize()
        .map_err(|_| "artifact_authorized_root_unavailable".to_string())?;
    Ok(vec![canonical.to_string_lossy().into_owned()])
}

async fn task_managed_artifact_root(
    state: &Arc<AppState>,
    conversation_id: &str,
) -> Result<PathBuf, String> {
    let store = state
        .canonical_task_runtime_store
        .as_ref()
        .ok_or_else(|| "canonical_task_runtime_store_unavailable".to_string())?;
    let database_path = store.lock().await.db_path().map(Path::to_path_buf);
    managed_artifact_root(database_path.as_deref(), conversation_id)
}

fn bounded_artifact_preview(content: &str) -> TaskArtifactPreviewViewModel {
    let char_count = content.chars().count();
    let bounded = content
        .chars()
        .take(TASK_ARTIFACT_PREVIEW_MAX_CHARS)
        .collect::<String>();
    TaskArtifactPreviewViewModel {
        status: if char_count > TASK_ARTIFACT_PREVIEW_MAX_CHARS {
            TaskArtifactPreviewStatus::Truncated
        } else {
            TaskArtifactPreviewStatus::Available
        },
        content: Some(bounded),
        reason_code: (char_count > TASK_ARTIFACT_PREVIEW_MAX_CHARS)
            .then(|| "artifact_preview_truncated".to_string()),
    }
}

async fn read_canonical_artifact_draft(
    state: &Arc<AppState>,
    reference: &str,
    expected_digest: &str,
    media_type: &str,
) -> Result<String, String> {
    let path = Path::new(reference);
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| "canonical_artifact_draft_missing".to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("canonical_artifact_draft_type_invalid".into());
    }
    if metadata.len() > TASK_ARTIFACT_READ_MAX_BYTES {
        return Err("canonical_artifact_draft_too_large".into());
    }
    if !cfg!(test) {
        let store = state
            .canonical_task_runtime_store
            .as_ref()
            .ok_or_else(|| "canonical_artifact_draft_root_unavailable".to_string())?;
        let root = store
            .lock()
            .await
            .db_path()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .map(|parent| parent.join("artifact-drafts"))
            .ok_or_else(|| "canonical_artifact_draft_root_unavailable".to_string())?;
        let canonical_root = root
            .canonicalize()
            .map_err(|_| "canonical_artifact_draft_root_unavailable".to_string())?;
        let canonical_path = path
            .canonicalize()
            .map_err(|_| "canonical_artifact_draft_missing".to_string())?;
        if !canonical_path.starts_with(canonical_root) {
            return Err("canonical_artifact_draft_outside_store".into());
        }
    }
    let bytes =
        std::fs::read(path).map_err(|_| "canonical_artifact_draft_read_failed".to_string())?;
    if crate::artifact_materializer::artifact_content_digest(&bytes) != expected_digest {
        return Err("canonical_artifact_draft_digest_mismatch".into());
    }
    artifact_preview_content(path, media_type, bytes)
}

fn read_verified_artifact(
    path: &str,
    safe_paths: &[String],
    media_type: &str,
) -> Result<(String, String), String> {
    let path = Path::new(path);
    let parent = path
        .parent()
        .ok_or_else(|| "artifact_materialized_parent_missing".to_string())?
        .canonicalize()
        .map_err(|_| "artifact_materialized_parent_unavailable".to_string())?;
    let within_safe_path = safe_paths.iter().any(|safe| {
        let safe = Path::new(safe);
        safe.is_absolute()
            && safe
                .canonicalize()
                .is_ok_and(|canonical_safe| parent.starts_with(canonical_safe))
    });
    if !within_safe_path {
        return Err("artifact_materialized_path_outside_safe_scope".to_string());
    }
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| "artifact_materialized_file_missing".to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("artifact_materialized_file_type_invalid".to_string());
    }
    if metadata.len() > TASK_ARTIFACT_READ_MAX_BYTES {
        return Err("artifact_materialized_file_too_large".to_string());
    }
    let bytes = std::fs::read(path).map_err(|_| "artifact_materialized_read_failed".to_string())?;
    let digest = {
        use sha2::{Digest, Sha256};
        format!("sha256:{:x}", Sha256::digest(&bytes))
    };
    let content = artifact_preview_content(path, media_type, bytes)?;
    Ok((digest, content))
}

fn artifact_preview_content(
    path: &Path,
    media_type: &str,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let normalized = media_type
        .split(';')
        .next()
        .unwrap_or(media_type)
        .trim()
        .to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "text/plain" | "text/markdown" | "text/html" | "text/csv" | "application/json"
    ) {
        return String::from_utf8(bytes).map_err(|_| "artifact_preview_text_not_utf8".to_string());
    }
    let binary_filename = match normalized.as_str() {
        "application/pdf" => Some("artifact.pdf"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            Some("artifact.docx")
        }
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            Some("artifact.xlsx")
        }
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            Some("artifact.pptx")
        }
        _ => None,
    };
    let filename = binary_filename
        .or_else(|| path.file_name().and_then(|value| value.to_str()))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "artifact_preview_filename_missing".to_string())?;
    let extraction = openlife_core::resource_parser::extract_resource(
        openlife_core::resource_parser::ResourceExtractionRequest {
            filename: filename.to_string(),
            declared_mime: normalized,
            bytes,
        },
    )
    .map_err(|_| "artifact_preview_format_verification_failed".to_string())?;
    let content = extraction
        .chunks
        .into_iter()
        .map(|chunk| chunk.content)
        .filter(|content| !content.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if content.is_empty() {
        return Err("artifact_preview_content_empty".into());
    }
    Ok(content)
}

fn canonical_task_preview(snapshot: &CanonicalTaskSnapshot) -> Option<String> {
    let total = snapshot.artifacts.len();
    let materialized = snapshot
        .artifacts
        .iter()
        .filter(|artifact| artifact.artifact.status == CanonicalArtifactStatus::Materialized)
        .count();
    match snapshot.task.status {
        CanonicalTaskStatus::WaitingReview => Some(format!("{total} 份结果正在等待你的审核。")),
        CanonicalTaskStatus::Completed => {
            Some(format!("已交付并核验 {materialized} / {total} 份结果。"))
        }
        CanonicalTaskStatus::Blocked => Some("任务尚未交付，需要先处理当前阻塞。".into()),
        CanonicalTaskStatus::EffectUnknown => {
            Some("结果写入状态未知，OpenLife 没有自动重放。".into())
        }
        CanonicalTaskStatus::Failed => Some("任务在完成可核验交付前失败。".into()),
        CanonicalTaskStatus::Cancelled => Some("任务已取消，未产生最终交付。".into()),
        CanonicalTaskStatus::Interrupted => Some("任务已中断，可以从保留的工作记录重试。".into()),
        CanonicalTaskStatus::Running => None,
    }
}

async fn load_review_envelope(state: &Arc<AppState>) -> ViewModelEnvelope<ReviewCenterViewModel> {
    match get_review_center_view_model_with_state(state).await {
        Ok(envelope) => envelope,
        Err(error) => error_envelope(
            "review_center_view_model_unavailable",
            format!("ReviewCenterViewModel could not be loaded: {error}"),
        ),
    }
}

fn review_projection_is_authoritative(
    envelope: &ViewModelEnvelope<ReviewCenterViewModel>,
    warnings: &mut Vec<ViewModelWarning>,
) -> bool {
    match envelope.status {
        ViewModelStatus::Ready | ViewModelStatus::Empty => match envelope.data.as_ref() {
            Some(_) => true,
            None => {
                warnings.push(warning(
                    "review_center_view_model_data_missing",
                    "TasksViewModel could not prove review-item absence because ReviewCenterViewModel returned no data.",
                ));
                false
            }
        },
        _ => {
            warnings.push(warning(
                "review_center_view_model_not_authoritative",
                format!(
                    "TasksViewModel could not prove review-item absence because ReviewCenterViewModel status is {:?}.",
                    envelope.status
                ),
            ));
            false
        }
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
        get_tasks_view_model_with_state, get_workbench_view_model_with_state,
        workspace_composition_status,
    };
    use openlife_core::agent::ViewModelStatus;
    use std::sync::Arc;

    #[tokio::test]
    async fn recent_work_uses_the_user_goal_instead_of_a_generic_internal_title() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        crate::main_chat_acceptance_test_support::
            configure_live_provider_eval_state_with_local_http_provider(
                &state,
                "A bounded research result.",
            )
            .await;
        let conversation_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_conversation(&conversation_id, "Continuous learning research")
            .unwrap();
        let goal = "整理 continuous learning 的核心概念和近期实践";
        crate::canonical_work_runtime::run_canonical_work(
            crate::canonical_work_runtime::CanonicalWorkInput {
                task_id: uuid::Uuid::new_v4().to_string(),
                run_id: uuid::Uuid::new_v4().to_string(),
                turn_id: uuid::Uuid::new_v4().to_string(),
                conversation_id,
                messages: vec![openlife_core::llm::ChatMessage {
                    role: "user".into(),
                    content: goal.into(),
                }],
                selected_skill_id: None,
                stream: false,
            },
            &state,
            &mut |_, _| {},
        )
        .await
        .unwrap();

        let envelope = get_tasks_view_model_with_state(&state).await.unwrap();
        let task = envelope.data.unwrap().items.into_iter().next().unwrap();
        assert_eq!(task.title, goal);
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
    async fn tasks_view_model_fails_closed_when_canonical_task_store_is_unavailable() {
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
    async fn empty_canonical_tasks_remain_usable_when_review_is_degraded() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .proposal_store = None;

        let envelope = get_tasks_view_model_with_state(&state).await.unwrap();

        assert_eq!(envelope.status, ViewModelStatus::Empty);
        assert!(envelope.data.is_some_and(|model| model.items.is_empty()));
        assert!(envelope
            .warnings
            .iter()
            .any(|warning| warning.code.contains("review_center")));
    }

    #[tokio::test]
    async fn stale_retry_scope_requires_a_new_work_instead_of_reoffering_retry() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        let store = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await;
        store
            .begin_general_task_run(openlife_core::task_runtime::BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: &turn_id,
                instruction_digest: &openlife_core::agent::metadata_safe_text_digest(
                    "retry under the original provider",
                )
                .1,
                plan_digest: None,
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
            .unwrap();
        store
            .terminalize_general_run(
                &task_id,
                &run_id,
                openlife_core::task_runtime::CanonicalTaskStatus::Failed,
            )
            .unwrap();
        store
            .record_attention(
                &task_id,
                &run_id,
                openlife_core::task_runtime::CanonicalAttentionKind::ScopeStale,
                "work_provider_binding_stale",
            )
            .unwrap();
        drop(store);

        let envelope = get_tasks_view_model_with_state(&state).await.unwrap();
        let task = envelope
            .data
            .unwrap()
            .items
            .into_iter()
            .find(|task| task.canonical_task_id == task_id)
            .unwrap();
        assert!(task.needs_attention);
        assert_eq!(
            task.attention_reason_codes,
            vec!["work_provider_binding_stale"]
        );
        assert!(!task
            .allowed_controls
            .iter()
            .any(|control| control.kind == openlife_core::agent::TaskControlKind::Retry));
    }

    #[tokio::test]
    async fn workbench_snapshot_filters_work_lanes_by_live_conversation_identity() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        crate::commands::chat::create_chat_session_with_state(
            &conversation_id,
            "Workbench aggregate",
            &state,
        )
        .await
        .unwrap();

        let snapshot = get_workbench_view_model_with_state(&state, Some(&conversation_id))
            .await
            .unwrap();

        assert!(!snapshot.captured_at.is_empty());
        assert_eq!(
            snapshot
                .workspace
                .data
                .as_ref()
                .and_then(|model| model.selected_conversation_id.as_deref()),
            Some(conversation_id.as_str())
        );
        assert!(snapshot.tasks.data.is_some());
        assert!(snapshot.review.data.is_some());
        assert!(snapshot.provider_boundary.data.is_some());
    }

    #[tokio::test]
    async fn tasks_view_model_projects_canonical_artifact_delivery_and_undo_availability() {
        use sha2::{Digest, Sha256};

        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let artifact_dir = tempfile::tempdir().unwrap();
        let artifact_path = artifact_dir.path().join("report-view.md");
        let artifact_path_text = artifact_path.to_string_lossy().into_owned();
        state.config.lock().await.system.safe_paths =
            vec![artifact_dir.path().to_string_lossy().into_owned()];
        let content = "# Canonical report";
        let content_digest = format!("sha256:{:x}", Sha256::digest(content.as_bytes()));
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let project_id = uuid::Uuid::new_v4().to_string();
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let project = {
            let conversation_store = state.conversation_store.as_ref().unwrap().lock().await;
            conversation_store
                .create_project(
                    &project_id,
                    "Artifact View Project",
                    Some(&artifact_dir.path().to_string_lossy()),
                )
                .unwrap();
            conversation_store
                .create_conversation(&conversation_id, "Artifact View")
                .unwrap();
            conversation_store
                .assign_conversation_project(&conversation_id, Some(&project_id))
                .unwrap();
            conversation_store
                .get_project(&project_id)
                .unwrap()
                .unwrap()
        };
        let scope_digest =
            openlife_core::conversation::ConversationStore::project_scope_digest(&project);
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_general_task_run(openlife_core::task_runtime::BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: "turn-artifact-view",
                instruction_digest: &format!(
                    "sha256:{:x}",
                    Sha256::digest(b"artifact view outcome")
                ),
                plan_digest: None,
                project_id: Some(&project_id),
                project_revision: Some(project.revision),
                scope_digest: Some(&scope_digest),
            })
            .unwrap();
        let work_plan = openlife_core::work_orchestration::StructuredWorkPlan {
            schema_version: openlife_core::work_orchestration::WORK_PLAN_SCHEMA_VERSION.into(),
            steps: vec![
                openlife_core::work_orchestration::WorkPlanStep {
                    id: "draft".into(),
                    kind: openlife_core::work_orchestration::WorkPlanStepKind::DraftArtifact,
                    required: true,
                    depends_on: Vec::new(),
                    target_id: None,
                    target_contract_digest: None,
                },
                openlife_core::work_orchestration::WorkPlanStep {
                    id: "verify".into(),
                    kind: openlife_core::work_orchestration::WorkPlanStepKind::Verify,
                    required: true,
                    depends_on: vec!["draft".into()],
                    target_id: None,
                    target_contract_digest: None,
                },
                openlife_core::work_orchestration::WorkPlanStep {
                    id: "deliver".into(),
                    kind: openlife_core::work_orchestration::WorkPlanStepKind::DeliverResult,
                    required: true,
                    depends_on: vec!["verify".into()],
                    target_id: None,
                    target_contract_digest: None,
                },
            ],
            completion: openlife_core::work_orchestration::WorkCompletionContract {
                result_kind: openlife_core::work_orchestration::WorkResultKind::Artifact,
                requires_verification: true,
                requirements: vec![
                    openlife_core::work_orchestration::WorkCompletionRequirement {
                        id: "artifact".into(),
                        description: "The requested Artifact is complete.".into(),
                        evidence_kind:
                            openlife_core::work_orchestration::WorkCompletionEvidenceKind::Result,
                        allow_transparent_limitation: false,
                    },
                ],
                requires_review_before_write: false,
            },
            source_constraints: Default::default(),
        };
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .persist_work_plan(
                &task_id,
                &run_id,
                1,
                &work_plan,
                openlife_core::work_orchestration::WorkRunBudgetPolicy::default(),
            )
            .unwrap();
        let prepared = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .prepare_general_artifact(openlife_core::task_runtime::GeneralArtifactDraftInput {
                task_id: &task_id,
                run_id: &run_id,
                target_reference: &artifact_path_text,
                content_digest: &content_digest,
                media_type: "text/markdown; charset=utf-8",
            })
            .unwrap();
        let draft_path = artifact_dir.path().join("report-view.v1.draft");
        std::fs::write(&draft_path, content).unwrap();
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .bind_general_artifact_version_source(
                &prepared.artifact_id,
                prepared.version,
                &artifact_path_text,
                &draft_path.to_string_lossy(),
                true,
                None,
            )
            .unwrap();
        let mut proposal = openlife_core::agent::AgentProposal::new(
            openlife_core::agent::ProposalType::ExternalWriteAction,
            &format!("filesystem.{artifact_path_text}"),
            serde_json::json!({
                "reviewSubjectSchema": openlife_core::task_runtime::CANONICAL_ARTIFACT_REVIEW_SUBJECT_SCHEMA,
                "generatedByProvider": true,
                "canonicalTaskId": task_id,
                "sourceRunId": run_id,
                "artifactDraftItemId": prepared.artifact_draft_item_id,
                "path": artifact_path_text,
                "operation": "create",
                "artifactKind": "markdown",
                "contentDigest": content_digest,
                "expectedTargetAbsent": true,
                "expectedTargetDigest": null,
                "artifactId": prepared.artifact_id,
                "artifactVersion": prepared.version,
            }),
            "test exact report preview",
            1.0,
            openlife_core::agent::RiskLevel::High,
            openlife_core::agent::ProposalSource::ChatConversation,
        );
        proposal.id = "proposal-report-view".into();
        proposal.run_id = Some(run_id.clone());
        proposal.source_detail = Some(task_id.clone());
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .bind_artifact_review(&prepared.artifact_id, "proposal-report-view")
            .unwrap();
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .defer_general_task_result(openlife_core::task_runtime::DeferGeneralTaskResultInput {
                task_id: &prepared.task_id,
                run_id: &run_id,
                conversation_item_id: "conversation-item-report-view",
                result_digest: &format!(
                    "sha256:{:x}",
                    Sha256::digest(b"report view waiting review")
                ),
                summary_code: "work_artifact_completed",
            })
            .unwrap();

        let waiting = get_tasks_view_model_with_state(&state).await.unwrap();
        assert_eq!(waiting.status, ViewModelStatus::Ready);
        let waiting = waiting.data.unwrap();
        assert_eq!(waiting.items.len(), 1);
        let task = &waiting.items[0];
        assert_eq!(task.canonical_task_id, prepared.task_id);
        assert_eq!(task.canonical_task_id, prepared.task_id);
        assert_eq!(
            task.lifecycle_status,
            openlife_core::agent::TaskLifecycleStatus::WaitingReview
        );
        assert_eq!(task.items.len(), 4);
        let projected_plan = task.work_plan.as_ref().expect("work plan is projected");
        assert_eq!(projected_plan.revision, 1);
        assert_eq!(projected_plan.steps.len(), 3);
        assert_eq!(
            projected_plan.completion.result_kind,
            openlife_core::work_orchestration::WorkResultKind::Artifact
        );
        assert!(projected_plan.completion.requires_verification);
        assert_eq!(
            task.items.iter().map(|item| item.kind).collect::<Vec<_>>(),
            vec![
                openlife_core::task_runtime::CanonicalTaskItemKind::Instruction,
                openlife_core::task_runtime::CanonicalTaskItemKind::ArtifactDraft,
                openlife_core::task_runtime::CanonicalTaskItemKind::ReviewCheckpoint,
                openlife_core::task_runtime::CanonicalTaskItemKind::ArtifactMaterialized,
            ]
        );
        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(
            task.artifacts[0].status,
            openlife_core::task_runtime::CanonicalArtifactStatus::WaitingReview
        );
        assert!(!task.final_delivery_evidence_present);
        assert_eq!(
            task.artifacts[0].change.kind,
            openlife_core::agent::TaskArtifactChangeKind::Create
        );
        assert_eq!(
            task.artifacts[0].preview.content.as_deref(),
            Some("# Canonical report")
        );
        assert!(!task.artifacts[0].undo.available);
        assert_eq!(
            task.artifacts[0].verification.status,
            openlife_core::agent::TaskArtifactVerificationStatus::Pending
        );
        assert!(task
            .latest_result_preview
            .as_ref()
            .is_some_and(|preview| preview.final_delivery_ref.is_none()));

        std::fs::write(&artifact_path, content).unwrap();
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .confirm_artifact_materialized(
                "proposal-report-view",
                &artifact_path_text,
                &content_digest,
            )
            .unwrap();
        let mut incomplete_snapshot = state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_task_snapshots(100)
            .unwrap()
            .pop()
            .unwrap();
        incomplete_snapshot.items.retain(|item| {
            item.kind != openlife_core::task_runtime::CanonicalTaskItemKind::FinalResult
        });
        let mut incomplete_artifact_views = Vec::new();
        for artifact in &incomplete_snapshot.artifacts {
            incomplete_artifact_views
                .push(super::canonical_artifact_view(&state, &incomplete_snapshot, artifact).await);
        }
        assert_eq!(
            super::canonical_artifact_delivery_status(
                &incomplete_snapshot,
                &incomplete_artifact_views
            ),
            (
                openlife_core::agent::TaskLifecycleStatus::CompletedNeedsEvidence,
                openlife_core::agent::TaskTerminalDeliveryStatus::MissingFinalDeliveryEvidence,
                false,
            )
        );
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
        assert_eq!(task.items.len(), 6);
        assert_eq!(
            task.items.iter().map(|item| item.kind).collect::<Vec<_>>(),
            vec![
                openlife_core::task_runtime::CanonicalTaskItemKind::Instruction,
                openlife_core::task_runtime::CanonicalTaskItemKind::ArtifactDraft,
                openlife_core::task_runtime::CanonicalTaskItemKind::ReviewCheckpoint,
                openlife_core::task_runtime::CanonicalTaskItemKind::ArtifactMaterialized,
                openlife_core::task_runtime::CanonicalTaskItemKind::Verification,
                openlife_core::task_runtime::CanonicalTaskItemKind::FinalResult,
            ]
        );
        assert_eq!(
            task.artifacts[0].materialized_reference.as_deref(),
            Some(artifact_path_text.as_str())
        );
        assert_eq!(
            task.artifacts[0].verification.status,
            openlife_core::agent::TaskArtifactVerificationStatus::Verified
        );
        assert_eq!(
            task.artifacts[0].preview.content.as_deref(),
            Some("# Canonical report")
        );
        assert!(task.artifacts[0].undo.available);
        assert!(task.artifacts[0].undo.status.is_none());

        std::fs::write(&artifact_path, "# Tampered after delivery").unwrap();
        let drifted = get_tasks_view_model_with_state(&state).await.unwrap();
        let drifted = drifted.data.unwrap();
        let task = &drifted.items[0];
        assert_eq!(
            task.lifecycle_status,
            openlife_core::agent::TaskLifecycleStatus::CompletedNeedsEvidence
        );
        assert_eq!(
            task.artifacts[0].verification.status,
            openlife_core::agent::TaskArtifactVerificationStatus::Failed
        );
        assert!(task.artifacts[0].preview.content.is_none());
        assert!(!task.final_delivery_evidence_present);
    }
}
