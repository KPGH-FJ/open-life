use crate::agent::action_executor::helpers::{
    extract_host_from_url, fetch_url_async, filesystem_access_error, is_path_in_safe_paths_async,
    is_path_lexically_in_safe_paths, prepare_web_content_observation,
    reserve_web_search_rate_limit, search_web_async, ToolCallInternalResult,
};
use crate::tool_execution_receipt::ToolExecutionReceiptTracker;
use crate::tool_manifest::ToolManifest;
use anyhow::Result;
use serde_json::Value;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

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

                let binary_document = project_document_mime(Path::new(path));
                let image_candidate = project_image_extension(Path::new(path));
                let max_size = if binary_document.is_some() || image_candidate {
                    crate::resource::MAX_RESOURCE_BYTES as u64
                } else {
                    100 * 1024
                };
                let execution = match tokio::fs::metadata(path).await {
                    Err(error) => ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to read file metadata: {error}")),
                    },
                    Ok(metadata) if !metadata.is_file() => ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some("file_read_target_is_not_regular_file".into()),
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
                    Ok(_) => {
                        if let Some(declared_mime) = binary_document {
                            read_project_document(
                                path,
                                args.get("workspaceRelativePath")
                                    .and_then(Value::as_str)
                                    .unwrap_or_else(|| {
                                        Path::new(path)
                                            .file_name()
                                            .and_then(|value| value.to_str())
                                            .unwrap_or(path)
                                    }),
                                declared_mime,
                                ctx,
                            )
                            .await
                        } else if image_candidate {
                            read_project_image(
                                path,
                                args.get("workspaceRelativePath")
                                    .and_then(Value::as_str)
                                    .unwrap_or_else(|| {
                                        Path::new(path)
                                            .file_name()
                                            .and_then(|value| value.to_str())
                                            .unwrap_or(path)
                                    }),
                            )
                            .await
                        } else {
                            match tokio::fs::read_to_string(path).await {
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
                            }
                        }
                    }
                };
                Ok(execution)
            }
            "folder.list" => {
                let path = required_filesystem_path(args, "folder.list")?;
                if !is_path_lexically_in_safe_paths(path, ctx.safe_paths) {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(filesystem_access_error(path, ctx.safe_paths)),
                    });
                }
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
                let max_entries = args
                    .get("maxEntries")
                    .and_then(Value::as_u64)
                    .unwrap_or(100)
                    .clamp(1, 200) as usize;
                let path = PathBuf::from(path);
                Ok(
                    tokio::task::spawn_blocking(move || bounded_folder_listing(&path, max_entries))
                        .await
                        .map_err(|_| anyhow::anyhow!("folder_list_worker_failed"))?,
                )
            }
            "file.search" => {
                let path = required_filesystem_path(args, "file.search")?;
                let query = args
                    .get("query")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("file_search_query_missing"))?;
                if !is_path_lexically_in_safe_paths(path, ctx.safe_paths) {
                    return Ok(ToolCallInternalResult {
                        success: false,
                        output: None,
                        error: Some(filesystem_access_error(path, ctx.safe_paths)),
                    });
                }
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
                let max_results = args
                    .get("maxResults")
                    .and_then(Value::as_u64)
                    .unwrap_or(20)
                    .clamp(1, 50) as usize;
                let path = PathBuf::from(path);
                let query = query.to_string();
                Ok(tokio::task::spawn_blocking(move || {
                    bounded_file_search(&path, &query, max_results)
                })
                .await
                .map_err(|_| anyhow::anyhow!("file_search_worker_failed"))?)
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

const MAX_PROJECT_DOCUMENT_OBSERVATION_CHARS: usize = 64 * 1024;
const MAX_PROJECT_DOCUMENT_OBSERVATION_CHUNKS: usize = 64;

fn project_document_mime(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "pdf" => Some("application/pdf"),
        "docx" => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "pptx" => Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        _ => None,
    }
}

fn project_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|extension| {
            matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif")
        })
}

async fn read_project_image(path: &str, workspace_relative_path: &str) -> ToolCallInternalResult {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes)
            if !bytes.is_empty()
                && bytes.len() <= crate::llm::MAX_PREPARED_PROVIDER_IMAGE_BYTES =>
        {
            bytes
        }
        Ok(_) => {
            return ToolCallInternalResult {
                success: false,
                output: None,
                error: Some("file_read_image_byte_limit_exceeded".into()),
            }
        }
        Err(error) => {
            return ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!("file_read_image_bytes_failed:{error}")),
            }
        }
    };
    let detected_mime = infer::get(&bytes)
        .map(|kind| kind.mime_type())
        .filter(|mime| {
            matches!(
                *mime,
                "image/png" | "image/jpeg" | "image/webp" | "image/gif"
            )
        });
    let Some(detected_mime) = detected_mime else {
        return ToolCallInternalResult {
            success: false,
            output: None,
            error: Some("file_read_image_magic_invalid".into()),
        };
    };
    let image = match crate::llm::BoundedProviderImage::from_governed_bytes(
        format!("project-image://{workspace_relative_path}"),
        detected_mime,
        bytes,
    ) {
        Ok(image) => image,
        Err(error) => {
            return ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!("file_read_image_contract_invalid:{error}")),
            }
        }
    };
    ToolCallInternalResult {
        success: true,
        output: Some(
            serde_json::json!({
                "schemaVersion": 1,
                "kind": "project_image_observation",
                "workspaceRelativePath": workspace_relative_path,
                "detectedMime": image.mime_type,
                "byteCount": image.byte_count,
                "sha256": image.sha256,
                "providerPayload": "transient_bytes_reloaded_and_verified_before_dispatch",
            })
            .to_string(),
        ),
        error: None,
    }
}

async fn read_project_document(
    path: &str,
    workspace_relative_path: &str,
    declared_mime: &str,
    ctx: &ActionExecutionContext<'_>,
) -> ToolCallInternalResult {
    let Some(parser) = ctx.resource_parser else {
        return ToolCallInternalResult {
            success: false,
            output: None,
            error: Some("file_read_resource_parser_unavailable".into()),
        };
    };
    let path_ref = Path::new(path);
    let Some(filename) = path_ref.file_name().and_then(|value| value.to_str()) else {
        return ToolCallInternalResult {
            success: false,
            output: None,
            error: Some("file_read_filename_invalid".into()),
        };
    };
    let bytes = match tokio::fs::read(path_ref).await {
        Ok(bytes) if bytes.len() <= crate::resource::MAX_RESOURCE_BYTES => bytes,
        Ok(_) => {
            return ToolCallInternalResult {
                success: false,
                output: None,
                error: Some("file_read_resource_byte_limit_exceeded".into()),
            }
        }
        Err(error) => {
            return ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!("file_read_resource_bytes_failed:{error}")),
            }
        }
    };
    let request = crate::resource_parser::ResourceExtractionRequest {
        filename: filename.to_string(),
        declared_mime: declared_mime.to_string(),
        bytes,
    };
    match parser
        .extract(
            request,
            &crate::resource_gateway::ResourceImportCancellation::default(),
        )
        .await
    {
        Ok(extraction) => ToolCallInternalResult {
            success: true,
            output: Some(
                project_document_observation(workspace_relative_path, path_ref, extraction)
                    .to_string(),
            ),
            error: None,
        },
        Err(error) => ToolCallInternalResult {
            success: false,
            output: None,
            error: Some(format!("file_read_resource_parse_failed:{error}")),
        },
    }
}

fn project_document_observation(
    workspace_relative_path: &str,
    path: &Path,
    extraction: crate::resource_parser::ResourceExtraction,
) -> Value {
    let total_chunk_count = extraction.chunks.len();
    let mut remaining_chars = MAX_PROJECT_DOCUMENT_OBSERVATION_CHARS;
    let mut chunks = Vec::new();
    for chunk in extraction
        .chunks
        .into_iter()
        .take(MAX_PROJECT_DOCUMENT_OBSERVATION_CHUNKS)
    {
        if remaining_chars == 0 {
            break;
        }
        let content = chunk
            .content
            .chars()
            .take(remaining_chars)
            .collect::<String>();
        remaining_chars = remaining_chars.saturating_sub(content.chars().count());
        chunks.push(serde_json::json!({
            "content": content,
            "provenance": chunk.provenance,
        }));
    }
    let included_chunk_count = chunks.len();
    serde_json::json!({
        "schemaVersion": 1,
        "kind": "project_document_extraction",
        "filename": path.file_name().and_then(|value| value.to_str()),
        "workspaceRelativePath": workspace_relative_path,
        "detectedMime": extraction.detected_mime,
        "format": extraction.format,
        "expandedBytes": extraction.expanded_bytes,
        "totalChunkCount": total_chunk_count,
        "includedChunkCount": included_chunk_count,
        "truncated": included_chunk_count < total_chunk_count || remaining_chars == 0,
        "chunks": chunks,
    })
}

#[cfg(test)]
mod project_document_read_tests {
    use super::{project_document_mime, project_document_observation, project_image_extension};
    use crate::resource::{ResourceChunkDraft, ResourceFormat, ResourceProvenance};
    use crate::resource_parser::ResourceExtraction;
    use std::path::Path;

    #[test]
    fn recognizes_only_process_isolated_project_document_formats() {
        assert_eq!(
            project_document_mime(Path::new("brief.PDF")),
            Some("application/pdf")
        );
        assert!(project_document_mime(Path::new("slides.pptx")).is_some());
        assert!(project_document_mime(Path::new("photo.png")).is_none());
        assert!(project_document_mime(Path::new("notes.txt")).is_none());
        assert!(project_image_extension(Path::new("photo.PNG")));
        assert!(project_image_extension(Path::new("photo.jpeg")));
        assert!(!project_image_extension(Path::new("photo.svg")));
    }

    #[test]
    fn project_document_observation_preserves_typed_provenance() {
        let observation = project_document_observation(
            "docs/report.pdf",
            Path::new("/safe/report.pdf"),
            ResourceExtraction {
                detected_mime: "application/pdf".into(),
                format: ResourceFormat::Pdf,
                expanded_bytes: 128,
                chunks: vec![ResourceChunkDraft {
                    content: "verified body".into(),
                    provenance: ResourceProvenance::Pdf { page: 3 },
                }],
            },
        );
        assert_eq!(observation["kind"], "project_document_extraction");
        assert_eq!(observation["workspaceRelativePath"], "docs/report.pdf");
        assert_eq!(observation["format"], "pdf");
        assert_eq!(observation["chunks"][0]["provenance"]["kind"], "pdf");
        assert_eq!(observation["chunks"][0]["provenance"]["page"], 3);
        assert_eq!(observation["truncated"], false);
    }

    #[test]
    fn project_document_observation_is_bounded_and_marks_truncation() {
        let observation = project_document_observation(
            "large.docx",
            Path::new("/safe/large.docx"),
            ResourceExtraction {
                detected_mime:
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(),
                format: ResourceFormat::Docx,
                expanded_bytes: 1_000_000,
                chunks: vec![ResourceChunkDraft {
                    content: "x".repeat(super::MAX_PROJECT_DOCUMENT_OBSERVATION_CHARS + 10),
                    provenance: ResourceProvenance::Docx {
                        paragraph_start: 1,
                        paragraph_end: 1,
                    },
                }],
            },
        );
        assert_eq!(observation["truncated"], true);
        assert_eq!(
            observation["chunks"][0]["content"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            super::MAX_PROJECT_DOCUMENT_OBSERVATION_CHARS
        );
    }
}

fn required_filesystem_path<'a>(args: &'a Value, tool: &str) -> Result<&'a str> {
    args.get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{}_path_missing", tool.replace('.', "_")))
}

fn bounded_folder_listing(path: &Path, max_entries: usize) -> ToolCallInternalResult {
    let read_dir = match std::fs::read_dir(path) {
        Ok(read_dir) => read_dir,
        Err(error) => {
            return ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(format!("folder_list_failed:{error}")),
            }
        }
    };
    let mut entries = read_dir
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            Some(serde_json::json!({
                "name": name,
                "kind": if file_type.is_dir() { "directory" } else if file_type.is_file() { "file" } else { "other" },
                "readable": file_type.is_dir() || file_type.is_file(),
            }))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["name"].as_str().unwrap_or_default())
    });
    let truncated = entries.len() > max_entries;
    entries.truncate(max_entries);
    ToolCallInternalResult {
        success: true,
        output: Some(
            serde_json::json!({
                "schemaVersion": "openlife.folder-list.v1",
                "entries": entries,
                "truncated": truncated,
            })
            .to_string(),
        ),
        error: None,
    }
}

fn bounded_file_search(root: &Path, query: &str, max_results: usize) -> ToolCallInternalResult {
    const MAX_VISITED_ENTRIES: usize = 2_000;
    const MAX_DEPTH: usize = 8;
    const MAX_TEXT_BYTES: u64 = 100 * 1024;
    let query_normalized = query.to_lowercase();
    let mut pending = VecDeque::from([(root.to_path_buf(), PathBuf::new(), 0usize)]);
    let mut visited = 0usize;
    let mut matches = Vec::new();
    while let Some((directory, relative_directory, depth)) = pending.pop_front() {
        if depth > MAX_DEPTH || visited >= MAX_VISITED_ENTRIES || matches.len() >= max_results {
            continue;
        }
        let Ok(read_dir) = std::fs::read_dir(&directory) else {
            continue;
        };
        let mut entries = read_dir.filter_map(Result::ok).collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if visited >= MAX_VISITED_ENTRIES || matches.len() >= max_results {
                break;
            }
            visited += 1;
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let relative = relative_directory.join(entry.file_name());
            if file_type.is_dir() {
                pending.push_back((entry.path(), relative, depth + 1));
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let relative_text = relative
                .iter()
                .map(|component| component.to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let name_match = relative_text.to_lowercase().contains(&query_normalized);
            let content = entry
                .metadata()
                .ok()
                .filter(|metadata| metadata.len() <= MAX_TEXT_BYTES)
                .and_then(|_| std::fs::read_to_string(entry.path()).ok());
            let content_match = content
                .as_deref()
                .is_some_and(|content| content.to_lowercase().contains(&query_normalized));
            if !name_match && !content_match {
                continue;
            }
            let snippet = content.map(|content| {
                let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
                normalized.chars().take(240).collect::<String>()
            });
            matches.push(serde_json::json!({
                "path": relative_text,
                "matchedName": name_match,
                "matchedContent": content_match,
                "snippet": snippet,
            }));
        }
    }
    let truncated =
        visited >= MAX_VISITED_ENTRIES || matches.len() >= max_results || !pending.is_empty();
    ToolCallInternalResult {
        success: true,
        output: Some(
            serde_json::json!({
                "schemaVersion": "openlife.file-search.v1",
                "query": query,
                "results": matches,
                "visitedEntries": visited,
                "truncated": truncated,
            })
            .to_string(),
        ),
        error: None,
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

#[cfg(test)]
mod project_read_tool_tests {
    use super::{bounded_file_search, bounded_folder_listing};

    #[test]
    fn folder_listing_is_bounded_sorted_and_never_exposes_absolute_paths() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("中.txt"), "content").unwrap();
        std::fs::create_dir(root.path().join("a-dir")).unwrap();
        std::fs::write(root.path().join("b.md"), "content").unwrap();

        let result = bounded_folder_listing(root.path(), 2);
        assert!(result.success);
        let output = result.output.unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["schemaVersion"], "openlife.folder-list.v1");
        assert_eq!(value["entries"].as_array().unwrap().len(), 2);
        assert_eq!(value["entries"][0]["name"], "a-dir");
        assert_eq!(value["entries"][1]["name"], "b.md");
        assert_eq!(value["truncated"], true);
        assert!(!output.contains(&root.path().to_string_lossy().into_owned()));
    }

    #[test]
    fn file_search_reads_names_and_text_without_following_symlinks() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("nested")).unwrap();
        std::fs::write(root.path().join("nested/说明.md"), "OpenLife 项目摘要").unwrap();
        std::fs::write(root.path().join("matching-name.txt"), "unrelated").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("nested/说明.md", root.path().join("linked.md")).unwrap();

        let content = bounded_file_search(root.path(), "项目", 20);
        assert!(content.success);
        let content_value: serde_json::Value =
            serde_json::from_str(&content.output.unwrap()).unwrap();
        assert_eq!(content_value["results"].as_array().unwrap().len(), 1);
        assert_eq!(content_value["results"][0]["path"], "nested/说明.md");
        assert_eq!(content_value["results"][0]["matchedContent"], true);

        let name = bounded_file_search(root.path(), "matching-name", 20);
        let name_value: serde_json::Value = serde_json::from_str(&name.output.unwrap()).unwrap();
        assert_eq!(name_value["results"].as_array().unwrap().len(), 1);
        assert_eq!(name_value["results"][0]["matchedName"], true);

        let bounded = bounded_file_search(root.path(), "项目", 1);
        let bounded_value: serde_json::Value =
            serde_json::from_str(&bounded.output.unwrap()).unwrap();
        assert_eq!(bounded_value["results"].as_array().unwrap().len(), 1);
        assert_eq!(bounded_value["truncated"], true);
    }
}
