use crate::agent::types::{
    AgentAction, AgentObservation, AgentProposal, ProposalSource, ProposalType, RiskLevel,
    ToolActionScope,
};
use crate::mcp::{McpArgumentInspection, McpRegistry};
use crate::mcp_audit::McpAuditStore;
use crate::privacy::PrivacyEngine;
use crate::tool_manifest::{ToolManifest, ToolSource};
use crate::tool_permissions::{ToolPermissionDecision, ToolPermissionStore};
use anyhow::Result;
use ring::digest::{digest, SHA256};
use serde_json::Value;
use std::net::ToSocketAddrs;

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

        // 4. Determine if blocked
        let inspection_blocks = inspection.requires_confirmation && inspection.pii_found;
        let blocked = manifest
            .as_ref()
            .is_none_or(|m| !m.enabled || m.declarative_only)
            || inspection_blocks
            || decision.requires_confirmation
            || !decision.allowed;

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
            if let Some(target_name) = args.get("tool_name").and_then(|v| v.as_str()) {
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

    fn call_tool_internal(
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
                    .map(|m| {
                        // Determine execution status
                        let execution_status = if !m.enabled {
                            "disabled"
                        } else if m.declarative_only {
                            "declarative_only"
                        } else if m.requires_confirmation
                            || m.risk_level == "high"
                            || m.capabilities.iter().any(|c| {
                                matches!(
                                    c.as_str(),
                                    "write"
                                        | "filesystem"
                                        | "memory"
                                        | "lifemodel"
                                        | "external_side_effect"
                                )
                            })
                        {
                            "needs_permission"
                        } else {
                            "executable"
                        };

                        serde_json::json!({
                            "name": m.name,
                            "description": m.description,
                            "source": m.source.to_string(),
                            "action_type": m.action_type,
                            "risk_level": m.risk_level,
                            "capabilities": m.capabilities,
                            "execution_status": execution_status,
                            "enabled": m.enabled,
                            "declarative_only": m.declarative_only,
                            "requires_confirmation": m.requires_confirmation,
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
        request: &AgentActionRequest,
    ) -> Result<ToolCallInternalResult> {
        // Check network policy for web tools
        if matches!(tool_name, "web.fetch" | "web.search") {
            if let Some(policy) = ctx.network_policy {
                if !policy.enabled {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(
                            "Network tools are disabled by policy. Enable network access in Settings to use web tools.".to_string(),
                        ),
                    });
                }

                // Check tool override
                if let Some(override_decision) = policy.tool_overrides.get(tool_name) {
                    if override_decision == "deny" {
                        return Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(format!(
                                "Tool '{}' is denied by network policy override",
                                tool_name
                            )),
                        });
                    }
                }

                // For web.fetch, check domain allowlist/denylist
                if tool_name == "web.fetch" {
                    if let Some(url) = args.get("url").and_then(|v| v.as_str()) {
                        if let Some(host) = extract_host_from_url(url) {
                            // Check denylist first
                            if policy.domain_denylist.iter().any(|d| host.ends_with(d)) {
                                return Ok(ToolCallInternalResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!(
                                        "Domain '{}' is in the network denylist",
                                        host
                                    )),
                                });
                            }
                            // If allowlist is not empty, only allow listed domains
                            if !policy.domain_allowlist.is_empty()
                                && !policy.domain_allowlist.iter().any(|d| host.ends_with(d))
                            {
                                return Ok(ToolCallInternalResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!(
                                        "Domain '{}' is not in the network allowlist",
                                        host
                                    )),
                                });
                            }
                        }
                    }
                }
            }
        }

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

                fetch_url_on_worker_thread(url)
            }
            "web.search" => {
                let query = args
                    .get("query")
                    .or_else(|| args.get("q"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'query' argument for web.search"))?;
                let max_results = args
                    .get("max_results")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5)
                    .clamp(1, 10) as usize;

                if query.trim().is_empty() {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some("Search query cannot be empty".to_string()),
                    });
                }

                search_web_on_worker_thread(query, max_results)
            }
            "mcp.call_tool" => {
                let target_tool_name =
                    args.get("tool_name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            anyhow::anyhow!("Missing 'tool_name' argument for mcp.call_tool")
                        })?;
                let server = args.get("server").and_then(|v| v.as_str());
                let tool_args = args
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));

                // 1. Find target manifest
                let manifests = ctx.registry.list_manifests();
                let mut target_manifests: Vec<_> = manifests
                    .into_iter()
                    .filter(|m| m.name == target_tool_name || m.id == target_tool_name)
                    .collect();

                let target_manifest = if let Some(server_name) = server {
                    target_manifests
                        .into_iter()
                        .find(|m| matches!(&m.source, ToolSource::Mcp { server_name: s } if s == server_name))
                        .ok_or_else(|| anyhow::anyhow!(
                            "MCP tool '{}' not found on server '{}'",
                            target_tool_name,
                            server_name
                        ))?
                } else if target_manifests.len() == 1 {
                    target_manifests.remove(0)
                } else if target_manifests.is_empty() {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!("MCP tool '{}' not found", target_tool_name)),
                    });
                } else {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "Multiple MCP tools named '{}'. Please specify 'server' parameter.",
                            target_tool_name
                        )),
                    });
                };

                // 2. Check permission using target tool's canonical scope
                let target_source = canonical_tool_source(&target_manifest);
                let target_decision = ctx
                    .permission_store
                    .check(
                        &target_manifest.name,
                        &target_source,
                        &target_manifest.risk_level,
                        &target_manifest.action_type,
                        &target_manifest.capabilities,
                    )
                    .unwrap_or(ToolPermissionDecision {
                        allowed: false,
                        requires_confirmation: true,
                        decision: "ask_every_time".into(),
                        reason: "permission check failed".into(),
                        policy_id: None,
                    });

                if !target_decision.allowed || target_decision.requires_confirmation {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "MCP tool '{}' blocked: {} (decision: {})",
                            target_tool_name, target_decision.reason, target_decision.decision
                        )),
                    });
                }

                // 3. Inspect PII using target tool
                let inspection = ctx
                    .registry
                    .inspect_call_arguments(&target_manifest.name, &tool_args);
                if inspection.requires_confirmation && inspection.pii_found {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "MCP tool '{}' blocked: PII detected in arguments",
                            target_tool_name
                        )),
                    });
                }

                // 4. Execute target tool
                Ok(self.call_tool_internal(
                    &target_manifest,
                    tool_args,
                    ctx.registry,
                    ctx.audit_store,
                    inspection.pii_found,
                ))
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

                // Compute content metadata
                let hash = digest(&SHA256, content.as_bytes());
                let content_hash: String =
                    hash.as_ref().iter().map(|b| format!("{:02x}", b)).collect();
                let size_bytes = content.len();
                let operation = if std::path::Path::new(path).exists() {
                    "overwrite"
                } else {
                    "create"
                };
                let content_preview = if content.len() > 4000 {
                    let preview: String = content.chars().take(4000).collect();
                    format!(
                        "{}... [truncated {} bytes]",
                        preview,
                        content.len() - preview.len()
                    )
                } else {
                    content.to_string()
                };

                // Auto-create ExternalWriteAction Proposal if path is non-empty
                let mut proposal_id: Option<String> = None;
                if !path.is_empty() {
                    if let Some(proposal_store) = ctx.proposal_store {
                        let mut proposal = AgentProposal::new(
                            ProposalType::ExternalWriteAction,
                            &format!("filesystem.{}", path),
                            serde_json::json!({
                                "path": path,
                                "content": content,
                                "content_preview": content_preview,
                                "content_hash": content_hash,
                                "size_bytes": size_bytes,
                                "encoding": "utf-8",
                                "operation": operation,
                            }),
                            &format!("Agent proposed file write to '{}' ({})", path, operation),
                            0.9,
                            RiskLevel::High,
                            ProposalSource::Manual,
                        );
                        // Link to source run if available
                        if let Some(ref run_id) = request.source_run_id {
                            proposal.run_id = Some(run_id.clone());
                        }
                        let id = proposal.id.clone();
                        if let Err(e) = proposal_store.create_proposal(&proposal) {
                            eprintln!(
                                "[warn] Failed to create ExternalWriteAction Proposal: {}",
                                e
                            );
                        } else {
                            proposal_id = Some(id);
                        }
                    }
                }

                // Return structured result with unified payload, including proposal_id
                let mut result_payload = serde_json::json!({
                    "proposal_type": "external_write_action",
                    "external_write_action": {
                        "path": path,
                        "content_preview": content_preview,
                        "content_hash": content_hash,
                        "size_bytes": size_bytes,
                        "encoding": "utf-8",
                        "operation": operation,
                    },
                    "path": path,
                    "content_preview": content_preview,
                    "content_hash": content_hash,
                    "size_bytes": size_bytes,
                    "encoding": "utf-8",
                    "operation": operation,
                    "requires_confirmation": true,
                    "reason": format!("Proposed file write to '{}' ({})", path, operation),
                });
                if let Some(id) = proposal_id {
                    if let Some(obj) = result_payload.as_object_mut() {
                        obj.insert("proposal_id".to_string(), serde_json::json!(id));
                    }
                }

                Ok(ToolCallInternalResult {
                    success: true,
                    output: Some(result_payload.to_string()),
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

    /// For declarative-only stub tools (calendar, email), create a Proposal instead of blocking.
    #[allow(clippy::too_many_arguments)]
    fn create_declarative_stub_proposal(
        &self,
        request: &AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        tool_name: &str,
        args: &Value,
        proposal_type: ProposalType,
        category: &str,
        reason: &str,
    ) -> Option<Result<ActionExecutionResult>> {
        let proposal_store = ctx.proposal_store?;

        // Build after payload from tool arguments
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

fn normalize_tool_name(tool_name: &str, registry: &McpRegistry) -> String {
    if registry
        .list_manifests()
        .iter()
        .any(|manifest| manifest.name == tool_name || manifest.id == tool_name)
    {
        return tool_name.to_string();
    }

    let trimmed = tool_name.trim();
    let candidate = match trimmed {
        "fetch" | ".fetch" => Some("web.fetch"),
        "search" | ".search" => Some("web.search"),
        "read" | ".read" => Some("file.read"),
        "write_proposal" | ".write_proposal" => Some("file.write_proposal"),
        _ => None,
    };

    if let Some(candidate) = candidate {
        if registry
            .list_manifests()
            .iter()
            .any(|manifest| manifest.name == candidate || manifest.id == candidate)
        {
            return candidate.to_string();
        }
    }

    trimmed.to_string()
}

fn should_mark_needs_confirmation(
    decision: &ToolPermissionDecision,
    inspection: &McpArgumentInspection,
) -> bool {
    decision.requires_confirmation || (inspection.requires_confirmation && inspection.pii_found)
}

fn fetch_url_on_worker_thread(url: &str) -> Result<ToolCallInternalResult> {
    let url = url.to_string();
    std::thread::spawn(move || fetch_url_blocking(&url))
        .join()
        .unwrap_or_else(|_| {
            Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some("web.fetch worker thread panicked".to_string()),
            })
        })
}

fn search_web_on_worker_thread(query: &str, max_results: usize) -> Result<ToolCallInternalResult> {
    let query = query.to_string();
    std::thread::spawn(move || search_web_blocking(&query, max_results))
        .join()
        .unwrap_or_else(|_| {
            Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some("web.search worker thread panicked".to_string()),
            })
        })
}

fn search_web_blocking(query: &str, max_results: usize) -> Result<ToolCallInternalResult> {
    let url = reqwest::Url::parse_with_params(
        "https://duckduckgo.com/html/",
        &[("q", query), ("kl", "wt-wt")],
    )
    .map_err(|e| anyhow::anyhow!("Failed to build search URL: {}", e))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("OpenLife/0.1 (+local agent web.search)")
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

    match client.get(url).send() {
        Ok(response) => {
            let status = response.status();
            if !status.is_success() {
                return Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!(
                        "Search HTTP {}: {}",
                        status.as_u16(),
                        status.canonical_reason().unwrap_or("Unknown error")
                    )),
                });
            }

            match response.text() {
                Ok(html) => {
                    let results = extract_duckduckgo_results(&html, max_results);
                    let output = if results.is_empty() {
                        truncate_text(
                            &format!(
                                "No structured search results parsed. Raw page text:\n{}",
                                html_to_text(&html)
                            ),
                            12_000,
                        )
                    } else {
                        format_search_results(query, &results)
                    };
                    Ok(ToolCallInternalResult {
                        success: true,
                        output: Some(output),
                        error: None,
                    })
                }
                Err(e) => Ok(ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to read search response body: {}", e)),
                }),
            }
        }
        Err(e) => Ok(ToolCallInternalResult {
            success: false,
            output: None,
            error: Some(format!("Search request failed: {}", e)),
        }),
    }
}

fn extract_host_from_url(url: &str) -> Option<String> {
    url.split("//")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .and_then(|s| s.split(':').next())
        .map(|s| s.to_lowercase())
}

fn fetch_url_blocking(url: &str) -> Result<ToolCallInternalResult> {
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
                        let text = if text.trim_start().starts_with('<') {
                            html_to_text(&text)
                        } else {
                            text
                        };
                        let max_length = 50_000;
                        let truncated = truncate_text(&text, max_length);
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

fn extract_duckduckgo_results(html: &str, max_results: usize) -> Vec<SearchResult> {
    let block_regex = regex::Regex::new(
        r#"(?is)<a[^>]*class=["'][^"']*result__a[^"']*["'][^>]*href=["']([^"']+)["'][^>]*>(.*?)</a>(?P<body>.*?)(?:<a[^>]*class=["'][^"']*result__a|</body>|$)"#,
    )
    .unwrap_or_else(|_| regex::Regex::new("$^").unwrap());
    let snippet_regex = regex::Regex::new(
        r#"(?is)<a[^>]*class=["'][^"']*result__snippet[^"']*["'][^>]*>(.*?)</a>"#,
    )
    .unwrap_or_else(|_| regex::Regex::new("$^").unwrap());

    let mut results = Vec::new();
    for caps in block_regex.captures_iter(html) {
        if results.len() >= max_results {
            break;
        }
        let raw_href = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        let title_html = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
        let body = caps.name("body").map(|m| m.as_str()).unwrap_or_default();
        let snippet_html = snippet_regex
            .captures(body)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or_default();

        let title = html_to_text(title_html);
        let url = normalize_duckduckgo_href(raw_href);
        let snippet = html_to_text(snippet_html);

        if !title.is_empty() && !url.is_empty() {
            results.push(SearchResult {
                title,
                url,
                snippet,
            });
        }
    }
    results
}

fn normalize_duckduckgo_href(raw_href: &str) -> String {
    let href = raw_href.replace("&amp;", "&");
    let absolute = if href.starts_with("//") {
        format!("https:{}", href)
    } else if href.starts_with('/') {
        format!("https://duckduckgo.com{}", href)
    } else {
        href
    };

    if let Ok(url) = reqwest::Url::parse(&absolute) {
        if let Some((_, uddg)) = url.query_pairs().find(|(key, _)| key == "uddg") {
            return uddg.into_owned();
        }
        return url.to_string();
    }
    String::new()
}

fn format_search_results(query: &str, results: &[SearchResult]) -> String {
    let mut lines = vec![format!("Search results for \"{}\":", query)];
    for (idx, result) in results.iter().enumerate() {
        lines.push(format!(
            "{}. {}\n   URL: {}\n   Snippet: {}",
            idx + 1,
            result.title,
            result.url,
            result.snippet
        ));
    }
    lines.join("\n")
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        format!(
            "{}\n\n[Truncated: response exceeded {} characters]",
            text.chars().take(max_chars).collect::<String>(),
            max_chars
        )
    }
}

/// Check if a path is within the safe paths list.
/// Returns false if safe_paths is empty: filesystem access must be explicitly scoped.
///
/// Security rules:
/// - safe_paths are canonicalized; failures skip that path. All invalid => deny.
/// - Paths containing ".." components are rejected.
/// - Existing files: canonicalize full path and check against safe_paths.
/// - Non-existing files: parent must exist and be canonicalized; only a single
///   valid filename may be appended. Empty or non-UTF8 filenames are rejected.
/// - Symlinks are resolved by canonicalize; escaping safe_paths is blocked.
pub fn is_path_in_safe_paths(path: &str, safe_paths: &[String]) -> bool {
    if safe_paths.is_empty() {
        return false;
    }

    let path = std::path::Path::new(path);

    // Reject paths with parent directory references
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return false;
        }
    }

    // Determine the canonical base path
    let canonical_base = if let Ok(canonical) = path.canonicalize() {
        // Path exists: use full canonical path
        canonical
    } else {
        // Path doesn't exist: parent must exist and be canonicalized
        let parent = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => return false,
        };

        let canonical_parent = match parent.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };

        // Validate filename: must exist, be non-empty, and valid UTF-8
        if let Some(filename) = path.file_name() {
            if let Some(name_str) = filename.to_str() {
                if name_str.is_empty() {
                    return false;
                }
            } else {
                return false; // Non-UTF8 filename
            }
        } else {
            return false; // No filename (e.g. trailing slash)
        }

        canonical_parent
    };

    // Canonicalize safe paths, skipping ones that fail or are symlinks.
    // If all safe paths fail to canonicalize, deny.
    let valid_safe_paths: Vec<std::path::PathBuf> = safe_paths
        .iter()
        .filter_map(|safe| {
            let safe_path = std::path::Path::new(safe);
            // Reject safe paths that are symlinks
            if let Ok(meta) = safe_path.symlink_metadata() {
                if meta.file_type().is_symlink() {
                    return None;
                }
            }
            safe_path.canonicalize().ok()
        })
        .collect();

    if valid_safe_paths.is_empty() {
        return false;
    }

    // Check if canonical_base is within any safe path
    valid_safe_paths
        .iter()
        .any(|safe| canonical_base.starts_with(safe))
}

pub fn filesystem_access_error(path: &str, safe_paths: &[String]) -> String {
    if safe_paths.is_empty() {
        "No safe paths configured for filesystem access".to_string()
    } else {
        format!("Path '{}' is not in safe paths list", path)
    }
}

/// Check if an IP address is private/internal.
/// Blocks loopback, private ranges, and link-local addresses.
pub fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local()
        }
        std::net::IpAddr::V6(ipv6) => {
            ipv6.is_loopback() || ipv6.is_unique_local() || ipv6.is_unicast_link_local()
        }
    }
}

/// Resolve a hostname and check if any resolved IP is private.
/// Returns true if any resolved address is private/internal.
fn resolve_host_is_private(host: &str) -> bool {
    // Try to add a dummy port for ToSocketAddrs resolution
    let addr_with_port = format!("{}:80", host);
    if let Ok(addrs) = addr_with_port.to_socket_addrs() {
        for addr in addrs {
            let ip = addr.ip();
            if is_private_ip(&ip) {
                return true;
            }
        }
    }
    false
}

/// Check if a URL points to a private/internal address.
/// Blocks localhost, private IP ranges, and link-local addresses.
/// Only checks the host portion of the URL; query/path fragments are ignored.
pub fn is_private_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };

    if let Some(host) = parsed.host_str() {
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            return is_private_ip(&ip);
        }

        let domain = host.trim_end_matches('.').to_ascii_lowercase();
        if domain == "localhost" || domain.ends_with(".localhost") {
            return true;
        }
        return resolve_host_is_private(&domain);
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
            network_policy: None,
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

        // Verify execution_status classification is present
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        let tools = json["tools"].as_array().unwrap();
        assert!(!tools.is_empty());

        // Check that at least one tool has execution_status field
        let first_tool = &tools[0];
        assert!(first_tool["execution_status"].is_string());
        let status = first_tool["execution_status"].as_str().unwrap();
        assert!(
            matches!(
                status,
                "executable" | "needs_permission" | "declarative_only" | "disabled"
            ),
            "unexpected execution_status: {}",
            status
        );

        // Verify enabled and declarative_only fields are present
        assert!(first_tool["enabled"].is_boolean());
        assert!(first_tool["declarative_only"].is_boolean());
        assert!(first_tool["requires_confirmation"].is_boolean());
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
    fn web_fetch_alias_normalizes_to_registered_tool() {
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
                    target: ".fetch".into(),
                    input: serde_json::json!({"arguments": {"url": "ftp://example.com"}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.action.target.as_deref(), Some("web.fetch"));
        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert!(result.observation.content.contains("Invalid URL scheme"));
    }

    #[test]
    fn is_private_url_blocks_localhost_and_private_ips() {
        assert!(is_private_url("http://localhost:8080"));
        assert!(is_private_url("http://foo.localhost/path"));
        assert!(is_private_url("http://127.0.0.1/api"));
        assert!(is_private_url("http://10.0.0.1/admin"));
        assert!(is_private_url("http://192.168.1.1/"));
        assert!(is_private_url("http://172.16.0.1/"));
        assert!(is_private_url("http://169.254.1.1/"));
        assert!(is_private_url("http://[::1]/"));
        assert!(is_private_url("http://[fe80::1]/"));
        assert!(is_private_url("http://[fc00::1]/"));
        assert!(!is_private_url("https://example.com/page"));
        assert!(!is_private_url(
            "https://example.com/api?callback=http://172.16.0.1"
        ));
        assert!(!is_private_url("https://api.openai.com/v1"));
    }

    #[test]
    fn is_private_ip_blocks_loopback_v4() {
        assert!(is_private_ip(&"127.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"127.255.255.255".parse().unwrap()));
    }

    #[test]
    fn is_private_ip_blocks_loopback_v6() {
        assert!(is_private_ip(&"::1".parse().unwrap()));
    }

    #[test]
    fn is_private_ip_blocks_private_ranges() {
        assert!(is_private_ip(&"10.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"10.255.255.255".parse().unwrap()));
        assert!(is_private_ip(&"172.16.0.1".parse().unwrap()));
        assert!(is_private_ip(&"172.31.255.255".parse().unwrap()));
        assert!(is_private_ip(&"192.168.0.1".parse().unwrap()));
        assert!(is_private_ip(&"192.168.255.255".parse().unwrap()));
    }

    #[test]
    fn is_private_ip_blocks_link_local() {
        assert!(is_private_ip(&"169.254.0.1".parse().unwrap()));
        assert!(is_private_ip(&"169.254.255.255".parse().unwrap()));
        assert!(is_private_ip(&"fe80::1".parse().unwrap()));
    }

    #[test]
    fn is_private_ip_allows_public_ip() {
        assert!(!is_private_ip(&"8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip(&"1.1.1.1".parse().unwrap()));
        assert!(!is_private_ip(&"104.16.249.249".parse().unwrap()));
        assert!(!is_private_ip(&"2001:4860:4860::8888".parse().unwrap()));
    }

    #[test]
    fn web_fetch_url_parsing_extracts_host_correctly() {
        // Test that host extraction handles various URL formats
        let test_cases = [
            ("http://example.com/path", "example.com"),
            ("https://api.example.com:8080/v1", "api.example.com"),
            (
                "http://sub.domain.example.com:443/path",
                "sub.domain.example.com",
            ),
        ];

        for (url, expected_host) in &test_cases {
            if let Some(host_start) = url.find("://") {
                let host_part = &url[host_start + 3..];
                let host = host_part.split('/').next().unwrap_or(host_part);
                let host = host.split(':').next().unwrap_or(host);
                assert_eq!(host, *expected_host, "URL: {}", url);
            } else {
                panic!("No :// in URL: {}", url);
            }
        }
    }

    #[test]
    fn duckduckgo_results_are_extracted_and_urls_unwrapped() {
        let html = r#"
            <html><body>
              <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpage%3Fx%3D1&amp;rut=abc">Example &amp; Result</a>
              <a class="result__snippet">A <b>useful</b> snippet &amp; context.</a>
            </body></html>
        "#;

        let results = extract_duckduckgo_results(html, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example & Result");
        assert_eq!(results[0].url, "https://example.com/page?x=1");
        assert_eq!(results[0].snippet, "A **useful** snippet & context.");
    }

    #[test]
    fn truncate_text_is_char_boundary_safe() {
        let text = "今天星期几🙂明天呢";
        let truncated = truncate_text(text, 5);
        assert!(truncated.starts_with("今天星期几"));
        assert!(truncated.contains("[Truncated"));
    }

    // =======================================================================
    // Safe paths security tests
    // =======================================================================

    #[test]
    fn safe_paths_empty_list_denies_all() {
        let safe_paths: Vec<String> = vec![];
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        assert!(!is_path_in_safe_paths(
            temp_file.path().to_str().unwrap(),
            &safe_paths
        ));
    }

    #[test]
    fn safe_paths_blocks_double_dot_traversal() {
        let safe_dir = tempfile::tempdir().unwrap();
        let safe_path = safe_dir.path().to_string_lossy().to_string();
        let safe_paths = vec![safe_path];

        // Attempt to escape using ..
        let escaped = safe_dir
            .path()
            .join("subdir")
            .join("..")
            .join("..")
            .join("etc")
            .join("passwd");
        assert!(!is_path_in_safe_paths(
            escaped.to_str().unwrap(),
            &safe_paths
        ));

        // Simple .. in path
        assert!(!is_path_in_safe_paths("/safe/../outside", &safe_paths));
    }

    #[test]
    fn safe_paths_blocks_symlink_escape() {
        let safe_dir = tempfile::tempdir().unwrap();
        let outside_dir = tempfile::tempdir().unwrap();
        let safe_path = safe_dir.path().to_string_lossy().to_string();
        let safe_paths = vec![safe_path];

        // Create a symlink inside safe_dir pointing outside
        let symlink_path = safe_dir.path().join("escape_link");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(outside_dir.path(), &symlink_path).unwrap();
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_dir;
            symlink_dir(outside_dir.path(), &symlink_path).unwrap();
        }

        // Attempt to access a file through the symlink
        let escaped_file = symlink_path.join("secret.txt");
        assert!(!is_path_in_safe_paths(
            escaped_file.to_str().unwrap(),
            &safe_paths
        ));

        // Clean up symlink
        let _ = std::fs::remove_file(&symlink_path);
    }

    #[test]
    fn safe_paths_blocks_nonexistent_parent() {
        let safe_dir = tempfile::tempdir().unwrap();
        let safe_path = safe_dir.path().to_string_lossy().to_string();
        let safe_paths = vec![safe_path];

        // Parent directory does not exist
        let new_file = safe_dir
            .path()
            .join("nonexistent_parent")
            .join("new_file.txt");
        assert!(!is_path_in_safe_paths(
            new_file.to_str().unwrap(),
            &safe_paths
        ));
    }

    #[test]
    fn safe_paths_allows_new_file_with_existing_parent() {
        let safe_dir = tempfile::tempdir().unwrap();
        let safe_path = safe_dir.path().to_string_lossy().to_string();
        let safe_paths = vec![safe_path];

        // Parent exists, file does not: should be allowed
        let new_file = safe_dir.path().join("new_file.txt");
        assert!(!new_file.exists());
        assert!(is_path_in_safe_paths(
            new_file.to_str().unwrap(),
            &safe_paths
        ));
    }

    #[test]
    fn safe_paths_allows_existing_file() {
        let safe_dir = tempfile::tempdir().unwrap();
        let safe_path = safe_dir.path().to_string_lossy().to_string();
        let safe_paths = vec![safe_path];

        // Create an existing file inside safe path
        let existing_file = safe_dir.path().join("existing.txt");
        std::fs::write(&existing_file, "content").unwrap();
        assert!(is_path_in_safe_paths(
            existing_file.to_str().unwrap(),
            &safe_paths
        ));
    }

    #[test]
    fn safe_paths_blocks_file_outside_safe_paths() {
        let safe_dir = tempfile::tempdir().unwrap();
        let outside_dir = tempfile::tempdir().unwrap();
        let safe_path = safe_dir.path().to_string_lossy().to_string();
        let safe_paths = vec![safe_path];

        let outside_file = outside_dir.path().join("outside.txt");
        std::fs::write(&outside_file, "content").unwrap();
        assert!(!is_path_in_safe_paths(
            outside_file.to_str().unwrap(),
            &safe_paths
        ));
    }

    #[test]
    fn safe_paths_all_canonicalize_fails_denies_all() {
        // Provide a safe path that does not exist -> canonicalize fails
        let safe_paths = vec!["/nonexistent_safe_path_12345".to_string()];
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        assert!(!is_path_in_safe_paths(
            temp_file.path().to_str().unwrap(),
            &safe_paths
        ));
    }

    #[test]
    fn snapshot_create_not_in_available_tools() {
        let registry = McpRegistry::new();
        let manifests = registry.list_manifests();
        let snapshot_manifests: Vec<_> = manifests
            .iter()
            .filter(|m| m.name == "snapshot.create")
            .collect();
        assert_eq!(snapshot_manifests.len(), 1);
        assert!(snapshot_manifests[0].declarative_only);

        let available = registry.tools_prompt();
        assert!(!available.contains("snapshot.create"));
    }

    #[test]
    fn mcp_call_tool_routes_to_target_manifest() {
        let mut registry = McpRegistry::new();
        // Use BuiltIn source for test so execute_manifest routes through builtins
        registry.register_builtin(
            ToolManifest {
                id: "search_files".into(),
                name: "search_files".into(),
                description: "test search".into(),
                parameters: serde_json::json!({"type": "object"}),
                permission_level: "low".into(),
                risk_level: "low".into(),
                version: "1.0.0".into(),
                source: ToolSource::BuiltIn,
                capabilities: vec!["read".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                tags: vec![],
            },
            Box::new(|_| Ok("found files".into())),
        );

        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        // Grant permission to mcp.call_tool wrapper
        permission_store
            .grant(
                "mcp.call_tool",
                "builtin",
                "medium",
                "external_side_effect",
                ToolPermissionPolicy::Allow,
                None,
            )
            .unwrap();
        // Grant permission to target tool
        permission_store
            .grant(
                "search_files",
                "builtin",
                "low",
                "read",
                ToolPermissionPolicy::Allow,
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
                    action_type: "builtin_tool".into(),
                    target: "mcp.call_tool".into(),
                    input: serde_json::json!({
                        "arguments": {
                            "tool_name": "search_files",
                            "arguments": {"query": "*.rs"}
                        }
                    }),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(
            result.status,
            ActionExecutionStatus::Succeeded,
            "Expected success but got: {}",
            result.observation.content
        );
        assert_eq!(result.observation.content, "found files");
    }

    #[test]
    fn mcp_call_tool_multiple_same_name_requires_server() {
        let mut registry = McpRegistry::new();
        // Use BuiltIn source for test
        registry.register_builtin(
            ToolManifest {
                id: "search_files_a".into(),
                name: "search_files".into(),
                description: "test search 1".into(),
                parameters: serde_json::json!({"type": "object"}),
                permission_level: "low".into(),
                risk_level: "low".into(),
                version: "1.0.0".into(),
                source: ToolSource::BuiltIn,
                capabilities: vec!["read".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                tags: vec![],
            },
            Box::new(|_| Ok("from a".into())),
        );
        registry.register_builtin(
            ToolManifest {
                id: "search_files_b".into(),
                name: "search_files".into(),
                description: "test search 2".into(),
                parameters: serde_json::json!({"type": "object"}),
                permission_level: "low".into(),
                risk_level: "low".into(),
                version: "1.0.0".into(),
                source: ToolSource::BuiltIn,
                capabilities: vec!["read".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                tags: vec![],
            },
            Box::new(|_| Ok("from b".into())),
        );

        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        // Grant permission to mcp.call_tool wrapper
        permission_store
            .grant(
                "mcp.call_tool",
                "builtin",
                "medium",
                "external_side_effect",
                ToolPermissionPolicy::Allow,
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
                    action_type: "builtin_tool".into(),
                    target: "mcp.call_tool".into(),
                    input: serde_json::json!({
                        "arguments": {
                            "tool_name": "search_files",
                            "arguments": {}
                        }
                    }),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert!(result.observation.content.contains("Multiple MCP tools"));
    }

    #[test]
    fn mcp_call_tool_uses_target_permission_scope() {
        let mut registry = McpRegistry::new();
        // Use BuiltIn source for test
        registry.register_builtin(
            ToolManifest {
                id: "dangerous_delete".into(),
                name: "dangerous_delete".into(),
                description: "test delete".into(),
                parameters: serde_json::json!({"type": "object"}),
                permission_level: "high".into(),
                risk_level: "high".into(),
                version: "1.0.0".into(),
                source: ToolSource::BuiltIn,
                capabilities: vec!["write".into()],
                requires_confirmation: true,
                enabled: true,
                declarative_only: false,
                action_type: "write".into(),
                tags: vec![],
            },
            Box::new(|_| Ok("deleted".into())),
        );

        // Grant permission to mcp.call_tool itself (low/medium risk wrapper)
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "mcp.call_tool",
                "builtin",
                "medium",
                "external_side_effect",
                ToolPermissionPolicy::Allow,
                None,
            )
            .unwrap();
        // But do NOT grant permission to dangerous_delete

        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "mcp.call_tool".into(),
                    input: serde_json::json!({
                        "arguments": {
                            "tool_name": "dangerous_delete",
                            "arguments": {}
                        }
                    }),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(
            result.status,
            ActionExecutionStatus::NeedsConfirmation,
            "Expected NeedsConfirmation but got: {}",
            result.observation.content
        );
        assert!(
            result.observation.content.contains("blocked")
                || result.observation.content.contains("ask_every_time"),
            "Expected permission error but got: {}",
            result.observation.content
        );
    }

    #[test]
    fn external_write_action_records_proposal_id_in_output() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        permission_store
            .grant(
                "file.write_proposal",
                "builtin",
                "high",
                "write",
                ToolPermissionPolicy::Allow,
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
        ctx.safe_paths = &safe_paths;

        // Use ProposalStore to capture the proposal
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        ctx.proposal_store = Some(&proposal_store);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "file.write_proposal".into(),
                    input: serde_json::json!({
                        "arguments": {
                            "path": safe_dir.path().join("new.txt").to_str().unwrap(),
                            "content": "hello"
                        }
                    }),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Succeeded);
        // Verify proposal_id is in the action output text
        let output = result.action.output.as_ref().expect("output should exist");
        let output_text = output
            .get("text")
            .and_then(|v| v.as_str())
            .expect("text field should exist");
        let output_json: serde_json::Value =
            serde_json::from_str(output_text).expect("output should be valid JSON");
        let proposal_id = output_json.get("proposal_id").and_then(|v| v.as_str());
        assert!(
            proposal_id.is_some(),
            "proposal_id should be present in action output"
        );
        // Verify the proposal was actually created
        let proposals = proposal_store.list_pending_proposals(10).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].id, proposal_id.unwrap());
    }

    #[test]
    fn network_policy_disabled_blocks_web_tools() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let mut ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);
        let policy = crate::config::NetworkPolicy {
            enabled: false,
            ..Default::default()
        };
        ctx.network_policy = Some(&policy);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "web.fetch".into(),
                    input: serde_json::json!({"arguments": {"url": "https://example.com"}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert!(result.observation.content.contains("disabled by policy"));
    }

    #[test]
    fn network_policy_domain_denylist_blocks_url() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let mut ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);
        let policy = crate::config::NetworkPolicy {
            enabled: true,
            domain_denylist: vec!["evil.com".to_string()],
            ..Default::default()
        };
        ctx.network_policy = Some(&policy);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "web.fetch".into(),
                    input: serde_json::json!({"arguments": {"url": "https://evil.com/page"}}),
                    source_run_id: Some("run-1".into()),
                    step_index: 0,
                },
                &ctx,
            )
            .unwrap();

        assert_eq!(result.status, ActionExecutionStatus::Failed);
        assert!(result.observation.content.contains("denylist"));
    }

    #[test]
    fn network_policy_domain_allowlist_blocks_unlisted() {
        let registry = McpRegistry::new();
        let permission_store = ToolPermissionStore::new_in_memory().unwrap();
        let audit_file = tempfile::NamedTempFile::new().unwrap();
        let audit_store = McpAuditStore::new(audit_file.path());
        let privacy_engine = PrivacyEngine::default();
        let mut ctx = test_context(&registry, &permission_store, &audit_store, &privacy_engine);
        let policy = crate::config::NetworkPolicy {
            enabled: true,
            domain_allowlist: vec!["github.com".to_string()],
            ..Default::default()
        };
        ctx.network_policy = Some(&policy);

        let result = ActionExecutor::new(ActionExecutorConfig::default())
            .execute(
                AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "web.fetch".into(),
                    input: serde_json::json!({"arguments": {"url": "https://example.com"}}),
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
            .contains("not in the network allowlist"));
    }
}
