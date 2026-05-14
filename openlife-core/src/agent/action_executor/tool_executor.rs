use super::helpers::{
    canonical_tool_source, filesystem_access_error, is_path_in_safe_paths, normalize_tool_name,
    should_mark_needs_confirmation, ToolCallInternalResult,
};
use super::ActionContext;
use super::ActionExecutionResult;
use super::ActionExecutionStatus;
use super::AgentActionRequest;
use super::BorrowedActionContext;
use crate::agent::shell_executor::{ShellCommandRequest, ShellExecutor};
use crate::agent::types::{
    AgentAction, AgentEventActor, AgentObservation, AgentProposal, AgentRunEvent,
    AgentRunEventType, ProposalSource, ProposalType, RiskLevel, ToolActionScope,
};
use crate::mcp::McpArgumentInspection;
use crate::mcp::McpRegistry;
use crate::mcp_audit::McpAuditStore;
use crate::tool_manifest::ToolManifest;
use crate::tool_manifest::ToolSource;
use crate::tool_permissions::ToolPermissionDecision;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Returns true if the tool name indicates a proposal-generation tool that
/// only creates a user-confirmable Proposal (no direct side effect).
/// Lock permission_store briefly and return the permission decision
/// for the given manifest (or a deny decision if no manifest).
async fn compute_permission_decision(
    consume_allow_once: bool,
    manifest: Option<&ToolManifest>,
    ac: &ActionContext,
) -> ToolPermissionDecision {
    if let Some(m) = manifest {
        if !m.enabled || m.declarative_only {
            return ToolPermissionDecision {
                allowed: false,
                requires_confirmation: false,
                decision: "deny".into(),
                reason: if m.declarative_only {
                    "tool is declarative-only (no executor available)"
                } else {
                    "tool is disabled"
                }
                .into(),
                policy_id: None,
            };
        }
        let source = canonical_tool_source(m);
        let perm = ac.permission_store.lock().await;
        let check = if consume_allow_once {
            perm.check(
                &m.name,
                &source,
                &m.risk_level,
                &m.action_type,
                &m.capabilities,
            )
        } else {
            perm.peek(
                &m.name,
                &source,
                &m.risk_level,
                &m.action_type,
                &m.capabilities,
            )
        };
        drop(perm);
        check.unwrap_or(ToolPermissionDecision {
            allowed: false,
            requires_confirmation: true,
            decision: "ask_every_time".into(),
            reason: "permission check failed".into(),
            policy_id: None,
        })
    } else {
        ToolPermissionDecision {
            allowed: false,
            requires_confirmation: false,
            decision: "deny".into(),
            reason: "tool is not registered or disabled".into(),
            policy_id: None,
        }
    }
}

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
    /// Execute a tool action using only short-lived locks.  No store lock
    /// is held across external I/O (MCP, web, A2A, shell, file).
    pub async fn execute_tool(
        &self,
        request: AgentActionRequest,
        ac: &ActionContext,
    ) -> Result<ActionExecutionResult> {
        let args = request
            .input
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| request.input.clone());

        // -- Phase 1: lock registry briefly, extract manifest + inspection --
        let (manifest, normalized_name, inspection) = {
            let reg = ac.registry.lock().await;
            let normalized = normalize_tool_name(&request.target, &reg);
            let m = reg
                .list_manifests()
                .into_iter()
                .find(|m| m.name == normalized || m.id == normalized);
            let insp = reg.inspect_call_arguments(&normalized, &args);
            (m, normalized, insp)
        };

        let tool_name = normalized_name;

        // -- P9: shell.run governed execution (short locks inside) --
        if tool_name == "shell.run" {
            return self
                .execute_shell_run_short(&args, ac, &request, manifest.as_ref(), &inspection)
                .await;
        }

        // -- Phase 2: lock permission_store briefly for decision --
        let decision =
            compute_permission_decision(self.config.consume_allow_once, manifest.as_ref(), ac)
                .await;

        // -- Phase 3: blocked check --
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

        // -- Phase 4: handle blocked path --
        if blocked {
            return self
                .handle_blocked(
                    &request,
                    &args,
                    &tool_name,
                    &decision,
                    &inspection,
                    manifest.as_ref(),
                    ac,
                )
                .await;
        }

        // -- Phase 5: safe-paths check (static data) --
        if let Some(ref m) = manifest {
            if m.capabilities.contains(&"filesystem".to_string()) {
                let path = args
                    .get("path")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("");
                if !is_path_in_safe_paths(path, &ac.safe_paths) {
                    let (action, observation) = self.build_blocked_action_observation(
                        &tool_name,
                        &args,
                        &inspection,
                        &ToolPermissionDecision {
                            allowed: false,
                            requires_confirmation: false,
                            decision: "blocked".into(),
                            reason: filesystem_access_error(path, &ac.safe_paths),
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

        // -- Phase 6: execute (NO store locks held during I/O) --
        let manifest_ref = manifest
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool manifest not found for '{}'", tool_name))?;
        let result = if manifest_ref.tags.contains(&"core_os".to_string()) {
            self.execute_core_os_short(&tool_name, &args, ac)
                .await
                .unwrap_or_else(|e| ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                })
        } else if manifest_ref.tags.contains(&"execution".to_string()) {
            self.execute_execution_tool_short(&tool_name, &args, ac, &request, manifest.as_ref())
                .await
                .unwrap_or_else(|e| ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                })
        } else {
            self.call_tool_internal_async(
                manifest_ref,
                args.clone(),
                &ac.registry,
                &ac.audit_store,
                inspection.pii_found,
            )
            .await
        };

        let (mut action, observation) = self.build_success_action_observation(
            &tool_name,
            &args,
            &result,
            manifest.as_ref(),
            &request,
        );

        // For mcp.call_tool: override tool_scope (lock registry briefly)
        if tool_name == "mcp.call_tool" {
            if let Some(target_name) = args.get("tool_name").and_then(|v: &Value| v.as_str()) {
                let reg = ac.registry.lock().await;
                if let Some(target_manifest) = reg
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

    /// Short-lock shell.run: locks permission_store briefly for check,
    /// then executes shell with NO store locks held.
    /// Short-lock shell.run: locks only the stores needed for the
    /// permission/sandbox checks, then releases them before the actual
    /// shell command executes.
    async fn execute_shell_run_short(
        &self,
        args: &Value,
        ac: &ActionContext,
        request: &AgentActionRequest,
        manifest: Option<&ToolManifest>,
        inspection: &McpArgumentInspection,
    ) -> Result<ActionExecutionResult> {
        // Lock stores needed by execute_shell_run
        let reg = ac.registry.lock().await;
        let perm = ac.permission_store.lock().await;
        let audit = ac.audit_store.lock().await;
        let pe = ac.privacy_engine.lock().await;
        let bc = BorrowedActionContext {
            registry: &*reg,
            permission_store: &*perm,
            audit_store: &*audit,
            privacy_engine: &*pe,
            life_model: ac.life_model.as_ref(),
            memory_store: None,
            proposal_store: None,
            agent_run_store: None,
            event_store: ac.event_store.clone(),
            execution_sandbox: &ac.execution_sandbox,
            agent_spec: ac.agent_spec.as_ref(),
        };
        self.execute_shell_run(args, &bc, request, manifest, inspection)
    }

    /// Short-lock core_os tool: locks stores briefly around the read.
    async fn execute_core_os_short(
        &self,
        tool_name: &str,
        args: &Value,
        ac: &ActionContext,
    ) -> Result<ToolCallInternalResult> {
        let reg = ac.registry.lock().await;
        let perm = ac.permission_store.lock().await;
        let audit = ac.audit_store.lock().await;
        let pe = ac.privacy_engine.lock().await;
        let ms = match &ac.memory_store {
            Some(s) => Some(s.lock().await),
            None => None,
        };
        let ps = match &ac.proposal_store {
            Some(s) => Some(s.lock().await),
            None => None,
        };
        let bc = BorrowedActionContext {
            registry: &*reg,
            permission_store: &*perm,
            audit_store: &*audit,
            privacy_engine: &*pe,
            life_model: ac.life_model.as_ref(),
            memory_store: ms.as_deref(),
            proposal_store: ps.as_deref(),
            agent_run_store: None,
            event_store: ac.event_store.clone(),
            execution_sandbox: &ac.execution_sandbox,
            agent_spec: ac.agent_spec.as_ref(),
        };
        self.execute_core_os_tool(tool_name, args, &bc)
    }

    /// Short-lock execution tool: NO store locks held here. Each tool
    /// variant in `execute_execution_tool` does its own short-lived
    /// locking for the stores it actually needs. No `MutexGuard` is held
    /// across external I/O (file, web, A2A, MCP).
    async fn execute_execution_tool_short(
        &self,
        tool_name: &str,
        args: &Value,
        ac: &ActionContext,
        request: &AgentActionRequest,
        _manifest: Option<&ToolManifest>,
    ) -> Result<ToolCallInternalResult> {
        self.execute_execution_tool(tool_name, args, ac, request)
            .await
    }

    /// Handle the blocked execution path: create declarative-stub
    /// proposals or tool-permission proposals, lock stores briefly.
    async fn handle_blocked(
        &self,
        request: &AgentActionRequest,
        args: &Value,
        tool_name: &str,
        decision: &ToolPermissionDecision,
        inspection: &McpArgumentInspection,
        manifest: Option<&ToolManifest>,
        ac: &ActionContext,
    ) -> Result<ActionExecutionResult> {
        // Declarative stubs → create proposal (lock proposal_store briefly)
        if let Some(m) = manifest {
            if m.declarative_only {
                match tool_name {
                    "calendar.propose_event" | "email.propose_draft" => {
                        if let Some(ref ps_arc) = ac.proposal_store {
                            let proposal_type = if tool_name.ends_with("propose_event") {
                                ProposalType::ScheduledTask
                            } else {
                                ProposalType::DataExport
                            };
                            let category = if tool_name.contains("calendar") {
                                "calendar"
                            } else {
                                "email"
                            };
                            let reason_text = if tool_name.contains("calendar") {
                                "Agent proposed calendar event"
                            } else {
                                "Agent proposed email draft"
                            };
                            let ps = ps_arc.lock().await;
                            if let Some(result) = self.create_declarative_stub_proposal_with_store(
                                request,
                                &ps,
                                tool_name,
                                args,
                                proposal_type,
                                category,
                                reason_text,
                            ) {
                                return result;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let needs_confirmation = should_mark_needs_confirmation(decision, inspection);

        // Auto-generate ToolPermission Proposal (lock proposal_store briefly)
        if needs_confirmation
            && manifest
                .is_some_and(|m| !m.declarative_only && !is_proposal_generation_tool(&m.name))
        {
            if let Some(ref ps_arc) = ac.proposal_store {
                let ps = ps_arc.lock().await;
                if let Some(result) = self.create_tool_permission_proposal_with_store(
                    request, &ps, tool_name, args, manifest, decision,
                ) {
                    return result;
                }
            }
        }

        let (action, observation) = self.build_blocked_action_observation(
            tool_name, args, inspection, decision, manifest, request,
        );
        let status = if needs_confirmation {
            ActionExecutionStatus::NeedsConfirmation
        } else {
            ActionExecutionStatus::Blocked
        };

        if let (Some(ref event_store), Some(ref run_id)) =
            (ac.event_store.as_ref(), &request.source_run_id)
        {
            let event = AgentRunEvent::new(
                run_id,
                AgentRunEventType::ToolCallBlocked,
                AgentEventActor::Tool(tool_name.to_string()),
                format!("Tool '{}' blocked: {}", tool_name, decision.reason),
                serde_json::json!({
                    "tool": tool_name,
                    "reason": decision.reason,
                    "declarative_only": manifest.is_some_and(|m| m.declarative_only),
                    "needs_confirmation": needs_confirmation,
                }),
            );
            let _ = event_store.append_event(&event);
        }

        Ok(ActionExecutionResult {
            action,
            observation,
            status,
            stop_reason: Some("blocked_by_policy".into()),
        })
    }

    /// Create declarative-stub proposal using an already-locked proposal store.
    fn create_declarative_stub_proposal_with_store(
        &self,
        request: &AgentActionRequest,
        proposal_store: &crate::agent::ProposalStore,
        tool_name: &str,
        args: &Value,
        proposal_type: ProposalType,
        category: &str,
        reason: &str,
    ) -> Option<anyhow::Result<ActionExecutionResult>> {
        let after = match tool_name {
            "calendar.propose_event" => serde_json::json!({
                "title": args.get("title").and_then(Value::as_str).unwrap_or("Untitled Event"),
                "scheduled_at": args.get("scheduled_at").or_else(|| args.get("date")).and_then(Value::as_str).unwrap_or(""),
                "description": args.get("description").and_then(Value::as_str).unwrap_or(""),
                "tool": tool_name,
                "raw_args": args,
            }),
            "email.propose_draft" => serde_json::json!({
                "to": args.get("to").and_then(Value::as_str).unwrap_or(""),
                "subject": args.get("subject").and_then(Value::as_str).unwrap_or(""),
                "body": args.get("body").and_then(Value::as_str).unwrap_or(""),
                "tool": tool_name,
                "raw_args": args,
            }),
            _ => args.clone(),
        };

        let affected_path = format!("{}.{}", category, tool_name);
        let mut proposal = AgentProposal::new(
            proposal_type,
            &affected_path,
            after,
            reason,
            0.8,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        if let Some(ref run_id) = request.source_run_id {
            proposal.run_id = Some(run_id.clone());
        }

        if let Err(e) = proposal_store.create_proposal(&proposal) {
            eprintln!(
                "[warn] Failed to create {} Proposal for {}: {}",
                proposal_type, tool_name, e
            );
            return None;
        }

        let result = self.build_proposal_required_action(
            request.clone(),
            &format!(
                "{}: created {} Proposal (id: {})",
                tool_name, proposal_type, proposal.id
            ),
        );
        Some(Ok(result))
    }

    /// Create tool-permission proposal using an already-locked proposal store.
    fn create_tool_permission_proposal_with_store(
        &self,
        request: &AgentActionRequest,
        proposal_store: &crate::agent::ProposalStore,
        tool_name: &str,
        args: &Value,
        manifest: Option<&ToolManifest>,
        decision: &ToolPermissionDecision,
    ) -> Option<anyhow::Result<ActionExecutionResult>> {
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

        let mut result = self.build_proposal_required_action(
            request.clone(),
            &format!(
                "{}: 已创建 ToolPermission 提案 (id: {})，请前往 Review Center 审批",
                tool_name, proposal.id
            ),
        );
        result.action.tool_scope = manifest.map(|m| ToolActionScope {
            tool_name: m.name.clone(),
            tool_id: m.id.clone(),
            source: canonical_tool_source(m),
            risk_level: m.risk_level.clone(),
            capabilities: m.capabilities.clone(),
            action_type: m.action_type.clone(),
            requires_confirmation: true,
            allowed: false,
        });
        Some(Ok(result))
    }

    /// Async tool execution with short-lived locks: for builtins, the
    /// registry lock is only held during the sync `Fn` call; for MCP tools
    /// the `Arc<McpClient>` is cloned out of the registry, the lock is
    /// released, and the remote call happens without holding any registry
    /// nor audit lock.  No nested block_on.
    pub async fn call_tool_internal_async(
        &self,
        manifest: &ToolManifest,
        args: Value,
        registry: &Arc<tokio::sync::Mutex<McpRegistry>>,
        audit: &Arc<tokio::sync::Mutex<McpAuditStore>>,
        pii_found: bool,
    ) -> ToolCallInternalResult {
        let outcome = 'exec: {
            let reg = registry.lock().await;
            match &manifest.source {
                ToolSource::BuiltIn => {
                    let func = match reg.get_builtin_fn(&manifest.name) {
                        Some(f) => f,
                        None => {
                            break 'exec Err(anyhow::anyhow!(
                                "built-in tool '{}' not found",
                                manifest.name
                            ));
                        }
                    };
                    drop(reg);
                    break 'exec func(args.clone());
                }
                ToolSource::A2A { .. } => {
                    drop(reg);
                    break 'exec Err(anyhow::anyhow!("A2A tool execution is not wired yet"));
                }
                ToolSource::Plugin { plugin_id } => {
                    drop(reg);
                    break 'exec Err(anyhow::anyhow!(
                        "Plugin tool '{}' from '{}' is declarative-only and not executable in this Beta",
                        manifest.name,
                        plugin_id
                    ));
                }
                ToolSource::Mcp { server_name } => {
                    let client = reg.get_mcp_client(server_name);
                    drop(reg);
                    match client {
                        Some(c) => break 'exec c.call_tool(&manifest.name, args.clone()).await,
                        None => {
                            break 'exec Err(anyhow::anyhow!(
                                "MCP server '{}' not found",
                                server_name
                            ))
                        }
                    }
                }
            }
        };
        match outcome {
            Ok(r) => {
                {
                    let a = audit.lock().await;
                    if let Err(e) = a.insert_log(&manifest.name, &args, &r, true, pii_found) {
                        eprintln!("[warn] audit log write failed: {}", e);
                    }
                }
                ToolCallInternalResult {
                    success: true,
                    output: Some(r),
                    error: None,
                }
            }
            Err(e) => {
                {
                    let a = audit.lock().await;
                    if let Err(log_err) =
                        a.insert_log(&manifest.name, &args, &e.to_string(), false, pii_found)
                    {
                        eprintln!("[warn] audit log write failed: {}", log_err);
                    }
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
        ctx: &BorrowedActionContext<'_>,
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

        let mut result = self.build_proposal_required_action(
            request.clone(),
            &format!(
                "{}: 已创建 ToolPermission 提案 (id: {})，请前往 Review Center 审批",
                tool_name, proposal.id
            ),
        );
        result.action.tool_scope = manifest.map(|m| ToolActionScope {
            tool_name: m.name.clone(),
            tool_id: m.id.clone(),
            source: canonical_tool_source(m),
            risk_level: m.risk_level.clone(),
            capabilities: m.capabilities.clone(),
            action_type: m.action_type.clone(),
            requires_confirmation: true,
            allowed: false,
        });

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
        ctx: &BorrowedActionContext<'_>,
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
                return Ok(self.build_blocked_result(tool_name, args, request, reason, false, None));
            }
        };

        if !manifest.enabled {
            let reason = "shell.run is disabled in manifest";
            record_blocked(reason, serde_json::json!({"reason": reason}));
            return Ok(self.build_blocked_result(
                tool_name,
                args,
                request,
                reason,
                false,
                Some(manifest),
            ));
        }

        if manifest.declarative_only {
            let reason = "shell.run is declarative-only (no executor available)";
            record_blocked(reason, serde_json::json!({"reason": reason}));
            return Ok(self.build_blocked_result(
                tool_name,
                args,
                request,
                reason,
                false,
                Some(manifest),
            ));
        }

        // ── 2. Sandbox check ───────────────────────────────────────────
        let sandbox = ctx.execution_sandbox;
        if !sandbox.bash_enabled {
            let reason = "shell execution is disabled (sandbox.bash_enabled = false)";
            record_blocked(
                reason,
                serde_json::json!({
                    "reason": reason,
                    "bash_enabled": false,
                }),
            );
            return Ok(self.build_blocked_result(
                tool_name,
                args,
                request,
                reason,
                false,
                Some(manifest),
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
                record_blocked(
                    reason,
                    serde_json::json!({
                        "reason": reason,
                        "agent_spec_id": ctx.agent_spec.map(|s| s.id.clone()),
                    }),
                );
                return Ok(self.build_blocked_result(
                    tool_name,
                    args,
                    request,
                    reason,
                    false,
                    Some(manifest),
                ));
            }
            None => {
                let reason =
                    "AgentSpec missing: cannot execute shell.run without governed AgentSpec";
                record_blocked(
                    reason,
                    serde_json::json!({
                        "reason": reason,
                    }),
                );
                return Ok(self.build_blocked_result(
                    tool_name,
                    args,
                    request,
                    reason,
                    false,
                    Some(manifest),
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

            record_blocked(
                reason,
                serde_json::json!({
                    "reason": reason,
                    "needs_confirmation": needs_confirmation,
                    "permission_decision": perm_decision.decision,
                }),
            );

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
        let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
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
                        format!(
                            "shell.run timed out after {} ms: {}",
                            output.elapsed_ms, command
                        )
                    } else {
                        format!(
                            "shell.run completed: {} (exit={})",
                            command, output.exit_code
                        )
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
                (ActionExecutionStatus::Failed, None, Some(err_str))
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

    #[tokio::test]
    async fn test_shell_run_disabled_sandbox_records_blocked() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_tool.db"));
        let pe = PrivacyEngine::new();
        let sandbox = crate::agent::execution_sandbox::ExecutionSandbox::always_disabled();
        let event_store = crate::agent::event_store::AgentRunEventStore::new_in_memory().unwrap();
        let mut ctx = crate::agent::ActionContext::new_for_test(reg, ps, audit, pe, vec![])
            .with_execution_sandbox(sandbox);
        ctx.event_store = Some(event_store.clone());

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "shell.run".into(),
            input: serde_json::json!({"arguments": {"command": "echo", "args": ["hello"]}}),
            source_run_id: Some("test-run-1".into()),
            step_index: 0,
        };
        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert!(result.action.error.unwrap_or_default().contains("disabled"));

        let events = event_store.list_events_by_run("test-run-1").unwrap();
        let has_blocked = events.iter().any(|e| {
            matches!(
                e.event_type,
                crate::agent::AgentRunEventType::ToolCallBlocked
            )
        });
        assert!(
            has_blocked,
            "blocked event must be recorded by ActionExecutor"
        );
    }

    #[tokio::test]
    async fn test_shell_run_manifest_disabled_blocks_in_action_executor() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_tool2.db"));
        let pe = PrivacyEngine::new();
        // Shell enabled sandbox, but manifest is disabled by default
        let tmp = std::env::temp_dir().to_string_lossy().to_string();
        let _sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
            bash_enabled: true,
            cwd: tmp.clone(),
            safe_paths: vec![tmp],
            command_allowlist: vec!["echo".into()],
            ..crate::agent::execution_sandbox::ExecutionSandbox::default()
        };
        let ctx = crate::agent::ActionContext::new_for_test(reg, ps, audit, pe, vec![]);

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "shell.run".into(),
            input: serde_json::json!({"arguments": {"command": "echo", "args": ["test"]}}),
            source_run_id: None,
            step_index: 0,
        };
        let result = executor.execute(request, &ctx).await.unwrap();
        // Manifest is disabled → blocked at the manifest check
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert!(result.action.error.unwrap_or_default().contains("disabled"));
    }

    #[tokio::test]
    async fn test_tool_permission_proposal_action_keeps_tool_scope_for_replay() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "web.search",
            "builtin",
            "medium",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AskEveryTime,
            None,
        )
        .unwrap();
        let audit = McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_tool3.db"));
        let pe = PrivacyEngine::new();
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let ctx = crate::agent::ActionContext::new_for_test(reg, ps, audit, pe, vec![])
            .with_proposal_store(std::sync::Arc::new(tokio::sync::Mutex::new(proposal_store)));

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "mcp_tool".into(),
            target: "web.search".into(),
            input: serde_json::json!({"arguments": {"query": "重庆万象城"}}),
            source_run_id: Some("test-run-2".into()),
            step_index: 0,
        };
        let result = executor.execute(request, &ctx).await.unwrap();

        assert_eq!(result.status, ActionExecutionStatus::NeedsConfirmation);
        let scope = result
            .action
            .tool_scope
            .expect("proposal-required action must retain tool_scope for replay");
        assert_eq!(scope.tool_name, "web.search");
        assert_eq!(scope.source, "builtin");
        assert_eq!(scope.risk_level, "medium");
        assert_eq!(scope.action_type, "read");
    }

    // ── deadlock + long-lock regression tests ─────────────────────

    /// Verify that executing a builtin tool completes within a short
    /// timeout — no self-deadlock from double-locking the registry.
    #[tokio::test]
    async fn test_builtin_tool_execute_does_not_deadlock() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "life_model.read",
            "builtin",
            "low",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        let audit = McpAuditStore::new(
            tempfile::tempdir()
                .unwrap()
                .path()
                .join("audit_deadlock.db"),
        );
        let pe = PrivacyEngine::new();
        let life_model = crate::life_model::LifeModel::default_model();
        let mut ctx = crate::agent::ActionContext::new_for_test(reg, ps, audit, pe, vec![])
            .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());
        ctx.life_model = Some(life_model);

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "life_model.read".into(),
            input: serde_json::json!({"arguments": {}}),
            source_run_id: Some("test-run-dl".into()),
            step_index: 0,
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            executor.execute(request, &ctx),
        )
        .await;

        assert!(
            result.is_ok(),
            "execute() timed out after 3s — likely self-deadlock"
        );
        let exec_result = result.unwrap().unwrap();
        assert_eq!(exec_result.status, ActionExecutionStatus::Succeeded);
    }

    /// Verify that during a tool execution, other stores remain
    /// accessible — the tool execution does not hold unrelated locks.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_long_tool_does_not_block_permission_store() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        let ps = Arc::new(tokio::sync::Mutex::new(
            ToolPermissionStore::new_in_memory().unwrap(),
        ));
        ps.lock()
            .await
            .grant(
                "file.read",
                "builtin",
                "low",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        let audit = McpAuditStore::new(
            tempfile::tempdir()
                .unwrap()
                .path()
                .join("audit_longlock.db"),
        );
        let pe = PrivacyEngine::new();

        let safe_paths = vec!["/tmp".to_string()];
        let mut ctx = crate::agent::ActionContext::new_for_test(
            reg,
            ToolPermissionStore::new_in_memory().unwrap(),
            audit,
            pe,
            safe_paths,
        )
        .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());
        ctx.permission_store = ps.clone();

        let tmp_path = std::env::temp_dir().join("openlife_lock_test.txt");
        std::fs::write(&tmp_path, b"test content").unwrap();
        let path_str = tmp_path.to_string_lossy().to_string();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "file.read".into(),
            input: serde_json::json!({"arguments": {"path": path_str}}),
            source_run_id: Some("test-run-ll".into()),
            step_index: 0,
        };

        let exec_handle = tokio::spawn(async move { executor.execute(request, &ctx).await });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let perm_result = tokio::time::timeout(std::time::Duration::from_secs(2), ps.lock()).await;

        assert!(
            perm_result.is_ok(),
            "permission_store lock timed out — blocked by tool execution"
        );
        drop(perm_result); // release guard before waiting for exec_handle

        let exec_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), exec_handle).await;

        assert!(exec_result.is_ok(), "tool execution timed out");

        let _ = std::fs::remove_file(&tmp_path);
    }

    // ── mcp.call_tool deadlock + lock-safety regression tests ────────

    /// Test A: mcp.call_tool targeting builtin_echo must not self-deadlock.
    /// Wraps execution in a 3-second timeout to catch any lock ordering bugs.
    #[tokio::test]
    async fn test_mcp_call_tool_does_not_deadlock() {
        let mut reg = McpRegistry::new();
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        // Grant mcp.call_tool
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        // Grant target builtin_echo
        ps.grant(
            "builtin_echo",
            "builtin",
            "low",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        let audit = McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_mcp_dl.db"));
        let pe = PrivacyEngine::new();
        let ctx = crate::agent::ActionContext::new_for_test(reg, ps, audit, pe, vec![])
            .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "builtin_echo",
                    "arguments": {"text": "hello"}
                }
            }),
            source_run_id: Some("test-run-mcp-dl".into()),
            step_index: 0,
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            executor.execute(request, &ctx),
        )
        .await;

        assert!(
            result.is_ok(),
            "mcp.call_tool timed out after 3s — likely self-deadlock"
        );
        let exec_result = result.unwrap().unwrap();
        assert!(
            matches!(exec_result.status, ActionExecutionStatus::Succeeded),
            "mcp.call_tool should succeed: {:?}",
            exec_result.observation.content
        );
    }

    /// Test B: During mcp.call_tool execution of a blocking target,
    /// `permission_store` must remain accessible — no long-lived
    /// `MutexGuard` leaked from the outer execution path.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_mcp_call_tool_does_not_hold_permission_store() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();

        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let rx = Arc::new(std::sync::Mutex::new(rx));

        // Register a custom blocking builtin that waits on a channel
        let blocker_manifest = crate::tool_manifest::ToolManifest {
            id: "test_blocker".into(),
            name: "test_blocker".into(),
            description: "Blocks until signalled (test utility)".into(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: crate::tool_manifest::ToolSource::BuiltIn,
            capabilities: vec![],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };
        let rx_clone = rx.clone();
        reg.register_builtin(
            blocker_manifest,
            Arc::new(move |_args| {
                let rx = rx_clone.lock().unwrap();
                let _ = rx.recv_timeout(std::time::Duration::from_secs(10));
                drop(rx);
                Ok("blocking call completed".to_string())
            }),
        );

        let ps = Arc::new(tokio::sync::Mutex::new(
            ToolPermissionStore::new_in_memory().unwrap(),
        ));
        ps.lock()
            .await
            .grant(
                "mcp.call_tool",
                "builtin",
                "medium",
                "external_side_effect",
                crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        ps.lock()
            .await
            .grant(
                "test_blocker",
                "builtin",
                "low",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        let audit = McpAuditStore::new(
            tempfile::tempdir()
                .unwrap()
                .path()
                .join("audit_mcp_lock.db"),
        );
        let pe = PrivacyEngine::new();
        let mut ctx = crate::agent::ActionContext::new_for_test(
            reg,
            ToolPermissionStore::new_in_memory().unwrap(),
            audit,
            pe,
            vec![],
        )
        .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());
        ctx.permission_store = ps.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "test_blocker",
                    "arguments": {}
                }
            }),
            source_run_id: Some("test-run-mcp-lock".into()),
            step_index: 0,
        };

        let exec_handle = tokio::spawn(async move { executor.execute(request, &ctx).await });
        // Give the executor time to enter the blocking tool
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // permission_store must be accessible within a short timeout
        let perm_result =
            tokio::time::timeout(std::time::Duration::from_millis(500), ps.lock()).await;
        assert!(
            perm_result.is_ok(),
            "permission_store lock timed out — held across tool execution"
        );

        // Signal the blocking tool to complete
        drop(perm_result); // drop the lock guard
        let _ = tx.send(());

        let exec_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), exec_handle).await;
        assert!(exec_result.is_ok(), "mcp.call_tool execution timed out");
    }

    /// Test C: `call_tool_internal_async` with an Mcp manifest must not
    /// hold the registry lock across the (failed) remote call await.
    /// Uses a non-existent server to trigger an error without deadlock.
    #[tokio::test]
    async fn test_call_tool_internal_async_mcp_does_not_hold_registry() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        let audit = McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_cia.db"));
        let registry = Arc::new(tokio::sync::Mutex::new(reg));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));

        // Construct an MCP-tagged manifest pointing to a non-existent server
        let mcp_manifest = crate::tool_manifest::ToolManifest {
            id: "mcp:ghost:ghost_tool".into(),
            name: "ghost_tool".into(),
            description: "Phantom MCP tool for lock test".into(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: crate::tool_manifest::ToolSource::Mcp {
                server_name: "ghost_server".into(),
            },
            capabilities: vec![],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            executor.call_tool_internal_async(
                &mcp_manifest,
                serde_json::json!({}),
                &registry,
                &audit_arc,
                false,
            ),
        )
        .await;

        assert!(
            result.is_ok(),
            "call_tool_internal_async for MCP timed out — likely registry deadlock"
        );
        let r = result.unwrap();
        assert!(!r.success, "expected error for non-existent MCP server");
        assert!(
            r.error.as_ref().is_some_and(|e| e.contains("ghost_server")),
            "error should mention the missing server: {:?}",
            r.error
        );

        // Also verify registry is lockable after the call returns
        let reg_lock =
            tokio::time::timeout(std::time::Duration::from_millis(500), registry.lock()).await;
        assert!(
            reg_lock.is_ok(),
            "registry lock couldn't be acquired after call_tool_internal_async — held across await?"
        );
    }

    /// Test D: During mcp.call_tool execution of a blocking BuiltIn target,
    /// `registry` must remain accessible — the BuiltIn function is invoked
    /// without holding the registry MutexGuard.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_mcp_call_tool_does_not_hold_registry_for_builtin_target() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();

        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let rx = Arc::new(std::sync::Mutex::new(rx));

        // Register a custom blocking builtin that waits on a channel
        let blocker_manifest = crate::tool_manifest::ToolManifest {
            id: "test_blocker2".into(),
            name: "test_blocker2".into(),
            description: "Blocks until signalled (test utility)".into(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: crate::tool_manifest::ToolSource::BuiltIn,
            capabilities: vec![],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };
        let rx_clone = rx.clone();
        reg.register_builtin(
            blocker_manifest,
            Arc::new(move |_args| {
                let rx = rx_clone.lock().unwrap();
                let _ = rx.recv_timeout(std::time::Duration::from_secs(10));
                drop(rx);
                Ok("blocking call completed".to_string())
            }),
        );

        let ps = Arc::new(tokio::sync::Mutex::new(
            ToolPermissionStore::new_in_memory().unwrap(),
        ));
        ps.lock()
            .await
            .grant(
                "mcp.call_tool",
                "builtin",
                "medium",
                "external_side_effect",
                crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        ps.lock()
            .await
            .grant(
                "test_blocker2",
                "builtin",
                "low",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        let audit =
            McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_mcp_reg.db"));
        let pe = PrivacyEngine::new();

        let registry_arc = Arc::new(tokio::sync::Mutex::new(reg));
        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            audit,
            pe,
            vec![],
        )
        .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());
        ctx.registry = registry_arc.clone();
        ctx.permission_store = ps.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "test_blocker2",
                    "arguments": {}
                }
            }),
            source_run_id: Some("test-run-mcp-reg".into()),
            step_index: 0,
        };

        let exec_handle = tokio::spawn(async move { executor.execute(request, &ctx).await });
        // Give the executor time to enter the blocking tool
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Registry must be accessible within a short timeout — proves
        // call_tool_internal_async no longer holds the registry
        // MutexGuard during BuiltIn function execution.
        let reg_result =
            tokio::time::timeout(std::time::Duration::from_millis(500), registry_arc.lock()).await;
        assert!(
            reg_result.is_ok(),
            "registry lock timed out — held across builtin tool execution"
        );
        drop(reg_result);

        // Signal the blocking tool to complete
        let _ = tx.send(());

        let exec_result =
            tokio::time::timeout(std::time::Duration::from_secs(5), exec_handle).await;
        assert!(exec_result.is_ok(), "mcp.call_tool execution timed out");
    }
}
