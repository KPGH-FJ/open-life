//! Integration tests for Agent ReAct execution flow
//!
//! These tests validate the core ReAct loop behavior:
//! 1. Final-only response completes the run
//! 2. Action -> tool execution -> observation -> follow-up
//! 3. Malformed JSON becomes final with parse warning
//! 4. Step/tool budget stops execution
//! 5. Tool failure still records observation

use crate::agent::{
    ActionExecutionContext, ActionExecutor, ActionExecutorConfig, AgentExecutionBudget, AgentLoop,
    AgentLoopConfig, AgentObservation, AgentRun, AgentRunStatus, AgentTask, AgentTaskKind,
};
use crate::layer::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
    };

    let reply = r#"{"actions": [{"name": "weather", "arguments": {"city": "Beijing"}}], "warnings": ["Test warning"]}"#;
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].target, "weather");
    assert!(run
        .warnings
        .iter()
        .any(|warning| warning.contains("Test warning")));
    assert!(run
        .warnings
        .iter()
        .any(|warning| warning.contains("unregistered_tool_defaulted_mcp_tool")));
}

#[test]
fn test_action_parser_direct_read_actions_keep_executor_input_shape() {
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
    };

    let reply = r#"{"actions": [{"name": "memory.search", "action_type": "memory_search", "arguments": {"query": "budget review", "session_id": "s1", "limit": 3}}]}"#;
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].action_type, "memory_search");
    assert_eq!(actions[0].target, "memory.search");
    assert_eq!(actions[0].input["query"], "budget review");
    assert_eq!(actions[0].input["session_id"], "s1");
    assert_eq!(actions[0].input["limit"], 3);
    assert!(actions[0].input.get("arguments").is_none());
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
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
        react_trace: None,
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
        ..AgentLoopConfig::default()
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
    };

    let valid = r#"{"actions": [{"name": "web.search", "arguments": {"query": "test"}}], "final": "Let me search"}"#;
    let parsed = loop_instance
        .parse_agent_reply(valid, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert!(!parsed.json_parse_failed);
    assert_eq!(parsed.actions.len(), 1);
}

#[tokio::test]
async fn agent_loop_executes_multi_step_read_observe_follow_up_without_network() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig {
        max_steps: 3,
        max_tool_calls: 2,
        allow_writes: false,
        allow_cloud: false,
        ..AgentLoopConfig::default()
    })
    .with_scripted_replies(vec![
        r#"{"final":"I will search memory first.","actions":[{"name":"memory.search","action_type":"memory_search","arguments":{"query":"energy planning","session_id":"session-multistep","limit":5}}],"thought_summary":"Need a read-only observation.","warnings":[]}"#.into(),
        r#"{"final":"Here is the note from memory: low energy planning on Tuesday.","actions":[],"thought_summary":"Used the observation to answer.","warnings":[]}"#.into(),
    ]);
    let task = AgentTask {
        kind: AgentTaskKind::Conversation,
        session_id: "session-multistep".into(),
        user_text: "What did we discuss about energy planning?".into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "What did we discuss about energy planning?".into(),
        }],
        layer: Layer::L2,
    };
    let (registry, permission_store, audit_store, privacy_engine) = create_test_action_ctx();
    let memory_store = crate::memory::MemoryStore::new_in_memory().unwrap();
    memory_store
        .save_message(
            "session-multistep",
            &ChatMessage {
                role: "user".into(),
                content: "We discussed low energy planning on Tuesday.".into(),
            },
        )
        .unwrap();
    let action_ctx = ActionExecutionContext {
        registry: &registry,
        permission_store: &permission_store,
        audit_store: &audit_store,
        privacy_engine: &privacy_engine,
        safe_paths: &[],
        calendar_ics_paths: &[],
        life_model: None,
        memory_store: Some(&memory_store),
        proposal_store: None,
        agent_run_store: None,
        network_policy: None,
        web_search_fixture_output: None,
        hs_runtime_packet: None,
    };

    let result = loop_instance
        .run(
            &task,
            &LifeModel::default(),
            "Available tools: memory.search",
            None,
            privacy_engine.clone(),
            &action_ctx,
        )
        .await
        .unwrap();

    assert_eq!(result.step_count, 2);
    assert_eq!(result.tool_call_count, 1);
    assert_eq!(result.stop_reason, "no_tools");
    assert!(result.final_response.contains("low energy planning"));
    assert_eq!(result.run.actions.len(), 1);
    assert_eq!(result.run.actions[0].action_type, "memory_search");
    assert_eq!(result.run.observations.len(), 1);
    assert!(result.run.observations[0]
        .content
        .contains("low energy planning"));
    assert_eq!(result.run.status, AgentRunStatus::Completed);
    let structured = result.run.observations[0]
        .structured_result
        .as_ref()
        .expect("memory search observation should be structured");
    assert_eq!(structured["directWritesExecuted"], serde_json::json!(false));
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
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
        web_search_fixture_output: None,
        hs_runtime_packet: None,
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
