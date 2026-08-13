use crate::state::AppState;
use openlife_core::agent::{
    build_tasks_view_model, build_workspace_view_model, BackendEntityKind, BackendEntityRef,
    EvidenceRef, EvidenceSensitivity, EvidenceSource, ProviderPrivacyBoundarySummary, ReviewItem,
    ReviewItemDecisionStatus, TaskArtifactChangeKind, TaskArtifactChangeViewModel,
    TaskArtifactPreviewStatus, TaskArtifactPreviewViewModel, TaskArtifactUndoViewModel,
    TaskArtifactVerificationStatus, TaskArtifactVerificationViewModel, TaskArtifactViewModel,
    TaskItemViewModel, TaskLifecycleStatus, TaskTerminalDeliveryStatus, TaskViewModelTaskInput,
    TasksViewModel, TasksViewModelBuildInput, ViewModelEnvelope, ViewModelStatus, ViewModelWarning,
    ViewModelWarningSeverity, WorkspaceActivityItem, WorkspaceViewModel,
    WorkspaceViewModelBuildInput,
};
use openlife_core::task_runtime::{
    CanonicalArtifactSnapshot, CanonicalArtifactStatus, CanonicalTaskSnapshot, CanonicalTaskStatus,
};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

use super::provider_privacy::get_provider_privacy_boundary_summary_with_state;
use super::review_center::get_review_center_view_model_with_state;

const TASK_ARTIFACT_PREVIEW_MAX_CHARS: usize = 12_000;
const TASK_ARTIFACT_READ_MAX_BYTES: u64 = 100 * 1024;

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
    let loaded_tasks = load_canonical_task_inputs(
        state,
        &review_items,
        review_projection_authoritative,
        &mut warnings,
    )
    .await;
    let model = build_tasks_view_model(TasksViewModelBuildInput {
        task_inputs: loaded_tasks.task_inputs,
        run_inputs: Vec::new(),
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

async fn load_canonical_task_inputs(
    state: &Arc<AppState>,
    review_items: &[ReviewItem],
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
    let snapshots = match store.lock().await.list_task_snapshots(100) {
        Ok(snapshots) => snapshots,
        Err(error) => {
            warnings.push(warning(
                "canonical_task_runtime_read_failed",
                format!("TasksViewModel could not load canonical Task snapshots: {error}"),
            ));
            return LoadedTaskInputs::default();
        }
    };
    let mut loaded = LoadedTaskInputs::default();
    for snapshot in snapshots {
        let (input, activity) = canonical_task_input(
            state,
            review_items,
            review_projection_authoritative,
            snapshot,
        )
        .await;
        loaded.activity_by_task.insert(
            input
                .canonical_task_id
                .clone()
                .unwrap_or_else(|| input.task_session_id.clone()),
            activity,
        );
        loaded.task_inputs.push(input);
    }
    loaded
}

async fn canonical_task_input(
    state: &Arc<AppState>,
    review_items: &[ReviewItem],
    review_projection_authoritative: bool,
    snapshot: CanonicalTaskSnapshot,
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
    let mut pending_review_item_refs = snapshot
        .artifacts
        .iter()
        .filter(|artifact| artifact.artifact.status == CanonicalArtifactStatus::WaitingReview)
        .filter_map(|artifact| artifact.artifact.proposal_id.as_ref())
        .map(|proposal_id| BackendEntityRef {
            id: proposal_id.clone(),
            kind: BackendEntityKind::ReviewItem,
            label: "Artifact review".into(),
            href: None,
        })
        .collect::<Vec<_>>();
    pending_review_item_refs.extend(review_refs_for_task(review_items, &snapshot.task.id));
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
        .or_else(|| Some(canonical_task_preview(&snapshot)));
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
    let title = match snapshot.task.task_kind.as_str() {
        "report" => "Generated report",
        "plan" => "Plan",
        _ => "Work task",
    };
    let attention_reason_codes = snapshot
        .attention
        .iter()
        .filter(|attention| attention.resolved_at.is_none())
        .map(|attention| attention.reason_code.clone())
        .collect::<Vec<_>>();
    let needs_attention = !attention_reason_codes.is_empty();
    (
        TaskViewModelTaskInput {
            task_session_id: snapshot.task.id.clone(),
            canonical_task_id: Some(snapshot.task.id.clone()),
            conversation_id: Some(snapshot.task.conversation_id),
            title: title.into(),
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
            needs_attention,
            attention_reason_codes,
            pending_review_item_refs,
            review_projection_authoritative,
            allowed_control_ids: match snapshot.task.status {
                CanonicalTaskStatus::Running => vec!["cancel".into()],
                CanonicalTaskStatus::Failed
                | CanonicalTaskStatus::Blocked
                | CanonicalTaskStatus::Cancelled
                | CanonicalTaskStatus::Interrupted => vec!["retry".into()],
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
            TaskLifecycleStatus::Blocked,
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
        let expected_id = openlife_core::task_runtime::report_final_result_item_id(
            &snapshot.task.id,
            &result.run_id,
        );
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
            let expected_verification_id = openlife_core::task_runtime::report_verification_item_id(
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
            TaskLifecycleStatus::Blocked,
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
        .artifact
        .proposal_id
        .as_ref()
        .map(|proposal_id| BackendEntityRef {
            id: proposal_id.clone(),
            kind: BackendEntityKind::ReviewItem,
            label: "Artifact Review checkpoint".into(),
            href: None,
        });
    let verification_item_present = snapshot
        .current_version
        .observed_content_digest
        .as_deref()
        .is_some_and(|observed| {
            let expected = openlife_core::task_runtime::report_verification_item_id(
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
    let presentation = artifact_presentation(state, snapshot, verification_item_present).await;
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

    if let Some(proposal_id) = snapshot.artifact.proposal_id.as_deref() {
        let proposal = if let Some(store) = state.proposal_store.as_ref() {
            store.lock().await.get_proposal(proposal_id).ok().flatten()
        } else {
            None
        };
        if let Some(proposal) = proposal {
            let after = &proposal.after;
            let target = after.get("path").and_then(serde_json::Value::as_str);
            let artifact_id = after.get("artifactId").and_then(serde_json::Value::as_str);
            let version = after
                .get("artifactVersion")
                .and_then(serde_json::Value::as_u64);
            let digest = after
                .get("contentDigest")
                .and_then(serde_json::Value::as_str);
            if artifact_id == Some(snapshot.artifact.id.as_str())
                && version == Some(snapshot.current_version.version)
                && digest == Some(snapshot.artifact.content_digest.as_str())
                && target.is_some_and(|value| {
                    openlife_core::agent::metadata_safe_text_digest(value).1
                        == snapshot.artifact.target_reference_digest
                })
            {
                change.kind = if after
                    .get("expected_target_absent")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                {
                    TaskArtifactChangeKind::Create
                } else {
                    TaskArtifactChangeKind::Replace
                };
                change.target_reference = target.map(str::to_string);
                change.expected_prior_digest = after
                    .get("expected_target_digest")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
            }
        }
    }

    if matches!(
        snapshot.artifact.status,
        CanonicalArtifactStatus::Draft | CanonicalArtifactStatus::WaitingReview
    ) {
        if let Some(proposal_id) = snapshot.artifact.proposal_id.as_deref() {
            let proposal = if let Some(store) = state.proposal_store.as_ref() {
                store.lock().await.get_proposal(proposal_id).ok().flatten()
            } else {
                None
            };
            if let Some(proposal) = proposal {
                let after = &proposal.after;
                let target = after.get("path").and_then(serde_json::Value::as_str);
                let content = after.get("content").and_then(serde_json::Value::as_str);
                let artifact_id = after.get("artifactId").and_then(serde_json::Value::as_str);
                let version = after
                    .get("artifactVersion")
                    .and_then(serde_json::Value::as_u64);
                let digest = after
                    .get("contentDigest")
                    .and_then(serde_json::Value::as_str);
                let exact = artifact_id == Some(snapshot.artifact.id.as_str())
                    && version == Some(snapshot.current_version.version)
                    && digest == Some(snapshot.artifact.content_digest.as_str())
                    && target.is_some_and(|value| {
                        openlife_core::agent::metadata_safe_text_digest(value).1
                            == snapshot.artifact.target_reference_digest
                    })
                    && content.is_some_and(|value| {
                        openlife_core::agent::metadata_safe_text_digest(value).1
                            == snapshot.artifact.content_digest
                    });
                if exact {
                    preview = bounded_artifact_preview(content.unwrap_or_default());
                } else {
                    preview = unavailable("artifact_proposal_binding_mismatch");
                }
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
        let safe_paths = state.config.lock().await.system.safe_paths.clone();
        match read_verified_artifact(path, &safe_paths) {
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

fn read_verified_artifact(path: &str, safe_paths: &[String]) -> Result<(String, String), String> {
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
    let content = String::from_utf8(bytes)
        .map_err(|_| "artifact_materialized_preview_not_utf8".to_string())?;
    Ok((digest, content))
}

fn canonical_task_preview(snapshot: &CanonicalTaskSnapshot) -> String {
    let total = snapshot.artifacts.len();
    let materialized = snapshot
        .artifacts
        .iter()
        .filter(|artifact| artifact.artifact.status == CanonicalArtifactStatus::Materialized)
        .count();
    match snapshot.task.status {
        CanonicalTaskStatus::WaitingReview => {
            format!("{total} artifact(s) are waiting for Review.")
        }
        CanonicalTaskStatus::Completed => {
            format!("{materialized} of {total} artifact(s) are materialized and verified.")
        }
        CanonicalTaskStatus::Blocked => "The task is blocked.".into(),
        CanonicalTaskStatus::EffectUnknown => {
            "The task effect is unknown and was not replayed.".into()
        }
        CanonicalTaskStatus::Failed => "The task failed before verified delivery.".into(),
        CanonicalTaskStatus::Cancelled => "The task was cancelled.".into(),
        CanonicalTaskStatus::Interrupted => "The task was interrupted and can be retried.".into(),
        CanonicalTaskStatus::Running => {
            format!("The task is running with {total} artifact draft(s).")
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
        get_tasks_view_model_with_state, review_refs_for_task, workspace_composition_status,
    };
    use openlife_core::agent::{
        build_review_center_view_model, AgentProposal, ProposalSource, ProposalType,
        ReviewCenterBuildInput, RiskLevel, ViewModelStatus,
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Arc;

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
        let task_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
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
                project_id: None,
                project_revision: None,
                scope_digest: None,
            })
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
        let mut proposal = openlife_core::agent::AgentProposal::new(
            openlife_core::agent::ProposalType::ExternalWriteAction,
            &format!("filesystem.{artifact_path_text}"),
            serde_json::json!({
                "path": artifact_path_text,
                "content": content,
                "contentDigest": content_digest,
                "expected_target_absent": true,
                "expected_target_digest": null,
                "artifactId": prepared.artifact_id,
                "artifactVersion": prepared.version,
            }),
            "test exact report preview",
            1.0,
            openlife_core::agent::RiskLevel::High,
            openlife_core::agent::ProposalSource::ChatConversation,
        );
        proposal.id = "proposal-report-view".into();
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
        assert_eq!(
            task.task_session_id.as_deref(),
            Some(prepared.task_id.as_str())
        );
        assert_eq!(
            task.lifecycle_status,
            openlife_core::agent::TaskLifecycleStatus::WaitingReview
        );
        assert_eq!(task.items.len(), 4);
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
