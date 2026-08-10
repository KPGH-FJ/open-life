use crate::agent::runtime_strategy_contract::select_historical_runtime_strategy;
use crate::agent::{
    AgentExecutionBudget, AgentTask, AgentTaskKind, GovernanceDecisionKind, ProposalStore,
    RuntimeInput, RuntimePolicyContext, RuntimeStrategyKind, StrategySelectionInput,
};
use crate::layer::Layer;
use crate::llm::ChatMessage;

fn runtime_input(user_text: &str, tools_prompt: &str, local_only: bool) -> RuntimeInput {
    let policy_context = if local_only {
        RuntimePolicyContext::fail_closed()
    } else {
        let ingress = crate::agent::main_chat_agent_v1::AgentIngress::default().decide(
            "session-strategy",
            user_text,
            None,
            AgentTaskKind::Conversation,
        );
        RuntimePolicyContext::new(
            crate::llm::ProviderPolicyAuthorization::from_main_chat_ingress(&ingress).unwrap(),
            Vec::new(),
            true,
        )
    };
    RuntimeInput::from_agent_task(
        AgentTask {
            kind: AgentTaskKind::Conversation,
            session_id: "session-strategy".into(),
            user_text: user_text.into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: user_text.into(),
            }],
            layer: Layer::L2,
        },
        Some("memory context must stay out of strategy summary".into()),
        tools_prompt,
        policy_context,
        AgentExecutionBudget::default(),
    )
}

fn select(
    runtime_input: RuntimeInput,
    allow_planning: bool,
    local_model_available: bool,
) -> crate::agent::StrategySelection {
    select_historical_runtime_strategy(StrategySelectionInput {
        runtime_input,
        allow_planning,
        local_model_available,
    })
}

#[test]
fn simple_chat_selects_react() {
    let selection = select(
        runtime_input(
            "What should I focus on today?",
            "Available tools: memory.search",
            false,
        ),
        true,
        true,
    );

    assert_eq!(selection.kind, RuntimeStrategyKind::ReAct);
    assert_eq!(
        selection.metadata_safe_summary["reasonCode"],
        serde_json::json!("default_react")
    );
}

#[test]
fn runtime_strategy_selection_report_includes_metadata_safe_candidate_matrix() {
    let selection = select(
        runtime_input(
            "What should I focus on today without exposing alice@example.com?",
            "Available tools: email.send with full payloads",
            false,
        ),
        true,
        true,
    );

    assert_eq!(selection.report.report_kind, "strategy_selection_report");
    assert_eq!(
        selection.report.selected_strategy_kind,
        RuntimeStrategyKind::ReAct
    );
    assert_eq!(selection.report.selection_reason_code, "default_react");
    assert!(!selection.report.blocked);
    assert_eq!(selection.report.candidates.len(), 2);

    let react = selection
        .report
        .candidates
        .iter()
        .find(|candidate| candidate.strategy_kind == RuntimeStrategyKind::ReAct)
        .expect("ReAct candidate is reported");
    assert!(react.supported);
    assert_eq!(react.reason_code, "default_react");
    assert!(react.planning_allowed);
    assert!(react.local_model_available);
    assert!(react.has_policy_context);
    assert!(!react.blocked);
    assert!(!react.fallback);

    let plan_execute = selection
        .report
        .candidates
        .iter()
        .find(|candidate| candidate.strategy_kind == RuntimeStrategyKind::PlanExecute)
        .expect("PlanExecute candidate is reported");
    assert!(!plan_execute.supported);
    assert_eq!(plan_execute.reason_code, "no_planning_or_write_intent");

    let serialized = serde_json::to_string(&selection.report).unwrap();
    assert!(!serialized.contains("alice@example.com"));
    assert!(!serialized.contains("email.send"));
    assert!(!serialized.contains("full payloads"));
}

#[test]
fn planning_intent_selects_plan_execute_when_allowed() {
    let selection = select(
        runtime_input("Create a plan with steps for my afternoon.", "", false),
        true,
        true,
    );

    assert_eq!(selection.kind, RuntimeStrategyKind::PlanExecute);
    assert_eq!(
        selection.metadata_safe_summary["reasonCode"],
        serde_json::json!("planning_intent_allowed")
    );
}

#[test]
fn planning_intent_falls_back_to_react_when_planning_disabled() {
    let selection = select(
        runtime_input("Plan the steps for tomorrow.", "", false),
        false,
        true,
    );

    assert_eq!(selection.kind, RuntimeStrategyKind::ReAct);
    assert_eq!(
        selection.metadata_safe_summary["reasonCode"],
        serde_json::json!("planning_disabled_fallback")
    );
    assert!(selection
        .warnings
        .iter()
        .any(|warning| warning.contains("planning disabled")));
}

#[test]
fn write_like_intent_selects_plan_execute_but_does_not_execute() {
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let selection = select(
        runtime_input("Send Bob the draft and schedule a follow-up.", "", false),
        true,
        true,
    );

    assert_eq!(selection.kind, RuntimeStrategyKind::PlanExecute);
    assert_eq!(
        selection.metadata_safe_summary["reasonCode"],
        serde_json::json!("write_like_intent")
    );
    assert!(proposal_store
        .list_pending_proposals(10)
        .unwrap()
        .is_empty());
}

#[test]
fn broad_tools_prompt_does_not_force_plan_execute() {
    let selection = select(
        runtime_input(
            "What should I focus on today?",
            "Available tools: file.write, calendar.create_event, email.send",
            false,
        ),
        true,
        true,
    );
    let serialized_summary = selection.metadata_safe_summary.to_string();

    assert_eq!(selection.kind, RuntimeStrategyKind::ReAct);
    assert!(!serialized_summary.contains("calendar.create_event"));
    assert!(!serialized_summary.contains("email.send"));
}

#[test]
fn sensitive_local_only_without_local_model_returns_blocked_selection() {
    let selection = select(
        runtime_input(
            "Talk through a sensitive health topic.",
            "Available tools: memory.search",
            true,
        ),
        true,
        false,
    );
    let decision = selection.governance_decision.as_ref().unwrap();

    assert_eq!(decision.kind, GovernanceDecisionKind::Block);
    assert_eq!(
        decision.metadata_safe_summary["policyReasonCode"],
        serde_json::json!("local_only_model_unavailable")
    );
    assert_eq!(
        selection.metadata_safe_summary["governanceDecisionKind"],
        serde_json::json!("block")
    );
    assert!(selection.reason.contains("blocked"));
    assert!(selection.report.blocked);
    assert_eq!(selection.report.selection_reason_code, "governance_blocked");
    assert!(selection
        .report
        .candidates
        .iter()
        .any(|candidate| candidate.blocked));
}

#[test]
fn strategy_selection_summary_is_metadata_safe() {
    let selection = select(
        runtime_input(
            "Plan steps for Alice and email alice@example.com the full draft.",
            "Available tools: email.send with body payloads and file.update",
            false,
        ),
        true,
        true,
    );
    let summary = selection.metadata_safe_summary.as_object().unwrap();
    let serialized_summary = selection.metadata_safe_summary.to_string();

    assert_eq!(summary.len(), 6);
    assert!(summary.contains_key("selectedStrategyKind"));
    assert!(summary.contains_key("taskKind"));
    assert!(summary.contains_key("riskLevel"));
    assert!(summary.contains_key("hasPolicyContext"));
    assert!(summary.contains_key("governanceDecisionKind"));
    assert!(summary.contains_key("reasonCode"));
    assert!(!serialized_summary.contains("Alice"));
    assert!(!serialized_summary.contains("alice@example.com"));
    assert!(!serialized_summary.contains("full draft"));
    assert!(!serialized_summary.contains("email.send"));
    assert!(!serialized_summary.contains("file.update"));
    assert!(!serialized_summary.contains("memory context"));
}

#[test]
fn strategy_selector_does_not_mutate_runtime_or_stores() {
    let proposal_store = ProposalStore::new_in_memory().unwrap();
    let runtime_input = runtime_input("Update my calendar.", "", false);
    let original_user_text = runtime_input.task.user_text.clone();
    let original_tools_prompt = runtime_input.tools_prompt.clone();

    let selection = select(runtime_input.clone(), true, true);

    assert_eq!(selection.kind, RuntimeStrategyKind::PlanExecute);
    assert_eq!(runtime_input.task.user_text, original_user_text);
    assert_eq!(runtime_input.tools_prompt, original_tools_prompt);
    assert!(proposal_store
        .list_pending_proposals(10)
        .unwrap()
        .is_empty());
}
