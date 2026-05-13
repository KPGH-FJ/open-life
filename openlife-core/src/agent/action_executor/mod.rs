pub mod core_os_tools;
pub mod declarative_stubs;
pub mod execution_tools;
pub mod helpers;
pub mod tool_executor;

// Re-export commonly used helpers
pub use helpers::{filesystem_access_error, is_path_in_safe_paths};

use crate::agent::types::{AgentAction, AgentObservation};
use crate::mcp::McpRegistry;
use crate::mcp_audit::McpAuditStore;
use crate::privacy::PrivacyEngine;
use crate::tool_permissions::ToolPermissionStore;
use anyhow::Result;
use serde_json::Value;
use std::sync::{Arc, LazyLock};

/// Always-disabled sandbox reference for paths that have not wired an
/// explicit sandbox policy. All fields are maximum-safety defaults.
pub static DISABLED_SANDBOX: LazyLock<crate::agent::execution_sandbox::ExecutionSandbox> =
    LazyLock::new(crate::agent::execution_sandbox::ExecutionSandbox::default);

/// Configuration for action execution.
#[derive(Debug, Clone)]
pub struct ActionExecutorConfig {
    pub allow_writes: bool,
    pub allow_cloud: bool,
    pub timeout_seconds: u64,
    /// Whether to consume `allow_once` policies during permission check.
    /// Default is `true`. Set to `false` for replay paths to avoid
    /// consuming one-time permissions.
    pub consume_allow_once: bool,
}

impl Default for ActionExecutorConfig {
    fn default() -> Self {
        Self {
            allow_writes: true,
            allow_cloud: true,
            timeout_seconds: 120,
            consume_allow_once: true,
        }
    }
}

/// Request to execute a single action.
#[derive(Debug, Clone)]
pub struct AgentActionRequest {
    pub action_type: String,
    pub target: String,
    pub input: Value,
    pub source_run_id: Option<String>,
    pub step_index: u32,
}

/// Result of executing an action.
#[derive(Debug, Clone)]
pub struct ActionExecutionResult {
    pub action: AgentAction,
    pub observation: AgentObservation,
    pub status: ActionExecutionStatus,
    pub stop_reason: Option<String>,
}

/// Status of action execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionExecutionStatus {
    Succeeded,
    Failed,
    Blocked,
    NeedsConfirmation,
}

/// Owned, cloneable action execution context. All store access goes through
/// `Arc<tokio::sync::Mutex<T>>` handles so that the context can be passed
/// across `.await` points without holding global-store `MutexGuard`s.
///
/// Constructed by cloning handles from `AppState`, then releasing all locks
/// before entering long-running async operations.
#[derive(Clone)]
pub struct ActionContext {
    pub registry: Arc<tokio::sync::Mutex<McpRegistry>>,
    pub permission_store: Arc<tokio::sync::Mutex<ToolPermissionStore>>,
    pub audit_store: Arc<tokio::sync::Mutex<McpAuditStore>>,
    pub privacy_engine: Arc<tokio::sync::Mutex<PrivacyEngine>>,
    pub safe_paths: Vec<String>,
    pub life_model: Option<crate::life_model::LifeModel>,
    pub memory_store: Option<Arc<tokio::sync::Mutex<crate::memory::MemoryStore>>>,
    pub proposal_store: Option<Arc<tokio::sync::Mutex<crate::agent::ProposalStore>>>,
    pub agent_run_store: Option<Arc<tokio::sync::Mutex<crate::agent::AgentRunStore>>>,
    pub event_store: Option<crate::agent::event_store::AgentRunEventStore>,
    pub network_policy: Option<crate::config::NetworkPolicy>,
    pub calendar_ics_paths: Vec<String>,
    pub execution_sandbox: crate::agent::execution_sandbox::ExecutionSandbox,
    pub agent_spec: Option<crate::agent::types::AgentSpec>,
}

impl ActionContext {
    /// Create a minimal context for testing, wrapping stores in temporary Arcs.
    pub fn new_for_test(
        registry: McpRegistry,
        permission_store: ToolPermissionStore,
        audit_store: McpAuditStore,
        privacy_engine: PrivacyEngine,
        safe_paths: Vec<String>,
    ) -> Self {
        Self {
            registry: Arc::new(tokio::sync::Mutex::new(registry)),
            permission_store: Arc::new(tokio::sync::Mutex::new(permission_store)),
            audit_store: Arc::new(tokio::sync::Mutex::new(audit_store)),
            privacy_engine: Arc::new(tokio::sync::Mutex::new(privacy_engine)),
            safe_paths,
            life_model: None,
            memory_store: None,
            proposal_store: None,
            agent_run_store: None,
            event_store: None,
            network_policy: None,
            calendar_ics_paths: Vec::new(),
            execution_sandbox: crate::agent::execution_sandbox::ExecutionSandbox::default(),
            agent_spec: None,
        }
    }

    /// Builder: set execution sandbox
    pub fn with_execution_sandbox(
        mut self,
        sandbox: crate::agent::execution_sandbox::ExecutionSandbox,
    ) -> Self {
        self.execution_sandbox = sandbox;
        self
    }

    /// Builder: set agent spec
    pub fn with_agent_spec(mut self, spec: crate::agent::types::AgentSpec) -> Self {
        self.agent_spec = Some(spec);
        self
    }

    /// Builder: set proposal store
    pub fn with_proposal_store(
        mut self,
        store: Arc<tokio::sync::Mutex<crate::agent::ProposalStore>>,
    ) -> Self {
        self.proposal_store = Some(store);
        self
    }
}

/// Compatibility wrapper for tests that need sync execute access.
#[cfg(test)]
impl ActionExecutor {
    pub fn execute_test(
        &self,
        request: AgentActionRequest,
        ctx: &ActionContext,
    ) -> Result<ActionExecutionResult> {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(self.execute(request, ctx))
    }
}

/// Borrowed, temporary context used internally during a single synchronous
/// tool execution. Created by locking Arc handles from `ActionContext`.
pub struct BorrowedActionContext<'a> {
    registry: &'a McpRegistry,
    permission_store: &'a ToolPermissionStore,
    audit_store: &'a McpAuditStore,
    #[allow(dead_code)]
    privacy_engine: &'a PrivacyEngine,
    safe_paths: &'a [String],
    life_model: Option<&'a crate::life_model::LifeModel>,
    memory_store: Option<&'a crate::memory::MemoryStore>,
    proposal_store: Option<&'a crate::agent::ProposalStore>,
    agent_run_store: Option<&'a crate::agent::AgentRunStore>,
    event_store: Option<crate::agent::event_store::AgentRunEventStore>,
    network_policy: Option<&'a crate::config::NetworkPolicy>,
    calendar_ics_paths: &'a [String],
    execution_sandbox: &'a crate::agent::execution_sandbox::ExecutionSandbox,
    agent_spec: Option<&'a crate::agent::types::AgentSpec>,
}
fn is_write_action(request: &AgentActionRequest) -> bool {
    let t = request.action_type.as_str();
    if t == "mcp_tool" || t == "builtin_tool" || t == "plugin_tool" {
        let tool_name = request
            .input
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&request.target);
        tool_name.starts_with("file.write")
            || tool_name.starts_with("file.write_proposal")
            || tool_name.starts_with("memory.propose_write")
            || tool_name.starts_with("memory.propose_archive")
            || tool_name.starts_with("life_model.propose_patch")
            || tool_name.starts_with("goal.propose_update")
            || tool_name.starts_with("permission.grant")
            || tool_name.starts_with("permission.request")
            || tool_name.starts_with("shell.run")
            || tool_name.starts_with("email.propose_draft")
            || tool_name.starts_with("calendar.propose_event")
            || tool_name.starts_with("task.create_proposal")
    } else {
        t == "memory_write" || t == "memory_archive" || t == "life_model_patch"
    }
}

/// Centralized action executor for all agent actions.
///
/// This is the single entry point for executing tools, memory operations,
/// and life model patches. It handles permission checks, PII inspection,
/// audit logging, and building the action/observation pair.
pub struct ActionExecutor {
    config: ActionExecutorConfig,
}

impl ActionExecutor {
    pub fn new(config: ActionExecutorConfig) -> Self {
        Self { config }
    }

    /// Execute a single action request.
    pub async fn execute(
        &self,
        request: AgentActionRequest,
        ctx: &ActionContext,
    ) -> Result<ActionExecutionResult> {
        // Hard enforcement: if writes are disallowed, block all write/side-effect actions
        if !self.config.allow_writes && is_write_action(&request) {
            return Ok(ActionExecutionResult {
                action: AgentAction {
                    id: uuid::Uuid::new_v4().to_string(),
                    action_type: request.action_type.clone(),
                    target: Some(request.target.clone()),
                    input: request.input.clone(),
                    output: None,
                    status: "blocked".to_string(),
                    permission_decision: Some("blocked".to_string()),
                    started_at: None,
                    finished_at: None,
                    error: Some("Write actions are disabled (allow_writes=false)".to_string()),
                    timestamp: chrono::Utc::now(),
                    tool_scope: None,
                },
                observation: AgentObservation {
                    id: uuid::Uuid::new_v4().to_string(),
                    action_id: None,
                    content: "Write actions disabled by policy".to_string(),
                    source: "system".to_string(),
                    structured_result: None,
                    timestamp: chrono::Utc::now(),
                },
                status: ActionExecutionStatus::Blocked,
                stop_reason: Some("allow_writes_disabled".to_string()),
            });
        }
        match request.action_type.as_str() {
            "mcp_tool" | "builtin_tool" | "plugin_tool" => self.execute_tool(request, ctx).await,
            "memory_write" => self.execute_memory_write(request),
            "memory_archive" => self.execute_memory_archive(request),
            "life_model_patch" => self.execute_life_model_patch(request),
            _ => Err(anyhow::anyhow!(
                "unsupported action type: {}",
                request.action_type
            )),
        }
    }

    pub fn execute_life_model_patch(
        &self,
        request: AgentActionRequest,
    ) -> Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "life_model_patch must be submitted as a LifeModel proposal before persistence",
        ))
    }

    pub fn execute_memory_write(
        &self,
        request: AgentActionRequest,
    ) -> Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "memory_write must be submitted as a MemoryWrite proposal before persistence",
        ))
    }

    pub fn execute_memory_archive(
        &self,
        request: AgentActionRequest,
    ) -> Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "memory_archive must be submitted as a MemoryArchive proposal before persistence",
        ))
    }
}
