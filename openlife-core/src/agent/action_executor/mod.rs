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

/// Dependencies required for action execution.
///
/// Essential fields (registry, permission_store, audit_store, privacy_engine,
/// safe_paths) are set via the constructor. Optional fields are set via builder
/// methods.
pub struct ActionExecutionContext<'a> {
    pub registry: &'a McpRegistry,
    pub permission_store: &'a ToolPermissionStore,
    pub audit_store: &'a McpAuditStore,
    pub privacy_engine: &'a PrivacyEngine,
    pub safe_paths: &'a [String],
    pub life_model: Option<&'a crate::life_model::LifeModel>,
    pub memory_store: Option<&'a crate::memory::MemoryStore>,
    pub proposal_store: Option<&'a crate::agent::ProposalStore>,
    pub agent_run_store: Option<&'a crate::agent::AgentRunStore>,
    pub network_policy: Option<&'a crate::config::NetworkPolicy>,
    pub hs_runtime_packet: Option<&'a crate::agent::RuntimeHSPacket>,
    /// ICS calendar file paths for calendar.read tool
    pub calendar_ics_paths: &'a [String],
}

impl<'a> ActionExecutionContext<'a> {
    /// Create a context with the essential dependencies.
    /// Optional fields default to None / empty.
    pub fn new(
        registry: &'a McpRegistry,
        permission_store: &'a ToolPermissionStore,
        audit_store: &'a McpAuditStore,
        privacy_engine: &'a PrivacyEngine,
        safe_paths: &'a [String],
    ) -> Self {
        Self {
            registry,
            permission_store,
            audit_store,
            privacy_engine,
            safe_paths,
            life_model: None,
            memory_store: None,
            proposal_store: None,
            agent_run_store: None,
            network_policy: None,
            hs_runtime_packet: None,
            calendar_ics_paths: &[],
        }
    }

    pub fn with_life_model(mut self, life_model: &'a crate::life_model::LifeModel) -> Self {
        self.life_model = Some(life_model);
        self
    }

    pub fn with_memory_store(mut self, memory_store: &'a crate::memory::MemoryStore) -> Self {
        self.memory_store = Some(memory_store);
        self
    }

    pub fn with_proposal_store(mut self, proposal_store: &'a crate::agent::ProposalStore) -> Self {
        self.proposal_store = Some(proposal_store);
        self
    }

    pub fn with_agent_run_store(
        mut self,
        agent_run_store: &'a crate::agent::AgentRunStore,
    ) -> Self {
        self.agent_run_store = Some(agent_run_store);
        self
    }

    pub fn with_network_policy(mut self, network_policy: &'a crate::config::NetworkPolicy) -> Self {
        self.network_policy = Some(network_policy);
        self
    }

    pub fn with_calendar_ics_paths(mut self, paths: &'a [String]) -> Self {
        self.calendar_ics_paths = paths;
        self
    }

    pub fn with_hs_runtime_packet(mut self, packet: &'a crate::agent::RuntimeHSPacket) -> Self {
        self.hs_runtime_packet = Some(packet);
        self
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
    pub fn execute(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
    ) -> Result<ActionExecutionResult> {
        match request.action_type.as_str() {
            "mcp_tool" | "builtin_tool" | "plugin_tool" => self.execute_tool(request, ctx),
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
