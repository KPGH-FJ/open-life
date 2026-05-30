use crate::mcp::McpArgumentInspection;
use crate::mcp::McpRegistry;
use crate::mcp_audit::McpAuditStore;
use crate::tool_manifest::{ToolManifest, ToolSource};
use crate::tool_permissions::ToolPermissionDecision;
use anyhow::Result;
use serde_json::Value;

use super::helpers::{
    canonical_tool_source, ensure_external_write_content_size, external_write_content_preview,
    filesystem_access_error, hs_requires_external_write_proposal, is_direct_external_write_tool,
    is_path_in_safe_paths, is_proposal_generation_tool, minimized_external_write_arguments,
    normalize_tool_name, should_mark_needs_confirmation, ToolCallInternalResult,
};
use super::ActionExecutionContext;
use super::ActionExecutionResult;
use super::ActionExecutionStatus;
use super::AgentActionRequest;
use crate::agent::policy_store::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST;
use crate::agent::types::{
    AgentAction, AgentObservation, AgentProposal, ProposalSource, ProposalType, RiskLevel,
    ToolActionScope,
};
use ring::digest::{digest, SHA256};

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

        if let Some(m) = manifest.as_ref().filter(|m| {
            hs_requires_external_write_proposal(ctx) && is_direct_external_write_tool(m)
        }) {
            if let Some(result) =
                self.create_external_write_action_proposal(&request, ctx, tool_name, &args, m)
            {
                return result;
            }

            let forced_decision = ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "proposal_required".into(),
                reason: "HS proposal-first policy requires an ExternalWriteAction proposal before direct external write".into(),
                policy_id: Some(BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.into()),
            };
            let (action, observation) = self.build_blocked_action_observation(
                tool_name,
                &args,
                &inspection,
                &forced_decision,
                manifest.as_ref(),
                &request,
            );
            return Ok(ActionExecutionResult {
                action,
                observation,
                status: ActionExecutionStatus::NeedsConfirmation,
                stop_reason: Some("hs_external_write_proposal_first".into()),
            });
        }

        let permission_blocks =
            !is_proposal_tool && (decision.requires_confirmation || !decision.allowed);
        let inspection_blocks =
            !is_proposal_tool && inspection.requires_confirmation && inspection.pii_found;
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

            // HS proposal-first policies convert blocked direct writes into
            // user-reviewable ExternalWriteAction proposals.
            if needs_confirmation {
                if let Some(m) = manifest.as_ref().filter(|m| {
                    hs_requires_external_write_proposal(ctx) && is_direct_external_write_tool(m)
                }) {
                    if let Some(result) = self
                        .create_external_write_action_proposal(&request, ctx, tool_name, &args, m)
                    {
                        return result;
                    }
                }
            }

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

        let (mut action, mut observation) = self.build_success_action_observation(
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
                    if error.contains("hs_external_write_proposal_first") {
                        action.status = "needs_confirmation".to_string();
                        action.permission_decision = Some("proposal_required".into());
                        if let Some(structured) = observation.structured_result.as_mut() {
                            if let Some(object) = structured.as_object_mut() {
                                object.insert(
                                    "status".into(),
                                    serde_json::json!("needs_confirmation"),
                                );
                                object.insert(
                                    "requires_confirmation".into(),
                                    serde_json::json!(true),
                                );
                                object.insert(
                                    "permission_decision".into(),
                                    serde_json::json!("proposal_required"),
                                );
                                object.insert("proposal_required".into(), serde_json::json!(true));
                            }
                        }
                        return Ok(ActionExecutionResult {
                            action,
                            observation,
                            status: ActionExecutionStatus::NeedsConfirmation,
                            stop_reason: Some("hs_external_write_proposal_first".into()),
                        });
                    }
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

    pub(crate) fn create_external_write_action_proposal(
        &self,
        request: &AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        tool_name: &str,
        args: &Value,
        manifest: &ToolManifest,
    ) -> Option<anyhow::Result<ActionExecutionResult>> {
        let proposal_id = match self
            .create_external_write_action_proposal_record(request, ctx, tool_name, args, manifest)
        {
            Some(Ok(proposal_id)) => proposal_id,
            Some(Err(e)) => return Some(Err(e)),
            None => return None,
        };

        let mut result = self.build_proposal_required_action(
            request.clone(),
            &format!(
                "{}: created ExternalWriteAction proposal (id: {}) for HS proposal-first policy",
                tool_name, proposal_id
            ),
        );
        result.stop_reason = Some("hs_external_write_proposal_first".into());

        Some(Ok(result))
    }

    pub(crate) fn create_external_write_action_proposal_record(
        &self,
        request: &AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        tool_name: &str,
        args: &Value,
        manifest: &ToolManifest,
    ) -> Option<anyhow::Result<String>> {
        let proposal_store = ctx.proposal_store?;
        let source = canonical_tool_source(manifest);
        let server = match &manifest.source {
            ToolSource::Mcp { server_name } => Some(server_name.clone()),
            _ => None,
        };
        let risk_level = manifest.risk_level.clone();
        let action_type = manifest.action_type.clone();
        let capabilities = manifest.capabilities.clone();
        let path = args
            .get("path")
            .or_else(|| args.get("file_path"))
            .or_else(|| args.get("destination"))
            .and_then(Value::as_str)
            .unwrap_or(tool_name);
        let content_value = args
            .get("content")
            .or_else(|| args.get("body"))
            .or_else(|| args.get("data"))
            .cloned()
            .unwrap_or(Value::Null);
        let content_text = content_value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| content_value.to_string());
        let hash = digest(&SHA256, content_text.as_bytes());
        let content_hash: String = hash.as_ref().iter().map(|b| format!("{:02x}", b)).collect();
        let size_bytes = content_text.len();
        if let Err(e) = ensure_external_write_content_size(&content_text) {
            return Some(Err(e));
        }
        let content_preview = external_write_content_preview(&content_text);
        let minimized_arguments =
            minimized_external_write_arguments(args, &content_hash, size_bytes, &content_preview);
        let operation = if !path.is_empty() && std::path::Path::new(path).exists() {
            "overwrite"
        } else {
            "create"
        };

        let mut proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("{}.{}", source, path),
            serde_json::json!({
                "tool_name": tool_name,
                "tool_id": manifest.id,
                "source": source,
                "server": server,
                "arguments": minimized_arguments,
                "path": path,
                "content": content_text,
                "content_preview": content_preview,
                "content_hash": content_hash,
                "size_bytes": size_bytes,
                "operation": operation,
                "risk_level": risk_level,
                "action_type": action_type,
                "capabilities": capabilities,
                "requires_confirmation": true,
                "hs_policy_id": BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST,
            }),
            &format!(
                "Agent proposed external write via '{}' ({})",
                tool_name, operation
            ),
            0.9,
            RiskLevel::High,
            ProposalSource::Manual,
        );

        if let Some(ref run_id) = request.source_run_id {
            proposal.run_id = Some(run_id.clone());
        }
        let proposal_id = proposal.id.clone();

        if let Err(e) = proposal_store.create_proposal(&proposal) {
            eprintln!(
                "[warn] Failed to create ExternalWriteAction Proposal for {}: {}",
                tool_name, e
            );
            return None;
        }

        Some(Ok(proposal_id))
    }
}
