use crate::agent::policy_store::{ModelRoutePolicy, BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY};
use crate::agent::{
    AgentExecutionBudget, AgentTask, AgentTaskKind, GovernanceDecisionKind, HSSelectionAudit,
    ProposalStore, RuntimeHSPacket, RuntimeInput, RuntimeStrategyKind, SelectedPolicyRef,
    StrategySelectionInput, StrategySelector,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;

fn runtime_input(
    user_text: &str,
    tools_prompt: &str,
    hs_packet: Option<RuntimeHSPacket>,
) -> RuntimeInput {
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
        LifeModel::default(),
        Some("memory context must stay out of strategy summary".into()),
        tools_prompt,
        hs_packet,
        AgentExecutionBudget::default(),
    )
}

fn select(
    runtime_input: RuntimeInput,
    allow_planning: bool,
    local_model_available: bool,
) -> crate::agent::StrategySelection {
    StrategySelector::default().select(StrategySelectionInput {
        runtime_input,
        allow_planning,
        local_model_available,
    })
}

fn sensitive_packet() -> RuntimeHSPacket {
    RuntimeHSPacket {
        selected_policies: vec![SelectedPolicyRef {
            policy_id: BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
            reason: "sensitive_topic_route".into(),
            route: Some(ModelRoutePolicy::LocalOnly),
            digest: "digest-sensitive".into(),
        }],
        selected_heuristics: Vec::new(),
        estimated_tokens: 12,
        audit: HSSelectionAudit {
            agent_task_id: Some("task-strategy".into()),
            agent_run_id: Some("run-strategy".into()),
            input_digest: "digest-input".into(),
            selected_policy_ids: vec![BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into()],
            selected_heuristic_ids: Vec::new(),
            excluded_assets: Vec::new(),
            estimated_tokens: 12,
            token_budget: 128,
        },
    }
}

#[test]
fn simple_chat_selects_react() {
    let selection = select(
        runtime_input(
            "What should I focus on today?",
            "Available tools: memory.search",
            None,
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
fn planning_intent_selects_plan_execute_when_allowed() {
    let selection = select(
        runtime_input("Create a plan with steps for my afternoon.", "", None),
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
        runtime_input("Plan the steps for tomorrow.", "", None),
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
        runtime_input("Send Bob the draft and schedule a follow-up.", "", None),
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
            None,
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
            Some(sensitive_packet()),
        ),
        true,
        false,
    );
    let decision = selection.governance_decision.as_ref().unwrap();

    assert_eq!(decision.kind, GovernanceDecisionKind::Block);
    assert_eq!(
        decision.metadata_safe_summary["policyReasonCode"],
        serde_json::json!("sensitive_local_only_no_local_model")
    );
    assert_eq!(
        selection.metadata_safe_summary["governanceDecisionKind"],
        serde_json::json!("block")
    );
    assert!(selection.reason.contains("blocked"));
}

#[test]
fn strategy_selection_summary_is_metadata_safe() {
    let selection = select(
        runtime_input(
            "Plan steps for Alice and email alice@example.com the full draft.",
            "Available tools: email.send with body payloads and file.update",
            None,
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
    assert!(summary.contains_key("hasHsPacket"));
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
    let mut runtime_input = runtime_input("Update my calendar.", "", None);
    runtime_input.life_model_compat.metadata.version = "strategy-test".into();
    let original_user_text = runtime_input.task.user_text.clone();
    let original_life_model_version = runtime_input.life_model_compat.metadata.version.clone();
    let original_tools_prompt = runtime_input.tools_prompt.clone();

    let selection = select(runtime_input.clone(), true, true);

    assert_eq!(selection.kind, RuntimeStrategyKind::PlanExecute);
    assert_eq!(runtime_input.task.user_text, original_user_text);
    assert_eq!(
        runtime_input.life_model_compat.metadata.version,
        original_life_model_version
    );
    assert_eq!(runtime_input.tools_prompt, original_tools_prompt);
    assert!(proposal_store
        .list_pending_proposals(10)
        .unwrap()
        .is_empty());
}
