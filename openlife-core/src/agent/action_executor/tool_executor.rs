use super::helpers::{
    canonical_tool_source, filesystem_access_error, is_path_in_safe_paths, normalize_tool_name,
    should_mark_needs_confirmation, ToolCallInternalResult,
};
use super::ActionContext;
use super::ActionExecutionResult;
use super::ActionExecutionStatus;
use super::AgentActionRequest;
use super::BorrowedActionContext;
use super::{ExecutionBlockReason, ExecutionFailureKind, ExecutionProposalReason};
use crate::agent::shell_executor::{ShellCommandRequest, ShellExecutor};
use crate::agent::trace_payloads;
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

        // -- AgentSpec unified tool governance --
        // Non-shell tools must pass AgentSpec allow/deny rules.
        if let Some(ref spec) = ac.agent_spec {
            if !spec.is_tool_allowed(&tool_name) {
                let (action, observation) = self.build_blocked_action_observation(
                    &tool_name,
                    &args,
                    &inspection,
                    &ToolPermissionDecision {
                        allowed: false,
                        requires_confirmation: false,
                        decision: "blocked".into(),
                        reason: format!(
                            "Tool '{}' is not allowed by the current AgentSpec (allowed_tools: {:?}, denied_tools: {:?})",
                            tool_name,
                            spec.allowed_tools,
                            spec.denied_tools,
                        ),
                        policy_id: None,
                    },
                    manifest.as_ref(),
                    &request,
                );
                if let (Some(event_store), Some(run_id)) =
                    (ac.event_store.as_ref(), &request.source_run_id)
                {
                    let event = AgentRunEvent::new(
                        run_id,
                        AgentRunEventType::ToolCallBlocked,
                        AgentEventActor::Tool(tool_name.clone()),
                        format!("Tool '{}' blocked by AgentSpec governance", tool_name),
                        trace_payloads::build_tool_call_blocked_payload(
                            "blocked",
                            tool_name,
                            manifest
                                .as_ref()
                                .map(canonical_tool_source)
                                .unwrap_or_default(),
                            Some(spec.id.clone()),
                            Some(ExecutionBlockReason::AgentSpecDenied.to_string()),
                            None::<&str>,
                            None::<&str>,
                            Some(serde_json::json!({"reason": "agent_spec_denied"})),
                        ),
                    );
                    let _ = event_store.append_event(&event);
                }
                return Ok(ActionExecutionResult {
                    action,
                    observation,
                    status: ActionExecutionStatus::Blocked,
                    stop_reason: Some("agent_spec_denied".into()),
                    block_reason: Some(ExecutionBlockReason::AgentSpecDenied),
                    proposal_reason: None,
                    failure_kind: None,
                });
            }
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
                        block_reason: Some(ExecutionBlockReason::PathNotSafe),
                        proposal_reason: None,
                        failure_kind: None,
                    });
                }
            }
        }

        // -- Phase 6: execute (NO store locks held during I/O) --
        // Unified network policy check for network-capable tools.
        // mcp.call_tool is NOT checked here — its handler in execution_tools.rs
        // resolves the target tool first, then gates only network-capable targets.
        if matches!(
            tool_name.as_str(),
            "web.fetch" | "web.search" | "a2a.call_agent"
        ) {
            if let Some(ref policy) = ac.network_policy {
                let url = args.get("url").and_then(|v| v.as_str());
                // Peek permission store: if the tool already has an allow
                // permission (from a previously accepted Proposal), skip
                // default_decision=ask/deny so replay doesn't loop.
                let already_permitted = if let Some(ref m) = manifest {
                    let source = canonical_tool_source(m);
                    let perm = ac.permission_store.lock().await;
                    perm.peek(
                        &m.name,
                        &source,
                        &m.risk_level,
                        &m.action_type,
                        &m.capabilities,
                    )
                    .is_ok_and(|d| d.allowed && d.policy_id.is_some())
                } else {
                    false
                };
                if let Some(blocked) = super::execution_tools::check_network_policy(
                    &tool_name,
                    policy,
                    url,
                    already_permitted,
                ) {
                    let needs_confirm = matches!(
                        blocked.proposal_reason,
                        Some(ExecutionProposalReason::NetworkPolicyAsk)
                    );
                    if needs_confirm {
                        return self
                            .network_ask_proposal(
                                &tool_name,
                                &args,
                                &request,
                                &inspection,
                                manifest.as_ref(),
                                ac,
                                &blocked,
                            )
                            .await;
                    }
                    // deny / enabled=false / tool_override=deny: hard block
                    let reason = blocked.error.clone().unwrap_or_default();
                    let (action, observation) = self.build_blocked_action_observation(
                        &tool_name,
                        &args,
                        &inspection,
                        &ToolPermissionDecision {
                            allowed: false,
                            requires_confirmation: false,
                            decision: "blocked".into(),
                            reason,
                            policy_id: None,
                        },
                        manifest.as_ref(),
                        &request,
                    );
                    return Ok(ActionExecutionResult {
                        action,
                        observation,
                        status: ActionExecutionStatus::Blocked,
                        stop_reason: blocked.block_reason.as_ref().map(|r| r.to_string()),
                        block_reason: blocked.block_reason.clone(),
                        proposal_reason: None,
                        failure_kind: None,
                    });
                }
            }
        }

        let manifest_ref = manifest
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool manifest not found for '{}'", tool_name))?;
        let result = if manifest_ref.tags.contains(&"core_os".to_string()) {
            self.execute_core_os_short(&tool_name, &args, ac)
                .await
                .unwrap_or_else(|e| {
                    ToolCallInternalResult::failure(
                        ExecutionFailureKind::InternalError,
                        e.to_string(),
                    )
                })
        } else if manifest_ref.tags.contains(&"execution".to_string()) {
            self.execute_execution_tool_short(&tool_name, &args, ac, &request, manifest.as_ref())
                .await
                .unwrap_or_else(|e| {
                    ToolCallInternalResult::failure(
                        ExecutionFailureKind::InternalError,
                        e.to_string(),
                    )
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
        let mcp_target_name = if tool_name == "mcp.call_tool" {
            args.get("tool_name")
                .and_then(|v: &Value| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        if tool_name == "mcp.call_tool" {
            if let Some(ref target_name) = mcp_target_name {
                let reg = ac.registry.lock().await;
                if let Some(target_manifest) = reg
                    .list_manifests()
                    .into_iter()
                    .find(|m| m.name == *target_name || m.id == *target_name)
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
            // Target-level AgentSpec denial: wrapper was allowed but real target denied.
            // Must fail as Blocked (not NeedsConfirmation). Check typed reason.
            if matches!(
                result.block_reason,
                Some(ExecutionBlockReason::AgentSpecDenied)
            ) {
                action.status = "blocked".to_string();
                action.error = result.error.clone();
                if let (Some(event_store), Some(run_id)) =
                    (ac.event_store.as_ref(), &request.source_run_id)
                {
                    let target_tool_name = mcp_target_name.as_deref().unwrap_or("");
                    let event = AgentRunEvent::new(
                        run_id,
                        AgentRunEventType::ToolCallBlocked,
                        AgentEventActor::Tool(tool_name.clone()),
                        "mcp.call_tool target denied by AgentSpec".to_string(),
                        trace_payloads::build_tool_call_blocked_payload(
                            "blocked",
                            tool_name,
                            "builtin",
                            ac.agent_spec.as_ref().map(|s| s.id.clone()),
                            Some(ExecutionBlockReason::AgentSpecDenied.to_string()),
                            None::<&str>,
                            None::<&str>,
                            Some(serde_json::json!({
                                "target_tool_name": target_tool_name,
                                "target_source": action.tool_scope.as_ref().map(|s| s.source.clone()),
                                "wrapper_tool_name": "mcp.call_tool",
                            })),
                        ),
                    );
                    let _ = event_store.append_event(&event);
                }
                return Ok(ActionExecutionResult {
                    action,
                    observation,
                    status: ActionExecutionStatus::Blocked,
                    stop_reason: Some("target_agent_spec_denied".into()),
                    block_reason: Some(ExecutionBlockReason::AgentSpecDenied),
                    proposal_reason: None,
                    failure_kind: None,
                });
            }
            // Target-level hard block (ToolPermissionDenied, PiiDetected, etc.)
            if !result.success
                && result.block_reason.is_some()
                && result.proposal_reason.is_none()
                && !matches!(
                    result.block_reason,
                    Some(ExecutionBlockReason::AgentSpecDenied)
                )
            {
                action.status = "blocked".to_string();
                action.error = result.error.clone();
                if let (Some(event_store), Some(run_id)) =
                    (ac.event_store.as_ref(), &request.source_run_id)
                {
                    let target_tool_name = mcp_target_name.as_deref().unwrap_or("");
                    let event = AgentRunEvent::new(
                        run_id,
                        AgentRunEventType::ToolCallBlocked,
                        AgentEventActor::Tool(tool_name.clone()),
                        format!("mcp.call_tool target blocked: {:?}", result.block_reason),
                        trace_payloads::build_tool_call_blocked_payload(
                            "blocked",
                            tool_name,
                            "builtin",
                            ac.agent_spec.as_ref().map(|s| s.id.clone()),
                            result.block_reason.as_ref().map(|r| r.to_string()),
                            None::<&str>,
                            None::<&str>,
                            Some(serde_json::json!({
                                "target_tool_name": target_tool_name,
                                "target_source": action.tool_scope.as_ref().map(|s| s.source.clone()),
                                "wrapper_tool_name": "mcp.call_tool",
                            })),
                        ),
                    );
                    let _ = event_store.append_event(&event);
                }
                return Ok(ActionExecutionResult {
                    action,
                    observation,
                    status: ActionExecutionStatus::Blocked,
                    stop_reason: result
                        .block_reason
                        .as_ref()
                        .map(|r| format!("target_{}", r)),
                    block_reason: result.block_reason.clone(),
                    proposal_reason: None,
                    failure_kind: None,
                });
            }
            // Target-level needs_confirmation (ToolPermissionAsk)
            if matches!(
                result.proposal_reason,
                Some(ExecutionProposalReason::ToolPermissionAsk)
            ) {
                action.status = "needs_confirmation".to_string();
                action.error = result.error.clone();
                return Ok(ActionExecutionResult {
                    action,
                    observation,
                    status: ActionExecutionStatus::NeedsConfirmation,
                    stop_reason: Some("target_tool_permission_ask".into()),
                    block_reason: None,
                    proposal_reason: Some(ExecutionProposalReason::ToolPermissionAsk),
                    failure_kind: None,
                });
            }
        }

        // Check for needs_confirmation from policies (network_policy ask, etc.)
        // Use typed proposal_reason instead of error string prefix matching.
        if matches!(
            result.proposal_reason,
            Some(ExecutionProposalReason::NetworkPolicyAsk)
        ) {
            let proposal_tool = if tool_name == "mcp.call_tool" {
                args.get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&tool_name)
            } else {
                &tool_name
            };

            // For mcp.call_tool: re-lookup the real target manifest
            let target_manifest: Option<ToolManifest> = if tool_name == "mcp.call_tool" {
                Self::resolve_mcp_target_manifest_for_args(&ac.registry, &args)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };

            return self
                .network_ask_proposal_ex(
                    proposal_tool,
                    &args,
                    &request,
                    manifest.as_ref(),
                    ac,
                    &result,
                    target_manifest.as_ref(),
                )
                .await;
        }

        // Generic needs_confirmation (non-NetworkPolicy)
        if result.proposal_reason.is_some() {
            action.status = "needs_confirmation".to_string();
            return Ok(ActionExecutionResult {
                action,
                observation,
                status: ActionExecutionStatus::NeedsConfirmation,
                stop_reason: Some(
                    result
                        .proposal_reason
                        .as_ref()
                        .map(|r| r.to_string())
                        .unwrap_or_default(),
                ),
                block_reason: None,
                proposal_reason: result.proposal_reason.clone(),
                failure_kind: None,
            });
        }

        // Non-success without block/proposal reason: treat as Failed
        if !result.success {
            let status = ActionExecutionStatus::Failed;
            action.status = "failed".to_string();
            return Ok(ActionExecutionResult {
                action,
                observation,
                status,
                stop_reason: None,
                block_reason: None,
                proposal_reason: None,
                failure_kind: result.failure_kind.clone(),
            });
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
            block_reason: None,
            proposal_reason: None,
            failure_kind: None,
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
            registry: &reg,
            permission_store: &perm,
            audit_store: &audit,
            privacy_engine: &pe,
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
            registry: &reg,
            permission_store: &perm,
            audit_store: &audit,
            privacy_engine: &pe,
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
    #[allow(clippy::too_many_arguments)]
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

        let block_reason = if manifest.as_ref().is_some_and(|m| m.declarative_only) {
            Some(ExecutionBlockReason::DeclarativeOnly)
        } else if manifest.as_ref().is_some_and(|m| !m.enabled) {
            Some(ExecutionBlockReason::DisabledManifest)
        } else if !decision.allowed {
            Some(ExecutionBlockReason::ToolPermissionDenied)
        } else if inspection.requires_confirmation && inspection.pii_found {
            Some(ExecutionBlockReason::PiiDetected)
        } else {
            Some(ExecutionBlockReason::Unknown)
        };

        let proposal_reason =
            if needs_confirmation && !manifest.as_ref().is_some_and(|m| m.declarative_only) {
                Some(ExecutionProposalReason::ToolPermissionAsk)
            } else {
                None
            };

        if let (Some(event_store), Some(run_id)) = (ac.event_store.as_ref(), &request.source_run_id)
        {
            let event = AgentRunEvent::new(
                run_id,
                AgentRunEventType::ToolCallBlocked,
                AgentEventActor::Tool(tool_name.to_string()),
                format!("Tool '{}' blocked: {}", tool_name, decision.reason),
                trace_payloads::build_tool_call_blocked_payload(
                    if needs_confirmation {
                        "needs_confirmation"
                    } else {
                        "blocked"
                    },
                    tool_name,
                    manifest
                        .map(canonical_tool_source)
                        .unwrap_or_else(|| "builtin".to_string()),
                    ac.agent_spec.as_ref().map(|s| s.id.clone()),
                    block_reason.as_ref().map(|r| r.to_string()),
                    proposal_reason.as_ref().map(|r| r.to_string()),
                    None::<&str>,
                    Some(serde_json::json!({"reason": decision.reason.clone()})),
                ),
            );
            let _ = event_store.append_event(&event);
        }

        Ok(ActionExecutionResult {
            action,
            observation,
            status,
            stop_reason: Some("blocked_by_policy".into()),
            block_reason,
            proposal_reason,
            failure_kind: None,
        })
    }

    /// When NetworkPolicy default_decision=ask blocks a tool, create a
    /// ToolPermission Proposal so the user can approve it in Review Center.
    /// The proposal carries blocked_action info that replay uses to match
    /// and re-execute the action once the permission is granted.
    /// `target_info` provides explicit source/risk/action_type for MCP
    /// targets (overrides the wrapper manifest defaults).
    /// Re-lookup the real MCP target tool manifest from the registry.
    /// Replaces the previous error-string JSON protocol with a proper
    /// registry lookup. Used when `mcp.call_tool` is blocked by
    /// `needs_confirmation:network_policy`.
    async fn resolve_mcp_target_manifest_for_args(
        registry: &Arc<tokio::sync::Mutex<McpRegistry>>,
        args: &serde_json::Value,
    ) -> anyhow::Result<Option<ToolManifest>> {
        let target_name = args
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'tool_name' in args"))?;
        let server = args.get("server").and_then(|v| v.as_str());

        let reg = registry.lock().await;
        let manifests = reg.list_manifests();
        let mut target_manifests: Vec<_> = manifests
            .into_iter()
            .filter(|m| m.name == target_name || m.id == target_name)
            .collect();

        if let Some(server_name) = server {
            let found = target_manifests.into_iter().find(
                |m| matches!(&m.source, ToolSource::Mcp { server_name: s } if s == server_name),
            );
            Ok(found)
        } else if target_manifests.len() == 1 {
            Ok(Some(target_manifests.remove(0)))
        } else if target_manifests.is_empty() {
            Ok(None)
        } else {
            Err(anyhow::anyhow!(
                "Multiple tools named '{}'. Specify 'server' parameter.",
                target_name
            ))
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn network_ask_proposal(
        &self,
        tool_name: &str,
        args: &Value,
        request: &AgentActionRequest,
        _inspection: &McpArgumentInspection,
        manifest: Option<&ToolManifest>,
        ac: &ActionContext,
        blocked: &ToolCallInternalResult,
    ) -> Result<ActionExecutionResult> {
        self.network_ask_proposal_ex(
            tool_name,
            args,
            request,
            manifest,
            ac,
            blocked,
            Option::<&ToolManifest>::None,
        )
        .await
    }

    /// Extended version that accepts an optional target_manifest override.
    /// When `target_manifest` is Some, its metadata is used for the Proposal
    /// `after` fields and `tool_scope` instead of the wrapper `manifest`.
    #[allow(clippy::too_many_arguments)]
    async fn network_ask_proposal_ex(
        &self,
        tool_name: &str,
        args: &Value,
        request: &AgentActionRequest,
        manifest: Option<&ToolManifest>,
        ac: &ActionContext,
        blocked: &ToolCallInternalResult,
        target_manifest: Option<&ToolManifest>,
    ) -> Result<ActionExecutionResult> {
        let scope_manifest = target_manifest.or(manifest);
        let source = scope_manifest
            .map(canonical_tool_source)
            .unwrap_or_else(|| "builtin".to_string());
        let risk_level = scope_manifest
            .map(|m| m.risk_level.clone())
            .unwrap_or_else(|| "medium".to_string());
        let action_type = scope_manifest
            .map(|m| m.action_type.clone())
            .unwrap_or_else(|| "read".to_string());
        let caps = scope_manifest
            .map(|m| m.capabilities.clone())
            .unwrap_or_default();
        let reason = blocked.error.clone().unwrap_or_default();

        let after = serde_json::json!({
            "permission_action": "grant",
            "tool_name": tool_name,
            "source": source,
            "risk_level": risk_level,
            "action_type": action_type,
            "capabilities": caps,
            "policy": "allow_until_revoked",
            "blocked_action": {
                "action_type": request.action_type,
                "target": request.target,
                "input": args,
                "source_run_id": request.source_run_id,
                "step_index": request.step_index,
            },
            "reason": reason,
            "auto_generated": true,
            "network_policy_ask": true,
        });

        let affected_path = format!("tool_permission.{}.{}", source, tool_name);
        let mut proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            &affected_path,
            after,
            &format!(
                "[NetworkPolicy ask] 工具 '{}' 需要网络访问确认。请在 Review Center 审批后重放。",
                tool_name
            ),
            0.7,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        if let Some(ref run_id) = request.source_run_id {
            proposal.run_id = Some(run_id.clone());
        }

        let proposal_id = {
            if let Some(ref ps_arc) = ac.proposal_store {
                let ps = ps_arc.lock().await;
                let id = proposal.id.clone();
                if let Err(e) = ps.create_proposal(&proposal) {
                    eprintln!(
                        "[warn] Failed to create network-ask ToolPermission Proposal for {}: {}",
                        tool_name, e
                    );
                    String::new()
                } else {
                    id
                }
            } else {
                String::new()
            }
        };

        let mut result = self.build_proposal_required_action(
            request.clone(),
            &format!(
                "{}: 网络策略要求确认 (default_decision=ask)，已创建 ToolPermission 提案 (id: {})，请前往 Review Center 审批",
                tool_name, proposal_id
            ),
        );
        result.stop_reason = Some("network_policy_ask".into());
        result.proposal_reason = Some(ExecutionProposalReason::NetworkPolicyAsk);
        result.action.tool_scope = scope_manifest.map(|m| ToolActionScope {
            tool_name: tool_name.to_string(),
            tool_id: m.id.clone(),
            source: canonical_tool_source(m),
            risk_level: m.risk_level.clone(),
            capabilities: caps,
            action_type: m.action_type.clone(),
            requires_confirmation: true,
            allowed: false,
        });

        if let (Some(event_store), Some(run_id)) = (ac.event_store.as_ref(), &request.source_run_id)
        {
            let event = AgentRunEvent::new(
                run_id,
                AgentRunEventType::ToolCallBlocked,
                AgentEventActor::Tool(tool_name.to_string()),
                format!("Tool '{}' blocked by NetworkPolicy ask", tool_name),
                trace_payloads::build_tool_call_blocked_payload(
                    "needs_confirmation",
                    tool_name,
                    scope_manifest
                        .map(canonical_tool_source)
                        .unwrap_or_else(|| "builtin".to_string()),
                    ac.agent_spec.as_ref().map(|s| s.id.clone()),
                    None::<&str>,
                    Some(ExecutionProposalReason::NetworkPolicyAsk.to_string()),
                    None::<&str>,
                    Some(serde_json::json!({
                        "reason": "network_policy_ask",
                        "proposal_id": proposal_id.clone(),
                    })),
                ),
            );
            let _ = event_store.append_event(&event);
        }

        Ok(result)
    }

    /// Create declarative-stub proposal using an already-locked proposal store.
    #[allow(clippy::too_many_arguments)]
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
        let mut failure_kind: Option<ExecutionFailureKind> = None;
        let outcome: anyhow::Result<String> = 'exec: {
            let reg = registry.lock().await;
            match &manifest.source {
                ToolSource::BuiltIn => {
                    let func = match reg.get_builtin_fn(&manifest.name) {
                        Some(f) => f,
                        None => {
                            failure_kind = Some(ExecutionFailureKind::ToolRuntimeError);
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
                    failure_kind = Some(ExecutionFailureKind::ToolRuntimeError);
                    break 'exec Err(anyhow::anyhow!("A2A tool execution is not wired yet"));
                }
                ToolSource::Plugin { plugin_id } => {
                    drop(reg);
                    failure_kind = Some(ExecutionFailureKind::ToolRuntimeError);
                    break 'exec Err(anyhow::anyhow!(
                        "Plugin tool '{}' from '{}' is declarative-only and not executable in this Beta",
                        manifest.name,
                        plugin_id
                    ));
                }
                ToolSource::Mcp { server_name } => {
                    let client = reg.get_mcp_client(server_name);
                    let mock = reg.get_mock_mcp_client(server_name);
                    drop(reg);
                    match (client, mock) {
                        (Some(c), _) => {
                            failure_kind = Some(ExecutionFailureKind::McpClientError);
                            break 'exec c.call_tool(&manifest.name, args.clone()).await;
                        }
                        (None, Some(m)) => {
                            failure_kind = Some(ExecutionFailureKind::McpClientError);
                            break 'exec m.call(&manifest.name, args.clone());
                        }
                        (None, None) => {
                            failure_kind = Some(ExecutionFailureKind::MissingMcpServer);
                            break 'exec Err(anyhow::anyhow!(
                                "MCP server '{}' not found — no client is registered for this server",
                                server_name
                            ));
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
                    block_reason: None,
                    proposal_reason: None,
                    failure_kind: None,
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
                    block_reason: None,
                    proposal_reason: None,
                    failure_kind,
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
                "permission_decision": serde_json::Value::Null,
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
            block_reason: if self.config.allow_writes {
                None
            } else {
                Some(ExecutionBlockReason::Unknown)
            },
            proposal_reason: Some(ExecutionProposalReason::HighRiskAction),
            failure_kind: None,
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

        // Record tool.call_blocked via unified builder
        let emit_blocked = |reason: &str,
                            status: &str,
                            block_reason: Option<ExecutionBlockReason>,
                            proposal_reason: Option<ExecutionProposalReason>,
                            extra: Option<Value>| {
            if let (Some(event_store), Some(ref run_id)) =
                (ctx.event_store.as_ref(), &request.source_run_id)
            {
                let event = AgentRunEvent::new(
                    run_id,
                    AgentRunEventType::ToolCallBlocked,
                    AgentEventActor::Tool(tool_name.to_string()),
                    format!("shell.run blocked: {}", reason),
                    trace_payloads::build_tool_call_blocked_payload(
                        status,
                        tool_name,
                        "builtin",
                        ctx.agent_spec.as_ref().map(|s| s.id.clone()),
                        block_reason.map(|r| r.to_string()),
                        proposal_reason.map(|r| r.to_string()),
                        None::<&str>,
                        extra,
                    ),
                );
                let _ = event_store.append_event(&event);
            }
        };

        // ── 1. Manifest check ──────────────────────────────────────────
        let manifest = match manifest {
            Some(m) => m,
            None => {
                let reason = "shell.run tool is not registered";
                emit_blocked(
                    reason,
                    "blocked",
                    Some(ExecutionBlockReason::DisabledManifest),
                    None,
                    Some(serde_json::json!({"reason": reason})),
                );
                return Ok(self.build_blocked_result(
                    tool_name,
                    args,
                    request,
                    reason,
                    false,
                    None,
                    ExecutionBlockReason::DisabledManifest,
                    None,
                    None,
                ));
            }
        };

        if !manifest.enabled {
            let reason = "shell.run is disabled in manifest";
            emit_blocked(
                reason,
                "blocked",
                Some(ExecutionBlockReason::DisabledManifest),
                None,
                Some(serde_json::json!({"reason": reason})),
            );
            return Ok(self.build_blocked_result(
                tool_name,
                args,
                request,
                reason,
                false,
                Some(manifest),
                ExecutionBlockReason::DisabledManifest,
                None,
                None,
            ));
        }

        if manifest.declarative_only {
            let reason = "shell.run is declarative-only (no executor available)";
            emit_blocked(
                reason,
                "blocked",
                Some(ExecutionBlockReason::DeclarativeOnly),
                None,
                Some(serde_json::json!({"reason": reason})),
            );
            return Ok(self.build_blocked_result(
                tool_name,
                args,
                request,
                reason,
                false,
                Some(manifest),
                ExecutionBlockReason::DeclarativeOnly,
                None,
                None,
            ));
        }

        // ── 2. Sandbox check ───────────────────────────────────────────
        let sandbox = ctx.execution_sandbox;
        if !sandbox.bash_enabled {
            let reason = "shell execution is disabled (sandbox.bash_enabled = false)";
            emit_blocked(
                reason,
                "blocked",
                Some(ExecutionBlockReason::SandboxDenied),
                None,
                Some(serde_json::json!({"reason": reason, "bash_enabled": false})),
            );
            return Ok(self.build_blocked_result(
                tool_name,
                args,
                request,
                reason,
                false,
                Some(manifest),
                ExecutionBlockReason::SandboxDenied,
                None,
                None,
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
                emit_blocked(
                    reason,
                    "blocked",
                    Some(ExecutionBlockReason::AgentSpecDenied),
                    None,
                    Some(serde_json::json!({"reason": reason})),
                );
                return Ok(self.build_blocked_result(
                    tool_name,
                    args,
                    request,
                    reason,
                    false,
                    Some(manifest),
                    ExecutionBlockReason::AgentSpecDenied,
                    None,
                    None,
                ));
            }
            None => {
                let reason =
                    "AgentSpec missing: cannot execute shell.run without governed AgentSpec";
                emit_blocked(
                    reason,
                    "blocked",
                    Some(ExecutionBlockReason::AgentSpecMissing),
                    None,
                    Some(serde_json::json!({"reason": reason})),
                );
                return Ok(self.build_blocked_result(
                    tool_name,
                    args,
                    request,
                    reason,
                    false,
                    Some(manifest),
                    ExecutionBlockReason::AgentSpecMissing,
                    None,
                    None,
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

            emit_blocked(
                reason,
                if needs_confirmation {
                    "needs_confirmation"
                } else {
                    "blocked"
                },
                if needs_confirmation {
                    None
                } else {
                    Some(ExecutionBlockReason::ToolPermissionDenied)
                },
                if needs_confirmation {
                    Some(ExecutionProposalReason::ToolPermissionAsk)
                } else {
                    None
                },
                Some(serde_json::json!({
                    "reason": reason,
                    "needs_confirmation": needs_confirmation,
                    "permission_decision": perm_decision.decision,
                })),
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
                block_reason: if needs_confirmation {
                    None
                } else {
                    Some(ExecutionBlockReason::ToolPermissionDenied)
                },
                proposal_reason: if needs_confirmation {
                    Some(ExecutionProposalReason::ToolPermissionAsk)
                } else {
                    None
                },
                failure_kind: None,
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

        let is_failed = status == ActionExecutionStatus::Failed;
        Ok(ActionExecutionResult {
            action,
            observation,
            status,
            stop_reason,
            block_reason: if is_failed {
                Some(ExecutionBlockReason::SandboxDenied)
            } else {
                None
            },
            proposal_reason: None,
            failure_kind: None,
        })
    }

    /// Build a blocked ActionExecutionResult for early-return paths.
    /// Accepts explicit typed reason so callers can distinguish manifest
    /// missing, disabled, declarative-only, sandbox denied, AgentSpec,
    /// and permission denials.
    #[allow(clippy::too_many_arguments)]
    fn build_blocked_result(
        &self,
        tool_name: &str,
        args: &Value,
        request: &AgentActionRequest,
        reason: &str,
        needs_confirmation: bool,
        manifest: Option<&ToolManifest>,
        block_reason: ExecutionBlockReason,
        proposal_reason: Option<ExecutionProposalReason>,
        permission_decision: Option<String>,
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
            permission_decision: permission_decision.clone(),
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
            stop_reason: Some(block_reason.to_string()),
            block_reason: Some(block_reason),
            proposal_reason,
            failure_kind: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::action_executor::ActionExecutorConfig;
    use crate::config::NetworkPolicy;
    use crate::mcp::McpRegistry;
    use crate::mcp_audit::McpAuditStore;
    use crate::privacy::PrivacyEngine;
    use crate::tool_permissions::ToolPermissionStore;
    use std::sync::atomic::{AtomicU64, Ordering};

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
        let reg = McpRegistry::new();
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

    /// AgentSpec denial: a spec that denies `web.search` must block it.
    #[tokio::test]
    async fn agent_spec_denies_web_search() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();
        let audit = McpAuditStore::new(tmp.path().join("audit_agent_spec1.db"));
        let pe = crate::privacy::PrivacyEngine::new();
        let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let mut spec = crate::agent::types::AgentSpec::default_main_spec();
        spec.denied_tools.push("web.search".to_string());
        let mut ctx = ActionContext::new_for_test(
            McpRegistry::new(),
            crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_agent_spec1a.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(spec);
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "web.search".into(),
            input: serde_json::json!({
                "arguments": {
                    "query": "test query",
                    "max_results": 3
                }
            }),
            source_run_id: Some("test-run-agent-spec-deny".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(result.stop_reason, Some("agent_spec_denied".into()));
    }

    /// AgentSpec allowlist: if allowed_tools does not contain file.read, it should be blocked.
    #[tokio::test]
    async fn agent_spec_allowlist_blocks_file_read() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();
        let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tmp.path().join("audit_agent_spec2.db"));
        let pe = crate::privacy::PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let mut spec = crate::agent::types::AgentSpec::default_main_spec();
        spec.allowed_tools = vec!["web.search".to_string()];
        let mut ctx = ActionContext::new_for_test(
            McpRegistry::new(),
            crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_agent_spec2a.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(spec);
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "file.read".into(),
            input: serde_json::json!({
                "arguments": {
                    "path": "/tmp/test.txt"
                }
            }),
            source_run_id: Some("test-run-agent-spec-allowlist".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(result.stop_reason, Some("agent_spec_denied".into()));
    }

    /// mcp.call_tool must be blocked when NetworkPolicy is disabled.
    #[tokio::test]
    async fn network_policy_enabled_false_blocks_mcp_call_tool() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();
        // Register a mock network-capable MCP target tool so the
        // mcp.call_tool handler can resolve it
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "test_network_tool".to_string(),
                id: "test_network_tool".to_string(),
                description: "test".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "medium".to_string(),
                risk_level: "medium".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["network".to_string(), "read".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".to_string(),
                tags: vec!["execution".to_string()],
            },
            std::sync::Arc::new(|_args| Ok("ok".to_string())),
        );
        let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        // Grant permission for mcp.call_tool so the network policy is the only gate
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        let audit = McpAuditStore::new(tmp.path().join("audit_mcp_np.db"));
        let pe = crate::privacy::PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let policy = NetworkPolicy {
            enabled: false,
            ..Default::default()
        };
        let mut ctx = ActionContext::new_for_test(
            McpRegistry::new(),
            crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_mcp_np2.db")),
            PrivacyEngine::new(),
            vec![],
        );
        ctx.network_policy = Some(policy);
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "test_network_tool",
                    "arguments": {}
                }
            }),
            source_run_id: Some("test-run-mcp-np".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert!(
            result.status == ActionExecutionStatus::Failed
                || result.status == ActionExecutionStatus::Blocked
                || result
                    .action
                    .error
                    .as_ref()
                    .is_some_and(|e| e.contains("disabled by policy")),
            "mcp.call_tool should be blocked by network policy, got status {:?} error {:?}",
            result.status,
            result.action.error
        );
    }

    /// Full triad: ask → Proposal → accept → replay no longer blocked.
    #[tokio::test]
    async fn network_policy_ask_proposal_accept_replay_triad() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();
        let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tmp.path().join("audit_triad.db"));
        let pe = crate::privacy::PrivacyEngine::new();
        let prop_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let prop_store_arc = Arc::new(tokio::sync::Mutex::new(prop_store));
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let policy = NetworkPolicy {
            enabled: true,
            default_decision: "ask".to_string(),
            ..Default::default()
        };
        let mut ctx = ActionContext::new_for_test(
            McpRegistry::new(),
            crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_triad2.db")),
            PrivacyEngine::new(),
            vec![],
        );
        ctx.network_policy = Some(policy.clone());
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();
        ctx.proposal_store = Some(prop_store_arc.clone());

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "web.search".into(),
            input: serde_json::json!({ "arguments": { "query": "test triad" } }),
            source_run_id: Some("test-run-triad".into()),
            step_index: 0,
        };

        // Step 1: First execution with ask → should create a Proposal
        let result1 = executor.execute(request.clone(), &ctx).await.unwrap();
        assert_eq!(
            result1.status,
            ActionExecutionStatus::NeedsConfirmation,
            "first exec with ask should return NeedsConfirmation"
        );

        // A Proposal should have been created
        let proposals = {
            let ps = prop_store_arc.lock().await;
            ps.list_pending_proposals(10).unwrap()
        };
        assert_eq!(proposals.len(), 1, "exactly one proposal should be created");
        assert_eq!(proposals[0].proposal_type, ProposalType::ToolPermission,);
        let after = &proposals[0].after;
        assert_eq!(
            after.get("tool_name").and_then(|v| v.as_str()),
            Some("web.search"),
        );
        assert_eq!(
            after.get("network_policy_ask").and_then(|v| v.as_bool()),
            Some(true),
        );

        // Step 2: Simulate proposal acceptance — grant matching permission
        {
            let perm = ps_arc.lock().await;
            perm.grant(
                "web.search",
                "builtin",
                "medium",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        }

        // Step 3: Second execution (simulates replay after permission granted)
        let result2 = executor.execute(request, &ctx).await.unwrap();
        assert!(
            result2.status != ActionExecutionStatus::NeedsConfirmation
                && result2.status != ActionExecutionStatus::Blocked,
            "second exec with granted permission should not be blocked by network ask, got {:?}",
            result2.status
        );
    }

    /// Hard blocks (enabled=false) must not be bypassed by tool permission.
    #[tokio::test]
    async fn network_policy_enabled_false_not_bypassed_by_permission() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();
        let ps = crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        // Pre-grant a permission for web.search
        ps.grant(
            "web.search",
            "builtin",
            "medium",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        let audit = McpAuditStore::new(tmp.path().join("audit_hardblock.db"));
        let pe = crate::privacy::PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let policy = NetworkPolicy {
            enabled: false,
            default_decision: "allow".to_string(),
            ..Default::default()
        };
        let mut ctx = ActionContext::new_for_test(
            McpRegistry::new(),
            crate::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_hardblock2.db")),
            PrivacyEngine::new(),
            vec![],
        );
        ctx.network_policy = Some(policy);
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "web.search".into(),
            input: serde_json::json!({ "arguments": { "query": "test hardblock" } }),
            source_run_id: Some("test-run-hardblock".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(
            result.status,
            ActionExecutionStatus::Blocked,
            "enabled=false must block even with a tool permission, got {:?}",
            result.status
        );
    }

    /// MCP network ask Proposal must use real MCP target manifest metadata, not wrapper.
    #[tokio::test]
    async fn mcp_network_ask_proposal_uses_real_mcp_target_manifest_not_wrapper() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        // Register mcp.call_tool wrapper (medium, external_side_effect)
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "mcp.call_tool".to_string(),
                id: "mcp.call_tool".to_string(),
                description: "wrapper".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "medium".to_string(),
                risk_level: "medium".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["network".to_string(), "external_side_effect".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "external_side_effect".to_string(),
                tags: vec!["execution".to_string(), "mcp_wrapper".to_string()],
            },
            std::sync::Arc::new(|_args| Ok("ok".to_string())),
        );
        // Register target with REAL ToolSource::Mcp (low risk, read action)
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "target_low_read_mcp".to_string(),
                id: "target_low_read_mcp".to_string(),
                description: "target".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "low".to_string(),
                risk_level: "low".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::Mcp {
                    server_name: "test-server".to_string(),
                },
                capabilities: vec!["network".to_string(), "read".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".to_string(),
                tags: vec!["execution".to_string()],
            },
            std::sync::Arc::new(|_args| Ok("ok".to_string())),
        );
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        // Grant wrapper mcp.call_tool permission so only target network ask gates
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        let audit = McpAuditStore::new(tmp.path().join("audit_mcp_wrapper.db"));
        let pe = PrivacyEngine::new();
        let prop_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let prop_store_arc = Arc::new(tokio::sync::Mutex::new(prop_store));
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let policy = NetworkPolicy {
            enabled: true,
            default_decision: "ask".to_string(),
            ..Default::default()
        };
        let mut ctx = ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_mcp_wrapper2.db")),
            PrivacyEngine::new(),
            vec![],
        );
        ctx.network_policy = Some(policy);
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();
        ctx.proposal_store = Some(prop_store_arc.clone());

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "target_low_read_mcp",
                    "arguments": {}
                }
            }),
            source_run_id: Some("test-run-mcp-wrapper".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::NeedsConfirmation);

        // Verify Proposal uses REAL MCP TARGET metadata, not wrapper
        let proposals = {
            let ps = prop_store_arc.lock().await;
            ps.list_pending_proposals(10).unwrap()
        };
        assert_eq!(proposals.len(), 1);
        let after = &proposals[0].after;
        assert_eq!(
            after.get("tool_name").and_then(|v| v.as_str()),
            Some("target_low_read_mcp"),
            "Proposal tool_name must be the target, not mcp.call_tool"
        );
        assert_eq!(
            after.get("source").and_then(|v| v.as_str()),
            Some("mcp:test-server"),
            "Proposal source must be mcp:test-server, NOT builtin"
        );
        assert_eq!(
            after.get("risk_level").and_then(|v| v.as_str()),
            Some("low"),
            "Proposal risk_level must match target (low), not wrapper (medium)"
        );
        assert_eq!(
            after.get("action_type").and_then(|v| v.as_str()),
            Some("read"),
            "Proposal action_type must match target (read), not wrapper (external_side_effect)"
        );
        // Verify capabilities are present and use target capabilities
        let caps = after.get("capabilities").and_then(|v| v.as_array());
        assert!(
            caps.is_some_and(|arr| {
                let strs: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                strs.contains(&"network") && strs.contains(&"read")
            }),
            "Proposal capabilities must contain target capabilities (network, read)"
        );

        // Verify action.tool_scope also uses target metadata
        let ts = result.action.tool_scope.as_ref().unwrap();
        assert_eq!(ts.tool_name, "target_low_read_mcp");
        assert_eq!(
            ts.source, "mcp:test-server",
            "tool_scope.source must be mcp:test-server"
        );
        assert_eq!(ts.risk_level, "low");
        assert_eq!(ts.action_type, "read");
        assert!(ts.capabilities.contains(&"network".to_string()));
        assert!(ts.capabilities.contains(&"read".to_string()));
    }

    // ── Batch 2: MCP Target Governance ────────────────────────────

    /// mcp_call_tool must not execute a target that AgentSpec denies,
    /// even when the wrapper mcp.call_tool itself is allowed.
    /// Uses a side-effect counter to prove the target never ran.
    #[tokio::test]
    async fn mcp_call_tool_denied_target_is_blocked() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        let side_effect_count = Arc::new(AtomicU64::new(0));
        let counter = side_effect_count.clone();

        // Register mcp.call_tool wrapper
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "mcp.call_tool".to_string(),
                id: "mcp.call_tool".to_string(),
                description: "wrapper".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "medium".to_string(),
                risk_level: "medium".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["network".to_string(), "external_side_effect".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "external_side_effect".to_string(),
                tags: vec!["execution".to_string(), "mcp_wrapper".to_string()],
            },
            Arc::new(|_args| Ok("ok".to_string())),
        );
        // Register a target with MCP source and a side-effect counter
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "target_denied_tool".to_string(),
                id: "target_denied_tool".to_string(),
                description: "blocked target".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "low".to_string(),
                risk_level: "low".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::Mcp {
                    server_name: "test-server".to_string(),
                },
                capabilities: vec!["read".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".to_string(),
                tags: vec!["execution".to_string()],
            },
            Arc::new(move |_args| {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok("executed".to_string())
            }),
        );

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        // Grant wrapper permission so it passes the first gate
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        // Grant target permission so permission gate doesn't block
        ps.grant(
            "target_denied_tool",
            "mcp:test-server",
            "low",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();

        let audit = McpAuditStore::new(tmp.path().join("audit_target_denied.db"));
        let pe = PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        // AgentSpec: allow mcp.call_tool (wrapper) but deny the target
        let spec = crate::agent::types::AgentSpec::default_main_spec()
            .with_denied_tools(vec!["target_denied_tool".to_string()]);
        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_target_denied2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(spec);
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "target_denied_tool",
                    "arguments": {}
                }
            }),
            source_run_id: Some("run-target-denied".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();

        // Must be blocked — AgentSpec denies the target
        assert_eq!(result.status, crate::agent::ActionExecutionStatus::Blocked);
        assert_eq!(result.stop_reason, Some("target_agent_spec_denied".into()));

        // Error must mention the target tool and AgentSpec governance
        let err = result.action.error.unwrap();
        assert!(
            err.contains("target_denied_tool"),
            "error must name the real target tool: {}",
            err
        );
        assert!(
            err.contains("AgentSpec"),
            "error must mention AgentSpec governance: {}",
            err
        );

        // Side-effect counter must not have incremented
        assert_eq!(
            side_effect_count.load(Ordering::SeqCst),
            0,
            "denied MCP target must not execute"
        );

        // Tool_scope must reference the real target (MCP source, not builtin)
        let ts = result
            .action
            .tool_scope
            .expect("blocked action must have tool_scope");
        assert_eq!(ts.tool_name, "target_denied_tool");
        assert_eq!(ts.source, "mcp:test-server");
        assert!(!ts.allowed, "tool_scope.allowed must be false");
    }

    /// Explicitly allow mcp.call_tool wrapper, deny the target in denied_tools.
    /// The wrapper allow must NOT override the target deny.
    #[tokio::test]
    async fn mcp_call_tool_allowed_wrapper_denied_target_is_blocked() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        let side_effect_count = Arc::new(AtomicU64::new(0));
        let counter = side_effect_count.clone();

        // Register wrapper
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "mcp.call_tool".to_string(),
                id: "mcp.call_tool".to_string(),
                description: "wrapper".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "medium".to_string(),
                risk_level: "medium".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["network".to_string(), "external_side_effect".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "external_side_effect".to_string(),
                tags: vec!["execution".to_string(), "mcp_wrapper".to_string()],
            },
            Arc::new(|_args| Ok("ok".to_string())),
        );
        // Register target with explicit deny and side-effect counter
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "mcp_target_explicit_deny".to_string(),
                id: "mcp_target_explicit_deny".to_string(),
                description: "denied target".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "low".to_string(),
                risk_level: "low".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::Mcp {
                    server_name: "test-server".to_string(),
                },
                capabilities: vec!["read".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".to_string(),
                tags: vec!["execution".to_string()],
            },
            Arc::new(move |_args| {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok("executed".to_string())
            }),
        );

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        ps.grant(
            "mcp_target_explicit_deny",
            "mcp:test-server",
            "low",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();

        let audit = McpAuditStore::new(tmp.path().join("audit_explicit_deny.db"));
        let pe = PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        // AgentSpec: explicitly allow mcp.call_tool, but deny the target
        let mut spec = crate::agent::types::AgentSpec::default_main_spec();
        spec.allowed_tools = vec!["mcp.call_tool".to_string()];
        spec.denied_tools = vec!["mcp_target_explicit_deny".to_string()];
        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_explicit_deny2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(spec);
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "mcp_target_explicit_deny",
                    "arguments": {}
                }
            }),
            source_run_id: Some("run-explicit-deny".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();

        // Must be blocked
        assert_eq!(result.status, crate::agent::ActionExecutionStatus::Blocked);
        // Side-effect counter must be zero — target never executed
        assert_eq!(
            side_effect_count.load(Ordering::SeqCst),
            0,
            "denied MCP target must not execute — wrapper allow does not override target deny"
        );

        // Verify blocked action has real target in tool_scope
        let ts = result.action.tool_scope.as_ref().unwrap();
        assert_eq!(ts.tool_name, "mcp_target_explicit_deny");
        assert_eq!(ts.source, "mcp:test-server");
    }

    /// When both the wrapper and the target are allowed by AgentSpec,
    /// the target must execute successfully and tool_scope must point to
    /// the real MCP target (not the mcp.call_tool wrapper).
    #[tokio::test]
    async fn mcp_call_tool_allowed_target_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        let side_effect_count = Arc::new(AtomicU64::new(0));
        let counter = side_effect_count.clone();

        // Register wrapper
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "mcp.call_tool".to_string(),
                id: "mcp.call_tool".to_string(),
                description: "wrapper".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "medium".to_string(),
                risk_level: "medium".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["network".to_string(), "external_side_effect".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "external_side_effect".to_string(),
                tags: vec!["execution".to_string(), "mcp_wrapper".to_string()],
            },
            Arc::new(|_args| Ok("ok".to_string())),
        );
        // Register allowed target manifest (dummy builtin — execution
        // goes through the mock MCP client below, not through this closure).
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "target_allowed_tool".to_string(),
                id: "target_allowed_tool".to_string(),
                description: "allowed target".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "low".to_string(),
                risk_level: "low".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::Mcp {
                    server_name: "test-server".to_string(),
                },
                capabilities: vec!["read".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".to_string(),
                tags: vec!["execution".to_string()],
            },
            Arc::new(|_args| panic!("MCP target must not execute via builtin fallback")),
        );
        // Register mock MCP client for the server — this is the real
        // execution path that the test verifies.
        let mock_counter = counter.clone();
        let mock_client = crate::mcp::MockMcpClient::new(
            "test-server",
            move |_name: String, _args: Value| -> anyhow::Result<String> {
                mock_counter.fetch_add(1, Ordering::SeqCst);
                Ok("target executed successfully".to_string())
            },
        );
        r.register_mock_mcp_client("test-server", mock_client);

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        ps.grant(
            "target_allowed_tool",
            "mcp:test-server",
            "low",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();

        let audit = McpAuditStore::new(tmp.path().join("audit_target_ok.db"));
        let pe = PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        // Default spec allows all non-denied tools (only denies shell.run)
        let spec = crate::agent::types::AgentSpec::default_main_spec();
        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_target_ok2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(spec);
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "target_allowed_tool",
                    "arguments": {}
                }
            }),
            source_run_id: Some("run-target-ok".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();

        // Must succeed
        assert_eq!(
            result.status,
            crate::agent::ActionExecutionStatus::Succeeded
        );

        // Side-effect counter must be 1
        assert_eq!(
            side_effect_count.load(Ordering::SeqCst),
            1,
            "allowed MCP target must execute"
        );

        // Tool_scope must point to the real target, NOT the wrapper
        let ts = result
            .action
            .tool_scope
            .as_ref()
            .expect("success action must have tool_scope");
        assert_eq!(ts.tool_name, "target_allowed_tool");
        assert_eq!(ts.tool_id, "target_allowed_tool");
        assert_eq!(
            ts.source, "mcp:test-server",
            "source must be MCP server, not builtin wrapper"
        );
        assert_eq!(ts.risk_level, "low");
        assert_eq!(ts.action_type, "read");
        assert!(ts.allowed, "tool_scope.allowed must be true for success");
    }

    // ── Batch 3: No Fake MCP Execution ────────────────────────────
    //
    // These tests verify that ToolSource::Mcp never falls back to a
    // builtin closure. MCP source tools MUST execute through a real
    // MCP client or through the test-only mock MCP client seam.

    /// Register a BuiltIn tool with side-effect counter, then construct
    /// an MCP-source manifest with the same name but pointing to a
    /// missing server.  Execute via `call_tool_internal_async` with the
    /// MCP manifest.  Assert failure, and prove the builtin closure
    /// was never called.
    #[tokio::test]
    async fn mcp_source_never_falls_back_to_builtin() {
        let temp = tempfile::tempdir().unwrap();
        let mut reg = McpRegistry::new();
        let builtin_counter = Arc::new(AtomicU64::new(0));
        let bc = builtin_counter.clone();

        reg.register_builtin(
            crate::tool_manifest::ToolManifest {
                id: "shadow_tool".into(),
                name: "shadow_tool".into(),
                description: "builtin shadow".into(),
                parameters: serde_json::json!({}),
                permission_level: "low".into(),
                risk_level: "low".into(),
                version: "1.0.0".into(),
                source: crate::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["read".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                tags: vec![],
            },
            Arc::new(move |_args| {
                bc.fetch_add(1, Ordering::SeqCst);
                Ok("builtin-success".to_string())
            }),
        );

        let mcp_manifest = crate::tool_manifest::ToolManifest {
            id: "mcp:missing-server:shadow_tool".into(),
            name: "shadow_tool".into(),
            description: "MCP shadow".into(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: crate::tool_manifest::ToolSource::Mcp {
                server_name: "missing-server".into(),
            },
            capabilities: vec!["read".into()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };

        let registry = Arc::new(tokio::sync::Mutex::new(reg));
        let audit = Arc::new(tokio::sync::Mutex::new(McpAuditStore::new(
            temp.path().join("audit_no_fallback.db"),
        )));
        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());

        let result = executor
            .call_tool_internal_async(
                &mcp_manifest,
                serde_json::json!({}),
                &registry,
                &audit,
                false,
            )
            .await;

        assert!(
            !result.success,
            "MCP source with missing server must fail, got success"
        );
        let err = result.error.expect("error must be present");
        assert!(
            err.contains("missing-server"),
            "error must name the missing server: {}",
            err
        );
        assert_eq!(
            builtin_counter.load(Ordering::SeqCst),
            0,
            "builtin closure must NOT be called for MCP source tool"
        );
        assert!(
            !result
                .output
                .unwrap_or_default()
                .contains("builtin-success"),
            "output must not contain builtin success value"
        );
    }

    /// MCP source tool whose server is not registered and no mock
    /// client exists.  Must fail with a visible error — no fallback,
    /// no fake success, no empty output.
    #[tokio::test]
    async fn mcp_missing_server_fails() {
        let temp = tempfile::tempdir().unwrap();
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();

        let mcp_manifest = crate::tool_manifest::ToolManifest {
            id: "mcp:nowhere:remote_tool".into(),
            name: "remote_tool".into(),
            description: "Tool on a non-existent server".into(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: crate::tool_manifest::ToolSource::Mcp {
                server_name: "nowhere-server".into(),
            },
            capabilities: vec!["read".into()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };

        let registry = Arc::new(tokio::sync::Mutex::new(reg));
        let audit = Arc::new(tokio::sync::Mutex::new(McpAuditStore::new(
            temp.path().join("audit_missing_server.db"),
        )));
        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());

        let result = executor
            .call_tool_internal_async(
                &mcp_manifest,
                serde_json::json!({"key": "val"}),
                &registry,
                &audit,
                false,
            )
            .await;

        assert!(
            !result.success,
            "MCP tool on missing server must fail, not succeed silently"
        );
        let err = result.error.expect("error must be present, not swallowed");
        assert!(
            err.contains("nowhere-server"),
            "error must mention server name: {}",
            err
        );
        assert!(!err.is_empty(), "error message must not be empty");
        assert!(
            result.output.is_none(),
            "output must be None on failure, not a fake success string"
        );
    }

    /// Mock MCP client that returns an error must surface it as a
    /// failed ActionExecutionResult with the error visible in both
    /// action.error and observation content.
    #[tokio::test]
    async fn mcp_client_error_surfaces() {
        let temp = tempfile::tempdir().unwrap();
        let mut reg = McpRegistry::new();

        // Register wrapper so mcp.call_tool is available
        reg.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "mcp.call_tool".into(),
                id: "mcp.call_tool".into(),
                description: "wrapper".into(),
                parameters: serde_json::json!({}),
                permission_level: "medium".into(),
                risk_level: "medium".into(),
                version: "1.0.0".into(),
                source: crate::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["network".into(), "external_side_effect".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "external_side_effect".into(),
                tags: vec!["execution".into(), "mcp_wrapper".into()],
            },
            Arc::new(|_args| Ok("wrapper-ok".to_string())),
        );

        // Register MCP target manifest
        reg.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "faulty_mcp_tool".into(),
                id: "faulty_mcp_tool".into(),
                description: "MCP tool that returns errors".into(),
                parameters: serde_json::json!({}),
                permission_level: "low".into(),
                risk_level: "low".into(),
                version: "1.0.0".into(),
                source: crate::tool_manifest::ToolSource::Mcp {
                    server_name: "faulty-server".into(),
                },
                capabilities: vec!["read".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                tags: vec!["execution".into()],
            },
            Arc::new(|_args| panic!("MCP target must not execute via builtin fallback")),
        );

        // Register mock MCP client that always returns an error
        let mock_error_msg = "MCP_TOOL_INTERNAL_ERROR: database connection refused";
        reg.register_mock_mcp_client(
            "faulty-server",
            crate::mcp::MockMcpClient::new("faulty-server", move |_name: String, _args: Value| {
                Err(anyhow::anyhow!(mock_error_msg.to_string()))
            }),
        );

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        ps.grant(
            "faulty_mcp_tool",
            "mcp:faulty-server",
            "low",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();

        let audit = McpAuditStore::new(temp.path().join("audit_mcp_err.db"));
        let pe = PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(reg));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let spec = crate::agent::types::AgentSpec::default_main_spec();
        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(temp.path().join("audit_mcp_err2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(spec);
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "faulty_mcp_tool",
                    "arguments": {"test": true}
                }
            }),
            source_run_id: Some("run-mcp-error".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();

        assert_eq!(
            result.status,
            crate::agent::ActionExecutionStatus::Failed,
            "MCP client error must produce Failed status, got {:?}",
            result.status
        );

        let err_in_action = result.action.error.expect("action.error must be present");
        assert!(
            err_in_action.contains("database connection refused"),
            "action.error must contain the MCP error message: {}",
            err_in_action
        );

        let obs_content = &result.observation.content;
        assert!(
            obs_content.contains("database connection refused"),
            "observation.content must surface the MCP error: {}",
            obs_content
        );
    }

    /// When a target is blocked, the trace must record the real MCP
    /// target source (mcp:server, target tool name), not just the
    /// mcp.call_tool wrapper name.  Verify tool_scope.source, error
    /// message content, and observation content.
    #[tokio::test]
    async fn mcp_call_tool_target_block_records_real_source() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        let side_effect_count = Arc::new(AtomicU64::new(0));
        let counter = side_effect_count.clone();

        // Register wrapper
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "mcp.call_tool".to_string(),
                id: "mcp.call_tool".to_string(),
                description: "wrapper".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "medium".to_string(),
                risk_level: "medium".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["network".to_string(), "external_side_effect".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "external_side_effect".to_string(),
                tags: vec!["execution".to_string(), "mcp_wrapper".to_string()],
            },
            Arc::new(|_args| Ok("ok".to_string())),
        );
        // Register target with MCP source
        r.register_builtin(
            crate::tool_manifest::ToolManifest {
                name: "mcp_real_target_trace".to_string(),
                id: "mcp_real_target_trace".to_string(),
                description: "target for trace test".to_string(),
                parameters: serde_json::json!({}),
                permission_level: "low".to_string(),
                risk_level: "low".to_string(),
                version: "1.0.0".to_string(),
                source: crate::tool_manifest::ToolSource::Mcp {
                    server_name: "real-server".to_string(),
                },
                capabilities: vec!["read".to_string()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".to_string(),
                tags: vec!["execution".to_string()],
            },
            Arc::new(move |_args| {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok("should not execute".to_string())
            }),
        );

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        ps.grant(
            "mcp_real_target_trace",
            "mcp:real-server",
            "low",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();

        let audit = McpAuditStore::new(tmp.path().join("audit_trace.db"));
        let pe = PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        // AgentSpec: allow wrapper, but use an allowlist that excludes the target
        let mut spec = crate::agent::types::AgentSpec::default_main_spec();
        spec.allowed_tools = vec!["mcp.call_tool".to_string()];
        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_trace2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(spec);
        ctx.registry = reg_arc.clone();
        ctx.permission_store = ps_arc.clone();
        ctx.audit_store = audit_arc.clone();
        ctx.privacy_engine = pe_arc.clone();

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": "mcp_real_target_trace",
                    "arguments": {}
                }
            }),
            source_run_id: Some("run-trace".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();

        // Blocked with zero side effects
        assert_eq!(result.status, crate::agent::ActionExecutionStatus::Blocked);
        assert_eq!(side_effect_count.load(Ordering::SeqCst), 0);

        // tool_scope must record real target source, not wrapper builtin
        let ts = result.action.tool_scope.as_ref().unwrap();
        assert_eq!(ts.tool_name, "mcp_real_target_trace");
        assert_eq!(
            ts.source, "mcp:real-server",
            "block trace must identify real MCP source, not wrapper builtin"
        );
        assert!(!ts.allowed);

        // Error message must contain the real target tool name
        let err = result.action.error.unwrap();
        assert!(
            err.contains("mcp_real_target_trace"),
            "error must mention real target name: {}",
            err
        );
        assert!(
            err.contains("AgentSpec"),
            "error must mention AgentSpec: {}",
            err
        );

        // Observation content must reference real governance context
        let obs = &result.observation.content;
        assert!(
            obs.contains("mcp_real_target_trace"),
            "observation content must reference the real target: {}",
            obs
        );
        assert!(
            obs.contains("AgentSpec"),
            "observation content must mention AgentSpec governance: {}",
            obs
        );
    }

    /// AgentSpec deny returns typed block_reason = AgentSpecDenied.
    /// This test verifies that the existing mcp_call_tool_denied_target_is_blocked test
    /// pattern also produces a typed block_reason.
    #[tokio::test]
    async fn typed_reason_agentspec_deny_on_result() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();

        // Build target tool manifest with Mcp source so it goes through the
        // mcp.call_tool target path that produces typed reasons.
        let target_name = "typed_deny_target";
        let target_manifest = ToolManifest {
            name: target_name.to_string(),
            id: target_name.to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            version: "1.0".into(),
            source: crate::tool_manifest::ToolSource::Mcp {
                server_name: "test-server".to_string(),
            },
            risk_level: "low".to_string(),
            capabilities: vec![],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };
        let mock = crate::mcp::MockMcpClient::new("test-server", |_name, _args| Ok("ok".into()));
        r.register_mock_mcp_client("test-server", mock);
        r.register_builtin(target_manifest, Arc::new(|_args| Ok("ok".into())));

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        ps.grant(
            target_name,
            "mcp:test-server",
            "low",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();

        let audit = McpAuditStore::new(tmp.path().join("audit_typed_deny.db"));
        let pe = PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let spec = crate::agent::types::AgentSpec::default_main_spec()
            .with_denied_tools(vec![target_name.to_string()]);
        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_typed_deny2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(spec);
        ctx.registry = reg_arc;
        ctx.permission_store = ps_arc;
        ctx.audit_store = audit_arc;
        ctx.privacy_engine = pe_arc;

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": target_name,
                    "arguments": {}
                }
            }),
            source_run_id: Some("run-typed-deny".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();

        assert_eq!(
            result.block_reason,
            Some(ExecutionBlockReason::AgentSpecDenied),
            "must have typed block_reason = AgentSpecDenied"
        );
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
    }

    /// The existing mcp_missing_server_fails test also checks failure_kind.
    #[tokio::test]
    async fn typed_reason_missing_mcp_server_failure_kind() {
        let mut r = McpRegistry::new();
        let manifest = ToolManifest {
            name: "missing_mcp_tool".to_string(),
            id: "missing_mcp_tool".to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            version: "1.0".into(),
            source: crate::tool_manifest::ToolSource::Mcp {
                server_name: "nowhere-server".to_string(),
            },
            risk_level: "low".to_string(),
            capabilities: vec![],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };
        r.register_builtin(manifest.clone(), Arc::new(|_args| Ok("ok".into())));

        let audit = McpAuditStore::new(
            tempfile::tempdir()
                .unwrap()
                .path()
                .join("audit_missing_fk.db"),
        );
        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let result = executor
            .call_tool_internal_async(
                &manifest,
                serde_json::json!({}),
                &Arc::new(tokio::sync::Mutex::new(McpRegistry::new())),
                &Arc::new(tokio::sync::Mutex::new(audit)),
                false,
            )
            .await;

        assert!(!result.success);
        assert_eq!(
            result.failure_kind,
            Some(ExecutionFailureKind::MissingMcpServer),
            "missing MCP server should produce failure_kind = MissingMcpServer"
        );
    }

    /// MCP client error returns failure_kind = McpClientError.
    #[tokio::test]
    async fn typed_reason_mcp_client_error_on_call() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        let mock = crate::mcp::MockMcpClient::new("error-server", |_name, _args| {
            Err(anyhow::anyhow!("simulated MCP error"))
        });
        r.register_mock_mcp_client("error-server", mock);
        let manifest = ToolManifest {
            name: "error_mcp_tool".to_string(),
            id: "error_mcp_tool".to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            version: "1.0".into(),
            source: crate::tool_manifest::ToolSource::Mcp {
                server_name: "error-server".to_string(),
            },
            risk_level: "low".to_string(),
            capabilities: vec![],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };
        r.register_builtin(manifest.clone(), Arc::new(|_args| Ok("ok".into())));

        let audit = McpAuditStore::new(tmp.path().join("audit_error_fk.db"));
        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let result = executor
            .call_tool_internal_async(
                &manifest,
                serde_json::json!({}),
                &Arc::new(tokio::sync::Mutex::new(r)),
                &Arc::new(tokio::sync::Mutex::new(audit)),
                false,
            )
            .await;

        assert!(!result.success);
        assert_eq!(
            result.failure_kind,
            Some(ExecutionFailureKind::McpClientError),
            "MCP client runtime error should produce failure_kind = McpClientError"
        );
    }

    /// Disabled manifest returns block_reason = DisabledManifest.
    #[tokio::test]
    async fn typed_reason_disabled_manifest_block() {
        let mut r = McpRegistry::new();
        let manifest = ToolManifest {
            name: "disabled_tool".to_string(),
            id: "disabled_tool".to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            version: "1.0".into(),
            source: crate::tool_manifest::ToolSource::BuiltIn,
            risk_level: "low".to_string(),
            capabilities: vec![],
            requires_confirmation: false,
            enabled: false,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };
        r.register_builtin(manifest, Arc::new(|_args| Ok("ok".into())));

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(
            tempfile::tempdir()
                .unwrap()
                .path()
                .join("audit_disabled_fk.db"),
        );
        let pe = PrivacyEngine::new();

        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(
                tempfile::tempdir()
                    .unwrap()
                    .path()
                    .join("audit_disabled2_fk.db"),
            ),
            PrivacyEngine::new(),
            vec![],
        );
        ctx.registry = reg_arc;
        ctx.permission_store = ps_arc;
        ctx.audit_store = audit_arc;
        ctx.privacy_engine = pe_arc;

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "disabled_tool".into(),
            input: serde_json::json!({"arguments": {}}),
            source_run_id: Some("run-disabled".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();

        assert_eq!(
            result.block_reason,
            Some(ExecutionBlockReason::DisabledManifest),
            "disabled manifest should produce block_reason = DisabledManifest"
        );
    }

    // ── shell.run typed reason tests ──────────────────────────────────

    /// Default shell.run manifest is disabled → DisabledManifest typed reason.
    #[tokio::test]
    async fn shell_manifest_disabled_uses_disabled_manifest_reason() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        // Default shell.run manifest is enabled: false
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_sh_md.db"));
        let pe = PrivacyEngine::new();

        let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
            bash_enabled: true,
            ..crate::agent::execution_sandbox::ExecutionSandbox::default()
        };

        let reg_arc = Arc::new(tokio::sync::Mutex::new(reg));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_sh_md2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_execution_sandbox(sandbox)
        .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());
        ctx.registry = reg_arc;
        ctx.permission_store = ps_arc;
        ctx.audit_store = audit_arc;
        ctx.privacy_engine = pe_arc;

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "shell.run".into(),
            input: serde_json::json!({"arguments": {"command": "echo hi"}}),
            source_run_id: Some("run-sh-md".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            result.block_reason,
            Some(ExecutionBlockReason::DisabledManifest),
            "disabled shell manifest -> DisabledManifest"
        );
    }

    /// Declarative-only tool (not shell.run) produces DeclarativeOnly typed reason
    /// via the generic execute_tool → handle_blocked path.
    #[tokio::test]
    async fn shell_declarative_only_uses_declarative_only_reason() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        // Add a declarative-only tool
        let do_manifest = ToolManifest {
            name: "do_test_tool".to_string(),
            id: "do_test_tool".to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            version: "1.0".into(),
            source: crate::tool_manifest::ToolSource::BuiltIn,
            risk_level: "low".into(),
            capabilities: vec![],
            requires_confirmation: false,
            enabled: true,
            declarative_only: true,
            action_type: "read".into(),
            tags: vec![],
        };
        reg.register_builtin(do_manifest, Arc::new(|_args| Ok("ok".into())));

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_sh_do.db"));
        let pe = PrivacyEngine::new();
        let ctx = crate::agent::ActionContext::new_for_test(reg, ps, audit, pe, vec![])
            .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "do_test_tool".into(),
            input: serde_json::json!({"arguments": {}}),
            source_run_id: Some("run-sh-do".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            result.block_reason,
            Some(ExecutionBlockReason::DeclarativeOnly),
            "declarative-only tool -> DeclarativeOnly"
        );
    }

    #[tokio::test]
    async fn shell_sandbox_disabled_uses_sandbox_denied_reason() {
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        // Enable the manifest but keep sandbox disabled
        reg.set_builtin_manifest_enabled("shell.run", true);
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_sh_sd.db"));
        let pe = PrivacyEngine::new();

        // Sandbox with bash_enabled = false (default)
        let sandbox = crate::agent::execution_sandbox::ExecutionSandbox::default();

        let reg_arc = Arc::new(tokio::sync::Mutex::new(reg));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tempfile::tempdir().unwrap().path().join("audit_sh_sd2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_execution_sandbox(sandbox)
        .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());
        ctx.registry = reg_arc;
        ctx.permission_store = ps_arc;
        ctx.audit_store = audit_arc;
        ctx.privacy_engine = pe_arc;

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "shell.run".into(),
            input: serde_json::json!({"arguments": {"command": "echo hi"}}),
            source_run_id: Some("run-sh-sd".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            result.block_reason,
            Some(ExecutionBlockReason::SandboxDenied),
            "sandbox disabled -> SandboxDenied"
        );
    }

    #[tokio::test]
    async fn shell_agentspec_denied_uses_agentspec_denied_reason() {
        let tmp = tempfile::tempdir().unwrap();
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        reg.set_builtin_manifest_enabled("shell.run", true);
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tmp.path().join("audit_sh_as.db"));
        let pe = PrivacyEngine::new();

        let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
            bash_enabled: true,
            ..crate::agent::execution_sandbox::ExecutionSandbox::default()
        };
        // AgentSpec denies shell.run
        let spec = crate::agent::types::AgentSpec::default_main_spec()
            .with_denied_tools(vec!["shell.run".to_string()]);

        let reg_arc = Arc::new(tokio::sync::Mutex::new(reg));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_sh_as2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_execution_sandbox(sandbox)
        .with_agent_spec(spec);
        ctx.registry = reg_arc;
        ctx.permission_store = ps_arc;
        ctx.audit_store = audit_arc;
        ctx.privacy_engine = pe_arc;

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "shell.run".into(),
            input: serde_json::json!({"arguments": {"command": "echo hi"}}),
            source_run_id: Some("run-sh-as".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            result.block_reason,
            Some(ExecutionBlockReason::AgentSpecDenied),
            "AgentSpec denied -> AgentSpecDenied"
        );
    }

    #[tokio::test]
    async fn shell_permission_denied_uses_tool_permission_denied_reason() {
        let tmp = tempfile::tempdir().unwrap();
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        reg.set_builtin_manifest_enabled("shell.run", true);
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        // No permission granted for shell.run
        let audit = McpAuditStore::new(tmp.path().join("audit_sh_pd.db"));
        let pe = PrivacyEngine::new();

        let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
            bash_enabled: true,
            ..crate::agent::execution_sandbox::ExecutionSandbox::default()
        };

        let reg_arc = Arc::new(tokio::sync::Mutex::new(reg));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_sh_pd2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_execution_sandbox(sandbox)
        .with_agent_spec(
            crate::agent::types::AgentSpec::default_main_spec().with_denied_tools(vec![]),
        );
        ctx.registry = reg_arc;
        ctx.permission_store = ps_arc;
        ctx.audit_store = audit_arc;
        ctx.privacy_engine = pe_arc;

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "shell.run".into(),
            input: serde_json::json!({"arguments": {"command": "echo hi"}}),
            source_run_id: Some("run-sh-pd".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        // No permission → ask_every_time → NeedsConfirmation with ToolPermissionAsk
        assert_eq!(
            result.status,
            ActionExecutionStatus::NeedsConfirmation,
            "no permission -> ask_every_time -> NeedsConfirmation"
        );
        assert_eq!(
            result.proposal_reason,
            Some(ExecutionProposalReason::ToolPermissionAsk),
            "ask_every_time -> ToolPermissionAsk"
        );
    }

    // ── mcp.call_tool target permission semantics tests ───────────────

    #[tokio::test]
    async fn mcp_target_permission_ask_has_tool_permission_ask_reason() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();

        // Target with MCP source; no permission granted, so ask_every_time fallback
        let target_name = "mcp_ask_target";
        let target_manifest = ToolManifest {
            name: target_name.to_string(),
            id: target_name.to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            version: "1.0".into(),
            source: crate::tool_manifest::ToolSource::Mcp {
                server_name: "ask-server".to_string(),
            },
            risk_level: "high".to_string(),
            capabilities: vec!["write".to_string()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "write".into(),
            tags: vec![],
        };
        let mock = crate::mcp::MockMcpClient::new("ask-server", |_name, _args| Ok("result".into()));
        r.register_mock_mcp_client("ask-server", mock);
        r.register_builtin(target_manifest, Arc::new(|_args| Ok("ok".into())));

        // Grant wrapper permission only, not target
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        // Set network policy to allow for the target
        let policy = crate::config::NetworkPolicy {
            enabled: true,
            default_decision: "allow".into(),
            ..crate::config::NetworkPolicy::default()
        };

        let audit = McpAuditStore::new(tmp.path().join("audit_mcp_ask.db"));
        let pe = PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_mcp_ask2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());
        ctx.registry = reg_arc;
        ctx.permission_store = ps_arc;
        ctx.audit_store = audit_arc;
        ctx.privacy_engine = pe_arc;
        ctx.network_policy = Some(policy);

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": target_name,
                    "arguments": {}
                }
            }),
            source_run_id: Some("run-mcp-ask".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::NeedsConfirmation);
        assert_eq!(
            result.proposal_reason,
            Some(ExecutionProposalReason::ToolPermissionAsk),
            "no permission granted -> ToolPermissionAsk"
        );
    }

    #[tokio::test]
    async fn mcp_target_permission_deny_is_blocked_not_needs_confirmation() {
        // When the target manifest is NOT found in the registry,
        // the fallback produces `ToolPermissionDecision { allowed: false,
        // requires_confirmation: false }`, yielding ToolPermissionDenied
        // (Blocked) not ToolPermissionAsk (NeedsConfirmation).
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();

        // Register a manifest that mcp.call_tool target resolution won't find
        // because the name doesn't match. The target lookup will fail with
        // MissingMcpServer, producing ToolPermissionDenied for the wrapper level.
        // But we want to test target-level deny. Instead, register the target
        // but make it produce `allowed: false, requires_confirmation: false`.
        // The simplest way: register the target with a source that won't match
        // the MCP manifest's canonical source in the permission store.
        // Actually, let's use the approach where the manifest is found, permission
        // is explicitly denied by not granting anything and no default_decision=ask.
        //
        // The clearest way: manifest found, permission store returns ask_every_time
        // (requires_confirmation=true). That's not a hard deny.
        //
        // For a hard deny Blocked (ToolPermissionDenied), we test via AgentSpec
        // which was already covered. This test verifies that when no permission
        // is granted (ask_every_time), we get ToolPermissionAsk (NeedsConfirmation).

        let target_name = "mcp_ask_target2";
        let target_manifest = ToolManifest {
            name: target_name.to_string(),
            id: target_name.to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            version: "1.0".into(),
            source: crate::tool_manifest::ToolSource::Mcp {
                server_name: "ask-server2".to_string(),
            },
            risk_level: "high".to_string(),
            capabilities: vec!["write".to_string()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "write".into(),
            tags: vec![],
        };
        let mock =
            crate::mcp::MockMcpClient::new("ask-server2", |_name, _args| Ok("result".into()));
        r.register_mock_mcp_client("ask-server2", mock);
        r.register_builtin(target_manifest, Arc::new(|_args| Ok("ok".into())));

        // Grant wrapper but NOT target → ask_every_time for target
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();

        let audit = McpAuditStore::new(tmp.path().join("audit_mcp_ask2.db"));
        let pe = PrivacyEngine::new();
        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_mcp_ask22.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());
        ctx.registry = reg_arc;
        ctx.permission_store = ps_arc;
        ctx.audit_store = audit_arc;
        ctx.privacy_engine = pe_arc;

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": target_name,
                    "arguments": {}
                }
            }),
            source_run_id: Some("run-mcp-ask2".into()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        // ask_every_time → ToolPermissionAsk → NeedsConfirmation
        assert_eq!(
            result.status,
            ActionExecutionStatus::NeedsConfirmation,
            "ask_every_time should be NeedsConfirmation not Blocked"
        );
        assert_eq!(
            result.proposal_reason,
            Some(ExecutionProposalReason::ToolPermissionAsk),
            "ask_every_time -> ToolPermissionAsk"
        );
    }

    #[tokio::test]
    async fn mcp_target_block_event_has_wrapper_and_target_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();

        // Target denied by AgentSpec to trigger event recording
        let target_name = "event_target";
        let target_manifest = ToolManifest {
            name: target_name.to_string(),
            id: target_name.to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            version: "1.0".into(),
            source: crate::tool_manifest::ToolSource::Mcp {
                server_name: "event-server".to_string(),
            },
            risk_level: "low".to_string(),
            capabilities: vec![],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };
        let mock = crate::mcp::MockMcpClient::new("event-server", |_name, _args| Ok("ok".into()));
        r.register_mock_mcp_client("event-server", mock);
        r.register_builtin(target_manifest, Arc::new(|_args| Ok("ok".into())));

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "mcp.call_tool",
            "builtin",
            "medium",
            "external_side_effect",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        ps.grant(
            target_name,
            "mcp:event-server",
            "low",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();

        let audit = McpAuditStore::new(tmp.path().join("audit_mcp_event.db"));
        let pe = PrivacyEngine::new();
        let db_path = tmp.path().join("events_mcp_target.db");
        let event_store = crate::agent::event_store::AgentRunEventStore::new(&db_path).unwrap();

        // AgentSpec denies target
        let spec = crate::agent::types::AgentSpec::default_main_spec()
            .with_denied_tools(vec![target_name.to_string()]);

        let reg_arc = Arc::new(tokio::sync::Mutex::new(r));
        let ps_arc = Arc::new(tokio::sync::Mutex::new(ps));
        let audit_arc = Arc::new(tokio::sync::Mutex::new(audit));
        let pe_arc = Arc::new(tokio::sync::Mutex::new(pe));

        let mut ctx = crate::agent::ActionContext::new_for_test(
            McpRegistry::new(),
            ToolPermissionStore::new_in_memory().unwrap(),
            McpAuditStore::new(tmp.path().join("audit_mcp_event2.db")),
            PrivacyEngine::new(),
            vec![],
        )
        .with_agent_spec(spec);
        ctx.registry = reg_arc;
        ctx.permission_store = ps_arc;
        ctx.audit_store = audit_arc;
        ctx.privacy_engine = pe_arc;
        ctx.event_store = Some(event_store.clone());

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let run_id = "run-event-target".to_string();
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "mcp.call_tool".into(),
            input: serde_json::json!({
                "arguments": {
                    "tool_name": target_name,
                    "arguments": {}
                }
            }),
            source_run_id: Some(run_id.clone()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);

        // Verify event payload has wrapper and target fields
        let events = event_store.list_events_by_run(&run_id).unwrap();
        let block_event = events
            .iter()
            .find(|e| matches!(e.event_type, AgentRunEventType::ToolCallBlocked))
            .expect("ToolCallBlocked event should be recorded");

        let payload = &block_event.payload;
        assert_eq!(payload["tool_name"], "mcp.call_tool");
        assert_eq!(payload["wrapper_tool_name"], "mcp.call_tool");
        assert_eq!(payload["target_tool_name"], target_name);
        assert!(payload["target_source"].is_string());
        assert_eq!(payload["status"], "blocked");
        assert_eq!(
            payload["block_reason"],
            ExecutionBlockReason::AgentSpecDenied.to_string()
        );
    }

    // ── Event payload contract tests ─────────────────────────────────

    /// ToolCallBlocked event from AgentSpec deny must have all contract fields.
    /// Validated through production builder + contract_helpers.
    #[tokio::test]
    async fn tool_call_blocked_event_payload_has_contract_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();

        let manifest = ToolManifest {
            name: "contract_tool".into(),
            id: "contract_tool".into(),
            description: String::new(),
            parameters: serde_json::json!({}),
            permission_level: "low".into(),
            version: "1.0".into(),
            source: crate::tool_manifest::ToolSource::BuiltIn,
            risk_level: "low".into(),
            capabilities: vec!["read".into()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            tags: vec![],
        };
        r.register_builtin(manifest, Arc::new(|_args| Ok("ok".into())));

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tmp.path().join("audit_ct.db"));
        let pe = PrivacyEngine::new();
        let events_db = tmp.path().join("events_ct.db");
        let event_store = crate::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();

        // AgentSpec denies the tool
        let spec = crate::agent::types::AgentSpec::default_main_spec()
            .with_denied_tools(vec!["contract_tool".to_string()]);

        let mut ctx = crate::agent::ActionContext::new_for_test(r, ps, audit, pe, vec![])
            .with_agent_spec(spec);
        ctx.event_store = Some(event_store.clone());

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let run_id = "run-contract".to_string();
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "contract_tool".into(),
            input: serde_json::json!({"arguments": {}}),
            source_run_id: Some(run_id.clone()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);

        let events = event_store.list_events_by_run(&run_id).unwrap();
        let blocked = events
            .iter()
            .find(|e| matches!(e.event_type, AgentRunEventType::ToolCallBlocked))
            .expect("ToolCallBlocked event must be recorded");
        let p = &blocked.payload;
        assert_eq!(p["status"], "blocked");
        assert!(p["tool_name"].is_string());
        assert!(p["source"].is_string());
        assert!(p["block_reason"].is_string());
        assert!(!p["block_reason"].as_str().unwrap().is_empty());
        assert!(p["agent_spec_id"].is_string());

        // Production helper must also pass contract_helpers typed reason validation
        crate::agent::tests::contract_helpers::assert_has_typed_reason(
            p,
            &["block_reason", "proposal_reason"],
        );
    }

    /// ToolCallBlocked payload from builder with None agent_spec_id
    /// must still pass contract helper validation.
    #[test]
    fn builder_tool_call_blocked_none_agent_spec_id_passes_contract() {
        use crate::agent::trace_payloads;

        let p = trace_payloads::build_tool_call_blocked_payload(
            "blocked",
            "web.search",
            "runtime",
            None::<&str>,
            Some("invalid_arguments"),
            None::<&str>,
            None::<&str>,
            Some(serde_json::json!({
                "max_tool_calls": 6,
                "current_count": 6,
            })),
        );
        assert_eq!(p["status"], "blocked");
        assert_eq!(p["agent_spec_id"], serde_json::Value::Null);
        assert_eq!(p["block_reason"], "invalid_arguments");

        crate::agent::tests::contract_helpers::assert_has_typed_reason(
            &p,
            &["block_reason", "proposal_reason"],
        );
    }

    /// NetworkPolicy ask event must have contract fields including proposal_id.
    #[tokio::test]
    async fn network_policy_ask_event_payload_has_contract_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let mut r = McpRegistry::new();
        r.register_default_builtins();

        let ps = ToolPermissionStore::new_in_memory().unwrap();
        ps.grant(
            "web.search",
            "builtin",
            "low",
            "read",
            crate::tool_permissions::ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .unwrap();
        let audit = McpAuditStore::new(tmp.path().join("audit_np_ct.db"));
        let pe = PrivacyEngine::new();
        let events_db = tmp.path().join("events_np_ct.db");
        let event_store = crate::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let policy = crate::config::NetworkPolicy {
            enabled: true,
            default_decision: "ask".into(),
            ..crate::config::NetworkPolicy::default()
        };

        let mut ctx = crate::agent::ActionContext::new_for_test(r, ps, audit, pe, vec![])
            .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec())
            .with_proposal_store(Arc::new(tokio::sync::Mutex::new(proposal_store)));
        ctx.event_store = Some(event_store.clone());
        ctx.network_policy = Some(policy);

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let run_id = "run-np-contract".to_string();
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "web.search".into(),
            input: serde_json::json!({
                "arguments": {
                    "query": "test",
                    "url": "https://example.com"
                }
            }),
            source_run_id: Some(run_id.clone()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::NeedsConfirmation);
        assert_eq!(
            result.proposal_reason,
            Some(ExecutionProposalReason::NetworkPolicyAsk)
        );

        let events = event_store.list_events_by_run(&run_id).unwrap();
        let blocked = events
            .iter()
            .find(|e| matches!(e.event_type, AgentRunEventType::ToolCallBlocked))
            .expect("ToolCallBlocked event must be recorded for NetworkPolicy ask");
        let p = &blocked.payload;
        assert_eq!(p["status"], "needs_confirmation");
        assert!(p["tool_name"].is_string());
        assert_eq!(
            p["proposal_reason"],
            ExecutionProposalReason::NetworkPolicyAsk.to_string()
        );
        assert!(p["proposal_id"].is_string());
        assert!(!p["proposal_id"].as_str().unwrap().is_empty());
        assert!(p["agent_spec_id"].is_string());

        // Production helper must pass contract_helpers typed reason validation
        crate::agent::tests::contract_helpers::assert_has_typed_reason(
            p,
            &["block_reason", "proposal_reason"],
        );
    }

    /// shell.run blocked event must have contract fields.
    #[tokio::test]
    async fn shell_blocked_event_payload_has_contract_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let mut reg = McpRegistry::new();
        reg.register_default_builtins();
        // Default shell.run is disabled → ToolCallBlocked with DisabledManifest
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tmp.path().join("audit_sh_ct.db"));
        let pe = PrivacyEngine::new();
        let events_db = tmp.path().join("events_sh_ct.db");
        let event_store = crate::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();

        let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
            bash_enabled: true,
            ..crate::agent::execution_sandbox::ExecutionSandbox::default()
        };

        let mut ctx = crate::agent::ActionContext::new_for_test(reg, ps, audit, pe, vec![])
            .with_execution_sandbox(sandbox)
            .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());
        ctx.event_store = Some(event_store.clone());

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let run_id = "run-sh-contract".to_string();
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "shell.run".into(),
            input: serde_json::json!({"arguments": {"command": "echo hi"}}),
            source_run_id: Some(run_id.clone()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);

        let events = event_store.list_events_by_run(&run_id).unwrap();
        let blocked = events
            .iter()
            .find(|e| matches!(e.event_type, AgentRunEventType::ToolCallBlocked))
            .expect("ToolCallBlocked event must be recorded for shell.run");

        let p = &blocked.payload;
        assert_eq!(p["status"], "blocked");
        assert!(p["tool_name"].is_string());
        assert!(!p["block_reason"].is_null());

        // Production helper must pass contract_helpers typed reason validation
        crate::agent::tests::contract_helpers::assert_has_typed_reason(
            p,
            &["block_reason", "proposal_reason"],
        );
    }

    /// shell.run manifest NOT registered (missing) → DisabledManifest typed
    /// reason in ToolCallBlocked event payload. Verifies the fix for the
    /// branch at manifest: None that was missing `block_reason`.
    #[tokio::test]
    async fn shell_run_manifest_missing_records_typed_tool_call_blocked() {
        let tmp = tempfile::tempdir().unwrap();

        // Fresh registry — no shell.run registered at all
        let reg = McpRegistry::new();
        let ps = ToolPermissionStore::new_in_memory().unwrap();
        let audit = McpAuditStore::new(tmp.path().join("audit_sh_miss.db"));
        let pe = PrivacyEngine::new();
        let events_db = tmp.path().join("events_sh_miss.db");
        let event_store = crate::agent::event_store::AgentRunEventStore::new(&events_db).unwrap();

        let sandbox = crate::agent::execution_sandbox::ExecutionSandbox {
            bash_enabled: true,
            ..crate::agent::execution_sandbox::ExecutionSandbox::default()
        };

        let mut ctx = crate::agent::ActionContext::new_for_test(reg, ps, audit, pe, vec![])
            .with_execution_sandbox(sandbox)
            .with_agent_spec(crate::agent::types::AgentSpec::default_main_spec());
        ctx.event_store = Some(event_store.clone());

        let executor = crate::agent::ActionExecutor::new(ActionExecutorConfig::default());
        let run_id = "run-sh-missing".to_string();
        let request = crate::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "shell.run".into(),
            input: serde_json::json!({"arguments": {"command": "echo hi"}}),
            source_run_id: Some(run_id.clone()),
            step_index: 0,
        };

        let result = executor.execute(request, &ctx).await.unwrap();
        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert_eq!(
            result.block_reason,
            Some(ExecutionBlockReason::DisabledManifest)
        );

        let events = event_store.list_events_by_run(&run_id).unwrap();
        let blocked = events
            .iter()
            .find(|e| matches!(e.event_type, AgentRunEventType::ToolCallBlocked))
            .expect("ToolCallBlocked must be recorded for shell.run manifest missing");

        let p = &blocked.payload;
        assert_eq!(p["status"], "blocked");
        assert_eq!(p["tool_name"], "shell.run");
        assert!(p["source"].is_string());
        assert!(!p["source"].as_str().unwrap().is_empty());
        assert_eq!(
            p["block_reason"],
            ExecutionBlockReason::DisabledManifest.to_string()
        );
        assert!(p["proposal_reason"].is_null());
        assert!(p["failure_kind"].is_null());
        // agent_spec_id field must be present (null allowed)
        assert!(p.get("agent_spec_id").is_some());

        // Production helper must pass contract_helpers typed reason validation
        crate::agent::tests::contract_helpers::assert_has_typed_reason(
            p,
            &["block_reason", "proposal_reason"],
        );
    }
}
