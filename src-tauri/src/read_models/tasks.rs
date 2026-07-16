use crate::main_chat_task_controls::{
    get_main_chat_agent_task_detail_with_state, list_main_chat_agent_tasks_with_state,
    MainChatAgentTaskFilter, TaskDetail,
};
use crate::state::AppState;
use openlife_core::agent::{
    build_tasks_view_model, build_workspace_view_model, AgentRun, BackendEntityKind,
    BackendEntityRef, EvidenceRef, EvidenceSensitivity, EvidenceSource,
    ProviderPrivacyBoundarySummary, ReviewItem, ReviewItemDecisionStatus, TaskViewModelRunInput,
    TaskViewModelTaskInput, TasksViewModel, TasksViewModelBuildInput, ViewModelEnvelope,
    ViewModelStatus, ViewModelWarning, ViewModelWarningSeverity, WorkspaceViewModel,
};
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
    let mut warnings = Vec::new();
    let review_items = load_review_items(state, &mut warnings).await;
    let task_inputs = load_task_inputs(state, &review_items, &mut warnings).await;
    let run_inputs = load_run_inputs(state, &mut warnings).await;
    let model = build_tasks_view_model(TasksViewModelBuildInput {
        task_inputs,
        run_inputs,
        source_refs: vec![
            source_ref("main_chat_task_controls", "Main Chat task controls"),
            source_ref("agent_run_store", "AgentRun store"),
            source_ref("review_center_view_model", "ReviewCenterViewModel"),
        ],
        contract_limitations: vec![
            "Resume, retry, cancel, and refresh controls are request eligibility only; completion requires a refreshed backend read model.".into(),
            "Run-only rows without task-session final delivery evidence are not treated as completed task proof.".into(),
        ],
    });
    let status = if model.items.is_empty() {
        if warnings.is_empty() {
            ViewModelStatus::Empty
        } else {
            ViewModelStatus::Error
        }
    } else {
        ViewModelStatus::Ready
    };
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.warnings = warnings;
    Ok(envelope)
}

pub(crate) async fn get_workspace_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<WorkspaceViewModel>, String> {
    let tasks_envelope = get_tasks_view_model_with_state(state).await?;
    let tasks = tasks_envelope
        .data
        .as_ref()
        .ok_or_else(|| "TasksViewModel data unavailable for WorkspaceViewModel".to_string())?;
    let provider_envelope = get_provider_privacy_boundary_summary_with_state(state).await?;
    let provider_summary = provider_envelope
        .data
        .clone()
        .unwrap_or_else(ProviderPrivacyBoundarySummary::unknown);
    let model = build_workspace_view_model(
        tasks,
        provider_summary,
        vec![
            "WorkspaceViewModel consumes R5 ProviderPrivacyBoundarySummary; request controls still do not prove task completion.".into(),
            "External transmission remains possible or unknown unless backend route evidence explicitly proves otherwise.".into(),
        ],
    );
    let status = if model.recent_task_refs.is_empty() {
        ViewModelStatus::Empty
    } else {
        ViewModelStatus::Ready
    };
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.warnings = tasks_envelope.warnings;
    envelope.warnings.extend(provider_envelope.warnings);
    Ok(envelope)
}

async fn load_task_inputs(
    state: &Arc<AppState>,
    review_items: &[ReviewItem],
    warnings: &mut Vec<ViewModelWarning>,
) -> Vec<TaskViewModelTaskInput> {
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
            return Vec::new();
        }
    };

    let mut inputs = Vec::new();
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
        inputs.push(TaskViewModelTaskInput {
            task_session_id: summary.task_session_id,
            conversation_id: Some(summary.conversation_id),
            title: summary.title,
            strategy: Some(detail.task_session.selected_strategy),
            session_status: Some(detail.task_session.status),
            related_run_ids,
            final_delivery_present: detail.final_delivery.is_some(),
            final_delivery_status,
            pending_blockers,
            pending_review_item_refs,
            allowed_control_ids: detail.allowed_controls,
            retry_action_id,
            next_recommended_control: Some(detail.next_recommended_control),
            latest_result_preview: Some(summary.last_observation_preview),
            evidence_refs: vec![source_ref("main_chat_task_detail", "Main Chat task detail")],
            updated_at: Some(detail.task_session.updated_at),
        });
    }
    inputs
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
    match store.lock().await.list_runs(100, 0) {
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
) -> Vec<ReviewItem> {
    match get_review_center_view_model_with_state(state).await {
        Ok(envelope) => envelope.data.map(|model| model.items).unwrap_or_default(),
        Err(err) => {
            warnings.push(warning(
                "review_center_view_model_unavailable",
                format!("TasksViewModel could not load ReviewCenterViewModel: {err}"),
            ));
            Vec::new()
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
    use super::review_refs_for_task;
    use openlife_core::agent::{
        build_review_center_view_model, AgentProposal, ProposalSource, ProposalType,
        ReviewCenterBuildInput, RiskLevel,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

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
}
