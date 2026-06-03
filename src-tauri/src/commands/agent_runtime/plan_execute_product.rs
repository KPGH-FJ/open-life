use crate::AppState;
use openlife_core::agent::{
    behavior_checks_for_packet, AgentExecutionBudget, AgentRun, AgentRunStatus, AgentTask,
    AgentTaskKind, ContextSummary, LifeModelGovernor, PlanExecuteInput, PlanExecuteProductContract,
    PlanExecuteProductScenario, PlanExecuteService, PlanExecuteSession, PlanExecuteStepEdit,
    PlanExecuteStepExecutionResult, PlanStepStatus, ReasoningTrace, RedactionLevel, RiskLevel,
    RuntimeInput,
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
    pub steps: Vec<PlanExecuteStepEditInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizePlanExecuteSessionInput {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelPlanExecuteSessionInput {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutePlanExecuteStepInput {
    pub session_id: String,
    #[serde(default)]
    pub step_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutePlanExecuteStepOutput {
    pub session: PlanExecuteSession,
    pub executed_step: PlanExecuteStepExecutionResult,
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
pub async fn execute_plan_execute_step(
    input: ExecutePlanExecuteStepInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ExecutePlanExecuteStepOutput, String> {
    execute_plan_execute_step_with_state(input, &state.inner().clone()).await
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
    let hs_packet = crate::build_chat_runtime_hs_packet(
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
    );
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
    let edits = input
        .steps
        .into_iter()
        .map(plan_execute_step_edit_from_input)
        .collect::<Result<Vec<_>, _>>()?;
    session
        .apply_draft_edits(edits)
        .map_err(|e| e.to_string())?;
    save_plan_execute_session(state, &session).await?;
    update_existing_product_run_for_session(state, &session).await?;
    Ok(session)
}

pub(crate) async fn finalize_plan_execute_session_with_state(
    input: FinalizePlanExecuteSessionInput,
    state: &Arc<AppState>,
) -> Result<PlanExecuteSession, String> {
    let mut session = load_plan_execute_session(state, &input.session_id).await?;
    session.finalize().map_err(|e| e.to_string())?;
    save_plan_execute_session(state, &session).await?;
    update_existing_product_run_for_session(state, &session).await?;
    Ok(session)
}

pub(crate) async fn cancel_plan_execute_session_with_state(
    input: CancelPlanExecuteSessionInput,
    state: &Arc<AppState>,
) -> Result<PlanExecuteSession, String> {
    let mut session = load_plan_execute_session(state, &input.session_id).await?;
    session.cancel().map_err(|e| e.to_string())?;
    save_plan_execute_session(state, &session).await?;
    update_existing_product_run_for_session(state, &session).await?;
    Ok(session)
}

pub(crate) async fn execute_plan_execute_step_with_state(
    input: ExecutePlanExecuteStepInput,
    state: &Arc<AppState>,
) -> Result<ExecutePlanExecuteStepOutput, String> {
    let mut session = load_plan_execute_session(state, &input.session_id).await?;
    let step_id = match input.step_id {
        Some(step_id) => step_id,
        None => session
            .steps
            .iter()
            .find(|step| {
                !matches!(
                    step.status,
                    PlanStepStatus::Executed
                        | PlanStepStatus::RequiresProposal
                        | PlanStepStatus::Blocked
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
    let executed_step = session
        .execute_step(&step_id, &LifeModelGovernor, &proposal_store)
        .map_err(|e| e.to_string())?;
    drop(proposal_store);
    save_plan_execute_session(state, &session).await?;
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
            "planExecuteProductVertical": true,
            "scenarioId": "weekly_planning",
            "status": "started",
            "strategyKind": "plan_execute",
            "metadataSafe": true,
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
            PlanStepStatus::Skipped | PlanStepStatus::RequiresConfirmation => {}
        }
    }
    json!({
        "planExecuteProductVertical": true,
        "scenarioId": session.scenario.as_id(),
        "planSessionId": session.session_id,
        "strategyKind": "plan_execute",
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
            },
            &state,
        )
        .await
        .unwrap_err();
        assert!(execute_error.contains("finalized"));

        let edited = update_plan_execute_session_draft_with_state(
            UpdatePlanExecuteSessionDraftInput {
                session_id: session.session_id.clone(),
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
            },
            &state,
        )
        .await
        .unwrap();
        assert_eq!(finalized.status, PlanExecuteSessionStatus::Finalized);

        let edit_error = update_plan_execute_session_draft_with_state(
            UpdatePlanExecuteSessionDraftInput {
                session_id: session.session_id,
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
            },
            &state,
        )
        .await
        .unwrap();
        let duplicate_result = execute_plan_execute_step_with_state(
            ExecutePlanExecuteStepInput {
                session_id: session.session_id.clone(),
                step_id: Some(write_step_id),
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
    }
}
