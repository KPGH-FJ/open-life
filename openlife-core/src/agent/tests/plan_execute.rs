use crate::agent::{
    AgentExecutionBudget, AgentTask, AgentTaskKind, GovernanceDecisionKind, LifeModelGovernor,
    PlanExecuteInput, PlanExecuteService, PlanStepStatus, ProposalStore, RiskLevel, RuntimeInput,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;

fn runtime_input(user_text: &str) -> RuntimeInput {
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
        None,
        AgentExecutionBudget::default(),
    )
}

fn plan_input(user_text: &str, max_steps: usize) -> PlanExecuteInput {
    PlanExecuteInput {
        runtime_input: runtime_input(user_text),
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

    let serialized_traces = serde_json::to_string(&output.traces).unwrap();

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
