use crate::agent::action_executor::helpers::is_private_url;
use crate::agent::action_executor::helpers::{
    canonical_tool_source, extract_host_from_url, fetch_url_on_worker_thread,
    filesystem_access_error, is_path_in_safe_paths, search_web_on_worker_thread,
    ToolCallInternalResult,
};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use crate::tool_manifest::ToolSource;
use anyhow::Result;
use ring::digest::{digest, SHA256};
use serde_json::Value;

use super::ActionExecutionContext;
use super::AgentActionRequest;

impl super::ActionExecutor {
    /// Execute an Execution tool (file.read, web.fetch, etc.).
    pub fn execute_execution_tool(
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
            "calendar.read" => {
                let range_start = args.get("range_start").and_then(|v: &Value| v.as_str());
                let range_end = args.get("range_end").and_then(|v: &Value| v.as_str());

                // Use the ICS file path from args or from safe_paths
                let ics_path = args.get("source").and_then(|v: &Value| v.as_str());

                let events = if let Some(path) = ics_path {
                    // Validate source path is within calendar_ics_paths or safe_paths
                    let mut all_calendar_paths: Vec<String> = ctx.calendar_ics_paths.to_vec();
                    all_calendar_paths.extend(ctx.safe_paths.iter().cloned());
                    if !is_path_in_safe_paths(path, &all_calendar_paths) {
                        return Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(filesystem_access_error(path, &all_calendar_paths)),
                        });
                    }
                    let metadata = match std::fs::metadata(path) {
                        Ok(m) => m,
                        Err(e) => {
                            return Ok(ToolCallInternalResult {
                                success: false,
                                output: None,
                                error: Some(format!("Failed to read ICS file metadata: {}", e)),
                            });
                        }
                    };
                    let max_size = 100 * 1024; // 100KB limit, same as file.read
                    if metadata.len() > max_size {
                        return Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(format!(
                                "ICS file size ({} bytes) exceeds maximum allowed ({} bytes)",
                                metadata.len(),
                                max_size
                            )),
                        });
                    }
                    match std::fs::read_to_string(path) {
                        Ok(content) => crate::calendar::parse_ics(&content, range_start, range_end),
                        Err(e) => {
                            return Ok(ToolCallInternalResult {
                                success: false,
                                output: None,
                                error: Some(format!("Failed to read ICS file '{}': {}", path, e)),
                            });
                        }
                    }
                } else {
                    // Try calendar_ics_paths first, then safe_paths for .ics files
                    let ics_search_paths: Vec<&String> = if ctx.calendar_ics_paths.is_empty() {
                        ctx.safe_paths.iter().collect()
                    } else {
                        ctx.calendar_ics_paths.iter().collect()
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
                        return Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(
                                "No .ics files found in safe_paths. Configure calendar_ics_paths in Settings or provide 'source' argument.".to_string(),
                            ),
                        });
                    }
                    all_events
                };

                let output = serde_json::json!({
                    "status": "success",
                    "events": events,
                    "count": events.len(),
                })
                .to_string();
                Ok(ToolCallInternalResult {
                    success: true,
                    output: Some(output),
                    error: None,
                })
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
                    .unwrap_or(crate::tool_permissions::ToolPermissionDecision {
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

                // Check if we have a proposal store (create proposal first for user confirmation)
                let task_args = serde_json::json!({
                    "title": title,
                    "description": description,
                    "due_date": due_date,
                    "scheduled_at": due_date,
                    "priority": priority,
                    "tool": "task.create_proposal",
                });

                if let Some(proposal_store) = ctx.proposal_store {
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
                    match proposal_store.create_proposal(&proposal) {
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
                            })
                        }
                        Err(e) => Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(format!("Failed to create task proposal: {}", e)),
                        }),
                    }
                } else {
                    Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some("ProposalStore not available in execution context".to_string()),
                    })
                }
            }
            _ => Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!("Unknown execution tool: {}", tool_name)),
            }),
        }
    }
}
