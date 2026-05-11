use openlife_core::agent::action_executor::ActionExecutionContext;
use openlife_core::agent::execution_sandbox::ExecutionSandbox;
use openlife_core::config::AppConfig;
use openlife_core::life_model::LifeModel;
use openlife_core::privacy::PrivacyEngine;
use openlife_core::scheduler::InferenceScheduler;
use std::sync::Arc;

pub struct ExecutionEnvironment {
    pub agent_spec: openlife_core::agent::types::AgentSpec,
    pub prompt_registry: openlife_core::agent::prompt_stack::PromptBlockRegistry,
    pub execution_sandbox: ExecutionSandbox,
    pub network_policy: openlife_core::config::NetworkPolicy,
}

pub fn build_loop_config(
    cfg: &AppConfig,
    shutdown_notify: Arc<tokio::sync::Notify>,
) -> openlife_core::agent::AgentLoopConfig {
    openlife_core::agent::AgentLoopConfig {
        max_steps: cfg.system.agent_loop_max_steps,
        max_tool_calls: cfg.system.agent_loop_max_tool_calls,
        timeout_seconds: cfg.system.agent_loop_timeout_seconds,
        allow_writes: true,
        allow_cloud: true,
        shutdown_notify: Some(shutdown_notify),
        ..Default::default()
    }
}

pub fn build_agent_loop(
    runtime: openlife_core::agent::AgentRuntime,
    action_executor: openlife_core::agent::ActionExecutor,
    scheduler: &InferenceScheduler,
    loop_config: openlife_core::agent::AgentLoopConfig,
    event_store: &Option<Arc<openlife_core::agent::event_store::AgentRunEventStore>>,
) -> openlife_core::agent::AgentLoop {
    let mut al = openlife_core::agent::AgentLoop::new(
        runtime,
        action_executor,
        scheduler.clone(),
        loop_config,
    );
    if let Some(ref es) = event_store {
        al = al.with_event_store((**es).clone());
    }
    al
}

pub fn resolve_execution_env(cfg: &AppConfig) -> ExecutionEnvironment {
    ExecutionEnvironment {
        agent_spec: openlife_core::agent::types::AgentSpec::default_main_spec(),
        prompt_registry: openlife_core::agent::prompt_stack::PromptBlockRegistry::built_in(),
        execution_sandbox: ExecutionSandbox::from_config(
            &cfg.system.execution_sandbox,
            &cfg.system.safe_paths,
        ),
        network_policy: cfg.system.network_policy.clone(),
    }
}

pub fn build_agent_task(
    kind: openlife_core::agent::AgentTaskKind,
    session_id: String,
    user_text: String,
    messages: Vec<openlife_core::llm::ChatMessage>,
    layer: openlife_core::layer_router::Layer,
) -> openlife_core::agent::AgentTask {
    openlife_core::agent::AgentTask {
        kind,
        session_id,
        user_text,
        messages,
        layer,
        ..Default::default()
    }
}

/// Assemble ActionExecutionContext from pre-acquired locks.
/// Caller must hold all necessary lock guards alive.
#[allow(clippy::too_many_arguments)]
pub fn assemble_action_ctx<'a>(
    reg: &'a openlife_core::mcp::McpRegistry,
    permission_store: &'a openlife_core::tool_permissions::ToolPermissionStore,
    audit: &'a openlife_core::mcp_audit::McpAuditStore,
    privacy_engine: &'a PrivacyEngine,
    safe_paths: &'a [String],
    life_model: &'a LifeModel,
    memory_store: &'a openlife_core::memory::MemoryStore,
    calendar_ics_paths: &'a [String],
    network_policy: &'a openlife_core::config::NetworkPolicy,
    execution_sandbox: &'a ExecutionSandbox,
    agent_spec: &'a openlife_core::agent::types::AgentSpec,
    proposal_store: Option<&'a openlife_core::agent::ProposalStore>,
    agent_run_store: Option<&'a openlife_core::agent::AgentRunStore>,
    event_store: Option<openlife_core::agent::event_store::AgentRunEventStore>,
) -> ActionExecutionContext<'a> {
    let mut ctx =
        ActionExecutionContext::new(reg, permission_store, audit, privacy_engine, safe_paths)
            .with_life_model(life_model)
            .with_memory_store(memory_store)
            .with_calendar_ics_paths(calendar_ics_paths)
            .with_network_policy(network_policy)
            .with_execution_sandbox(execution_sandbox)
            .with_agent_spec(agent_spec);

    if let Some(store) = proposal_store {
        ctx = ctx.with_proposal_store(store);
    }
    if let Some(store) = agent_run_store {
        ctx = ctx.with_agent_run_store(store);
    }
    if let Some(es) = event_store {
        ctx = ctx.with_event_store(es);
    }
    ctx
}
