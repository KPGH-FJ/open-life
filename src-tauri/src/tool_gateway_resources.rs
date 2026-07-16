//! Owned ToolGateway resource snapshots for Tauri product execution paths.
//!
//! `AppState` mutexes protect replaceable handles; they must never become part
//! of provider, tool, or network I/O lifetimes. This module is the sole lock
//! acquisition authority for product `ActionExecutionContext` construction.
//! Each public function returns a purpose-specific type whose required stores
//! are non-optional, and every outer guard is dropped before the next lock.
//!
//! Product source map and lock-order contract:
//! 1. `config` (copy only non-secret typed fields),
//! 2. `tool_permission_store`,
//! 3. `mcp_registry`,
//! 4. canonical `mcp_audit_store` writer handle (Arc clone; no store guard),
//! 5. `privacy_engine`,
//! 6. `memory_store` for governed product execution,
//! 7. `memory_lifecycle_store` retrieval authority where available,
//! 8. required `proposal_store` or `agent_run_store` only where the
//!    purpose-specific return type declares them.
//!
//! The consumers are the dev ToolGateway command, Main Chat read execution,
//! Main Chat replay/AgentLoop execution and scheduled execution. MCP's
//! `McpSession` mutex is intentionally outside this table: it
//! is a single-owner transport/session lock on an owned registry snapshot, not
//! an `AppState` registry guard, and cancellation poisons/kills that transport.

use std::sync::Arc;

use crate::AppState;

/// Product audit writer resolver. Cloning this handle copies only an `Arc`;
/// the active keyring and authority generation remain owned by the canonical
/// `AppState` store. The outer Tokio mutex is acquired only by the bounded
/// blocking audit commit, never across provider/tool/network awaits.
#[derive(Clone)]
pub(crate) struct CanonicalMcpAuditWriter {
    store: Arc<tokio::sync::Mutex<openlife_core::mcp_audit::McpAuditStore>>,
    persistence_coordinator: Arc<crate::persistence_coordinator::PersistenceCoordinator>,
}

impl CanonicalMcpAuditWriter {
    fn new(
        store: Arc<tokio::sync::Mutex<openlife_core::mcp_audit::McpAuditStore>>,
        persistence_coordinator: Arc<crate::persistence_coordinator::PersistenceCoordinator>,
    ) -> Self {
        Self {
            store,
            persistence_coordinator,
        }
    }

    #[cfg(test)]
    pub(crate) async fn list_logs(
        &self,
        limit: usize,
    ) -> anyhow::Result<Vec<openlife_core::mcp_audit::McpLogEntry>> {
        self.store.lock().await.list_logs(limit)
    }
}

impl openlife_core::mcp_audit::McpAuditDurableWriter for CanonicalMcpAuditWriter {
    fn clone_owned_writer(&self) -> Arc<dyn openlife_core::mcp_audit::McpAuditDurableWriter> {
        Arc::new(self.clone())
    }

    fn insert_log_durably(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
        result: &str,
        success: bool,
        pii_found: bool,
    ) -> anyhow::Result<i64> {
        self.store
            .blocking_lock()
            .insert_log(tool_name, arguments, result, success, pii_found)
    }

    fn report_runtime_failure(&self, reason_code: &'static str, detail: &str) {
        // Reporters can run in a Tokio worker (closed gate) or a cancelled
        // future's Drop path before spawn_blocking starts. They therefore
        // cannot use `blocking_lock`. The coordinator is the shipped,
        // process-wide observer authority and its registration is synchronous,
        // infallible, and monotonic after seal.
        self.persistence_coordinator
            .register_unavailable("McpAuditStore", reason_code, detail);
    }
}

#[derive(Clone)]
pub(crate) struct SharedToolGatewayResources {
    pub(crate) safe_paths: Vec<String>,
    pub(crate) permission_store: openlife_core::tool_permissions::ToolPermissionStore,
    pub(crate) registry: openlife_core::mcp::McpRegistry,
    pub(crate) audit_store: CanonicalMcpAuditWriter,
    pub(crate) privacy_engine: openlife_core::privacy::PrivacyEngine,
}

#[derive(Clone)]
pub(crate) struct GovernedToolGatewayResources {
    pub(crate) shared: SharedToolGatewayResources,
    pub(crate) calendar_ics_paths: Vec<String>,
    pub(crate) network_policy: openlife_core::config::NetworkPolicy,
    pub(crate) memory_store: openlife_core::memory::MemoryStore,
    pub(crate) memory_lifecycle_retrieval_reader:
        Option<openlife_core::agent::MemoryLifecycleRetrievalReader>,
}

#[cfg(any(test, feature = "dev-extensions"))]
pub(crate) struct DevToolGatewayResources {
    pub(crate) shared: SharedToolGatewayResources,
    pub(crate) agent_run_store: openlife_core::agent::AgentRunStore,
}

pub(crate) struct MainChatReadToolGatewayResources {
    pub(crate) governed: GovernedToolGatewayResources,
    pub(crate) agent_run_store: openlife_core::agent::AgentRunStore,
}

pub(crate) struct MainChatExecutionToolGatewayResources {
    pub(crate) governed: GovernedToolGatewayResources,
    pub(crate) agent_run_store: openlife_core::agent::AgentRunStore,
}

#[derive(Clone)]
pub(crate) struct MainChatAgentLoopLimitSnapshot {
    pub(crate) max_steps: u32,
    pub(crate) max_tool_calls: u32,
    pub(crate) timeout_seconds: u64,
}

pub(crate) struct MainChatAgentLoopToolGatewayResources {
    pub(crate) execution: MainChatExecutionToolGatewayResources,
    pub(crate) agent_runtime_config: openlife_core::agent::AgentRuntimeConfig,
    pub(crate) limits: MainChatAgentLoopLimitSnapshot,
}

pub(crate) struct ScheduledToolGatewayResources {
    pub(crate) governed: GovernedToolGatewayResources,
    pub(crate) proposal_store: openlife_core::agent::ProposalStore,
    pub(crate) agent_run_store: openlife_core::agent::AgentRunStore,
    pub(crate) agent_runtime_config: openlife_core::agent::AgentRuntimeConfig,
}

struct CapturedNonSecretConfig {
    safe_paths: Vec<String>,
    calendar_ics_paths: Vec<String>,
    network_policy: openlife_core::config::NetworkPolicy,
    agent_runtime_config: openlife_core::agent::AgentRuntimeConfig,
    main_chat_agent_loop_limits: MainChatAgentLoopLimitSnapshot,
}

async fn capture_non_secret_config(state: &Arc<AppState>) -> CapturedNonSecretConfig {
    let config = state.config.lock().await;
    CapturedNonSecretConfig {
        safe_paths: config.system.safe_paths.clone(),
        calendar_ics_paths: config.system.calendar_ics_paths.clone(),
        network_policy: config.system.network_policy.clone(),
        agent_runtime_config: openlife_core::agent::AgentRuntimeConfig {
            default_strategy: config.reasoning.default_strategy.clone(),
            meaning_timeout_ms: config.reasoning.meaning_timeout_ms,
            strategy_timeout_ms: config.reasoning.strategy_timeout_ms,
            generation_timeout_ms: config.reasoning.generation_timeout_ms,
        },
        main_chat_agent_loop_limits: MainChatAgentLoopLimitSnapshot {
            max_steps: config.system.agent_loop_max_steps,
            max_tool_calls: config.system.agent_loop_max_tool_calls,
            timeout_seconds: config.system.agent_loop_timeout_seconds,
        },
    }
}

async fn capture_shared_after_config(
    state: &Arc<AppState>,
    safe_paths: Vec<String>,
) -> SharedToolGatewayResources {
    let permission_store = { state.tool_permission_store.lock().await.clone() };
    let registry = { state.mcp_registry.lock().await.clone() };
    let audit_store = CanonicalMcpAuditWriter::new(
        Arc::clone(&state.mcp_audit_store),
        Arc::clone(&state.persistence_coordinator),
    );
    let privacy_engine = { state.privacy_engine.lock().await.clone() };
    SharedToolGatewayResources {
        safe_paths,
        permission_store,
        registry,
        audit_store,
        privacy_engine,
    }
}

async fn capture_governed(
    state: &Arc<AppState>,
) -> (GovernedToolGatewayResources, CapturedNonSecretConfig) {
    let config = capture_non_secret_config(state).await;
    let shared = capture_shared_after_config(state, config.safe_paths.clone()).await;
    let memory_store = { state.memory_store.lock().await.clone() };
    let memory_lifecycle_retrieval_reader =
        if let Some(store) = state.memory_lifecycle_store.as_ref() {
            Some(store.lock().await.retrieval_reader())
        } else {
            None
        };
    (
        GovernedToolGatewayResources {
            shared,
            calendar_ics_paths: config.calendar_ics_paths.clone(),
            network_policy: config.network_policy.clone(),
            memory_store,
            memory_lifecycle_retrieval_reader,
        },
        config,
    )
}

async fn require_proposal_store(
    state: &Arc<AppState>,
) -> Result<openlife_core::agent::ProposalStore, String> {
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "tool_gateway_proposal_store_unavailable".to_string())?;
    Ok(store.lock().await.clone())
}

async fn require_agent_run_store(
    state: &Arc<AppState>,
) -> Result<openlife_core::agent::AgentRunStore, String> {
    let store = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "tool_gateway_agent_run_store_unavailable".to_string())?;
    Ok(store.lock().await.clone())
}

#[cfg(any(test, feature = "dev-extensions"))]
pub(crate) async fn snapshot_tool_gateway_resources_for_dev_command(
    state: &Arc<AppState>,
) -> Result<DevToolGatewayResources, String> {
    let safe_paths = { state.config.lock().await.system.safe_paths.clone() };
    let shared = capture_shared_after_config(state, safe_paths).await;
    let agent_run_store = require_agent_run_store(state).await?;
    Ok(DevToolGatewayResources {
        shared,
        agent_run_store,
    })
}

pub(crate) async fn snapshot_tool_gateway_resources_for_main_chat_read(
    state: &Arc<AppState>,
) -> Result<MainChatReadToolGatewayResources, String> {
    let (governed, _) = capture_governed(state).await;
    let agent_run_store = require_agent_run_store(state).await?;
    Ok(MainChatReadToolGatewayResources {
        governed,
        agent_run_store,
    })
}

pub(crate) async fn snapshot_tool_gateway_resources_for_main_chat_execution(
    state: &Arc<AppState>,
) -> Result<MainChatExecutionToolGatewayResources, String> {
    let (governed, _) = capture_governed(state).await;
    let agent_run_store = require_agent_run_store(state).await?;
    Ok(MainChatExecutionToolGatewayResources {
        governed,
        agent_run_store,
    })
}

pub(crate) async fn snapshot_tool_gateway_resources_for_main_chat_agent_loop(
    state: &Arc<AppState>,
) -> Result<MainChatAgentLoopToolGatewayResources, String> {
    let (governed, config) = capture_governed(state).await;
    let agent_run_store = require_agent_run_store(state).await?;
    Ok(MainChatAgentLoopToolGatewayResources {
        execution: MainChatExecutionToolGatewayResources {
            governed,
            agent_run_store,
        },
        agent_runtime_config: config.agent_runtime_config,
        limits: config.main_chat_agent_loop_limits,
    })
}

pub(crate) async fn snapshot_tool_gateway_resources_for_scheduler(
    state: &Arc<AppState>,
) -> Result<ScheduledToolGatewayResources, String> {
    let (governed, config) = capture_governed(state).await;
    let proposal_store = require_proposal_store(state).await?;
    let agent_run_store = require_agent_run_store(state).await?;
    Ok(ScheduledToolGatewayResources {
        governed,
        proposal_store,
        agent_run_store,
        agent_runtime_config: config.agent_runtime_config,
    })
}
