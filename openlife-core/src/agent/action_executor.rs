use crate::agent::types::{AgentAction, AgentObservation, ToolActionScope};
use crate::mcp::{McpArgumentInspection, McpRegistry};
use crate::mcp_audit::McpAuditStore;
use crate::privacy::PrivacyEngine;
use crate::tool_manifest::{ToolManifest, ToolSource};
use crate::tool_permissions::{ToolPermissionDecision, ToolPermissionStore};
use anyhow::Result;
use serde_json::Value;

/// Configuration for action execution.
#[derive(Debug, Clone)]
pub struct ActionExecutorConfig {
    pub allow_writes: bool,
    pub allow_cloud: bool,
    pub timeout_seconds: u64,
}

impl Default for ActionExecutorConfig {
    fn default() -> Self {
        Self {
            allow_writes: true,
            allow_cloud: true,
            timeout_seconds: 120,
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
pub struct ActionExecutionContext<'a> {
    pub registry: &'a McpRegistry,
    pub permission_store: &'a ToolPermissionStore,
    pub audit_store: &'a McpAuditStore,
    pub privacy_engine: &'a PrivacyEngine,
}

/// Centralized action executor for all agent actions.
///
/// This is the single entry point for executing tools, memory operations,
/// and life model patches. It handles permission checks, PII inspection,
/// audit logging, and building the action/observation pair.
pub struct ActionExecutor {
    _config: ActionExecutorConfig,
}

impl ActionExecutor {
    pub fn new(config: ActionExecutorConfig) -> Self {
        Self { _config: config }
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

    /// Execute a tool action (MCP, builtin, or plugin).
    fn execute_tool(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
    ) -> Result<ActionExecutionResult> {
        let tool_name = &request.target;
        let args = request
            .input
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| request.input.clone());

        // 1. Lookup manifest
        let manifest = ctx
            .registry
            .list_manifests()
            .into_iter()
            .find(|m| m.name == *tool_name || m.id == *tool_name);

        // 2. Inspect PII
        let inspection = ctx.registry.inspect_call_arguments(tool_name, &args);

        // 3. Check permission
        let decision = if let Some(ref manifest) = manifest {
            if !manifest.enabled {
                ToolPermissionDecision {
                    allowed: false,
                    requires_confirmation: false,
                    decision: "deny".into(),
                    reason: "tool is disabled".into(),
                    policy_id: None,
                }
            } else {
                let source = match &manifest.source {
                    ToolSource::BuiltIn => "builtin".to_string(),
                    ToolSource::Mcp { .. } => "mcp".to_string(),
                    ToolSource::A2A { .. } => "a2a".to_string(),
                    ToolSource::Plugin { plugin_id } => format!("plugin:{}", plugin_id),
                };

                ctx.permission_store
                    .check(
                        &manifest.name,
                        &source,
                        &manifest.risk_level,
                        "mcp_tool_call",
                        &manifest.capabilities,
                    )
                    .unwrap_or(ToolPermissionDecision {
                        allowed: false,
                        requires_confirmation: true,
                        decision: "ask_every_time".into(),
                        reason: "permission check failed".into(),
                        policy_id: None,
                    })
            }
        } else {
            // No manifest found
            ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "deny".into(),
                reason: "tool is not registered or disabled".into(),
                policy_id: None,
            }
        };

        // 4. Determine if blocked
        let blocked = manifest.as_ref().is_none_or(|m| !m.enabled)
            || inspection.requires_confirmation
            || decision.requires_confirmation
            || !decision.allowed;

        if blocked {
            let (action, observation) = self.build_blocked_action_observation(
                tool_name,
                &args,
                &inspection,
                &decision,
                manifest.as_ref(),
                &request,
            );
            let status = if decision.requires_confirmation || inspection.requires_confirmation {
                ActionExecutionStatus::NeedsConfirmation
            } else {
                ActionExecutionStatus::Blocked
            };
            return Ok(ActionExecutionResult {
                action,
                observation,
                status,
                stop_reason: Some("blocked_by_policy".into()),
            });
        }

        // 5. Execute
        let result =
            self.call_tool_internal(tool_name, args.clone(), ctx.registry, ctx.audit_store);

        let (action, observation) = self.build_success_action_observation(
            tool_name,
            &args,
            &result,
            manifest.as_ref(),
            &request,
        );

        let status = if result.success {
            ActionExecutionStatus::Succeeded
        } else {
            ActionExecutionStatus::Failed
        };

        Ok(ActionExecutionResult {
            action,
            observation,
            status,
            stop_reason: None,
        })
    }

    fn call_tool_internal(
        &self,
        name: &str,
        args: Value,
        registry: &McpRegistry,
        audit: &McpAuditStore,
    ) -> ToolCallInternalResult {
        match registry.call_tool(name, args.clone()) {
            Ok(r) => {
                let pii_found = registry.inspect_call_arguments(name, &args).pii_found;
                if let Err(e) = audit.insert_log(name, &args, &r, true, pii_found) {
                    eprintln!("[warn] audit log write failed: {}", e);
                }
                ToolCallInternalResult {
                    success: true,
                    output: Some(r),
                    error: None,
                }
            }
            Err(e) => {
                let pii_found = registry.inspect_call_arguments(name, &args).pii_found;
                if let Err(log_err) =
                    audit.insert_log(name, &args, &e.to_string(), false, pii_found)
                {
                    eprintln!("[warn] audit log write failed: {}", log_err);
                }
                ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                }
            }
        }
    }

    fn build_blocked_action_observation(
        &self,
        tool_name: &str,
        args: &Value,
        inspection: &McpArgumentInspection,
        decision: &ToolPermissionDecision,
        manifest: Option<&ToolManifest>,
        request: &AgentActionRequest,
    ) -> (AgentAction, AgentObservation) {
        let now = chrono::Utc::now();
        let action_id = format!(
            "action-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );

        let status = if decision.requires_confirmation || inspection.requires_confirmation {
            "needs_confirmation"
        } else {
            "failed"
        };

        let tool_scope = manifest.map(|m| ToolActionScope {
            tool_name: m.name.clone(),
            tool_id: m.id.clone(),
            source: m.source.to_string(),
            risk_level: m.risk_level.clone(),
            capabilities: m.capabilities.clone(),
            action_type: "mcp_tool_call".into(),
            requires_confirmation: decision.requires_confirmation
                || inspection.requires_confirmation,
            allowed: false,
        });

        let action = AgentAction {
            id: action_id.clone(),
            action_type: request.action_type.clone(),
            target: Some(tool_name.to_string()),
            input: args.clone(),
            output: None,
            status: status.into(),
            permission_decision: Some(decision.decision.clone()),
            tool_scope,
            started_at: Some(now),
            finished_at: Some(now),
            error: if decision.requires_confirmation || inspection.requires_confirmation {
                None
            } else {
                Some(decision.reason.clone())
            },
            timestamp: now,
        };

        let observation = AgentObservation {
            id: format!(
                "observation-{}-{}",
                request.step_index,
                now.timestamp_nanos_opt().unwrap_or_default()
            ),
            action_id: Some(action_id),
            content: if decision.requires_confirmation || inspection.requires_confirmation {
                "Tool call requires permission confirmation".to_string()
            } else {
                decision.reason.clone()
            },
            source: manifest
                .map(|m| m.source.to_string())
                .unwrap_or_else(|| "builtin".to_string()),
            structured_result: Some(serde_json::json!({
                "success": false,
                "status": status,
                "requires_confirmation": decision.requires_confirmation || inspection.requires_confirmation,
                "permission_decision": decision.decision,
            })),
            timestamp: now,
        };

        (action, observation)
    }

    fn build_success_action_observation(
        &self,
        tool_name: &str,
        args: &Value,
        result: &ToolCallInternalResult,
        manifest: Option<&ToolManifest>,
        request: &AgentActionRequest,
    ) -> (AgentAction, AgentObservation) {
        let now = chrono::Utc::now();
        let action_id = format!(
            "action-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );

        let status = if result.success {
            "succeeded"
        } else {
            "failed"
        };

        let tool_scope = manifest.map(|m| ToolActionScope {
            tool_name: m.name.clone(),
            tool_id: m.id.clone(),
            source: m.source.to_string(),
            risk_level: m.risk_level.clone(),
            capabilities: m.capabilities.clone(),
            action_type: "mcp_tool_call".into(),
            requires_confirmation: false,
            allowed: result.success,
        });

        let action = AgentAction {
            id: action_id.clone(),
            action_type: request.action_type.clone(),
            target: Some(tool_name.to_string()),
            input: args.clone(),
            output: result
                .output
                .as_ref()
                .map(|output| serde_json::json!({ "text": output })),
            status: status.into(),
            permission_decision: None,
            tool_scope,
            started_at: Some(now),
            finished_at: Some(now),
            error: result.error.clone(),
            timestamp: now,
        };

        let observation = AgentObservation {
            id: format!(
                "observation-{}-{}",
                request.step_index,
                now.timestamp_nanos_opt().unwrap_or_default()
            ),
            action_id: Some(action_id),
            content: result
                .output
                .clone()
                .or_else(|| result.error.clone())
                .unwrap_or_else(|| "Tool call produced no output".to_string()),
            source: manifest
                .map(|m| m.source.to_string())
                .unwrap_or_else(|| "builtin".to_string()),
            structured_result: Some(serde_json::json!({
                "success": result.success,
                "status": status,
                "requires_confirmation": false,
                "permission_decision": null,
            })),
            timestamp: now,
        };

        (action, observation)
    }

    fn execute_memory_write(&self, _request: AgentActionRequest) -> Result<ActionExecutionResult> {
        Err(anyhow::anyhow!(
            "memory_write action not yet implemented in ActionExecutor"
        ))
    }

    fn execute_memory_archive(
        &self,
        _request: AgentActionRequest,
    ) -> Result<ActionExecutionResult> {
        Err(anyhow::anyhow!(
            "memory_archive action not yet implemented in ActionExecutor"
        ))
    }

    fn execute_life_model_patch(
        &self,
        _request: AgentActionRequest,
    ) -> Result<ActionExecutionResult> {
        Err(anyhow::anyhow!(
            "life_model_patch action not yet implemented in ActionExecutor"
        ))
    }
}

#[derive(Debug)]
struct ToolCallInternalResult {
    success: bool,
    output: Option<String>,
    error: Option<String>,
}
