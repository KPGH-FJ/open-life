use crate::agent::hs_selector::{HSSelector, HSSelectorInput, RuntimeHSPacket};
use crate::agent::policy_store::{
    PolicyStore, PolicyTopic, BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
    BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST,
};
use crate::agent::{
    ActionExecutionContext, ActionExecutor, ActionExecutorConfig, AgentActionRequest, AgentRuntime,
    AgentRuntimeConfig, AgentTask, AgentTaskKind, HeuristicStore, ModelRouter,
    ProviderAvailability, RiskLevel, TaskType,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;
use crate::tool_manifest::{ToolManifest, ToolSource};

fn seeded_packet(
    task_kind: AgentTaskKind,
    topic: PolicyTopic,
    state: serde_json::Value,
    tool_requirements: Vec<String>,
) -> RuntimeHSPacket {
    let policy_store = PolicyStore::mvp_builtin();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    heuristic_store.seed_mvp_heuristics().unwrap();
    HSSelector::default()
        .select(
            &policy_store,
            &heuristic_store,
            &HSSelectorInput {
                task_kind,
                intent_summary: "sanitized runtime integration scenario".into(),
                privacy_topic: topic,
                risk_level: RiskLevel::Medium,
                tool_requirements,
                current_state_hints: state,
                token_budget: 512,
                agent_task_id: Some("task-runtime".into()),
                agent_run_id: Some("run-runtime".into()),
            },
        )
        .unwrap()
}

fn test_router(ollama_available: bool, cloud_available: bool) -> ModelRouter {
    let mut router = ModelRouter::new();
    router.providers.insert(
        "ollama".into(),
        ProviderAvailability {
            provider: "ollama".into(),
            available: ollama_available,
            latency_ms: Some(100),
            models: vec!["local-model".into()],
            last_checked: chrono::Utc::now(),
            last_error: None,
            health_is_estimated: false,
        },
    );
    router.providers.insert(
        "deepseek".into(),
        ProviderAvailability {
            provider: "deepseek".into(),
            available: cloud_available,
            latency_ms: Some(400),
            models: vec!["deepseek-chat".into()],
            last_checked: chrono::Utc::now(),
            last_error: None,
            health_is_estimated: false,
        },
    );
    router
}

#[test]
fn hs_policy_forces_model_router_local_only_and_fails_closed_without_local() {
    let packet = seeded_packet(
        AgentTaskKind::Conversation,
        PolicyTopic::Health,
        serde_json::json!({}),
        vec![],
    );

    let local_decision = test_router(true, true)
        .route_with_hs_packet(TaskType::Chat, false, &packet)
        .unwrap();
    assert_eq!(local_decision.provider, "ollama");
    assert_eq!(
        local_decision.privacy_level,
        crate::agent::RedactionLevel::LocalOnly
    );

    let no_local = test_router(false, true).route_with_hs_packet(TaskType::Chat, false, &packet);
    assert!(no_local.is_err());
}

#[tokio::test]
async fn hs_runtime_packet_adds_bounded_guidance_and_metadata_safe_audit() {
    let packet = seeded_packet(
        AgentTaskKind::Planning,
        PolicyTopic::General,
        serde_json::json!({ "energy": 2 }),
        vec![],
    );
    let runtime = AgentRuntime::with_config(
        LifeModel::default(),
        InferenceScheduler::default(),
        AgentRuntimeConfig::default(),
    );
    let task = AgentTask {
        kind: AgentTaskKind::Planning,
        session_id: "session-runtime".into(),
        user_text: "raw-private-planning-text-789".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "Plan my day while tired".into(),
        }],
        layer: Layer::L1,
    };

    let output = runtime
        .generate_direct_with_hs_packet(
            &task,
            &LifeModel::default(),
            "",
            None,
            vec![],
            PrivacyEngine::new(),
            Some(packet),
        )
        .await
        .unwrap();

    let system_prompt = output
        .final_messages
        .iter()
        .find(|message| message.role == "system")
        .map(|message| message.content.as_str())
        .unwrap_or("");
    assert!(system_prompt.contains("Reduce planning intensity"));
    assert!(output
        .hs_selection_audit
        .as_ref()
        .unwrap()
        .selected_heuristic_ids
        .contains(&BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING.to_string()));

    let audit_json = serde_json::to_string(&output.hs_selection_audit).unwrap();
    assert!(!audit_json.contains("raw-private-planning-text-789"));
    assert!(!audit_json.contains("Reduce planning intensity"));
}

#[test]
fn hs_external_write_policy_converts_direct_write_to_proposal_first() {
    let packet = seeded_packet(
        AgentTaskKind::ToolExecution,
        PolicyTopic::General,
        serde_json::json!({}),
        vec!["write".into()],
    );
    assert!(packet
        .selected_policies
        .iter()
        .any(|policy| policy.policy_id == BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST));

    let mut registry = crate::mcp::McpRegistry::new();
    registry.register_builtin(
        ToolManifest {
            id: "file.write".into(),
            name: "file.write".into(),
            description: "Direct file write test executor".into(),
            parameters: serde_json::json!({ "type": "object" }),
            permission_level: "high".into(),
            risk_level: "high".into(),
            version: "1.0.0".into(),
            source: ToolSource::BuiltIn,
            capabilities: vec!["filesystem".into(), "write".into()],
            requires_confirmation: true,
            enabled: true,
            declarative_only: false,
            action_type: "write".into(),
            tags: vec!["execution".into()],
        },
        Box::new(|_| Ok("direct write should not run".into())),
    );

    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let safe_dir = tempfile::TempDir::new().unwrap();
    let safe_path = safe_dir.path().to_str().unwrap().to_string();

    let ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[safe_path],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: Some(&proposal_store),
        agent_run_store: None,
        network_policy: None,
        hs_runtime_packet: Some(&packet),
    };

    let result = ActionExecutor::new(ActionExecutorConfig::default())
        .execute(
            AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "file.write".into(),
                input: serde_json::json!({
                    "arguments": {
                        "path": safe_dir.path().join("out.txt").to_str().unwrap(),
                        "content": "hello"
                    }
                }),
                source_run_id: Some("run-runtime".into()),
                step_index: 0,
            },
            &ctx,
        )
        .unwrap();

    assert_eq!(
        result.status,
        crate::agent::ActionExecutionStatus::NeedsConfirmation
    );
    let proposals = proposal_store
        .list_proposals_filtered(
            None,
            Some(crate::agent::ProposalType::ExternalWriteAction),
            None,
            10,
        )
        .unwrap();
    assert_eq!(proposals.len(), 1);
}
