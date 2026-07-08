use crate::agent::action_executor::helpers::is_private_url;
use crate::agent::action_executor::helpers::{
    call_a2a_agent_blocking, canonical_tool_source, ensure_external_write_content_size,
    external_write_content_preview, extract_host_from_url, fetch_url_on_worker_thread,
    filesystem_access_error, hs_requires_external_write_proposal, is_direct_external_write_tool,
    is_path_in_safe_paths, search_web_on_worker_thread, summarize_content_blocking,
    ToolCallInternalResult,
};
use crate::agent::review_workflow::{
    DurableWriteRequest, DurableWriteSource, DurableWriteSubject, ReviewWorkflow,
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

                let result = fetch_url_on_worker_thread(url)?;
                // Optional summarization via Ollama
                let summarize = args
                    .get("summarize")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if summarize && result.success && result.output.is_some() {
                    let content = result.output.clone().unwrap_or_default();
                    match summarize_content_blocking(&content, url) {
                        Ok(summary) => Ok(ToolCallInternalResult {
                            success: true,
                            output: Some(summary),
                            error: None,
                        }),
                        Err(_) => {
                            // Summarization failed, return original content
                            Ok(ToolCallInternalResult {
                                success: true,
                                output: Some(content),
                                error: Some(
                                    "Content summarization failed, showing raw content".to_string(),
                                ),
                            })
                        }
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
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some("Search query cannot be empty".to_string()),
                    });
                }

                if let Some(fixture_output) = ctx.web_search_fixture_output {
                    return Ok(ToolCallInternalResult {
                        success: true,
                        output: Some(fixture_output.to_string()),
                        error: None,
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

                if hs_requires_external_write_proposal(ctx)
                    && is_direct_external_write_tool(&target_manifest)
                {
                    return match self.create_external_write_action_proposal_record(
                        request,
                        ctx,
                        &target_manifest.name,
                        &tool_args,
                        &target_manifest,
                    ) {
                        Some(Ok(proposal_id)) => Ok(ToolCallInternalResult {
                            success: false,
                            output: Some(serde_json::json!({
                                "proposal_required": true,
                                "proposal_type": "external_write_action",
                                "proposal_id": proposal_id.clone(),
                                "target_tool": target_manifest.name,
                            }).to_string()),
                            error: Some(format!(
                                "hs_external_write_proposal_first: created ExternalWriteAction proposal (id: {})",
                                proposal_id
                            )),
                        }),
                        Some(Err(e)) => Err(e),
                        None => Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(
                                "hs_external_write_proposal_first: proposal store unavailable; direct execution blocked"
                                    .to_string(),
                            ),
                        }),
                    };
                }

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

                if let Err(e) = ensure_external_write_content_size(content) {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(e.to_string()),
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
                let content_preview = external_write_content_preview(content);

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
                        match ReviewWorkflow::new(proposal_store).submit(
                            DurableWriteRequest::from_agent_proposal(
                                DurableWriteSource::ToolPermission,
                                DurableWriteSubject::FileWrite,
                                proposal,
                                "File write proposal is pending Review Center approval.",
                            ),
                        ) {
                            Ok(outcome) => proposal_id = Some(outcome.proposal_id().to_string()),
                            Err(e) => {
                                eprintln!(
                                    "[warn] Failed to create ExternalWriteAction Proposal: {}",
                                    e
                                );
                            }
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
            "calendar.propose_event" => {
                let title = args
                    .get("title")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("Untitled Event");
                let scheduled_at = args
                    .get("scheduled_at")
                    .or_else(|| args.get("date"))
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("");
                let description = args
                    .get("description")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("");
                let location = args
                    .get("location")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("");

                let after = serde_json::json!({
                    "title": title,
                    "description": description,
                    "location": location,
                    "scheduled_at": scheduled_at,
                    "priority": args.get("priority").and_then(|v| v.as_str()).unwrap_or("medium"),
                    "tool": "calendar.propose_event",
                    "proposal_kind": "calendar_event",
                });

                if let Some(proposal_store) = ctx.proposal_store {
                    let mut proposal = AgentProposal::new(
                        ProposalType::ScheduledTask,
                        "calendar.events",
                        after,
                        &format!("Agent proposed calendar event: {}", title),
                        0.85,
                        RiskLevel::Medium,
                        ProposalSource::Manual,
                    );
                    if let Some(ref run_id) = request.source_run_id {
                        proposal.run_id = Some(run_id.clone());
                    }
                    match ReviewWorkflow::new(proposal_store).submit(
                        DurableWriteRequest::from_agent_proposal(
                            DurableWriteSource::ToolPermission,
                            DurableWriteSubject::Calendar,
                            proposal,
                            "Calendar event proposal is pending Review Center approval.",
                        ),
                    ) {
                        Ok(outcome) => Ok(ToolCallInternalResult {
                            success: true,
                            output: Some(
                                serde_json::json!({
                                    "status": "proposal_created",
                                    "proposal_id": outcome.proposal_id(),
                                    "proposal_type": "scheduled_task",
                                    "title": title,
                                })
                                .to_string(),
                            ),
                            error: None,
                        }),
                        Err(e) => Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(format!("Failed to create calendar proposal: {}", e)),
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
            "email.propose_draft" => {
                let to = args.get("to").and_then(|v| v.as_str()).unwrap_or("");
                let cc = args.get("cc").and_then(|v| v.as_str()).unwrap_or("");
                let bcc = args.get("bcc").and_then(|v| v.as_str()).unwrap_or("");
                let subject = args.get("subject").and_then(|v| v.as_str()).unwrap_or("");
                let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");

                let after = serde_json::json!({
                    "to": to,
                    "cc": cc,
                    "bcc": bcc,
                    "subject": subject,
                    "body": body,
                    "content": body,
                    "filename": "email-draft.txt",
                    "tool": "email.propose_draft",
                    "proposal_kind": "email_draft",
                });

                if let Some(proposal_store) = ctx.proposal_store {
                    let mut proposal = AgentProposal::new(
                        ProposalType::DataExport,
                        "email.drafts",
                        after,
                        &format!("Agent proposed email draft: {}", subject),
                        0.85,
                        RiskLevel::Medium,
                        ProposalSource::Manual,
                    );
                    if let Some(ref run_id) = request.source_run_id {
                        proposal.run_id = Some(run_id.clone());
                    }
                    match ReviewWorkflow::new(proposal_store).submit(
                        DurableWriteRequest::from_agent_proposal(
                            DurableWriteSource::ToolPermission,
                            DurableWriteSubject::Email,
                            proposal,
                            "Email draft proposal is pending Review Center approval.",
                        ),
                    ) {
                        Ok(outcome) => Ok(ToolCallInternalResult {
                            success: true,
                            output: Some(
                                serde_json::json!({
                                    "status": "proposal_created",
                                    "proposal_id": outcome.proposal_id(),
                                    "proposal_type": "data_export",
                                    "proposal_kind": "email_draft",
                                    "subject": subject,
                                })
                                .to_string(),
                            ),
                            error: None,
                        }),
                        Err(e) => Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(format!("Failed to create email draft proposal: {}", e)),
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
                    match ReviewWorkflow::new(proposal_store).submit(
                        DurableWriteRequest::from_agent_proposal(
                            DurableWriteSource::ToolPermission,
                            DurableWriteSubject::Calendar,
                            proposal,
                            "Task proposal is pending Review Center approval.",
                        ),
                    ) {
                        Ok(outcome) => {
                            let output = serde_json::json!({
                                "status": "proposal_created",
                                "proposal_id": outcome.proposal_id(),
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
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!("Invalid A2A URL scheme: {}", agent_url)),
                    });
                }
                if is_private_url(agent_url) {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "A2A agent URL '{}' points to a private address and is blocked",
                            agent_url
                        )),
                    });
                }

                // Run A2A call on blocking thread
                call_a2a_agent_blocking(agent_url, task_text, session_id)
            }
            _ => Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!("Unknown execution tool: {}", tool_name)),
            }),
        }
    }
}
