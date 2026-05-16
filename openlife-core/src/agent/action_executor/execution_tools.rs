use crate::agent::action_executor::helpers::is_private_url;
use crate::agent::action_executor::helpers::{
    call_a2a_agent_blocking, canonical_tool_source, extract_host_from_url,
    fetch_url_on_worker_thread, filesystem_access_error, is_path_in_safe_paths,
    search_web_on_worker_thread, summarize_content_blocking, ToolCallInternalResult,
};
use crate::agent::action_executor::{
    ExecutionBlockReason, ExecutionFailureKind, ExecutionProposalReason,
};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use crate::config::NetworkPolicy;
use anyhow::Result;
use ring::digest::{digest, SHA256};
use serde_json::Value;

use super::ActionContext;
use super::AgentActionRequest;

/// Evaluate network policy for a tool that makes network requests.
/// Returns None if the tool is allowed; Some(blocked_result) if blocked.
/// `url_for_domain` enables domain allowlist/denylist checks (only for tools with
/// an explicit URL argument, e.g. web.fetch and a2a.call_agent).
/// `already_permitted` indicates the tool already has an allow permission
/// (e.g. from a previously accepted ToolPermission Proposal); when true,
/// `default_decision=ask/deny` is skipped, but hard blocks (enabled=false,
/// tool_overrides=deny, domain denylist) still apply.
pub(crate) fn check_network_policy(
    tool_name: &str,
    policy: &NetworkPolicy,
    url_for_domain: Option<&str>,
    already_permitted: bool,
) -> Option<ToolCallInternalResult> {
    if !policy.enabled {
        return Some(ToolCallInternalResult::blocked(
            ExecutionBlockReason::NetworkPolicyDenied,
            "Network tools are disabled by policy. Enable network access in Settings to use web tools."
        ));
    }
    if let Some(d) = policy.tool_overrides.get(tool_name) {
        if d == "deny" {
            return Some(ToolCallInternalResult::blocked(
                ExecutionBlockReason::NetworkPolicyDenied,
                format!("Tool '{}' is denied by network policy override", tool_name),
            ));
        }
    }
    if let Some(host) = url_for_domain.and_then(extract_host_from_url) {
        if policy.domain_denylist.iter().any(|d| host.ends_with(d)) {
            return Some(ToolCallInternalResult::blocked(
                ExecutionBlockReason::DomainBlocked,
                format!("Domain '{}' is in the network denylist", host),
            ));
        }
        if !policy.domain_allowlist.is_empty()
            && !policy.domain_allowlist.iter().any(|d| host.ends_with(d))
        {
            return Some(ToolCallInternalResult::blocked(
                ExecutionBlockReason::DomainBlocked,
                format!("Domain '{}' is not in the network allowlist", host),
            ));
        }
    }
    // default_decision is only enforced when the tool is not already permitted
    if already_permitted {
        return None;
    }
    match policy.default_decision.as_str() {
        "deny" => Some(ToolCallInternalResult::blocked(
            ExecutionBlockReason::NetworkPolicyDenied,
            format!(
                "Tool '{}' is blocked by network policy (default_decision=deny)",
                tool_name
            ),
        )),
        "ask" => Some(ToolCallInternalResult::needs_confirmation(
            ExecutionProposalReason::NetworkPolicyAsk,
            format!(
                "Tool '{}' requires user confirmation before network access (default_decision=ask)",
                tool_name
            ),
        )),
        _ => None,
    }
}

impl super::ActionExecutor {
    /// Execute an Execution tool (file.read, web.fetch, etc.).
    /// Each tool variant uses only short-lived locks on the stores it
    /// actually needs — no `MutexGuard` is held across external I/O
    /// (file, web, A2A, MCP).
    pub async fn execute_execution_tool(
        &self,
        tool_name: &str,
        args: &Value,
        ac: &ActionContext,
        request: &AgentActionRequest,
    ) -> Result<ToolCallInternalResult> {
        match tool_name {
            "file.read" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument for file.read"))?;

                // Validate path is within safe_paths
                if !is_path_in_safe_paths(path, &ac.safe_paths) {
                    return Ok(ToolCallInternalResult::blocked(
                        ExecutionBlockReason::PathNotSafe,
                        filesystem_access_error(path, &ac.safe_paths),
                    ));
                }

                // Check file size before reading
                let metadata = std::fs::metadata(path)
                    .map_err(|e| anyhow::anyhow!("Failed to read file metadata: {}", e))?;
                let max_size = 100 * 1024; // 100KB limit
                if metadata.len() > max_size {
                    return Ok(ToolCallInternalResult::blocked(
                        ExecutionBlockReason::InvalidArguments,
                        format!(
                            "File size ({} bytes) exceeds maximum allowed ({} bytes)",
                            metadata.len(),
                            max_size
                        ),
                    ));
                }

                match std::fs::read_to_string(path) {
                    Ok(content) => Ok(ToolCallInternalResult::success(content)),
                    Err(e) => Ok(ToolCallInternalResult::failure(
                        ExecutionFailureKind::ToolRuntimeError,
                        format!("Failed to read file '{}': {}", path, e),
                    )),
                }
            }
            "calendar.read" => {
                let range_start = args.get("range_start").and_then(|v: &Value| v.as_str());
                let range_end = args.get("range_end").and_then(|v: &Value| v.as_str());

                // Use the ICS file path from args or from safe_paths
                let ics_path = args.get("source").and_then(|v: &Value| v.as_str());

                let events = if let Some(path) = ics_path {
                    // Validate source path is within calendar_ics_paths or safe_paths
                    let mut all_calendar_paths: Vec<String> = ac.calendar_ics_paths.to_vec();
                    all_calendar_paths.extend(ac.safe_paths.iter().cloned());
                    if !is_path_in_safe_paths(path, &all_calendar_paths) {
                        return Ok(ToolCallInternalResult::blocked(
                            ExecutionBlockReason::PathNotSafe,
                            filesystem_access_error(path, &all_calendar_paths),
                        ));
                    }
                    let metadata = match std::fs::metadata(path) {
                        Ok(m) => m,
                        Err(e) => {
                            return Ok(ToolCallInternalResult::failure(
                                ExecutionFailureKind::ToolRuntimeError,
                                format!("Failed to read ICS file metadata: {}", e),
                            ));
                        }
                    };
                    let max_size = 100 * 1024; // 100KB limit, same as file.read
                    if metadata.len() > max_size {
                        return Ok(ToolCallInternalResult::blocked(
                            ExecutionBlockReason::InvalidArguments,
                            format!(
                                "ICS file size ({} bytes) exceeds maximum allowed ({} bytes)",
                                metadata.len(),
                                max_size
                            ),
                        ));
                    }
                    match std::fs::read_to_string(path) {
                        Ok(content) => crate::calendar::parse_ics(&content, range_start, range_end),
                        Err(e) => {
                            return Ok(ToolCallInternalResult::failure(
                                ExecutionFailureKind::ToolRuntimeError,
                                format!("Failed to read ICS file '{}': {}", path, e),
                            ));
                        }
                    }
                } else {
                    // Try calendar_ics_paths first, then safe_paths for .ics files
                    let ics_search_paths: Vec<&String> = if ac.calendar_ics_paths.is_empty() {
                        ac.safe_paths.iter().collect()
                    } else {
                        ac.calendar_ics_paths.iter().collect()
                    };
                    let mut all_events = Vec::new();
                    for search_path in &ics_search_paths {
                        if let Ok(entries) = std::fs::read_dir(search_path) {
                            for entry in entries.flatten() {
                                let p = entry.path();
                                if p.extension() == Some(std::ffi::OsStr::new("ics")) {
                                    if let Ok(content) = std::fs::read_to_string(&p) {
                                        all_events.extend(crate::calendar::parse_ics(
                                            &content,
                                            range_start,
                                            range_end,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    if all_events.is_empty() {
                        return Ok(ToolCallInternalResult::failure(
                            ExecutionFailureKind::ToolRuntimeError,
                            "No .ics files found in safe_paths. Configure calendar_ics_paths in Settings or provide 'source' argument.",
                        ));
                    }
                    all_events
                };

                let output = serde_json::json!({
                    "status": "success",
                    "events": events,
                    "count": events.len(),
                })
                .to_string();
                Ok(ToolCallInternalResult::success(output))
            }
            "web.fetch" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'url' argument for web.fetch"))?;

                // Validate URL scheme
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Ok(ToolCallInternalResult::blocked(
                        ExecutionBlockReason::InvalidArguments,
                        format!(
                            "Invalid URL scheme. Only http:// and https:// are allowed, got: {}",
                            url
                        ),
                    ));
                }

                // Block private IP ranges and localhost
                if is_private_url(url) {
                    return Ok(ToolCallInternalResult::blocked(
                        ExecutionBlockReason::DomainBlocked,
                        format!(
                            "URL '{}' points to a private/internal address and is blocked for security",
                            url
                        ),
                    ));
                }

                let result = fetch_url_on_worker_thread(url)?;
                // Optional summarization via Ollama
                let summarize = args
                    .get("summarize")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if summarize && result.success && result.output.is_some() {
                    let content = result.output.clone().unwrap_or_default();
                    match summarize_content_blocking(&content, url) {
                        Ok(summary) => Ok(ToolCallInternalResult::success(summary)),
                        Err(_) => Ok(ToolCallInternalResult {
                            success: true,
                            output: Some(content),
                            error: Some(
                                "Content summarization failed, showing raw content".to_string(),
                            ),
                            block_reason: None,
                            proposal_reason: None,
                            failure_kind: None,
                        }),
                    }
                } else {
                    Ok(result)
                }
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
                    return Ok(ToolCallInternalResult::blocked(
                        ExecutionBlockReason::InvalidArguments,
                        "Search query cannot be empty",
                    ));
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

                // 1. Short-lock registry: find target manifest + inspect PII
                let (target_manifest, inspection) = {
                    let reg = ac.registry.lock().await;
                    let manifests = reg.list_manifests();
                    let mut target_manifests: Vec<_> = manifests
                        .into_iter()
                        .filter(|m| m.name == target_tool_name || m.id == target_tool_name)
                        .collect();

                    let target_manifest = if let Some(server_name) = server {
                        match target_manifests
                            .into_iter()
                            .find(|m| matches!(&m.source, crate::tool_manifest::ToolSource::Mcp { server_name: s } if s == server_name))
                        {
                            Some(m) => m,
                            None => {
                                return Ok(ToolCallInternalResult::failure(
                                    ExecutionFailureKind::MissingMcpServer,
                                    format!(
                                        "MCP tool '{}' not found on server '{}'",
                                        target_tool_name, server_name
                                    ),
                                ));
                            }
                        }
                    } else if target_manifests.len() == 1 {
                        target_manifests.remove(0)
                    } else if target_manifests.is_empty() {
                        return Ok(ToolCallInternalResult::failure(
                            ExecutionFailureKind::MissingMcpServer,
                            format!("MCP tool '{}' not found", target_tool_name),
                        ));
                    } else {
                        return Ok(ToolCallInternalResult::blocked(
                            ExecutionBlockReason::InvalidArguments,
                            format!(
                                "Multiple MCP tools named '{}'. Please specify 'server' parameter.",
                                target_tool_name
                            ),
                        ));
                    };
                    let inspection = reg.inspect_call_arguments(&target_manifest.name, &tool_args);
                    (target_manifest, inspection)
                }; // registry lock released

                // 1.5. AgentSpec target-level governance — check real MCP target,
                // not only the mcp.call_tool wrapper. Wrapper allow in execute_tool()
                // does NOT imply target allow. deny always wins.
                if let Some(ref spec) = ac.agent_spec {
                    if !spec.is_tool_allowed(&target_manifest.name) {
                        return Ok(ToolCallInternalResult::blocked(
                            ExecutionBlockReason::AgentSpecDenied,
                            format!(
                                "MCP target tool '{}' is denied by the current AgentSpec — wrapper allow for mcp.call_tool does not override target deny (denied_tools: {:?})",
                                target_manifest.name, spec.denied_tools
                            ),
                        ));
                    }
                }

                // 2. NetworkPolicy gate — only for network-capable MCP targets.
                // This is checked here (after target resolution) rather than in
                // execute_tool() so that local/stdio/file MCP tools are not
                // incorrectly blocked by the global network policy.
                if target_manifest.capabilities.iter().any(|c| c == "network") {
                    if let Some(ref policy) = ac.network_policy {
                        let already_permitted = {
                            let perm = ac.permission_store.lock().await;
                            let target_source = canonical_tool_source(&target_manifest);
                            perm.peek(
                                &target_manifest.name,
                                &target_source,
                                &target_manifest.risk_level,
                                &target_manifest.action_type,
                                &target_manifest.capabilities,
                            )
                            .is_ok_and(|d| d.allowed && d.policy_id.is_some())
                        };
                        if let Some(blocked) = check_network_policy(
                            &target_manifest.name,
                            policy,
                            None,
                            already_permitted,
                        ) {
                            return Ok(blocked);
                        }
                    }
                }

                // 3. Short-lock permission_store: check target permission
                let target_decision = {
                    let perm = ac.permission_store.lock().await;
                    let target_source = canonical_tool_source(&target_manifest);
                    perm.check(
                        &target_manifest.name,
                        &target_source,
                        &target_manifest.risk_level,
                        &target_manifest.action_type,
                        &target_manifest.capabilities,
                    )
                    .unwrap_or(
                        crate::tool_permissions::ToolPermissionDecision {
                            allowed: false,
                            requires_confirmation: true,
                            decision: "ask_every_time".into(),
                            reason: "permission check failed".into(),
                            policy_id: None,
                        },
                    )
                }; // permission_store lock released

                if !target_decision.allowed || target_decision.requires_confirmation {
                    if target_decision.requires_confirmation {
                        return Ok(ToolCallInternalResult::needs_confirmation(
                            ExecutionProposalReason::ToolPermissionAsk,
                            format!(
                                "MCP tool '{}' requires user confirmation: {} (decision: {})",
                                target_tool_name, target_decision.reason, target_decision.decision
                            ),
                        ));
                    }
                    return Ok(ToolCallInternalResult::blocked(
                        ExecutionBlockReason::ToolPermissionDenied,
                        format!(
                            "MCP tool '{}' denied: {} (decision: {})",
                            target_tool_name, target_decision.reason, target_decision.decision
                        ),
                    ));
                }

                if inspection.requires_confirmation && inspection.pii_found {
                    return Ok(ToolCallInternalResult::blocked(
                        ExecutionBlockReason::PiiDetected,
                        format!(
                            "MCP tool '{}' blocked: PII detected in arguments",
                            target_tool_name
                        ),
                    ));
                }

                // 3. Execute target tool — NO store locks held
                Ok(self
                    .call_tool_internal_async(
                        &target_manifest,
                        tool_args,
                        &ac.registry,
                        &ac.audit_store,
                        inspection.pii_found,
                    )
                    .await)
            }
            "file.write_proposal" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

                // Validate path is within safe_paths
                if !path.is_empty() && !is_path_in_safe_paths(path, &ac.safe_paths) {
                    return Ok(ToolCallInternalResult::blocked(
                        ExecutionBlockReason::PathNotSafe,
                        filesystem_access_error(path, &ac.safe_paths),
                    ));
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

                // Auto-create ExternalWriteAction Proposal — lock proposal_store briefly
                let mut proposal_id: Option<String> = None;
                if !path.is_empty() {
                    if let Some(ref ps_arc) = ac.proposal_store {
                        let ps = ps_arc.lock().await;
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
                        if let Err(e) = ps.create_proposal(&proposal) {
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
                    block_reason: None,
                    proposal_reason: None,
                    failure_kind: None,
                })
            }
            "task.create_proposal" => {
                let title = args
                    .get("title")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("Untitled Task");
                let description = args
                    .get("description")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("");
                let due_date = args
                    .get("due_date")
                    .or_else(|| args.get("dueDate"))
                    .and_then(|v: &Value| v.as_str())
                    .map(|s| s.to_string());
                let priority = args
                    .get("priority")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("medium");

                // Create proposal for user confirmation — lock proposal_store briefly
                let task_args = serde_json::json!({
                    "title": title,
                    "description": description,
                    "due_date": due_date,
                    "scheduled_at": due_date,
                    "priority": priority,
                    "tool": "task.create_proposal",
                });

                if let Some(ref ps_arc) = ac.proposal_store {
                    let ps = ps_arc.lock().await;
                    let mut proposal = AgentProposal::new(
                        ProposalType::ScheduledTask,
                        "tasks",
                        task_args,
                        &format!("Agent proposed task: {}", title),
                        0.85,
                        RiskLevel::Low,
                        ProposalSource::Manual,
                    );
                    if let Some(ref run_id) = request.source_run_id {
                        proposal.run_id = Some(run_id.clone());
                    }
                    let proposal_id = proposal.id.clone();
                    match ps.create_proposal(&proposal) {
                        Ok(_) => {
                            let output = serde_json::json!({
                                "status": "proposal_created",
                                "proposal_id": proposal_id,
                                "proposal_type": "scheduled_task",
                                "title": title,
                            })
                            .to_string();
                            Ok(ToolCallInternalResult {
                                success: true,
                                output: Some(output),
                                error: None,
                                block_reason: None,
                                proposal_reason: None,
                                failure_kind: None,
                            })
                        }
                        Err(e) => Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(format!("Failed to create task proposal: {}", e)),
                            block_reason: None,
                            proposal_reason: None,
                            failure_kind: None,
                        }),
                    }
                } else {
                    Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some("ProposalStore not available in execution context".to_string()),
                        block_reason: None,
                        proposal_reason: None,
                        failure_kind: None,
                    })
                }
            }
            "a2a.call_agent" => {
                let agent_url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'url' argument for a2a.call_agent"))?;
                let task_text = args
                    .get("task")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Perform the requested task");
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .or(request.source_run_id.as_deref());

                // Validate URL scheme and block private IPs
                if !agent_url.starts_with("http://") && !agent_url.starts_with("https://") {
                    return Ok(ToolCallInternalResult::blocked(
                        ExecutionBlockReason::InvalidArguments,
                        format!("Invalid A2A URL scheme: {}", agent_url),
                    ));
                }
                if is_private_url(agent_url) {
                    return Ok(ToolCallInternalResult::blocked(
                        ExecutionBlockReason::DomainBlocked,
                        format!(
                            "A2A agent URL '{}' points to a private address and is blocked",
                            agent_url
                        ),
                    ));
                }

                // Run A2A call on blocking thread
                call_a2a_agent_blocking(agent_url, task_text, session_id)
            }
            _ => Ok(ToolCallInternalResult::failure(
                ExecutionFailureKind::ToolRuntimeError,
                format!("Unknown execution tool: {}", tool_name),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NetworkPolicy;

    fn make_policy(enabled: bool, default_decision: &str) -> NetworkPolicy {
        NetworkPolicy {
            enabled,
            default_decision: default_decision.to_string(),
            ..NetworkPolicy::default()
        }
    }

    #[test]
    fn check_network_policy_enabled_false_blocks() {
        let policy = make_policy(false, "allow");
        let result = check_network_policy("web.search", &policy, None, false);
        assert!(result.is_some());
        assert!(result
            .unwrap()
            .error
            .unwrap()
            .contains("disabled by policy"));
    }

    #[test]
    fn check_network_policy_tool_override_deny_blocks() {
        let mut policy = make_policy(true, "allow");
        policy
            .tool_overrides
            .insert("web.search".into(), "deny".into());
        let result = check_network_policy("web.search", &policy, None, false);
        assert!(result.is_some());
        assert!(result
            .unwrap()
            .error
            .unwrap()
            .contains("denied by network policy override"));
    }

    #[test]
    fn check_network_policy_domain_denylist_blocks() {
        let mut policy = make_policy(true, "allow");
        policy.domain_denylist.push("evil.com".to_string());
        let result =
            check_network_policy("web.fetch", &policy, Some("https://evil.com/page"), false);
        assert!(result.is_some());
        assert!(result.unwrap().error.unwrap().contains("network denylist"));
    }

    #[test]
    fn check_network_policy_domain_allowlist_blocks_non_listed() {
        let mut policy = make_policy(true, "allow");
        policy.domain_allowlist.push("example.com".to_string());
        let result =
            check_network_policy("web.fetch", &policy, Some("https://other.com/page"), false);
        assert!(result.is_some());
        assert!(result.unwrap().error.unwrap().contains("network allowlist"));
    }

    #[test]
    fn check_network_policy_domain_allowlist_allows_listed() {
        let mut policy = make_policy(true, "allow");
        policy.domain_allowlist.push("example.com".to_string());
        let result = check_network_policy(
            "web.fetch",
            &policy,
            Some("https://example.com/page"),
            false,
        );
        assert!(result.is_none());
    }

    #[test]
    fn check_network_policy_default_deny_blocks() {
        let policy = make_policy(true, "deny");
        let result = check_network_policy("web.search", &policy, None, false);
        assert!(result.is_some());
        assert!(result
            .unwrap()
            .error
            .unwrap()
            .contains("default_decision=deny"));
    }

    #[test]
    fn check_network_policy_default_ask_returns_needs_confirmation() {
        let policy = make_policy(true, "ask");
        let result = check_network_policy("web.search", &policy, None, false);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(
            r.proposal_reason,
            Some(ExecutionProposalReason::NetworkPolicyAsk),
            "default_decision=ask should have typed proposal_reason"
        );
        assert!(r.error.unwrap().contains("default_decision=ask"));
    }

    #[test]
    fn check_network_policy_default_ask_skipped_when_already_permitted() {
        let policy = make_policy(true, "ask");
        let result = check_network_policy("web.search", &policy, None, true);
        assert!(
            result.is_none(),
            "already_permitted should skip default_decision=ask"
        );
    }

    #[test]
    fn check_network_policy_default_allow_passes() {
        let policy = make_policy(true, "allow");
        let result = check_network_policy("web.search", &policy, None, false);
        assert!(result.is_none());
    }

    #[test]
    fn check_network_policy_blocks_a2a_with_disabled() {
        let policy = make_policy(false, "allow");
        let result = check_network_policy(
            "a2a.call_agent",
            &policy,
            Some("https://example.com/a2a"),
            false,
        );
        assert!(result.is_some());
        assert!(result
            .unwrap()
            .error
            .unwrap()
            .contains("disabled by policy"));
    }

    #[test]
    fn check_network_policy_a2a_domain_enforcement() {
        let mut policy = make_policy(true, "allow");
        policy.domain_denylist.push("malicious.com".to_string());
        let result = check_network_policy(
            "a2a.call_agent",
            &policy,
            Some("https://malicious.com/a2a"),
            false,
        );
        assert!(result.is_some());
        assert!(result.unwrap().error.unwrap().contains("network denylist"));
    }

    #[test]
    fn check_network_policy_mcp_call_tool_no_domain_check() {
        let mut policy = make_policy(true, "allow");
        policy.domain_denylist.push("evil.com".to_string());
        let result = check_network_policy("mcp.call_tool", &policy, None, false);
        assert!(
            result.is_none(),
            "mcp without URL should not hit domain denylist"
        );
    }

    #[test]
    fn check_network_policy_hard_blocks_not_bypassed_by_permitted_flag() {
        // enabled=false, tool_override=deny, domain denylist must still block
        // even when already_permitted=true
        let mut policy = make_policy(true, "ask");
        policy
            .tool_overrides
            .insert("web.search".into(), "deny".into());
        let result = check_network_policy("web.search", &policy, None, true);
        assert!(
            result.is_some(),
            "tool_override=deny must block even when permitted"
        );
        assert!(result.unwrap().error.unwrap().contains("override"));
    }

    #[test]
    fn check_network_policy_default_deny_skipped_when_already_permitted() {
        let policy = make_policy(true, "deny");
        let result = check_network_policy("web.search", &policy, None, true);
        assert!(
            result.is_none(),
            "already_permitted should skip default_decision=deny"
        );
    }
}
