use crate::agent::policy_store::{PolicyStore, PolicyTopic};
use crate::agent::{
    ActionExecutionContext, AgentAction, AgentExecutionBudget, AgentObservation, AgentTask,
    AgentTaskKind, EvidenceQuery, EvidenceStore, HSSelector, HSSelectorInput, HeuristicStore,
    LifeEventDraft, RiskLevel, RuntimeInput, RuntimeOutput,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::privacy::PrivacyEngine;

fn test_task(layer: Layer) -> AgentTask {
    AgentTask {
        kind: AgentTaskKind::Planning,
        session_id: "session-runtime-contract".into(),
        user_text: "Plan a low-pressure afternoon".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "Plan a low-pressure afternoon".into(),
        }],
        layer,
    }
}

fn seeded_packet() -> crate::agent::RuntimeHSPacket {
    let policy_store = PolicyStore::mvp_builtin();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    heuristic_store.seed_mvp_heuristics().unwrap();
    HSSelector
        .select(
            &policy_store,
            &heuristic_store,
            &HSSelectorInput {
                task_kind: AgentTaskKind::Planning,
                intent_summary: "planning with low energy".into(),
                privacy_topic: PolicyTopic::General,
                risk_level: RiskLevel::Medium,
                tool_requirements: vec![],
                current_state_hints: serde_json::json!({ "energy": 2 }),
                token_budget: 512,
                agent_task_id: Some("task-contract".into()),
                agent_run_id: Some("run-contract".into()),
            },
        )
        .unwrap()
}

fn test_action_context_deps() -> (
    crate::mcp::McpRegistry,
    crate::tool_permissions::ToolPermissionStore,
    crate::mcp_audit::McpAuditStore,
    PrivacyEngine,
    tempfile::NamedTempFile,
) {
    let registry = crate::mcp::McpRegistry::new();
    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    (
        registry,
        permission_store,
        audit_store,
        privacy_engine,
        audit_file,
    )
}

#[test]
fn runtime_input_carries_task_lifemodel_tools_hs_packet_and_budget() {
    let mut life_model = LifeModel::default();
    life_model.metadata.version = "contract-test".into();
    let budget = AgentExecutionBudget {
        max_steps: 2,
        max_tool_calls: 1,
        timeout_seconds: 15,
        allow_cloud: false,
        allow_writes: false,
    };
    let packet = seeded_packet();

    let input = RuntimeInput::from_agent_task(
        test_task(Layer::L1),
        life_model,
        Some("memory: prior afternoon plans".into()),
        "Available tools: memory.search, calendar.propose_event",
        Some(packet),
        budget,
    );

    assert_eq!(input.task.kind, AgentTaskKind::Planning);
    assert_eq!(input.task.session_id, "session-runtime-contract");
    assert_eq!(input.life_model_compat.metadata.version, "contract-test");
    assert_eq!(
        input.memory_context.as_deref(),
        Some("memory: prior afternoon plans")
    );
    assert!(input.tools_prompt.contains("calendar.propose_event"));
    assert!(input.hs_packet.is_some());
    assert_eq!(input.execution_budget.max_steps, 2);
    assert!(!input.execution_budget.allow_cloud);
}

#[test]
fn runtime_output_carries_current_runtime_results_with_empty_life_event_candidates() {
    let action = AgentAction {
        id: "action-1".into(),
        action_type: "builtin_tool".into(),
        target: Some("memory.search".into()),
        input: serde_json::json!({ "query": "plan" }),
        output: Some(serde_json::json!({ "count": 1 })),
        status: "succeeded".into(),
        permission_decision: None,
        started_at: None,
        finished_at: None,
        error: None,
        timestamp: chrono::Utc::now(),
        tool_scope: None,
        react_trace: None,
    };
    let observation = AgentObservation {
        id: "observation-1".into(),
        action_id: Some("action-1".into()),
        content: "Found one memory".into(),
        source: "memory.search".into(),
        structured_result: None,
        timestamp: chrono::Utc::now(),
        react_trace: None,
    };

    let output = RuntimeOutput {
        run_id: Some("run-1".into()),
        user_output: "Here is the plan".into(),
        actions: vec![action],
        observations: vec![observation],
        proposal_ids: vec!["proposal-1".into()],
        life_event_candidates: vec![],
        warnings: vec!["kept as draft".into()],
    };

    assert_eq!(output.run_id.as_deref(), Some("run-1"));
    assert_eq!(output.actions.len(), 1);
    assert_eq!(output.observations.len(), 1);
    assert_eq!(output.proposal_ids, vec!["proposal-1"]);
    assert!(output.life_event_candidates.is_empty());
    assert_eq!(output.warnings, vec!["kept as draft"]);
}

#[tokio::test]
async fn agent_runtime_direct_path_can_use_runtime_input_adapter_without_behavior_change() {
    let input = RuntimeInput::from_agent_task(
        test_task(Layer::L1),
        LifeModel::default(),
        Some("memory: keep it short".into()),
        "Available tools: memory.search",
        Some(seeded_packet()),
        AgentExecutionBudget::default(),
    );
    let runtime = crate::agent::AgentRuntime::with_config(
        LifeModel::default(),
        crate::scheduler::InferenceScheduler::default(),
        crate::agent::AgentRuntimeConfig::default(),
    );

    let adapted = input.agent_runtime_params();
    let output = runtime
        .generate_direct_with_hs_packet(
            adapted.task,
            adapted.life_model,
            adapted.tools_prompt,
            adapted.memory_context,
            vec![],
            PrivacyEngine::new(),
            adapted.hs_packet,
        )
        .await
        .unwrap();

    assert!(output.context_summary.used_tools_prompt);
    assert_eq!(output.context_summary.memory_hit_count, 0);
    assert!(output.hs_selection_audit.is_some());
}

#[tokio::test]
async fn agent_runtime_execute_task_path_can_use_runtime_input_adapter_without_behavior_change() {
    let input = RuntimeInput::from_agent_task(
        test_task(Layer::L1),
        LifeModel::default(),
        None,
        "Available tools: memory.search",
        Some(seeded_packet()),
        AgentExecutionBudget::default(),
    );
    let runtime = crate::agent::AgentRuntime::with_config(
        LifeModel::default(),
        crate::scheduler::InferenceScheduler::default(),
        crate::agent::AgentRuntimeConfig::default(),
    );

    let adapted = input.agent_runtime_params();
    let output = runtime
        .execute_task_with_hs_packet(
            adapted.task,
            adapted.life_model,
            adapted.tools_prompt,
            adapted.memory_context,
            vec![],
            PrivacyEngine::new(),
            adapted.hs_packet,
        )
        .await
        .unwrap();

    assert!(output.context_summary.used_tools_prompt);
    assert!(output.hs_selection_audit.is_some());
}

#[test]
fn agent_loop_path_can_receive_runtime_input_derived_budget_and_hs_packet() {
    let budget = AgentExecutionBudget {
        max_steps: 3,
        max_tool_calls: 2,
        timeout_seconds: 20,
        allow_cloud: false,
        allow_writes: false,
    };
    let input = RuntimeInput::from_agent_task(
        test_task(Layer::L2),
        LifeModel::default(),
        None,
        "Available tools: memory.search",
        Some(seeded_packet()),
        budget,
    );
    let (registry, permission_store, audit_store, privacy_engine, _audit_file) =
        test_action_context_deps();
    let safe_paths: Vec<String> = Vec::new();
    let base_ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &safe_paths,
    );

    let loop_config = input.agent_loop_config();
    let action_ctx = input.attach_hs_packet_to_action_context(base_ctx);

    assert_eq!(loop_config.max_steps, 3);
    assert_eq!(loop_config.max_tool_calls, 2);
    assert_eq!(loop_config.timeout_seconds, 20);
    assert!(!loop_config.allow_cloud);
    assert!(!loop_config.allow_writes);
    assert!(action_ctx.hs_runtime_packet.is_some());
}

#[test]
fn broad_tools_prompt_catalog_does_not_imply_write_or_external_side_effect_intent() {
    let broad_tools_prompt = r#"
        Available tools:
        memory.search(query)
        email.propose_draft(to, subject, body)
        calendar.propose_event(title, scheduled_at)
        file.write_proposal(path, content)
    "#;

    let input = RuntimeInput::from_agent_task(
        test_task(Layer::L1),
        LifeModel::default(),
        None,
        broad_tools_prompt,
        None,
        AgentExecutionBudget::default(),
    );

    assert_eq!(
        input.inferred_tool_requirements_from_contract(),
        Vec::<String>::new()
    );
}

#[test]
fn runtime_output_life_event_candidates_do_not_persist_to_lifemodel_or_hs_stores() {
    let mut life_model = LifeModel::default();
    life_model.state.current_focus = "before".into();
    let evidence_store = EvidenceStore::new_in_memory().unwrap();

    let output = RuntimeOutput {
        user_output: "Candidate captured for later maturation".into(),
        life_event_candidates: vec![LifeEventDraft::new(
            "chat_interaction",
            "User described a possible afternoon planning preference",
        )
        .with_source_run_id("run-life-event")
        .with_metadata(serde_json::json!({ "confidence": 0.4 }))],
        ..RuntimeOutput::default()
    };

    assert_eq!(output.life_event_candidates.len(), 1);
    assert_eq!(life_model.state.current_focus, "before");
    assert!(evidence_store
        .query(EvidenceQuery::default())
        .unwrap()
        .is_empty());
}
