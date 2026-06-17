use crate::agent::main_chat_agent_productization_v1::{
    assemble_main_chat_agent_state, main_chat_agent_product_scenarios,
    MainChatAgentProductScenarioRunMode, MainChatAgentProductStrategyRoute,
    MainChatAgentStateAssemblerInput, MainChatAgentStateEventType,
};
use crate::agent::main_chat_agent_v1::{
    ActionQueueStore, AgentTaskSessionDraft, AgentTaskSessionStatus, AgentTaskSessionStore,
    ExecutionAction, ExecutionPolicy, ExecutionPolicyDecision, ExecutionQueueStatus,
    ExecutionTranscriptEntryDraft, ExecutionTranscriptEntryKind, MainChatAgentStrategy,
    MainChatPolicyLevel,
};
use crate::agent::types::{
    AgentProposal, AgentRun, AgentRunStatus, AgentTaskKind, ContextSummary, ModelRouteTrace,
    ProposalSource, ProposalType, RedactionLevel, RiskLevel,
};

fn fixture_run(session_id: &str, output: &str) -> AgentRun {
    let mut run = AgentRun::new_chat_run(session_id, "productization fixture prompt");
    run.id = "run-productization-fixture".into();
    run.task_id = "task-productization-fixture".into();
    run.status = AgentRunStatus::Completed;
    run.output_preview = Some(output.into());
    run.model_route = Some(ModelRouteTrace {
        provider: "scripted_eval".into(),
        model: "productization-fixture".into(),
        route_type: "local".into(),
        prefer_local: true,
        local_model: "productization-fixture".into(),
        reason: "deterministic productization fixture".into(),
        privacy_level: RedactionLevel::LocalOnly,
        latency_ms: Some(1),
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(false),
    });
    run.context_summary = Some(ContextSummary {
        life_model_empty: true,
        included_life_model_sections: vec![],
        memory_hit_count: 0,
        memory_sources: vec!["ctx:workspace:AGENTS.md".into()],
        used_tools_prompt: false,
        redaction_applied: true,
        redaction_level: RedactionLevel::LocalOnly,
    });
    run.kind = AgentTaskKind::Conversation;
    run
}

#[test]
fn main_chat_agent_productization_v1_scenario_fixture_uses_canonical_routes() {
    let scenarios = main_chat_agent_product_scenarios();
    assert_eq!(
        scenarios.len(),
        93,
        "the product fixture must account for every row in the scenario inventory, including the opt-in live row"
    );

    let canonical_routes = MainChatAgentProductStrategyRoute::canonical_values();
    for scenario in &scenarios {
        assert!(
            canonical_routes.contains(&scenario.expected_strategy_route.as_str()),
            "{} used a non-canonical product strategy route",
            scenario.id
        );
        assert!(
            !scenario.required_ui_states.is_empty(),
            "{} must preserve visible UI state expectations",
            scenario.id
        );
        assert!(
            !scenario.required_runtime_evidence.is_empty(),
            "{} must preserve runtime evidence expectations",
            scenario.id
        );
        assert!(
            !scenario.negative_assertions.is_empty(),
            "{} must preserve at least one negative assertion",
            scenario.id
        );
    }

    let live = scenarios
        .iter()
        .find(|scenario| scenario.id == "WR-LIVE-01")
        .expect("WR-LIVE-01 fixture must exist");
    assert_eq!(
        live.run_mode,
        MainChatAgentProductScenarioRunMode::ExternalLiveOptIn
    );
    assert!(
        !live.included_in_default_gate,
        "external live scenarios must not count toward default deterministic product readiness"
    );

    let deterministic_default_count = scenarios
        .iter()
        .filter(|scenario| scenario.included_in_default_gate)
        .count();
    assert_eq!(deterministic_default_count, 92);
}

#[test]
fn main_chat_agent_productization_v1_task_control_scenarios_reference_prior_objects() {
    let scenarios = main_chat_agent_product_scenarios();
    let task_control = scenarios
        .iter()
        .filter(|scenario| {
            scenario.expected_strategy_route == MainChatAgentProductStrategyRoute::TaskControl
        })
        .collect::<Vec<_>>();

    assert!(
        task_control.len() >= 20,
        "fixture must not drop task_control rows from product acceptance"
    );

    for scenario in task_control {
        let preconditions = scenario.preconditions.as_ref().unwrap_or_else(|| {
            panic!(
                "{} task_control scenario must have explicit preconditions",
                scenario.id
            )
        });
        assert!(
            preconditions.prior_task_session_id.is_some(),
            "{} must reference a prior task/session id",
            scenario.id
        );
        assert!(
            preconditions.prior_run_id.is_some(),
            "{} must reference a prior run id",
            scenario.id
        );
        assert!(
            preconditions.target_action_id.is_some()
                || preconditions.target_proposal_id.is_some()
                || preconditions.target_blocker_id.is_some()
                || preconditions.target_final_delivery_id.is_some(),
            "{} must target an existing action, proposal, blocker, or final delivery",
            scenario.id
        );
        assert!(
            scenario.control_action.is_some(),
            "{} must declare the exact control action",
            scenario.id
        );
        assert!(
            scenario.expected_state_transition.is_some(),
            "{} must prove an exact state transition",
            scenario.id
        );
    }
}

#[test]
fn main_chat_agent_productization_v1_assembles_snapshot_and_ordered_events_from_runtime_evidence() {
    let session_store = AgentTaskSessionStore::new_in_memory().expect("session store");
    let action_queue = ActionQueueStore::new_in_memory().expect("action queue");
    let policy = ExecutionPolicy::default();

    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: "chat-productization".into(),
            user_goal: "Read the productization plan and summarize what happened.".into(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some("Read the plan, observe it, then synthesize.".into()),
            context_snapshot_refs: vec!["ctx:workspace:AGENTS.md".into()],
        })
        .expect("create session");

    let action = action_queue
        .enqueue(
            &session.id,
            ExecutionAction::new(
                "file.read",
                "plans/main_chat_agent_productization_v1_goal_spec.md",
            ),
            policy.classify(&ExecutionAction::new(
                "file.read",
                "plans/main_chat_agent_productization_v1_goal_spec.md",
            )),
        )
        .expect("enqueue action");
    session_store
        .record_action_queue_id(&session.id, &action.id)
        .expect("record action id");
    action_queue
        .transition(&action.id, ExecutionQueueStatus::Executing, None)
        .expect("running action");
    action_queue
        .transition(
            &action.id,
            ExecutionQueueStatus::Observed,
            Some(serde_json::json!({
                "sourceKind": "file",
                "sourceLabel": "plans/main_chat_agent_productization_v1_goal_spec.md",
                "preview": "Main Chat Agent Productization v1 requires runtime-backed UI evidence."
            })),
        )
        .expect("observed action");
    let completed_action = action_queue
        .transition(&action.id, ExecutionQueueStatus::Completed, None)
        .expect("completed action");

    session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::RouteDecision,
            summary: "Route selected ReAct.".into(),
            metadata: serde_json::json!({
                "selectedStrategy": "react_tool_execution",
                "reason": "tool-required read task"
            }),
        })
        .expect("route transcript");
    session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::Plan,
            summary: "Read then synthesize.".into(),
            metadata: serde_json::json!({ "planId": "plan-productization" }),
        })
        .expect("plan transcript");
    let observation = session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::Observation,
            summary: "Observed productization plan requirements.".into(),
            metadata: serde_json::json!({
                "actionId": completed_action.id,
                "sourceKind": "file",
                "sourceLabel": "plans/main_chat_agent_productization_v1_goal_spec.md",
                "preview": "Runtime payload/snapshot/event/evidence-gap gate is required.",
                "structuredResult": {
                    "readExecutionEvidence": {
                        "kind": "file_system_read",
                        "sourceKind": "file",
                        "sourceLabel": "plans/main_chat_agent_productization_v1_goal_spec.md",
                        "target": "plans/main_chat_agent_productization_v1_goal_spec.md",
                        "realReadOnlyExecution": true,
                        "fixtureBacked": false,
                        "networkReadAttempted": false,
                        "directWritesExecuted": false
                    }
                }
            }),
        })
        .expect("observation transcript");
    let final_entry = session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::FinalResult,
            summary: "I read the plan and summarized the runtime-backed requirements.".into(),
            metadata: serde_json::json!({
                "observationIds": [observation.id],
                "directWritesExecuted": false
            }),
        })
        .expect("final transcript");

    let mut proposal = AgentProposal::new(
        ProposalType::MemoryWrite,
        "memory.project.agent_productization_preference",
        serde_json::json!({"text": "Prefer execution-first Agent product behavior."}),
        "Fixture proposal with evidence.",
        0.8,
        RiskLevel::Low,
        ProposalSource::ChatConversation,
    );
    proposal.id = "proposal-productization-memory".into();
    proposal.run_id = Some("run-productization-fixture".into());
    proposal.source_detail = Some(session.id.clone());

    session_store
        .complete_session(&session.id, "Structured final delivery ready.")
        .expect("complete session");
    let session = session_store
        .load_session(&session.id)
        .expect("load session")
        .expect("session exists");
    assert_eq!(session.status, AgentTaskSessionStatus::Completed);

    let snapshot = assemble_main_chat_agent_state(MainChatAgentStateAssemblerInput {
        session,
        run: Some(fixture_run("chat-productization", &final_entry.summary)),
        transcript: session_store
            .list_transcript_entries(&action.session_id)
            .expect("transcript"),
        actions: action_queue
            .list_for_session(&action.session_id)
            .expect("actions"),
        proposals: vec![proposal],
    })
    .expect("assemble product state");

    assert_eq!(snapshot.task.task_id, action.session_id);
    assert_eq!(
        snapshot.route.strategy,
        MainChatAgentProductStrategyRoute::ReactToolExecution
    );
    assert_eq!(snapshot.actions.len(), 1);
    assert_eq!(snapshot.observations.len(), 1);
    let snapshot_json = serde_json::to_value(&snapshot).expect("serialize snapshot");
    assert_eq!(
        snapshot_json["observations"][0]["readExecution"]["kind"],
        "file_system_read",
        "agent_state observations must preserve real read-only execution evidence for the visible control plane"
    );
    assert_eq!(
        snapshot_json["observations"][0]["readExecution"]["realReadOnlyExecution"],
        true
    );
    assert_eq!(
        snapshot_json["observations"][0]["readExecution"]["fixtureBacked"],
        false
    );
    assert_eq!(snapshot.proposals.len(), 1);
    assert!(
        snapshot.final_delivery.is_some(),
        "completed runtime evidence must create a canonical final delivery object"
    );
    assert!(
        snapshot.diagnostics.is_empty(),
        "complete evidence should not produce evidence-gap diagnostics: {:?}",
        snapshot.diagnostics
    );

    let event_types = snapshot
        .events
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    for required in [
        MainChatAgentStateEventType::TaskCreated,
        MainChatAgentStateEventType::RouteSelected,
        MainChatAgentStateEventType::ContextSelected,
        MainChatAgentStateEventType::PlanUpdated,
        MainChatAgentStateEventType::ActionQueued,
        MainChatAgentStateEventType::ActionUpdated,
        MainChatAgentStateEventType::ObservationCreated,
        MainChatAgentStateEventType::ProposalCreated,
        MainChatAgentStateEventType::FinalDeliveryCreated,
        MainChatAgentStateEventType::TaskUpdated,
    ] {
        assert!(
            event_types.contains(&required),
            "snapshot events missing {required:?}: {event_types:?}"
        );
    }

    let mut previous = 0;
    for event in &snapshot.events {
        assert!(
            event.sequence > previous,
            "event sequence must be strictly monotonic"
        );
        previous = event.sequence;
    }
    assert_eq!(snapshot.sequence, previous);
}

#[test]
fn main_chat_agent_productization_v1_fails_closed_when_observation_lacks_action_evidence() {
    let session_store = AgentTaskSessionStore::new_in_memory().expect("session store");
    let mut session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: "chat-productization-gap".into(),
            user_goal: "Pretend a file was read from assistant text only.".into(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: None,
            context_snapshot_refs: vec![],
        })
        .expect("create session");
    session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::Observation,
            summary: "Assistant text says the file was read.".into(),
            metadata: serde_json::json!({
                "actionId": "missing-action",
                "sourceKind": "file",
                "sourceLabel": "plans/main_chat_agent_productization_v1_goal_spec.md"
            }),
        })
        .expect("fake observation transcript");
    let transcript = session_store
        .list_transcript_entries(&session.id)
        .expect("transcript");
    session.status = AgentTaskSessionStatus::Completed;
    session.final_summary = None;

    let snapshot = assemble_main_chat_agent_state(MainChatAgentStateAssemblerInput {
        session,
        run: None,
        transcript,
        actions: vec![],
        proposals: vec![],
    })
    .expect("assemble gap state");

    assert!(
        snapshot.actions.is_empty(),
        "no action card may be rendered without action evidence"
    );
    assert!(
        snapshot.observations.is_empty(),
        "observation cards require matching action evidence"
    );
    assert!(snapshot.final_delivery.is_none());
    let gap_codes = snapshot
        .diagnostics
        .iter()
        .map(|gap| gap.gap_code.as_str())
        .collect::<Vec<_>>();
    assert!(gap_codes.contains(&"missing_run_identity"));
    assert!(gap_codes.contains(&"missing_observation_evidence"));
    assert!(gap_codes.contains(&"missing_final_delivery"));
    assert!(
        snapshot
            .events
            .iter()
            .any(|event| event.event_type == MainChatAgentStateEventType::DiagnosticCreated),
        "evidence gaps must be emitted as ordered diagnostics events"
    );
}

#[test]
fn main_chat_agent_productization_v1_does_not_promote_assistant_text_to_runtime_objects() {
    let session_store = AgentTaskSessionStore::new_in_memory().expect("session store");
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: "chat-productization-fake-text".into(),
            user_goal:
                "Assistant text claims it read a file, made a proposal, and delivered final output."
                    .into(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: None,
            context_snapshot_refs: vec![],
        })
        .expect("create session");
    let session = session_store
        .complete_session(
            &session.id,
            "I read fake.file, observed the result, created proposal fake-proposal, and delivered.",
        )
        .expect("complete with fake assistant text");
    let snapshot = assemble_main_chat_agent_state(MainChatAgentStateAssemblerInput {
        session,
        run: None,
        transcript: Vec::new(),
        actions: Vec::new(),
        proposals: Vec::new(),
    })
    .expect("assemble fake text state");

    assert!(snapshot.actions.is_empty());
    assert!(snapshot.observations.is_empty());
    assert!(snapshot.proposals.is_empty());
    assert!(
        snapshot.final_delivery.is_none(),
        "final delivery requires runtime final-result/run evidence, not assistant text in a session summary"
    );
    let gap_codes = snapshot
        .diagnostics
        .iter()
        .map(|gap| gap.gap_code.as_str())
        .collect::<Vec<_>>();
    assert!(gap_codes.contains(&"missing_run_identity"));
    assert!(gap_codes.contains(&"missing_final_delivery"));
}

#[test]
fn main_chat_agent_productization_v1_links_tool_permission_proposal_to_pending_action() {
    let session_store = AgentTaskSessionStore::new_in_memory().expect("session store");
    let action_queue = ActionQueueStore::new_in_memory().expect("action queue");

    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: "chat-productization-permission".into(),
            user_goal: "Read a registered MCP resource after approval.".into(),
            selected_strategy: MainChatAgentStrategy::ReActToolExecution,
            current_plan_summary: Some(
                "Wait for permission, then replay exact read action.".into(),
            ),
            context_snapshot_refs: vec![],
        })
        .expect("create session");
    let action = action_queue
        .enqueue(
            &session.id,
            ExecutionAction::new("mcp.read_only", "registered_mcp://notes.read"),
            ExecutionPolicyDecision {
                level: MainChatPolicyLevel::L2ProposalFirst,
                reason_code: "tool_permission_required".into(),
                execution_allowed: false,
                requires_confirmation: true,
                requires_proposal: true,
                requires_blocker: true,
                silent_write_allowed: false,
            },
        )
        .expect("enqueue action");
    session_store
        .record_action_queue_id(&session.id, &action.id)
        .expect("record action id");
    let action = action_queue
        .transition(
            &action.id,
            ExecutionQueueStatus::PendingPermission,
            Some(serde_json::json!({
                "proposalId": "proposal-tool-permission-action-link",
                "permissionProposalCreated": true,
                "directWritesExecuted": false
            })),
        )
        .expect("pending permission action");
    session_store
        .mark_waiting_permission(&session.id)
        .expect("waiting permission");

    let mut proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        "tool_permission.registered_mcp.notes_read",
        serde_json::json!({
            "permission": "allow_once",
            "tool_name": "notes.read",
            "source": "registered_mcp",
            "risk_level": "medium",
            "action_type": "mcp_tool"
        }),
        "Allow exactly the pending registered MCP read action once.",
        0.83,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    proposal.id = "proposal-tool-permission-action-link".into();
    proposal.source_detail = Some(format!("main_chat_agent_task_session:{}", session.id));

    let session = session_store
        .load_session(&session.id)
        .expect("load session")
        .expect("session");
    let snapshot = assemble_main_chat_agent_state(MainChatAgentStateAssemblerInput {
        session,
        run: Some(fixture_run(
            "chat-productization-permission",
            "Permission is pending.",
        )),
        transcript: session_store
            .list_transcript_entries(&action.session_id)
            .expect("transcript"),
        actions: action_queue
            .list_for_session(&action.session_id)
            .expect("actions"),
        proposals: vec![proposal],
    })
    .expect("assemble state");

    let proposal = snapshot
        .proposals
        .iter()
        .find(|proposal| proposal.proposal_id == "proposal-tool-permission-action-link")
        .expect("proposal evidence");
    assert_eq!(
        proposal.action_ids,
        vec![action.id.clone()],
        "ToolPermission proposal evidence must expose the exact pending action it can approve"
    );
}
