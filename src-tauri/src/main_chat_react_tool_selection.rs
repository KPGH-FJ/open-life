use openlife_core::llm::ChatMessage;

#[derive(Clone)]
pub(crate) struct MainChatReactToolCandidate {
    pub(crate) candidate_id: String,
    pub(crate) executor_action_type: String,
    pub(crate) target: String,
    pub(crate) arguments: serde_json::Value,
    pub(crate) manifest_source: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) selection_rank: usize,
    pub(crate) match_reason: String,
}

#[derive(Clone)]
pub(crate) struct MainChatReactActionPlan {
    pub(crate) queue_action_type: String,
    pub(crate) executor_action_type: String,
    pub(crate) target: String,
    pub(crate) arguments: serde_json::Value,
    pub(crate) description: String,
    pub(crate) requires_network: bool,
    pub(crate) uses_ephemeral_file_permission: bool,
    pub(crate) uses_ephemeral_mcp_wrapper_permission: bool,
    pub(crate) tool_candidates: Vec<MainChatReactToolCandidate>,
}

impl MainChatReactActionPlan {
    pub(crate) fn default_tool_candidate(&self) -> MainChatReactToolCandidate {
        MainChatReactToolCandidate {
            candidate_id: self.queue_action_type.clone(),
            executor_action_type: self.executor_action_type.clone(),
            target: self.target.clone(),
            arguments: self.arguments.clone(),
            manifest_source: "planned_action".into(),
            capabilities: vec!["read".into()],
            selection_rank: 1,
            match_reason: "planned_action".into(),
        }
    }

    pub(crate) fn tool_candidates(&self) -> Vec<MainChatReactToolCandidate> {
        if self.tool_candidates.is_empty() {
            vec![self.default_tool_candidate()]
        } else {
            self.tool_candidates.clone()
        }
    }

    pub(crate) fn tool_candidate_count(&self) -> usize {
        self.tool_candidates().len()
    }

    pub(crate) fn tool_candidate_ids(&self) -> Vec<String> {
        self.tool_candidates()
            .into_iter()
            .map(|candidate| candidate.candidate_id)
            .collect()
    }

    pub(crate) fn allowed_tool_targets(&self) -> Vec<String> {
        let mut targets = Vec::new();
        for candidate in self.tool_candidates() {
            if !targets.iter().any(|target| target == &candidate.target) {
                targets.push(candidate.target);
            }
        }
        targets
    }

    pub(crate) fn tool_candidate_for_action(
        &self,
        action_type: &str,
        target: Option<&str>,
    ) -> Option<MainChatReactToolCandidate> {
        let target = target?;
        self.tool_candidates().into_iter().find(|candidate| {
            candidate.executor_action_type == action_type && candidate.target == target
        })
    }

    pub(crate) fn tool_candidate_contract(&self) -> String {
        let candidates = self.tool_candidates();
        let candidate_count = candidates.len();
        let candidate_contract = candidates
            .into_iter()
            .map(|candidate| {
                let arguments_digest = openlife_core::agent::react_beta::metadata_safe_value_digest(
                    &candidate.arguments,
                );
                let arguments_digest_label =
                    format!("bytes:{} hash:{}", arguments_digest.0, arguments_digest.1);
                let capabilities_digest =
                    openlife_core::agent::react_beta::metadata_safe_value_digest(
                        &serde_json::json!(candidate.capabilities),
                    );
                let capabilities_digest_label = format!(
                    "bytes:{} hash:{}",
                    capabilities_digest.0, capabilities_digest.1
                );
                format!(
                    concat!(
                        "{{candidateId={}; candidateActionType={}; ",
                        "candidateTarget={}; candidateRank={}; candidateSource={}; ",
                        "argumentDigest={}; capabilitiesDigest={}; matchReason={}; ",
                        "risk=read_only; allowWrites=false}}"
                    ),
                    candidate.candidate_id,
                    candidate.executor_action_type,
                    candidate.target,
                    candidate.selection_rank,
                    candidate.manifest_source,
                    arguments_digest_label,
                    capabilities_digest_label,
                    candidate.match_reason
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            concat!(
                "allowedToolCandidates=[{}]; ",
                "candidateCount={}; toolsetAllowlistRequired=true"
            ),
            candidate_contract, candidate_count
        )
    }
}

pub(crate) struct MainChatMcpReadResolution {
    pub(crate) target: String,
    pub(crate) arguments: serde_json::Value,
    pub(crate) resolved: bool,
    pub(crate) blocker_reason: Option<String>,
}

pub(crate) fn build_main_chat_react_action_plan(
    session_id: &str,
    user_text: &str,
) -> Result<MainChatReactActionPlan, String> {
    let lower = user_text.to_ascii_lowercase();
    if lower.contains("mcp") {
        let tool_name = infer_main_chat_mcp_tool_name(user_text).unwrap_or_default();
        return Ok(MainChatReactActionPlan {
            queue_action_type: "mcp.read_only".into(),
            executor_action_type: "mcp_tool".into(),
            target: "mcp.call_tool".into(),
            arguments: serde_json::json!({
                "tool_name": tool_name,
                "arguments": {},
                "selection_query": main_chat_search_query(user_text),
            }),
            description: "Call a registered MCP read tool through ActionExecutor.".into(),
            requires_network: false,
            uses_ephemeral_file_permission: false,
            uses_ephemeral_mcp_wrapper_permission: true,
            tool_candidates: Vec::new(),
        });
    }

    if lower.contains("agents.md") || lower.contains("read ") || lower.contains("file") {
        let (path_label, path) = main_chat_workspace_file_target(user_text)?;
        return Ok(MainChatReactActionPlan {
            queue_action_type: "file.read".into(),
            executor_action_type: "mcp_tool".into(),
            target: "file.read".into(),
            arguments: serde_json::json!({ "path": path }),
            description: format!("Read workspace file {path_label} through ActionExecutor."),
            requires_network: false,
            uses_ephemeral_file_permission: true,
            uses_ephemeral_mcp_wrapper_permission: false,
            tool_candidates: Vec::new(),
        });
    }

    if lower.contains("web")
        || lower.contains("fetch")
        || lower.contains("http://")
        || lower.contains("https://")
    {
        if lower.contains("fetch") || lower.contains("http://") || lower.contains("https://") {
            if let Some(url) = extract_main_chat_url(user_text) {
                return Ok(MainChatReactActionPlan {
                    queue_action_type: "web.fetch".into(),
                    executor_action_type: "mcp_tool".into(),
                    target: "web.fetch".into(),
                    arguments: serde_json::json!({ "url": url, "summarize": true }),
                    description: "Fetch a URL through governed ActionExecutor network policy."
                        .into(),
                    requires_network: true,
                    uses_ephemeral_file_permission: false,
                    uses_ephemeral_mcp_wrapper_permission: false,
                    tool_candidates: Vec::new(),
                });
            }
        }
        return Ok(MainChatReactActionPlan {
            queue_action_type: "web.search".into(),
            executor_action_type: "mcp_tool".into(),
            target: "web.search".into(),
            arguments: serde_json::json!({
                "query": main_chat_search_query(user_text),
                "max_results": 5,
            }),
            description: "Search the web through governed ActionExecutor network policy.".into(),
            requires_network: true,
            uses_ephemeral_file_permission: false,
            uses_ephemeral_mcp_wrapper_permission: false,
            tool_candidates: Vec::new(),
        });
    }

    if lower.contains("yesterday")
        || lower.contains("past sessions")
        || lower.contains("what did i ask")
    {
        return Ok(MainChatReactActionPlan {
            queue_action_type: "session.search".into(),
            executor_action_type: "session_search".into(),
            target: "session.search".into(),
            arguments: serde_json::json!({
                "query": main_chat_search_query(user_text),
                "limit": 5,
            }),
            description: "Search prior chat/session memory through ActionExecutor.".into(),
            requires_network: false,
            uses_ephemeral_file_permission: false,
            uses_ephemeral_mcp_wrapper_permission: false,
            tool_candidates: Vec::new(),
        });
    }

    Ok(MainChatReactActionPlan {
        queue_action_type: "memory.search".into(),
        executor_action_type: "memory_search".into(),
        target: "memory.search".into(),
        arguments: serde_json::json!({
            "query": main_chat_search_query(user_text),
            "session_id": session_id,
            "limit": 5,
        }),
        description: "Search current session memory through ActionExecutor.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: false,
        tool_candidates: Vec::new(),
    })
}

pub(crate) fn build_main_chat_react_agent_loop_messages(
    messages_for_generation: &[ChatMessage],
    plan: &MainChatReactActionPlan,
) -> Vec<ChatMessage> {
    let arguments_digest =
        openlife_core::agent::react_beta::metadata_safe_value_digest(&plan.arguments);
    let arguments_digest_label =
        format!("bytes:{} hash:{}", arguments_digest.0, arguments_digest.1);
    let mut guided_messages = Vec::with_capacity(messages_for_generation.len() + 1);
    guided_messages.push(ChatMessage {
        role: "system".into(),
        content: format!(
            concat!(
                "Main Chat Agent v1 selected a governed read-only ReAct action for this turn.\n",
                "Use at most one allowed tool candidate. If the action is unnecessary, answer directly.\n",
                "When using the action, return only a JSON envelope shaped as ",
                "{{\"final\":\"...\",\"actions\":[{{\"name\":\"<allowedTarget>\",",
                "\"action_type\":\"<plannedExecutorActionType>\",\"arguments\":{{}}}}],",
                "\"thought_summary\":\"...\",\"warnings\":[]}}.\n",
                "Do not call any tool outside the allowed candidate set and do not execute durable writes.\n",
                "plannedActionType={}; plannedExecutorActionType={}; plannedTarget={}; ",
                "argumentsDigest={}; {}; allowWrites=false; directWritesAllowed=false; ",
                "durableWritesAllowed=false; externalWritesAllowed=false."
            ),
            plan.queue_action_type,
            plan.executor_action_type,
            plan.target,
            arguments_digest_label,
            plan.tool_candidate_contract()
        ),
    });
    guided_messages.extend_from_slice(messages_for_generation);
    guided_messages
}

pub(crate) fn main_chat_react_agent_loop_execution_plan(
    registry: &openlife_core::mcp::McpRegistry,
    plan: &MainChatReactActionPlan,
) -> MainChatReactActionPlan {
    if plan.target != "mcp.call_tool" {
        return plan.clone();
    }

    let resolution = resolve_main_chat_mcp_read_target(registry, plan);
    if !resolution.resolved {
        if resolution.blocker_reason.as_deref() == Some("mcp_read_tool_name_missing") {
            let selection_query = plan
                .arguments
                .get("selection_query")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let candidates =
                main_chat_governed_mcp_read_tool_candidates(registry, selection_query, 16);
            if !candidates.is_empty() {
                let mut resolved = plan.clone();
                if let Some(primary) = candidates.first() {
                    resolved.target = primary.target.clone();
                    resolved.arguments = primary.arguments.clone();
                }
                resolved.description =
                    "Select one registered governed read target through AgentLoop.".into();
                resolved.tool_candidates = candidates;
                return resolved;
            }
        }
        return plan.clone();
    }

    let mut resolved = plan.clone();
    resolved.target = resolution.target;
    resolved.arguments = resolution.arguments;
    resolved.description = format!(
        "Call registered MCP read target {} through governed AgentLoop.",
        resolved.target
    );
    resolved.tool_candidates = vec![MainChatReactToolCandidate {
        candidate_id: resolved.target.clone(),
        executor_action_type: resolved.executor_action_type.clone(),
        target: resolved.target.clone(),
        arguments: resolved.arguments.clone(),
        manifest_source: "registered_manifest".into(),
        capabilities: vec!["read".into()],
        selection_rank: 1,
        match_reason: "explicit_tool_name".into(),
    }];
    resolved
}

pub(crate) fn resolve_main_chat_mcp_read_target(
    registry: &openlife_core::mcp::McpRegistry,
    plan: &MainChatReactActionPlan,
) -> MainChatMcpReadResolution {
    if plan.target != "mcp.call_tool" {
        return MainChatMcpReadResolution {
            target: plan.target.clone(),
            arguments: plan.arguments.clone(),
            resolved: false,
            blocker_reason: None,
        };
    }

    let tool_name = plan
        .arguments
        .get("tool_name")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();
    if tool_name.is_empty() {
        return MainChatMcpReadResolution {
            target: plan.target.clone(),
            arguments: plan.arguments.clone(),
            resolved: false,
            blocker_reason: Some("mcp_read_tool_name_missing".into()),
        };
    }

    let manifest = registry
        .list_manifests()
        .into_iter()
        .find(|manifest| manifest.name == tool_name || manifest.id == tool_name);
    let Some(manifest) = manifest else {
        return MainChatMcpReadResolution {
            target: tool_name.into(),
            arguments: plan.arguments.clone(),
            resolved: false,
            blocker_reason: Some("mcp_read_tool_not_registered".into()),
        };
    };

    let read_only = manifest.action_type.eq_ignore_ascii_case("read")
        || manifest
            .capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("read"));
    if !manifest.enabled || manifest.declarative_only || !read_only {
        return MainChatMcpReadResolution {
            target: manifest.name,
            arguments: plan.arguments.clone(),
            resolved: false,
            blocker_reason: Some("mcp_read_tool_not_governed_read_only".into()),
        };
    }

    MainChatMcpReadResolution {
        target: manifest.name,
        arguments: plan
            .arguments
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
        resolved: true,
        blocker_reason: None,
    }
}

fn main_chat_governed_mcp_read_tool_candidates(
    registry: &openlife_core::mcp::McpRegistry,
    selection_query: &str,
    limit: usize,
) -> Vec<MainChatReactToolCandidate> {
    let mut manifests = registry
        .list_manifests()
        .into_iter()
        .filter(main_chat_manifest_is_governed_read_candidate)
        .collect::<Vec<_>>();
    let selection_terms = main_chat_selection_terms(selection_query);
    manifests.sort_by(|left, right| {
        let left_score = main_chat_manifest_selection_score(left, &selection_terms);
        let right_score = main_chat_manifest_selection_score(right, &selection_terms);
        right_score
            .cmp(&left_score)
            .then_with(|| left.name.cmp(&right.name))
    });
    manifests
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(index, manifest)| {
            let selection_score = main_chat_manifest_selection_score(&manifest, &selection_terms);
            MainChatReactToolCandidate {
                candidate_id: manifest.name.clone(),
                executor_action_type: "mcp_tool".into(),
                target: manifest.name,
                arguments: serde_json::json!({}),
                manifest_source: manifest.source.to_string(),
                capabilities: manifest.capabilities,
                selection_rank: index + 1,
                match_reason: if selection_score > 0 {
                    "capability_or_name_match".into()
                } else {
                    "manifest_default_order".into()
                },
            }
        })
        .collect()
}

fn main_chat_manifest_selection_score(
    manifest: &openlife_core::tool_manifest::ToolManifest,
    selection_terms: &[String],
) -> usize {
    if selection_terms.is_empty() {
        return 0;
    }
    let searchable = std::iter::once(manifest.name.as_str())
        .chain(std::iter::once(manifest.id.as_str()))
        .chain(std::iter::once(manifest.description.as_str()))
        .chain(manifest.capabilities.iter().map(String::as_str))
        .chain(manifest.tags.iter().map(String::as_str))
        .map(normalize_main_chat_selection_text)
        .collect::<Vec<_>>();
    selection_terms
        .iter()
        .filter(|term| searchable.iter().any(|value| value.contains(term.as_str())))
        .count()
}

fn main_chat_selection_terms(selection_query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for raw_token in selection_query.split_whitespace() {
        let term = normalize_main_chat_selection_text(raw_token);
        if term.len() < 3 || main_chat_generic_selection_term(&term) {
            continue;
        }
        if !terms.iter().any(|existing| existing == &term) {
            terms.push(term);
        }
    }
    terms
}

fn normalize_main_chat_selection_text(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn main_chat_generic_selection_term(term: &str) -> bool {
    matches!(
        term,
        "mcp"
            | "read"
            | "readonly"
            | "tool"
            | "tools"
            | "utility"
            | "use"
            | "now"
            | "please"
            | "with"
            | "for"
            | "and"
            | "the"
    )
}

fn main_chat_manifest_is_governed_read_candidate(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> bool {
    if manifest.name == "mcp.call_tool" || !manifest.enabled || manifest.declarative_only {
        return false;
    }
    if manifest.requires_confirmation
        || matches!(
            manifest.risk_level.to_ascii_lowercase().as_str(),
            "high" | "critical"
        )
        || matches!(
            manifest.permission_level.to_ascii_lowercase().as_str(),
            "high" | "critical"
        )
        || matches!(
            manifest.action_type.to_ascii_lowercase().as_str(),
            "write" | "external_side_effect"
        )
        || manifest.capabilities.iter().any(|capability| {
            matches!(
                capability.to_ascii_lowercase().as_str(),
                "write" | "external_side_effect"
            )
        })
    {
        return false;
    }
    manifest.action_type.eq_ignore_ascii_case("read")
        || manifest
            .capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("read"))
}

pub(crate) fn main_chat_workspace_file_target(user_text: &str) -> Result<(String, String), String> {
    crate::workspace_file_resolver::resolve_main_chat_workspace_file_target(user_text)
}

fn main_chat_search_query(user_text: &str) -> String {
    user_text.trim().to_string()
}

fn extract_main_chat_url(user_text: &str) -> Option<String> {
    user_text
        .split_whitespace()
        .map(trim_main_chat_tool_token)
        .find(|token| token.starts_with("http://") || token.starts_with("https://"))
        .map(str::to_string)
}

fn infer_main_chat_mcp_tool_name(user_text: &str) -> Option<String> {
    let tokens = user_text
        .split_whitespace()
        .map(trim_main_chat_tool_token)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mcp_index = tokens
        .iter()
        .position(|token| token.eq_ignore_ascii_case("mcp"))?;
    tokens
        .iter()
        .skip(mcp_index + 1)
        .copied()
        .find(|token| is_main_chat_specific_mcp_tool_token(token))
        .map(str::to_string)
}

fn is_main_chat_specific_mcp_tool_token(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "read" | "read-only" | "readonly" | "tool" | "tools" | "utility" | "now" | "please"
    ) {
        return false;
    }
    token.contains('.') || token.contains('_') || token.contains('-')
}

fn trim_main_chat_tool_token(token: &str) -> &str {
    let trimmed = token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '`' | ',' | ':' | ';' | ')' | '(' | '[' | ']' | '{' | '}'
        )
    });
    trimmed.strip_suffix('.').unwrap_or(trimmed)
}
