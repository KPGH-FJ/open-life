use crate::agent::{
    AgentExecutionBudget, AgentTask, AgentTaskKind, GovernanceDecisionKind, HSSelectionAudit,
    LifeModelGovernor, PlanExecuteInput, PlanExecuteService, PlanStepStatus, ProposalStore,
    RiskLevel, RuntimeHSPacket, RuntimeInput,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;

fn runtime_input(user_text: &str) -> RuntimeInput {
    runtime_input_with_source_run(user_text, None)
}

fn runtime_input_with_source_run(user_text: &str, source_run_id: Option<&str>) -> RuntimeInput {
    RuntimeInput::from_agent_task(
        AgentTask {
            kind: AgentTaskKind::Planning,
            session_id: "session-plan-execute".into(),
            user_text: user_text.into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: user_text.into(),
            }],
            layer: Layer::L2,
        },
        LifeModel::default(),
        Some("memory context should not be copied into trace".into()),
        "Available tools: memory.search, calendar.create_event, file.update",
        source_run_id.map(test_hs_packet),
        AgentExecutionBudget::default(),
    )
}

fn test_hs_packet(source_run_id: &str) -> RuntimeHSPacket {
    RuntimeHSPacket {
        selected_policies: Vec::new(),
        selected_heuristics: Vec::new(),
        estimated_tokens: 8,
        audit: HSSelectionAudit {
            agent_task_id: Some("task-plan-execute".into()),
            agent_run_id: Some(source_run_id.into()),
            input_digest: "digest-input".into(),
            selected_policy_ids: Vec::new(),
            selected_heuristic_ids: Vec::new(),
            excluded_assets: Vec::new(),
            estimated_tokens: 8,
            token_budget: 128,
        },
    }
}

fn plan_input(user_text: &str, max_steps: usize) -> PlanExecuteInput {
    PlanExecuteInput {
        runtime_input: runtime_input(user_text),
        objective: "metadata-safe objective".into(),
        max_steps,
    }
}

fn plan_input_with_source_run(
    user_text: &str,
    max_steps: usize,
    source_run_id: &str,
) -> PlanExecuteInput {
    PlanExecuteInput {
        runtime_input: runtime_input_with_source_run(user_text, Some(source_run_id)),
        objective: "metadata-safe objective".into(),
        max_steps,
    }
}

#[test]
fn draft_plan_creates_read_only_step_for_simple_question() {
    let service = PlanExecuteService::default();
    let plan = service.draft_plan(&plan_input("What should I focus on today?", 4));

    assert_eq!(plan.steps.len(), 1);
    let step = &plan.steps[0];
    assert_eq!(step.id, "step-1");
    assert_eq!(step.action_kind, "reason");
    assert_eq!(step.risk_level, RiskLevel::Low);
    assert!(!step.declared_write);
}

#[test]
fn draft_plan_marks_write_like_intent_without_executing() {
    let service = PlanExecuteService::default();
    let plan = service.draft_plan(&plan_input("Create a calendar event for tomorrow.", 4));

    assert_eq!(plan.steps.len(), 1);
    let step = &plan.steps[0];
    assert_eq!(step.action_kind, "create");
    assert_eq!(step.risk_level, RiskLevel::Medium);
    assert!(step.declared_write);
    assert_eq!(step.tool_name.as_deref(), Some("external.write_proposal"));
}

#[test]
fn plan_step_uses_governor_before_execution() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(plan_input("Update my calendar.", 4), &governor);

    assert_eq!(output.traces.len(), 1);
    assert_eq!(
        output.traces[0].decision.kind,
        GovernanceDecisionKind::RequireProposal
    );
    assert_eq!(output.traces[0].status, PlanStepStatus::RequiresProposal);
    assert!(output.runtime_outputs.is_empty());
}

#[test]
fn write_like_step_requires_proposal_and_is_not_executed() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(plan_input("Send an email to Alice.", 4), &governor);

    assert_eq!(output.traces.len(), 1);
    assert_eq!(output.traces[0].status, PlanStepStatus::RequiresProposal);
    assert_ne!(output.traces[0].status, PlanStepStatus::Executed);
    assert!(output.runtime_outputs.is_empty());
}

#[test]
fn read_only_step_is_allowed() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(
        plan_input("Search my notes for project context.", 4),
        &governor,
    );

    assert_eq!(output.traces.len(), 1);
    assert_eq!(
        output.traces[0].decision.kind,
        GovernanceDecisionKind::Allow
    );
    assert_eq!(output.traces[0].status, PlanStepStatus::Executed);
}

#[test]
fn plan_execute_v1_report_summarizes_governed_vertical_slice() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(
        plan_input_with_source_run(
            "Search notes for Alice and then update the calendar with alice@example.com.",
            4,
            "run-source-123",
        ),
        &governor,
    );

    assert!(output.report.plan_id.starts_with("plan-"));
    assert_eq!(
        output.report.source_run_id.as_deref(),
        Some("run-source-123")
    );
    assert_eq!(output.report.step_count, 2);
    assert_eq!(output.report.executed_read_only_step_count, 1);
    assert_eq!(output.report.blocked_or_proposal_required_step_count, 1);
    assert_eq!(output.report.governance_decisions.len(), 2);
    assert_eq!(output.report.observation_summaries.len(), 1);
    assert_eq!(
        output.report.metadata_safe_summary["reportKind"],
        "plan_execute_v1"
    );
}

#[test]
fn read_only_step_execution_produces_metadata_safe_observation_summary() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(
        plan_input(
            "Search memory for Alice's private note alice@example.com and raw draft text.",
            4,
        ),
        &governor,
    );

    assert_eq!(output.report.executed_read_only_step_count, 1);
    let observation = &output.report.observation_summaries[0];
    assert_eq!(observation.step_id, "step-1");
    assert_eq!(observation.source, "internal_read_only");
    assert!(observation.summary.contains("read-only"));
    assert!(!observation.summary.contains("Alice"));
    assert!(!observation.summary.contains("alice@example.com"));
    assert!(!observation.summary.contains("raw draft"));
}

#[test]
fn plan_execution_respects_max_steps() {
    let service = PlanExecuteService::default();
    let input = plan_input("Search my notes and then create a calendar event.", 1);

    let plan = service.draft_plan(&input);
    let output = service.execute_plan(input, &LifeModelGovernor::default());

    assert_eq!(plan.steps.len(), 1);
    assert_eq!(output.plan.steps.len(), 1);
    assert_eq!(output.traces.len(), 1);
}

#[test]
fn plan_trace_is_metadata_safe() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let output = service.execute_plan(
        plan_input(
            "Search for Alice's note, alice@example.com, and send her the full draft.",
            4,
        ),
        &governor,
    );

    let serialized_traces = serde_json::to_string(&output).unwrap();

    assert!(serialized_traces.contains("policyReasonCode"));
    assert!(!serialized_traces.contains("Alice"));
    assert!(!serialized_traces.contains("alice@example.com"));
    assert!(!serialized_traces.contains("full draft"));
    assert!(!serialized_traces.contains("memory context should not be copied"));
}

#[test]
fn plan_execute_does_not_write_lifemodel_memory_or_proposal_store() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let proposal_store = ProposalStore::new_in_memory().unwrap();

    let output = service.execute_plan(plan_input("Create a new reminder.", 4), &governor);

    assert!(output.runtime_outputs.is_empty());
    assert!(output.warnings.is_empty());
    assert!(proposal_store
        .list_pending_proposals(10)
        .unwrap()
        .is_empty());
}

#[test]
fn write_intents_for_sensitive_surfaces_are_proposal_required_without_direct_apply() {
    let service = PlanExecuteService::default();
    let governor = LifeModelGovernor::default();
    let proposal_store = ProposalStore::new_in_memory().unwrap();

    for user_text in [
        "Update my LifeModel with a new preference.",
        "Write this detail into memory.",
        "Update a file with this draft.",
        "Create a calendar event.",
        "Send an email with this draft.",
    ] {
        let output = service.execute_plan(plan_input(user_text, 4), &governor);

        assert_eq!(
            output.traces[0].status,
            PlanStepStatus::RequiresProposal,
            "{user_text}"
        );
        assert!(output.runtime_outputs.is_empty(), "{user_text}");
        assert!(
            output.report.observation_summaries.is_empty(),
            "{user_text}"
        );
    }

    assert!(proposal_store
        .list_pending_proposals(10)
        .unwrap()
        .is_empty());
}
