use crate::agent::hs_selector::{
    build_runtime_hs_packet, HSSelector, HSSelectorInput, RuntimeHSPacket,
    RuntimeHSPacketBuildInput,
};
use crate::agent::policy_store::{
    PolicyStore, PolicyTopic, BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
    BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST,
};
use crate::agent::{
    ActionExecutionContext, ActionExecutor, ActionExecutorConfig, AgentActionRequest, AgentRun,
    AgentRunStore, AgentRuntime, AgentRuntimeConfig, AgentTask, AgentTaskKind,
    HSBehaviorCheckSummary, HeuristicStore, ModelRouter, ProviderAvailability, RiskLevel, TaskType,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;
use crate::tool_manifest::{ToolManifest, ToolSource};
use crate::tool_permissions::ToolPermissionPolicy;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

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

#[test]
fn hs_runtime_packet_builder_selects_metadata_safe_assets_for_real_task_inputs() {
    let policy_store = PolicyStore::mvp_builtin();
    let heuristic_store = HeuristicStore::new_in_memory().unwrap();
    heuristic_store.seed_mvp_heuristics().unwrap();
    let task = AgentTask {
        kind: AgentTaskKind::Planning,
        session_id: "session-builder".into(),
        user_text: "raw-health-and-energy-note-456".into(),
        messages: vec![],
        layer: Layer::L1,
    };

    let packet = build_runtime_hs_packet(
        &policy_store,
        &heuristic_store,
        RuntimeHSPacketBuildInput {
            task: &task,
            sanitized_intent_summary: "planning request with sensitive topic".into(),
            privacy_topic: PolicyTopic::Health,
            risk_level: RiskLevel::Medium,
            tool_requirements: vec!["write".into()],
            current_state_hints: serde_json::json!({ "energy": 2 }),
            token_budget: 256,
            agent_run_id: Some("run-builder".into()),
        },
    )
    .unwrap()
    .expect("sensitive planning task should select HS assets");

    assert!(packet
        .selected_policies
        .iter()
        .any(|policy| policy.route == Some(crate::agent::ModelRoutePolicy::LocalOnly)));
    assert!(packet
        .selected_heuristics
        .iter()
        .any(|heuristic| heuristic.heuristic_id == BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING));
    assert!(packet.guidance_refs.is_empty());
    assert_eq!(packet.audit.agent_run_id.as_deref(), Some("run-builder"));

    let audit_json = serde_json::to_string(&packet.audit).unwrap();
    assert!(!audit_json.contains("raw-health-and-energy-note-456"));
    assert!(!audit_json.contains("Reduce planning intensity"));
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
async fn hs_runtime_packet_keeps_seeded_heuristics_out_of_runtime_guidance_prompt_by_default() {
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
    assert!(!system_prompt.contains("gentle_planning"));
    assert!(!system_prompt.contains("Reduce planning intensity"));
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

#[tokio::test]
async fn agent_runtime_execute_task_can_receive_hs_packet_on_real_path() {
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
        session_id: "session-runtime-main".into(),
        user_text: "raw-main-runtime-text-123".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "Plan my day with low energy".into(),
        }],
        layer: Layer::L1,
    };

    let output = runtime
        .execute_task_with_hs_packet(
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
    assert!(!system_prompt.contains("gentle_planning"));
    assert!(!system_prompt.contains("Reduce planning intensity"));
    assert!(output.hs_selection_audit.is_some());

    let audit_json = serde_json::to_string(&output.hs_selection_audit).unwrap();
    assert!(!audit_json.contains("raw-main-runtime-text-123"));
}

#[test]
fn agent_run_store_persists_metadata_safe_hs_audit_and_behavior_checks() {
    let packet = seeded_packet(
        AgentTaskKind::ToolExecution,
        PolicyTopic::General,
        serde_json::json!({}),
        vec!["write".into()],
    );
    let store = AgentRunStore::new_in_memory().unwrap();
    let mut run = AgentRun::new_chat_run("session-hs-store", "raw user text stays out of audit");
    run.hs_selection_audit = Some(packet.audit.clone());
    run.behavior_checks = vec![HSBehaviorCheckSummary {
        id: "regression.external_write_proposal_first".into(),
        label: "External writes stay reviewable".into(),
        passed: true,
        summary: Some("Direct writes become proposals.".into()),
    }];

    store.create_run(&run).unwrap();
    let fetched = store.get_run(&run.id).unwrap().unwrap();

    let audit = fetched.hs_selection_audit.expect("audit should persist");
    assert!(audit
        .selected_policy_ids
        .contains(&BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.to_string()));
    assert_eq!(fetched.behavior_checks.len(), 1);
    assert_eq!(
        fetched.behavior_checks[0].label,
        "External writes stay reviewable"
    );

    let serialized = serde_json::to_string(&serde_json::json!({
        "audit": audit,
        "behaviorChecks": fetched.behavior_checks,
    }))
    .unwrap();
    assert!(!serialized.contains("raw user text stays out of audit"));
    assert!(!serialized.contains("external write action must create"));
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

#[test]
fn calendar_propose_event_creates_scheduled_task_never_external_write_action() {
    let registry = crate::mcp::McpRegistry::new();
    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &[],
    )
    .with_proposal_store(&proposal_store);

    let result = ActionExecutor::new(ActionExecutorConfig::default())
        .execute(
            AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "calendar.propose_event".into(),
                input: serde_json::json!({
                    "arguments": {
                        "title": "Doctor follow-up",
                        "scheduled_at": "2026-06-05T09:00:00Z",
                        "description": "Review results"
                    }
                }),
                source_run_id: Some("run-calendar-proposal".into()),
                step_index: 0,
            },
            &ctx,
        )
        .unwrap();

    assert_eq!(
        result.status,
        crate::agent::ActionExecutionStatus::Succeeded
    );
    let scheduled = proposal_store
        .list_proposals_filtered(
            None,
            Some(crate::agent::ProposalType::ScheduledTask),
            None,
            10,
        )
        .unwrap();
    assert_eq!(scheduled.len(), 1);
    assert_eq!(
        scheduled[0].after["tool"],
        serde_json::json!("calendar.propose_event")
    );
    assert_eq!(
        scheduled[0].after["title"],
        serde_json::json!("Doctor follow-up")
    );

    let external = proposal_store
        .list_proposals_filtered(
            None,
            Some(crate::agent::ProposalType::ExternalWriteAction),
            None,
            10,
        )
        .unwrap();
    assert!(external.is_empty());
}

#[test]
fn email_propose_draft_creates_data_export_never_external_write_action() {
    let registry = crate::mcp::McpRegistry::new();
    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &[],
    )
    .with_proposal_store(&proposal_store);

    let result = ActionExecutor::new(ActionExecutorConfig::default())
        .execute(
            AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "email.propose_draft".into(),
                input: serde_json::json!({
                    "arguments": {
                        "to": "team@example.com",
                        "subject": "Weekly notes",
                        "body": "Draft body"
                    }
                }),
                source_run_id: Some("run-email-proposal".into()),
                step_index: 0,
            },
            &ctx,
        )
        .unwrap();

    assert_eq!(
        result.status,
        crate::agent::ActionExecutionStatus::Succeeded
    );
    let data_exports = proposal_store
        .list_proposals_filtered(None, Some(crate::agent::ProposalType::DataExport), None, 10)
        .unwrap();
    assert_eq!(data_exports.len(), 1);
    assert_eq!(
        data_exports[0].after["tool"],
        serde_json::json!("email.propose_draft")
    );
    assert_eq!(
        data_exports[0].after["body"],
        serde_json::json!("Draft body")
    );

    let external = proposal_store
        .list_proposals_filtered(
            None,
            Some(crate::agent::ProposalType::ExternalWriteAction),
            None,
            10,
        )
        .unwrap();
    assert!(external.is_empty());
}

#[test]
fn file_write_proposal_rejects_oversized_content_before_proposal_insertion() {
    let registry = crate::mcp::McpRegistry::new();
    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let safe_dir = tempfile::TempDir::new().unwrap();
    let safe_path = safe_dir.path().to_str().unwrap().to_string();
    let safe_paths = vec![safe_path];
    let file_path = safe_dir.path().join("too-large.txt");
    let oversized_content = "x".repeat(100 * 1024 + 1);
    let ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &safe_paths,
    )
    .with_proposal_store(&proposal_store);

    let result = ActionExecutor::new(ActionExecutorConfig::default())
        .execute(
            AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "file.write_proposal".into(),
                input: serde_json::json!({
                    "arguments": {
                        "path": file_path.to_string_lossy().to_string(),
                        "content": oversized_content
                    }
                }),
                source_run_id: Some("run-oversized-file-proposal".into()),
                step_index: 0,
            },
            &ctx,
        )
        .unwrap();

    assert_eq!(result.status, crate::agent::ActionExecutionStatus::Failed);
    assert!(result
        .action
        .error
        .as_deref()
        .is_some_and(|error| error.contains("exceeds maximum allowed")));
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[test]
fn hs_direct_external_write_rejects_oversized_content_before_proposal_insertion() {
    let packet = seeded_packet(
        AgentTaskKind::ToolExecution,
        PolicyTopic::General,
        serde_json::json!({}),
        vec!["write".into()],
    );
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
    let safe_paths = vec![safe_path];
    let oversized_content = "x".repeat(100 * 1024 + 1);
    let ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &safe_paths,
    )
    .with_proposal_store(&proposal_store)
    .with_hs_runtime_packet(&packet);

    let err = ActionExecutor::new(ActionExecutorConfig::default())
        .execute(
            AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "file.write".into(),
                input: serde_json::json!({
                    "arguments": {
                        "path": safe_dir.path().join("too-large-direct.txt").to_string_lossy().to_string(),
                        "content": oversized_content
                    }
                }),
                source_run_id: Some("run-oversized-direct-write".into()),
                step_index: 0,
            },
            &ctx,
        )
        .unwrap_err();

    assert!(err.to_string().contains("exceeds maximum allowed"));
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[test]
fn hs_wrapped_mcp_external_write_rejects_oversized_content_before_proposal_insertion() {
    let packet = seeded_packet(
        AgentTaskKind::ToolExecution,
        PolicyTopic::General,
        serde_json::json!({}),
        vec!["write".into()],
    );
    let mut registry = crate::mcp::McpRegistry::new();
    registry.register_builtin(
        ToolManifest {
            id: "wrapped.file.write".into(),
            name: "wrapped.file.write".into(),
            description: "Wrapped direct file write test executor".into(),
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
            tags: vec![],
        },
        Box::new(|_| Ok("wrapped direct write should not run".into())),
    );

    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    permission_store
        .grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let safe_dir = tempfile::TempDir::new().unwrap();
    let safe_path = safe_dir.path().to_str().unwrap().to_string();
    let safe_paths = vec![safe_path];
    let oversized_content = "x".repeat(100 * 1024 + 1);
    let ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &safe_paths,
    )
    .with_proposal_store(&proposal_store)
    .with_hs_runtime_packet(&packet);

    let result = ActionExecutor::new(ActionExecutorConfig::default())
        .execute(
            AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "mcp.call_tool".into(),
                input: serde_json::json!({
                    "arguments": {
                        "tool_name": "wrapped.file.write",
                        "arguments": {
                            "path": safe_dir.path().join("too-large-wrapped.txt").to_string_lossy().to_string(),
                            "content": oversized_content
                        }
                    }
                }),
                source_run_id: Some("run-oversized-wrapped-write".into()),
                step_index: 0,
            },
            &ctx,
        )
        .unwrap();

    assert_eq!(result.status, crate::agent::ActionExecutionStatus::Failed);
    assert!(result
        .action
        .error
        .as_deref()
        .is_some_and(|error| error.contains("exceeds maximum allowed")));
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[test]
fn hs_external_write_policy_overrides_allow_until_revoked_and_skips_executor() {
    let packet = seeded_packet(
        AgentTaskKind::ToolExecution,
        PolicyTopic::General,
        serde_json::json!({}),
        vec!["write".into()],
    );

    let executor_ran = Arc::new(AtomicBool::new(false));
    let executor_ran_for_tool = executor_ran.clone();
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
            tags: vec![],
        },
        Box::new(move |_| {
            executor_ran_for_tool.store(true, Ordering::SeqCst);
            Ok("direct write should not run".into())
        }),
    );

    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    permission_store
        .grant(
            "file.write",
            "builtin",
            "high",
            "write",
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let safe_dir = tempfile::TempDir::new().unwrap();
    let safe_path = safe_dir.path().to_str().unwrap().to_string();
    let file_path = safe_dir.path().join("allowed-but-proposal-first.txt");

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
                        "path": file_path.to_string_lossy().to_string(),
                        "content": "hello from HS proposal-first"
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
    assert!(!executor_ran.load(Ordering::SeqCst));
    assert!(!file_path.exists());

    let proposals = proposal_store
        .list_proposals_filtered(
            None,
            Some(crate::agent::ProposalType::ExternalWriteAction),
            None,
            10,
        )
        .unwrap();
    assert_eq!(proposals.len(), 1);
    assert_eq!(
        proposals[0].after["content"],
        serde_json::json!("hello from HS proposal-first")
    );
}

#[test]
fn hs_external_write_policy_intercepts_mcp_call_tool_target_even_when_allowed() {
    let packet = seeded_packet(
        AgentTaskKind::ToolExecution,
        PolicyTopic::General,
        serde_json::json!({}),
        vec!["write".into()],
    );

    let executor_ran = Arc::new(AtomicBool::new(false));
    let executor_ran_for_tool = executor_ran.clone();
    let mut registry = crate::mcp::McpRegistry::new();
    registry.register_builtin(
        ToolManifest {
            id: "wrapped.file.write".into(),
            name: "wrapped.file.write".into(),
            description: "Wrapped direct file write test executor".into(),
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
            tags: vec![],
        },
        Box::new(move |_| {
            executor_ran_for_tool.store(true, Ordering::SeqCst);
            Ok("wrapped direct write should not run".into())
        }),
    );

    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    permission_store
        .grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
    permission_store
        .grant(
            "wrapped.file.write",
            "builtin",
            "high",
            "write",
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();

    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let safe_dir = tempfile::TempDir::new().unwrap();
    let safe_path = safe_dir.path().to_str().unwrap().to_string();
    let file_path = safe_dir
        .path()
        .join("wrapped-allowed-but-proposal-first.txt");
    let content = "hello from wrapped HS proposal-first";

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
                target: "mcp.call_tool".into(),
                input: serde_json::json!({
                    "arguments": {
                        "tool_name": "wrapped.file.write",
                        "arguments": {
                            "path": file_path.to_string_lossy().to_string(),
                            "content": content,
                            "body": "raw body should not be duplicated",
                            "data": "raw data should not be duplicated"
                        }
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
    assert_eq!(
        result.stop_reason.as_deref(),
        Some("hs_external_write_proposal_first")
    );
    assert!(!executor_ran.load(Ordering::SeqCst));
    assert!(!file_path.exists());

    let proposals = proposal_store
        .list_proposals_filtered(
            None,
            Some(crate::agent::ProposalType::ExternalWriteAction),
            None,
            10,
        )
        .unwrap();
    assert_eq!(proposals.len(), 1);
    let after = &proposals[0].after;
    assert_eq!(after["tool_name"], serde_json::json!("wrapped.file.write"));
    assert_eq!(after["source"], serde_json::json!("builtin"));
    assert_eq!(after["server"], serde_json::Value::Null);
    assert!(after["arguments"].get("content").is_none());
    assert!(after["arguments"].get("body").is_none());
    assert!(after["arguments"].get("data").is_none());
    assert_eq!(after["content"], serde_json::json!(content));
    assert_eq!(after["risk_level"], serde_json::json!("high"));
    assert_eq!(after["action_type"], serde_json::json!("write"));
    assert_eq!(
        after["capabilities"],
        serde_json::json!(["filesystem", "write"])
    );
    assert!(after["content_hash"]
        .as_str()
        .is_some_and(|hash| hash.len() == 64));
    assert_eq!(after["size_bytes"], serde_json::json!(content.len()));
}
