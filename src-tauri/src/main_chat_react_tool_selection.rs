use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;

const MAIN_CHAT_CONTRACT_SAFE_LABEL_MAX_LEN: usize = 96;
const MAIN_CHAT_CANDIDATE_RANKING_TOOLS_PROMPT: &str =
    "Main Chat metadata-safe candidate ranking; no tool execution.";
const MAIN_CHAT_WRITE_LIKE_CONTAINS_TERMS: &[&str] = &[
    "write",
    "send",
    "delete",
    "remove",
    "update",
    "create",
    "modify",
    "mutate",
    "externalwrite",
    "externalsideeffect",
    "realwrite",
    "emailsend",
    "calendarsend",
    "calendarwrite",
    "providerwrite",
    "shellexec",
    "execute",
];

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

impl MainChatReactToolCandidate {
    pub(crate) fn capabilities_digest_label(&self) -> String {
        let capabilities_digest = openlife_core::agent::react_beta::metadata_safe_value_digest(
            &serde_json::json!(self.capabilities),
        );
        format!(
            "bytes:{} hash:{}",
            capabilities_digest.0, capabilities_digest.1
        )
    }

    pub(crate) fn capability_labels_label(&self) -> String {
        let mut labels = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for capability in &self.capabilities {
            if !main_chat_contract_safe_label(capability, false)
                || main_chat_surface_contains_write_like_term(capability)
                || !seen.insert(capability.as_str())
            {
                continue;
            }
            let next_label = if labels.is_empty() {
                capability.clone()
            } else {
                format!("{}/{}", labels.join("/"), capability)
            };
            if next_label.len() > MAIN_CHAT_CONTRACT_SAFE_LABEL_MAX_LEN {
                break;
            }
            labels.push(capability.clone());
        }
        if labels.is_empty() {
            "none".into()
        } else {
            labels.join("/")
        }
    }

    pub(crate) fn manifest_source_label(&self) -> String {
        main_chat_contract_label_or(&self.manifest_source, true, "contract_unsafe_source")
    }

    pub(crate) fn match_reason_label(&self) -> String {
        main_chat_contract_label_or(&self.match_reason, false, "contract_unsafe")
    }
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

#[derive(Clone)]
pub(crate) struct MainChatReactToolSelectionRanking {
    pub(crate) model_ranked: bool,
    pub(crate) ranking_source: String,
    pub(crate) ranking_provider: Option<String>,
    pub(crate) ranking_model: Option<String>,
    pub(crate) ranking_route_type: Option<String>,
    pub(crate) provider_backed: bool,
    pub(crate) model_response_digest: Option<String>,
    pub(crate) ignored: bool,
    pub(crate) ranked_candidate_ids: Vec<String>,
}

impl MainChatReactToolSelectionRanking {
    pub(crate) fn deterministic() -> Self {
        Self {
            model_ranked: false,
            ranking_source: "deterministic_local".into(),
            ranking_provider: None,
            ranking_model: None,
            ranking_route_type: None,
            provider_backed: false,
            model_response_digest: None,
            ignored: false,
            ranked_candidate_ids: Vec::new(),
        }
    }

    fn deterministic_with_route(route: openlife_core::agent::ModelRouteTrace) -> Self {
        Self {
            model_ranked: false,
            ranking_source: "deterministic_local".into(),
            ranking_provider: Some(route.provider),
            ranking_model: Some(route.model),
            ranking_route_type: Some(route.route_type),
            provider_backed: false,
            model_response_digest: None,
            ignored: false,
            ranked_candidate_ids: Vec::new(),
        }
    }
}

impl MainChatReactActionPlan {
    fn governed_candidate_input(candidate: &MainChatReactToolCandidate) -> serde_json::Value {
        match candidate.executor_action_type.as_str() {
            "memory_search" | "session_search" => candidate.arguments.clone(),
            _ => serde_json::json!({ "arguments": candidate.arguments }),
        }
    }

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

    pub(crate) fn allowed_tool_actions(
        &self,
    ) -> Vec<openlife_core::agent::AgentLoopAllowedToolAction> {
        let mut actions = Vec::new();
        for candidate in self.tool_candidates() {
            if !actions.iter().any(
                |action: &openlife_core::agent::AgentLoopAllowedToolAction| {
                    action.action_type == candidate.executor_action_type
                        && action.target == candidate.target
                },
            ) {
                actions.push(openlife_core::agent::AgentLoopAllowedToolAction {
                    action_type: candidate.executor_action_type.clone(),
                    target: candidate.target.clone(),
                    input: Self::governed_candidate_input(&candidate),
                });
            }
        }
        actions
    }

    pub(crate) fn allowed_tool_action_metadata(&self) -> Vec<serde_json::Value> {
        self.allowed_tool_actions()
            .into_iter()
            .map(|action| {
                serde_json::json!({
                    "actionType": action.action_type,
                    "target": action.target,
                })
            })
            .collect()
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
                let capabilities_digest_label = candidate.capabilities_digest_label();
                let capability_labels = candidate.capability_labels_label();
                let candidate_id = main_chat_contract_label_or(
                    &candidate.candidate_id,
                    false,
                    "contract_unsafe_candidate",
                );
                let candidate_action_type = main_chat_contract_label_or(
                    &candidate.executor_action_type,
                    false,
                    "contract_unsafe_action",
                );
                let candidate_target =
                    main_chat_contract_label_or(&candidate.target, false, "contract_unsafe_target");
                let candidate_source = candidate.manifest_source_label();
                let match_reason = candidate.match_reason_label();
                format!(
                    concat!(
                        "{{candidateId={}; candidateActionType={}; ",
                        "candidateTarget={}; candidateRank={}; candidateSource={}; ",
                        "argumentDigest={}; capabilitiesDigest={}; capabilityLabels={}; ",
                        "matchReason={}; ",
                        "risk=read_only; allowWrites=false}}"
                    ),
                    candidate_id,
                    candidate_action_type,
                    candidate_target,
                    candidate.selection_rank,
                    candidate_source,
                    arguments_digest_label,
                    capabilities_digest_label,
                    capability_labels,
                    match_reason
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

pub(crate) async fn rank_main_chat_react_tool_candidates_with_model(
    scheduler: &InferenceScheduler,
    _life_model: &LifeModel,
    messages_for_generation: &[ChatMessage],
    plan: MainChatReactActionPlan,
    allow_cloud: bool,
) -> (MainChatReactActionPlan, MainChatReactToolSelectionRanking) {
    if !allow_cloud
        || plan.tool_candidate_count() < 2
        || !main_chat_provider_rankable_candidate_contract(&plan)
        || scheduler.scripted_generation_response.is_some()
        || scheduler.effective_api_key().trim().is_empty()
    {
        return (plan, MainChatReactToolSelectionRanking::deterministic());
    }

    let ranking_route = scheduler
        .preview_chat_route(Some(MAIN_CHAT_CANDIDATE_RANKING_TOOLS_PROMPT))
        .await;
    let ranking_provider_backed = ranking_route.route_type == "cloud"
        && main_chat_contract_safe_label(&ranking_route.provider, false)
        && main_chat_contract_safe_label(&ranking_route.model, false)
        && main_chat_provider_ranked_route_provider_allowed(&ranking_route.provider);
    if !ranking_provider_backed {
        return (
            plan,
            MainChatReactToolSelectionRanking::deterministic_with_route(ranking_route),
        );
    }
    if !main_chat_ranking_route_matches_scheduler(&ranking_route, scheduler) {
        return (
            plan,
            MainChatReactToolSelectionRanking::deterministic_with_route(ranking_route),
        );
    }

    let ranking_messages =
        build_main_chat_candidate_ranking_messages(messages_for_generation, &plan);
    let ranking_response = openlife_core::llm::chat_with_openrouter_raw(
        ranking_messages,
        None,
        &scheduler.provider,
        &scheduler.openai_base,
        &scheduler.openai_key,
        &scheduler.chat_model,
    )
    .await;
    let Ok(response) = ranking_response else {
        return (
            plan,
            MainChatReactToolSelectionRanking {
                model_ranked: false,
                ranking_source: "deterministic_local".into(),
                ranking_provider: Some(ranking_route.provider),
                ranking_model: Some(ranking_route.model),
                ranking_route_type: Some(ranking_route.route_type),
                provider_backed: ranking_provider_backed,
                model_response_digest: None,
                ignored: false,
                ranked_candidate_ids: Vec::new(),
            },
        );
    };

    let response_digest =
        openlife_core::agent::react_beta::metadata_safe_value_digest(&serde_json::json!({
            "response": response,
        }));
    let response_digest_label = format!("bytes:{} hash:{}", response_digest.0, response_digest.1);
    let Some(ranked_candidate_ids) = parse_main_chat_model_ranked_candidate_ids(&response) else {
        return (
            plan,
            MainChatReactToolSelectionRanking {
                model_ranked: false,
                ranking_source: "deterministic_local".into(),
                ranking_provider: Some(ranking_route.provider),
                ranking_model: Some(ranking_route.model),
                ranking_route_type: Some(ranking_route.route_type),
                provider_backed: ranking_provider_backed,
                model_response_digest: Some(response_digest_label),
                ignored: true,
                ranked_candidate_ids: Vec::new(),
            },
        );
    };
    let Some(ranked_plan) = apply_model_ranked_candidate_ids(&plan, &ranked_candidate_ids) else {
        return (
            plan,
            MainChatReactToolSelectionRanking {
                model_ranked: false,
                ranking_source: "deterministic_local".into(),
                ranking_provider: Some(ranking_route.provider),
                ranking_model: Some(ranking_route.model),
                ranking_route_type: Some(ranking_route.route_type),
                provider_backed: ranking_provider_backed,
                model_response_digest: Some(response_digest_label),
                ignored: true,
                ranked_candidate_ids: Vec::new(),
            },
        );
    };

    (
        ranked_plan,
        MainChatReactToolSelectionRanking {
            model_ranked: true,
            ranking_source: "provider_model".into(),
            ranking_provider: Some(ranking_route.provider),
            ranking_model: Some(ranking_route.model),
            ranking_route_type: Some(ranking_route.route_type),
            provider_backed: ranking_provider_backed,
            model_response_digest: Some(response_digest_label),
            ignored: false,
            ranked_candidate_ids,
        },
    )
}

fn main_chat_ranking_route_matches_scheduler(
    route: &openlife_core::agent::ModelRouteTrace,
    scheduler: &InferenceScheduler,
) -> bool {
    route.provider == scheduler.provider && route.model == scheduler.chat_model
}

fn main_chat_provider_ranked_route_provider_allowed(provider: &str) -> bool {
    if !main_chat_contract_safe_label(provider, false) {
        return false;
    }
    let provider = provider.to_ascii_lowercase();
    if matches!(
        provider.as_str(),
        "" | "none"
            | "ollama"
            | "local"
            | "localhost"
            | "127.0.0.1"
            | "::1"
            | "0.0.0.0"
            | "local_test_http"
            | "local-test-http"
            | "local_http"
            | "local-http"
            | "mock"
            | "fixture"
            | "synthetic"
            | "scripted"
    ) {
        return false;
    }
    if main_chat_provider_ranked_route_provider_is_local_network_alias(&provider) {
        return false;
    }
    let has_synthetic_token = provider
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| {
            matches!(
                token,
                "local" | "localhost" | "mock" | "fixture" | "synthetic" | "scripted"
            )
        });
    if has_synthetic_token {
        return false;
    }
    [
        "ollama",
        "local",
        "localhost",
        "mock",
        "fixture",
        "synthetic",
        "scripted",
    ]
    .iter()
    .all(|alias| !provider.contains(alias))
}

fn main_chat_provider_ranked_route_provider_is_local_network_alias(provider: &str) -> bool {
    let normalized = provider
        .chars()
        .map(|ch| {
            if matches!(ch, '-' | '_' | '/') {
                '.'
            } else {
                ch
            }
        })
        .collect::<String>();
    let parts = normalized.split('.').collect::<Vec<_>>();
    if parts.len() < 4 {
        return false;
    }
    parts.windows(4).any(|octets| {
        if octets
            .iter()
            .any(|octet| octet.is_empty() || !octet.chars().all(|ch| ch.is_ascii_digit()))
        {
            return false;
        }
        let Some(first) = octets.first().and_then(|octet| octet.parse::<u8>().ok()) else {
            return false;
        };
        let Some(second) = octets.get(1).and_then(|octet| octet.parse::<u8>().ok()) else {
            return false;
        };

        first == 0
            || first == 10
            || first == 127
            || (first == 169 && second == 254)
            || (first == 172 && (16..=31).contains(&second))
            || (first == 192 && second == 168)
    }) || main_chat_provider_ranked_route_provider_has_embedded_local_network_alias(provider)
}

fn main_chat_provider_ranked_route_provider_has_embedded_local_network_alias(
    provider: &str,
) -> bool {
    let mut octets = Vec::new();
    let mut current = String::new();
    for ch in provider.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(octet) = current.parse::<u16>() {
                octets.push(octet);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(octet) = current.parse::<u16>() {
            octets.push(octet);
        }
    }

    octets.windows(4).any(|window| {
        if window.iter().any(|octet| *octet > 255) {
            return false;
        }
        let first = window[0];
        let second = window[1];

        first == 0
            || first == 10
            || first == 127
            || (first == 169 && second == 254)
            || (first == 172 && (16..=31).contains(&second))
            || (first == 192 && second == 168)
    })
}

fn main_chat_provider_rankable_candidate_contract(plan: &MainChatReactActionPlan) -> bool {
    let candidates = plan.tool_candidates();
    let mut candidate_ids = std::collections::BTreeSet::new();
    for candidate in &candidates {
        if !main_chat_contract_safe_label(&candidate.candidate_id, false)
            || !main_chat_contract_safe_label(&candidate.executor_action_type, false)
            || !main_chat_contract_safe_label(&candidate.target, false)
            || !main_chat_contract_safe_label(&candidate.manifest_source, true)
            || !main_chat_contract_safe_label(&candidate.match_reason, false)
            || !candidate_ids.insert(candidate.candidate_id.as_str())
        {
            return false;
        }
    }
    true
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

fn build_main_chat_candidate_ranking_messages(
    messages_for_generation: &[ChatMessage],
    plan: &MainChatReactActionPlan,
) -> Vec<ChatMessage> {
    let bounded_context = messages_for_generation
        .iter()
        .rev()
        .take(4)
        .map(|message| {
            format!(
                "{}: {}",
                main_chat_contract_label_or(&message.role, false, "role"),
                bounded_main_chat_ranking_text(&message.content, 1200)
            )
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    vec![
        ChatMessage {
            role: "system".into(),
            content: concat!(
                "Rank OpenLife Main Chat governed read-only tool candidates for relevance. ",
                "Use only the metadata-safe candidate contract. Do not invent candidate IDs, ",
                "do not include tool arguments, and do not request writes. Return only JSON ",
                "shaped as {\"ranked_candidate_ids\":[\"candidateId\",...]}.",
            )
            .into(),
        },
        ChatMessage {
            role: "user".into(),
            content: format!(
                "Sanitized conversation context:\n{}\n\nMetadata-safe candidate contract:\n{}\n\nReturn ranked_candidate_ids now.",
                bounded_context,
                plan.tool_candidate_contract()
            ),
        },
    ]
}

fn bounded_main_chat_ranking_text(value: &str, max_chars: usize) -> String {
    let normalized = value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    let (masked, _) = openlife_core::privacy::PrivacyEngine::new().desensitize(&normalized);
    masked.chars().take(max_chars).collect()
}

fn parse_main_chat_model_ranked_candidate_ids(response: &str) -> Option<Vec<String>> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(response) else {
        return None;
    };
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let ids = object
        .get("ranked_candidate_ids")
        .and_then(serde_json::Value::as_array)?;
    let mut candidate_ids = Vec::new();
    for id in ids {
        let candidate_id = id.as_str()?;
        if !main_chat_contract_safe_label(candidate_id, false) {
            return None;
        }
        candidate_ids.push(candidate_id.to_string());
    }
    Some(candidate_ids)
}

fn apply_model_ranked_candidate_ids(
    plan: &MainChatReactActionPlan,
    ranked_candidate_ids: &[String],
) -> Option<MainChatReactActionPlan> {
    if ranked_candidate_ids.is_empty() {
        return None;
    }
    let original_candidates = plan.tool_candidates();
    let original_candidate_ids = original_candidates
        .iter()
        .map(|candidate| candidate.candidate_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let ranked_candidate_id_set = ranked_candidate_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    if ranked_candidate_ids.len() != original_candidates.len()
        || original_candidate_ids.len() != original_candidates.len()
        || ranked_candidate_id_set.len() != ranked_candidate_ids.len()
        || ranked_candidate_ids.iter().any(|candidate_id| {
            !original_candidates
                .iter()
                .any(|candidate| &candidate.candidate_id == candidate_id)
        })
        || original_candidates.iter().any(|candidate| {
            !ranked_candidate_ids
                .iter()
                .any(|candidate_id| candidate_id == &candidate.candidate_id)
        })
    {
        return None;
    }
    let mut ranked_candidates = Vec::new();
    for candidate_id in ranked_candidate_ids {
        if let Some(candidate) = original_candidates
            .iter()
            .find(|candidate| &candidate.candidate_id == candidate_id)
        {
            if !ranked_candidates
                .iter()
                .any(|existing: &MainChatReactToolCandidate| {
                    existing.candidate_id == candidate.candidate_id
                })
            {
                let mut candidate = candidate.clone();
                candidate.match_reason = "provider_model_ranked".into();
                ranked_candidates.push(candidate);
            }
        }
    }
    if ranked_candidates.is_empty() {
        return None;
    }
    for candidate in original_candidates {
        if !ranked_candidates
            .iter()
            .any(|existing| existing.candidate_id == candidate.candidate_id)
        {
            ranked_candidates.push(candidate);
        }
    }
    for (index, candidate) in ranked_candidates.iter_mut().enumerate() {
        candidate.selection_rank = index + 1;
    }
    let mut ranked_plan = plan.clone();
    if let Some(primary) = ranked_candidates.first() {
        ranked_plan.target = primary.target.clone();
        ranked_plan.arguments = primary.arguments.clone();
    }
    ranked_plan.tool_candidates = ranked_candidates;
    Some(ranked_plan)
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

    if !main_chat_manifest_is_explicit_read_target_candidate(&manifest) {
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
    let mut seen_targets = std::collections::HashSet::new();
    manifests
        .into_iter()
        .filter(|manifest| seen_targets.insert(manifest.name.clone()))
        .take(limit)
        .enumerate()
        .map(|(index, manifest)| {
            let selection_score = main_chat_manifest_selection_score(&manifest, &selection_terms);
            let capabilities = main_chat_manifest_read_candidate_capabilities(&manifest);
            MainChatReactToolCandidate {
                candidate_id: manifest.name.clone(),
                executor_action_type: "mcp_tool".into(),
                target: manifest.name,
                arguments: serde_json::json!({}),
                manifest_source: manifest.source.to_string(),
                capabilities,
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

fn main_chat_manifest_read_candidate_capabilities(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> Vec<String> {
    let mut capabilities = manifest.capabilities.clone();
    if manifest.action_type.eq_ignore_ascii_case("read")
        && !capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("read"))
    {
        capabilities.insert(0, "read".into());
    }
    capabilities
}

fn main_chat_manifest_selection_score(
    manifest: &openlife_core::tool_manifest::ToolManifest,
    selection_terms: &[String],
) -> usize {
    if selection_terms.is_empty() {
        return 0;
    }
    let searchable = std::iter::once(manifest.name.as_str())
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
    if !main_chat_manifest_has_contract_safe_name(manifest)
        || !main_chat_manifest_has_contract_safe_source(manifest)
    {
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
        || main_chat_manifest_has_write_like_surface(manifest)
    {
        return false;
    }
    manifest.action_type.eq_ignore_ascii_case("read")
        || manifest
            .capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("read"))
}

fn main_chat_manifest_is_explicit_read_target_candidate(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> bool {
    if manifest.name == "mcp.call_tool" || !manifest.enabled || manifest.declarative_only {
        return false;
    }
    if !main_chat_manifest_has_contract_safe_name(manifest)
        || !main_chat_manifest_has_contract_safe_source(manifest)
    {
        return false;
    }
    if matches!(
        manifest.risk_level.to_ascii_lowercase().as_str(),
        "high" | "critical"
    ) || matches!(
        manifest.permission_level.to_ascii_lowercase().as_str(),
        "high" | "critical"
    ) || matches!(
        manifest.action_type.to_ascii_lowercase().as_str(),
        "write" | "external_side_effect"
    ) || manifest.capabilities.iter().any(|capability| {
        matches!(
            capability.to_ascii_lowercase().as_str(),
            "write" | "external_side_effect"
        )
    }) || main_chat_manifest_has_write_like_surface(manifest)
    {
        return false;
    }
    manifest.action_type.eq_ignore_ascii_case("read")
        || manifest
            .capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("read"))
}

fn main_chat_manifest_has_contract_safe_name(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> bool {
    main_chat_contract_safe_label(&manifest.name, false)
}

fn main_chat_manifest_has_contract_safe_source(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> bool {
    main_chat_contract_safe_label(&manifest.source.to_string(), true)
}

fn main_chat_contract_safe_label(value: &str, allow_colon: bool) -> bool {
    !value.is_empty()
        && value.len() <= MAIN_CHAT_CONTRACT_SAFE_LABEL_MAX_LEN
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(ch, '.' | '_' | '-' | '/')
                || (allow_colon && ch == ':')
        })
}

fn main_chat_contract_label_or(value: &str, allow_colon: bool, fallback: &str) -> String {
    if main_chat_contract_safe_label(value, allow_colon) {
        value.to_string()
    } else {
        fallback.to_string()
    }
}

fn main_chat_manifest_has_write_like_surface(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> bool {
    std::iter::once(manifest.id.as_str())
        .chain(std::iter::once(manifest.name.as_str()))
        .chain(std::iter::once(manifest.action_type.as_str()))
        .chain(manifest.capabilities.iter().map(String::as_str))
        .chain(manifest.tags.iter().map(String::as_str))
        .any(main_chat_surface_contains_write_like_term)
}

fn main_chat_surface_contains_write_like_term(value: &str) -> bool {
    let surface = normalize_main_chat_selection_text(value);
    matches!(
        surface.as_str(),
        "write"
            | "send"
            | "delete"
            | "remove"
            | "update"
            | "create"
            | "modify"
            | "mutate"
            | "externalwrite"
            | "externalsideeffect"
            | "realwrite"
            | "emailsend"
            | "calendarsend"
            | "calendarwrite"
            | "providerwrite"
            | "shellexec"
            | "execute"
            | "exec"
    ) || MAIN_CHAT_WRITE_LIKE_CONTAINS_TERMS
        .iter()
        .any(|term| surface.contains(term))
        || surface.ends_with("write")
        || surface.ends_with("send")
        || surface.ends_with("delete")
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
