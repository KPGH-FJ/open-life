use crate::agent::types::{
    AgentAction, AgentObservation, AgentProposal, ProposalSource, ProposalType, RiskLevel,
    ToolActionScope,
};
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
    pub safe_paths: &'a [String],
    pub life_model: Option<&'a crate::life_model::LifeModel>,
    pub memory_store: Option<&'a crate::memory::MemoryStore>,
    pub proposal_store: Option<&'a crate::agent::ProposalStore>,
    pub agent_run_store: Option<&'a crate::agent::AgentRunStore>,
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

                ctx.permission_store
                    .check(
                        &manifest.name,
                        &source,
                        &manifest.risk_level,
                        &manifest.action_type,
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
        let blocked = manifest
            .as_ref()
            .is_none_or(|m| !m.enabled || m.declarative_only)
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

        // 5. Safe Paths check for filesystem tools
        if let Some(ref m) = manifest {
            if m.capabilities.contains(&"filesystem".to_string()) {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
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
            .expect("manifest exists when execution is not blocked");
        let result = if manifest_ref.tags.contains(&"core_os".to_string()) {
            self.execute_core_os_tool(tool_name, &args, ctx)
                .unwrap_or_else(|e| ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                })
        } else if manifest_ref.tags.contains(&"execution".to_string()) {
            self.execute_execution_tool(tool_name, &args, ctx)
                .unwrap_or_else(|e| ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                })
        } else {
            self.call_tool_internal(manifest_ref, args.clone(), ctx.registry, ctx.audit_store)
        };

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

    /// Execute a Core OS tool with real data from LifeModel.
    fn execute_core_os_tool(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ActionExecutionContext<'_>,
    ) -> Result<ToolCallInternalResult> {
        let output = match tool_name {
            "life_model.read" => {
                let life_model = ctx.life_model.ok_or_else(|| {
                    anyhow::anyhow!(
                        "LifeModel not available in execution context for core_os tool '{}'",
                        tool_name
                    )
                })?;
                serde_json::to_string_pretty(&life_model)
                    .unwrap_or_else(|_| "{\"error\": \"serialization failed\"}".to_string())
            }
            "goal.read" => {
                let life_model = ctx.life_model.ok_or_else(|| {
                    anyhow::anyhow!(
                        "LifeModel not available in execution context for core_os tool '{}'",
                        tool_name
                    )
                })?;
                serde_json::to_string_pretty(&life_model.goals)
                    .unwrap_or_else(|_| "{\"error\": \"serialization failed\"}".to_string())
            }
            "state.read" => {
                let life_model = ctx.life_model.ok_or_else(|| {
                    anyhow::anyhow!(
                        "LifeModel not available in execution context for core_os tool '{}'",
                        tool_name
                    )
                })?;
                serde_json::to_string_pretty(&life_model.state)
                    .unwrap_or_else(|_| "{\"error\": \"serialization failed\"}".to_string())
            }
            "memory.search" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");

                if let Some(memory_store) = ctx.memory_store {
                    match memory_store.search_text_memories(None, query, 10) {
                        Ok(hits) => {
                            let results: Vec<_> = hits
                                .into_iter()
                                .map(|hit| {
                                    serde_json::json!({
                                        "content": hit.chunk.content,
                                        "source": hit.chunk.source,
                                        "relevance": hit.relevance_score,
                                        "tier": hit.chunk.tier,
                                    })
                                })
                                .collect();
                            serde_json::json!({
                                "status": "success",
                                "query": query,
                                "hits": results,
                                "count": results.len()
                            })
                            .to_string()
                        }
                        Err(e) => serde_json::json!({
                            "status": "error",
                            "reason": format!("Search failed: {}", e),
                            "hits": []
                        })
                        .to_string(),
                    }
                } else {
                    serde_json::json!({
                        "status": "unavailable",
                        "reason": "MemoryStore not available in execution context",
                        "hits": []
                    })
                    .to_string()
                }
            }
            "tool.list_available" => {
                let manifests = ctx.registry.list_manifests();
                let tools: Vec<_> = manifests
                    .into_iter()
                    .filter(|m| m.enabled && !m.declarative_only)
                    .map(|m| {
                        serde_json::json!({
                            "name": m.name,
                            "description": m.description,
                            "source": m.source.to_string(),
                            "action_type": m.action_type,
                            "risk_level": m.risk_level,
                            "capabilities": m.capabilities,
                        })
                    })
                    .collect();
                serde_json::json!({ "tools": tools }).to_string()
            }
            "proposal.list" => {
                if let Some(store) = ctx.proposal_store {
                    let proposals = store.list_pending_proposals(20)?;
                    serde_json::to_string(&proposals)
                        .unwrap_or_else(|_| "{\"error\":\"serialization failed\"}".to_string())
                } else {
                    serde_json::json!({
                        "status": "unavailable",
                        "reason": "ProposalStore not available in execution context",
                        "proposals": []
                    })
                    .to_string()
                }
            }
            "agent_run.lookup" => {
                let run_id = args
                    .get("run_id")
                    .or_else(|| args.get("runId"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if run_id.is_empty() {
                    serde_json::json!({
                        "status": "error",
                        "reason": "agent_run.lookup requires run_id"
                    })
                    .to_string()
                } else if let Some(store) = ctx.agent_run_store {
                    match store.get_run(run_id)? {
                        Some(run) => serde_json::to_string(&run)
                            .unwrap_or_else(|_| "{\"error\":\"serialization failed\"}".to_string()),
                        None => serde_json::json!({
                            "status": "not_found",
                            "run_id": run_id
                        })
                        .to_string(),
                    }
                } else {
                    serde_json::json!({
                        "status": "unavailable",
                        "reason": "AgentRunStore not available in execution context"
                    })
                    .to_string()
                }
            }
            "life_model.propose_patch" => self
                .create_core_os_proposal(
                    ctx,
                    ProposalType::LifeModelUpdate,
                    args.get("path")
                        .and_then(Value::as_str)
                        .unwrap_or("life_model"),
                    args.clone(),
                    "Agent proposed a LifeModel patch via Core OS tool.",
                    RiskLevel::High,
                )?
                .to_string(),
            "memory.propose_write" => self
                .create_core_os_proposal(
                    ctx,
                    ProposalType::MemoryWrite,
                    "memory.candidates",
                    args.clone(),
                    "Agent proposed a MemoryWrite via Core OS tool.",
                    RiskLevel::Medium,
                )?
                .to_string(),
            "memory.propose_archive" => self
                .create_core_os_proposal(
                    ctx,
                    ProposalType::MemoryArchive,
                    "memory.archive",
                    args.clone(),
                    "Agent proposed a MemoryArchive via Core OS tool.",
                    RiskLevel::Medium,
                )?
                .to_string(),
            _ => {
                return Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!("Unknown core_os tool: {}", tool_name)),
                });
            }
        };

        Ok(ToolCallInternalResult {
            success: true,
            output: Some(output),
            error: None,
        })
    }

    fn create_core_os_proposal(
        &self,
        ctx: &ActionExecutionContext<'_>,
        proposal_type: ProposalType,
        affected_path: &str,
        after: Value,
        reason: &str,
        risk: RiskLevel,
    ) -> Result<Value> {
        let store = ctx
            .proposal_store
            .ok_or_else(|| anyhow::anyhow!("ProposalStore not available in execution context"))?;
        let proposal = AgentProposal::new(
            proposal_type,
            affected_path,
            after,
            reason,
            0.8,
            risk,
            ProposalSource::Manual,
        );
        let proposal_id = proposal.id.clone();
        store.create_proposal(&proposal)?;
        Ok(serde_json::json!({
            "status": "proposal_created",
            "proposal_id": proposal_id,
            "proposal_type": proposal.proposal_type.to_string(),
            "affected_path": proposal.affected_path,
        }))
    }

    /// Execute an Execution tool (file.read, web.fetch, etc.).
    fn execute_execution_tool(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ActionExecutionContext<'_>,
    ) -> Result<ToolCallInternalResult> {
        match tool_name {
            "file.read" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument for file.read"))?;

                // Validate path is within safe_paths
                if !is_path_in_safe_paths(path, ctx.safe_paths) {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(filesystem_access_error(path, ctx.safe_paths)),
                    });
                }

                // Check file size before reading
                let metadata = std::fs::metadata(path)
                    .map_err(|e| anyhow::anyhow!("Failed to read file metadata: {}", e))?;
                let max_size = 100 * 1024; // 100KB limit
                if metadata.len() > max_size {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "File size ({} bytes) exceeds maximum allowed ({} bytes)",
                            metadata.len(),
                            max_size
                        )),
                    });
                }

                match std::fs::read_to_string(path) {
                    Ok(content) => Ok(ToolCallInternalResult {
                        success: true,
                        output: Some(content),
                        error: None,
                    }),
                    Err(e) => Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to read file '{}': {}", path, e)),
                    }),
                }
            }
            "web.fetch" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'url' argument for web.fetch"))?;

                // Validate URL scheme
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "Invalid URL scheme. Only http:// and https:// are allowed, got: {}",
                            url
                        )),
                    });
                }

                // Block private IP ranges and localhost
                if is_private_url(url) {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "URL '{}' points to a private/internal address and is blocked for security",
                            url
                        )),
                    });
                }

                // Perform HTTP GET with timeout
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .build()
                    .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

                match client.get(url).send() {
                    Ok(response) => {
                        let status = response.status();
                        if status.is_success() {
                            match response.text() {
                                Ok(text) => {
                                    // Convert HTML to plain text if it looks like HTML
                                    let text = if text.trim_start().starts_with('<') {
                                        html_to_text(&text)
                                    } else {
                                        text
                                    };
                                    // Limit response size
                                    let max_length = 50_000; // 50KB
                                    let truncated = if text.len() > max_length {
                                        format!(
                                            "{}\n\n[Truncated: response exceeded {} characters]",
                                            &text[..max_length],
                                            max_length
                                        )
                                    } else {
                                        text
                                    };
                                    Ok(ToolCallInternalResult {
                                        success: true,
                                        output: Some(truncated),
                                        error: None,
                                    })
                                }
                                Err(e) => Ok(ToolCallInternalResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!("Failed to read response body: {}", e)),
                                }),
                            }
                        } else {
                            Ok(ToolCallInternalResult {
                                success: false,
                                output: None,
                                error: Some(format!(
                                    "HTTP {}: {}",
                                    status.as_u16(),
                                    status.canonical_reason().unwrap_or("Unknown error")
                                )),
                            })
                        }
                    }
                    Err(e) => Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!("HTTP request failed: {}", e)),
                    }),
                }
            }
            "file.write_proposal" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

                // Validate path is within safe_paths
                if !path.is_empty() && !is_path_in_safe_paths(path, ctx.safe_paths) {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(filesystem_access_error(path, ctx.safe_paths)),
                    });
                }

                // Return a structured proposal for file write
                let proposal = serde_json::json!({
                    "proposal_type": "external_write_action",
                    "external_write_action": {
                        "path": path,
                        "content": content,
                        "content_preview": if content.len() > 200 {
                            format!("{}... [truncated]", &content[..200])
                        } else {
                            content.to_string()
                        },
                        "content_length": content.len()
                    },
                    "path": path,
                    "content": content,
                    "content_preview": if content.len() > 200 {
                        format!("{}... [truncated]", &content[..200])
                    } else {
                        content.to_string()
                    },
                    "content_length": content.len(),
                    "requires_confirmation": true,
                    "reason": format!("Proposed file write to '{}'", path),
                });

                Ok(ToolCallInternalResult {
                    success: true,
                    output: Some(proposal.to_string()),
                    error: None,
                })
            }
            _ => Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!("Unknown execution tool: {}", tool_name)),
            }),
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

/// Check if a path is within the safe paths list.
/// Returns false if safe_paths is empty: filesystem access must be explicitly scoped.
fn is_path_in_safe_paths(path: &str, safe_paths: &[String]) -> bool {
    if safe_paths.is_empty() {
        return false;
    }

    let path = std::path::Path::new(path);
    let canonical_path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // If canonicalize fails, try with the original path
            // This handles paths that don't exist yet
            path.to_path_buf()
        }
    };

    for safe in safe_paths {
        let safe_path = std::path::Path::new(safe);
        let canonical_safe = match safe_path.canonicalize() {
            Ok(p) => p,
            Err(_) => safe_path.to_path_buf(),
        };

        if canonical_path.starts_with(&canonical_safe) {
            return true;
        }
    }

    false
}

fn filesystem_access_error(path: &str, safe_paths: &[String]) -> String {
    if safe_paths.is_empty() {
        "No safe paths configured for filesystem access".to_string()
    } else {
        format!("Path '{}' is not in safe paths list", path)
    }
}

/// Check if a URL points to a private/internal address.
/// Blocks localhost, private IP ranges, and link-local addresses.
fn is_private_url(url: &str) -> bool {
    // Quick check for localhost variants
    if url.contains("://localhost")
        || url.contains("://127.")
        || url.contains("://10.")
        || url.contains("://192.168.")
        || url.contains("://169.254.")
        || url.contains("://172.")
    {
        // More precise check for 172.16.0.0/12
        if url.contains("://172.") {
            let after_172 = url.split("://172.").nth(1).unwrap_or("");
            let second_octet: u32 = after_172
                .split('.')
                .next()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
            if (16..=31).contains(&second_octet) {
                return true;
            }
        } else {
            return true;
        }
    }

    // Check for private IP patterns more thoroughly
    if let Some(host_start) = url.find("://") {
        let host_part = &url[host_start + 3..];
        let host = host_part.split('/').next().unwrap_or(host_part);
        let host = host.split(':').next().unwrap_or(host);

        if host == "localhost" {
            return true;
        }

        // Check for numeric IPs
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(ipv4) => {
                    return ipv4.is_loopback() || ipv4.is_private();
                }
                std::net::IpAddr::V6(ipv6) => {
                    return ipv6.is_loopback();
                }
            }
        }
    }

    false
}

/// Simple HTML to plain text converter.
/// Strips tags and converts common elements to readable text.
fn html_to_text(html: &str) -> String {
    let mut text = html.to_string();

    // Replace common block elements with newlines
    let block_replacements = [
        ("<p>", "\n\n"),
        ("</p>", ""),
        ("<div>", "\n"),
        ("</div>", ""),
        ("<br>", "\n"),
        ("<br/>", "\n"),
        ("<li>", "\n- "),
        ("</li>", ""),
        ("<h1>", "\n\n# "),
        ("</h1>", "\n\n"),
        ("<h2>", "\n\n## "),
        ("</h2>", "\n\n"),
        ("<h3>", "\n\n### "),
        ("</h3>", "\n\n"),
        ("<h4>", "\n\n#### "),
        ("</h4>", "\n\n"),
        ("<h5>", "\n\n##### "),
        ("</h5>", "\n\n"),
        ("<h6>", "\n\n###### "),
        ("</h6>", "\n\n"),
        ("<ul>", "\n"),
        ("</ul>", "\n"),
        ("<ol>", "\n"),
        ("</ol>", "\n"),
        ("<pre>", "\n\n```\n"),
        ("</pre>", "\n```\n\n"),
        ("<code>", " `"),
        ("</code>", "` "),
        ("<strong>", " **"),
        ("</strong>", "** "),
        ("<b>", " **"),
        ("</b>", "** "),
        ("<em>", " *"),
        ("</em>", "* "),
        ("<i>", " *"),
        ("</i>", "* "),
    ];

    for (tag, replacement) in &block_replacements {
        text = text.replace(tag, replacement);
    }

    // Remove remaining HTML tags
    let tag_regex =
        regex::Regex::new(r"<[^>]+>").unwrap_or_else(|_| regex::Regex::new(r"").unwrap());
    text = tag_regex.replace_all(&text, "").to_string();

    // Decode common HTML entities
    let entities = [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&nbsp;", " "),
        ("&mdash;", "—"),
        ("&ndash;", "–"),
        ("&hellip;", "…"),
    ];

    for (entity, decoded) in &entities {
        text = text.replace(entity, decoded);
    }

    // Clean up excessive whitespace
    text = text.replace("\n\n\n", "\n\n");
    text = text.replace("  ", " ");

    text.trim().to_string()
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
            safe_paths: &[],
            life_model: None,
            memory_store: None,
            proposal_store: None,
            agent_run_store: None,
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
                declarative_only: false,
                action_type: "write".into(),
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
                "write",
                ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let mut ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);
        let safe_dir = tempfile::tempdir().unwrap();
        let safe_path = safe_dir.path().to_string_lossy().to_string();
        let safe_paths = [safe_path];
        let target_path = safe_dir.path().join("a.txt");
        std::fs::write(&target_path, "").unwrap();
        ctx.safe_paths = &safe_paths;

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "write_file".into(),
                    input: serde_json::json!({"arguments": {"path": target_path}}),
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
                declarative_only: false,
                action_type: "write".into(),
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
                "write",
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
                declarative_only: false,
                action_type: "write".into(),
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

    #[test]
    fn core_os_life_model_read_returns_life_model_json() {
        // Use default registry which already has core_os tools registered
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "life_model.read",
                "builtin",
                "low",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::Allow,
                None,
            )
            .unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let life_model = crate::life_model::LifeModel::default();
        let mut ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);
        ctx.life_model = Some(&life_model);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "life_model.read".into(),
                    input: serde_json::json!({"arguments": {}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Succeeded);
        let output = result.observation.content;
        assert!(output.contains("identity") || output.contains("goals"));
    }

    #[test]
    fn core_os_tool_without_life_model_returns_error() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "life_model.read",
                "builtin",
                "low",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::Allow,
                None,
            )
            .unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);
        // life_model is None by default

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "life_model.read".into(),
                    input: serde_json::json!({"arguments": {}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert!(result
            .observation
            .content
            .contains("LifeModel not available"));
    }

    #[test]
    fn core_os_tool_list_available_returns_tools() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "tool.list_available",
                "builtin",
                "low",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::Allow,
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
                    target: "tool.list_available".into(),
                    input: serde_json::json!({"arguments": {}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Succeeded);
        let output = result.observation.content;
        assert!(output.contains("tools"));
    }

    #[test]
    fn execution_tool_file_read_reads_file_content() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "file.read",
                "builtin",
                "low",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::Allow,
                None,
            )
            .unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let mut ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);

        // Create a temp file to read
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "Hello, OpenLife!").unwrap();
        let safe_path = temp_file
            .path()
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let safe_paths = [safe_path];
        ctx.safe_paths = &safe_paths;

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "file.read".into(),
                    input: serde_json::json!({"arguments": {"path": temp_file.path()}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Succeeded);
        assert_eq!(result.observation.content, "Hello, OpenLife!");
    }

    #[test]
    fn execution_tool_file_read_blocks_when_safe_paths_empty() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "file.read",
                "builtin",
                "low",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::Allow,
                None,
            )
            .unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);

        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "blocked").unwrap();

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "file.read".into(),
                    input: serde_json::json!({"arguments": {"path": temp_file.path()}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert!(result
            .observation
            .content
            .contains("No safe paths configured for filesystem access"));
    }

    #[test]
    fn execution_tool_file_read_blocks_path_outside_safe_paths() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "file.read",
                "builtin",
                "low",
                "read",
                crate::tool_permissions::ToolPermissionPolicy::Allow,
                None,
            )
            .unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let mut ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);
        // Set a specific safe path
        let safe_dir = std::env::temp_dir().to_string_lossy().to_string();
        let safe_paths = [safe_dir];
        ctx.safe_paths = &safe_paths;

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "file.read".into(),
                    input: serde_json::json!({"arguments": {"path": "/etc/passwd"}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Blocked);
        assert!(result.observation.content.contains("not in safe paths"));
    }

    #[test]
    fn execution_tool_web_fetch_blocks_private_url() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "web.fetch",
                "builtin",
                "medium",
                "network",
                crate::tool_permissions::ToolPermissionPolicy::Allow,
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
                    target: "web.fetch".into(),
                    input: serde_json::json!({"arguments": {"url": "http://localhost:8080/admin"}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert!(result
            .observation
            .content
            .contains("private/internal address"));
    }

    #[test]
    fn is_private_url_blocks_localhost_and_private_ips() {
        assert!(is_private_url("http://localhost:8080"));
        assert!(is_private_url("http://127.0.0.1/api"));
        assert!(is_private_url("http://10.0.0.1/admin"));
        assert!(is_private_url("http://192.168.1.1/"));
        assert!(is_private_url("http://172.16.0.1/"));
        assert!(is_private_url("http://169.254.1.1/"));
        assert!(!is_private_url("https://example.com/page"));
        assert!(!is_private_url("https://api.openai.com/v1"));
    }
}
