use crate::agent::{
    evaluate_react_beta_execution_readiness, evaluate_react_beta_execution_readiness_for_input,
    evaluate_tool_registry_beta_readiness, ActionExecutionContext, ActionExecutionStatus,
    ActionExecutor, ActionExecutorConfig, AgentActionRequest, ReactBetaExecutionReadinessInput,
    ReactBetaReadinessComponentOverride,
};
use crate::agent::{AgentTaskKind, PolicyTopic, RiskLevel};
use crate::mcp::McpRegistry;
use crate::mcp_audit::McpAuditStore;
use crate::privacy::PrivacyEngine;
use crate::tool_manifest::{ToolManifest, ToolSource};
use crate::tool_permissions::{ToolPermissionPolicy, ToolPermissionStore};

fn test_context<'a>(
    registry: &'a McpRegistry,
    permission_store: &'a ToolPermissionStore,
    audit_store: &'a McpAuditStore,
    privacy_engine: &'a PrivacyEngine,
    safe_paths: &'a [String],
) -> ActionExecutionContext<'a> {
    ActionExecutionContext::new(
        registry,
        permission_store,
        audit_store,
        privacy_engine,
        safe_paths,
    )
}

#[test]
fn react_beta_readiness_report_is_metadata_safe_and_never_migration_permission() {
    let report = evaluate_react_beta_execution_readiness();

    assert_eq!(report.report_kind, "react_beta_execution_readiness");
    assert!(report.react_loop_present);
    assert!(report.runtime_strategy_ready);
    assert!(report.default_chat_unchanged);
    assert!(!report.migration_permission);
    assert!(report.metadata_safe);

    let serialized = serde_json::to_string(&report).unwrap();
    for raw in [
        "raw prompt",
        "raw assistant output",
        "raw tool payload",
        "memory context",
        "LifeModel text",
        "secret@example.com",
    ] {
        assert!(
            !serialized.contains(raw),
            "readiness report leaked raw marker: {raw}"
        );
    }
}

#[test]
fn react_beta_readiness_fails_closed_when_required_component_missing() {
    let input = ReactBetaExecutionReadinessInput {
        action_schema: ReactBetaReadinessComponentOverride::Blocked(
            "missing_action_schema_contract".into(),
        ),
        tool_registry: ReactBetaReadinessComponentOverride::Ready,
        permission_replay: ReactBetaReadinessComponentOverride::Ready,
        ..ReactBetaExecutionReadinessInput::current()
    };

    let report = evaluate_react_beta_execution_readiness_for_input(input);

    assert!(!report.ready);
    assert!(!report.action_schema_ready);
    assert!(report
        .blocking_reasons
        .contains(&"missing_action_schema_contract".to_string()));
    assert!(!report.migration_permission);
}

#[test]
fn react_beta_tool_registry_taxonomy_covers_required_tools_and_plugin_stubs_not_executable() {
    let registry = McpRegistry::new();
    let report = evaluate_tool_registry_beta_readiness(&registry);

    assert!(report.ready, "{:?}", report.blocking_reasons);
    assert!(report.metadata_safe);
    assert!(report
        .required_tool_ids
        .contains(&"life_model.read".to_string()));
    assert!(report
        .required_tool_ids
        .contains(&"email.propose_draft".to_string()));

    let email_read = report.tool("email.read").expect("email.read covered");
    assert_eq!(email_read.actual_state, "declarative_only");
    assert!(!email_read.executable);

    let calendar_event = report
        .tool("calendar.propose_event")
        .expect("calendar proposal tool covered");
    assert_eq!(calendar_event.actual_state, "proposal_only");
    assert_eq!(
        calendar_event.proposal_type.as_deref(),
        Some("scheduled_task")
    );

    let email_draft = report
        .tool("email.propose_draft")
        .expect("email draft proposal tool covered");
    assert_eq!(email_draft.actual_state, "proposal_only");
    assert_eq!(email_draft.proposal_type.as_deref(), Some("data_export"));

    let serialized = serde_json::to_string(&report).unwrap();
    assert!(!serialized.contains("raw tool payload"));
    assert!(!serialized.contains("secret@example.com"));

    let mut bad_registry = McpRegistry::new();
    bad_registry.register_builtin(
        ToolManifest {
            id: "plugin.demo.write".into(),
            name: "plugin.demo.write".into(),
            description: "Bad plugin executor".into(),
            parameters: serde_json::json!({ "type": "object" }),
            permission_level: "high".into(),
            risk_level: "high".into(),
            version: "1.0.0".into(),
            source: ToolSource::Plugin {
                plugin_id: "demo".into(),
            },
            capabilities: vec!["write".into()],
            requires_confirmation: true,
            enabled: true,
            declarative_only: false,
            action_type: "write".into(),
            tags: vec![],
        },
        Box::new(|_| Ok("must not execute".into())),
    );

    let bad = evaluate_tool_registry_beta_readiness(&bad_registry);
    assert!(!bad.ready);
    assert!(bad
        .blocking_reasons
        .iter()
        .any(|reason| reason.contains("plugin_tool_executable_without_executor")));
}

#[test]
fn react_beta_executor_blocks_unknown_and_allow_writes_false_direct_write() {
    let mut registry = McpRegistry::new();
    registry.register_builtin(
        ToolManifest {
            id: "external.write".into(),
            name: "external.write".into(),
            description: "Direct external write test".into(),
            parameters: serde_json::json!({ "type": "object" }),
            permission_level: "high".into(),
            risk_level: "high".into(),
            version: "1.0.0".into(),
            source: ToolSource::BuiltIn,
            capabilities: vec!["write".into(), "external_side_effect".into()],
            requires_confirmation: true,
            enabled: true,
            declarative_only: false,
            action_type: "external_side_effect".into(),
            tags: vec![],
        },
        Box::new(|_| Ok("direct write should not execute".into())),
    );
    let permission_store = ToolPermissionStore::new_in_memory().unwrap();
    permission_store
        .grant(
            "external.write",
            "builtin",
            "high",
            "external_side_effect",
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let safe_paths = vec![];
    let ctx = test_context(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &safe_paths,
    );

    let unknown = ActionExecutor::new(ActionExecutorConfig::default())
        .execute(
            AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "unknown.tool".into(),
                input: serde_json::json!({"arguments": {"secret": "raw-secret-123"}}),
                source_run_id: Some("run-react-beta-unknown".into()),
                step_index: 0,
            },
            &ctx,
        )
        .unwrap();
    assert_eq!(unknown.status, ActionExecutionStatus::Blocked);
    assert!(unknown.action.tool_scope.is_none());
    assert!(!serde_json::to_string(&unknown.action.react_trace)
        .unwrap()
        .contains("raw-secret"));

    let blocked = ActionExecutor::new(ActionExecutorConfig {
        allow_writes: false,
        ..Default::default()
    })
    .execute(
        AgentActionRequest {
            action_type: "mcp_tool".into(),
            target: "external.write".into(),
            input: serde_json::json!({"arguments": {"body": "raw-write-payload-456"}}),
            source_run_id: Some("run-react-beta-write-disabled".into()),
            step_index: 1,
        },
        &ctx,
    )
    .unwrap();
    assert_eq!(blocked.status, ActionExecutionStatus::Blocked);
    assert_eq!(
        blocked.action.react_trace.as_ref().unwrap().status,
        "blocked"
    );
    assert_eq!(
        blocked
            .observation
            .react_trace
            .as_ref()
            .unwrap()
            .action_category,
        "external_side_effect"
    );
}

#[test]
fn react_beta_permission_proposal_scope_uses_hash_not_raw_payload() {
    let registry = McpRegistry::new();
    let permission_store = ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
    let safe_dir = tempfile::TempDir::new().unwrap();
    let safe_path = safe_dir.path().to_str().unwrap().to_string();
    let target_path = safe_dir
        .path()
        .join("out.txt")
        .to_string_lossy()
        .to_string();
    let safe_paths = vec![safe_path];
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
                        "path": target_path,
                        "content": "raw-file-write-secret-789"
                    }
                }),
                source_run_id: Some("run-react-beta-permission".into()),
                step_index: 0,
            },
            &ctx,
        )
        .unwrap();

    assert_eq!(result.status, ActionExecutionStatus::Succeeded);
    assert!(result
        .action
        .react_trace
        .as_ref()
        .unwrap()
        .proposal_id
        .is_some());
    let trace_json = serde_json::to_string(&result.action.react_trace).unwrap();
    assert!(trace_json.contains("sha256:"));
    assert!(!trace_json.contains("raw-file-write-secret-789"));
}

#[test]
fn react_beta_trace_envelope_serializes_without_raw_output_or_pii() {
    let registry = McpRegistry::new();
    let permission_store = ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let safe_paths = vec![];
    let ctx = test_context(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &safe_paths,
    );

    let result = ActionExecutor::new(ActionExecutorConfig::default())
        .execute(
            AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "builtin_echo".into(),
                input: serde_json::json!({"arguments": {"text": "secret@example.com raw-output"}}),
                source_run_id: Some("run-react-beta-trace".into()),
                step_index: 7,
            },
            &ctx,
        )
        .unwrap();

    assert_eq!(result.status, ActionExecutionStatus::Succeeded);
    let trace = result.action.react_trace.as_ref().unwrap();
    assert_eq!(trace.run_id.as_deref(), Some("run-react-beta-trace"));
    assert_eq!(trace.step_index, 7);
    assert_eq!(trace.tool_name, "builtin_echo");
    assert_eq!(trace.status, "succeeded");
    assert!(trace.output_hash.as_deref().unwrap().starts_with("sha256:"));

    let serialized_trace = serde_json::to_string(trace).unwrap();
    assert!(!serialized_trace.contains("secret@example.com"));
    assert!(!serialized_trace.contains("raw-output"));
}

#[test]
fn react_beta_readiness_current_tool_registry_can_be_blocked_explicitly() {
    let input = ReactBetaExecutionReadinessInput {
        tool_registry: ReactBetaReadinessComponentOverride::Blocked(
            "required_tool_missing:memory.search".into(),
        ),
        ..ReactBetaExecutionReadinessInput::current()
    };
    let report = evaluate_react_beta_execution_readiness_for_input(input);

    assert!(!report.ready);
    assert!(!report.tool_registry_ready);
    assert!(report
        .blocking_reasons
        .contains(&"required_tool_missing:memory.search".to_string()));
}

#[test]
fn react_beta_readiness_permission_replay_fails_closed() {
    let input = ReactBetaExecutionReadinessInput {
        permission_replay: ReactBetaReadinessComponentOverride::Blocked(
            "permission_replay_not_canonical".into(),
        ),
        ..ReactBetaExecutionReadinessInput::current()
    };
    let report = evaluate_react_beta_execution_readiness_for_input(input);

    assert!(!report.ready);
    assert!(!report.permission_replay_ready);
    assert!(report
        .blocking_reasons
        .contains(&"permission_replay_not_canonical".to_string()));
}

#[test]
fn react_beta_runtime_policy_packet_marks_proposal_first_writes_ready() {
    let packet = {
        let policy_store = crate::agent::PolicyStore::mvp_builtin();
        let heuristic_store = crate::agent::HeuristicStore::new_in_memory().unwrap();
        heuristic_store.seed_mvp_heuristics().unwrap();
        crate::agent::HSSelector::default()
            .select(
                &policy_store,
                &heuristic_store,
                &crate::agent::HSSelectorInput {
                    task_kind: AgentTaskKind::ToolExecution,
                    intent_summary: "metadata-safe write request".into(),
                    privacy_topic: PolicyTopic::General,
                    risk_level: RiskLevel::Medium,
                    tool_requirements: vec!["write".into()],
                    current_state_hints: serde_json::json!({}),
                    token_budget: 256,
                    agent_task_id: None,
                    agent_run_id: Some("run-react-beta-hs".into()),
                },
            )
            .unwrap()
    };

    assert!(packet.selected_policies.iter().any(|policy| {
        policy.policy_id == crate::agent::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST
    }));
    let report = evaluate_react_beta_execution_readiness();
    assert!(report.proposal_first_writes_ready);
}
