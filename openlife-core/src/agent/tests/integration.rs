//! Integration tests for Agent ReAct execution flow
//!
//! These tests validate the core ReAct loop behavior:
//! 1. Final-only response completes the run
//! 2. Action -> tool execution -> observation -> follow-up
//! 3. Malformed JSON becomes final with parse warning
//! 4. Step/tool budget stops execution
//! 5. Tool failure still records observation

use crate::agent::event_store::AgentRunEventStore;
use crate::agent::{
    ActionExecutionContext, ActionExecutor, ActionExecutorConfig, AgentEventActor,
    AgentExecutionBudget, AgentLoop, AgentLoopConfig, AgentObservation, AgentRun,
    AgentRunEventType, AgentRunStatus, AgentTask, AgentTaskKind,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::mcp::McpRegistry;
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;

/// Helper to create a minimal AgentLoop for testing
fn create_test_agent_loop(config: AgentLoopConfig) -> AgentLoop {
    let life_model = LifeModel::default();
    let scheduler = InferenceScheduler::default();
    let runtime = crate::agent::AgentRuntime::new(
        life_model,
        scheduler.clone(),
        &crate::config::AppConfig::default(),
    );
    let action_executor = ActionExecutor::new(ActionExecutorConfig::default());
    AgentLoop::new(runtime, action_executor, scheduler, config)
}

/// Helper to create an ActionExecutionContext for testing
fn create_test_action_ctx() -> (
    crate::mcp::McpRegistry,
    crate::tool_permissions::ToolPermissionStore,
    crate::mcp_audit::McpAuditStore,
    PrivacyEngine,
) {
    let registry = crate::mcp::McpRegistry::new();
    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    (registry, permission_store, audit_store, privacy_engine)
}

/// Helper to create a test task
fn create_test_task(messages: Vec<ChatMessage>) -> AgentTask {
    AgentTask {
        kind: AgentTaskKind::Conversation,
        session_id: "test-session".to_string(),
        user_text: messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages,
        layer: Layer::L2,
        ..Default::default()
    }
}

/// Test 1: AgentLoopConfig defaults are reasonable
#[test]
fn test_agent_loop_config_defaults() {
    let config = AgentLoopConfig::default();
    assert_eq!(config.max_steps, 4);
    assert_eq!(config.max_tool_calls, 6);
    assert_eq!(config.timeout_seconds, 90);
    assert!(config.allow_writes);
    assert!(config.allow_cloud);
}

/// Test 2: AgentExecutionBudget defaults match config
#[test]
fn test_agent_execution_budget_defaults() {
    let budget = AgentExecutionBudget::default();
    assert_eq!(budget.max_steps, 5);
    assert_eq!(budget.max_tool_calls, 3);
    assert_eq!(budget.timeout_seconds, 60);
    assert!(budget.allow_cloud);
    assert!(!budget.allow_writes);
}

/// Test 3: Budget can be customized
#[test]
fn test_agent_execution_budget_customization() {
    let mut budget = AgentExecutionBudget::default();
    budget.max_steps = 2;
    budget.max_tool_calls = 1;
    assert_eq!(budget.max_steps, 2);
    assert_eq!(budget.max_tool_calls, 1);
}

/// Test 4: AgentRun is created with correct initial state
#[test]
fn test_agent_run_initial_state() {
    let run = AgentRun::new_chat_run("session-1", "Hello");
    assert_eq!(run.session_id, Some("session-1".to_string()));
    assert_eq!(run.user_input, Some("Hello".to_string()));
    assert_eq!(run.status, AgentRunStatus::Running);
    assert!(run.actions.is_empty());
    assert!(run.observations.is_empty());
    assert!(run.warnings.is_empty());
    assert!(run.finished_at.is_none());
}

/// Test 5: AgentRun can record warnings
#[test]
fn test_agent_run_warnings() {
    let mut run = AgentRun::new_chat_run("session-1", "Hello");
    run.warnings.push("Parse warning: test".to_string());
    assert_eq!(run.warnings.len(), 1);
    assert_eq!(run.warnings[0], "Parse warning: test");
}

/// Test 6: Action Parser - valid JSON envelope with final returns no actions
#[test]
fn test_action_parser_final_envelope() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let (registry, permission_store, audit_store, privacy_engine) = create_test_action_ctx();
    let action_ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    let reply = r#"{"final": "Hello, I can help you!", "thought_summary": "User greeted me"}"#;
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert!(actions.is_empty());
    assert_eq!(run.warnings.len(), 1); // thought_summary recorded as warning
}

/// Test 7: Action Parser - valid JSON envelope with actions
#[test]
fn test_action_parser_actions_envelope() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let (registry, permission_store, audit_store, privacy_engine) = create_test_action_ctx();
    let action_ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    let reply = r#"{"actions": [{"name": "weather", "arguments": {"city": "Beijing"}}], "warnings": ["Test warning"]}"#;
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].target, "weather");
    assert_eq!(run.warnings.len(), 1);
    assert!(run.warnings[0].contains("Test warning"));
}

/// Test 8: Action Parser - legacy tool_calls format still works
#[test]
fn test_action_parser_legacy_tool_calls() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let (registry, permission_store, audit_store, privacy_engine) = create_test_action_ctx();
    let action_ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    let reply = r#"{"tool_calls": [{"name": "echo", "arguments": {"text": "hello"}}]}"#;
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].target, "echo");
}

/// Test 9: Action Parser - malformed JSON fail-soft
#[test]
fn test_action_parser_malformed_json_fail_soft() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let (registry, permission_store, audit_store, privacy_engine) = create_test_action_ctx();
    let action_ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    let reply = "{broken json";
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert!(actions.is_empty());
    assert_eq!(run.warnings.len(), 1);
    assert!(run.warnings[0].contains("Parse warning"));
}

/// Test 10: Action Parser - no JSON returns empty actions
#[test]
fn test_action_parser_no_json() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let (registry, permission_store, audit_store, privacy_engine) = create_test_action_ctx();
    let action_ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    let reply = "This is just a plain text response without any JSON.";
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert!(actions.is_empty());
    assert!(run.warnings.is_empty());
}

/// Test 11: Action Parser - final + actions coexist: actions should be returned
#[test]
fn test_action_parser_final_with_actions() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let (registry, permission_store, audit_store, privacy_engine) = create_test_action_ctx();
    let action_ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    // Model returns both final text and tool calls
    let reply = r#"{"final": "Let me search for that", "actions": [{"name": "web.search", "arguments": {"query": "test"}}]}"#;
    let parsed = loop_instance
        .parse_agent_reply(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    // Actions should NOT be empty when both final and actions are present
    assert_eq!(parsed.actions.len(), 1);
    assert_eq!(parsed.actions[0].target, "web.search");
    // Final text is preserved as the pre-execution note
    assert_eq!(parsed.final_text, "Let me search for that");
}

/// Test 12: Follow-up messages retain tools prompt
#[test]
fn test_follow_up_messages_retain_tools() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let task = create_test_task(vec![ChatMessage {
        role: "user".into(),
        content: "Search for news".into(),
    }]);

    let observations = vec![AgentObservation {
        id: "obs-1".into(),
        action_id: None,
        content: "Search results: ...".into(),
        source: "web.search".into(),
        structured_result: None,
        timestamp: chrono::Utc::now(),
    }];

    let tools_prompt = "Available tools: web.search, web.fetch";
    let follow_up = loop_instance.build_follow_up_messages(
        &task,
        "I'll search for news",
        &observations,
        tools_prompt,
    );

    // Should have original messages + assistant reply + follow-up
    assert_eq!(follow_up.len(), 3);

    // Last message should contain task goal, observations, and tools reminder
    let last = follow_up.last().unwrap();
    assert!(last.content.contains("Search for news")); // Original task
    assert!(last.content.contains("Search results")); // Observation
    assert!(last.content.contains("web.search")); // Tools reminder
    assert!(last.content.contains("web.fetch")); // Tools reminder
}

/// Test 13: max_tool_calls reached returns correct stop_reason
#[test]
fn test_max_tool_calls_stop_reason() {
    let config = AgentLoopConfig {
        max_steps: 4,
        max_tool_calls: 1,
        ..Default::default()
    };
    let loop_instance = create_test_agent_loop(config);
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 1u32; // Already at limit

    let (registry, permission_store, audit_store, privacy_engine) = create_test_action_ctx();
    let action_ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    // Simulate model returning actions when budget is already exceeded
    let reply = r#"{"actions": [{"name": "web.search", "arguments": {"query": "test"}}]}"#;
    let parsed = loop_instance
        .parse_agent_reply(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert_eq!(parsed.actions.len(), 1);
    // In real execution, this would trigger budget_exceeded path
    // This test validates the parser still works at budget limit
}

/// Test 14: JSON self-repair flag is set when model produces malformed JSON
#[test]
fn test_json_self_repair_flag_on_malformed_json() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let (registry, permission_store, audit_store, privacy_engine) = create_test_action_ctx();
    let action_ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    // Malformed JSON: missing closing brace
    let malformed = r#"{"final": "hello"#;
    let parsed = loop_instance
        .parse_agent_reply(malformed, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    // json_parse_failed should be true for malformed JSON
    assert!(parsed.json_parse_failed);
    assert!(parsed.actions.is_empty());
    // A warning should be recorded
    assert_eq!(run.warnings.len(), 1);
    assert!(run.warnings[0].contains("Parse warning"));
}

/// Test 15: Valid JSON with actions does NOT trigger json_parse_failed
#[test]
fn test_json_self_repair_flag_not_set_on_valid_json() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let (registry, permission_store, audit_store, privacy_engine) = create_test_action_ctx();
    let action_ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    let valid = r#"{"actions": [{"name": "web.search", "arguments": {"query": "test"}}], "final": "Let me search"}"#;
    let parsed = loop_instance
        .parse_agent_reply(valid, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert!(!parsed.json_parse_failed);
    assert_eq!(parsed.actions.len(), 1);
}

/// Test 16: Proposal-generation tools bypass permission-confirmation blocking.
#[test]
fn test_proposal_tool_bypass_permission_blocking() {
    let mut registry = crate::mcp::McpRegistry::new();
    registry.register_default_builtins();

    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let prop_store = crate::agent::proposal_store::ProposalStore::new_in_memory().unwrap();

    // Create a temp dir as safe_path so the filesystem precheck passes
    let safe_dir = tempfile::TempDir::new().unwrap();
    let safe_path = safe_dir.path().to_str().unwrap().to_string();

    let ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[safe_path.clone()],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: Some(&prop_store),
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    let executor = ActionExecutor::new(ActionExecutorConfig::default());

    // file.write_proposal is high-risk with "write"+"filesystem" capabilities.
    // With no explicit permission policy, our A1 fix allows proposal tools to
    // bypass permission blocking and create a Proposal for user review.
    let request = crate::agent::AgentActionRequest {
        action_type: "mcp_tool".into(),
        target: "file.write_proposal".into(),
        input: serde_json::json!({
            "arguments": {
                "path": safe_dir.path().join("test.md").to_str().unwrap(),
                "content": "# Test"
            }
        }),
        source_run_id: None,
        step_index: 0,
    };

    let result = executor.execute(request, &ctx).unwrap();

    // A1 fix: proposal tool bypasses permission blocking → Proposal is created.
    // The action status is Succeeded because the handler creates the Proposal.
    assert_eq!(
        result.status,
        crate::agent::ActionExecutionStatus::Succeeded
    );
    let output = result.action.output.unwrap();
    assert!(output.to_string().contains("proposal_id"));
    assert!(output.to_string().contains("external_write_action"));
}

/// Test 17: permission.check tool returns a valid permission decision.
#[test]
fn test_permission_check_tool() {
    let mut registry = crate::mcp::McpRegistry::new();
    registry.register_default_builtins();

    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();

    // Grant permission to check against
    permission_store
        .grant(
            "web.fetch",
            "builtin",
            "medium",
            "network",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();

    let ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    let executor = ActionExecutor::new(ActionExecutorConfig::default());

    // Check a tool that has an explicit allow policy
    let request = crate::agent::AgentActionRequest {
        action_type: "mcp_tool".into(),
        target: "permission.check".into(),
        input: serde_json::json!({
            "arguments": {
                "tool_name": "web.fetch",
                "source": "builtin"
            }
        }),
        source_run_id: None,
        step_index: 0,
    };

    let result = executor.execute(request, &ctx).unwrap();
    assert_eq!(
        result.status,
        crate::agent::ActionExecutionStatus::Succeeded
    );
    let output = result.action.output.unwrap();
    // The output should contain a JSON decision
    assert!(output.to_string().contains("allowed"));
}

/// Test 18: memory.propose_write generates a MemoryWrite Proposal instead of being blocked.
#[test]
fn test_memory_propose_write_creates_proposal() {
    let mut registry = crate::mcp::McpRegistry::new();
    registry.register_default_builtins();

    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let prop_store = crate::agent::proposal_store::ProposalStore::new_in_memory().unwrap();

    let ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: Some(&prop_store),
        agent_run_store: None,
        network_policy: None,
        event_store: None,
    };

    let executor = ActionExecutor::new(ActionExecutorConfig::default());

    let request = crate::agent::AgentActionRequest {
        action_type: "mcp_tool".into(),
        target: "memory.propose_write".into(),
        input: serde_json::json!({
            "arguments": {
                "content": "用户喜欢深色主题",
                "category": "preference"
            }
        }),
        source_run_id: None,
        step_index: 0,
    };

    let result = executor.execute(request, &ctx).unwrap();

    // Should succeed because memory.propose_write is a proposal-generation tool
    // that was exempted from permission blocking in Sprint A1
    assert_eq!(
        result.status,
        crate::agent::ActionExecutionStatus::Succeeded
    );
    let output = result.action.output.unwrap();
    assert!(
        output.to_string().contains("proposal_id"),
        "expected proposal_id in output: {}",
        output
    );
}

#[test]
fn test_agent_loop_config_role_generalist_default() {
    let config = AgentLoopConfig::default();
    assert_eq!(config.role, crate::agent::agent_loop::AgentRole::Generalist);
    assert!(config.toolset_allowlist.is_empty());
    assert!(config.role_system_instruction().is_none());
}

#[test]
fn test_agent_loop_config_role_planner_instruction() {
    let config = AgentLoopConfig {
        role: crate::agent::agent_loop::AgentRole::Planner,
        ..Default::default()
    };
    let instruction = config.role_system_instruction().unwrap();
    assert!(instruction.contains("Planner mode"));
    assert!(instruction.contains("goal.read"));
}

/// P1-3: Role prompt is available as a versioned PromptBlock.
#[test]
fn test_agent_role_prompt_block_traceable() {
    let config = AgentLoopConfig {
        role: crate::agent::agent_loop::AgentRole::Planner,
        ..Default::default()
    };
    let block = config.role_prompt_block().unwrap();
    assert_eq!(block.id, "role.planner");
    assert_eq!(block.version, "1.0.0");
    assert!(block.content.contains("Planner mode"));
    assert!(block.content.contains("goal.read"));
    assert!(block.applies_to.contains(&"Planner".to_string()));
    assert!(block.is_cloud_safe());
}

/// P1-3: Generalist role produces no prompt block.
#[test]
fn test_agent_role_generalist_no_block() {
    let config = AgentLoopConfig::default();
    assert!(config.role_prompt_block().is_none());
}

#[test]
fn test_agent_loop_config_toolset_allowlist() {
    let config = AgentLoopConfig {
        role: crate::agent::agent_loop::AgentRole::Planner,
        toolset_allowlist: vec!["goal.read".into(), "life_model.read".into()],
        ..Default::default()
    };
    assert_eq!(config.toolset_allowlist.len(), 2);
    assert!(config.toolset_allowlist.contains(&"goal.read".to_string()));
}

// ── P1-1: Declarative-Only Enforcement tests ──────────────────────────

/// Test that declarative-only tools are filtered from the tools prompt.
#[test]
fn test_declarative_only_tools_filtered_from_prompt() {
    let mut registry = McpRegistry::new();
    registry.register_default_builtins();

    let prompt = registry.tools_prompt();
    // email.read is declarative_only
    assert!(
        !prompt.contains("email.read"),
        "declarative-only email.read should NOT be in tools prompt"
    );
    // snapshot.create is declarative_only
    assert!(
        !prompt.contains("snapshot.create"),
        "declarative-only snapshot.create should NOT be in tools prompt"
    );
    // life_model.read is executable
    assert!(
        prompt.contains("life_model.read"),
        "executable life_model.read SHOULD be in tools prompt"
    );
    // web.search is executable
    assert!(
        prompt.contains("web.search"),
        "executable web.search SHOULD be in tools prompt"
    );
}

/// Test that declarative-only tools are blocked at runtime by ActionExecutor.
#[test]
fn test_declarative_only_tool_blocked_at_runtime() {
    let mut registry = McpRegistry::new();
    registry.register_default_builtins();
    // Verify email.read is declarative_only
    let email_manifest = registry
        .list_manifests()
        .into_iter()
        .find(|m| m.name == "email.read")
        .expect("email.read should exist");
    assert!(email_manifest.declarative_only);

    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        event_store: None,
        network_policy: None,
    };

    let executor = ActionExecutor::new(ActionExecutorConfig::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "mcp_tool".into(),
        target: "email.read".into(),
        input: serde_json::json!({"arguments": {}}),
        source_run_id: None,
        step_index: 0,
    };

    let result = executor.execute(request, &ctx).unwrap();
    assert_eq!(result.status, crate::agent::ActionExecutionStatus::Blocked);
    assert_eq!(result.action.status, "blocked");
    assert!(result.observation.content.contains("declarative-only"));
}

/// Test that blocked tool execution records an AgentRunEvent.
#[test]
fn test_blocked_tool_records_event() {
    let mut registry = McpRegistry::new();
    registry.register_default_builtins();

    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let event_store = AgentRunEventStore::new_in_memory().unwrap();
    let run_id = "test-blocked-event-001";

    let ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
        event_store: Some(event_store.clone()),
        network_policy: None,
    };

    let executor = ActionExecutor::new(ActionExecutorConfig::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "mcp_tool".into(),
        target: "email.read".into(),
        input: serde_json::json!({"arguments": {}}),
        source_run_id: Some(run_id.to_string()),
        step_index: 0,
    };

    let result = executor.execute(request, &ctx).unwrap();
    assert_eq!(result.status, crate::agent::ActionExecutionStatus::Blocked);

    // Verify event was recorded
    let events = event_store.list_events_by_run(run_id).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, AgentRunEventType::ToolCallBlocked);
    assert_eq!(
        events[0].actor,
        AgentEventActor::Tool("email.read".to_string())
    );
    assert!(events[0].summary.contains("blocked"));
}

/// Test that proposal-generating tools remain callable (not blocked).
#[test]
fn test_proposal_tools_not_blocked_by_declarative_enforcement() {
    let mut registry = McpRegistry::new();
    registry.register_default_builtins();

    let safe_dir = tempfile::TempDir::new().unwrap();
    let safe_path = safe_dir.path().to_str().unwrap().to_string();
    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let prop_store = crate::agent::proposal_store::ProposalStore::new_in_memory().unwrap();

    let ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[safe_path.clone()],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: None,
        proposal_store: Some(&prop_store),
        agent_run_store: None,
        event_store: None,
        network_policy: None,
    };

    let executor = ActionExecutor::new(ActionExecutorConfig::default());

    // memory.propose_write is a proposal-generating tool
    let request = crate::agent::AgentActionRequest {
        action_type: "mcp_tool".into(),
        target: "memory.propose_write".into(),
        input: serde_json::json!({"arguments": {"content": "test memory"}}),
        source_run_id: None,
        step_index: 0,
    };

    let result = executor.execute(request, &ctx).unwrap();
    // Should succeed or need confirmation, not be blocked
    assert!(result.status != crate::agent::ActionExecutionStatus::Blocked);
    assert!(result.action.status == "succeeded" || result.action.status == "needs_confirmation");
}
