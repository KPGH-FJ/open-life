use crate::main_chat_policy_runtime::build_chat_runtime_policy_context;
use crate::AppState;
use openlife_core::agent::main_chat_runtime_contract::{
    ActionEvidence, BlockerEvidence, ObservationEvidence, PlanArtifactFactView,
    PlanArtifactRouteEvidence, PlanArtifactRunEvidence, PlanArtifactSourceEvidence,
    PlanArtifactStepView, PlanArtifactView, ProposalEvidence, StrategyEvidence,
};
use openlife_core::agent::{
    AgentExecutionBudget, AgentTask, AgentTaskKind, PlanExecuteInput, PlanExecuteProductContract,
    PlanExecuteProductScenario, PlanExecuteService, PlanExecuteSession, PlanExecuteSessionStatus,
    PlanStepStatus, RuntimeInput,
};
use openlife_core::layer::Layer;
use openlife_core::llm::ChatMessage;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

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

pub(crate) async fn create_plan_execute_session_for_main_chat_with_state(
    input: CreatePlanExecuteSessionInput,
    state: &Arc<AppState>,
    source_run_id: &str,
    execution_epoch: &crate::main_chat_cancellation::MainChatExecutionEpoch,
    task_text: &str,
    life_model_hints: Vec<openlife_core::agent::PlanExecuteLifeModelHint>,
) -> Result<PlanExecuteSession, String> {
    let session =
        draft_plan_execute_session(input, state, source_run_id, task_text, life_model_hints)
            .await?;
    let store_arc = state
        .plan_execute_session_store
        .as_ref()
        .ok_or_else(|| "Plan-Execute session store not available".to_string())?;
    let store = store_arc.lock().await;
    let commit_permit = execution_epoch
        .begin_canonical_commit("plan_execute", format!("session:{}", session.session_id))
        .map_err(|rejection| format!("Plan-Execute session commit rejected: {rejection:?}"))?;
    match store.create_session(&session) {
        Ok(()) => commit_permit.finish_committed(),
        Err(error) => {
            commit_permit.finish_failed();
            return Err(format!("failed to create Plan-Execute session: {error}"));
        }
    }
    drop(store);
    append_plan_created_events(state, &session).await?;
    Ok(session)
}

async fn draft_plan_execute_session(
    input: CreatePlanExecuteSessionInput,
    state: &Arc<AppState>,
    source_run_id: &str,
    task_text: &str,
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

    let tools_prompt = {
        let registry = state.mcp_registry.lock().await;
        registry.tools_prompt()
    };
    let task_text = if task_text.trim().is_empty() {
        "Plan this week using confirmed context."
    } else {
        task_text
    };
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
    let policy_context = build_chat_runtime_policy_context(state, &task, &tools_prompt)?;
    let runtime_input = RuntimeInput::from_agent_task(
        task,
        None,
        tools_prompt,
        policy_context,
        AgentExecutionBudget {
            max_steps: max_steps as u32,
            max_tool_calls: 0,
            timeout_seconds: 30,
            allow_cloud: false,
            allow_writes: false,
        },
    )
    .with_source_run_id(source_run_id.to_string());
    let service = PlanExecuteService;
    let plan_input = PlanExecuteInput::from_runtime_input(
        runtime_input,
        "scenario=weekly_planning product=workspace",
        max_steps,
    )
    .with_life_model_hints(life_model_hints);
    let draft = service.draft_product_plan(&plan_input, scenario);
    PlanExecuteSession::new_draft(
        Some(source_chat_session_id),
        Some(source_run_id.to_string()),
        contract,
        draft,
    )
    .map_err(|e| e.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::RiskLevel;

    fn draft_input(chat_id: &str) -> CreatePlanExecuteSessionInput {
        CreatePlanExecuteSessionInput {
            scenario_id: Some("weekly_planning".into()),
            source_chat_session_id: Some(chat_id.into()),
            max_steps: Some(5),
        }
    }

    #[tokio::test]
    async fn plan_artifact_keeps_realtime_facts_unknown_without_sources() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let session = draft_plan_execute_session(
            draft_input("chat-offline-plan"),
            &state,
            "run-offline-plan",
            "Plan this week.",
            Vec::new(),
        )
        .await
        .expect("draft plan");

        let route = StrategyEvidence {
            strategy: openlife_core::agent::main_chat_runtime_contract::MainChatAgentProductStrategyRoute::PlanExecute,
            reason: "kernel_supported_plan_execute".into(),
            confidence: None,
        };
        let artifact = build_plan_artifact_view(
            &session,
            PlanArtifactRuntimeEvidence {
                task_session_id: "task-offline-plan",
                run_id: Some("run-offline-plan"),
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
            .body
            .contains("No source/tool evidence is attached to this plan artifact yet."));
    }

    #[tokio::test]
    async fn plan_artifact_binds_source_tool_evidence() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut session = draft_plan_execute_session(
            draft_input("chat-source-plan"),
            &state,
            "run-source-plan",
            "Plan a museum visit.",
            Vec::new(),
        )
        .await
        .expect("draft plan");
        let observation_id = "observation-hours-1".to_string();
        session.steps[0]
            .linked_observation_ids
            .push(observation_id.clone());
        session.steps[0].evidence_ids.push(observation_id.clone());

        let action = ActionEvidence {
            action_id: "action-hours-1".into(),
            action_type: "web.read".into(),
            target: "https://example.invalid/hours".into(),
            label: "Read official opening hours".into(),
            status: "succeeded".into(),
            risk_level: "safe_read".into(),
            policy_decision_id: "policy-hours-1".into(),
            started_at: None,
            finished_at: None,
            observation_ids: vec![observation_id.clone()],
            retryable: false,
        };
        let observation = ObservationEvidence {
            observation_id: observation_id.clone(),
            action_id: action.action_id.clone(),
            source_kind: "web".into(),
            source_label: "Official opening hours".into(),
            preview: "Opening hours require same-day verification.".into(),
            citation_available: true,
            read_execution: Some(
                openlife_core::agent::main_chat_runtime_contract::ReadExecutionEvidence {
                    kind: "web_read".into(),
                    source_kind: "web".into(),
                    source_label: "Official opening hours".into(),
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
            confidence: Some(0.9),
        };
        let artifact = build_plan_artifact_view(
            &session,
            PlanArtifactRuntimeEvidence {
                task_session_id: "task-source-plan",
                run_id: Some("run-source-plan"),
                route: &route,
                actions: &[action],
                observations: &[observation],
                proposals: &[],
                blockers: &[],
                final_delivery_id: Some("delivery-source-plan"),
            },
        );

        assert!(artifact.steps.iter().any(|step| step
            .source_tool_evidence
            .iter()
            .any(|source| source.evidence_id == observation_id)));
        assert!(!artifact
            .unknowns
            .iter()
            .any(|unknown| unknown.label == "opening hours"));
        assert_eq!(
            artifact.run_evidence.final_delivery_id.as_deref(),
            Some("delivery-source-plan")
        );
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
            draft_input("plan-cancel-chat"),
            &state,
            "run-plan-cancel",
            &registration.execution_epoch(),
            "Plan this week.",
            Vec::new(),
        )
        .await
        .expect_err("cancel-winning epoch must reject the canonical commit");

        assert!(error.contains("Plan-Execute session commit rejected"));
        assert!(state
            .plan_execute_session_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_sessions(10)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn plan_draft_applies_bounded_lifemodel_goal_hint() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let session = draft_plan_execute_session(
            draft_input("lifemodel-plan-hint"),
            &state,
            "run-lifemodel-plan",
            "Plan this week around OpenLife.",
            vec![openlife_core::agent::PlanExecuteLifeModelHint {
                item_id: "goal-openlife".into(),
                section: openlife_core::life_model::v2::LifeModelSectionV2::LongTermGoals,
                value: "完成 OpenLife: 让个人 Agent OS 真正可用".into(),
                selected_reason: "task keyword matches: 1".into(),
            }],
        )
        .await
        .expect("LifeModel-aware plan draft");

        assert!(session.steps[0].title.contains("OpenLife"));
        assert_eq!(session.steps[0].intent, "lifemodel_goal_alignment");
        assert!(!session.steps[0].declared_write);
        assert!(session.steps[0].tool_name.is_none());
        assert_eq!(session.steps[0].risk_level, RiskLevel::Low);
    }
}
