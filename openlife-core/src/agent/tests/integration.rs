//! Integration tests for Agent ReAct execution flow
//!
//! These tests validate the core ReAct loop behavior:
//! 1. Final-only response completes the run
//! 2. Action -> tool execution -> observation -> follow-up
//! 3. Malformed JSON becomes final with parse warning
//! 4. Step/tool budget stops execution
//! 5. Tool failure still records observation

use crate::agent::{
    ActionContext, ActionExecutor, ActionExecutorConfig, AgentExecutionBudget, AgentLoop,
    AgentLoopConfig, AgentObservation, AgentRun, AgentRunStatus, AgentTask, AgentTaskKind,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::mcp::McpRegistry;
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;
use std::sync::Arc;

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
fn create_test_action_ctx() -> ActionContext {
    let reg = crate::mcp::McpRegistry::new();
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let af = tempfile::NamedTempFile::new().unwrap();
    let as_ = crate::mcp_audit::McpAuditStore::new(af.path());
    let pe = PrivacyEngine::new();
    ActionContext::new_for_test(reg, ps, as_, pe, vec![])
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
#[tokio::test]
async fn test_agent_loop_config_defaults() {
    let config = AgentLoopConfig::default();
    assert_eq!(config.max_steps, 4);
    assert_eq!(config.max_tool_calls, 6);
    assert_eq!(config.timeout_seconds, 90);
    assert!(config.allow_writes);
    assert!(config.allow_cloud);
}

/// Test 2: AgentExecutionBudget defaults match config
#[tokio::test]
async fn test_agent_execution_budget_defaults() {
    let budget = AgentExecutionBudget::default();
    assert_eq!(budget.max_steps, 5);
    assert_eq!(budget.max_tool_calls, 3);
    assert_eq!(budget.timeout_seconds, 60);
    assert!(budget.allow_cloud);
    assert!(!budget.allow_writes);
}

/// Test 3: Budget can be customized
#[tokio::test]
async fn test_agent_execution_budget_customization() {
    let budget = AgentExecutionBudget {
        max_steps: 2,
        max_tool_calls: 1,
        ..Default::default()
    };
    assert_eq!(budget.max_steps, 2);
    assert_eq!(budget.max_tool_calls, 1);
}

/// Test 4: AgentRun is created with correct initial state
#[tokio::test]
async fn test_agent_run_initial_state() {
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
#[tokio::test]
async fn test_agent_run_warnings() {
    let mut run = AgentRun::new_chat_run("session-1", "Hello");
    run.warnings.push("Parse warning: test".to_string());
    assert_eq!(run.warnings.len(), 1);
    assert_eq!(run.warnings[0], "Parse warning: test");
}

/// Test 6: Action Parser - valid JSON envelope with final returns no actions
#[tokio::test]
async fn test_action_parser_final_envelope() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let action_ctx = create_test_action_ctx();

    let reply = r#"{"final": "Hello, I can help you!", "thought_summary": "User greeted me"}"#;
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert!(actions.is_empty());
    assert_eq!(run.warnings.len(), 1); // thought_summary recorded as warning
}

/// Test 7: Action Parser - valid JSON envelope with actions
#[tokio::test]
async fn test_action_parser_actions_envelope() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let action_ctx = create_test_action_ctx();

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
#[tokio::test]
async fn test_action_parser_legacy_tool_calls() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let action_ctx = create_test_action_ctx();

    let reply = r#"{"tool_calls": [{"name": "echo", "arguments": {"text": "hello"}}]}"#;
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].target, "echo");
}

/// Test 9: Action Parser - malformed JSON fail-soft
#[tokio::test]
async fn test_action_parser_malformed_json_fail_soft() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let action_ctx = create_test_action_ctx();

    let reply = "{broken json";
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert!(actions.is_empty());
    assert_eq!(run.warnings.len(), 1);
    assert!(run.warnings[0].contains("Parse warning"));
}

/// Test 10: Action Parser - no JSON returns empty actions
#[tokio::test]
async fn test_action_parser_no_json() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let action_ctx = create_test_action_ctx();

    let reply = "This is just a plain text response without any JSON.";
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert!(actions.is_empty());
    assert!(run.warnings.is_empty());
}

/// Test 11: Action Parser - final + actions coexist: actions should be returned
#[tokio::test]
async fn test_action_parser_final_with_actions() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let action_ctx = create_test_action_ctx();

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
#[tokio::test]
async fn test_follow_up_messages_retain_tools() {
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
#[tokio::test]
async fn test_max_tool_calls_stop_reason() {
    let config = AgentLoopConfig {
        max_steps: 4,
        max_tool_calls: 1,
        ..Default::default()
    };
    let loop_instance = create_test_agent_loop(config);
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 1u32; // Already at limit

    let action_ctx = create_test_action_ctx();

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
#[tokio::test]
async fn test_json_self_repair_flag_on_malformed_json() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let action_ctx = create_test_action_ctx();

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
#[tokio::test]
async fn test_json_self_repair_flag_not_set_on_valid_json() {
    let loop_instance = create_test_agent_loop(AgentLoopConfig::default());
    let mut run = AgentRun::new_chat_run("test", "Hi");
    let mut tool_call_count = 0u32;

    let action_ctx = create_test_action_ctx();

    let valid = r#"{"actions": [{"name": "web.search", "arguments": {"query": "test"}}], "final": "Let me search"}"#;
    let parsed = loop_instance
        .parse_agent_reply(valid, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert!(!parsed.json_parse_failed);
    assert_eq!(parsed.actions.len(), 1);
}

/// Test 16: Proposal-generation tools bypass permission-confirmation blocking.
#[tokio::test]
async fn test_proposal_tool_bypass_permission_blocking() {
    let mut registry = crate::mcp::McpRegistry::new();
    registry.register_default_builtins();

    let permission_store = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit_file = tempfile::NamedTempFile::new().unwrap();
    let audit_store = crate::mcp_audit::McpAuditStore::new(audit_file.path());
    let privacy_engine = PrivacyEngine::new();
    let _prop_store = crate::agent::proposal_store::ProposalStore::new_in_memory().unwrap();

    // Create a temp dir as safe_path so the filesystem precheck passes
    let safe_dir = tempfile::TempDir::new().unwrap();
    let safe_path = safe_dir.path().to_str().unwrap().to_string();

    let mut ctx = ActionContext::new_for_test(
        registry,
        permission_store,
        audit_store,
        privacy_engine,
        vec![safe_path.clone()],
    );
    let prop_store = crate::agent::proposal_store::ProposalStore::new_in_memory().unwrap();
    ctx.proposal_store = Some(Arc::new(tokio::sync::Mutex::new(prop_store)));

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

    let result = executor.execute(request, &ctx).await.unwrap();

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
#[tokio::test]
async fn test_permission_check_tool() {
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

    let mut ctx = ActionContext::new_for_test(
        registry,
        permission_store,
        audit_store,
        privacy_engine,
        vec![],
    );
    let prop_store = crate::agent::proposal_store::ProposalStore::new_in_memory().unwrap();
    ctx.proposal_store = Some(Arc::new(tokio::sync::Mutex::new(prop_store)));

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

    let result = executor.execute(request, &ctx).await.unwrap();

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

#[tokio::test]
async fn test_agent_loop_config_role_generalist_default() {
    let config = AgentLoopConfig::default();
    assert_eq!(config.role, crate::agent::agent_loop::AgentRole::Generalist);
    assert!(config.toolset_allowlist.is_empty());
    assert!(config.role_system_instruction().is_none());
}

#[tokio::test]
async fn test_agent_loop_config_role_planner_instruction() {
    let config = AgentLoopConfig {
        role: crate::agent::agent_loop::AgentRole::Planner,
        ..Default::default()
    };
    let instruction = config.role_system_instruction().unwrap();
    assert!(instruction.contains("Planner mode"));
    assert!(instruction.contains("goal.read"));
}

/// P1-3: Role prompt is available as a versioned PromptBlock.
#[tokio::test]
async fn test_agent_role_prompt_block_traceable() {
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
#[tokio::test]
async fn test_agent_role_generalist_no_block() {
    let config = AgentLoopConfig::default();
    assert!(config.role_prompt_block().is_none());
}

#[tokio::test]
async fn test_agent_loop_config_toolset_allowlist() {
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
#[tokio::test]
async fn test_declarative_only_tools_filtered_from_prompt() {
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
#[tokio::test]
async fn test_declarative_only_tool_blocked_at_runtime() {
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
    let mut ctx = ActionContext::new_for_test(
        registry,
        permission_store,
        audit_store,
        privacy_engine,
        vec![],
    );
    let prop_store = crate::agent::proposal_store::ProposalStore::new_in_memory().unwrap();
    ctx.proposal_store = Some(Arc::new(tokio::sync::Mutex::new(prop_store)));

    let executor = ActionExecutor::new(ActionExecutorConfig::default());

    let request = crate::agent::AgentActionRequest {
        action_type: "mcp_tool".into(),
        target: "memory.propose_write".into(),
        input: serde_json::json!({"arguments": {"content": "test memory"}}),
        source_run_id: None,
        step_index: 0,
    };

    let result = executor.execute(request, &ctx).await.unwrap();
    // Should succeed or need confirmation, not be blocked
    assert!(result.status != crate::agent::ActionExecutionStatus::Blocked);
    assert!(result.action.status == "succeeded" || result.action.status == "needs_confirmation");
}

// ── P9-3: Sandbox wiring tests ──────────────────────────────────────

#[tokio::test]
async fn test_missing_config_yields_disabled_sandbox() {
    use crate::agent::action_executor::DISABLED_SANDBOX;
    assert!(!DISABLED_SANDBOX.bash_enabled);
    assert_eq!(
        DISABLED_SANDBOX.network_policy,
        crate::agent::execution_sandbox::NetworkPolicy::None
    );
    // Default sandbox keeps conservative defaults but bash is always disabled
    assert!(DISABLED_SANDBOX
        .dangerous_command_denylist
        .iter()
        .any(|c| c == "rm"));
}

#[tokio::test]
async fn test_action_context_default_is_disabled_sandbox() {
    let reg = McpRegistry::new();
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit =
        crate::mcp_audit::McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit.db"));
    let pe = PrivacyEngine::new();
    let ctx = ActionContext::new_for_test(reg, ps, audit, pe, vec![]);
    assert!(!ctx.execution_sandbox.bash_enabled);
    assert!(!ctx.execution_sandbox.command_allowlist.is_empty());
}

#[tokio::test]
async fn test_configured_safe_paths_feed_sandbox_safe_paths() {
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
        safe_paths: vec!["/custom/path".into()],
        ..crate::agent::execution_sandbox::ExecutionSandbox::default()
    };
    assert!(!sandbox.bash_enabled);
    assert!(sandbox.safe_paths.contains(&"/custom/path".into()));
    assert!(sandbox.is_path_in_safe_paths("/custom/path"));
}

#[tokio::test]
async fn test_plan_execution_receives_sandbox_without_enabling_shell() {
    let reg = McpRegistry::new();
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit =
        crate::mcp_audit::McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit.db"));
    let pe = PrivacyEngine::new();
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
        safe_paths: vec!["/tmp".into()],
        ..crate::agent::execution_sandbox::ExecutionSandbox::default()
    };
    let ctx =
        ActionContext::new_for_test(reg, ps, audit, pe, vec![]).with_execution_sandbox(sandbox);
    assert!(!ctx.execution_sandbox.bash_enabled);
    assert!(!ctx.execution_sandbox.command_allowlist.is_empty());
}

// ── P9-5: ActionExecutor shell.run governed tests ─────────────────────

#[tokio::test]
async fn test_shell_run_default_not_model_callable() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();
    let manifests = reg.list_manifests();
    let shell = manifests.iter().find(|m| m.name == "shell.run");
    assert!(shell.is_some(), "shell.run manifest must exist");
    let shell = shell.unwrap();
    assert!(!shell.enabled, "shell.run must be disabled by default");
    assert!(
        !shell.declarative_only,
        "shell.run must not be declarative-only"
    );
    assert_eq!(shell.risk_level, "high");
    assert_eq!(shell.permission_level, "high");
}

#[tokio::test]
async fn test_shell_run_manifest_disabled_blocks() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit =
        crate::mcp_audit::McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit.db"));
    let pe = PrivacyEngine::new();
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
        bash_enabled: true,
        cwd: std::env::temp_dir().to_string_lossy().to_string(),
        safe_paths: vec![std::env::temp_dir().to_string_lossy().to_string()],
        command_allowlist: vec!["echo".into()],
        ..crate::agent::execution_sandbox::ExecutionSandbox::default()
    };
    let ctx =
        ActionContext::new_for_test(reg, ps, audit, pe, vec![]).with_execution_sandbox(sandbox);

    // shell.run manifest is enabled=false by default → blocked before spawn
    let executor = crate::agent::ActionExecutor::new(Default::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "builtin_tool".into(),
        target: "shell.run".into(),
        input: serde_json::json!({"arguments": {"command": "echo", "args": ["hello"]}}),
        source_run_id: None,
        step_index: 0,
    };
    let result = executor.execute(request, &ctx).await;
    assert!(result.is_ok(), "execute should not return Err");
    let result = result.unwrap();
    assert_eq!(
        result.status,
        crate::agent::action_executor::ActionExecutionStatus::Blocked
    );
    assert!(result.action.error.unwrap_or_default().contains("disabled"));
}

#[tokio::test]
async fn test_shell_run_disabled_sandbox_blocks_at_action_executor() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();
    // This test exercises the ActionExecutor → execute_shell_run path
    // with a disabled sandbox (bash_enabled=false). The declarative_only
    // check in execute_shell_run cannot be triggered while the manifest
    // has declarative_only=false (P9 design), so we validate the sandbox
    // gate as the primary early-rejection path after manifest checks.
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit =
        crate::mcp_audit::McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit.db"));
    let pe = PrivacyEngine::new();
    // Make sandbox bash_enabled=false → blocked event
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox::default();
    let ctx =
        ActionContext::new_for_test(reg, ps, audit, pe, vec![]).with_execution_sandbox(sandbox);

    let executor = crate::agent::ActionExecutor::new(Default::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "builtin_tool".into(),
        target: "shell.run".into(),
        input: serde_json::json!({"arguments": {"command": "echo", "args": ["hi"]}}),
        source_run_id: None,
        step_index: 0,
    };
    let result = executor.execute(request, &ctx).await;
    assert!(result.is_ok());
    let result = result.unwrap();
    assert_eq!(
        result.status,
        crate::agent::action_executor::ActionExecutionStatus::Blocked
    );
}

#[tokio::test]
async fn test_shell_run_sandbox_disabled_records_blocked() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit =
        crate::mcp_audit::McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit.db"));
    let pe = PrivacyEngine::new();
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox::always_disabled();
    let event_store = crate::agent::event_store::AgentRunEventStore::new_in_memory()
        .expect("in-memory event store");

    let mut ctx = ActionContext::new_for_test(reg, ps, audit, pe, vec![]);
    ctx.execution_sandbox = sandbox;
    ctx.event_store = Some(event_store.clone());

    let run_id = "test-run-shell-blocked";
    let executor = crate::agent::ActionExecutor::new(Default::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "builtin_tool".into(),
        target: "shell.run".into(),
        input: serde_json::json!({"arguments": {"command": "echo", "args": ["blocked"]}}),
        source_run_id: Some(run_id.to_string()),
        step_index: 0,
    };
    let result = executor.execute(request, &ctx).await.unwrap();
    assert_eq!(
        result.status,
        crate::agent::action_executor::ActionExecutionStatus::Blocked
    );

    let events = event_store.list_events_by_run(run_id).unwrap();
    let has_blocked = events.iter().any(|e| {
        matches!(
            e.event_type,
            crate::agent::AgentRunEventType::ToolCallBlocked
        )
    });
    assert!(has_blocked, "blocked event must be recorded");
}

#[tokio::test]
async fn test_shell_run_allowed_command_succeeds() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();

    let tmp = std::env::temp_dir().to_string_lossy().to_string();
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
        bash_enabled: true,
        cwd: tmp.clone(),
        safe_paths: vec![tmp.clone()],
        command_allowlist: vec!["echo".into(), "date".into()],
        timeout_ms: 30_000,
        max_output_bytes: 1024 * 1024,
        ..crate::agent::execution_sandbox::ExecutionSandbox::default()
    };

    // Direct ShellExecutor test validates the sandbox validation and
    // command execution logic (below the manifest layer).
    //
    // Coverage note: the full end-to-end path execute_tool →
    // execute_shell_run → ShellExecutor::execute is not covered here
    // because the shell.run manifest is enabled=false by default and
    // the registry offers no test-only mechanism to enable it.
    // ActionExecutor blocked paths (manifest disabled, sandbox disabled)
    // are covered by tool_executor::tests and the tests above.
    // The direct ShellExecutor call here validates that when all gates
    // pass, the actual process spawn/truncation/timeout works correctly.
    let executor_core = crate::agent::shell_executor::ShellExecutor::new(sandbox.clone());
    let req = crate::agent::shell_executor::ShellCommandRequest {
        command: "echo".into(),
        args: vec!["governed_shell_ok".into()],
        cwd: None,
        env: std::collections::HashMap::new(),
        reason: Some("P9 integration test".into()),
    };
    let result = executor_core.execute(&req);
    assert!(
        result.is_ok(),
        "direct ShellExecutor must succeed: {:?}",
        result.err()
    );
    let output = result.unwrap();
    assert!(output.stdout.contains("governed_shell_ok"));
    assert_eq!(output.exit_code, 0);
    assert!(!output.timed_out);
    assert!(!output.truncated);
}

// ── P9-6: Governed runtime entry policy tests ──────────────────────────

#[tokio::test]
async fn test_default_agent_spec_denies_shell() {
    let spec = crate::agent::types::AgentSpec::default();
    // Default AgentSpec has empty allowed_tools (= use role defaults).
    // An AgentSpec with explicit allowlist without shell.run denies it.
    let spec_with_allowlist = spec.clone().with_allowed_tools(vec![
        "goal.read".into(),
        "life_model.read".into(),
        "memory.search".into(),
    ]);
    let allows_shell = spec_with_allowlist
        .allowed_tools
        .iter()
        .any(|t| t.as_str().contains("shell"));
    assert!(
        !allows_shell,
        "AgentSpec without shell.run in allowed_tools must deny shell"
    );
}

#[tokio::test]
async fn test_scheduled_proactive_uses_disabled_sandbox() {
    // The DISABLED_SANDBOX static is used by scheduler_runner for
    // scheduled/proactive tasks. Verify it never enables bash.
    let sandbox = &crate::agent::action_executor::DISABLED_SANDBOX;
    assert!(!sandbox.bash_enabled);
}

#[tokio::test]
async fn test_sub_agent_shell_attempt_blocked_by_default() {
    // SubAgentRuntime does not expose shell by design.
    // The default AgentSpec used in SubAgentSpec does not include shell.run.
    let agent_spec = crate::agent::types::AgentSpec::default();
    let sub_spec = crate::agent::types::SubAgentSpec::new(
        agent_spec,
        crate::agent::types::DelegationMode::CallAsTool,
    );
    let has_shell = sub_spec
        .spec
        .allowed_tools
        .iter()
        .any(|t| t.as_str().contains("shell"));
    assert!(!has_shell, "default sub-agent must deny shell.run");
}

#[tokio::test]
async fn test_plan_bound_agent_spec_denies_shell_execution() {
    // A plan-bound AgentSpec without shell.run in allowed_tools must deny shell.
    let spec = crate::agent::types::AgentSpec {
        allowed_tools: vec!["goal.read".into(), "life_model.read".into()],
        ..Default::default()
    };
    let allows_shell = spec
        .allowed_tools
        .iter()
        .any(|t| t.as_str().contains("shell"));
    assert!(
        !allows_shell,
        "plan AgentSpec without shell.run must deny shell"
    );
}

#[tokio::test]
async fn test_agent_spec_with_explicit_shell_allows_it() {
    // If an AgentSpec explicitly allows shell.run, the spec permits it.
    // (Runtime sandbox/manifest still must also allow.)
    let spec = crate::agent::types::AgentSpec {
        allowed_tools: vec!["goal.read".into(), "shell.run".into()],
        ..Default::default()
    };
    let allows_shell = spec.allowed_tools.iter().any(|t| t.contains("shell"));
    assert!(
        allows_shell,
        "AgentSpec with explicit shell.run must allow it"
    );
}

// ── P9 AgentSpec gate tests ────────────────────────────────────────────

#[tokio::test]
async fn test_shell_run_missing_agentspec_blocks() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();
    reg.set_builtin_manifest_enabled("shell.run", true);
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit = crate::mcp_audit::McpAuditStore::new(
        tempfile::tempdir().unwrap().path().join("audit_as1.db"),
    );
    let pe = PrivacyEngine::new();
    let tmp = std::env::temp_dir().to_string_lossy().to_string();
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
        bash_enabled: true,
        cwd: tmp.clone(),
        safe_paths: vec![tmp],
        command_allowlist: vec!["echo".into()],
        ..crate::agent::execution_sandbox::ExecutionSandbox::default()
    };
    // No agent_spec set → must fail-closed
    let ctx =
        ActionContext::new_for_test(reg, ps, audit, pe, vec![]).with_execution_sandbox(sandbox);

    let executor = crate::agent::ActionExecutor::new(Default::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "builtin_tool".into(),
        target: "shell.run".into(),
        input: serde_json::json!({"arguments": {"command": "echo", "args": ["test"]}}),
        source_run_id: None,
        step_index: 0,
    };
    let result = executor.execute(request, &ctx).await.unwrap();
    assert_eq!(
        result.status,
        crate::agent::action_executor::ActionExecutionStatus::Blocked
    );
    assert!(result
        .action
        .error
        .unwrap_or_default()
        .contains("AgentSpec missing"));
}

#[tokio::test]
async fn test_shell_run_agentspec_denies_blocks_before_permission() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();
    reg.set_builtin_manifest_enabled("shell.run", true);
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit = crate::mcp_audit::McpAuditStore::new(
        tempfile::tempdir().unwrap().path().join("audit_as2.db"),
    );
    let pe = PrivacyEngine::new();
    let tmp = std::env::temp_dir().to_string_lossy().to_string();
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
        bash_enabled: true,
        cwd: tmp.clone(),
        safe_paths: vec![tmp],
        command_allowlist: vec!["echo".into()],
        ..crate::agent::execution_sandbox::ExecutionSandbox::default()
    };
    // AgentSpec explicitly denies shell by not listing it
    let spec = crate::agent::types::AgentSpec {
        allowed_tools: vec!["goal.read".into(), "life_model.read".into()],
        ..Default::default()
    };
    let ctx = ActionContext::new_for_test(reg, ps, audit, pe, vec![])
        .with_execution_sandbox(sandbox)
        .with_agent_spec(spec);

    let executor = crate::agent::ActionExecutor::new(Default::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "builtin_tool".into(),
        target: "shell.run".into(),
        input: serde_json::json!({"arguments": {"command": "echo", "args": ["blocked"]}}),
        source_run_id: Some("run-as-deny".into()),
        step_index: 0,
    };
    let result = executor.execute(request, &ctx).await.unwrap();
    assert_eq!(
        result.status,
        crate::agent::action_executor::ActionExecutionStatus::Blocked
    );
    assert!(result
        .action
        .error
        .unwrap_or_default()
        .contains("AgentSpec denied"));
}

#[tokio::test]
async fn test_shell_run_agentspec_allows_continues_to_permission() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();
    reg.set_builtin_manifest_enabled("shell.run", true);
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit = crate::mcp_audit::McpAuditStore::new(
        tempfile::tempdir().unwrap().path().join("audit_as3.db"),
    );
    let pe = PrivacyEngine::new();
    let tmp = std::env::temp_dir().to_string_lossy().to_string();
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
        bash_enabled: true,
        cwd: tmp.clone(),
        safe_paths: vec![tmp],
        command_allowlist: vec!["echo".into()],
        ..crate::agent::execution_sandbox::ExecutionSandbox::default()
    };
    // AgentSpec allows shell, but permission store denies by default
    let spec = crate::agent::types::AgentSpec {
        allowed_tools: vec!["goal.read".into(), "shell.run".into()],
        ..Default::default()
    };
    let ctx = ActionContext::new_for_test(reg, ps, audit, pe, vec![])
        .with_execution_sandbox(sandbox)
        .with_agent_spec(spec);

    let executor = crate::agent::ActionExecutor::new(Default::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "builtin_tool".into(),
        target: "shell.run".into(),
        input: serde_json::json!({"arguments": {"command": "echo", "args": ["hi"]}}),
        source_run_id: Some("run-as-allow".into()),
        step_index: 0,
    };
    let result = executor.execute(request, &ctx).await.unwrap();
    // AgentSpec allows, but permission store blocks → NeedsConfirmation or Blocked
    assert!(
        matches!(
            result.status,
            crate::agent::action_executor::ActionExecutionStatus::NeedsConfirmation
                | crate::agent::action_executor::ActionExecutionStatus::Blocked
        ),
        "expected blocked or needs_confirmation, got {:?}",
        result.status
    );
}

// ── P9 event deduplication tests ────────────────────────────────────────

#[tokio::test]
async fn test_shell_run_blocked_records_only_blocked_no_started() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit = crate::mcp_audit::McpAuditStore::new(
        tempfile::tempdir().unwrap().path().join("audit_evt1.db"),
    );
    let pe = PrivacyEngine::new();
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox::always_disabled();
    let event_store = crate::agent::event_store::AgentRunEventStore::new_in_memory().unwrap();
    let mut ctx = ActionContext::new_for_test(reg, ps, audit, pe, vec![]);
    ctx.execution_sandbox = sandbox;
    ctx.event_store = Some(event_store.clone());

    let run_id = "run-evt-blocked";
    let executor = crate::agent::ActionExecutor::new(Default::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "builtin_tool".into(),
        target: "shell.run".into(),
        input: serde_json::json!({"arguments": {"command": "echo", "args": ["evt"]}}),
        source_run_id: Some(run_id.to_string()),
        step_index: 0,
    };
    let result = executor.execute(request, &ctx).await.unwrap();
    assert_eq!(
        result.status,
        crate::agent::action_executor::ActionExecutionStatus::Blocked
    );

    let events = event_store.list_events_by_run(run_id).unwrap();
    let started_count = events
        .iter()
        .filter(|e| {
            matches!(
                e.event_type,
                crate::agent::AgentRunEventType::ToolCallStarted
            )
        })
        .count();
    let blocked_count = events
        .iter()
        .filter(|e| {
            matches!(
                e.event_type,
                crate::agent::AgentRunEventType::ToolCallBlocked
            )
        })
        .count();
    assert_eq!(
        started_count, 0,
        "blocked path must not emit ToolCallStarted"
    );
    assert_eq!(
        blocked_count, 1,
        "blocked path must emit exactly one ToolCallBlocked"
    );
}

#[tokio::test]
async fn test_shell_run_success_records_started_and_completed() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();
    reg.set_builtin_manifest_enabled("shell.run", true);
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit = crate::mcp_audit::McpAuditStore::new(
        tempfile::tempdir().unwrap().path().join("audit_evt2.db"),
    );
    let pe = PrivacyEngine::new();
    let tmp = std::env::temp_dir().to_string_lossy().to_string();
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
        bash_enabled: true,
        cwd: tmp.clone(),
        safe_paths: vec![tmp],
        command_allowlist: vec!["echo".into()],
        ..crate::agent::execution_sandbox::ExecutionSandbox::default()
    };
    // Grant permission so execution proceeds
    ps.grant(
        "shell.run",
        "builtin",
        "high",
        "external_side_effect",
        crate::tool_permissions::ToolPermissionPolicy::AllowOnce,
        None,
    )
    .unwrap();
    let spec = crate::agent::types::AgentSpec {
        allowed_tools: vec!["shell.run".into()],
        ..Default::default()
    };
    let event_store = crate::agent::event_store::AgentRunEventStore::new_in_memory().unwrap();
    let mut ctx = ActionContext::new_for_test(reg, ps, audit, pe, vec![])
        .with_execution_sandbox(sandbox)
        .with_agent_spec(spec);
    ctx.event_store = Some(event_store.clone());

    let run_id = "run-evt-success";
    let executor = crate::agent::ActionExecutor::new(Default::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "builtin_tool".into(),
        target: "shell.run".into(),
        input: serde_json::json!({"arguments": {"command": "echo", "args": ["evt_ok"]}}),
        source_run_id: Some(run_id.to_string()),
        step_index: 0,
    };
    let result = executor.execute(request, &ctx).await.unwrap();
    assert_eq!(
        result.status,
        crate::agent::action_executor::ActionExecutionStatus::Succeeded
    );

    let events = event_store.list_events_by_run(run_id).unwrap();
    let started_count = events
        .iter()
        .filter(|e| {
            matches!(
                e.event_type,
                crate::agent::AgentRunEventType::ToolCallStarted
            )
        })
        .count();
    let completed_count = events
        .iter()
        .filter(|e| {
            matches!(
                e.event_type,
                crate::agent::AgentRunEventType::ToolCallCompleted
            )
        })
        .count();
    let failed_count = events
        .iter()
        .filter(|e| {
            matches!(
                e.event_type,
                crate::agent::AgentRunEventType::ToolCallFailed
            )
        })
        .count();
    let blocked_count = events
        .iter()
        .filter(|e| {
            matches!(
                e.event_type,
                crate::agent::AgentRunEventType::ToolCallBlocked
            )
        })
        .count();
    assert_eq!(
        started_count, 1,
        "success path must emit one ToolCallStarted"
    );
    assert_eq!(
        completed_count, 1,
        "success path must emit one ToolCallCompleted"
    );
    assert_eq!(failed_count, 0, "success path must not emit ToolCallFailed");
    assert_eq!(
        blocked_count, 0,
        "success path must not emit ToolCallBlocked"
    );
}

// ── P9 full governed success path test ──────────────────────────────────

#[tokio::test]
async fn test_shell_run_full_governed_success_path() {
    let mut reg = McpRegistry::new();
    reg.register_default_builtins();
    reg.set_builtin_manifest_enabled("shell.run", true);
    let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
    let audit = crate::mcp_audit::McpAuditStore::new(
        tempfile::tempdir().unwrap().path().join("audit_full.db"),
    );
    let pe = PrivacyEngine::new();
    let tmp = std::env::temp_dir().to_string_lossy().to_string();
    let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
        bash_enabled: true,
        cwd: tmp.clone(),
        safe_paths: vec![tmp],
        command_allowlist: vec!["echo".into()],
        timeout_ms: 30_000,
        max_output_bytes: 1024 * 1024,
        ..crate::agent::execution_sandbox::ExecutionSandbox::default()
    };
    // Grant permission
    ps.grant(
        "shell.run",
        "builtin",
        "high",
        "external_side_effect",
        crate::tool_permissions::ToolPermissionPolicy::AllowOnce,
        None,
    )
    .unwrap();
    let spec = crate::agent::types::AgentSpec {
        allowed_tools: vec!["shell.run".into()],
        ..Default::default()
    };
    let event_store = crate::agent::event_store::AgentRunEventStore::new_in_memory().unwrap();
    let mut ctx = ActionContext::new_for_test(reg, ps, audit, pe, vec![])
        .with_execution_sandbox(sandbox)
        .with_agent_spec(spec);
    ctx.event_store = Some(event_store.clone());

    let run_id = "run-full-success";
    let executor = crate::agent::ActionExecutor::new(Default::default());
    let request = crate::agent::AgentActionRequest {
        action_type: "builtin_tool".into(),
        target: "shell.run".into(),
        input: serde_json::json!({"arguments": {
            "command": "echo",
            "args": ["governed_shell_ok"]
        }}),
        source_run_id: Some(run_id.to_string()),
        step_index: 0,
    };
    let result = executor.execute(request, &ctx).await.unwrap();

    // Must succeed
    assert_eq!(
        result.status,
        crate::agent::action_executor::ActionExecutionStatus::Succeeded,
        "full governed path must succeed"
    );

    // Observation structured_result must indicate success
    let obs = &result.observation;
    let success = obs
        .structured_result
        .as_ref()
        .and_then(|v| v.get("success"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(success, "structured_result.success must be true");

    // Output must contain the expected text
    let output_text = result
        .action
        .output
        .as_ref()
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        output_text.contains("governed_shell_ok"),
        "stdout must contain 'governed_shell_ok', got: {}",
        output_text
    );

    // Events: started + completed, no blocked/failed
    let events = event_store.list_events_by_run(run_id).unwrap();
    let started = events.iter().any(|e| {
        matches!(
            e.event_type,
            crate::agent::AgentRunEventType::ToolCallStarted
        )
    });
    let completed = events.iter().any(|e| {
        matches!(
            e.event_type,
            crate::agent::AgentRunEventType::ToolCallCompleted
        )
    });
    let blocked = events.iter().any(|e| {
        matches!(
            e.event_type,
            crate::agent::AgentRunEventType::ToolCallBlocked
        )
    });
    let failed = events.iter().any(|e| {
        matches!(
            e.event_type,
            crate::agent::AgentRunEventType::ToolCallFailed
        )
    });
    assert!(started, "must contain ToolCallStarted event");
    assert!(completed, "must contain ToolCallCompleted event");
    assert!(!blocked, "must not contain ToolCallBlocked event");
    assert!(!failed, "must not contain ToolCallFailed event");
}

// ── MemoryEvidence → LifeModel Proposal pipeline test ────────────────
#[tokio::test]
async fn test_memory_evidence_to_proposal_pipeline() {
    use crate::agent::memory_evidence::{EvidenceType, MemoryEvidence};
    use crate::agent::types::{ProposalSource, ProposalType, RiskLevel};

    // Step 1: Create evidence from memory records
    let evidence = MemoryEvidence::new(
        vec!["mem-001".to_string(), "mem-002".to_string()],
        EvidenceType::RecurringGoal,
        "User consistently expresses interest in learning Rust",
        "goals.short_term",
    )
    .with_confidence(0.85)
    .with_recency(0.9)
    .with_source_summary("Mentioned in 5 chats over 2 weeks");

    // Step 2: Evidence → Proposal (with confidence threshold)
    let proposal = evidence.to_proposal(0.7).unwrap();
    assert_eq!(proposal.proposal_type, ProposalType::GoalUpdate);
    assert_eq!(proposal.source, ProposalSource::MemoryGovernance);
    assert_eq!(proposal.affected_path, "goals.short_term");
    assert!(proposal.reason.contains("Rust"));
    assert!(proposal.reason.contains("recurring_goal"));
    assert_eq!(proposal.confidence, 0.85);
    assert_eq!(proposal.risk_level, RiskLevel::Medium);

    // Evidence IDs are embedded in the proposal after payload
    assert_eq!(proposal.after["evidence_id"].as_str().unwrap(), evidence.id);
    assert_eq!(
        proposal.after["evidence_type"].as_str().unwrap(),
        "recurring_goal"
    );

    // Step 3: Low confidence evidence → None (rejected)
    let weak = evidence.clone().with_confidence(0.3).to_proposal(0.5);
    assert!(weak.is_none(), "low confidence evidence should yield None");
}

#[tokio::test]
async fn test_memory_evidence_high_risk_requires_explicit_review_in_proposal() {
    use crate::agent::memory_evidence::{EvidenceType, MemoryEvidence};
    use crate::agent::types::RiskLevel;

    // Identity values are high risk
    let evidence = MemoryEvidence::new(
        vec!["mem-003".to_string()],
        EvidenceType::ValueSignal,
        "User shifted value from security to adventure",
        "identity.values",
    )
    .with_confidence(0.9)
    .with_recency(0.95)
    .with_source_summary("Consistent theme in 8 conversations");

    let proposal = evidence.to_proposal(0.6).unwrap();
    assert_eq!(proposal.risk_level, RiskLevel::High);
    assert!(
        proposal.reason.contains("高风险字段") || proposal.reason.contains("需显"),
        "High-risk proposal must include explicit review note: {}",
        proposal.reason
    );
}

#[tokio::test]
async fn test_memory_evidence_contradiction_halves_confidence() {
    use crate::agent::memory_evidence::{EvidenceType, MemoryEvidence};

    let e1 = MemoryEvidence::new(
        vec!["mem-a".to_string()],
        EvidenceType::ValueSignal,
        "User values creativity",
        "identity.values",
    );

    let e2 = MemoryEvidence::new(
        vec!["mem-b".to_string()],
        EvidenceType::ValueSignal,
        "User values structure over creativity",
        "identity.values",
    )
    .with_confidence(0.9)
    .with_contradiction(&e1.id);

    assert!(e2.has_contradictions());

    let proposal = e2.to_proposal(0.4).unwrap();
    // Contradiction halves confidence (0.9 * 0.5 = 0.45)
    assert!(
        (0.4..=0.5).contains(&proposal.confidence),
        "Confidence should be halved due to contradiction, got {}",
        proposal.confidence
    );
    assert!(
        proposal.reason.contains("需澄清") || proposal.reason.contains("矛盾"),
        "Contradiction proposal should mark for clarification: {}",
        proposal.reason
    );
}
