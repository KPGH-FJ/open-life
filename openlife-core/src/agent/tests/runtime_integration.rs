use crate::agent::hs_selector::{
    build_runtime_hs_packet, HSSelector, HSSelectorInput, RuntimeHSPacket,
    RuntimeHSPacketBuildInput,
};
use crate::agent::policy_store::{
    PolicyStore, PolicyTopic, BUILTIN_HEURISTIC_LOW_ENERGY_PLANNING,
    BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST, BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY,
};
use crate::agent::{
    ActionExecutionContext, ActionExecutorConfig, AgentActionRequest, AgentRun, AgentRunStore,
    AgentTask, AgentTaskKind, GovernanceDecisionClassification, HSBehaviorCheckSummary,
    HSSelectionAudit, HeuristicStore, ModelRouter, ProviderAvailability, RiskLevel, TaskType,
    ToolGateway,
};
use crate::layer::Layer;
use crate::privacy::PrivacyEngine;
use crate::tool_manifest::{ToolIdempotencyContract, ToolManifest, ToolSource};
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
    HSSelector
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

fn canonical_tool_owner() -> (AgentRunStore, String) {
    let store = AgentRunStore::new_in_memory().unwrap();
    let run = AgentRun::new_tool_execution_run("runtime-integration");
    let run_id = run.id.clone();
    store.create_run(&run).unwrap();
    (store, run_id)
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

fn test_router_with_latencies(
    ollama_available: bool,
    cloud_available: bool,
    ollama_latency_ms: u64,
    cloud_latency_ms: u64,
) -> ModelRouter {
    let mut router = ModelRouter::new();
    router.providers.insert(
        "ollama".into(),
        ProviderAvailability {
            provider: "ollama".into(),
            available: ollama_available,
            latency_ms: Some(ollama_latency_ms),
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
            latency_ms: Some(cloud_latency_ms),
            models: vec!["deepseek-chat".into()],
            last_checked: chrono::Utc::now(),
            last_error: None,
            health_is_estimated: false,
        },
    );
    router
}

fn test_router(ollama_available: bool, cloud_available: bool) -> ModelRouter {
    test_router_with_latencies(ollama_available, cloud_available, 100, 400)
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

#[test]
fn hs_model_router_enforces_local_only_from_audit_ids_and_removes_cloud_fallback() {
    let packet = RuntimeHSPacket {
        selected_policies: Vec::new(),
        selected_heuristics: Vec::new(),
        guidance_refs: Vec::new(),
        estimated_tokens: 8,
        audit: HSSelectionAudit {
            agent_task_id: Some("task-route-audit-only".into()),
            agent_run_id: Some("run-route-audit-only".into()),
            input_digest: "digest-route-audit-only".into(),
            selected_policy_ids: vec![BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into()],
            selected_heuristic_ids: Vec::new(),
            selected_guidance_ids: Vec::new(),
            selected_guidance_refs: Vec::new(),
            excluded_assets: Vec::new(),
            estimated_tokens: 8,
            token_budget: 128,
        },
        provider_authorization: crate::llm::ProviderPolicyAuthorization::local_only_fail_closed(
            crate::llm::ProviderLocalOnlyReason::TestFixture,
        ),
    };

    let decision = test_router_with_latencies(true, true, 5_000, 10)
        .route_with_hs_packet(TaskType::ToolUse, true, &packet)
        .unwrap();

    assert_eq!(decision.provider, "ollama");
    assert_eq!(decision.route_type, "local");
    assert!(decision.prefer_local);
    assert_eq!(
        decision.privacy_level,
        crate::agent::RedactionLevel::LocalOnly
    );
    assert_eq!(decision.fallback_provider, None);
    assert_eq!(decision.fallback_model, None);
    let report = decision
        .governance_report
        .as_ref()
        .expect("HS route should include metadata-safe governor report");
    assert_eq!(
        report.classification,
        GovernanceDecisionClassification::LocalOnly
    );
    assert!(report.requires_local_only);
    assert!(report
        .selected_policy_ids
        .contains(&BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.to_string()));
    let report_json = serde_json::to_string(report).unwrap();
    assert!(!report_json.contains("raw prompt"));
    assert!(!report_json.contains("raw user text"));
    assert!(!report_json.contains("raw assistant output"));
    assert!(!report_json.contains("raw memory"));
    assert!(!report_json.contains("raw LifeModel"));
    assert!(!report_json.contains("raw tool payload"));
}

#[test]
fn hs_model_router_fails_closed_when_local_only_audit_id_has_no_local_model() {
    let packet = RuntimeHSPacket {
        selected_policies: Vec::new(),
        selected_heuristics: Vec::new(),
        guidance_refs: Vec::new(),
        estimated_tokens: 8,
        audit: HSSelectionAudit {
            agent_task_id: Some("task-route-no-local".into()),
            agent_run_id: Some("run-route-no-local".into()),
            input_digest: "digest-route-no-local".into(),
            selected_policy_ids: vec![BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into()],
            selected_heuristic_ids: Vec::new(),
            selected_guidance_ids: Vec::new(),
            selected_guidance_refs: Vec::new(),
            excluded_assets: Vec::new(),
            estimated_tokens: 8,
            token_budget: 128,
        },
        provider_authorization: crate::llm::ProviderPolicyAuthorization::local_only_fail_closed(
            crate::llm::ProviderLocalOnlyReason::TestFixture,
        ),
    };

    let result = test_router_with_latencies(false, true, 5_000, 10).route_with_hs_packet(
        TaskType::Planner,
        true,
        &packet,
    );

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("local-only policy selected but no local model is available"));
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
    assert!(packet
        .audit
        .selected_policy_ids
        .contains(&BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.to_string()));
    assert_eq!(audit.selected_policy_ids.len(), 1);
    assert!(audit.selected_policy_ids[0].starts_with("policy_id:bytes="));
    assert!(!audit
        .selected_policy_ids
        .contains(&BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.to_string()));
    assert_eq!(fetched.behavior_checks.len(), 1);
    assert_eq!(
        run.behavior_checks[0].label,
        "External writes stay reviewable"
    );
    assert!(fetched.behavior_checks[0]
        .label
        .starts_with("behavior_check_label:bytes="));

    let serialized = serde_json::to_string(&serde_json::json!({
        "audit": audit,
        "behaviorChecks": fetched.behavior_checks,
    }))
    .unwrap();
    assert!(!serialized.contains("raw user text stays out of audit"));
    assert!(!serialized.contains("external write action must create"));
}

#[tokio::test]
async fn hs_external_write_policy_converts_direct_write_to_proposal_first() {
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
            idempotency_contract: ToolIdempotencyContract::NonIdempotent,
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
        canonical_state: None,
        memory_store: None,
        memory_lifecycle_retrieval_reader: None,
        proposal_store: Some(&proposal_store),
        agent_run_store: None,
        bound_content_receipt_issuer: None,
        network_policy: None,
        web_search_fixture_output: None,
        external_write_requires_proposal: true,
        tool_dispatch_observer: None,
        tool_started_transition_observer: None,
        tool_audit_persistence_observer: None,
        durable_store_failure_observer: None,
        a2a_outbound_authorization: None,
        action_bound_tool_permission: None,
        canonical_write_admission: Some(
            &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
        ),
    };

    let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
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
        .await
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

#[tokio::test]
async fn unsupported_plugin_tool_is_blocked_before_permission_replay_or_execution() {
    let mut registry = crate::mcp::McpRegistry::new();
    registry.register_builtin(
        ToolManifest {
            id: "plugin.demo.write".into(),
            name: "plugin.demo.write".into(),
            description: "Plugin write hook without governed executor".into(),
            parameters: serde_json::json!({ "type": "object" }),
            permission_level: "high".into(),
            risk_level: "high".into(),
            version: "1.0.0".into(),
            source: ToolSource::Plugin {
                plugin_id: "demo".into(),
            },
            capabilities: vec!["write".into(), "external_side_effect".into()],
            requires_confirmation: true,
            enabled: true,
            declarative_only: false,
            action_type: "write".into(),
            idempotency_contract: ToolIdempotencyContract::NonIdempotent,
            tags: vec![],
        },
        Box::new(|_| Ok("unsupported plugin executor must not run".into())),
    );
    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    permission_store
        .grant(
            "plugin.demo.write",
            "plugin:demo",
            "high",
            "write",
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &[],
    );

    let result = ToolGateway::from_executor_config(ActionExecutorConfig {
        consume_allow_once: false,
        ..Default::default()
    })
    .execute(
        AgentActionRequest {
            action_type: "plugin_tool".into(),
            target: "plugin.demo.write".into(),
            input: serde_json::json!({
                "arguments": {
                    "body": "raw plugin payload must not appear in governance report"
                }
            }),
            source_run_id: Some("run-plugin-block".into()),
            step_index: 0,
        },
        &ctx,
    )
    .await
    .unwrap();

    assert_eq!(result.status, crate::agent::ActionExecutionStatus::Blocked);
    assert_eq!(
        result.stop_reason.as_deref(),
        Some("tool_gateway_source_executor_unavailable")
    );
    assert!(result.governance_report.is_none());
    assert!(!serde_json::to_string(&result.execution_receipt)
        .unwrap()
        .contains("raw plugin payload"));
}

#[tokio::test]
async fn calendar_propose_event_creates_scheduled_task_never_external_write_action() {
    let registry = crate::mcp::McpRegistry::new();
    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let (agent_run_store, run_id) = canonical_tool_owner();
    let ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &[],
    )
    .with_proposal_store(&proposal_store)
    .with_agent_run_store(&agent_run_store)
    .with_canonical_write_admission(
        &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
    );

    let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
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
                source_run_id: Some(run_id.clone()),
                step_index: 0,
            },
            &ctx,
        )
        .await
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

#[tokio::test]
async fn email_propose_draft_creates_data_export_never_external_write_action() {
    let registry = crate::mcp::McpRegistry::new();
    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let (agent_run_store, run_id) = canonical_tool_owner();
    let ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &[],
    )
    .with_proposal_store(&proposal_store)
    .with_agent_run_store(&agent_run_store)
    .with_canonical_write_admission(
        &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
    );

    let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
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
                source_run_id: Some(run_id.clone()),
                step_index: 0,
            },
            &ctx,
        )
        .await
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

#[tokio::test]
async fn file_write_proposal_rejects_oversized_content_before_proposal_insertion() {
    let registry = crate::mcp::McpRegistry::new();
    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let (agent_run_store, run_id) = canonical_tool_owner();
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
    .with_proposal_store(&proposal_store)
    .with_agent_run_store(&agent_run_store)
    .with_canonical_write_admission(
        &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
    );

    let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
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
                source_run_id: Some(run_id.clone()),
                step_index: 0,
            },
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(result.status, crate::agent::ActionExecutionStatus::Failed);
    assert!(result
        .action
        .error
        .as_deref()
        .is_some_and(|error| error.contains("exceeds maximum allowed")));
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[tokio::test]
async fn typed_policy_direct_external_write_rejects_oversized_content_before_proposal_insertion() {
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
            idempotency_contract: ToolIdempotencyContract::NonIdempotent,
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
    .with_canonical_write_admission(
        &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
    )
    .with_external_write_proposal_policy(true);

    let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
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
        .await
        .unwrap();

    assert_eq!(result.status, crate::agent::ActionExecutionStatus::Failed);
    assert!(result
        .action
        .error
        .as_deref()
        .is_some_and(|error| error.contains("exceeds maximum allowed")));
    assert_eq!(
        result.execution_receipt.execution_outcome,
        crate::tool_execution_receipt::ToolExecutionOutcome::Failed
    );
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[tokio::test]
async fn typed_policy_wrapped_external_write_rejects_oversized_content_before_proposal_insertion() {
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
            idempotency_contract: ToolIdempotencyContract::NonIdempotent,
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
    let (agent_run_store, run_id) = canonical_tool_owner();
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
    .with_agent_run_store(&agent_run_store)
    .with_canonical_write_admission(
        &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
    )
    .with_external_write_proposal_policy(true);

    let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
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
                source_run_id: Some(run_id.clone()),
                step_index: 0,
            },
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(result.status, crate::agent::ActionExecutionStatus::Failed);
    assert!(result
        .action
        .error
        .as_deref()
        .is_some_and(|error| error.contains("exceeds maximum allowed")));
    assert_eq!(proposal_store.pending_count().unwrap(), 0);
}

#[tokio::test]
async fn typed_policy_external_write_overrides_allow_until_revoked_and_skips_executor() {
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
            idempotency_contract: ToolIdempotencyContract::NonIdempotent,
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
        canonical_state: None,
        memory_store: None,
        memory_lifecycle_retrieval_reader: None,
        proposal_store: Some(&proposal_store),
        agent_run_store: None,
        bound_content_receipt_issuer: None,
        network_policy: None,
        web_search_fixture_output: None,
        external_write_requires_proposal: true,
        tool_dispatch_observer: None,
        tool_started_transition_observer: None,
        tool_audit_persistence_observer: None,
        durable_store_failure_observer: None,
        a2a_outbound_authorization: None,
        action_bound_tool_permission: None,
        canonical_write_admission: Some(
            &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
        ),
    };

    let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
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
        .await
        .unwrap();

    assert_eq!(
        result.status,
        crate::agent::ActionExecutionStatus::NeedsConfirmation
    );
    let report = result
        .governance_report
        .as_ref()
        .expect("HS proposal-first write should include governor report");
    assert_eq!(
        report.classification,
        GovernanceDecisionClassification::ProposalFirst
    );
    assert!(report.requires_proposal);
    assert!(!report.raw_tool_payload_included);
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

#[tokio::test]
async fn typed_policy_intercepts_mcp_call_tool_target_even_when_allowed() {
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
            idempotency_contract: ToolIdempotencyContract::NonIdempotent,
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
    let (agent_run_store, run_id) = canonical_tool_owner();
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
        canonical_state: None,
        memory_store: None,
        memory_lifecycle_retrieval_reader: None,
        proposal_store: Some(&proposal_store),
        agent_run_store: Some(&agent_run_store),
        bound_content_receipt_issuer: Some(&agent_run_store),
        network_policy: None,
        web_search_fixture_output: None,
        external_write_requires_proposal: true,
        tool_dispatch_observer: None,
        tool_started_transition_observer: None,
        tool_audit_persistence_observer: None,
        durable_store_failure_observer: None,
        a2a_outbound_authorization: None,
        action_bound_tool_permission: None,
        canonical_write_admission: Some(
            &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
        ),
    };

    let result = ToolGateway::from_executor_config(ActionExecutorConfig::default())
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
                source_run_id: Some(run_id.clone()),
                step_index: 0,
            },
            &ctx,
        )
        .await
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
