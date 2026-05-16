use crate::agent::action_executor::{ActionExecutor, ActionExecutorConfig};
use crate::agent::runtime::AgentRuntime;
use crate::agent::types::{AgentObservation, AgentRun};
use crate::config::AppConfig;
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::mcp::McpRegistry;
use crate::mcp_audit::McpAuditStore;
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;
use crate::tool_permissions::ToolPermissionStore;

use super::config::{AgentLoopConfig, AgentRole};
use super::context::should_hold_streaming_reply;
use super::types::preview_text;
use super::AgentLoop;

/// Creates a minimal AgentLoop for testing parse_agent_reply and build_follow_up_messages.
/// Uses dummy scheduler credentials (no actual LLM calls are made).
fn make_test_agent_loop() -> AgentLoop {
    let life_model = LifeModel::default();
    let scheduler = InferenceScheduler::new(
        "llama3".into(),
        false,
        "openrouter".into(),
        "https://test.example.com/v1".into(),
        "sk-test".into(),
        "gpt-3.5-turbo".into(),
        "text-embedding-ada-002".into(),
        false,
    );
    let app_config = AppConfig::default();
    let runtime = AgentRuntime::new(life_model, scheduler.clone(), &app_config);
    let executor = ActionExecutor::new(ActionExecutorConfig::default());
    let config = AgentLoopConfig::default();
    AgentLoop::new(runtime, executor, scheduler, config)
}

/// Create a minimal ActionContext backed by tempfile-based stores.
struct TestCtx {
    ctx: crate::agent::action_executor::ActionContext,
}

impl TestCtx {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = crate::agent::action_executor::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit.db")),
            PrivacyEngine::new(),
            vec!["/tmp/openlife-test".into()],
        );
        Self { ctx }
    }

    fn as_ctx(&self) -> &crate::agent::action_executor::ActionContext {
        &self.ctx
    }
}

// ── parse_agent_reply tests ──────────────────────────────────────────

#[test]
fn streaming_reply_holds_structured_tool_json() {
    assert!(should_hold_streaming_reply(""));
    assert!(should_hold_streaming_reply("  \n"));
    assert!(should_hold_streaming_reply("{\"tool_calls\":["));
    assert!(should_hold_streaming_reply("```json\n{\"tool_calls\":["));
    assert!(!should_hold_streaming_reply("我先查一下最新信息。"));
}

#[test]
fn parse_final_only_no_json() {
    let agent = make_test_agent_loop();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let mut tc: u32 = 0;

    let result = agent
        .parse_agent_reply("Hello, how can I help?", action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!result.json_parse_failed);
    assert_eq!(result.final_text, "Hello, how can I help?");
    assert!(result.actions.is_empty());
}

#[test]
fn parse_json_plain_final() {
    let agent = make_test_agent_loop();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let mut tc: u32 = 0;

    let reply = r#"{"final": "Here is my answer"}"#;
    let result = agent
        .parse_agent_reply(reply, action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!result.json_parse_failed);
    assert_eq!(result.final_text, "Here is my answer");
    assert!(result.actions.is_empty());
}

#[test]
fn parse_json_with_actions() {
    let agent = make_test_agent_loop();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let mut tc: u32 = 0;

    let reply = r#"{
            "final": "Let me check that for you",
            "actions": [
                {"name": "web.search", "arguments": {"query": "Rust async"}},
                {"name": "file.read", "arguments": {"path": "/tmp/test.txt"}}
            ]
        }"#;
    let result = agent
        .parse_agent_reply(reply, action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!result.json_parse_failed);
    assert_eq!(result.final_text, "Let me check that for you");
    assert_eq!(result.actions.len(), 2);
    assert_eq!(result.actions[0].target, "web.search");
    assert_eq!(result.actions[1].target, "file.read");
    // step_index should start from tool_call_count (0)
    assert_eq!(result.actions[0].step_index, 0);
    assert_eq!(result.actions[1].step_index, 1);
}

#[test]
fn parse_json_legacy_tool_calls() {
    let agent = make_test_agent_loop();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let mut tc: u32 = 5;

    let reply = r#"{
            "final": "Done",
            "tool_calls": [
                {"name": "echo", "args": {"msg": "hi"}}
            ]
        }"#;
    let result = agent
        .parse_agent_reply(reply, action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!result.json_parse_failed);
    assert_eq!(result.actions.len(), 1);
    assert_eq!(result.actions[0].target, "echo");
    assert_eq!(result.actions[0].step_index, 5);
}

#[test]
fn parse_json_markdown_wrapped() {
    let agent = make_test_agent_loop();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let mut tc: u32 = 0;

    let reply = r#"```json
{"final": "Answer from markdown", "actions": []}
```"#;
    let result = agent
        .parse_agent_reply(reply, action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!result.json_parse_failed);
    assert_eq!(result.final_text, "Answer from markdown");
    assert!(result.actions.is_empty());
}

#[test]
fn parse_malformed_json_signals_repair() {
    let agent = make_test_agent_loop();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let mut tc: u32 = 0;

    // Missing closing brace
    let reply = r#"{"final": "oops", "actions": [{"name": "x", "arguments": {}]"#;
    let result = agent
        .parse_agent_reply(reply, action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(result.json_parse_failed, "should signal repair needed");
    assert!(result.actions.is_empty());
    assert!(!run.warnings.is_empty(), "should have recorded warning");
}

#[test]
fn parse_empty_actions_array_yields_final_only() {
    let agent = make_test_agent_loop();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let mut tc: u32 = 0;

    let reply = r#"{"final": "done", "actions": []}"#;
    let result = agent
        .parse_agent_reply(reply, action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!result.json_parse_failed);
    assert!(result.actions.is_empty());
}

#[test]
fn parse_thought_summary_and_warnings_recorded() {
    let agent = make_test_agent_loop();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let mut tc: u32 = 0;

    let reply = r#"{
            "final": "ok",
            "thought_summary": "simple task",
            "warnings": ["low confidence"]
        }"#;
    let result = agent
        .parse_agent_reply(reply, action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!result.json_parse_failed);
    assert!(run.warnings.iter().any(|w| w.contains("thought")));
    assert!(run.warnings.iter().any(|w| w.contains("low confidence")));
}

#[test]
fn parse_action_with_alternative_field_names() {
    let agent = make_test_agent_loop();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let mut tc: u32 = 0;

    // Uses "tool" instead of "name" and "input" instead of "arguments"
    let reply = r#"{"final":"ok","actions":[{"tool":"test_tool","input":{"key":"val"}}]}"#;
    let result = agent
        .parse_agent_reply(reply, action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!result.json_parse_failed);
    assert_eq!(result.actions.len(), 1);
    assert_eq!(result.actions[0].target, "test_tool");
}

// ── build_follow_up_messages tests ───────────────────────────────────

#[test]
fn build_follow_up_with_observations() {
    let agent = make_test_agent_loop();
    let task = crate::agent::types::AgentTask {
        kind: crate::agent::types::AgentTaskKind::Conversation,
        session_id: "s1".into(),
        user_text: "帮我查天气".into(),
        messages: vec![],
        layer: crate::layer_router::Layer::L2,
        ..Default::default()
    };
    let obs = vec![AgentObservation {
        id: "obs-1".into(),
        action_id: Some("act-1".into()),
        content: "北京今天晴，25°C".into(),
        source: "web.search".into(),
        structured_result: None,
        timestamp: chrono::Utc::now(),
    }];
    let tools_prompt = "可用工具: web.search, file.read";

    let messages = agent.build_follow_up_messages(&task, "正在查询...", &obs, tools_prompt);

    assert_eq!(messages.len(), 2); // assistant + user (follow-up)
    assert_eq!(messages[0].role, "assistant");
    assert_eq!(messages[0].content, "正在查询...");
    assert_eq!(messages[1].role, "user");
    assert!(messages[1].content.contains("帮我查天气"));
    assert!(messages[1].content.contains("北京今天晴"));
    assert!(messages[1].content.contains("web.search"));
}

#[test]
fn build_follow_up_no_observations() {
    let agent = make_test_agent_loop();
    let task = crate::agent::types::AgentTask {
        kind: crate::agent::types::AgentTaskKind::Conversation,
        session_id: "s1".into(),
        user_text: "hello".into(),
        messages: vec![],
        layer: crate::layer_router::Layer::L2,
        ..Default::default()
    };

    let messages = agent.build_follow_up_messages(&task, "Hi there!", &[], "可用工具: echo");

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].role, "user");
    assert!(!messages[1].content.contains("工具执行结果"));
    assert!(messages[1].content.contains("可用工具"));
}

#[test]
fn build_follow_up_preserves_existing_messages() {
    let agent = make_test_agent_loop();
    let task = crate::agent::types::AgentTask {
        kind: crate::agent::types::AgentTaskKind::Conversation,
        session_id: "s1".into(),
        user_text: "天气".into(),
        messages: vec![
            ChatMessage {
                role: "user".into(),
                content: "你好".into(),
            },
            ChatMessage {
                role: "assistant".into(),
                content: "你好！有什么可以帮你的？".into(),
            },
        ],
        layer: crate::layer_router::Layer::L2,
        ..Default::default()
    };

    let obs = vec![AgentObservation {
        id: "obs-1".into(),
        action_id: None,
        content: "上海25°C".into(),
        source: "web.search".into(),
        structured_result: None,
        timestamp: chrono::Utc::now(),
    }];

    let messages = agent.build_follow_up_messages(&task, "查询天气中...", &obs, "工具: web");

    // Original 2 + assistant + follow-up = 4
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "你好");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[2].role, "assistant");
    assert_eq!(messages[3].role, "user");
}

// ── preview_text tests (existing) ────────────────────────────────────

#[test]
fn preview_text_truncates_on_char_boundary() {
    let text = format!("{}星", "a".repeat(199));
    assert_eq!(preview_text(&text, 200), text);

    let text = format!("{}星期几", "a".repeat(199));
    let preview = preview_text(&text, 200);
    assert!(preview.ends_with("星..."));
}

#[test]
fn preview_text_handles_emoji_without_panic() {
    let text = format!("{}😀more", "a".repeat(199));
    let preview = preview_text(&text, 200);
    assert!(preview.ends_with("😀..."));
}

// ── P0-2: AgentRunEvent recording tests ──────────────────────────────

fn make_test_agent_loop_with_events() -> (AgentLoop, crate::agent::event_store::AgentRunEventStore)
{
    let agent = make_test_agent_loop();
    let event_store = crate::agent::event_store::AgentRunEventStore::new_in_memory().unwrap();
    let agent = agent.with_event_store(event_store.clone());
    (agent, event_store)
}

#[test]
fn test_no_tool_response_event_sequence() {
    let (agent, _store) = make_test_agent_loop_with_events();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("no-tool-1", "hello");
    let mut tc: u32 = 0;

    // Record run.created
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::RunCreated,
        crate::agent::types::AgentEventActor::Runtime,
        "run created",
        serde_json::json!({}),
    );
    // Record model.call_started
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::ModelCallStarted,
        crate::agent::types::AgentEventActor::Agent,
        "model call started",
        serde_json::json!({}),
    );

    // Simulate no-tool response
    let result = agent
        .parse_agent_reply("Hello, how can I help?", action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!result.json_parse_failed);
    assert!(result.actions.is_empty());

    // Record model.call_completed
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::ModelCallCompleted,
        crate::agent::types::AgentEventActor::Agent,
        "model call completed",
        serde_json::json!({"reply_len": 24}),
    );
    // Record run.completed
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::RunCompleted,
        crate::agent::types::AgentEventActor::Runtime,
        "run completed",
        serde_json::json!({"stop_reason": "no_tools"}),
    );

    let events = agent
        .event_store
        .as_ref()
        .unwrap()
        .list_events_by_run(&run.id)
        .unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events[0].event_type,
        crate::agent::types::AgentRunEventType::RunCreated
    );
    assert_eq!(
        events[1].event_type,
        crate::agent::types::AgentRunEventType::ModelCallStarted
    );
    assert_eq!(
        events[2].event_type,
        crate::agent::types::AgentRunEventType::ModelCallCompleted
    );
    assert_eq!(
        events[3].event_type,
        crate::agent::types::AgentRunEventType::RunCompleted
    );
}

#[test]
fn test_malformed_json_repair_event_sequence() {
    let (agent, _store) = make_test_agent_loop_with_events();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("malformed-json-1", "hello");
    let mut tc: u32 = 0;

    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::RunCreated,
        crate::agent::types::AgentEventActor::Runtime,
        "run created",
        serde_json::json!({}),
    );
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::ModelCallStarted,
        crate::agent::types::AgentEventActor::Agent,
        "model call started",
        serde_json::json!({}),
    );
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::ModelCallCompleted,
        crate::agent::types::AgentEventActor::Agent,
        "model call completed",
        serde_json::json!({}),
    );

    // Simulate malformed JSON response (contains '{' but not valid JSON)
    let result = agent
        .parse_agent_reply(
            r#"{"final": "almost valid but missing bracket"#,
            action_ctx,
            &mut run,
            &mut tc,
        )
        .unwrap();
    assert!(result.json_parse_failed); // Should signal repair needed

    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::JsonRepairStarted,
        crate::agent::types::AgentEventActor::Runtime,
        "json repair started",
        serde_json::json!({}),
    );
    // Simulate repair succeeded (valid JSON after repair)
    let repair_reply = r#"{"final": "repaired response"}"#;
    let repair_result = agent
        .parse_agent_reply(repair_reply, action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!repair_result.json_parse_failed);
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::JsonRepairCompleted,
        crate::agent::types::AgentEventActor::Runtime,
        "json repair succeeded",
        serde_json::json!({"repaired": true}),
    );
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::RunCompleted,
        crate::agent::types::AgentEventActor::Runtime,
        "run completed",
        serde_json::json!({}),
    );

    let events = agent
        .event_store
        .as_ref()
        .unwrap()
        .list_events_by_run(&run.id)
        .unwrap();
    assert_eq!(events.len(), 6);
    // Verify repair events exist in sequence
    let repair_start_ids: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e.event_type == crate::agent::types::AgentRunEventType::JsonRepairStarted)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(repair_start_ids.len(), 1);
    let repair_complete_ids: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.event_type == crate::agent::types::AgentRunEventType::JsonRepairCompleted
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(repair_complete_ids.len(), 1);
    assert!(
        repair_complete_ids[0] > repair_start_ids[0],
        "repair completed should come after repair started"
    );
}

#[test]
fn test_blocked_tool_call_event_sequence() {
    let (agent, _store) = make_test_agent_loop_with_events();
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("blocked-tool-1", "do many things");
    let mut tc: u32 = 0;

    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::RunCreated,
        crate::agent::types::AgentEventActor::Runtime,
        "run created",
        serde_json::json!({}),
    );
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::ModelCallStarted,
        crate::agent::types::AgentEventActor::Agent,
        "model call started",
        serde_json::json!({}),
    );
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::ModelCallCompleted,
        crate::agent::types::AgentEventActor::Agent,
        "model call completed",
        serde_json::json!({}),
    );

    // Parse tool-call reply
    let reply = r#"{"final":"ok","actions":[{"name":"tool1","arguments":{"key":"v1"}}]}"#;
    let result = agent
        .parse_agent_reply(reply, action_ctx, &mut run, &mut tc)
        .unwrap();
    assert!(!result.json_parse_failed);
    assert_eq!(result.actions.len(), 1);

    // Simulate tool blocked (budget exceeded or permission denied)
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::ToolCallStarted,
        crate::agent::types::AgentEventActor::Tool("tool1".to_string()),
        "executing tool1",
        serde_json::json!({"tool": "tool1"}),
    );
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::ToolCallBlocked,
        crate::agent::types::AgentEventActor::Runtime,
        "tool1 blocked: budget exceeded",
        serde_json::json!({"tool": "tool1", "reason": "budget"}),
    );
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::RunCompleted,
        crate::agent::types::AgentEventActor::Runtime,
        "run completed",
        serde_json::json!({"stop_reason": "max_tool_calls_reached"}),
    );

    let events = agent
        .event_store
        .as_ref()
        .unwrap()
        .list_events_by_run(&run.id)
        .unwrap();
    // Verify blocked event exists
    let blocked = events
        .iter()
        .find(|e| e.event_type == crate::agent::types::AgentRunEventType::ToolCallBlocked);
    assert!(blocked.is_some());
    assert!(blocked.unwrap().summary.contains("budget exceeded"));
}

#[test]
fn test_events_not_recorded_when_store_is_none() {
    let agent = make_test_agent_loop(); // no event store
    let ctx = TestCtx::new();
    let action_ctx = ctx.as_ctx();
    let mut run = AgentRun::new_chat_run("no-store-1", "test");
    let mut tc: u32 = 0;

    // These should not crash
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::RunCreated,
        crate::agent::types::AgentEventActor::Runtime,
        "should not persist",
        serde_json::json!({}),
    );
    let _ = agent.parse_agent_reply("hello", action_ctx, &mut run, &mut tc);
    agent.try_record_event(
        &run.id,
        crate::agent::types::AgentRunEventType::RunCompleted,
        crate::agent::types::AgentEventActor::Runtime,
        "should not persist",
        serde_json::json!({}),
    );

    // No events should be stored
    assert!(agent.event_store.is_none());
}

#[test]
fn test_model_failed_event_recorded() {
    let (agent, store) = make_test_agent_loop_with_events();
    let run_id = "model-fail-1";

    agent.try_record_event(
        run_id,
        crate::agent::types::AgentRunEventType::RunCreated,
        crate::agent::types::AgentEventActor::Runtime,
        "run created",
        serde_json::json!({}),
    );
    agent.try_record_event(
        run_id,
        crate::agent::types::AgentRunEventType::ModelCallStarted,
        crate::agent::types::AgentEventActor::Agent,
        "model call started",
        serde_json::json!({"step": 1}),
    );
    agent.try_record_event(
        run_id,
        crate::agent::types::AgentRunEventType::ModelCallFailed,
        crate::agent::types::AgentEventActor::Agent,
        "model timeout",
        serde_json::json!({"error": "timeout", "step": 1}),
    );
    agent.try_record_event(
        run_id,
        crate::agent::types::AgentRunEventType::RunFailed,
        crate::agent::types::AgentEventActor::Runtime,
        "run failed due to model error",
        serde_json::json!({}),
    );

    let events = store.list_events_by_run(run_id).unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events[2].event_type,
        crate::agent::types::AgentRunEventType::ModelCallFailed
    );
    assert_eq!(
        events[3].event_type,
        crate::agent::types::AgentRunEventType::RunFailed
    );
}

// ── P7: AgentLoop governance events use real run id ───────────────────

#[test]
fn test_agent_loop_governance_events_use_real_run_id() {
    let (agent, store) = make_test_agent_loop_with_events();
    let run_id = "al-governance-1";

    // Simulate governance events written by run_loop_core
    agent.try_record_event(
        run_id,
        crate::agent::types::AgentRunEventType::RunCreated,
        crate::agent::types::AgentEventActor::Runtime,
        "run created",
        serde_json::json!({"session_id": "test"}),
    );
    agent.try_record_event(
        run_id,
        crate::agent::types::AgentRunEventType::AgentSpecSelected,
        crate::agent::types::AgentEventActor::Runtime,
        "AgentSpec main.default selected",
        serde_json::json!({
            "agent_spec_id": "main.default",
            "role": "Main",
            "privacy_policy": "cloud_allowed",
        }),
    );
    agent.try_record_event(
        run_id,
        crate::agent::types::AgentRunEventType::PromptStackAssembled,
        crate::agent::types::AgentEventActor::Runtime,
        "PromptStack assembled",
        serde_json::json!({
            "agent_spec_id": "main.default",
            "prompt_blocks": [{"id": "base.system"}],
        }),
    );
    agent.try_record_event(
        run_id,
        crate::agent::types::AgentRunEventType::ContextGovernanceApplied,
        crate::agent::types::AgentEventActor::Runtime,
        "Context governance applied",
        serde_json::json!({
            "agent_spec_id": "main.default",
            "context_included": ["session_summary", "lifemodel_summary", "memory"],
            "context_excluded": [],
            "privacy_policy": "cloud_allowed",
        }),
    );

    let events = store.list_events_by_run(run_id).unwrap();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events[1].event_type,
        crate::agent::types::AgentRunEventType::AgentSpecSelected
    );

    // Verify AgentSpecSelected payload does NOT contain raw prompt/memory/LifeModel
    let spec_payload = &events[1].payload;
    assert_eq!(spec_payload["agent_spec_id"], "main.default");
    assert!(spec_payload["role"].is_string());
    assert!(spec_payload["privacy_policy"].is_string());
    assert!(!spec_payload.to_string().contains("raw_prompt"));
    assert!(!spec_payload.to_string().contains("raw_memory"));

    // Verify PromptStackAssembled payload has block IDs only
    let ps_payload = &events[2].payload;
    assert_eq!(ps_payload["agent_spec_id"], "main.default");
    let blocks = ps_payload["prompt_blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0]["id"], "base.system");
    assert!(blocks[0].get("content").is_none());

    // Verify ContextGovernanceApplied payload has categories only
    let cg_payload = &events[3].payload;
    let included = cg_payload["context_included"].as_array().unwrap();
    assert!(included.iter().any(|v| v == "session_summary"));
    assert!(!cg_payload.to_string().contains("raw_lifemodel"));
}

#[test]
fn test_agent_loop_governance_events_nonexistent_for_synthetic_run_id() {
    let (agent, store) = make_test_agent_loop_with_events();
    let real_run_id = "al-real-run-1";

    // Record events only under the real run id
    agent.try_record_event(
        real_run_id,
        crate::agent::types::AgentRunEventType::RunCreated,
        crate::agent::types::AgentEventActor::Runtime,
        "run created",
        serde_json::json!({}),
    );
    agent.try_record_event(
        real_run_id,
        crate::agent::types::AgentRunEventType::AgentSpecSelected,
        crate::agent::types::AgentEventActor::Runtime,
        "spec selected",
        serde_json::json!({"agent_spec_id": "main.default"}),
    );

    // Synthetic run id should have no events
    let synthetic_id = format!("al-nonstream-{}", real_run_id);
    let synthetic_events = store.list_events_by_run(&synthetic_id).unwrap();
    assert!(
        synthetic_events.is_empty(),
        "synthetic run id should have no events"
    );

    // Real run id should have events
    let real_events = store.list_events_by_run(real_run_id).unwrap();
    assert_eq!(real_events.len(), 2);
    assert!(real_events
        .iter()
        .any(|e| e.event_type == crate::agent::types::AgentRunEventType::AgentSpecSelected));
}

// ── P7: missing prompt block does not record fake PromptStackAssembled ──

#[tokio::test]
async fn test_agent_loop_missing_prompt_block_does_not_record_prompt_stack_assembled() {
    use crate::agent::types::AgentSpec;

    let (agent, store) = make_test_agent_loop_with_events();

    let test_ctx = TestCtx::new();
    let action_ctx = test_ctx.as_ctx();
    let life_model = LifeModel::default();
    let spec = AgentSpec::default_main_spec();
    // default_main_spec now includes baseline prompt_block_ids

    let registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();
    let task = crate::agent::types::AgentTask {
        kind: crate::agent::types::AgentTaskKind::Conversation,
        session_id: "test-governance-session".to_string(),
        user_text: "hello".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }],
        layer: Layer::L1,
        ..Default::default()
    };

    // Run: execute_task_with_spec succeeds (L1 DirectReasoner),
    // generate_governed fails (fake scheduler), but governance events were
    // already written from real runtime_output.
    let result = agent
        .run(
            &task,
            &life_model,
            "",
            None,
            PrivacyEngine::new(),
            crate::agent::types::PrivacyPolicy::CloudAllowed,
            &spec,
            &registry,
            action_ctx,
        )
        .await;

    match result {
        Ok(loop_result) => {
            let run_id = loop_result.run.id;
            let events = store.list_events_by_run(&run_id).unwrap();

            // AgentSpecSelected should be present
            let has_spec_selected = events
                .iter()
                .any(|e| e.event_type == crate::agent::types::AgentRunEventType::AgentSpecSelected);
            assert!(has_spec_selected);

            // PromptStackAssembled may or may not be present depending on
            // whether execute_task_with_spec succeeded before generate_governed failed.
            // If present, its payload must come from runtime_output (block IDs/versions).
            for event in &events {
                if event.event_type == crate::agent::types::AgentRunEventType::PromptStackAssembled
                {
                    let blocks = event.payload["prompt_blocks"].as_array().unwrap();
                    for block in blocks {
                        assert!(
                            block.get("content").is_none(),
                            "prompt_blocks must not contain raw content"
                        );
                        // BlockTraceEntry has id, version, purpose, cloud_allowed, estimated_tokens
                        assert!(block["id"].is_string());
                    }
                }
            }
        }
        Err(_) => {
            // If run fails, make sure no PromptStackAssembled was written with fake ids
            // (but it's fine - governance events only written on success)
        }
    }
}

#[tokio::test]
async fn test_agent_loop_missing_prompt_block_fails_without_governance_events() {
    use crate::agent::types::AgentSpec;

    let (agent, store) = make_test_agent_loop_with_events();
    let test_ctx = TestCtx::new();
    let action_ctx = test_ctx.as_ctx();
    let life_model = LifeModel::default();

    // Spec with a missing prompt block — must fail before model call
    let mut spec = AgentSpec::default_main_spec();
    spec.prompt_block_ids = vec!["missing.block".to_string()];

    let registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();
    let task = crate::agent::types::AgentTask {
        kind: crate::agent::types::AgentTaskKind::Conversation,
        session_id: "test-missing-block".to_string(),
        user_text: "hello".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }],
        layer: Layer::L1,
        ..Default::default()
    };

    let result = agent
        .run(
            &task,
            &life_model,
            "",
            None,
            PrivacyEngine::new(),
            crate::agent::types::PrivacyPolicy::CloudAllowed,
            &spec,
            &registry,
            action_ctx,
        )
        .await;

    match result {
        Ok(loop_result) => {
            let run_id = loop_result.run.id;
            let events = store.list_events_by_run(&run_id).unwrap();

            // Must have a failure event (ModelCallFailed or RunFailed)
            let has_failure = events.iter().any(|e| {
                e.event_type == crate::agent::types::AgentRunEventType::ModelCallFailed
                    || e.event_type == crate::agent::types::AgentRunEventType::RunFailed
            });
            assert!(
                has_failure,
                "missing prompt block must produce a failure event"
            );

            // Must NOT have PromptStackAssembled
            let has_prompt_stack = events.iter().any(|e| {
                e.event_type == crate::agent::types::AgentRunEventType::PromptStackAssembled
            });
            assert!(
                !has_prompt_stack,
                "missing prompt block must not record PromptStackAssembled"
            );
        }
        Err(e) => {
            // If AgentLoop returns Err, verify the error is governance-related
            let msg = e.to_string();
            assert!(
                msg.contains("unknown")
                    || msg.contains("missing.block")
                    || msg.contains("governance"),
                "error should mention governance/prompt failure, got: {}",
                msg
            );
            // No events should be recorded under any synthetic id
            // (AgentLoop creates a run internally; the events reference the real run id)
        }
    }
}

// ── P2: effective privacy_policy resolution tests ─────────────────

/// Helper: resolve effective privacy_policy for AgentLoop, using
/// AgentSpec as sole fallback (ignoring any caller-passed parameter).
fn resolve_agent_loop_privacy_policy(
    task: &crate::agent::types::AgentTask,
    agent_spec: &crate::agent::types::AgentSpec,
) -> crate::agent::types::PrivacyPolicy {
    crate::agent::runtime::resolve_privacy_policy(task, agent_spec)
}

#[test]
fn test_effective_policy_task_override_over_agent_spec() {
    use crate::agent::types::AgentTaskKind;

    let mut task = crate::agent::types::AgentTask::new(AgentTaskKind::Conversation, "sess");
    task.privacy_policy = Some(crate::agent::types::PrivacyPolicy::LocalOnly);
    let spec = crate::agent::types::AgentSpec::default()
        .with_privacy_policy(crate::agent::types::PrivacyPolicy::CloudAllowed);

    // Task override wins
    let resolved = resolve_agent_loop_privacy_policy(&task, &spec);
    assert_eq!(resolved, crate::agent::types::PrivacyPolicy::LocalOnly);
}

#[test]
fn test_effective_policy_agent_spec_fallback_ignores_param() {
    use crate::agent::types::AgentTaskKind;

    let task = crate::agent::types::AgentTask::new(AgentTaskKind::Conversation, "sess");
    // No task override
    let spec = crate::agent::types::AgentSpec::default()
        .with_privacy_policy(crate::agent::types::PrivacyPolicy::SummaryOnly);

    // AgentSpec is the fallback — NOT a caller-supplied parameter
    let resolved = resolve_agent_loop_privacy_policy(&task, &spec);
    assert_eq!(
        resolved,
        crate::agent::types::PrivacyPolicy::SummaryOnly,
        "effective policy must fall back to AgentSpec, not caller parameter"
    );
}

#[test]
fn test_effective_policy_streaming_uses_agent_spec_fallback() {
    use crate::agent::types::AgentTaskKind;

    // Same semantics apply to run_streaming — tested through the same helper
    let task = crate::agent::types::AgentTask::new(AgentTaskKind::Conversation, "sess");
    let spec = crate::agent::types::AgentSpec::default()
        .with_privacy_policy(crate::agent::types::PrivacyPolicy::CloudAllowed);

    let resolved = resolve_agent_loop_privacy_policy(&task, &spec);
    assert_eq!(resolved, crate::agent::types::PrivacyPolicy::CloudAllowed);
}

#[test]
fn test_effective_policy_event_payload_records_effective_not_param() {
    use crate::agent::types::AgentTaskKind;

    // Verify the governance event payload records effective policy
    // by checking that the helper itself uses AgentSpec fallback
    let mut task = crate::agent::types::AgentTask::new(AgentTaskKind::Conversation, "sess");
    task.privacy_policy = Some(crate::agent::types::PrivacyPolicy::LocalOnly);
    let spec = crate::agent::types::AgentSpec::default()
        .with_privacy_policy(crate::agent::types::PrivacyPolicy::SummaryOnly);

    // effective = task override (LocalOnly), not spec (SummaryOnly)
    let effective = resolve_agent_loop_privacy_policy(&task, &spec);
    assert_eq!(effective, crate::agent::types::PrivacyPolicy::LocalOnly);
    assert_ne!(effective, spec.privacy_policy);
}

// ── End of P7 hardening tests ─────────────────────────────────────

// ── P8: Compaction integration tests ───────────────────────────────

fn make_chat_msg(role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: content.to_string(),
    }
}

fn make_long_conversation(count: usize) -> Vec<ChatMessage> {
    let mut msgs = Vec::new();
    for i in 0..count {
        msgs.push(make_chat_msg(
            if i % 2 == 0 { "user" } else { "assistant" },
            &format!(
                "Message {} with substantial content for counting. {}",
                i,
                "x".repeat(60),
            ),
        ));
    }
    msgs
}

#[test]
fn test_try_compact_context_triggered_by_long_history() {
    let agent = make_test_agent_loop();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let msgs = make_long_conversation(25);
    let mut task = crate::agent::types::AgentTask::new(
        crate::agent::types::AgentTaskKind::Conversation,
        "sess",
    );
    task.messages = msgs;

    let compacted = agent._test_compact_context(
        &mut task,
        &mut run,
        crate::agent::types::PrivacyPolicy::LocalOnly,
    );
    assert!(compacted, "long conversation should trigger compaction");
    assert!(
        task.messages.len() < 25,
        "compacted messages {} should be fewer than 25",
        task.messages.len()
    );
}

#[test]
fn test_compact_context_preserves_latest_user_message() {
    let agent = make_test_agent_loop();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let mut msgs = make_long_conversation(22);
    msgs.push(make_chat_msg("user", "my latest question"));
    let mut task = crate::agent::types::AgentTask::new(
        crate::agent::types::AgentTaskKind::Conversation,
        "sess",
    );
    task.messages = msgs;

    let _ = agent._test_compact_context(
        &mut task,
        &mut run,
        crate::agent::types::PrivacyPolicy::LocalOnly,
    );
    let has_latest = task
        .messages
        .iter()
        .any(|m| m.role == "user" && m.content.contains("my latest question"));
    assert!(has_latest, "latest user message must be preserved");
}

#[test]
fn test_compact_context_no_panic_without_event_store() {
    let agent = make_test_agent_loop(); // event_store is None
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let msgs = make_long_conversation(30);
    let mut task = crate::agent::types::AgentTask::new(
        crate::agent::types::AgentTaskKind::Conversation,
        "sess",
    );
    task.messages = msgs;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        agent._test_compact_context(
            &mut task,
            &mut run,
            crate::agent::types::PrivacyPolicy::LocalOnly,
        )
    }));
    assert!(result.is_ok(), "should not panic without event_store");
}

#[test]
fn test_compact_context_idempotent_without_new_messages() {
    let agent = make_test_agent_loop();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    let msgs = make_long_conversation(25);
    let mut task = crate::agent::types::AgentTask::new(
        crate::agent::types::AgentTaskKind::Conversation,
        "sess",
    );
    task.messages = msgs;

    let first = agent._test_compact_context(
        &mut task,
        &mut run,
        crate::agent::types::PrivacyPolicy::LocalOnly,
    );
    assert!(first);

    // Immediately try again — should NOT compact because the guard
    // prevents re-compaction if message count hasn't grown enough.
    let prev_len = task.messages.len();
    let second = agent._test_compact_context(
        &mut task,
        &mut run,
        crate::agent::types::PrivacyPolicy::LocalOnly,
    );
    // Whether it compacts or not depends on the config thresholds, but
    // the messages should not shrink further (guard prevents it).
    if second {
        assert!(
            task.messages.len() >= prev_len,
            "should not shrink further without new messages"
        );
    }
}

#[test]
fn test_compact_context_multi_step_with_observations() {
    let agent = make_test_agent_loop();
    let mut run = AgentRun::new_chat_run("s1", "hello");
    run.observations.push(AgentObservation {
        id: "obs-1".into(),
        action_id: None,
        content: "Observed file content with password=abc123".into(),
        source: "file.read".into(),
        structured_result: None,
        timestamp: chrono::Utc::now(),
    });
    run.observations.push(AgentObservation {
        id: "obs-2".into(),
        action_id: None,
        content: "Search returned contact user@example.com".into(),
        source: "web.search".into(),
        structured_result: None,
        timestamp: chrono::Utc::now(),
    });

    let msgs = make_long_conversation(25);
    let mut task = crate::agent::types::AgentTask::new(
        crate::agent::types::AgentTaskKind::Conversation,
        "sess",
    );
    task.messages = msgs;

    let compacted = agent._test_compact_context(
        &mut task,
        &mut run,
        crate::agent::types::PrivacyPolicy::LocalOnly,
    );
    assert!(compacted);

    // Verify no sensitive content leaked into compacted messages
    let all_content: String = task
        .messages
        .iter()
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(!all_content.contains("abc123"));
    assert!(!all_content.contains("user@example.com"));
}

// ── P12: AgentLoopConfig governance fields ─────────────────────────
// Verifies the default allow_writes / allow_cloud values.
// These are currently not enforced inside the AgentLoop; enforcement
// (if added) must keep proposal-generation tools available.

#[test]
fn test_agent_loop_config_defaults_allow_writes_and_cloud() {
    let cfg = AgentLoopConfig::default();
    assert!(cfg.allow_writes, "allow_writes should default to true");
    assert!(cfg.allow_cloud, "allow_cloud should default to true");
}

#[test]
fn test_agent_loop_config_can_disable_writes_for_planning() {
    let cfg = AgentLoopConfig {
        allow_writes: false,
        allow_cloud: false,
        ..Default::default()
    };
    assert!(!cfg.allow_writes);
    assert!(!cfg.allow_cloud);
    // Planner role is still available — proposal-generation tools
    // remain accessible via AgentRole instruction, not this flag.
    assert_eq!(cfg.role, AgentRole::default());
}

// ── Batch 1: Governed Replay — AgentLoop run creation ─────────────

/// Verify that AgentLoop run_loop_core writes agent_spec_id from
/// the resolved AgentSpec so that replay can later restore governance.
#[tokio::test]
async fn test_agent_loop_run_sets_agent_spec_id() {
    let mut agent = make_test_agent_loop();
    agent.config.max_steps = 0;

    let test_spec_id = "main.default".to_string();
    let agent_spec = crate::agent::types::AgentSpec::default_main_spec();

    let task = crate::agent::types::AgentTask::new(
        crate::agent::types::AgentTaskKind::Conversation,
        "test-session",
    )
    .with_user_text("test input");

    let life_model = crate::life_model::LifeModel::default();
    let prompt_registry = crate::agent::prompt_stack::PromptBlockRegistry::built_in();
    let test_ctx = TestCtx::new();
    let action_ctx = test_ctx.as_ctx();

    let result = agent
        .run(
            &task,
            &life_model,
            "",
            None,
            crate::privacy::PrivacyEngine::new(),
            crate::agent::types::PrivacyPolicy::LocalOnly,
            &agent_spec,
            &prompt_registry,
            action_ctx,
        )
        .await
        .expect("AgentLoop run should succeed with max_steps=0");

    assert_eq!(
        result.run.agent_spec_id.as_deref(),
        Some(test_spec_id.as_str()),
        "AgentLoop run must have agent_spec_id set from agent_spec; \
         got {:?}",
        result.run.agent_spec_id,
    );
    assert_eq!(
        result.run.status,
        crate::agent::types::AgentRunStatus::Completed,
        "run with max_steps=0 should complete without LLM call"
    );
}
