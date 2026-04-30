use crate::agent::types::{AgentAction, AgentObservation, ToolActionScope};
use crate::mcp::{McpArgumentInspection, McpRegistry};
use crate::mcp_audit::McpAuditStore;
use crate::privacy::PrivacyEngine;
use crate::tool_manifest::ToolManifest;
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
                let source = canonical_tool_source(manifest);

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
                requires_confirmation: false,
                decision: "deny".into(),
                reason: "tool is not registered or disabled".into(),
                policy_id: None,
            }
        };

        // 4. Determine if blocked
        let inspection_blocks = inspection.requires_confirmation && inspection.pii_found;
        let blocked = manifest.as_ref().is_none_or(|m| !m.enabled)
            || inspection_blocks
            || decision.requires_confirmation
            || !decision.allowed;

        if blocked {
            let needs_confirmation = should_mark_needs_confirmation(&decision, &inspection);
            let (action, observation) = self.build_blocked_action_observation(
                tool_name,
                &args,
                &inspection,
                &decision,
                manifest.as_ref(),
                &request,
            );
            let status = if needs_confirmation {
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
        let manifest_ref = manifest
            .as_ref()
            .expect("manifest exists when execution is not blocked");
        let result =
            self.call_tool_internal(manifest_ref, args.clone(), ctx.registry, ctx.audit_store);

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
        manifest: &ToolManifest,
        args: Value,
        registry: &McpRegistry,
        audit: &McpAuditStore,
    ) -> ToolCallInternalResult {
        match registry.execute_manifest(manifest, args.clone()) {
            Ok(r) => {
                let pii_found = registry
                    .inspect_call_arguments(&manifest.name, &args)
                    .pii_found;
                if let Err(e) = audit.insert_log(&manifest.name, &args, &r, true, pii_found) {
                    eprintln!("[warn] audit log write failed: {}", e);
                }
                ToolCallInternalResult {
                    success: true,
                    output: Some(r),
                    error: None,
                }
            }
            Err(e) => {
                let pii_found = registry
                    .inspect_call_arguments(&manifest.name, &args)
                    .pii_found;
                if let Err(log_err) =
                    audit.insert_log(&manifest.name, &args, &e.to_string(), false, pii_found)
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
        let needs_confirmation = should_mark_needs_confirmation(decision, inspection);
        let action_id = format!(
            "action-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );

        let status = if needs_confirmation {
            "needs_confirmation"
        } else {
            "blocked"
        };

        let tool_scope = manifest.map(|m| ToolActionScope {
            tool_name: m.name.clone(),
            tool_id: m.id.clone(),
            source: canonical_tool_source(m),
            risk_level: m.risk_level.clone(),
            capabilities: m.capabilities.clone(),
            action_type: "mcp_tool_call".into(),
            requires_confirmation: needs_confirmation,
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
            error: if needs_confirmation {
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
            content: if needs_confirmation {
                "Tool call requires permission confirmation".to_string()
            } else {
                decision.reason.clone()
            },
            source: manifest
                .map(canonical_tool_source)
                .unwrap_or_else(|| "builtin".to_string()),
            structured_result: Some(serde_json::json!({
                "success": false,
                "status": status,
                "requires_confirmation": needs_confirmation,
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
            source: canonical_tool_source(m),
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
                .map(canonical_tool_source)
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

    fn execute_memory_write(&self, request: AgentActionRequest) -> Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "memory_write must be submitted as a MemoryWrite proposal before persistence",
        ))
    }

    fn execute_memory_archive(&self, request: AgentActionRequest) -> Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "memory_archive must be submitted as a MemoryArchive proposal before persistence",
        ))
    }

    fn execute_life_model_patch(
        &self,
        request: AgentActionRequest,
    ) -> Result<ActionExecutionResult> {
        Ok(self.build_proposal_required_action(
            request,
            "life_model_patch must be submitted as a LifeModel proposal before persistence",
        ))
    }

    fn build_proposal_required_action(
        &self,
        request: AgentActionRequest,
        reason: &str,
    ) -> ActionExecutionResult {
        let now = chrono::Utc::now();
        let action_id = format!(
            "action-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );
        let status = if self.config.allow_writes {
            "needs_confirmation"
        } else {
            "blocked"
        };
        let action = AgentAction {
            id: action_id.clone(),
            action_type: request.action_type.clone(),
            target: Some(request.target.clone()),
            input: request.input.clone(),
            output: None,
            status: status.into(),
            permission_decision: Some("proposal_required".into()),
            tool_scope: None,
            started_at: Some(now),
            finished_at: Some(now),
            error: Some(reason.to_string()),
            timestamp: now,
        };
        let observation = AgentObservation {
            id: format!(
                "observation-{}-{}",
                request.step_index,
                now.timestamp_nanos_opt().unwrap_or_default()
            ),
            action_id: Some(action_id),
            content: reason.to_string(),
            source: "action_executor".into(),
            structured_result: Some(serde_json::json!({
                "success": false,
                "status": status,
                "requires_confirmation": self.config.allow_writes,
                "permission_decision": "proposal_required",
                "proposal_required": true,
            })),
            timestamp: now,
        };
        ActionExecutionResult {
            action,
            observation,
            status: if self.config.allow_writes {
                ActionExecutionStatus::NeedsConfirmation
            } else {
                ActionExecutionStatus::Blocked
            },
            stop_reason: Some("proposal_required".into()),
        }
    }
}

fn canonical_tool_source(manifest: &ToolManifest) -> String {
    manifest.source.to_string()
}

fn should_mark_needs_confirmation(
    decision: &ToolPermissionDecision,
    inspection: &McpArgumentInspection,
) -> bool {
    decision.requires_confirmation || (inspection.requires_confirmation && inspection.pii_found)
}

#[derive(Debug)]
struct ToolCallInternalResult {
    success: bool,
    output: Option<String>,
    error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::McpRegistry;
    use crate::mcp_audit::McpAuditStore;
    use crate::privacy::PrivacyEngine;
    use crate::tool_manifest::ToolSource;
    use crate::tool_permissions::{ToolPermissionPolicy, ToolPermissionStore};

    fn test_context<'a>(
        registry: &'a McpRegistry,
        permission_store: &'a ToolPermissionStore,
        audit_store: &'a McpAuditStore,
        privacy_engine: &'a PrivacyEngine,
    ) -> ActionExecutionContext<'a> {
        ActionExecutionContext {
            registry,
            permission_store,
            audit_store,
            privacy_engine,
        }
    }

    #[test]
    fn builtin_tool_executes_through_manifest_registry_path() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "builtin_echo".into(),
                    input: serde_json::json!({"arguments": {"text": "hello beta"}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Succeeded);
        assert_eq!(result.action.status, "succeeded");
        assert_eq!(result.observation.content, "hello beta");
        assert_eq!(result.action.tool_scope.as_ref().unwrap().source, "builtin");
    }

    #[test]
    fn tool_permission_check_uses_canonical_manifest_source() {
        let mut registry = McpRegistry::new();
        registry.register_builtin(
            ToolManifest {
                id: "write_file".into(),
                name: "write_file".into(),
                description: "test write".into(),
                parameters: serde_json::json!({"type": "object"}),
                permission_level: "high".into(),
                risk_level: "high".into(),
                version: "1.0.0".into(),
                source: ToolSource::Mcp {
                    server_name: "filesystem".into(),
                },
                capabilities: vec!["write".into(), "filesystem".into()],
                requires_confirmation: true,
                enabled: true,
                tags: vec![],
            },
            Box::new(|_| Ok("written".into())),
        );
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "write_file",
                "mcp:filesystem",
                "high",
                "mcp_tool_call",
                ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "write_file".into(),
                    input: serde_json::json!({"arguments": {"path": "a.txt"}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert_eq!(result.action.status, "failed");
        assert_eq!(result.stop_reason, None);
        assert_eq!(result.action.permission_decision, None);
        assert_eq!(
            result.action.tool_scope.as_ref().unwrap().source,
            "mcp:filesystem"
        );
    }

    #[test]
    fn explicit_deny_is_blocked_not_confirmation() {
        let mut registry = McpRegistry::new();
        registry.register_builtin(
            ToolManifest {
                id: "dangerous_write".into(),
                name: "dangerous_write".into(),
                description: "test deny".into(),
                parameters: serde_json::json!({"type": "object"}),
                permission_level: "high".into(),
                risk_level: "high".into(),
                version: "1.0.0".into(),
                source: ToolSource::Mcp {
                    server_name: "filesystem".into(),
                },
                capabilities: vec!["write".into(), "filesystem".into()],
                requires_confirmation: true,
                enabled: true,
                tags: vec![],
            },
            Box::new(|_| Ok("should not run".into())),
        );
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "dangerous_write",
                "mcp:filesystem",
                "high",
                "mcp_tool_call",
                ToolPermissionPolicy::Deny,
                None,
            )
            .unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "dangerous_write".into(),
                    input: serde_json::json!({"arguments": {"path": "a.txt"}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(result.action.status, "blocked");
        assert_eq!(result.action.permission_decision.as_deref(), Some("deny"));
        assert_eq!(
            result
                .observation
                .structured_result
                .as_ref()
                .and_then(|v| v.get("requires_confirmation"))
                .and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn unknown_tool_is_blocked_not_confirmation() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "missing_tool".into(),
                    input: serde_json::json!({"arguments": {}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(result.action.status, "blocked");
        assert_eq!(result.action.permission_decision.as_deref(), Some("deny"));
    }

    #[test]
    fn disabled_tool_is_blocked_not_confirmation() {
        let mut registry = McpRegistry::new();
        registry.register_builtin(
            ToolManifest {
                id: "disabled_write".into(),
                name: "disabled_write".into(),
                description: "disabled".into(),
                parameters: serde_json::json!({"type": "object"}),
                permission_level: "high".into(),
                risk_level: "high".into(),
                version: "1.0.0".into(),
                source: ToolSource::Mcp {
                    server_name: "filesystem".into(),
                },
                capabilities: vec!["write".into()],
                requires_confirmation: true,
                enabled: false,
                tags: vec![],
            },
            Box::new(|_| Ok("should not run".into())),
        );
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "disabled_write".into(),
                    input: serde_json::json!({"arguments": {}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(result.action.status, "blocked");
        assert_eq!(result.action.permission_decision.as_deref(), Some("deny"));
    }

    #[test]
    fn internal_write_actions_return_proposal_required_observation() {
        let executor = ActionExecutor::new(ActionExecutorConfig::default());
        let result = executor
            .execute_memory_write(AgentActionRequest {
                action_type: "memory_write".into(),
                target: "memory".into(),
                input: serde_json::json!({"content": "remember this"}),
                source_run_id: Some("run-1".into()),
                step_index: 1,
            })
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::NeedsConfirmation);
        assert_eq!(result.stop_reason.as_deref(), Some("proposal_required"));
        assert_eq!(
            result.action.permission_decision.as_deref(),
            Some("proposal_required")
        );
        assert_eq!(
            result
                .observation
                .structured_result
                .as_ref()
                .and_then(|v| v.get("proposal_required"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
    }
}
