use openlife_core::config::{AppConfig, NetworkPolicy};
use openlife_core::tool_manifest::{ToolManifest, ToolSource};
use std::sync::Arc;

use super::contract::{
    matches_exact_runtime_fact_phrase, merge_json_object, trim_outer_punctuation,
    MainChatRuntimeFactAnswer, MainChatRuntimeFactBinding, MainChatToolAvailabilityIntent,
    RUNTIME_FACT_KEY_TOOL_MCP_REGISTERED_COUNT,
    RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT, RUNTIME_FACT_KEY_TOOL_MCP_SERVER_STATUS,
    RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE, RUNTIME_FACT_KEY_TOOL_WEB_CONFIG_ENABLED,
    RUNTIME_FACT_KEY_TOOL_WEB_CREDENTIAL_AVAILABLE, RUNTIME_FACT_KEY_TOOL_WEB_POLICY_ALLOWED,
    RUNTIME_FACT_KEY_TOOL_WEB_REACHABLE, RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE,
    RUNTIME_FACT_KEY_TOOL_WRITE_REQUIRES_PERMISSION,
    RUNTIME_FACT_TOOL_AVAILABILITY_GENERATION_PATH,
};
use crate::main_chat_react_tool_selection::main_chat_manifest_is_governed_read_candidate;
use crate::AppState;

#[derive(Debug, Clone)]
struct WebAvailabilityFactSnapshot {
    config_enabled: bool,
    credential_available: bool,
    credential_status: String,
    policy_allowed: bool,
    policy_blockers: Vec<String>,
    reachability_status: String,
    reachability_ttl_status: String,
    cached_or_preflight_known_reachability: bool,
    active_reachability_probe: bool,
    available_status: String,
}

#[derive(Debug, Clone)]
struct McpAvailabilityFactSnapshot {
    registered_count: usize,
    safe_read_candidate_count: usize,
    server_status: String,
    available_status: String,
    raw_manifest_exposed: bool,
}

#[derive(Debug, Clone)]
struct WriteAvailabilityFactSnapshot {
    available_status: String,
    requires_permission: bool,
    silent_write_available: bool,
}

#[derive(Debug, Clone)]
struct ToolAvailabilityFactSnapshot {
    web: WebAvailabilityFactSnapshot,
    mcp: McpAvailabilityFactSnapshot,
    write: WriteAvailabilityFactSnapshot,
}

pub(crate) async fn resolve_tool_availability_fact_answer(
    user_text: &str,
    state: &Arc<AppState>,
) -> Option<MainChatRuntimeFactAnswer> {
    let intent = classify_tool_availability_query(user_text)?;
    let config = state.config.lock().await.clone();
    let manifests = {
        let registry = state.mcp_registry.lock().await;
        registry.list_cached_manifest_snapshots()
    };
    let snapshot = tool_availability_snapshot(&config, &manifests);
    let facts = tool_availability_fact_bindings(intent, &snapshot);
    let fact_keys = intent.fact_keys();
    let labels = tool_availability_labels(&snapshot);
    let reply = tool_availability_reply(intent, &snapshot);
    let ui_status = tool_availability_ui_status(intent, &snapshot);
    let mut extra_metadata = serde_json::json!({
        "providerGenerationPath": RUNTIME_FACT_TOOL_AVAILABILITY_GENERATION_PATH,
        "modelGenerated": false,
        "schedulerGenerationCalled": false,
        "toolCalled": false,
        "directWritesExecuted": false,
        "legacyFallbackUsed": false,
        "toolWebConfigEnabled": snapshot.web.config_enabled,
        "toolWebCredentialAvailable": snapshot.web.credential_available,
        "toolWebCredentialStatus": snapshot.web.credential_status,
        "toolWebPolicyAllowed": snapshot.web.policy_allowed,
        "toolWebPolicyBlockers": snapshot.web.policy_blockers,
        "toolWebReachabilityStatus": snapshot.web.reachability_status,
        "toolWebReachabilityTtlStatus": snapshot.web.reachability_ttl_status,
        "toolWebReachabilityTtlPolicy": "explicit",
        "toolWebReachabilityObservedAt": null,
        "toolWebCachedOrPreflightKnownReachability": snapshot.web.cached_or_preflight_known_reachability,
        "toolWebActiveReachabilityProbe": snapshot.web.active_reachability_probe,
        "toolWebAvailable": snapshot.web.available_status,
        "toolMcpRegisteredCount": snapshot.mcp.registered_count,
        "toolMcpSafeReadCandidateCount": snapshot.mcp.safe_read_candidate_count,
        "toolMcpServerStatus": snapshot.mcp.server_status,
        "toolMcpAvailable": snapshot.mcp.available_status,
        "toolMcpRawManifestExposed": snapshot.mcp.raw_manifest_exposed,
        "toolWriteAvailable": snapshot.write.available_status,
        "toolWriteRequiresPermission": snapshot.write.requires_permission,
        "toolWriteSilentWriteAvailable": snapshot.write.silent_write_available,
        "toolAvailabilityLabels": labels,
        "uiPrimarySourceChip": "工具可用性",
        "uiStatus": ui_status,
        "uiSecondaryChips": tool_availability_secondary_chips(intent, &snapshot),
        "runtimeFactTtl": "turn",
        "runtimeFactTtlStatus": "fresh",
        "runtimeFactMissingBehavior": if intent == MainChatToolAvailabilityIntent::AskWriteCapability {
            "blocker"
        } else {
            "answer_unknown"
        },
    });
    if intent == MainChatToolAvailabilityIntent::AskToolAvailability {
        merge_json_object(
            &mut extra_metadata,
            serde_json::json!({
                "runtimeFactObservation": {
                    "tool.web.reachable": {
                        "observedAt": null,
                        "ttlStatus": snapshot.web.reachability_ttl_status,
                        "ttlPolicy": "explicit"
                    },
                    "tool.mcp.server_status": {
                        "observedAt": null,
                        "ttlStatus": "not_observed",
                        "ttlPolicy": "explicit"
                    }
                }
            }),
        );
    }

    Some(MainChatRuntimeFactAnswer {
        reply,
        intent: intent.as_str().into(),
        fact_keys,
        facts,
        observed_at: Some(chrono::Utc::now().to_rfc3339()),
        source: match intent {
            MainChatToolAvailabilityIntent::AskToolAvailability => {
                vec!["config", "tool_policy", "tool_preflight", "tool_registry"]
            }
            MainChatToolAvailabilityIntent::AskWriteCapability => vec!["tool_policy"],
        },
        authority: "policy",
        freshness: "turn_snapshot",
        visibility: vec!["answer", "ui_badge", "trace_only"],
        privacy: vec!["public", "internal"],
        timezone: None,
        trace_gap: false,
        extra_metadata,
    })
}

fn tool_availability_snapshot(
    config: &AppConfig,
    manifests: &[ToolManifest],
) -> ToolAvailabilityFactSnapshot {
    let web_search_configured = manifests
        .iter()
        .any(|manifest| manifest.enabled && manifest.name == "web.search");
    let web_fetch_configured = manifests
        .iter()
        .any(|manifest| manifest.enabled && manifest.name == "web.fetch");
    let web_config_enabled = web_search_configured || web_fetch_configured;
    let (web_credential_available, web_credential_status) =
        web_credential_snapshot(config, web_search_configured, web_fetch_configured);
    let (web_policy_allowed, web_policy_blockers) =
        web_policy_snapshot(&config.system.network_policy);
    let reachability_status = "unknown".to_string();
    let cached_or_preflight_known_reachability = false;
    let available_status = if !web_config_enabled {
        "unconfigured".to_string()
    } else if !web_credential_available {
        "missing_credential".to_string()
    } else if !web_policy_allowed {
        "blocked".to_string()
    } else if cached_or_preflight_known_reachability {
        "available".to_string()
    } else {
        reachability_status.clone()
    };

    let mcp_manifests = manifests
        .iter()
        .filter(|manifest| matches!(manifest.source, ToolSource::Mcp { .. }))
        .collect::<Vec<_>>();
    let mcp_registered_count = mcp_manifests.len();
    let mcp_safe_read_candidate_count = mcp_manifests
        .iter()
        .filter(|manifest| main_chat_manifest_is_governed_read_candidate(manifest))
        .count();
    let mcp_server_status = "unknown".to_string();
    let mcp_available_status = if mcp_registered_count == 0 {
        "not_registered".to_string()
    } else if mcp_safe_read_candidate_count == 0 {
        "no_safe_read_candidate".to_string()
    } else {
        "unknown_server_status".to_string()
    };

    ToolAvailabilityFactSnapshot {
        web: WebAvailabilityFactSnapshot {
            config_enabled: web_config_enabled,
            credential_available: web_credential_available,
            credential_status: web_credential_status,
            policy_allowed: web_policy_allowed,
            policy_blockers: web_policy_blockers,
            reachability_status,
            reachability_ttl_status: "not_observed".into(),
            cached_or_preflight_known_reachability,
            active_reachability_probe: false,
            available_status,
        },
        mcp: McpAvailabilityFactSnapshot {
            registered_count: mcp_registered_count,
            safe_read_candidate_count: mcp_safe_read_candidate_count,
            server_status: mcp_server_status,
            available_status: mcp_available_status,
            raw_manifest_exposed: false,
        },
        write: WriteAvailabilityFactSnapshot {
            available_status: "proposal_permission_or_blocker".into(),
            requires_permission: true,
            silent_write_available: false,
        },
    }
}

fn web_credential_snapshot(
    config: &AppConfig,
    web_search_configured: bool,
    web_fetch_configured: bool,
) -> (bool, String) {
    if !web_search_configured && !web_fetch_configured {
        return (false, "unconfigured".into());
    }
    if web_fetch_configured {
        return (true, "not_required".into());
    }

    match config
        .system
        .search_provider
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "brave" => {
            let available = !config.system.search_provider_key.trim().is_empty();
            (
                available,
                if available {
                    "configured"
                } else {
                    "missing_search_provider_key"
                }
                .into(),
            )
        }
        "searxng" => {
            let available = !config.system.searxng_url.trim().is_empty();
            (
                available,
                if available {
                    "configured"
                } else {
                    "missing_searxng_url"
                }
                .into(),
            )
        }
        "" | "duckduckgo" => (true, "not_required".into()),
        _ => (false, "unsupported_search_provider".into()),
    }
}

fn web_policy_snapshot(policy: &NetworkPolicy) -> (bool, Vec<String>) {
    let mut blockers = Vec::new();
    if !policy.enabled {
        blockers.push("network_policy_disabled".into());
    }
    for tool_name in ["web.search", "web.fetch"] {
        if policy
            .tool_overrides
            .get(tool_name)
            .is_some_and(|decision| decision == "deny")
        {
            blockers.push(format!("{tool_name}_policy_denied"));
        }
    }
    blockers.sort();
    blockers.dedup();
    (blockers.is_empty(), blockers)
}

fn tool_availability_fact_bindings(
    intent: MainChatToolAvailabilityIntent,
    snapshot: &ToolAvailabilityFactSnapshot,
) -> Vec<MainChatRuntimeFactBinding> {
    match intent {
        MainChatToolAvailabilityIntent::AskToolAvailability => vec![
            tool_fact_binding(
                RUNTIME_FACT_KEY_TOOL_WEB_CONFIG_ENABLED,
                "boolean",
                Some(snapshot.web.config_enabled.to_string()),
                vec!["config", "tool_registry"],
                "config",
                "turn_snapshot",
                "trace_only",
                "internal",
                false,
            ),
            tool_fact_binding(
                RUNTIME_FACT_KEY_TOOL_WEB_CREDENTIAL_AVAILABLE,
                "boolean",
                Some(snapshot.web.credential_available.to_string()),
                vec!["config"],
                "config",
                "turn_snapshot",
                "trace_only",
                "internal",
                false,
            ),
            tool_fact_binding(
                RUNTIME_FACT_KEY_TOOL_WEB_POLICY_ALLOWED,
                "boolean_or_blocker",
                Some(snapshot.web.policy_allowed.to_string()),
                vec!["tool_policy"],
                "policy",
                "turn_snapshot",
                "ui_badge",
                "public",
                false,
            ),
            tool_fact_binding(
                RUNTIME_FACT_KEY_TOOL_WEB_REACHABLE,
                "reachable_unreachable_unknown_or_stale",
                Some(snapshot.web.reachability_status.clone()),
                vec!["tool_preflight"],
                "policy",
                "store_snapshot",
                "trace_only",
                "internal",
                snapshot.web.reachability_status == "unknown",
            ),
            tool_fact_binding(
                RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE,
                "derived_status",
                Some(snapshot.web.available_status.clone()),
                vec!["config", "tool_policy", "tool_preflight"],
                "policy",
                "turn_snapshot",
                "answer",
                "public",
                false,
            ),
            tool_fact_binding(
                RUNTIME_FACT_KEY_TOOL_MCP_REGISTERED_COUNT,
                "integer",
                Some(snapshot.mcp.registered_count.to_string()),
                vec!["tool_registry"],
                "config",
                "turn_snapshot",
                "trace_only",
                "internal",
                false,
            ),
            tool_fact_binding(
                RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT,
                "integer",
                Some(snapshot.mcp.safe_read_candidate_count.to_string()),
                vec!["tool_registry", "tool_policy"],
                "policy",
                "turn_snapshot",
                "answer",
                "internal",
                false,
            ),
            tool_fact_binding(
                RUNTIME_FACT_KEY_TOOL_MCP_SERVER_STATUS,
                "online_offline_or_unknown",
                Some(snapshot.mcp.server_status.clone()),
                vec!["tool_preflight"],
                "policy",
                "turn_snapshot",
                "trace_only",
                "internal",
                snapshot.mcp.server_status == "unknown",
            ),
        ],
        MainChatToolAvailabilityIntent::AskWriteCapability => vec![
            tool_fact_binding(
                RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE,
                "proposal_permission_or_blocker",
                Some(snapshot.write.available_status.clone()),
                vec!["tool_policy"],
                "policy",
                "turn_snapshot",
                "ui_badge",
                "public",
                false,
            ),
            tool_fact_binding(
                RUNTIME_FACT_KEY_TOOL_WRITE_REQUIRES_PERMISSION,
                "boolean",
                Some(snapshot.write.requires_permission.to_string()),
                vec!["tool_policy"],
                "policy",
                "turn_snapshot",
                "trace_only",
                "public",
                false,
            ),
        ],
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
fn tool_fact_binding(
    key: &'static str,
    value_shape: &'static str,
    value: Option<String>,
    source: Vec<&'static str>,
    authority: &'static str,
    freshness: &'static str,
    visibility: &'static str,
    privacy: &'static str,
    missing: bool,
) -> MainChatRuntimeFactBinding {
    MainChatRuntimeFactBinding {
        key,
        value_shape,
        value,
        source,
        authority,
        freshness,
        visibility,
        privacy,
        missing,
    }
}

fn tool_availability_labels(snapshot: &ToolAvailabilityFactSnapshot) -> Vec<String> {
    vec![
        format!(
            "web: config_enabled={} credential_available={} policy_allowed={} reachability={} available={}",
            snapshot.web.config_enabled,
            snapshot.web.credential_available,
            snapshot.web.policy_allowed,
            snapshot.web.reachability_status,
            snapshot.web.available_status
        ),
        format!(
            "mcp: registered_count={} safe_read_candidate_count={} server_status={} available={}",
            snapshot.mcp.registered_count,
            snapshot.mcp.safe_read_candidate_count,
            snapshot.mcp.server_status,
            snapshot.mcp.available_status
        ),
        format!(
            "write: available={} requires_permission={} silent_write_available={}",
            snapshot.write.available_status,
            snapshot.write.requires_permission,
            snapshot.write.silent_write_available
        ),
    ]
}

fn tool_availability_secondary_chips(
    intent: MainChatToolAvailabilityIntent,
    snapshot: &ToolAvailabilityFactSnapshot,
) -> Vec<&'static str> {
    match intent {
        MainChatToolAvailabilityIntent::AskToolAvailability => {
            let mut chips = vec!["无外部调用"];
            if snapshot.web.available_status != "available" {
                chips.push("外部读取未接入");
            }
            if snapshot.mcp.available_status != "available" {
                chips.push("上下文有限");
            }
            chips
        }
        MainChatToolAvailabilityIntent::AskWriteCapability => {
            vec!["无写入", "需要用户确认"]
        }
    }
}

fn tool_availability_ui_status(
    intent: MainChatToolAvailabilityIntent,
    snapshot: &ToolAvailabilityFactSnapshot,
) -> &'static str {
    match intent {
        MainChatToolAvailabilityIntent::AskWriteCapability => "waiting_for_user",
        MainChatToolAvailabilityIntent::AskToolAvailability
            if snapshot.web.available_status == "blocked"
                || snapshot.mcp.available_status == "no_safe_read_candidate" =>
        {
            "restricted"
        }
        MainChatToolAvailabilityIntent::AskToolAvailability
            if snapshot.web.available_status == "unknown"
                || snapshot.mcp.available_status == "unknown_server_status" =>
        {
            "unknown"
        }
        MainChatToolAvailabilityIntent::AskToolAvailability => "completed",
    }
}

fn tool_availability_reply(
    intent: MainChatToolAvailabilityIntent,
    snapshot: &ToolAvailabilityFactSnapshot,
) -> String {
    match intent {
        MainChatToolAvailabilityIntent::AskToolAvailability => {
            let web_policy = if snapshot.web.policy_allowed {
                "策略允许外部读取".to_string()
            } else {
                format!(
                    "策略阻止外部读取（{}）",
                    snapshot.web.policy_blockers.join(", ")
                )
            };
            let web_status = match snapshot.web.available_status.as_str() {
                "blocked" => "因此当前不能把联网能力标为可用。",
                "unknown" => {
                    "但没有缓存或显式 preflight 可达性记录，本轮不会主动探测网络，所以可达性是 unknown。"
                }
                "unconfigured" => "web 工具表面未配置，因此不可用。",
                "available" => "已有缓存/显式 preflight 证明可达，可作为可用能力。",
                _ => "当前可用性有限。",
            };
            let mcp_status = if snapshot.mcp.registered_count == 0 {
                "MCP：没有已注册 MCP manifest。".to_string()
            } else if snapshot.mcp.safe_read_candidate_count == 0 {
                format!(
                    "MCP：registry 中有 {} 个 manifest，但 policy-allowed read-only candidate 为 0，因此不能声称 MCP 可用。",
                    snapshot.mcp.registered_count
                )
            } else {
                format!(
                    "MCP：registry 中有 {} 个 manifest，policy-allowed read-only candidate 为 {}；server_status=unknown，所以只能标为 unknown，不能标为 available。",
                    snapshot.mcp.registered_count, snapshot.mcp.safe_read_candidate_count
                )
            };
            format!(
                "联网/工具可用性来自 runtime facts：web config_enabled={}，credential_available={}（{}），{}，reachability={}。{} {}",
                snapshot.web.config_enabled,
                snapshot.web.credential_available,
                snapshot.web.credential_status,
                web_policy,
                snapshot.web.reachability_status,
                web_status,
                mcp_status
            )
        }
        MainChatToolAvailabilityIntent::AskWriteCapability => {
            "写入能力来自 runtime facts：不能静默写入。当前写能力只允许走 proposal / permission / blocker 路径；write_requires_permission=true，directWritesExecuted=false。".into()
        }
    }
}

pub(crate) fn classify_tool_availability_query(
    user_text: &str,
) -> Option<MainChatToolAvailabilityIntent> {
    let normalized = user_text.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    let compact = normalized
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let compact = trim_outer_punctuation(&compact);
    let english_phrase = trim_outer_punctuation(&normalized);

    if matches_exact_runtime_fact_phrase(
        compact,
        &[
            "你有写入能力吗",
            "你支持写入吗",
            "你能直接写入吗",
            "你会静默写入吗",
            "写入能力是什么",
            "你能做写操作吗",
        ],
    ) || matches_exact_runtime_fact_phrase(
        english_phrase,
        &[
            "can you write",
            "can you write files",
            "do you have write capability",
            "what write capability do you have",
            "can you silently write",
        ],
    ) {
        return Some(MainChatToolAvailabilityIntent::AskWriteCapability);
    }

    if matches_exact_runtime_fact_phrase(
        compact,
        &[
            "你能联网吗",
            "你可以联网吗",
            "你现在能联网吗",
            "能联网吗",
            "可以联网吗",
            "你能上网吗",
            "你可以上网吗",
            "你能访问网页吗",
            "你能用工具吗",
            "你现在有哪些工具能力",
            "你能调用mcp吗",
            "你可以调用mcp吗",
            "mcp可用吗",
        ],
    ) || matches_exact_runtime_fact_phrase(
        english_phrase,
        &[
            "can you access the internet",
            "can you browse the web",
            "do you have internet access",
            "can you use tools",
            "can you use mcp",
            "is mcp available",
        ],
    ) {
        return Some(MainChatToolAvailabilityIntent::AskToolAvailability);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::web_credential_snapshot;

    #[test]
    fn web_credential_truth_matches_the_exact_search_provider_contract() {
        let mut config = openlife_core::config::AppConfig::default();

        config.system.search_provider = "duckduckgo".into();
        assert_eq!(
            web_credential_snapshot(&config, true, false),
            (true, "not_required".into())
        );

        config.system.search_provider = "brave".into();
        assert_eq!(
            web_credential_snapshot(&config, true, false),
            (false, "missing_search_provider_key".into())
        );

        config.system.search_provider = "searxng".into();
        assert_eq!(
            web_credential_snapshot(&config, true, false),
            (false, "missing_searxng_url".into())
        );

        config.system.search_provider = "unimplemented-search".into();
        assert_eq!(
            web_credential_snapshot(&config, true, false),
            (false, "unsupported_search_provider".into()),
            "runtime facts must not report a provider available when ToolGateway fails it closed"
        );

        assert_eq!(
            web_credential_snapshot(&config, true, true),
            (true, "not_required".into()),
            "web.fetch remains independently available when search is misconfigured"
        );
    }
}
