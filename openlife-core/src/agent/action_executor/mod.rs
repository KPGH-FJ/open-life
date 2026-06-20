pub mod core_os_tools;
pub mod declarative_stubs;
pub mod execution_tools;
pub mod helpers;
pub mod tool_executor;

// Re-export commonly used helpers
pub use helpers::{filesystem_access_error, is_path_in_safe_paths};

use crate::agent::types::{AgentAction, AgentObservation};
use crate::agent::GovernorDecisionReport;
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
    pub governance_report: Option<GovernorDecisionReport>,
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
    pub web_search_fixture_output: Option<&'a str>,
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
            web_search_fixture_output: None,
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

    pub fn with_web_search_fixture_output(mut self, output: &'a str) -> Self {
        self.web_search_fixture_output = Some(output);
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

fn metadata_safe_preview(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
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
            "memory_search" | "session_search" => self.execute_memory_search(request, ctx),
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

    pub fn execute_memory_search(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
    ) -> Result<ActionExecutionResult> {
        let now = chrono::Utc::now();
        let action_id = format!(
            "action-memory-search-{}",
            now.timestamp_nanos_opt().unwrap_or_default()
        );
        let observation_id = format!(
            "observation-memory-search-{}",
            now.timestamp_nanos_opt().unwrap_or_default()
        );
        let Some(memory_store) = ctx.memory_store else {
            let action = AgentAction {
                id: action_id.clone(),
                action_type: request.action_type,
                target: Some(request.target),
                input: request.input,
                output: None,
                status: "failed".into(),
                error: Some("memory store unavailable for read-only search".into()),
                permission_decision: None,
                started_at: Some(now),
                finished_at: Some(now),
                timestamp: now,
                tool_scope: None,
                react_trace: None,
            };
            let observation = AgentObservation {
                id: observation_id,
                action_id: Some(action_id),
                content: "Memory search failed: memory store unavailable.".into(),
                source: "memory_search".into(),
                structured_result: Some(serde_json::json!({
                    "success": false,
                    "status": "failed",
                    "hitCount": 0,
                    "directWritesExecuted": false,
                })),
                timestamp: now,
                react_trace: None,
            };
            return Ok(ActionExecutionResult {
                action,
                observation,
                status: ActionExecutionStatus::Failed,
                stop_reason: Some("memory_store_unavailable".into()),
                governance_report: None,
            });
        };

        let query = request
            .input
            .get("query")
            .or_else(|| request.input.get("q"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let session_id = request.input.get("session_id").and_then(Value::as_str);
        let limit = request
            .input
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(5)
            .clamp(1, 10) as usize;
        let hits = memory_store.search_text_memories(session_id, query, limit)?;
        let hit_previews = hits
            .iter()
            .map(|hit| {
                serde_json::json!({
                    "sessionId": hit.chunk.session_id,
                    "source": hit.chunk.source,
                    "score": hit.relevance_score,
                    "preview": metadata_safe_preview(&hit.chunk.content, 160),
                    "createdAt": hit.chunk.created_at,
                })
            })
            .collect::<Vec<_>>();
        let content = if hits.is_empty() {
            format!("No memory/session hits found for query '{}'.", query)
        } else {
            let joined = hits
                .iter()
                .map(|hit| metadata_safe_preview(&hit.chunk.content, 180))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "Found {} memory/session hit(s) for query '{}':\n{}",
                hits.len(),
                query,
                joined
            )
        };
        let output = serde_json::json!({
            "query": query,
            "sessionId": session_id,
            "hitCount": hits.len(),
            "hits": hit_previews,
            "directWritesExecuted": false,
        });
        let action = AgentAction {
            id: action_id.clone(),
            action_type: request.action_type,
            target: Some(request.target),
            input: request.input,
            output: Some(output.clone()),
            status: "succeeded".into(),
            error: None,
            permission_decision: Some("read_only_memory_search".into()),
            started_at: Some(now),
            finished_at: Some(now),
            timestamp: now,
            tool_scope: None,
            react_trace: None,
        };
        let observation = AgentObservation {
            id: observation_id,
            action_id: Some(action_id),
            content,
            source: "memory_search".into(),
            structured_result: Some(serde_json::json!({
                "success": true,
                "status": "succeeded",
                "hitCount": hits.len(),
                "hits": output["hits"].clone(),
                "directWritesExecuted": false,
                "promotedToMemory": false,
            })),
            timestamp: now,
            react_trace: None,
        };

        Ok(ActionExecutionResult {
            action,
            observation,
            status: ActionExecutionStatus::Succeeded,
            stop_reason: None,
            governance_report: None,
        })
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
