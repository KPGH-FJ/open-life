use crate::agent::action_executor::helpers::{
    extract_host_from_url, fetch_url_async, filesystem_access_error, is_path_in_safe_paths_async,
    is_path_lexically_in_safe_paths, prepare_web_content_observation,
    reserve_web_search_rate_limit, search_web_async, ToolCallInternalResult,
};
use crate::tool_execution_receipt::ToolExecutionReceiptTracker;
use crate::tool_manifest::ToolManifest;
use anyhow::Result;
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
