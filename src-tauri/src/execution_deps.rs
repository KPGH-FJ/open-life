use openlife_core::agent::action_executor::ActionContext;
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

/// Assemble an owned `ActionContext` from `Arc` clones of global stores.
/// No locks are held across this call — all handles are cloned, and the
/// resulting context is safe to pass across `.await` points.
#[allow(clippy::too_many_arguments)]
pub fn assemble_action_context(
    mcp_registry: Arc<tokio::sync::Mutex<openlife_core::mcp::McpRegistry>>,
    permission_store: Arc<tokio::sync::Mutex<openlife_core::tool_permissions::ToolPermissionStore>>,
    audit_store: Arc<tokio::sync::Mutex<openlife_core::mcp_audit::McpAuditStore>>,
    privacy_engine: Arc<tokio::sync::Mutex<PrivacyEngine>>,
    safe_paths: Vec<String>,
    life_model: Option<LifeModel>,
    memory_store: Option<Arc<tokio::sync::Mutex<openlife_core::memory::MemoryStore>>>,
    calendar_ics_paths: Vec<String>,
    network_policy: openlife_core::config::NetworkPolicy,
    execution_sandbox: ExecutionSandbox,
    agent_spec: openlife_core::agent::types::AgentSpec,
    proposal_store: Option<Arc<tokio::sync::Mutex<openlife_core::agent::ProposalStore>>>,
    agent_run_store: Option<Arc<tokio::sync::Mutex<openlife_core::agent::AgentRunStore>>>,
    event_store: Option<openlife_core::agent::event_store::AgentRunEventStore>,
) -> ActionContext {
    ActionContext {
        registry: mcp_registry,
        permission_store,
        audit_store,
        privacy_engine,
        safe_paths,
        life_model,
        memory_store,
        proposal_store,
        agent_run_store,
        event_store,
        network_policy: Some(network_policy),
        calendar_ics_paths,
        execution_sandbox,
        agent_spec: Some(agent_spec),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::config::AppConfig;
    use openlife_core::layer_router::Layer;

    #[test]
    fn test_build_agent_task_fields() {
        let msgs = vec![openlife_core::llm::ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }];
        let task = build_agent_task(
            openlife_core::agent::AgentTaskKind::Conversation,
            "sess-1".into(),
            "hello".into(),
            msgs.clone(),
            Layer::L2,
        );
        assert_eq!(task.session_id, "sess-1");
        assert_eq!(task.user_text, "hello");
        assert_eq!(task.messages.len(), 1);
        assert_eq!(task.messages[0].content, "hello");
        assert_eq!(task.kind, openlife_core::agent::AgentTaskKind::Conversation);
        assert_eq!(task.layer, Layer::L2);
    }

    #[test]
    fn test_build_loop_config_reads_from_app_config() {
        let cfg = AppConfig::default();
        let shutdown = Arc::new(tokio::sync::Notify::new());
        let config = build_loop_config(&cfg, shutdown);
        assert!(config.max_steps > 0);
        assert!(config.max_tool_calls > 0);
        assert!(config.timeout_seconds > 0);
        assert!(config.allow_writes);
        assert!(config.allow_cloud);
        assert!(config.shutdown_notify.is_some());
    }

    #[test]
    fn test_resolve_execution_env_populates_all_fields() {
        let cfg = AppConfig::default();
        let env = resolve_execution_env(&cfg);
        assert!(!env.agent_spec.id.is_empty());
        assert!(env.prompt_registry.get("base_system").is_some());
    }
}
