use crate::agent::action_executor::helpers::{
    call_a2a_agent, canonical_tool_source, ensure_external_write_content_size,
    external_write_content_preview, extract_host_from_url, fetch_url_async,
    filesystem_access_error, is_direct_external_write_tool, is_path_in_safe_paths_async,
    is_path_lexically_in_safe_paths, policy_requires_external_write_proposal,
    prepare_web_content_observation, reserve_web_search_rate_limit, search_web_async,
    ToolCallInternalResult,
};
use crate::agent::review_workflow::{DurableWriteRequest, DurableWriteSource, DurableWriteSubject};
use crate::agent::types::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use crate::tool_execution_receipt::ToolExecutionReceiptTracker;
use crate::tool_manifest::{ToolManifest, ToolSource};
use anyhow::Result;
use ring::digest::{digest, SHA256};
use serde_json::Value;

use super::ActionExecutionContext;
use super::AgentActionRequest;

impl super::ActionExecutor {
    /// Execute an Execution tool (file.read, web.fetch, etc.).
    // ToolGateway keeps action, cancellation, queue, and authorized network
    // capabilities explicit at the execution boundary.
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub(crate) async fn execute_execution_tool(
        &self,
        tool_name: &str,
        args: &Value,
        ctx: &ActionExecutionContext<'_>,
        request: &AgentActionRequest,
        manifest: &ToolManifest,
        receipt_tracker: ToolExecutionReceiptTracker,
        authorized_network_policy: Option<&crate::config::NetworkPolicy>,
    ) -> Result<ToolCallInternalResult> {
        let network_policy = authorized_network_policy.or(ctx.network_policy);
        // Check network policy for web tools
        if matches!(tool_name, "web.fetch" | "web.search") {
            if let Some(policy) = network_policy {
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
                            if policy
                                .domain_denylist
                                .iter()
                                .any(|rule| crate::network_client::domain_matches(&host, rule))
                            {
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
                                && !policy
                                    .domain_allowlist
                                    .iter()
                                    .any(|rule| crate::network_client::domain_matches(&host, rule))
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

        let mut result = match tool_name {
            "document.read" => {
                let message_id = args
                    .get("message_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("document_read_message_id_missing"))?;
                let query = args
                    .get("query")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("document_read_query_missing"))?;
                let selection_request_id = args
                    .get("selection_request_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("document_read_selection_request_id_missing"))?;
                let privacy_decision_id = args
                    .get("privacy_decision_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                    anyhow::anyhow!("document_read_privacy_decision_id_missing")
                })?;
                let Some(store) = ctx.resource_store else {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some("document_read_resource_store_unavailable".into()),
                    });
                };
                if message_id.trim().is_empty() || query.trim().is_empty() {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some("document_read_bound_input_invalid".into()),
                    });
                }
                ctx.authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                    .await?
                    .observe_local()
                    .await?;
                let selected = crate::resource_selection::DeterministicResourceSelector
                    .select_for_message(
                        store,
                        selection_request_id,
                        privacy_decision_id,
                        message_id,
                        query,
                        vec![crate::llm::ProviderPayloadCategory::CurrentUserConversation],
                    );
                match selected {
                    Ok(selected) if selected.context_blocks.is_empty() => {
                        Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some("document_read_no_bound_content".into()),
                        })
                    }
                    Ok(selected) => {
                        let chunks = selected
                            .context_blocks
                            .iter()
                            .map(|block| {
                                serde_json::json!({
                                    "sourceRef": block.source_ref,
                                    "content": block.content,
                                })
                            })
                            .collect::<Vec<_>>();
                        Ok(ToolCallInternalResult {
                            success: true,
                            output: Some(
                                serde_json::json!({
                                    "schemaVersion": 1,
                                    "messageId": message_id,
                                    "selectionDigest": selected.citation_set.selection_digest(),
                                    "selectedChunkCount": chunks.len(),
                                    "chunks": chunks,
                                })
                                .to_string(),
                            ),
                            error: None,
                        })
                    }
                    Err(error) => Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!("document_read_selection_failed:{error}")),
                    }),
                }
            }
            "file.read" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument for file.read"))?;

                // Validate path is within safe_paths
                if !is_path_lexically_in_safe_paths(path, ctx.safe_paths) {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(filesystem_access_error(path, ctx.safe_paths)),
                    });
                }

                // From this point onward every filesystem observation is an
                // adapter attempt owned by ToolGateway. A missing file cannot
                // be downgraded to a caller-shaped pre-gateway blocker.
                ctx.authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                    .await?
                    .observe_local()
                    .await?;
                if !is_path_in_safe_paths_async(path, ctx.safe_paths).await {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(filesystem_access_error(path, ctx.safe_paths)),
                    });
                }

                let max_size = 100 * 1024; // 100KB limit
                let execution = match tokio::fs::metadata(path).await {
                    Err(error) => ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to read file metadata: {error}")),
                    },
                    Ok(metadata) if metadata.len() > max_size => ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "File size ({} bytes) exceeds maximum allowed ({} bytes)",
                            metadata.len(),
                            max_size
                        )),
                    },
                    Ok(_) => match tokio::fs::read_to_string(path).await {
                        Ok(content) => ToolCallInternalResult {
                            success: true,
                            output: Some(content),
                            error: None,
                        },
                        Err(error) => ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(format!("Failed to read file '{path}': {error}")),
                        },
                    },
                };
                Ok(execution)
            }
            "calendar.read" => {
                let range_start = args.get("range_start").and_then(|v: &Value| v.as_str());
                let range_end = args.get("range_end").and_then(|v: &Value| v.as_str());

                // Use the ICS file path from args or from safe_paths
                let ics_path = args.get("source").and_then(|v: &Value| v.as_str());
                let mut all_calendar_paths: Vec<String> = ctx.calendar_ics_paths.to_vec();
                all_calendar_paths.extend(ctx.safe_paths.iter().cloned());

                let events = if let Some(path) = ics_path {
                    // Validate source path is within calendar_ics_paths or safe_paths
                    if !is_path_lexically_in_safe_paths(path, &all_calendar_paths) {
                        return Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(filesystem_access_error(path, &all_calendar_paths)),
                        });
                    }
                    ctx.authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                        .await?
                        .observe_local()
                        .await?;
                    if !is_path_in_safe_paths_async(path, &all_calendar_paths).await {
                        return Ok(ToolCallInternalResult {
                            success: false,
                            output: None,
                            error: Some(filesystem_access_error(path, &all_calendar_paths)),
                        });
                    }
                    let metadata = match tokio::fs::metadata(path).await {
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
                    match tokio::fs::read_to_string(path).await {
                        Ok(content) => crate::calendar::parse_ics(&content, range_start, range_end),
                        Err(e) => {
                            return Ok(complete_local_early_result(
                                &receipt_tracker,
                                ToolCallInternalResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!(
                                        "Failed to read ICS file '{}': {}",
                                        path, e
                                    )),
                                },
                            ));
                        }
                    }
                } else {
                    // Try calendar_ics_paths first, then safe_paths for .ics files
                    let ics_search_paths: Vec<&String> = if ctx.calendar_ics_paths.is_empty() {
                        ctx.safe_paths.iter().collect()
                    } else {
                        ctx.calendar_ics_paths.iter().collect()
                    };
                    ctx.authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                        .await?
                        .observe_local()
                        .await?;
                    let mut all_events = Vec::new();
                    for search_path in &ics_search_paths {
                        if !is_path_in_safe_paths_async(search_path, &all_calendar_paths).await {
                            continue;
                        }
                        if let Ok(mut entries) = tokio::fs::read_dir(search_path).await {
                            while let Ok(Some(entry)) = entries.next_entry().await {
                                let path = entry.path();
                                if path.extension() == Some(std::ffi::OsStr::new("ics")) {
                                    if let Ok(content) = tokio::fs::read_to_string(&path).await {
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
                        return Ok(complete_local_early_result(
                            &receipt_tracker,
                            ToolCallInternalResult {
                                success: false,
                                output: None,
                                error: Some(
                                    "No .ics files found in safe_paths. Configure calendar_ics_paths in Settings or provide 'source' argument.".to_string(),
                                ),
                            },
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

                let admission = ctx
                    .authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                    .await?;
                let result = fetch_url_async(url, network_policy, admission).await?;
                // Keep model synthesis inside the active TurnRuntime. The tool returns an
                // explicitly untrusted, bounded observation instead of starting a hidden
                // provider request of its own.
                let summarize = args
                    .get("summarize")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if summarize && result.success && result.output.is_some() {
                    let content = result.output.clone().unwrap_or_default();
                    Ok(ToolCallInternalResult {
                        success: true,
                        output: Some(prepare_web_content_observation(&content, url)),
                        error: None,
                    })
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
                    ctx.authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                        .await?
                        .observe_simulated()
                        .await?;
                    receipt_tracker.mark_response_observed();
                    super::tool_executor::record_effect_outcome(&receipt_tracker, true);
                    return Ok(ToolCallInternalResult {
                        success: true,
                        output: Some(fixture_output.to_string()),
                        error: None,
                    });
                }

                if let Some(error) = reserve_web_search_rate_limit() {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(error),
                    });
                }

                let admission = ctx
                    .authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                    .await?;
                search_web_async(
                    query,
                    max_results,
                    &self.config.search_provider,
                    network_policy,
                    admission,
                )
                .await
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

                if policy_requires_external_write_proposal(ctx)
                    && is_direct_external_write_tool(&target_manifest)
                {
                    ctx.authorize_tool_dispatch(
                        &target_manifest,
                        request,
                        &tool_args,
                        &receipt_tracker,
                    )
                    .await?
                    .observe_local()
                    .await?;
                    let proposal_result = match self.create_external_write_action_proposal_record(
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
                    return match proposal_result {
                        Ok(result) => Ok(complete_local_early_result(&receipt_tracker, result)),
                        Err(error) => {
                            receipt_tracker.mark_response_observed();
                            super::tool_executor::record_effect_outcome(&receipt_tracker, false);
                            Err(error)
                        }
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
                let admission = ctx
                    .authorize_tool_dispatch(
                        &target_manifest,
                        request,
                        &tool_args,
                        &receipt_tracker,
                    )
                    .await?;
                Ok(self
                    .call_tool_internal(
                        &target_manifest,
                        tool_args,
                        ctx,
                        inspection.pii_found,
                        admission,
                    )
                    .await)
            }
            "file.write_proposal" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

                if path.is_empty() {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some("file.write_proposal requires a non-empty path".into()),
                    });
                }

                // Validate path is within safe_paths
                if !is_path_lexically_in_safe_paths(path, ctx.safe_paths) {
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
                let operation = if tokio::fs::try_exists(path).await.unwrap_or(false) {
                    "overwrite"
                } else {
                    "create"
                };
                let content_preview = external_write_content_preview(content);

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
                if let Some(ref run_id) = request.source_run_id {
                    proposal.run_id = Some(run_id.clone());
                }
                ctx.authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                    .await?
                    .observe_local()
                    .await?;
                let proposal_id =
                    match ctx.submit_review_proposal(DurableWriteRequest::from_agent_proposal(
                        DurableWriteSource::ToolPermission,
                        DurableWriteSubject::FileWrite,
                        proposal,
                        "File write proposal is pending Review Center approval.",
                    )) {
                        Ok(outcome) => outcome.proposal_id().to_string(),
                        Err(error) => {
                            return Ok(complete_local_early_result(
                                &receipt_tracker,
                                ToolCallInternalResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!(
                                        "Failed to create ExternalWriteAction Proposal: {error}"
                                    )),
                                },
                            ));
                        }
                    };

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
                if let Some(obj) = result_payload.as_object_mut() {
                    obj.insert("proposal_id".to_string(), serde_json::json!(proposal_id));
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

                if ctx.proposal_store.is_some() {
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
                    ctx.authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                        .await?
                        .observe_local()
                        .await?;
                    match ctx.submit_review_proposal(DurableWriteRequest::from_agent_proposal(
                        DurableWriteSource::ToolPermission,
                        DurableWriteSubject::Calendar,
                        proposal,
                        "Calendar event proposal is pending Review Center approval.",
                    )) {
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

                if ctx.proposal_store.is_some() {
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
                    ctx.authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                        .await?
                        .observe_local()
                        .await?;
                    match ctx.submit_review_proposal(DurableWriteRequest::from_agent_proposal(
                        DurableWriteSource::ToolPermission,
                        DurableWriteSubject::Email,
                        proposal,
                        "Email draft proposal is pending Review Center approval.",
                    )) {
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
                let mut task_args = serde_json::json!({
                    "title": title,
                    "description": description,
                    "due_date": due_date,
                    "scheduled_at": due_date,
                    "priority": priority,
                    "tool": "task.create_proposal",
                });
                if let Some(provider_route) = args.get("provider_route") {
                    if let Some(task_args) = task_args.as_object_mut() {
                        task_args.insert("provider_route".into(), provider_route.clone());
                    }
                }

                if ctx.proposal_store.is_some() {
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
                    ctx.authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                        .await?
                        .observe_local()
                        .await?;
                    match ctx.submit_review_proposal(DurableWriteRequest::from_agent_proposal(
                        DurableWriteSource::ToolPermission,
                        DurableWriteSubject::Task,
                        proposal,
                        "Task proposal is pending Review Center approval.",
                    )) {
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
                let request_id = args.get("request_id").and_then(|v| v.as_str());

                // Validate URL scheme and block private IPs
                if !agent_url.starts_with("http://") && !agent_url.starts_with("https://") {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!("Invalid A2A URL scheme: {}", agent_url)),
                    });
                }
                let admission = ctx
                    .authorize_tool_dispatch(manifest, request, args, &receipt_tracker)
                    .await?;
                call_a2a_agent(
                    agent_url,
                    task_text,
                    session_id,
                    request_id,
                    admission,
                    ctx.a2a_outbound_authorization,
                )
                .await
            }
            _ => Ok(ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!("Unknown execution tool: {}", tool_name)),
            }),
        }?;

        finalize_adapter_result(&receipt_tracker, &mut result);
        Ok(result)
    }
}

fn finalize_adapter_result(
    receipt_tracker: &ToolExecutionReceiptTracker,
    result: &mut ToolCallInternalResult,
) {
    use crate::tool_execution_receipt::{ToolDispatchKind, ToolTransportStatus};

    let snapshot = receipt_tracker.snapshot();
    match snapshot.transport_status {
        ToolTransportStatus::Dispatched
            if matches!(
                snapshot.dispatch_kind,
                ToolDispatchKind::Local | ToolDispatchKind::Simulated
            ) =>
        {
            receipt_tracker.mark_response_observed();
            super::tool_executor::record_effect_outcome(receipt_tracker, result.success);
        }
        ToolTransportStatus::ResponseObserved => {
            super::tool_executor::record_effect_outcome(receipt_tracker, result.success);
        }
        ToolTransportStatus::Dispatched => {
            receipt_tracker.mark_remote_unknown();
            if result.success {
                result.success = false;
                result.output = None;
                result.error = Some("tool_adapter_success_without_observed_response".into());
            }
        }
        ToolTransportStatus::NotAttempted if result.success => {
            result.success = false;
            result.output = None;
            result.error = Some("tool_adapter_success_without_dispatch_receipt".into());
        }
        ToolTransportStatus::NotAttempted
        | ToolTransportStatus::LocalAborted
        | ToolTransportStatus::RemoteUnknown => {}
    }
}

fn complete_local_early_result(
    receipt_tracker: &ToolExecutionReceiptTracker,
    mut result: ToolCallInternalResult,
) -> ToolCallInternalResult {
    finalize_adapter_result(receipt_tracker, &mut result);
    result
}
