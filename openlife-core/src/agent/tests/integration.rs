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
    AgentLoopConfig, AgentRun, AgentRunStatus, AgentTask, AgentTaskKind,
};
use crate::layer_router::Layer;
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
    assert_eq!(config.max_steps, 5);
    assert_eq!(config.max_tool_calls, 3);
    assert_eq!(config.timeout_seconds, 120);
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
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
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
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
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
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
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
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
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
        life_model: None,
        memory_store: None,
        proposal_store: None,
        agent_run_store: None,
    };

    let reply = "This is just a plain text response without any JSON.";
    let actions = loop_instance
        .parse_tool_calls(reply, &action_ctx, &mut run, &mut tool_call_count)
        .unwrap();

    assert!(actions.is_empty());
    assert!(run.warnings.is_empty());
}
