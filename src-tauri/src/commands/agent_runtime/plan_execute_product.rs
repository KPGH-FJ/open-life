use crate::main_chat_policy_runtime::build_chat_runtime_policy_packet;
use crate::AppState;
use openlife_core::agent::main_chat_runtime_contract::{
    ActionEvidence, BlockerEvidence, ObservationEvidence, PlanArtifactFactView,
    PlanArtifactRouteEvidence, PlanArtifactRunEvidence, PlanArtifactSourceEvidence,
    PlanArtifactStepView, PlanArtifactView, ProposalEvidence, StrategyEvidence,
};
use openlife_core::agent::{
    behavior_checks_for_packet, AgentExecutionBudget, AgentRun, AgentTask, AgentTaskKind,
    ContextSummary, LifeModelGovernor, PlanExecuteInput, PlanExecuteProductContract,
    PlanExecuteProductScenario, PlanExecuteService, PlanExecuteSession, PlanExecuteSessionStatus,
    PlanExecuteStepEdit, PlanExecuteStepExecutionResult, PlanStepStatus, ReasoningTrace,
    RedactionLevel, RiskLevel, RuntimeInput, RuntimeStrategyRegistry,
};
use openlife_core::layer::Layer;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlanExecuteSessionInput {
    #[serde(default)]
    pub scenario_id: Option<String>,
    #[serde(default)]
    pub source_chat_session_id: Option<String>,
    #[serde(default)]
    pub max_steps: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPlanExecuteSessionInput {
    pub session_id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPlanExecuteSessionsInput {
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecuteStepEditInput {
    pub step_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub action_kind: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub declared_write: Option<bool>,
    #[serde(default)]
    pub risk_level: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlanExecuteSessionDraftInput {
    pub session_id: String,
    #[serde(default)]
    pub base_revision: Option<u64>,
    #[serde(default)]
    pub steps: Vec<PlanExecuteStepEditInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizePlanExecuteSessionInput {
    pub session_id: String,
    #[serde(default)]
    pub base_revision: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelPlanExecuteSessionInput {
    pub session_id: String,
    #[serde(default)]
    pub base_revision: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlanExecuteSessionInput {
    pub session_id: String,
    #[serde(default)]
    pub base_revision: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutePlanExecuteStepInput {
    pub session_id: String,
    #[serde(default)]
    pub step_id: Option<String>,
    #[serde(default)]
    pub base_revision: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutePlanExecuteStepOutput {
    pub session: PlanExecuteSession,
    pub executed_step: PlanExecuteStepExecutionResult,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipPlanExecuteStepInput {
    pub session_id: String,
    pub step_id: String,
    pub base_revision: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipPlanExecuteStepOutput {
    pub session: PlanExecuteSession,
    pub skipped_step: PlanExecuteStepExecutionResult,
    pub metadata_safe_summary: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlanExecuteSessionOutput {
    pub session: PlanExecuteSession,
    pub summary: openlife_core::agent::PlanExecuteReviewSummary,
    pub metadata_safe_summary: Value,
}

pub(crate) struct PlanArtifactRuntimeEvidence<'a> {
    pub task_session_id: &'a str,
    pub run_id: Option<&'a str>,
    pub route: &'a StrategyEvidence,
    pub actions: &'a [ActionEvidence],
    pub observations: &'a [ObservationEvidence],
    pub proposals: &'a [ProposalEvidence],
    pub blockers: &'a [BlockerEvidence],
    pub final_delivery_id: Option<&'a str>,
}

#[tauri::command]
pub async fn create_plan_execute_session(
    input: CreatePlanExecuteSessionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanExecuteSession, String> {
    create_plan_execute_session_with_state(input, &state.inner().clone()).await
}

#[tauri::command]
pub async fn get_plan_execute_session(
    input: GetPlanExecuteSessionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<PlanExecuteSession>, String> {
    get_plan_execute_session_with_state(input, &state.inner().clone()).await
}

#[tauri::command]
pub async fn list_plan_execute_sessions(
    input: Option<ListPlanExecuteSessionsInput>,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<PlanExecuteSession>, String> {
    list_plan_execute_sessions_with_state(input.unwrap_or_default(), &state.inner().clone()).await
}

#[tauri::command]
pub async fn update_plan_execute_session_draft(
    input: UpdatePlanExecuteSessionDraftInput,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanExecuteSession, String> {
    update_plan_execute_session_draft_with_state(input, &state.inner().clone()).await
}

#[tauri::command]
pub async fn finalize_plan_execute_session(
    input: FinalizePlanExecuteSessionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanExecuteSession, String> {
    finalize_plan_execute_session_with_state(input, &state.inner().clone()).await
}

#[tauri::command]
pub async fn cancel_plan_execute_session(
    input: CancelPlanExecuteSessionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<PlanExecuteSession, String> {
    cancel_plan_execute_session_with_state(input, &state.inner().clone()).await
}

#[tauri::command]
pub async fn review_plan_execute_session(
    input: ReviewPlanExecuteSessionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ReviewPlanExecuteSessionOutput, String> {
    review_plan_execute_session_with_state(input, &state.inner().clone()).await
}

#[tauri::command]
pub async fn execute_plan_execute_step(
    input: ExecutePlanExecuteStepInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ExecutePlanExecuteStepOutput, String> {
    execute_plan_execute_step_with_state(input, &state.inner().clone()).await
}

#[tauri::command]
pub async fn skip_plan_execute_step(
    input: SkipPlanExecuteStepInput,
    state: State<'_, Arc<AppState>>,
) -> Result<SkipPlanExecuteStepOutput, String> {
    skip_plan_execute_step_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn create_plan_execute_session_with_state(
    input: CreatePlanExecuteSessionInput,
    state: &Arc<AppState>,
) -> Result<PlanExecuteSession, String> {
    create_plan_execute_session_with_source_run(input, state, None, None, None, Vec::new()).await
}

pub(crate) async fn create_plan_execute_session_for_main_chat_with_state(
    input: CreatePlanExecuteSessionInput,
    state: &Arc<AppState>,
    source_run_id: &str,
    execution_epoch: &crate::main_chat_cancellation::MainChatExecutionEpoch,
    task_text: &str,
    life_model_hints: Vec<openlife_core::agent::PlanExecuteLifeModelHint>,
) -> Result<PlanExecuteSession, String> {
    create_plan_execute_session_with_source_run(
        input,
        state,
        Some(source_run_id),
        Some(execution_epoch),
        Some(task_text),
        life_model_hints,
    )
    .await
}

async fn create_plan_execute_session_with_source_run(
    input: CreatePlanExecuteSessionInput,
    state: &Arc<AppState>,
    source_run_id: Option<&str>,
    execution_epoch: Option<&crate::main_chat_cancellation::MainChatExecutionEpoch>,
    task_text: Option<&str>,
    life_model_hints: Vec<openlife_core::agent::PlanExecuteLifeModelHint>,
) -> Result<PlanExecuteSession, String> {
    let scenario = PlanExecuteProductScenario::try_from_id(
        input.scenario_id.as_deref().unwrap_or("weekly_planning"),
    )
    .map_err(|report| format!("Plan-Execute scenario blocked: {}", report.reason_code))?;
    let contract = match scenario {
        PlanExecuteProductScenario::WeeklyPlanning => PlanExecuteProductContract::weekly_planning(),
    };
    let max_steps = input.max_steps.unwrap_or(contract.max_step_count);
    if max_steps == 0 || max_steps > contract.max_step_count {
        return Err("Plan-Execute maxSteps exceeds product contract".into());
    }
    let source_chat_session_id = input
        .source_chat_session_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "workspace_weekly_planning".into());

    let mut owned_plan_run = if source_run_id.is_some() {
        None
    } else {
        Some(new_plan_execute_product_run(&source_chat_session_id))
    };
    let run_id = source_run_id
        .map(str::to_string)
        .or_else(|| owned_plan_run.as_ref().map(|run| run.id.clone()))
        .ok_or_else(|| "Plan-Execute source AgentRun id missing".to_string())?;

    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().unwrap_or_else(|_| LifeModel::default())
    };
    let tools_prompt = {
        let registry = state.mcp_registry.lock().await;
        registry.tools_prompt()
    };
    let task_text = task_text
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Plan this week using confirmed context.");
    let task = AgentTask {
        kind: AgentTaskKind::Planning,
        session_id: source_chat_session_id.clone(),
        user_text: task_text.into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: task_text.into(),
        }],
        layer: Layer::L2,
    };
    let policy_packet = Some(build_chat_runtime_policy_packet(
        state,
        &task,
        &tools_prompt,
        Some(run_id.clone()),
    )?);
    let behavior_checks = policy_packet
        .as_ref()
        .map(behavior_checks_for_packet)
        .unwrap_or_default();
    let policy_selection_audit = policy_packet.as_ref().map(|packet| packet.audit.clone());
    let runtime_input = RuntimeInput::from_agent_task(
        task,
        life_model.clone(),
        None,
        tools_prompt,
        policy_packet,
        AgentExecutionBudget {
            max_steps: max_steps as u32,
            max_tool_calls: 0,
            timeout_seconds: 30,
            allow_cloud: false,
            allow_writes: false,
        },
    );
    let service = PlanExecuteService;
    let plan_input = PlanExecuteInput::from_runtime_input(
        runtime_input,
        "scenario=weekly_planning product=workspace",
        max_steps,
    )
    .with_life_model_hints(life_model_hints);
    let draft = service.draft_product_plan(&plan_input, scenario);
    let session = PlanExecuteSession::new_draft(
        Some(source_chat_session_id),
        Some(run_id.clone()),
        contract,
        draft,
    )
    .map_err(|e| e.to_string())?;

    if let Some(run) = owned_plan_run.as_mut() {
        run.task_id = session.session_id.clone();
        run.hs_selection_audit = policy_selection_audit;
        run.behavior_checks = behavior_checks;
        initialize_product_run_immutable_evidence(run, &session);
        create_product_run(state, run).await?;
    }

    let session_creation = {
        let store_arc = state
            .plan_execute_session_store
            .as_ref()
            .ok_or_else(|| "Plan-Execute session store not available".to_string())?;
        let store = store_arc.lock().await;
        let commit_permit = execution_epoch
            .map(|epoch| {
                epoch.begin_canonical_commit(
                    "plan_execute",
                    format!("session:{}", session.session_id),
                )
            })
            .transpose()
            .map_err(|rejection| format!("Plan-Execute session commit rejected: {rejection:?}"))?;
        let creation = store
            .create_session(&session)
            .map_err(|e| format!("failed to create Plan-Execute session: {e}"));
        match creation {
            Ok(()) => {
                if let Some(commit_permit) = commit_permit {
                    commit_permit.finish_committed();
                }
                Ok(())
            }
            Err(error) => {
                if let Some(commit_permit) = commit_permit {
                    commit_permit.finish_failed();
                }
                Err(error)
            }
        }
    };
    if let Err(error) = session_creation {
        if let Some(run) = owned_plan_run.as_ref() {
            if let Err(finalization_error) =
                crate::terminal_owner_write_gateway::fail_agent_run_from_owned_phase(
                    state,
                    &run.id,
                    crate::terminal_owner_write_gateway::AgentRunOwnedFailure::PlanSessionCreate,
                )
                .await
            {
                return Err(format!(
                    "{error}; Plan-Execute AgentRun failure projection is degraded: {finalization_error}"
                ));
            }
        }
        return Err(error);
    }
    if let Err(error) = append_plan_created_events(state, &session).await {
        if let Some(run) = owned_plan_run.as_ref() {
            if let Err(finalization_error) = crate::terminal_owner_write_gateway::fail_agent_run_from_owned_phase(
                state,
                &run.id,
                crate::terminal_owner_write_gateway::AgentRunOwnedFailure::PlanCreatedEventProjection,
            )
            .await
            {
                return Err(format!(
                    "{error}; Plan-Execute AgentRun failure projection is degraded: {finalization_error}"
                ));
            }
        }
        return Err(error);
    }

    if let Some(run) = owned_plan_run.as_ref() {
        crate::terminal_owner_write_gateway::project_agent_run_from_plan_execute_session(
            state,
            &run.id,
            &session.session_id,
        )
        .await?;
    }
    Ok(session)
}

pub(crate) async fn get_plan_execute_session_with_state(
    input: GetPlanExecuteSessionInput,
    state: &Arc<AppState>,
) -> Result<Option<PlanExecuteSession>, String> {
    let store_arc = state
        .plan_execute_session_store
        .as_ref()
        .ok_or_else(|| "Plan-Execute session store not available".to_string())?;
    let store = store_arc.lock().await;
    store
        .get_session(&input.session_id)
        .map_err(|e| format!("failed to get Plan-Execute session: {e}"))
}

pub(crate) async fn list_plan_execute_sessions_with_state(
    input: ListPlanExecuteSessionsInput,
    state: &Arc<AppState>,
) -> Result<Vec<PlanExecuteSession>, String> {
    let store_arc = state
        .plan_execute_session_store
        .as_ref()
        .ok_or_else(|| "Plan-Execute session store not available".to_string())?;
    let store = store_arc.lock().await;
    store
        .list_sessions(input.limit.unwrap_or(10).clamp(1, 50))
        .map_err(|e| format!("failed to list Plan-Execute sessions: {e}"))
}

pub(crate) async fn update_plan_execute_session_draft_with_state(
    input: UpdatePlanExecuteSessionDraftInput,
    state: &Arc<AppState>,
) -> Result<PlanExecuteSession, String> {
    let mut session = load_plan_execute_session(state, &input.session_id).await?;
    ensure_plan_revision_or_append_blocker(state, &session, input.base_revision).await?;
    let edited_step_ids = input
        .steps
        .iter()
        .map(|step| step.step_id.clone())
        .collect::<Vec<_>>();
    let edits = input
        .steps
        .into_iter()
        .map(plan_execute_step_edit_from_input)
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(base_revision) = input.base_revision {
        session
            .apply_draft_edits_at_revision(base_revision, edits)
            .map_err(|e| e.to_string())?;
    } else {
        session
            .apply_draft_edits(edits)
            .map_err(|e| e.to_string())?;
    }
    save_plan_execute_session(state, &session).await?;
    append_plan_updated_events(state, &session, &edited_step_ids).await?;
    update_existing_product_run_for_session(state, &session).await?;
    Ok(session)
}

pub(crate) async fn finalize_plan_execute_session_with_state(
    input: FinalizePlanExecuteSessionInput,
    state: &Arc<AppState>,
) -> Result<PlanExecuteSession, String> {
    let mut session = load_plan_execute_session(state, &input.session_id).await?;
    ensure_plan_revision_or_append_blocker(state, &session, input.base_revision).await?;
    if let Some(base_revision) = input.base_revision {
        session
            .finalize_at_revision(base_revision)
            .map_err(|e| e.to_string())?;
    } else {
        session.finalize().map_err(|e| e.to_string())?;
    }
    save_plan_execute_session(state, &session).await?;
    append_plan_confirmed_event(state, &session).await?;
    update_existing_product_run_for_session(state, &session).await?;
    Ok(session)
}

pub(crate) async fn cancel_plan_execute_session_with_state(
    input: CancelPlanExecuteSessionInput,
    state: &Arc<AppState>,
) -> Result<PlanExecuteSession, String> {
    let mut session = load_plan_execute_session(state, &input.session_id).await?;
    ensure_plan_revision_or_append_blocker(state, &session, input.base_revision).await?;
    let cancel_result = if let Some(base_revision) = input.base_revision {
        session
            .cancel_at_revision(base_revision)
            .map_err(|e| e.to_string())?
    } else {
        session.cancel().map_err(|e| e.to_string())?
    };
    save_plan_execute_session(state, &session).await?;
    append_plan_cancelled_events(state, &session, &cancel_result.cancelled_step_ids).await?;
    update_existing_product_run_for_session(state, &session).await?;
    Ok(session)
}

pub(crate) async fn review_plan_execute_session_with_state(
    input: ReviewPlanExecuteSessionInput,
    state: &Arc<AppState>,
) -> Result<ReviewPlanExecuteSessionOutput, String> {
    let mut session = load_plan_execute_session(state, &input.session_id).await?;
    ensure_plan_revision_or_append_blocker(state, &session, input.base_revision).await?;
    let summary = if let Some(base_revision) = input.base_revision {
        session
            .review_at_revision(base_revision)
            .map_err(|e| e.to_string())?
    } else {
        let base_revision = session.revision;
        session
            .review_at_revision(base_revision)
            .map_err(|e| e.to_string())?
    };
    save_plan_execute_session(state, &session).await?;
    append_plan_reviewed_event(state, &session, &summary).await?;
    update_existing_product_run_for_session(state, &session).await?;
    Ok(ReviewPlanExecuteSessionOutput {
        metadata_safe_summary: json!({
            "planExecuteProductVertical": true,
            "scenarioId": session.scenario.as_id(),
            "planSessionId": session.session_id,
            "planId": session.plan_id,
            "reviewId": summary.review_id,
            "revision": session.revision,
            "metadataSafe": true,
            "directLifeModelWrites": false,
            "memoryWrites": false,
            "externalWritesExecuted": false,
        }),
        session,
        summary,
    })
}

pub(crate) async fn execute_plan_execute_step_with_state(
    input: ExecutePlanExecuteStepInput,
    state: &Arc<AppState>,
) -> Result<ExecutePlanExecuteStepOutput, String> {
    let mut session = load_plan_execute_session(state, &input.session_id).await?;
    ensure_plan_revision_or_append_blocker(state, &session, input.base_revision).await?;
    let step_id = match input.step_id {
        Some(step_id) => {
            ensure_plan_step_or_append_blocker(state, &session, &step_id).await?;
            step_id
        }
        None => session
            .steps
            .iter()
            .find(|step| {
                !matches!(
                    step.status,
                    PlanStepStatus::Executed
                        | PlanStepStatus::RequiresProposal
                        | PlanStepStatus::Blocked
                        | PlanStepStatus::Cancelled
                ) && step.linked_proposal_id.is_none()
            })
            .map(|step| step.step_id.clone())
            .ok_or_else(|| "No executable Plan-Execute step remains".to_string())?,
    };
    let proposal_store_arc = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "Proposal store not available for Plan-Execute step".to_string())?;
    let proposal_store = proposal_store_arc.lock().await;
    let executed_step = if let Some(base_revision) = input.base_revision {
        session
            .execute_step_at_revision(&step_id, base_revision, &LifeModelGovernor, &proposal_store)
            .map_err(|e| e.to_string())?
    } else {
        session
            .execute_step(&step_id, &LifeModelGovernor, &proposal_store)
            .map_err(|e| e.to_string())?
    };
    drop(proposal_store);
    save_plan_execute_session(state, &session).await?;
    append_plan_step_execution_events(state, &session, &executed_step).await?;
    update_existing_product_run_for_session(state, &session).await?;
    Ok(ExecutePlanExecuteStepOutput {
        metadata_safe_summary: json!({
            "planExecuteProductVertical": true,
            "scenarioId": session.scenario.as_id(),
            "planSessionId": session.session_id,
            "executedStepId": executed_step.step_id,
            "linkedProposalId": executed_step.linked_proposal_id,
            "metadataSafe": true,
            "directLifeModelWrites": false,
            "externalWritesExecuted": false,
        }),
        session,
        executed_step,
    })
}

pub(crate) async fn skip_plan_execute_step_with_state(
    input: SkipPlanExecuteStepInput,
    state: &Arc<AppState>,
) -> Result<SkipPlanExecuteStepOutput, String> {
    let mut session = load_plan_execute_session(state, &input.session_id).await?;
    ensure_plan_revision_or_append_blocker(state, &session, Some(input.base_revision)).await?;
    ensure_plan_step_or_append_blocker(state, &session, &input.step_id).await?;
    let skipped_step = session
        .skip_step_at_revision(&input.step_id, input.base_revision, &input.reason)
        .map_err(|e| e.to_string())?;
    save_plan_execute_session(state, &session).await?;
    append_plan_step_skipped_events(state, &session, &skipped_step).await?;
    update_existing_product_run_for_session(state, &session).await?;
    Ok(SkipPlanExecuteStepOutput {
        metadata_safe_summary: json!({
            "planExecuteProductVertical": true,
            "scenarioId": session.scenario.as_id(),
            "planSessionId": session.session_id,
            "planId": session.plan_id,
            "skippedStepId": skipped_step.step_id,
            "revision": session.revision,
            "metadataSafe": true,
            "directLifeModelWrites": false,
            "externalWritesExecuted": false,
        }),
        session,
        skipped_step,
    })
}

pub(crate) fn build_plan_artifact_view(
    session: &PlanExecuteSession,
    runtime: PlanArtifactRuntimeEvidence<'_>,
) -> PlanArtifactView {
    let run_id = session
        .source_agent_run_id
        .as_deref()
        .or(runtime.run_id)
        .unwrap_or("unknown")
        .to_string();
    let source_evidence = source_tool_evidence(runtime.observations, runtime.actions);
    let steps = session
        .steps
        .iter()
        .map(|step| {
            let step_sources = source_evidence_for_ids(
                &source_evidence,
                &step
                    .linked_observation_ids
                    .iter()
                    .chain(step.evidence_ids.iter())
                    .cloned()
                    .collect::<Vec<_>>(),
            );
            PlanArtifactStepView {
                step_id: step.step_id.clone(),
                index: step.index,
                title: safe_plan_text(&step.title, 120),
                description: safe_plan_text(&step.description, 240),
                status: plan_execute_step_status_label(step.status).into(),
                kind: safe_plan_text(&step.kind, 40),
                evidence_ids: step.evidence_ids.clone(),
                source_tool_evidence: step_sources,
                controls: artifact_step_controls(session.status, step.status),
            }
        })
        .collect::<Vec<_>>();
    let assumptions = artifact_assumptions(session, &source_evidence);
    let unknowns = artifact_unknowns(&source_evidence);
    let controls = artifact_plan_controls(session.status);
    let summary = format!(
        "{} PlanExecute plan with {} steps, {} proposal-required step{}.",
        status_title(session.status),
        session.step_count,
        session.proposal_required_step_count,
        if session.proposal_required_step_count == 1 {
            ""
        } else {
            "s"
        }
    );
    let title = format!("{} plan", scenario_title(session.scenario.as_id()));
    let route_evidence = PlanArtifactRouteEvidence {
        strategy: runtime.route.strategy.as_str().into(),
        reason: safe_plan_text(&runtime.route.reason, 160),
        confidence: runtime.route.confidence,
        evidence_ids: vec![
            runtime.task_session_id.to_string(),
            run_id.clone(),
            session.session_id.clone(),
        ],
    };
    let run_evidence = PlanArtifactRunEvidence {
        task_session_id: runtime.task_session_id.to_string(),
        run_id: run_id.clone(),
        plan_session_id: session.session_id.clone(),
        action_ids: runtime
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect(),
        observation_ids: runtime
            .observations
            .iter()
            .map(|observation| observation.observation_id.clone())
            .collect(),
        proposal_ids: runtime
            .proposals
            .iter()
            .map(|proposal| proposal.proposal_id.clone())
            .collect(),
        blocker_ids: runtime
            .blockers
            .iter()
            .map(|blocker| blocker.blocker_id.clone())
            .collect(),
        final_delivery_id: runtime.final_delivery_id.map(str::to_string),
        metadata_safe: true,
    };
    let body = build_plan_artifact_body(PlanArtifactBodyInput {
        session,
        title: &title,
        summary: &summary,
        steps: &steps,
        assumptions: &assumptions,
        unknowns: &unknowns,
        source_evidence: &source_evidence,
        route_evidence: &route_evidence,
        run_evidence: &run_evidence,
    });

    PlanArtifactView {
        plan_id: session.plan_id.clone(),
        plan_session_id: session.session_id.clone(),
        task_session_id: runtime.task_session_id.to_string(),
        run_id,
        status: session.status.to_string(),
        title,
        summary,
        body,
        steps,
        assumptions,
        unknowns,
        controls,
        route_evidence,
        run_evidence,
    }
}

struct PlanArtifactBodyInput<'a> {
    session: &'a PlanExecuteSession,
    title: &'a str,
    summary: &'a str,
    steps: &'a [PlanArtifactStepView],
    assumptions: &'a [PlanArtifactFactView],
    unknowns: &'a [PlanArtifactFactView],
    source_evidence: &'a [PlanArtifactSourceEvidence],
    route_evidence: &'a PlanArtifactRouteEvidence,
    run_evidence: &'a PlanArtifactRunEvidence,
}

fn build_plan_artifact_body(input: PlanArtifactBodyInput<'_>) -> String {
    let mut lines = vec![
        format!("# {}", input.title),
        String::new(),
        input.summary.to_string(),
        String::new(),
        format!("Plan ID: {}", input.session.plan_id),
        format!("Plan session: {}", input.session.session_id),
        format!("Task session: {}", input.run_evidence.task_session_id),
        format!("Run: {}", input.run_evidence.run_id),
        format!("Status: {}", input.session.status),
        String::new(),
        "Steps".into(),
    ];
    for step in input.steps {
        let source_suffix = if step.source_tool_evidence.is_empty() {
            "source/tool evidence: none attached".to_string()
        } else {
            format!(
                "source/tool evidence: {}",
                step.source_tool_evidence
                    .iter()
                    .map(|source| source.evidence_id.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        lines.push(format!(
            "{}. {} - {} ({})",
            step.index, step.title, step.description, source_suffix
        ));
    }
    lines.push(String::new());
    lines.push("Assumptions".into());
    for assumption in input.assumptions {
        lines.push(format!("- {}: {}", assumption.label, assumption.detail));
    }
    lines.push(String::new());
    lines.push("Unknowns".into());
    for unknown in input.unknowns {
        lines.push(format!("- {}: {}", unknown.label, unknown.detail));
    }
    lines.push(String::new());
    lines.push("Source/tool evidence".into());
    if input.source_evidence.is_empty() {
        lines.push("- No source/tool evidence is attached to this plan artifact yet.".into());
    } else {
        for source in input.source_evidence {
            lines.push(format!(
                "- {} · {} · {}{}",
                source.evidence_id,
                source.source_kind,
                source.source_label,
                source
                    .tool_name
                    .as_ref()
                    .map(|tool| format!(" · tool {tool}"))
                    .unwrap_or_default()
            ));
        }
    }
    lines.push(String::new());
    lines.push("Route evidence".into());
    lines.push(format!(
        "- strategy: {}; reason: {}; evidence: {}",
        input.route_evidence.strategy,
        input.route_evidence.reason,
        input.route_evidence.evidence_ids.join(", ")
    ));
    lines.push(String::new());
    lines.push("Run evidence".into());
    lines.push(format!(
        "- actions: {}; observations: {}; proposals: {}; blockers: {}",
        count_or_none(input.run_evidence.action_ids.len()),
        count_or_none(input.run_evidence.observation_ids.len()),
        count_or_none(input.run_evidence.proposal_ids.len()),
        count_or_none(input.run_evidence.blocker_ids.len())
    ));
    lines.join("\n")
}

fn source_tool_evidence(
    observations: &[ObservationEvidence],
    actions: &[ActionEvidence],
) -> Vec<PlanArtifactSourceEvidence> {
    observations
        .iter()
        .map(|observation| {
            let tool_name = observation
                .read_execution
                .as_ref()
                .map(|read| read.kind.clone())
                .or_else(|| {
                    actions
                        .iter()
                        .find(|action| action.action_id == observation.action_id)
                        .map(|action| action.action_type.clone())
                });
            PlanArtifactSourceEvidence {
                evidence_id: observation.observation_id.clone(),
                source_kind: safe_plan_text(&observation.source_kind, 80),
                source_label: safe_plan_text(&observation.source_label, 120),
                tool_name: tool_name.map(|value| safe_plan_text(&value, 80)),
                preview: Some(safe_plan_text(&observation.preview, 240)),
            }
        })
        .collect()
}

fn source_evidence_for_ids(
    source_evidence: &[PlanArtifactSourceEvidence],
    evidence_ids: &[String],
) -> Vec<PlanArtifactSourceEvidence> {
    source_evidence
        .iter()
        .filter(|source| evidence_ids.iter().any(|id| id == &source.evidence_id))
        .cloned()
        .collect()
}

fn artifact_assumptions(
    session: &PlanExecuteSession,
    source_evidence: &[PlanArtifactSourceEvidence],
) -> Vec<PlanArtifactFactView> {
    let mut assumptions = vec![
        PlanArtifactFactView {
            label: "Draft only".into(),
            detail: "This plan is reviewable output, not accepted LifeModel or Memory truth."
                .into(),
            evidence_ids: vec![session.session_id.clone()],
            source_tool_evidence: Vec::new(),
        },
        PlanArtifactFactView {
            label: "Governed execution".into(),
            detail:
                "Write-like steps remain proposal or confirmation gated until a supported control is used."
                    .into(),
            evidence_ids: vec![session.session_id.clone()],
            source_tool_evidence: Vec::new(),
        },
    ];
    for (label, keywords) in realtime_fact_categories() {
        let evidence = matching_realtime_sources(source_evidence, keywords);
        if !evidence.is_empty() {
            assumptions.push(PlanArtifactFactView {
                label: format!("Source-backed {label} note"),
                detail: "Use only the attached source/tool evidence for this realtime planning constraint."
                    .into(),
                evidence_ids: evidence
                    .iter()
                    .map(|source| source.evidence_id.clone())
                    .collect(),
                source_tool_evidence: evidence,
            });
        }
    }
    assumptions
}

fn artifact_unknowns(source_evidence: &[PlanArtifactSourceEvidence]) -> Vec<PlanArtifactFactView> {
    realtime_fact_categories()
        .into_iter()
        .filter_map(|(label, keywords)| {
            let evidence = matching_realtime_sources(source_evidence, keywords);
            if evidence.is_empty() {
                Some(PlanArtifactFactView {
                    label: label.to_string(),
                    detail: match label {
                        "opening hours" => "No source/tool evidence is attached. Treat venue opening hours, closure days, and ticket rules as unknown until a governed read provides evidence.".into(),
                        "weather" => "No source/tool evidence is attached. Treat weather, temperature, and rain risk as unknown until a governed read provides evidence.".into(),
                        "transportation" => "No source/tool evidence is attached. Treat traffic, transit routes, ride time, and parking as unknown until a governed read provides evidence.".into(),
                        _ => "No source/tool evidence is attached for this realtime fact.".into(),
                    },
                    evidence_ids: Vec::new(),
                    source_tool_evidence: Vec::new(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn matching_realtime_sources(
    source_evidence: &[PlanArtifactSourceEvidence],
    keywords: &[&str],
) -> Vec<PlanArtifactSourceEvidence> {
    source_evidence
        .iter()
        .filter(|source| {
            let haystack = format!(
                "{} {} {}",
                source.source_kind,
                source.source_label,
                source.preview.as_deref().unwrap_or_default()
            )
            .to_lowercase();
            keywords.iter().any(|keyword| haystack.contains(keyword))
        })
        .cloned()
        .collect()
}

fn realtime_fact_categories() -> Vec<(&'static str, &'static [&'static str])> {
    vec![
        (
            "opening hours",
            &[
                "opening",
                "hours",
                "open hours",
                "closed",
                "closure",
                "开放",
                "闭馆",
            ],
        ),
        (
            "weather",
            &[
                "weather",
                "temperature",
                "rain",
                "forecast",
                "天气",
                "气温",
                "降雨",
            ],
        ),
        (
            "transportation",
            &[
                "traffic", "transit", "metro", "bus", "parking", "route", "交通", "地铁", "公交",
                "停车",
            ],
        ),
    ]
}

fn artifact_plan_controls(status: PlanExecuteSessionStatus) -> Vec<String> {
    match status {
        PlanExecuteSessionStatus::Draft => vec![
            "confirm_plan".into(),
            "cancel_task".into(),
            "open_trace".into(),
        ],
        PlanExecuteSessionStatus::Finalized | PlanExecuteSessionStatus::InProgress => vec![
            "execute_step".into(),
            "skip_step".into(),
            "cancel_task".into(),
            "open_trace".into(),
        ],
        PlanExecuteSessionStatus::Completed => vec!["review_plan".into(), "open_trace".into()],
        PlanExecuteSessionStatus::Cancelled => vec!["open_trace".into()],
    }
}

fn artifact_step_controls(
    session_status: PlanExecuteSessionStatus,
    step_status: PlanStepStatus,
) -> Vec<String> {
    match (session_status, step_status) {
        (PlanExecuteSessionStatus::Draft, PlanStepStatus::Planned) => vec!["skip_step".into()],
        (
            PlanExecuteSessionStatus::Finalized | PlanExecuteSessionStatus::InProgress,
            PlanStepStatus::Planned | PlanStepStatus::RequiresConfirmation,
        ) => vec!["execute_step".into(), "skip_step".into()],
        _ => Vec::new(),
    }
}

fn plan_execute_step_status_label(status: PlanStepStatus) -> &'static str {
    match status {
        PlanStepStatus::Planned => "planned",
        PlanStepStatus::Skipped => "skipped",
        PlanStepStatus::Blocked => "blocked",
        PlanStepStatus::RequiresProposal => "requires_proposal",
        PlanStepStatus::RequiresConfirmation => "requires_confirmation",
        PlanStepStatus::Executed => "executed",
        PlanStepStatus::Cancelled => "cancelled",
    }
}

fn scenario_title(scenario_id: &str) -> String {
    scenario_id
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn status_title(status: PlanExecuteSessionStatus) -> &'static str {
    match status {
        PlanExecuteSessionStatus::Draft => "Draft",
        PlanExecuteSessionStatus::Finalized => "Finalized",
        PlanExecuteSessionStatus::InProgress => "In-progress",
        PlanExecuteSessionStatus::Completed => "Completed",
        PlanExecuteSessionStatus::Cancelled => "Cancelled",
    }
}

fn count_or_none(count: usize) -> String {
    if count == 0 {
        "none".into()
    } else {
        count.to_string()
    }
}

fn safe_plan_text(value: &str, max_chars: usize) -> String {
    value
        .replace(|ch: char| ch.is_control(), " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

async fn ensure_plan_revision_or_append_blocker(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
    base_revision: Option<u64>,
) -> Result<(), String> {
    let Some(base_revision) = base_revision else {
        return Ok(());
    };
    if base_revision == session.revision {
        return Ok(());
    }
    let blocker_id = format!(
        "plan-blocker:{}:stale-revision:{}:{}",
        session.session_id, base_revision, session.revision
    );
    append_plan_runtime_event(
        state,
        session,
        "blocker.created",
        "blocker",
        &blocker_id,
        json!({
            "blockerId": blocker_id,
            "reasonCode": "stale_plan_revision",
            "planId": session.plan_id,
            "planSessionId": session.session_id,
            "expectedRevision": session.revision,
            "baseRevision": base_revision,
            "metadataSafe": true,
            "directLifeModelWrites": false,
            "externalWritesExecuted": false,
        }),
    )
    .await?;
    Err(format!(
        "Plan-Execute stale revision: expected {}, got {}",
        session.revision, base_revision
    ))
}

async fn ensure_plan_step_or_append_blocker(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
    step_id: &str,
) -> Result<(), String> {
    if session.steps.iter().any(|step| step.step_id == step_id) {
        return Ok(());
    }
    let blocker_id = format!(
        "plan-blocker:{}:invalid-step:{}",
        session.session_id, step_id
    );
    append_plan_runtime_event(
        state,
        session,
        "blocker.created",
        "blocker",
        &blocker_id,
        json!({
            "blockerId": blocker_id,
            "reasonCode": "invalid_plan_step",
            "planId": session.plan_id,
            "planSessionId": session.session_id,
            "stepId": step_id,
            "revision": session.revision,
            "metadataSafe": true,
            "directLifeModelWrites": false,
            "externalWritesExecuted": false,
        }),
    )
    .await?;
    Err("Plan-Execute step not found".into())
}

async fn append_plan_created_events(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
) -> Result<(), String> {
    append_plan_runtime_event(
        state,
        session,
        "plan.created",
        "plan",
        &session.plan_id,
        plan_event_payload(session),
    )
    .await?;
    for step in &session.steps {
        append_plan_runtime_event(
            state,
            session,
            "step.created",
            "step",
            &step.step_id,
            step_event_payload(session, step),
        )
        .await?;
    }
    Ok(())
}

async fn append_plan_updated_events(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
    edited_step_ids: &[String],
) -> Result<(), String> {
    append_plan_runtime_event(
        state,
        session,
        "plan.updated",
        "plan",
        &session.plan_id,
        plan_event_payload(session),
    )
    .await?;
    for step_id in edited_step_ids {
        if let Some(step) = session.steps.iter().find(|step| &step.step_id == step_id) {
            append_plan_runtime_event(
                state,
                session,
                "step.updated",
                "step",
                &step.step_id,
                step_event_payload(session, step),
            )
            .await?;
        }
    }
    Ok(())
}

async fn append_plan_confirmed_event(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
) -> Result<(), String> {
    append_plan_runtime_event(
        state,
        session,
        "plan.confirmed",
        "plan",
        &session.plan_id,
        plan_event_payload(session),
    )
    .await
    .map(|_| ())
}

async fn append_plan_cancelled_events(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
    cancelled_step_ids: &[String],
) -> Result<(), String> {
    append_plan_runtime_event(
        state,
        session,
        "plan.updated",
        "plan",
        &session.plan_id,
        plan_event_payload(session),
    )
    .await?;
    for step_id in cancelled_step_ids {
        if let Some(step) = session.steps.iter().find(|step| &step.step_id == step_id) {
            append_plan_runtime_event(
                state,
                session,
                "step.updated",
                "step",
                &step.step_id,
                step_event_payload(session, step),
            )
            .await?;
            append_plan_runtime_event(
                state,
                session,
                "step.cancelled",
                "step",
                &step.step_id,
                json!({
                    "planId": session.plan_id,
                    "planSessionId": session.session_id,
                    "stepId": step.step_id,
                    "revision": step.revision,
                    "basePlanRevision": step.base_plan_revision,
                    "status": "cancelled",
                    "reasonCode": step.status_reason,
                    "evidenceIds": step.evidence_ids,
                    "metadataSafe": true,
                    "directLifeModelWrites": false,
                    "memoryWrites": false,
                    "externalWritesExecuted": false,
                }),
            )
            .await?;
        }
    }
    Ok(())
}

async fn append_plan_step_execution_events(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
    step_result: &PlanExecuteStepExecutionResult,
) -> Result<(), String> {
    append_step_updated_event(state, session, step_result).await?;
    for action_id in &step_result.linked_action_ids {
        append_plan_runtime_event(
            state,
            session,
            "action.queued",
            "action",
            action_id,
            json!({
                "actionId": action_id,
                "planId": step_result.plan_id,
                "stepId": step_result.step_id,
                "revision": step_result.revision,
                "actionType": "plan_execute.read_only",
                "status": "queued",
                "metadataSafe": true,
                "directWritesExecuted": false,
            }),
        )
        .await?;
        append_plan_runtime_event(
            state,
            session,
            "action.completed",
            "action",
            action_id,
            json!({
                "actionId": action_id,
                "status": "succeeded",
                "planId": step_result.plan_id,
                "stepId": step_result.step_id,
                "revision": step_result.revision,
                "observationIds": step_result.linked_observation_ids,
                "metadataSafe": true,
                "directWritesExecuted": false,
            }),
        )
        .await?;
    }
    for observation_id in &step_result.linked_observation_ids {
        append_plan_runtime_event(
            state,
            session,
            "observation.created",
            "observation",
            observation_id,
            json!({
                "observationId": observation_id,
                "planId": step_result.plan_id,
                "stepId": step_result.step_id,
                "revision": step_result.revision,
                "preview": step_result.observation_summary,
                "metadataSafe": true,
                "directWritesExecuted": false,
            }),
        )
        .await?;
    }
    for proposal_id in &step_result.linked_proposal_ids {
        append_plan_runtime_event(
            state,
            session,
            "proposal.created",
            "proposal",
            proposal_id,
            json!({
                "proposalId": proposal_id,
                "planId": step_result.plan_id,
                "stepId": step_result.step_id,
                "revision": step_result.revision,
                "metadataSafe": true,
                "directWritesExecuted": false,
            }),
        )
        .await?;
    }
    for blocker_id in &step_result.blocker_ids {
        append_plan_runtime_event(
            state,
            session,
            "blocker.created",
            "blocker",
            blocker_id,
            json!({
                "blockerId": blocker_id,
                "planId": step_result.plan_id,
                "stepId": step_result.step_id,
                "revision": step_result.revision,
                "reasonCode": step_result.status_reason,
                "metadataSafe": true,
                "directWritesExecuted": false,
            }),
        )
        .await?;
    }
    Ok(())
}

async fn append_plan_step_skipped_events(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
    step_result: &PlanExecuteStepExecutionResult,
) -> Result<(), String> {
    append_step_updated_event(state, session, step_result).await?;
    append_plan_runtime_event(
        state,
        session,
        "step.skipped",
        "step",
        &step_result.step_id,
        json!({
            "planId": step_result.plan_id,
            "stepId": step_result.step_id,
            "revision": step_result.revision,
            "basePlanRevision": step_result.base_plan_revision,
            "status": "skipped",
            "skipReason": step_result.skip_reason,
            "evidenceIds": step_result.evidence_ids,
            "metadataSafe": true,
            "directWritesExecuted": false,
        }),
    )
    .await
    .map(|_| ())
}

async fn append_plan_reviewed_event(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
    summary: &openlife_core::agent::PlanExecuteReviewSummary,
) -> Result<(), String> {
    append_plan_runtime_event(
        state,
        session,
        "plan.reviewed",
        "plan_review",
        &summary.review_id,
        json!({
            "reviewId": summary.review_id,
            "planId": summary.plan_id,
            "planSessionId": summary.plan_session_id,
            "planStatus": summary.plan_status,
            "basePlanRevision": summary.base_plan_revision,
            "completedStepCount": summary.completed_steps.len(),
            "skippedStepCount": summary.skipped_steps.len(),
            "blockedStepCount": summary.blocked_steps.len(),
            "proposalCreatedCount": summary.proposals_created.len(),
            "observationUsedCount": summary.observations_used.len(),
            "unresolvedCount": summary.unresolved.len(),
            "recommendedNextActionCount": summary.recommended_next_action.len(),
            "completionClaimed": summary.completion_claimed,
            "metadataSafe": true,
            "directLifeModelWrites": false,
            "memoryWrites": false,
            "externalWritesExecuted": false,
        }),
    )
    .await
    .map(|_| ())
}

async fn append_step_updated_event(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
    step_result: &PlanExecuteStepExecutionResult,
) -> Result<(), String> {
    append_plan_runtime_event(
        state,
        session,
        "step.updated",
        "step",
        &step_result.step_id,
        json!({
            "planId": step_result.plan_id,
            "stepId": step_result.step_id,
            "status": format!("{:?}", step_result.step_status).to_ascii_lowercase(),
            "revision": step_result.revision,
            "basePlanRevision": step_result.base_plan_revision,
            "linkedActionIds": step_result.linked_action_ids,
            "linkedObservationIds": step_result.linked_observation_ids,
            "linkedProposalIds": step_result.linked_proposal_ids,
            "blockerIds": step_result.blocker_ids,
            "linkedFinalDeliveryIds": step_result.linked_final_delivery_ids,
            "skipReasonPresent": step_result.skip_reason.is_some(),
            "policyDecisionId": step_result.policy_decision_id,
            "evidenceIds": step_result.evidence_ids,
            "metadataSafe": true,
            "directWritesExecuted": false,
        }),
    )
    .await
    .map(|_| ())
}

async fn append_plan_runtime_event(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
    event_type: &str,
    object_type: &str,
    object_id: &str,
    mut payload: Value,
) -> Result<crate::main_chat_event_stream::MainChatAgentDurableEvent, String> {
    let run_id = session
        .source_agent_run_id
        .as_deref()
        .ok_or_else(|| "Plan-Execute source AgentRun id missing".to_string())?;
    let task_session_id = load_plan_execute_source_task_id(state, run_id).await?;
    if let Some(object) = payload.as_object_mut() {
        object.insert("taskSessionId".into(), serde_json::json!(task_session_id));
        object.insert(
            "planSessionId".into(),
            serde_json::json!(session.session_id),
        );
        object.insert(
            "childWorkflowProvenance".into(),
            serde_json::json!({
                "kind": "plan_execute_session",
                "id": session.session_id,
                "sourceRunId": run_id,
                "eventTaskBoundToSourceRun": true,
            }),
        );
    }
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
        state,
        task_session_id,
        run_id.to_string(),
        event_type,
        object_type,
        object_id,
        "plan_runtime",
        payload,
    )
    .await
}

async fn load_plan_execute_source_task_id(
    state: &Arc<AppState>,
    run_id: &str,
) -> Result<String, String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for Plan-Execute event".to_string())?;
    let store = store_arc.lock().await;
    crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store.get_run(run_id).map_err(|error| error.to_string()),
    )
    .map_err(|error| format!("load Plan-Execute source AgentRun failed: {error}"))?
    .map(|run| run.task_id)
    .ok_or_else(|| "Plan-Execute source AgentRun missing".to_string())
}

fn plan_event_payload(session: &PlanExecuteSession) -> Value {
    json!({
        "planId": session.plan_id,
        "planSessionId": session.session_id,
        "runId": session.source_agent_run_id,
        "status": session.status.to_string(),
        "revision": session.revision,
        "revisionId": session.revision_id,
        "goal": session.metadata_safe_objective,
        "confirmedAt": session.confirmed_at,
        "reviewId": session.review_id,
        "reviewSummaryPresent": session.review_summary.is_some(),
        "sourceEvidenceIds": session.source_evidence_ids,
        "supersededByPlanId": session.superseded_by_plan_id,
        "stepIds": session.steps.iter().map(|step| step.step_id.clone()).collect::<Vec<_>>(),
        "metadataSafe": true,
        "directLifeModelWrites": false,
        "memoryWrites": false,
        "externalWritesExecuted": false,
    })
}

fn step_event_payload(
    session: &PlanExecuteSession,
    step: &openlife_core::agent::PlanExecuteStepRecord,
) -> Value {
    json!({
        "planId": session.plan_id,
        "planSessionId": session.session_id,
        "stepId": step.step_id,
        "index": step.index,
        "title": step.title,
        "description": step.description,
        "kind": step.kind,
        "status": format!("{:?}", step.status).to_ascii_lowercase(),
        "revision": step.revision,
        "basePlanRevision": step.base_plan_revision,
        "linkedActionIds": step.linked_action_ids,
        "linkedObservationIds": step.linked_observation_ids,
        "linkedProposalIds": step.linked_proposal_ids,
        "blockerIds": step.blocker_ids,
        "linkedFinalDeliveryIds": step.linked_final_delivery_ids,
        "skipReason": step.skip_reason,
        "policyDecisionId": step.policy_decision_id,
        "reason": step.status_reason,
        "evidenceIds": step.evidence_ids,
        "metadataSafe": true,
        "directLifeModelWrites": false,
        "externalWritesExecuted": false,
        "memoryWrites": false,
    })
}

async fn load_plan_execute_session(
    state: &Arc<AppState>,
    session_id: &str,
) -> Result<PlanExecuteSession, String> {
    let store_arc = state
        .plan_execute_session_store
        .as_ref()
        .ok_or_else(|| "Plan-Execute session store not available".to_string())?;
    let store = store_arc.lock().await;
    store
        .get_session(session_id)
        .map_err(|e| format!("failed to load Plan-Execute session: {e}"))?
        .ok_or_else(|| "Plan-Execute session not found".to_string())
}

async fn save_plan_execute_session(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
) -> Result<(), String> {
    let store_arc = state
        .plan_execute_session_store
        .as_ref()
        .ok_or_else(|| "Plan-Execute session store not available".to_string())?;
    let store = store_arc.lock().await;
    store
        .update_session(session)
        .map_err(|e| format!("failed to update Plan-Execute session: {e}"))
}

fn plan_execute_step_edit_from_input(
    input: PlanExecuteStepEditInput,
) -> Result<PlanExecuteStepEdit, String> {
    Ok(PlanExecuteStepEdit {
        step_id: input.step_id,
        title: input.title,
        intent: input.intent,
        action_kind: input.action_kind,
        tool_name: input.tool_name.map(Some),
        declared_write: input.declared_write,
        risk_level: input
            .risk_level
            .as_deref()
            .map(parse_risk_level)
            .transpose()?,
    })
}

fn parse_risk_level(value: &str) -> Result<RiskLevel, String> {
    match value {
        "low" => Ok(RiskLevel::Low),
        "medium" => Ok(RiskLevel::Medium),
        "high" => Ok(RiskLevel::High),
        "critical" => Ok(RiskLevel::Critical),
        _ => Err("Unsupported Plan-Execute riskLevel".into()),
    }
}

fn new_plan_execute_product_run(session_id: &str) -> AgentRun {
    let mut run = AgentRun::new_chat_run(session_id, "");
    run.kind = AgentTaskKind::Planning;
    run.user_input = None;
    run.reasoning_strategy = Some("plan_execute".into());
    run.output_preview = Some("Weekly plan draft started".into());
    run.context_summary = Some(plan_execute_context_summary(false));
    run
}

fn initialize_product_run_immutable_evidence(run: &mut AgentRun, session: &PlanExecuteSession) {
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(plan_execute_trace_metadata(session)),
        output: Some("plan_execute_product".into()),
        stable_steps: vec![
            "weekly_plan_session".into(),
            "review_finalize_gate".into(),
            "proposal_first_step_execution".into(),
            "metadata_safe_trace".into(),
        ],
        ..ReasoningTrace::default()
    });
}

async fn create_product_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    crate::terminal_owner_write_gateway::create_agent_run(state, run)
        .await
        .map_err(|error| format!("failed to create Plan-Execute AgentRun: {error}"))
}

async fn update_existing_product_run_for_session(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
) -> Result<(), String> {
    let Some(run_id) = session.source_agent_run_id.as_deref() else {
        return Ok(());
    };
    crate::terminal_owner_write_gateway::project_agent_run_from_plan_execute_session(
        state,
        run_id,
        &session.session_id,
    )
    .await
    .map_err(|error| format!("failed to update Plan-Execute AgentRun: {error}"))
}

fn plan_execute_context_summary(life_model_empty: bool) -> ContextSummary {
    ContextSummary {
        life_model_empty,
        included_life_model_sections: vec![
            "goal_priority".into(),
            "energy_current_state".into(),
            "planning_intensity".into(),
            "privacy_model_route".into(),
            "proposal_boundaries".into(),
        ],
        memory_hit_count: 0,
        memory_sources: Vec::new(),
        used_tools_prompt: true,
        redaction_applied: true,
        redaction_level: RedactionLevel::Strict,
    }
}

fn plan_execute_trace_metadata(session: &PlanExecuteSession) -> Value {
    let mut planned = 0;
    let mut executed = 0;
    let mut proposal_required = 0;
    let mut blocked = 0;
    for step in &session.steps {
        match step.status {
            PlanStepStatus::Planned => planned += 1,
            PlanStepStatus::Executed => executed += 1,
            PlanStepStatus::RequiresProposal => proposal_required += 1,
            PlanStepStatus::Blocked => blocked += 1,
            PlanStepStatus::Skipped
            | PlanStepStatus::RequiresConfirmation
            | PlanStepStatus::Cancelled => {}
        }
    }
    json!({
        "runtimeStrategyTraceKind": "plan_execute_product",
        "planExecuteProductVertical": true,
        "scenarioId": session.scenario.as_id(),
        "planSessionId": session.session_id,
        "strategyKind": "plan_execute",
        "selectedStrategyKind": "plan_execute",
        "payloadKind": "plan_execute",
        "strategyDescriptorId": "plan_execute",
        "strategyCapabilityIds": ["planning.plan_execute", "proposal_first_steps", "metadata_safe_trace"],
        "selectionReasonCode": "weekly_planning_product",
        "governanceDecisionKind": plan_execute_trace_governance_decision_kind(proposal_required, blocked),
        "registryReady": RuntimeStrategyRegistry::fixed_readiness_report().ready,
        "status": session.status.to_string(),
        "sourceAgentRunId": session.source_agent_run_id,
        "sourceChatSessionId": session.source_chat_session_id,
        "stepCount": session.step_count,
        "stepStatusCounts": {
            "planned": planned,
            "executed": executed,
            "requiresProposal": proposal_required,
            "blocked": blocked,
        },
        "generatedProposalIds": session.linked_proposal_ids,
        "generatedProposalCount": session.linked_proposal_ids.len(),
        "governanceDecisionCounts": {
            "allow": executed,
            "requireProposal": proposal_required,
            "block": blocked,
        },
        "warningCount": session.warnings.len(),
        "metadataSafe": true,
        "defaultChatUnchanged": true,
        "sideEffectBudget": plan_execute_trace_side_effect_budget(),
        "rawPromptStored": false,
        "rawWeeklyPlanProseStored": false,
        "rawLifeModelStored": false,
        "rawMemoryStored": false,
        "rawToolPayloadStored": false,
        "rawProposalPayloadStored": false,
        "directLifeModelWrites": false,
        "externalWritesExecuted": false,
    })
}

fn plan_execute_trace_governance_decision_kind(
    proposal_required: usize,
    blocked: usize,
) -> &'static str {
    if blocked > 0 {
        "block"
    } else if proposal_required > 0 {
        "require_proposal"
    } else {
        "allow"
    }
}

fn plan_execute_trace_side_effect_budget() -> Value {
    json!({
        "runtimeCalls": 0,
        "modelCalls": 0,
        "toolCalls": 0,
        "storeWrites": 0,
        "proposalWrites": 0,
        "memoryWrites": 0,
        "lifeModelWrites": 0,
        "mcpAuditWrites": 0,
        "externalWrites": 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{
        AgentRunStatus, PlanExecuteSessionStatus, PlanStepStatus, ProposalSource,
    };

    fn install_release_like_persistence_coordinator(state: &mut Arc<AppState>) {
        let coordinator = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        coordinator.seal();
        coordinator
            .require_effects_allowed()
            .expect("complete sealed manifest enables canonical writes");
        Arc::get_mut(state)
            .expect("test state must have one outer owner")
            .persistence_coordinator = coordinator;
    }

    #[tokio::test]
    async fn plan_execute_source_preflight_read_failure_degrades_before_future_effects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("plan-source-agent-run-failure.db");
        let store = openlife_core::agent::AgentRunStore::new(&path).unwrap();
        let run = new_plan_execute_product_run("plan-source-read-failure");
        let run_id = run.id.clone();
        store.create_run(&run).unwrap();
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(store)));
        install_release_like_persistence_coordinator(&mut state);

        let missing = load_plan_execute_source_task_id(&state, "missing-plan-source")
            .await
            .unwrap_err();
        assert_eq!(missing, "Plan-Execute source AgentRun missing");
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadWrite
        );

        let fault = rusqlite::Connection::open(&path).unwrap();
        fault.execute_batch("DROP TABLE agent_runs;").unwrap();
        drop(fault);
        let error = load_plan_execute_source_task_id(&state, &run_id)
            .await
            .expect_err("Plan-Execute source read must fail closed on durable corruption");
        assert!(error.to_ascii_lowercase().contains("no such table"));
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
    async fn late_degradation_blocks_standalone_plan_execute_agent_run_create() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        install_release_like_persistence_coordinator(&mut state);
        let run = new_plan_execute_product_run("late-degraded-plan-execute-create");
        let (reached, release) =
            crate::terminal_owner_write_gateway::install_agent_run_lifecycle_commit_test_barrier(
                &run.id,
            );
        let operation_state = Arc::clone(&state);
        let operation_run = run.clone();
        let operation =
            tokio::spawn(async move { create_product_run(&operation_state, &operation_run).await });

        reached
            .await
            .expect("PlanExecute create reached the post-precheck commit barrier");
        state
            .persistence_coordinator
            .degrade_globally("injected_after_plan_execute_create_precheck");
        release.send(()).unwrap();

        let error = operation.await.unwrap().unwrap_err();
        assert!(
            error.contains(crate::persistence_coordinator::PERSISTENCE_ADMISSION_INVALIDATED),
            "late degradation returned the wrong PlanExecute create error: {error}"
        );
        assert!(state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run_including_deleted(&run.id)
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn plan_execute_create_command_stores_draft_session_and_trace_without_proposals() {
        let state = crate::test_utils::test_app_state();

        let session = create_plan_execute_session_with_state(
            CreatePlanExecuteSessionInput {
                scenario_id: Some("weekly_planning".into()),
                source_chat_session_id: Some("chat-weekly".into()),
                max_steps: Some(5),
            },
            &state,
        )
        .await
        .unwrap();

        assert_eq!(session.status, PlanExecuteSessionStatus::Draft);
        assert_eq!(
            session.source_chat_session_id.as_deref(),
            Some("chat-weekly")
        );
        assert!(session.source_agent_run_id.is_some());
        assert!(session.step_count >= 2);

        let plan_store = state
            .plan_execute_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await;
        assert!(plan_store
            .get_session(&session.session_id)
            .unwrap()
            .is_some());
        drop(plan_store);

        let proposal_store = state.proposal_store.as_ref().unwrap().lock().await;
        assert_eq!(proposal_store.list_pending_proposals(10).unwrap().len(), 0);
        drop(proposal_store);

        let agent_run_store = state.agent_run_store.as_ref().unwrap().lock().await;
        let run = agent_run_store
            .get_run(session.source_agent_run_id.as_deref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(run.reasoning_strategy.as_deref(), Some("plan_execute"));
        assert!(
            run.reasoning_trace.is_none(),
            "AgentRun persists a digest, not a second copy of reasoning payload"
        );
        assert!(run
            .reasoning_trace_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("hmac-sha256:")));

        // Counterfactual: lifecycle projection may reuse the create-time
        // receipt, but it may never replace Plan-Execute immutable evidence.
        let mut tampered = run.clone();
        tampered.reasoning_trace = Some(ReasoningTrace {
            strategy_result: Some(json!({ "counterfactual": "late_trace_rewrite" })),
            ..ReasoningTrace::default()
        });
        tampered.reasoning_trace_digest = None;
        let error = agent_run_store.update_run(&tampered).unwrap_err();
        assert!(error
            .to_string()
            .contains("agent_run_immutable_evidence_update_conflict"));
    }

    #[tokio::test]
    async fn plan_execute_lifecycle_commands_edit_finalize_and_fail_closed() {
        let state = crate::test_utils::test_app_state();
        let session = create_plan_execute_session_with_state(
            CreatePlanExecuteSessionInput {
                scenario_id: Some("weekly_planning".into()),
                source_chat_session_id: None,
                max_steps: Some(5),
            },
            &state,
        )
        .await
        .unwrap();
        let first_step_id = session.steps[0].step_id.clone();

        let execute_error = execute_plan_execute_step_with_state(
            ExecutePlanExecuteStepInput {
                session_id: session.session_id.clone(),
                step_id: Some(first_step_id.clone()),
                base_revision: None,
            },
            &state,
        )
        .await
        .unwrap_err();
        assert!(execute_error.contains("finalized"));

        let edited = update_plan_execute_session_draft_with_state(
            UpdatePlanExecuteSessionDraftInput {
                session_id: session.session_id.clone(),
                base_revision: None,
                steps: vec![PlanExecuteStepEditInput {
                    step_id: first_step_id.clone(),
                    title: Some("Review priorities before planning".into()),
                    intent: Some("read_only_reasoning".into()),
                    action_kind: Some("reason".into()),
                    tool_name: None,
                    declared_write: Some(false),
                    risk_level: Some("low".into()),
                }],
            },
            &state,
        )
        .await
        .unwrap();
        assert_eq!(edited.steps[0].title, "Review priorities before planning");

        let finalized = finalize_plan_execute_session_with_state(
            FinalizePlanExecuteSessionInput {
                session_id: session.session_id.clone(),
                base_revision: None,
            },
            &state,
        )
        .await
        .unwrap();
        assert_eq!(finalized.status, PlanExecuteSessionStatus::Finalized);

        let edit_error = update_plan_execute_session_draft_with_state(
            UpdatePlanExecuteSessionDraftInput {
                session_id: session.session_id,
                base_revision: None,
                steps: vec![PlanExecuteStepEditInput {
                    step_id: first_step_id,
                    title: Some("Too late".into()),
                    intent: None,
                    action_kind: None,
                    tool_name: None,
                    declared_write: None,
                    risk_level: None,
                }],
            },
            &state,
        )
        .await
        .unwrap_err();
        assert!(edit_error.contains("not editable"));
    }

    #[tokio::test]
    async fn plan_execute_step_command_records_read_observation_and_proposal_idempotently() {
        let state = crate::test_utils::test_app_state();
        let session = create_plan_execute_session_with_state(
            CreatePlanExecuteSessionInput {
                scenario_id: Some("weekly_planning".into()),
                source_chat_session_id: Some("chat-weekly".into()),
                max_steps: Some(5),
            },
            &state,
        )
        .await
        .unwrap();
        let session = finalize_plan_execute_session_with_state(
            FinalizePlanExecuteSessionInput {
                session_id: session.session_id.clone(),
                base_revision: None,
            },
            &state,
        )
        .await
        .unwrap();
        let read_step_id = session
            .steps
            .iter()
            .find(|step| !step.declared_write)
            .unwrap()
            .step_id
            .clone();
        let write_step_id = session
            .steps
            .iter()
            .find(|step| step.declared_write)
            .unwrap()
            .step_id
            .clone();

        let read_result = execute_plan_execute_step_with_state(
            ExecutePlanExecuteStepInput {
                session_id: session.session_id.clone(),
                step_id: Some(read_step_id),
                base_revision: None,
            },
            &state,
        )
        .await
        .unwrap();
        assert_eq!(
            read_result.executed_step.step_status,
            PlanStepStatus::Executed
        );
        assert!(read_result.executed_step.observation_summary.is_some());
        assert!(read_result.executed_step.linked_proposal_id.is_none());

        let write_result = execute_plan_execute_step_with_state(
            ExecutePlanExecuteStepInput {
                session_id: session.session_id.clone(),
                step_id: Some(write_step_id.clone()),
                base_revision: None,
            },
            &state,
        )
        .await
        .unwrap();
        let duplicate_result = execute_plan_execute_step_with_state(
            ExecutePlanExecuteStepInput {
                session_id: session.session_id.clone(),
                step_id: Some(write_step_id),
                base_revision: None,
            },
            &state,
        )
        .await
        .unwrap();

        assert_eq!(
            write_result.executed_step.step_status,
            PlanStepStatus::RequiresProposal
        );
        assert_eq!(
            duplicate_result.executed_step.linked_proposal_id,
            write_result.executed_step.linked_proposal_id
        );
        assert_eq!(write_result.session.linked_proposal_ids.len(), 1);

        let proposal_store = state.proposal_store.as_ref().unwrap().lock().await;
        let proposals = proposal_store.list_pending_proposals(10).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].source, ProposalSource::PlanningSession);
        assert_eq!(proposals[0].run_id, session.source_agent_run_id);
        assert_eq!(proposals[0].after["externalWriteExecuted"], false);
        drop(proposal_store);

        let agent_run_store = state.agent_run_store.as_ref().unwrap().lock().await;
        let run = agent_run_store
            .get_run(session.source_agent_run_id.as_deref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(run.generated_proposals.len(), 1);
        assert_eq!(run.generated_proposals[0], proposals[0].id);
        assert_eq!(run.status, AgentRunStatus::WaitingPermission);
        assert!(run.finished_at.is_none());
        assert!(run.reasoning_trace.is_none());
        assert!(run
            .reasoning_trace_digest
            .as_deref()
            .is_some_and(|digest| digest.starts_with("hmac-sha256:")));
        drop(agent_run_store);

        let replay = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
            &state,
            session.session_id.clone(),
            Some(0),
            Some(250),
        )
        .await
        .unwrap();
        let proposal_event = replay
            .iter()
            .find(|event| {
                event.event_type == "proposal.created" && event.object_id == proposals[0].id
            })
            .expect("write-like plan step must append proposal.created event");
        assert_eq!(proposal_event.payload["directWritesExecuted"], false);
        assert!(
            !replay.iter().any(|event| {
                event
                    .payload
                    .get("directLifeModelWrites")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                    || event
                        .payload
                        .get("externalWritesExecuted")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
            }),
            "Plan-Execute command events must not claim direct LifeModel or external writes"
        );

        crate::commands::proposal::accept_proposal_with_state(proposals[0].id.clone(), &state)
            .await
            .unwrap();
        let session_after_review = get_plan_execute_session_with_state(
            GetPlanExecuteSessionInput {
                session_id: session.session_id.clone(),
            },
            &state,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            session_after_review.status,
            PlanExecuteSessionStatus::InProgress,
            "Proposal approval does not complete the remaining Plan-Execute steps"
        );
        let run_after_review = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(session.source_agent_run_id.as_deref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(run_after_review.status, AgentRunStatus::Running);
        assert!(run_after_review.finished_at.is_none());
    }

    #[tokio::test]
    async fn plan_execute_cancel_and_review_commands_persist_step_state_and_events() {
        let state = crate::test_utils::test_app_state();
        let session = create_plan_execute_session_with_state(
            CreatePlanExecuteSessionInput {
                scenario_id: Some("weekly_planning".into()),
                source_chat_session_id: Some("chat-weekly".into()),
                max_steps: Some(5),
            },
            &state,
        )
        .await
        .unwrap();
        let session = finalize_plan_execute_session_with_state(
            FinalizePlanExecuteSessionInput {
                session_id: session.session_id.clone(),
                base_revision: Some(session.revision),
            },
            &state,
        )
        .await
        .unwrap();
        let read_step_id = session
            .steps
            .iter()
            .find(|step| !step.declared_write)
            .unwrap()
            .step_id
            .clone();
        let executed = execute_plan_execute_step_with_state(
            ExecutePlanExecuteStepInput {
                session_id: session.session_id.clone(),
                step_id: Some(read_step_id),
                base_revision: Some(session.revision),
            },
            &state,
        )
        .await
        .unwrap();

        let cancelled = cancel_plan_execute_session_with_state(
            CancelPlanExecuteSessionInput {
                session_id: executed.session.session_id.clone(),
                base_revision: Some(executed.session.revision),
            },
            &state,
        )
        .await
        .unwrap();
        assert_eq!(cancelled.status, PlanExecuteSessionStatus::Cancelled);
        let cancelled_run = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(cancelled.source_agent_run_id.as_deref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(cancelled_run.status, AgentRunStatus::Cancelled);
        assert!(cancelled_run.finished_at.is_some());
        assert!(cancelled
            .steps
            .iter()
            .filter(|step| step.status == PlanStepStatus::Cancelled)
            .all(|step| !step.evidence_ids.is_empty()));

        let review = review_plan_execute_session_with_state(
            ReviewPlanExecuteSessionInput {
                session_id: cancelled.session_id.clone(),
                base_revision: Some(cancelled.revision),
            },
            &state,
        )
        .await
        .unwrap();
        assert_eq!(review.summary.plan_status, "cancelled");
        assert!(!review.summary.completed_steps.is_empty());
        assert!(!review.summary.unresolved.is_empty());
        assert!(!review.summary.recommended_next_action.is_empty());
        assert_eq!(
            review.session.review_id.as_deref(),
            Some(review.summary.review_id.as_str())
        );

        let replay = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
            &state,
            cancelled.session_id.clone(),
            Some(0),
            Some(250),
        )
        .await
        .unwrap();
        let event_types = replay
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>();
        for required in [
            "plan.updated",
            "step.updated",
            "step.cancelled",
            "plan.reviewed",
        ] {
            assert!(
                event_types.contains(&required),
                "missing {required} in Plan-Execute cancel/review events: {event_types:?}"
            );
        }
        let review_event = replay
            .iter()
            .find(|event| event.event_type == "plan.reviewed")
            .expect("review event");
        assert_eq!(review_event.payload["reviewId"], review.summary.review_id);
        assert_eq!(review_event.payload["directLifeModelWrites"], false);
        assert_eq!(review_event.payload["externalWritesExecuted"], false);
    }

    #[tokio::test]
    async fn plan_execute_product_artifact_view_builds_body_with_source_tool_evidence() {
        let state = crate::test_utils::test_app_state();
        let mut session = create_plan_execute_session_with_state(
            CreatePlanExecuteSessionInput {
                scenario_id: Some("weekly_planning".into()),
                source_chat_session_id: Some("chat-sichuan-museum".into()),
                max_steps: Some(5),
            },
            &state,
        )
        .await
        .unwrap();
        let source_observation_id = "observation-sichuan-hours-1".to_string();
        session.steps[0]
            .linked_observation_ids
            .push(source_observation_id.clone());
        session.steps[0]
            .evidence_ids
            .push(source_observation_id.clone());

        let action = ActionEvidence {
            action_id: "action-web-hours-1".into(),
            action_type: "web.read".into(),
            target: "https://example.invalid/sichuan-museum-hours".into(),
            label: "Read Sichuan Museum opening hours".into(),
            status: "succeeded".into(),
            risk_level: "safe_read".into(),
            policy_decision_id: "policy-hours-1".into(),
            started_at: None,
            finished_at: None,
            observation_ids: vec![source_observation_id.clone()],
            retryable: false,
        };
        let observation = ObservationEvidence {
            observation_id: source_observation_id.clone(),
            action_id: action.action_id.clone(),
            source_kind: "web".into(),
            source_label: "Sichuan Museum official opening hours".into(),
            preview: "Source states opening hours require same-day verification.".into(),
            citation_available: true,
            read_execution: Some(
                openlife_core::agent::main_chat_runtime_contract::ReadExecutionEvidence {
                    kind: "web_read".into(),
                    source_kind: "web".into(),
                    source_label: "Sichuan Museum official opening hours".into(),
                    target: action.target.clone(),
                    real_read_only_execution: true,
                    fixture_backed: false,
                    network_read_attempted: true,
                    direct_writes_executed: false,
                },
            ),
            created_at: chrono::Utc::now(),
        };
        let route = StrategyEvidence {
            strategy: openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductStrategyRoute::PlanExecute,
            reason: "kernel_supported_plan_execute".into(),
            confidence: Some(0.91),
        };
        let artifact = build_plan_artifact_view(
            &session,
            PlanArtifactRuntimeEvidence {
                task_session_id: "task-plan-artifact-1",
                run_id: session.source_agent_run_id.as_deref(),
                route: &route,
                actions: &[action],
                observations: &[observation],
                proposals: &[],
                blockers: &[],
                final_delivery_id: Some("delivery-plan-1"),
            },
        );

        assert_eq!(artifact.plan_id, session.plan_id);
        assert_eq!(artifact.plan_session_id, session.session_id);
        assert_eq!(artifact.task_session_id, "task-plan-artifact-1");
        assert_eq!(
            artifact.run_id.as_str(),
            session.source_agent_run_id.as_deref().unwrap()
        );
        assert!(artifact.body.contains("Plan ID:"));
        assert!(artifact.body.contains("Steps"));
        assert!(artifact
            .body
            .contains("source/tool evidence: observation-sichuan-hours-1"));
        assert!(artifact.steps.iter().any(|step| step
            .source_tool_evidence
            .iter()
            .any(|source| source.evidence_id == source_observation_id
                && source.tool_name.as_deref() == Some("web_read"))));
        assert!(artifact
            .assumptions
            .iter()
            .any(|assumption| assumption.label == "Source-backed opening hours note"));
        assert!(!artifact
            .unknowns
            .iter()
            .any(|unknown| unknown.label == "opening hours"));
        assert!(artifact
            .unknowns
            .iter()
            .any(|unknown| unknown.label == "weather"));
        assert!(artifact.controls.contains(&"confirm_plan".to_string()));
        assert!(!artifact.controls.contains(&"continue".to_string()));
        assert!(!artifact.controls.contains(&"edit_plan".to_string()));
        assert_eq!(artifact.route_evidence.strategy, "plan_execute");
        assert_eq!(
            artifact.run_evidence.observation_ids,
            vec![source_observation_id]
        );
        assert_eq!(
            artifact.run_evidence.final_delivery_id.as_deref(),
            Some("delivery-plan-1")
        );
    }

    #[tokio::test]
    async fn plan_execute_product_artifact_view_keeps_realtime_facts_unknown_without_sources() {
        let state = crate::test_utils::test_app_state();
        let session = create_plan_execute_session_with_state(
            CreatePlanExecuteSessionInput {
                scenario_id: Some("weekly_planning".into()),
                source_chat_session_id: Some("chat-offline-plan".into()),
                max_steps: Some(5),
            },
            &state,
        )
        .await
        .unwrap();
        let route = StrategyEvidence {
            strategy: openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductStrategyRoute::PlanExecute,
            reason: "kernel_supported_plan_execute".into(),
            confidence: None,
        };
        let artifact = build_plan_artifact_view(
            &session,
            PlanArtifactRuntimeEvidence {
                task_session_id: "task-offline-plan-1",
                run_id: session.source_agent_run_id.as_deref(),
                route: &route,
                actions: &[],
                observations: &[],
                proposals: &[],
                blockers: &[],
                final_delivery_id: None,
            },
        );

        for label in ["opening hours", "weather", "transportation"] {
            assert!(
                artifact
                    .unknowns
                    .iter()
                    .any(|unknown| unknown.label == label
                        && unknown.detail.contains("No source/tool evidence")),
                "missing realtime unknown {label}: {:?}",
                artifact.unknowns
            );
        }
        assert!(artifact
            .unknowns
            .iter()
            .any(|unknown| unknown.detail.contains("venue opening hours")));
        assert!(artifact
            .body
            .contains("No source/tool evidence is attached to this plan artifact yet."));
        assert!(artifact.body.contains("Review current priorities"));
        assert!(!artifact.body.contains("created governed draft"));
    }

    #[tokio::test]
    async fn cancel_winning_epoch_rejects_main_chat_plan_session_commit() {
        let state = crate::test_utils::test_app_state();
        let task_session_id = "plan-cancel-wins";
        let registry = {
            state
                .main_chat_runtime_state
                .lock()
                .await
                .cancellation_registry
                .clone()
        };
        let registration = registry.register(task_session_id);
        registry.request_cancel(task_session_id);

        let error = create_plan_execute_session_for_main_chat_with_state(
            CreatePlanExecuteSessionInput {
                scenario_id: Some("weekly_planning".into()),
                source_chat_session_id: Some("plan-cancel-chat".into()),
                max_steps: Some(5),
            },
            &state,
            "run-plan-cancel",
            &registration.execution_epoch(),
            "Plan this week.",
            Vec::new(),
        )
        .await
        .expect_err("cancel-winning epoch must reject PlanExecute canonical commit");
        assert!(error.contains("Plan-Execute session commit rejected"));
        let sessions = state
            .plan_execute_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_sessions(10)
            .unwrap();
        assert!(sessions.is_empty());
        assert!(registration
            .execution_epoch()
            .snapshot()
            .commit_facts
            .iter()
            .any(|fact| {
                fact.domain == "plan_execute"
                    && fact.outcome
                        == crate::main_chat_cancellation::MainChatCanonicalCommitOutcome::RejectedAfterCancel
            }));
    }

    #[tokio::test]
    async fn main_chat_plan_session_applies_bounded_lifemodel_goal_hint() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let session = create_plan_execute_session_with_source_run(
            CreatePlanExecuteSessionInput {
                scenario_id: Some("weekly_planning".into()),
                source_chat_session_id: Some("lifemodel-plan-hint".into()),
                max_steps: Some(5),
            },
            &state,
            None,
            None,
            Some("Plan this week around OpenLife."),
            vec![openlife_core::agent::PlanExecuteLifeModelHint {
                item_id: "goal-openlife".into(),
                section: openlife_core::life_model::v2::LifeModelSectionV2::LongTermGoals,
                value: "完成 OpenLife: 让个人 Agent OS 真正可用".into(),
                selected_reason: "task keyword matches: 1".into(),
            }],
        )
        .await
        .expect("LifeModel-aware PlanExecute draft");

        assert!(session.steps[0].title.contains("OpenLife"));
        assert_eq!(session.steps[0].intent, "lifemodel_goal_alignment");
        assert!(!session.steps[0].declared_write);
        assert!(session.steps[0].tool_name.is_none());
        assert_eq!(session.steps[0].risk_level, RiskLevel::Low);
    }
}
