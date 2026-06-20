use crate::main_chat_hs_runtime::build_chat_runtime_hs_packet;
use crate::AppState;
use openlife_core::agent::{
    behavior_checks_for_packet, AgentExecutionBudget, AgentRun, AgentRunStatus, AgentTask,
    AgentTaskKind, ContextSummary, LifeModelGovernor, PlanExecuteInput, PlanExecuteProductContract,
    PlanExecuteProductScenario, PlanExecuteService, PlanExecuteSession, PlanExecuteStepEdit,
    PlanExecuteStepExecutionResult, PlanStepStatus, ReasoningTrace, RedactionLevel, RiskLevel,
    RuntimeGuidanceConsumptionMode, RuntimeInput, RuntimeStrategyRegistry,
};
use openlife_core::layer_router::Layer;
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

    let mut run = new_plan_execute_product_run(&source_chat_session_id);
    let run_id = run.id.clone();
    create_product_run(state, &run).await?;

    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().unwrap_or_else(|_| LifeModel::default())
    };
    let tools_prompt = {
        let registry = state.mcp_registry.lock().await;
        registry.tools_prompt()
    };
    let task = AgentTask {
        kind: AgentTaskKind::Planning,
        session_id: source_chat_session_id.clone(),
        user_text: "Use my LifeModel to plan this week.".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "Use my LifeModel to plan this week.".into(),
        }],
        layer: Layer::L2,
    };
    let hs_packet = build_chat_runtime_hs_packet(
        state,
        &task,
        &life_model,
        &tools_prompt,
        Some(run_id.clone()),
    )
    .await
    .ok()
    .flatten();
    let behavior_checks = hs_packet
        .as_ref()
        .map(behavior_checks_for_packet)
        .unwrap_or_default();
    let hs_selection_audit = hs_packet.as_ref().map(|packet| packet.audit.clone());
    let runtime_input = RuntimeInput::from_agent_task(
        task,
        life_model.clone(),
        None,
        tools_prompt,
        hs_packet,
        AgentExecutionBudget {
            max_steps: max_steps as u32,
            max_tool_calls: 0,
            timeout_seconds: 30,
            allow_cloud: false,
            allow_writes: false,
        },
    )
    .with_guidance_consumption_mode(RuntimeGuidanceConsumptionMode::ExplicitRuntime);
    let service = PlanExecuteService;
    let plan_input = PlanExecuteInput::from_runtime_input(
        runtime_input,
        "scenario=weekly_planning product=workspace",
        max_steps,
    );
    let draft = service.draft_product_plan(&plan_input, scenario);
    let session = PlanExecuteSession::new_draft(
        Some(source_chat_session_id),
        Some(run_id.clone()),
        contract,
        draft,
    )
    .map_err(|e| e.to_string())?;

    {
        let store_arc = state
            .plan_execute_session_store
            .as_ref()
            .ok_or_else(|| "Plan-Execute session store not available".to_string())?;
        let store = store_arc.lock().await;
        store
            .create_session(&session)
            .map_err(|e| format!("failed to create Plan-Execute session: {e}"))?;
    }
    append_plan_created_events(state, &session).await?;

    run.hs_selection_audit = hs_selection_audit;
    run.behavior_checks = behavior_checks;
    update_product_run_for_session(state, &mut run, &session).await?;
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
    payload: Value,
) -> Result<crate::main_chat_event_stream::MainChatAgentDurableEvent, String> {
    crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
        state,
        session.session_id.clone(),
        session
            .source_agent_run_id
            .clone()
            .unwrap_or_else(|| session.session_id.clone()),
        event_type,
        object_type,
        object_id,
        "plan_runtime",
        payload,
    )
    .await
}

fn plan_event_payload(session: &PlanExecuteSession) -> Value {
    json!({
        "planId": session.plan_id,
        "planSessionId": session.session_id,
        "taskSessionId": session.session_id,
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
    run.reasoning_strategy = Some("plan_execute_product".into());
    run.output_preview = Some("Weekly plan draft started".into());
    run.context_summary = Some(plan_execute_context_summary(false));
    run.reasoning_trace = Some(ReasoningTrace {
        strategy_result: Some(json!({
            "runtimeStrategyTraceKind": "plan_execute_product",
            "planExecuteProductVertical": true,
            "scenarioId": "weekly_planning",
            "status": "started",
            "strategyKind": "plan_execute",
            "selectedStrategyKind": "plan_execute",
            "payloadKind": "plan_execute",
            "strategyDescriptorId": "plan_execute",
            "strategyCapabilityIds": ["planning.plan_execute", "proposal_first_steps", "metadata_safe_trace"],
            "selectionReasonCode": "weekly_planning_product",
            "governanceDecisionKind": "allow",
            "registryReady": RuntimeStrategyRegistry::fixed_readiness_report().ready,
            "metadataSafe": true,
            "defaultChatUnchanged": true,
            "sideEffectBudget": plan_execute_trace_side_effect_budget(),
            "rawPromptStored": false,
            "rawWeeklyPlanProseStored": false,
            "directLifeModelWrites": false,
            "externalWritesExecuted": false,
        })),
        output: Some("plan_execute_product_started".into()),
        ..ReasoningTrace::default()
    });
    run
}

async fn create_product_run(state: &Arc<AppState>, run: &AgentRun) -> Result<(), String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for Plan-Execute product".to_string())?;
    let store = store_arc.lock().await;
    store
        .create_run(run)
        .map_err(|e| format!("failed to create Plan-Execute AgentRun: {e}"))
}

async fn update_existing_product_run_for_session(
    state: &Arc<AppState>,
    session: &PlanExecuteSession,
) -> Result<(), String> {
    let Some(run_id) = session.source_agent_run_id.as_deref() else {
        return Ok(());
    };
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for Plan-Execute product".to_string())?;
    let store = store_arc.lock().await;
    let Some(mut run) = store
        .get_run(run_id)
        .map_err(|e| format!("failed to load Plan-Execute AgentRun: {e}"))?
    else {
        return Ok(());
    };
    drop(store);
    update_product_run_for_session(state, &mut run, session).await
}

async fn update_product_run_for_session(
    state: &Arc<AppState>,
    run: &mut AgentRun,
    session: &PlanExecuteSession,
) -> Result<(), String> {
    run.status = AgentRunStatus::Completed;
    run.finished_at = Some(chrono::Utc::now());
    run.generated_proposals = session.linked_proposal_ids.clone();
    run.warnings = session.warnings.clone();
    run.step_count = session.step_count as u32;
    run.tool_call_count = 0;
    run.context_summary = Some(plan_execute_context_summary(false));
    run.output_preview = Some(plan_execute_output_preview(session));
    run.reasoning_strategy = Some("plan_execute_product".into());
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
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "AgentRun store not available for Plan-Execute product".to_string())?;
    let store = store_arc.lock().await;
    store
        .update_run(run)
        .map_err(|e| format!("failed to update Plan-Execute AgentRun: {e}"))
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

fn plan_execute_output_preview(session: &PlanExecuteSession) -> String {
    format!(
        "Weekly plan {}: {} steps, {} proposals",
        session.status,
        session.step_count,
        session.linked_proposal_ids.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{PlanExecuteSessionStatus, PlanStepStatus, ProposalSource};

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
        assert_eq!(
            run.reasoning_strategy.as_deref(),
            Some("plan_execute_product")
        );
        let trace = run.reasoning_trace.unwrap().strategy_result.unwrap();
        let serialized = serde_json::to_string(&trace).unwrap();
        assert_eq!(trace["planExecuteProductVertical"], true);
        assert_eq!(trace["scenarioId"], "weekly_planning");
        assert_eq!(trace["planSessionId"], session.session_id);
        assert_eq!(trace["runtimeStrategyTraceKind"], "plan_execute_product");
        assert_eq!(trace["selectedStrategyKind"], "plan_execute");
        assert_eq!(trace["payloadKind"], "plan_execute");
        assert_eq!(trace["strategyDescriptorId"], "plan_execute");
        assert_eq!(trace["selectionReasonCode"], "weekly_planning_product");
        assert_eq!(trace["registryReady"], true);
        assert_eq!(trace["defaultChatUnchanged"], true);
        assert_eq!(trace["sideEffectBudget"]["externalWrites"], 0);
        assert!(!serialized.contains("Use my LifeModel"));
        assert!(!serialized.contains("raw weekly"));
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
        let trace = run.reasoning_trace.unwrap().strategy_result.unwrap();
        assert_eq!(trace["generatedProposalIds"][0], proposals[0].id);
        assert_eq!(trace["externalWritesExecuted"], false);
        assert_eq!(trace["directLifeModelWrites"], false);
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
}
