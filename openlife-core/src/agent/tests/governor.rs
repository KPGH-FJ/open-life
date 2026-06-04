use crate::agent::policy_store::{ModelRoutePolicy, BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY};
use crate::agent::{
    AgentExecutionBudget, AgentTask, AgentTaskKind, GovernanceDecisionKind, HSSelectionAudit,
    LifeModelGovernor, MaturationProposalCandidate, ModelRouteGovernanceInput, ProposalType,
    RiskLevel, RuntimeHSPacket, RuntimeInput, SelectedPolicyRef, ToolGovernanceInput,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;

fn candidate(
    proposal_type: ProposalType,
    risk_level: RiskLevel,
    proposal_only: bool,
) -> MaturationProposalCandidate {
    MaturationProposalCandidate {
        proposal_type,
        affected_path: "/identity/values".into(),
        payload: serde_json::json!({
            "summary": "raw life event summary must not be copied",
            "content": "raw user_text raw assistant_output alice@example.com",
        }),
        reason: "raw user_text raw assistant_output should not be copied".into(),
        confidence: 0.91,
        risk_level,
        source_run_id: Some("run-governor".into()),
        source_event_type: "identity.values".into(),
        proposal_only,
    }
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
        guidance_refs: Vec::new(),
        estimated_tokens: 12,
        audit: HSSelectionAudit {
            agent_task_id: Some("task-governor".into()),
            agent_run_id: Some("run-governor".into()),
            input_digest: "digest-input".into(),
            selected_policy_ids: vec![BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into()],
            selected_heuristic_ids: Vec::new(),
            selected_guidance_ids: Vec::new(),
            selected_guidance_refs: Vec::new(),
            excluded_assets: Vec::new(),
            estimated_tokens: 12,
            token_budget: 128,
        },
    }
}

fn runtime_input(tools_prompt: &str) -> RuntimeInput {
    RuntimeInput::from_agent_task(
        AgentTask {
            kind: AgentTaskKind::Conversation,
            session_id: "session-governor".into(),
            user_text: "raw prompt should not become governance metadata".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "raw prompt should not become governance metadata".into(),
            }],
            layer: Layer::L2,
        },
        LifeModel::default(),
        None,
        tools_prompt,
        None,
        AgentExecutionBudget::default(),
    )
}

#[test]
fn high_risk_lifemodel_candidate_requires_confirmation() {
    let governor = LifeModelGovernor::default();
    let decision = governor.govern_maturation_candidate(&candidate(
        ProposalType::LifeModelUpdate,
        RiskLevel::High,
        true,
    ));

    assert_eq!(decision.kind, GovernanceDecisionKind::RequireConfirmation);
    assert_eq!(decision.risk_level, RiskLevel::High);
    assert_eq!(
        decision.metadata_safe_summary["proposalType"],
        serde_json::json!("life_model_update")
    );
}

#[test]
fn memory_write_candidate_requires_proposal() {
    let governor = LifeModelGovernor::default();
    let decision = governor.govern_maturation_candidate(&candidate(
        ProposalType::MemoryWrite,
        RiskLevel::Low,
        true,
    ));

    assert_eq!(decision.kind, GovernanceDecisionKind::RequireProposal);
    assert_eq!(decision.risk_level, RiskLevel::Low);
}

#[test]
fn proposal_only_false_candidate_is_blocked() {
    let governor = LifeModelGovernor::default();
    let decision = governor.govern_maturation_candidate(&candidate(
        ProposalType::PreferenceUpdate,
        RiskLevel::Medium,
        false,
    ));

    assert_eq!(decision.kind, GovernanceDecisionKind::Block);
    assert!(decision.reason.contains("proposal_only=false"));
}

#[test]
fn external_write_tool_requires_proposal() {
    let governor = LifeModelGovernor::default();
    let decision = governor.govern_tool_action(ToolGovernanceInput {
        tool_name: "calendar.create_event".into(),
        action_kind: "write".into(),
        risk_level: RiskLevel::Medium,
        declared_write: true,
    });

    assert_eq!(decision.kind, GovernanceDecisionKind::RequireProposal);
    assert_eq!(decision.risk_level, RiskLevel::Medium);
}

#[test]
fn read_only_tool_is_allowed() {
    let governor = LifeModelGovernor::default();
    let decision = governor.govern_tool_action(ToolGovernanceInput {
        tool_name: "memory.search".into(),
        action_kind: "search".into(),
        risk_level: RiskLevel::Low,
        declared_write: false,
    });

    assert_eq!(decision.kind, GovernanceDecisionKind::Allow);
}

#[test]
fn broad_tools_prompt_does_not_create_write_governance_decision() {
    let governor = LifeModelGovernor::default();
    let input = runtime_input("Available tools: file.write, calendar.create_event, email.send");
    let decision = governor.govern_runtime_input(&input, true);

    assert_eq!(decision.kind, GovernanceDecisionKind::Allow);
    assert!(!decision
        .metadata_safe_summary
        .to_string()
        .contains("calendar.create_event"));
}

#[test]
fn sensitive_runtime_requires_local_only() {
    let governor = LifeModelGovernor::default();
    let decision = governor.govern_model_route(ModelRouteGovernanceInput {
        hs_packet: Some(sensitive_packet()),
        risk_level: RiskLevel::High,
        local_model_available: true,
    });

    assert_eq!(decision.kind, GovernanceDecisionKind::RequireLocalOnly);
    assert_eq!(
        decision.metadata_safe_summary["policyReasonCode"],
        serde_json::json!("sensitive_local_only")
    );
}

#[test]
fn governance_decision_summary_is_metadata_safe() {
    let governor = LifeModelGovernor::default();
    let decision = governor.govern_maturation_candidate(&candidate(
        ProposalType::LifeModelUpdate,
        RiskLevel::High,
        true,
    ));
    let serialized = serde_json::to_string(&decision).unwrap();

    assert!(serialized.contains("maturation_candidate"));
    assert!(!serialized.contains("raw user_text"));
    assert!(!serialized.contains("raw assistant_output"));
    assert!(!serialized.contains("raw life event summary"));
    assert!(!serialized.contains("alice@example.com"));
}
