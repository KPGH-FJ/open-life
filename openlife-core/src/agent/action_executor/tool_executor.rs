use crate::mcp::McpArgumentInspection;
use crate::mcp::McpRegistry;
use crate::mcp_audit::McpAuditStore;
use crate::tool_manifest::ToolManifest;
use crate::tool_permissions::ToolPermissionDecision;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

use super::helpers::{
    canonical_tool_source, filesystem_access_error, is_path_in_safe_paths, normalize_tool_name,
    should_mark_needs_confirmation, ToolCallInternalResult,
};
use super::ActionExecutionContext;
use super::ActionExecutionResult;
use super::ActionExecutionStatus;
use super::AgentActionRequest;
use crate::agent::shell_executor::{ShellCommandRequest, ShellExecutor};
use crate::agent::types::{
    AgentAction, AgentEventActor, AgentObservation, AgentProposal, AgentRunEvent,
    AgentRunEventType, ProposalSource, ProposalType, RiskLevel, ToolActionScope,
};

/// Returns true if the tool name indicates a proposal-generation tool that
/// only creates a user-confirmable Proposal (no direct side effect).
fn is_proposal_generation_tool(name: &str) -> bool {
    name.ends_with("_proposal")
        || name.ends_with("_propose_write")
        || name.ends_with("_propose_archive")
        || name.ends_with("_propose_patch")
        || name.ends_with("_propose_update")
        || name.ends_with(".propose_write")
        || name.ends_with(".propose_archive")
        || name.ends_with(".propose_patch")
        || name.ends_with(".propose_update")
        || name.ends_with(".propose_event")
}

impl super::ActionExecutor {
    /// Execute a tool action (MCP, builtin, or plugin).
    pub fn execute_tool(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
    ) -> Result<ActionExecutionResult> {
        let normalized_target = normalize_tool_name(&request.target, ctx.registry);
        let tool_name = &normalized_target;
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

        // ── P9: shell.run governed execution ──────────────────────────
        if tool_name == "shell.run" {
            return self.execute_shell_run(&args, ctx, &request, manifest.as_ref(), &inspection);
        }

        // 3. Check permission with canonical decision order:
        //    unknown -> blocked
        //    disabled/declarative-only -> blocked
        //    explicit deny -> blocked
        //    allow_once -> execute (consume in step 5)
        //    allow_until_revoked -> execute
        //    high-risk without allow -> needs_confirmation
        //    low-risk read -> execute
        let decision = if let Some(ref manifest) = manifest {
            if !manifest.enabled || manifest.declarative_only {
                ToolPermissionDecision {
                    allowed: false,
                    requires_confirmation: false,
                    decision: "deny".into(),
                    reason: if manifest.declarative_only {
                        "tool is declarative-only (no executor available)"
                    } else {
                        "tool is disabled"
                    }
                    .into(),
                    policy_id: None,
                }
            } else {
                let source = canonical_tool_source(manifest);

                let perm_check = if self.config.consume_allow_once {
                    ctx.permission_store.check(
                        &manifest.name,
                        &source,
                        &manifest.risk_level,
                        &manifest.action_type,
                        &manifest.capabilities,
                    )
                } else {
                    ctx.permission_store.peek(
                        &manifest.name,
                        &source,
                        &manifest.risk_level,
                        &manifest.action_type,
                        &manifest.capabilities,
                    )
                };

                perm_check.unwrap_or(ToolPermissionDecision {
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

        // 4. Determine if blocked.
        // Proposal-generation tools (file.write_proposal, memory.propose_write, etc.)
        // only create proposals; they don't execute side effects directly.
        // They are exempt from permission-confirmation blocking so the agent can
        // always reach the handler that creates the proposal for user review.
        let is_proposal_tool = manifest
            .as_ref()
            .is_none_or(|m| is_proposal_generation_tool(&m.name));
        let permission_blocks =
            !is_proposal_tool && (decision.requires_confirmation || !decision.allowed);
        let inspection_blocks = inspection.requires_confirmation && inspection.pii_found;
        let blocked = manifest
            .as_ref()
            .is_none_or(|m| !m.enabled || m.declarative_only)
            || inspection_blocks
            || permission_blocks;

        if blocked {
            // Special handling for declarative stubs that should create proposals
            if let Some(ref m) = manifest {
                if m.declarative_only {
                    match tool_name.as_str() {
                        "calendar.propose_event" => {
                            if let Some(result) = self.create_declarative_stub_proposal(
                                &request,
                                ctx,
                                tool_name,
                                &args,
                                ProposalType::ScheduledTask,
                                "calendar",
                                "Agent proposed calendar event",
                            ) {
                                return result;
                            }
                        }
                        "email.propose_draft" => {
                            if let Some(result) = self.create_declarative_stub_proposal(
                                &request,
                                ctx,
                                tool_name,
                                &args,
                                ProposalType::DataExport,
                                "email",
                                "Agent proposed email draft",
                            ) {
                                return result;
                            }
                        }
                        _ => {}
                    }
                }
            }

            let needs_confirmation = should_mark_needs_confirmation(&decision, &inspection);

            // Auto-generate ToolPermission Proposal when blocked by policy
            // so the user can grant permission and continue in the Review Center.
            if needs_confirmation
                && manifest
                    .as_ref()
                    .is_some_and(|m| !m.declarative_only && !is_proposal_generation_tool(&m.name))
            {
                if let Some(result) = self.create_tool_permission_proposal(
                    &request,
                    ctx,
                    tool_name,
                    &args,
                    manifest.as_ref(),
                    &decision,
                ) {
                    return result;
                }
                // Fall-through: if proposal creation fails, return NeedsConfirmation status
            }

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

            // Record tool.call_blocked event
            if let (Some(event_store), Some(ref run_id)) =
                (ctx.event_store.as_ref(), &request.source_run_id)
            {
                let event = AgentRunEvent::new(
                    run_id,
                    AgentRunEventType::ToolCallBlocked,
                    AgentEventActor::Tool(tool_name.to_string()),
                    format!("Tool '{}' blocked: {}", tool_name, decision.reason),
                    serde_json::json!({
                        "tool": tool_name,
                        "reason": decision.reason,
                        "declarative_only": manifest.as_ref().is_some_and(|m| m.declarative_only),
                        "needs_confirmation": needs_confirmation,
                    }),
                );
                let _ = event_store.append_event(&event);
            }

            return Ok(ActionExecutionResult {
                action,
                observation,
                status,
                stop_reason: Some("blocked_by_policy".into()),
            });
        }

        // 5. Safe Paths check for filesystem tools
        if let Some(ref m) = manifest {
            if m.capabilities.contains(&"filesystem".to_string()) {
                let path = args
                    .get("path")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("");
                if !is_path_in_safe_paths(path, ctx.safe_paths) {
                    let (action, observation) = self.build_blocked_action_observation(
                        tool_name,
                        &args,
                        &inspection,
                        &ToolPermissionDecision {
                            allowed: false,
                            requires_confirmation: false,
                            decision: "blocked".into(),
                            reason: filesystem_access_error(path, ctx.safe_paths),
                            policy_id: None,
                        },
                        manifest.as_ref(),
                        &request,
                    );
                    return Ok(ActionExecutionResult {
                        action,
                        observation,
                        status: ActionExecutionStatus::Blocked,
                        stop_reason: Some("path_not_in_safe_paths".into()),
                    });
                }
            }
        }

        // 6. Execute
        let manifest_ref = manifest
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool manifest not found for '{}'", tool_name))?;
        let result = if manifest_ref.tags.contains(&"core_os".to_string()) {
            self.execute_core_os_tool(tool_name, &args, ctx)
                .unwrap_or_else(|e| ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                })
        } else if manifest_ref.tags.contains(&"execution".to_string()) {
            self.execute_execution_tool(tool_name, &args, ctx, &request)
                .unwrap_or_else(|e| ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                })
        } else {
            self.call_tool_internal(
                manifest_ref,
                args.clone(),
                ctx.registry,
                ctx.audit_store,
                inspection.pii_found,
            )
        };

        let (mut action, observation) = self.build_success_action_observation(
            tool_name,
            &args,
            &result,
            manifest.as_ref(),
            &request,
        );

        // For mcp.call_tool: override tool_scope with target manifest and handle
        // target tool permission failures as NeedsConfirmation instead of Failed
        if tool_name == "mcp.call_tool" {
            if let Some(target_name) = args.get("tool_name").and_then(|v: &Value| v.as_str()) {
                if let Some(target_manifest) = ctx
                    .registry
                    .list_manifests()
                    .into_iter()
                    .find(|m| m.name == target_name || m.id == target_name)
                {
                    action.tool_scope = Some(ToolActionScope {
                        tool_name: target_manifest.name.clone(),
                        tool_id: target_manifest.id.clone(),
                        source: canonical_tool_source(&target_manifest),
                        risk_level: target_manifest.risk_level.clone(),
                        capabilities: target_manifest.capabilities.clone(),
                        action_type: target_manifest.action_type.clone(),
                        requires_confirmation: false,
                        allowed: result.success,
                    });
                }
            }
            // If target tool permission was denied, treat as NeedsConfirmation
            if !result.success {
                if let Some(ref error) = result.error {
                    if error.contains("blocked") || error.contains("ask_every_time") {
                        action.status = "needs_confirmation".to_string();
                        return Ok(ActionExecutionResult {
                            action,
                            observation,
                            status: ActionExecutionStatus::NeedsConfirmation,
                            stop_reason: Some("target_tool_needs_confirmation".into()),
                        });
                    }
                }
            }
        }

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

    pub fn call_tool_internal(
        &self,
        manifest: &ToolManifest,
        args: Value,
        registry: &McpRegistry,
        audit: &McpAuditStore,
        pii_found: bool,
    ) -> ToolCallInternalResult {
        match registry.execute_manifest(manifest, args.clone()) {
            Ok(r) => {
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

    pub fn build_blocked_action_observation(
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
            action_type: m.action_type.clone(),
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

    pub fn build_success_action_observation(
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
            action_type: m.action_type.clone(),
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

    pub fn build_proposal_required_action(
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

    /// Auto-create a ToolPermission Proposal when a tool is blocked by policy.
    /// The proposal records the blocked action so it can be replayed after the
    /// user grants permission in the Review Center.
    fn create_tool_permission_proposal(
        &self,
        request: &AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        tool_name: &str,
        args: &Value,
        manifest: Option<&ToolManifest>,
        decision: &ToolPermissionDecision,
    ) -> Option<anyhow::Result<ActionExecutionResult>> {
        let proposal_store = ctx.proposal_store?;
        let source = manifest
            .map(canonical_tool_source)
            .unwrap_or_else(|| "builtin".to_string());
        let risk_level = manifest
            .map(|m| m.risk_level.clone())
            .unwrap_or_else(|| "medium".to_string());

        let after = serde_json::json!({
            "permission_action": "grant",
            "tool_name": tool_name,
            "source": source,
            "risk_level": risk_level,
            "policy": "allow_until_revoked",
            "blocked_action": {
                "action_type": request.action_type,
                "target": request.target,
                "input": args,
                "source_run_id": request.source_run_id,
                "step_index": request.step_index,
            },
            "reason": decision.reason,
            "auto_generated": true,
        });

        let affected_path = format!("tool_permission.{}.{}", source, tool_name);
        let mut proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            &affected_path,
            after,
            &format!(
                "[Auto] 工具 '{}' ({}，风险等级：{}) 需要权限确认。原因：{}",
                tool_name, source, risk_level, decision.reason
            ),
            0.7,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );

        if let Some(ref run_id) = request.source_run_id {
            proposal.run_id = Some(run_id.clone());
        }

        if let Err(e) = proposal_store.create_proposal(&proposal) {
            eprintln!(
                "[warn] Failed to create ToolPermission Proposal for {}: {}",
                tool_name, e
            );
            return None;
        }

        let result = self.build_proposal_required_action(
            request.clone(),
            &format!(
                "{}: 已创建 ToolPermission 提案 (id: {})，请前往 Review Center 审批",
                tool_name, proposal.id
            ),
        );

        Some(Ok(result))
    }

    /// P9: Governed shell.run execution through ActionExecutor only.
    ///
    /// Checks in order:
    /// 1. manifest exists
    /// 2. manifest enabled && !declarative_only
    /// 3. sandbox.bash_enabled == true
    /// 4. permission policy
    /// 5. ShellExecutor validation
    ///
    /// Records AgentRunEvent for every outcome (blocked/started/completed/failed/timeout).
    fn execute_shell_run(
        &self,
        args: &Value,
        ctx: &ActionExecutionContext<'_>,
        request: &AgentActionRequest,
        manifest: Option<&ToolManifest>,
        inspection: &McpArgumentInspection,
    ) -> Result<ActionExecutionResult> {
        let tool_name = "shell.run";

        // Record tool.call_blocked helper
        let record_blocked = |reason: &str, payload: serde_json::Value| {
            if let (Some(event_store), Some(ref run_id)) =
                (ctx.event_store.as_ref(), &request.source_run_id)
            {
                let event = AgentRunEvent::new(
                    run_id,
                    AgentRunEventType::ToolCallBlocked,
                    AgentEventActor::Tool(tool_name.to_string()),
                    format!("shell.run blocked: {}", reason),
                    payload,
                );
                let _ = event_store.append_event(&event);
            }
        };

        // ── 1. Manifest check ──────────────────────────────────────────
        let manifest = match manifest {
            Some(m) => m,
            None => {
                let reason = "shell.run tool is not registered";
                record_blocked(reason, serde_json::json!({"reason": reason}));
                return Ok(self.build_blocked_result(
                    tool_name, args, request, reason, false, None,
                ));
            }
        };

        if !manifest.enabled {
            let reason = "shell.run is disabled in manifest";
            record_blocked(reason, serde_json::json!({"reason": reason}));
            return Ok(self.build_blocked_result(
                tool_name, args, request, reason, false, Some(manifest),
            ));
        }

        if manifest.declarative_only {
            let reason = "shell.run is declarative-only (no executor available)";
            record_blocked(reason, serde_json::json!({"reason": reason}));
            return Ok(self.build_blocked_result(
                tool_name, args, request, reason, false, Some(manifest),
            ));
        }

        // ── 2. Sandbox check ───────────────────────────────────────────
        let sandbox = ctx.execution_sandbox;
        if !sandbox.bash_enabled {
            let reason = "shell execution is disabled (sandbox.bash_enabled = false)";
            record_blocked(reason, serde_json::json!({
                "reason": reason,
                "bash_enabled": false,
            }));
            return Ok(self.build_blocked_result(
                tool_name, args, request, reason, false, Some(manifest),
            ));
        }

        // ── 3. AgentSpec gate ──────────────────────────────────────────
        // AgentSpec is mandatory for shell.run; missing spec = fail-closed.
        match ctx.agent_spec {
            Some(spec) if spec.is_tool_allowed(tool_name) => {
                // AgentSpec allows — continue to permission check.
            }
            Some(_) => {
                let reason = "AgentSpec denied shell.run";
                record_blocked(reason, serde_json::json!({
                    "reason": reason,
                    "agent_spec_id": ctx.agent_spec.map(|s| s.id.clone()),
                }));
                return Ok(self.build_blocked_result(
                    tool_name, args, request, reason, false, Some(manifest),
                ));
            }
            None => {
                let reason = "AgentSpec missing: cannot execute shell.run without governed AgentSpec";
                record_blocked(reason, serde_json::json!({
                    "reason": reason,
                }));
                return Ok(self.build_blocked_result(
                    tool_name, args, request, reason, false, Some(manifest),
                ));
            }
        }

        // ── 4. Permission check ────────────────────────────────────────
        let source = canonical_tool_source(manifest);
        let perm_decision = if self.config.consume_allow_once {
            ctx.permission_store.check(
                &manifest.name,
                &source,
                &manifest.risk_level,
                &manifest.action_type,
                &manifest.capabilities,
            )
        } else {
            ctx.permission_store.peek(
                &manifest.name,
                &source,
                &manifest.risk_level,
                &manifest.action_type,
                &manifest.capabilities,
            )
        }
        .unwrap_or(ToolPermissionDecision {
            allowed: false,
            requires_confirmation: true,
            decision: "ask_every_time".into(),
            reason: "permission check failed".into(),
            policy_id: None,
        });

        if !perm_decision.allowed {
            let needs_confirmation = perm_decision.requires_confirmation
                || manifest.requires_confirmation
                || inspection.requires_confirmation;
            let reason = if perm_decision.requires_confirmation {
                "shell.run requires user permission confirmation"
            } else {
                "shell.run permission denied"
            };

            record_blocked(&reason, serde_json::json!({
                "reason": reason,
                "needs_confirmation": needs_confirmation,
                "permission_decision": perm_decision.decision,
            }));

            if needs_confirmation {
                // Auto-generate ToolPermission Proposal
                if let Some(result) = self.create_tool_permission_proposal(
                    request,
                    ctx,
                    tool_name,
                    args,
                    Some(manifest),
                    &perm_decision,
                ) {
                    return result;
                }
            }

            let (action, observation) = self.build_blocked_action_observation(
                tool_name,
                args,
                inspection,
                &perm_decision,
                Some(manifest),
                request,
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
                stop_reason: Some(if needs_confirmation {
                    "shell_needs_confirmation".to_string()
                } else {
                    "shell_blocked".to_string()
                }),
            });
        }

        // ── 4. Build ShellCommandRequest ────────────────────────────────
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let cmd_args: Vec<String> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let cwd = args.get("cwd").and_then(|v| v.as_str()).map(String::from);
        let env: HashMap<String, String> = args
            .get("env")
            .and_then(|v| v.as_object())
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let shell_request = ShellCommandRequest {
            command: command.to_string(),
            args: cmd_args,
            cwd,
            env,
            reason: args
                .get("reason")
                .and_then(|v| v.as_str())
                .map(String::from),
        };

        // ── 5. Execute via ShellExecutor ────────────────────────────────
        let executor = ShellExecutor::new(sandbox.clone());

        // Record tool.call_started
        if let (Some(event_store), Some(ref run_id)) =
            (ctx.event_store.as_ref(), &request.source_run_id)
        {
            let event = AgentRunEvent::new(
                run_id,
                AgentRunEventType::ToolCallStarted,
                AgentEventActor::Tool(tool_name.to_string()),
                format!("shell.run executing command: {}", command),
                serde_json::json!({
                    "command": command,
                    "args": shell_request.args,
                    "cwd": shell_request.cwd,
                }),
            );
            let _ = event_store.append_event(&event);
        }

        let (status, output, error_msg) = match executor.execute(&shell_request) {
            Ok(output) => {
                let truncated = output.truncated || output.timed_out;
                let status_str = if output.timed_out {
                    ActionExecutionStatus::Failed
                } else if output.exit_code != 0 {
                    ActionExecutionStatus::Succeeded // non-zero exit is still a completed execution
                } else {
                    ActionExecutionStatus::Succeeded
                };

                let output_json = serde_json::json!({
                    "stdout": output.stdout,
                    "stderr": output.stderr,
                    "exit_code": output.exit_code,
                    "timed_out": output.timed_out,
                    "truncated": truncated,
                    "elapsed_ms": output.elapsed_ms,
                });

                if let (Some(event_store), Some(ref run_id)) =
                    (ctx.event_store.as_ref(), &request.source_run_id)
                {
                    let event_type = if output.timed_out {
                        AgentRunEventType::ToolCallFailed
                    } else {
                        AgentRunEventType::ToolCallCompleted
                    };
                    let summary = if output.timed_out {
                        format!("shell.run timed out after {} ms: {}", output.elapsed_ms, command)
                    } else {
                        format!("shell.run completed: {} (exit={})", command, output.exit_code)
                    };
                    let event = AgentRunEvent::new(
                        run_id,
                        event_type,
                        AgentEventActor::Tool(tool_name.to_string()),
                        summary,
                        output_json.clone(),
                    );
                    let _ = event_store.append_event(&event);
                }

                (status_str, Some(output_json.to_string()), None)
            }
            Err(e) => {
                let err_str = e.to_string();
                if let (Some(event_store), Some(ref run_id)) =
                    (ctx.event_store.as_ref(), &request.source_run_id)
                {
                    let event = AgentRunEvent::new(
                        run_id,
                        AgentRunEventType::ToolCallFailed,
                        AgentEventActor::Tool(tool_name.to_string()),
                        format!("shell.run failed: {}", err_str),
                        serde_json::json!({
                            "command": command,
                            "error": err_str,
                        }),
                    );
                    let _ = event_store.append_event(&event);
                }
                (
                    ActionExecutionStatus::Failed,
                    None,
                    Some(err_str),
                )
            }
        };

        // ── 6. Build result ────────────────────────────────────────────
        let now = chrono::Utc::now();
        let action_id = format!(
            "action-shell-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );

        let obs_content = if let Some(ref out) = output {
            format!("[shell.run] {} executed: {}", command, out)
        } else if let Some(ref err) = error_msg {
            format!("[shell.run] {} failed: {}", command, err)
        } else {
            format!("[shell.run] {} executed", command)
        };

        let tool_scope = Some(ToolActionScope {
            tool_name: tool_name.to_string(),
            tool_id: manifest.id.clone(),
            source: canonical_tool_source(manifest),
            risk_level: manifest.risk_level.clone(),
            capabilities: manifest.capabilities.clone(),
            action_type: manifest.action_type.clone(),
            requires_confirmation: false,
            allowed: status != ActionExecutionStatus::Failed,
        });

        let status_str: &str = match status {
            ActionExecutionStatus::Succeeded => "succeeded",
            ActionExecutionStatus::Failed => "failed",
            ActionExecutionStatus::Blocked => "blocked",
            ActionExecutionStatus::NeedsConfirmation => "needs_confirmation",
        };

        let action = AgentAction {
            id: action_id.clone(),
            action_type: request.action_type.clone(),
            target: Some(tool_name.to_string()),
            input: args.clone(),
            output: output
                .as_ref()
                .map(|s: &String| serde_json::json!({"text": s})),
            status: status_str.to_string(),
            permission_decision: Some(perm_decision.decision),
            tool_scope,
            started_at: Some(now),
            finished_at: Some(now),
            error: error_msg.clone(),
            timestamp: now,
        };

        let observation = AgentObservation {
            id: format!(
                "observation-shell-{}-{}",
                request.step_index,
                now.timestamp_nanos_opt().unwrap_or_default()
            ),
            action_id: Some(action_id),
            content: obs_content,
            source: canonical_tool_source(manifest),
            structured_result: Some(serde_json::json!({
                "success": status == ActionExecutionStatus::Succeeded,
                "status": status_str,
                "truncated": output.as_ref().and_then(|s| {
                    serde_json::from_str::<serde_json::Value>(s).ok()
                        .and_then(|v| v.get("truncated").cloned())
                }).unwrap_or(serde_json::Value::Bool(false)),
                "timed_out": output.as_ref().and_then(|s| {
                    serde_json::from_str::<serde_json::Value>(s).ok()
                        .and_then(|v| v.get("timed_out").cloned())
                }).unwrap_or(serde_json::Value::Bool(false)),
            })),
            timestamp: now,
        };

        let stop_reason = if status == ActionExecutionStatus::Failed {
            Some("shell_execution_failed".to_string())
        } else {
            None
        };

        Ok(ActionExecutionResult {
            action,
            observation,
            status,
            stop_reason,
        })
    }

    /// Build a blocked ActionExecutionResult for early-return paths.
    fn build_blocked_result(
        &self,
        tool_name: &str,
        args: &Value,
        request: &AgentActionRequest,
        reason: &str,
        needs_confirmation: bool,
        manifest: Option<&ToolManifest>,
    ) -> ActionExecutionResult {
        let now = chrono::Utc::now();
        let action_id = format!(
            "action-shell-blocked-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );
        let status_str = if needs_confirmation {
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
            action_type: m.action_type.clone(),
            requires_confirmation: needs_confirmation,
            allowed: false,
        });

        let action = AgentAction {
            id: action_id.clone(),
            action_type: request.action_type.clone(),
            target: Some(tool_name.to_string()),
            input: args.clone(),
            output: None,
            status: status_str.to_string(),
            permission_decision: Some("deny".to_string()),
            tool_scope,
            started_at: Some(now),
            finished_at: Some(now),
            error: Some(reason.to_string()),
            timestamp: now,
        };

        let observation = AgentObservation {
            id: format!(
                "observation-shell-blocked-{}-{}",
                request.step_index,
                now.timestamp_nanos_opt().unwrap_or_default()
            ),
            action_id: Some(action_id),
            content: format!("[shell.run] blocked: {}", reason),
            source: manifest
                .map(canonical_tool_source)
                .unwrap_or_else(|| "builtin".to_string()),
            structured_result: Some(serde_json::json!({
                "success": false,
                "status": status_str,
                "blocked_reason": reason,
            })),
            timestamp: now,
        };

        let status = if needs_confirmation {
            ActionExecutionStatus::NeedsConfirmation
        } else {
            ActionExecutionStatus::Blocked
        };

        ActionExecutionResult {
            action,
            observation,
            status,
            stop_reason: Some("shell_blocked".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::action_executor::ActionExecutorConfig;
    use crate::mcp::McpRegistry;
    use crate::mcp_audit::McpAuditStore;
    use crate::privacy::PrivacyEngine;
    use crate::tool_permissions::ToolPermissionStore;

    // ── P9-5: ActionExecutor shell.run governed path tests ───────────

    #[test]
    fn test_shell_run_disabled_sandbox_records_blocked() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit =
            McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_tool.db"));
        let pe = PrivacyEngine::new();
        let sandbox = crate::agent::execution_sandbox::ExecutionSandbox::always_disabled();
        let event_store = crate::agent::event_store::AgentRunEventStore::new_in_memory().unwrap();
        let ctx = crate::agent::ActionExecutionContext::new(&reg, &ps, &audit, &pe, &[])
            .with_execution_sandbox(&sandbox)
            .with_event_store(event_store.clone());

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "shell.run".into(),
            input: serde_json::json!({"arguments": {"command": "echo", "args": ["hello"]}}),
            source_run_id: Some("test-run-1".into()),
            step_index: 0,
        };
        let result = executor.execute(request, &ctx).unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert!(result
            .action
            .error
            .unwrap_or_default()
            .contains("disabled"));

        let events = event_store.list_events_by_run("test-run-1").unwrap();
        let has_blocked = events.iter().any(|e| {
            matches!(
                e.event_type,
                crate::agent::AgentRunEventType::ToolCallBlocked
            )
        });
        assert!(has_blocked, "blocked event must be recorded by ActionExecutor");
    }

    #[test]
    fn test_shell_run_manifest_disabled_blocks_in_action_executor() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit =
            McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_tool2.db"));
        let pe = PrivacyEngine::new();
        // Shell enabled sandbox, but manifest is disabled by default
        let tmp = std::env::temp_dir().to_string_lossy().to_string();
        let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
            bash_enabled: true,
            cwd: tmp.clone(),
            safe_paths: vec![tmp],
            command_allowlist: vec!["echo".into()],
            ..crate::agent::execution_sandbox::ExecutionSandbox::default()
        };
        let ctx = crate::agent::ActionExecutionContext::new(&reg, &ps, &audit, &pe, &[])
            .with_execution_sandbox(&sandbox);

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "shell.run".into(),
            input: serde_json::json!({"arguments": {"command": "echo", "args": ["test"]}}),
            source_run_id: None,
            step_index: 0,
        };
        let result = executor.execute(request, &ctx).unwrap();
        // Manifest is disabled → blocked at the manifest check
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert!(result.action.error.unwrap_or_default().contains("disabled"));
    }
}
