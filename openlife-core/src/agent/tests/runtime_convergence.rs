use crate::agent::{
    ActionExecutionContext, ActionExecutor, ActionExecutorConfig, AgentExecutionBudget, AgentLoop,
    AgentLoopConfig, AgentTask, AgentTaskKind, ModelRouter, ProviderAvailability, RuntimeInput,
};
use crate::layer_router::Layer;
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;

fn no_network_scheduler() -> InferenceScheduler {
    let mut router = ModelRouter::new();
    router.providers.insert(
        "deepseek".into(),
        ProviderAvailability {
            provider: "deepseek".into(),
            available: true,
            latency_ms: Some(50),
            models: vec!["deepseek-chat".into()],
            last_checked: chrono::Utc::now(),
            last_error: None,
            health_is_estimated: false,
        },
    );

    InferenceScheduler {
        prefer_local: false,
        provider: "contract-test".into(),
        openai_key: String::new(),
        model_router: Some(router),
        ..InferenceScheduler::default()
    }
}

fn test_task(text: &str) -> AgentTask {
    AgentTask {
        kind: AgentTaskKind::Conversation,
        session_id: "session-runtime-convergence".into(),
        user_text: text.into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: text.into(),
        }],
        layer: Layer::L2,
    }
}

fn test_runtime() -> crate::agent::AgentRuntime {
    crate::agent::AgentRuntime::with_config(
        LifeModel::default(),
        no_network_scheduler(),
        crate::agent::AgentRuntimeConfig::default(),
    )
}

fn test_agent_loop(config: AgentLoopConfig) -> AgentLoop {
    let scheduler = no_network_scheduler();
    let runtime = crate::agent::AgentRuntime::with_config(
        LifeModel::default(),
        scheduler.clone(),
        crate::agent::AgentRuntimeConfig::default(),
    );
    let action_executor = ActionExecutor::new(ActionExecutorConfig::default());
    AgentLoop::new(runtime, action_executor, scheduler, config)
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

#[tokio::test]
async fn runtime_input_executes_through_agent_runtime_and_returns_runtime_output() {
    let input = RuntimeInput::from_agent_task(
        test_task("Summarize the current context without writing anything."),
        LifeModel::default(),
        Some("memory: prefers concise answers".into()),
        "Available tools: memory.search",
        None,
        AgentExecutionBudget::default(),
    );

    let output = test_runtime().execute_runtime_input(input).await.unwrap();

    assert!(output.run_id.is_some());
    assert!(!output.user_output.trim().is_empty());
    assert!(output.actions.is_empty());
    assert!(output.observations.is_empty());
    assert!(output.proposal_ids.is_empty());
    assert!(output.life_event_candidates.is_empty());
    assert!(output.warnings.is_empty());
}

#[tokio::test]
async fn agent_loop_runtime_input_entry_matches_existing_run_entry_for_final_only_task() {
    let input = RuntimeInput::from_agent_task(
        test_task("Answer briefly without using tools."),
        LifeModel::default(),
        None,
        "Available tools: memory.search",
        None,
        AgentExecutionBudget::default(),
    );
    let loop_config = input.agent_loop_config();
    let legacy_loop = test_agent_loop(loop_config.clone());
    let runtime_input_loop = test_agent_loop(loop_config);
    let (registry, permission_store, audit_store, privacy_engine, _audit_file) =
        test_action_context_deps();
    let safe_paths: Vec<String> = Vec::new();
    let action_ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &safe_paths,
    );

    let legacy = legacy_loop
        .run(
            &input.task,
            &input.life_model_compat,
            &input.tools_prompt,
            input.memory_context.clone(),
            privacy_engine.clone(),
            &action_ctx,
        )
        .await
        .unwrap();
    let converged = runtime_input_loop
        .run_runtime_input(input, privacy_engine.clone(), &action_ctx)
        .await
        .unwrap();

    assert_eq!(legacy.stop_reason, "no_tools");
    assert_eq!(legacy.final_response, converged.user_output);
    assert_eq!(legacy.run.actions.len(), converged.actions.len());
    assert_eq!(legacy.run.generated_proposals, converged.proposal_ids);
    assert_eq!(legacy.run.warnings, converged.warnings);
}

#[tokio::test]
async fn runtime_input_with_broad_tools_prompt_does_not_infer_external_write_intent() {
    let broad_tools_prompt = r#"
        Available tools:
        memory.search(query)
        email.propose_draft(to, subject, body)
        calendar.propose_event(title, scheduled_at)
        file.write_proposal(path, content)
        mcp.external_write(target, payload)
    "#;
    let input = RuntimeInput::from_agent_task(
        test_task("What can you infer from my recent notes? Do not write or schedule anything."),
        LifeModel::default(),
        None,
        broad_tools_prompt,
        None,
        AgentExecutionBudget::default(),
    );
    let loop_instance = test_agent_loop(input.agent_loop_config());
    let (registry, permission_store, audit_store, privacy_engine, _audit_file) =
        test_action_context_deps();
    let safe_paths: Vec<String> = Vec::new();
    let action_ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &safe_paths,
    );

    let output = loop_instance
        .run_runtime_input(input, privacy_engine.clone(), &action_ctx)
        .await
        .unwrap();

    assert!(!output.user_output.trim().is_empty());
    assert!(output.actions.is_empty());
    assert!(output.observations.is_empty());
    assert!(output.proposal_ids.is_empty());
    assert!(output.warnings.is_empty());
}

#[tokio::test]
async fn runtime_input_execution_budget_controls_agent_loop_run() {
    let input = RuntimeInput::from_agent_task(
        test_task("This should stop before model generation."),
        LifeModel::default(),
        None,
        "Available tools: memory.search",
        None,
        AgentExecutionBudget {
            max_steps: 0,
            max_tool_calls: 0,
            timeout_seconds: 60,
            allow_cloud: true,
            allow_writes: false,
        },
    );
    let loop_instance = test_agent_loop(AgentLoopConfig::default());
    let (registry, permission_store, audit_store, privacy_engine, _audit_file) =
        test_action_context_deps();
    let safe_paths: Vec<String> = Vec::new();
    let action_ctx = ActionExecutionContext::new(
        &registry,
        &permission_store,
        &audit_store,
        &privacy_engine,
        &safe_paths,
    );

    let output = loop_instance
        .run_runtime_input(input, privacy_engine.clone(), &action_ctx)
        .await
        .unwrap();

    assert!(output.user_output.contains("最大执行步数 (0)"));
    assert!(output.actions.is_empty());
    assert!(output.observations.is_empty());
    assert!(output.proposal_ids.is_empty());
}
