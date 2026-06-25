use chrono::{Datelike, Offset};
use openlife_core::agent::{
    model_router::{ModelRouter, ProviderAvailability},
    AgentRunStatus, ModelRouteTrace,
};
use openlife_core::config::{AppConfig, NetworkPolicy};
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;
use openlife_core::tool_manifest::{ToolManifest, ToolSource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::main_chat_react_tool_selection::main_chat_manifest_is_governed_read_candidate;
use crate::AppState;

pub(crate) const RUNTIME_FACT_SOURCE_TYPE: &str = "runtime_fact";
pub(crate) const RUNTIME_FACT_KEY_DATE: &str = "runtime.current_time.date";
pub(crate) const RUNTIME_FACT_KEY_TIME: &str = "runtime.current_time.time";
pub(crate) const RUNTIME_FACT_KEY_WEEKDAY: &str = "runtime.current_time.weekday";
pub(crate) const RUNTIME_FACT_KEY_TIMEZONE: &str = "runtime.current_time.timezone";
pub(crate) const RUNTIME_FACT_KEY_TRACE_GAP: &str = "runtime.current_time.trace_gap";
pub(crate) const RUNTIME_FACT_PROVIDER_GENERATION_PATH: &str = "main_chat_runtime_fact";
pub(crate) const RUNTIME_FACT_PROVIDER_ROUTE_GENERATION_PATH: &str =
    "main_chat_provider_route_runtime_fact";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER: &str =
    "provider.configured.default_provider";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_MODEL: &str =
    "provider.configured.default_model";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_CURRENT_PROVIDER: &str =
    "provider.current_turn_generation.provider";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL: &str =
    "provider.current_turn_generation.model";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_CURRENT_ROUTE_TYPE: &str =
    "provider.current_turn_generation.route_type";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED: &str =
    "provider.current_turn_generation.model_generated";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_PROVIDER: &str =
    "provider.last_completed_generation.provider";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_MODEL: &str =
    "provider.last_completed_generation.model";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_RUN_ID: &str =
    "provider.last_completed_generation.run_id";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER: &str =
    "provider.planned_route_if_model_needed.provider";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_PLANNED_MODEL: &str =
    "provider.planned_route_if_model_needed.model";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_PLANNED_ROUTE_TYPE: &str =
    "provider.planned_route_if_model_needed.route_type";
pub(crate) const RUNTIME_FACT_KEY_PROVIDER_PREFLIGHT_STATUS: &str = "provider.preflight.status";
pub(crate) const RUNTIME_FACT_TOOL_AVAILABILITY_GENERATION_PATH: &str =
    "main_chat_tool_availability_runtime_fact";
pub(crate) const RUNTIME_FACT_KEY_TOOL_WEB_CONFIG_ENABLED: &str = "tool.web.config_enabled";
pub(crate) const RUNTIME_FACT_KEY_TOOL_WEB_CREDENTIAL_AVAILABLE: &str =
    "tool.web.credential_available";
pub(crate) const RUNTIME_FACT_KEY_TOOL_WEB_POLICY_ALLOWED: &str = "tool.web.policy_allowed";
pub(crate) const RUNTIME_FACT_KEY_TOOL_WEB_REACHABLE: &str = "tool.web.reachable";
pub(crate) const RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE: &str = "tool.web.available";
pub(crate) const RUNTIME_FACT_KEY_TOOL_MCP_REGISTERED_COUNT: &str = "tool.mcp.registered_count";
pub(crate) const RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT: &str =
    "tool.mcp.read_only_allowed_count";
pub(crate) const RUNTIME_FACT_KEY_TOOL_MCP_SERVER_STATUS: &str = "tool.mcp.server_status";
pub(crate) const RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE: &str = "tool.write.available";
pub(crate) const RUNTIME_FACT_KEY_TOOL_WRITE_REQUIRES_PERMISSION: &str =
    "tool.write.requires_permission";

const SLICE_A_SCENARIOS: [&str; 6] = ["RF-01", "RF-02", "RF-03", "RF-04", "RF-05", "RF-06"];
const SLICE_B_SCENARIOS: [&str; 4] = ["RF-07", "RF-08", "RF-09", "RF-10"];
const SLICE_C_SCENARIOS: [&str; 5] = ["RF-11", "RF-12", "RF-13", "RF-14", "RF-15"];
const FIXED_CLOCK_RFC3339: &str = "2026-06-23T09:15:00+08:00";

#[derive(Debug, Clone)]
pub enum MainChatRuntimeClockSource {
    LocalSystem,
    Fixed(chrono::DateTime<chrono::FixedOffset>),
    Unavailable,
}

impl Default for MainChatRuntimeClockSource {
    fn default() -> Self {
        Self::LocalSystem
    }
}

impl MainChatRuntimeClockSource {
    fn now(&self) -> Option<chrono::DateTime<chrono::FixedOffset>> {
        match self {
            Self::LocalSystem => {
                let now = chrono::Local::now();
                let fixed_offset = now.offset().fix();
                Some(now.with_timezone(&fixed_offset))
            }
            Self::Fixed(now) => Some(*now),
            Self::Unavailable => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatRuntimeClockIntent {
    AskCurrentWeekday,
    AskCurrentDate,
    AskCurrentTime,
}

impl MainChatRuntimeClockIntent {
    fn as_str(self) -> &'static str {
        match self {
            Self::AskCurrentWeekday => "ask_current_weekday",
            Self::AskCurrentDate => "ask_current_date",
            Self::AskCurrentTime => "ask_current_time",
        }
    }

    fn fact_keys(self) -> Vec<&'static str> {
        match self {
            Self::AskCurrentWeekday => vec![
                RUNTIME_FACT_KEY_DATE,
                RUNTIME_FACT_KEY_WEEKDAY,
                RUNTIME_FACT_KEY_TIMEZONE,
            ],
            Self::AskCurrentDate => vec![
                RUNTIME_FACT_KEY_DATE,
                RUNTIME_FACT_KEY_WEEKDAY,
                RUNTIME_FACT_KEY_TIMEZONE,
            ],
            Self::AskCurrentTime => vec![
                RUNTIME_FACT_KEY_DATE,
                RUNTIME_FACT_KEY_TIME,
                RUNTIME_FACT_KEY_TIMEZONE,
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactAnswer {
    pub(crate) reply: String,
    pub(crate) intent: String,
    pub(crate) fact_keys: Vec<&'static str>,
    pub(crate) facts: Vec<MainChatRuntimeFactBinding>,
    pub(crate) observed_at: Option<String>,
    pub(crate) source: Vec<&'static str>,
    pub(crate) authority: &'static str,
    pub(crate) freshness: &'static str,
    pub(crate) visibility: Vec<&'static str>,
    pub(crate) privacy: Vec<&'static str>,
    pub(crate) timezone: Option<String>,
    pub(crate) trace_gap: bool,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub(crate) extra_metadata: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactBinding {
    pub(crate) key: &'static str,
    pub(crate) value_shape: &'static str,
    pub(crate) value: Option<String>,
    pub(crate) source: Vec<&'static str>,
    pub(crate) authority: &'static str,
    pub(crate) freshness: &'static str,
    pub(crate) visibility: &'static str,
    pub(crate) privacy: &'static str,
    pub(crate) missing: bool,
}

impl MainChatRuntimeFactAnswer {
    pub(crate) fn generation_metadata(&self) -> Value {
        let mut metadata = serde_json::json!({
            "sourceType": RUNTIME_FACT_SOURCE_TYPE,
            "runtimeFactKeys": self.fact_keys,
            "runtimeFacts": self.facts,
            "runtimeFactSource": self.source,
            "runtimeFactAuthority": self.authority,
            "runtimeFactFreshness": self.freshness,
            "runtimeFactVisibility": self.visibility,
            "runtimeFactPrivacy": self.privacy,
            "runtimeFactIntent": self.intent.as_str(),
            "runtimeFactObservedAt": self.observed_at,
            "runtimeFactTimezone": self.timezone,
            "runtimeFactTtl": "none",
            "runtimeFactTtlStatus": if self.trace_gap { "not_observed" } else { "fresh" },
            "runtimeFactMissingBehavior": "answer_unknown",
            "runtimeFactModelFallbackAllowed": false,
            "runtimeFactTraceGap": self.trace_gap,
            "modelGenerated": false,
            "schedulerGenerationCalled": false,
            "toolCalled": false,
            "directWritesExecuted": false,
            "legacyFallbackUsed": false,
            "providerGenerationPath": RUNTIME_FACT_PROVIDER_GENERATION_PATH,
            "currentTurnGenerationProvider": null,
            "currentTurnGenerationModel": null,
            "currentTurnGenerationRouteType": "none",
            "currentTurnGenerationModelGenerated": false,
        });
        merge_json_object(&mut metadata, self.extra_metadata.clone());
        metadata
    }
}

pub(crate) fn resolve_runtime_clock_fact_answer(
    user_text: &str,
    clock_source: &MainChatRuntimeClockSource,
) -> Option<MainChatRuntimeFactAnswer> {
    let intent = classify_runtime_clock_query(user_text)?;
    let fact_keys = intent.fact_keys();
    let Some(now) = clock_source.now() else {
        let mut trace_gap_keys = fact_keys.clone();
        trace_gap_keys.push(RUNTIME_FACT_KEY_TRACE_GAP);
        return Some(MainChatRuntimeFactAnswer {
            reply: "当前时间未知：本机运行时钟不可用，无法回答当前日期或时间。".into(),
            intent: intent.as_str().into(),
            facts: missing_clock_fact_bindings(&fact_keys),
            fact_keys: trace_gap_keys,
            observed_at: None,
            source: vec!["local_clock"],
            authority: "runtime",
            freshness: "unknown",
            visibility: vec!["answer", "trace_only"],
            privacy: vec!["public", "internal"],
            timezone: None,
            trace_gap: true,
            extra_metadata: Value::Null,
        });
    };

    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();
    let weekday = chinese_weekday(now.weekday());
    let timezone = format!("UTC{}", now.format("%:z"));
    let facts = runtime_clock_fact_bindings(intent, &date, &time, weekday, &timezone);
    let reply = match intent {
        MainChatRuntimeClockIntent::AskCurrentTime => format!(
            "根据本机运行时钟，现在是 {} {}，{}（{}）。",
            date, time, weekday, timezone
        ),
        MainChatRuntimeClockIntent::AskCurrentDate
        | MainChatRuntimeClockIntent::AskCurrentWeekday => format!(
            "根据本机运行时钟，今天是 {}，{}（{}）。",
            date, weekday, timezone
        ),
    };

    Some(MainChatRuntimeFactAnswer {
        reply,
        intent: intent.as_str().into(),
        fact_keys,
        facts,
        observed_at: Some(now.to_rfc3339()),
        source: vec!["local_clock"],
        authority: "runtime",
        freshness: "instant",
        visibility: vec!["answer"],
        privacy: vec!["public", "internal"],
        timezone: Some(timezone),
        trace_gap: false,
        extra_metadata: Value::Null,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatProviderRouteIntent {
    AskCurrentModelRoute,
    AskPreviousTurnModelRoute,
}

impl MainChatProviderRouteIntent {
    fn as_str(self) -> &'static str {
        match self {
            Self::AskCurrentModelRoute => "ask_model_route",
            Self::AskPreviousTurnModelRoute => "ask_previous_turn_model_route",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatToolAvailabilityIntent {
    AskToolAvailability,
    AskWriteCapability,
}

impl MainChatToolAvailabilityIntent {
    fn as_str(self) -> &'static str {
        match self {
            Self::AskToolAvailability => "ask_tool_availability",
            Self::AskWriteCapability => "ask_write_capability",
        }
    }

    fn fact_keys(self) -> Vec<&'static str> {
        match self {
            Self::AskToolAvailability => vec![
                RUNTIME_FACT_KEY_TOOL_WEB_CONFIG_ENABLED,
                RUNTIME_FACT_KEY_TOOL_WEB_CREDENTIAL_AVAILABLE,
                RUNTIME_FACT_KEY_TOOL_WEB_POLICY_ALLOWED,
                RUNTIME_FACT_KEY_TOOL_WEB_REACHABLE,
                RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE,
                RUNTIME_FACT_KEY_TOOL_MCP_REGISTERED_COUNT,
                RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT,
                RUNTIME_FACT_KEY_TOOL_MCP_SERVER_STATUS,
            ],
            Self::AskWriteCapability => vec![
                RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE,
                RUNTIME_FACT_KEY_TOOL_WRITE_REQUIRES_PERMISSION,
            ],
        }
    }
}

#[derive(Debug, Clone)]
struct ProviderRouteFactSnapshot {
    provider: Option<String>,
    model: Option<String>,
    route_type: Option<String>,
    run_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ProviderPreflightFactSnapshot {
    status: String,
    blockers: Vec<String>,
}

pub(crate) async fn provider_route_fact_should_block_before_model(
    state: &Arc<AppState>,
    scheduler: &InferenceScheduler,
) -> bool {
    let config = state.config.lock().await.clone();
    let planned = planned_route_without_probe(scheduler);
    let preflight = provider_preflight_snapshot(&config, scheduler, &planned);
    preflight.status == "blocked"
}

pub(crate) async fn resolve_provider_route_fact_answer(
    user_text: &str,
    state: &Arc<AppState>,
    scheduler: &InferenceScheduler,
    session_id: &str,
    current_route: Option<ModelRouteTrace>,
    current_model_generated: bool,
    scheduler_generation_called: bool,
    provider_generation_path: &str,
) -> Option<MainChatRuntimeFactAnswer> {
    let intent = classify_provider_route_query(user_text)?;
    let config = state.config.lock().await.clone();
    let planned_route = planned_route_without_probe(scheduler);
    let preflight = provider_preflight_snapshot(&config, scheduler, &planned_route);
    let configured = ProviderRouteFactSnapshot {
        provider: Some(bounded_runtime_fact_label(&config.llm.provider)),
        model: Some(bounded_runtime_fact_label(&config.llm.chat_model)),
        route_type: None,
        run_id: None,
    };
    let planned = route_snapshot_from_trace(&planned_route, None);
    let current = if current_model_generated {
        current_route
            .as_ref()
            .map(|route| route_snapshot_from_trace(route, None))
            .unwrap_or_else(no_current_generation_snapshot)
    } else {
        no_current_generation_snapshot()
    };
    let last_completed_generation = last_completed_generation_snapshot(state, session_id).await;
    let last_turn = last_turn_snapshot(state, session_id).await;
    let facts = provider_route_fact_bindings(
        &configured,
        &current,
        &last_completed_generation,
        &planned,
        current_model_generated,
        &preflight,
    );
    let fact_keys = provider_route_fact_keys();
    let route_labels = provider_route_labels(
        &configured,
        &current,
        &last_completed_generation,
        &planned,
        current_model_generated,
        &preflight,
    );
    let reply = provider_route_reply(
        intent,
        &configured,
        &current,
        &last_completed_generation,
        &planned,
        &last_turn,
        current_model_generated,
        &preflight,
    );
    let mut extra_metadata = serde_json::json!({
        "providerGenerationPath": provider_generation_path,
        "modelGenerated": current_model_generated,
        "schedulerGenerationCalled": scheduler_generation_called,
        "toolCalled": false,
        "directWritesExecuted": false,
        "legacyFallbackUsed": false,
        "configuredProvider": configured.provider.clone(),
        "configuredModel": configured.model.clone(),
        "configuredDefaultRouteLabel": route_labels.configured.clone(),
        "currentTurnGenerationProvider": current.provider.clone(),
        "currentTurnGenerationModel": current.model.clone(),
        "currentTurnGenerationRouteType": current.route_type.clone().unwrap_or_else(|| "none".into()),
        "currentTurnGenerationModelGenerated": current_model_generated,
        "currentTurnGenerationRouteLabel": route_labels.current.clone(),
        "lastCompletedGenerationProvider": last_completed_generation.as_ref().and_then(|route| route.provider.clone()),
        "lastCompletedGenerationModel": last_completed_generation.as_ref().and_then(|route| route.model.clone()),
        "lastCompletedGenerationRunId": last_completed_generation.as_ref().and_then(|route| route.run_id.clone()),
        "lastCompletedGenerationRouteLabel": route_labels.last_completed.clone(),
        "plannedRouteIfModelNeededProvider": planned.provider.clone(),
        "plannedRouteIfModelNeededModel": planned.model.clone(),
        "plannedRouteIfModelNeededRouteType": planned.route_type.clone(),
        "plannedRouteIfModelNeededLabel": route_labels.planned.clone(),
        "providerPreflightStatus": preflight.status.clone(),
        "providerPreflightBlockers": preflight.blockers.clone(),
        "providerPreflightIsInvocationProof": false,
        "routeLabels": [
            route_labels.current.clone(),
            route_labels.last_completed.clone(),
            route_labels.configured.clone(),
            route_labels.planned.clone(),
        ],
        "uiPrimarySourceChip": "运行时路线",
        "uiStatus": if preflight.status == "blocked" { "restricted" } else { "completed" },
    });
    if let Some(last_turn) = last_turn {
        merge_json_object(
            &mut extra_metadata,
            serde_json::json!({
                "lastTurnProvider": last_turn.provider,
                "lastTurnModel": last_turn.model,
                "lastTurnRouteType": last_turn.route_type,
                "lastTurnModelGenerated": route_snapshot_is_model_generation(&last_turn),
            }),
        );
    }

    Some(MainChatRuntimeFactAnswer {
        reply,
        intent: intent.as_str().into(),
        fact_keys,
        facts,
        observed_at: Some(chrono::Utc::now().to_rfc3339()),
        source: vec![
            "provider_route",
            "agent_run",
            "config",
            "model_router",
            "provider_preflight",
        ],
        authority: if preflight.status == "blocked" {
            "policy"
        } else {
            "run_trace"
        },
        freshness: "run_trace",
        visibility: vec!["answer", "ui_badge", "trace_only"],
        privacy: vec!["internal"],
        timezone: None,
        trace_gap: false,
        extra_metadata,
    })
}

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
        _ => (true, "not_required".into()),
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

#[allow(clippy::too_many_arguments)]
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

fn runtime_clock_fact_bindings(
    intent: MainChatRuntimeClockIntent,
    date: &str,
    time: &str,
    weekday: &str,
    timezone: &str,
) -> Vec<MainChatRuntimeFactBinding> {
    let mut facts = vec![
        clock_fact_binding(
            RUNTIME_FACT_KEY_DATE,
            "YYYY-MM-DD",
            Some(date),
            "answer",
            "public",
            "instant",
            false,
        ),
        clock_fact_binding(
            RUNTIME_FACT_KEY_WEEKDAY,
            "localized_weekday_label",
            Some(weekday),
            "answer",
            "public",
            "instant",
            false,
        ),
        clock_fact_binding(
            RUNTIME_FACT_KEY_TIMEZONE,
            "offset_label",
            Some(timezone),
            "answer",
            "internal",
            "instant",
            false,
        ),
    ];
    if intent == MainChatRuntimeClockIntent::AskCurrentTime {
        facts.insert(
            1,
            clock_fact_binding(
                RUNTIME_FACT_KEY_TIME,
                "HH:mm",
                Some(time),
                "answer",
                "public",
                "instant",
                false,
            ),
        );
    }
    facts
}

fn missing_clock_fact_bindings(keys: &[&'static str]) -> Vec<MainChatRuntimeFactBinding> {
    keys.iter()
        .copied()
        .map(|key| {
            let (value_shape, privacy) = match key {
                RUNTIME_FACT_KEY_DATE => ("YYYY-MM-DD", "public"),
                RUNTIME_FACT_KEY_TIME => ("HH:mm", "public"),
                RUNTIME_FACT_KEY_WEEKDAY => ("localized_weekday_label", "public"),
                RUNTIME_FACT_KEY_TIMEZONE => ("offset_label", "internal"),
                _ => ("unknown", "internal"),
            };
            clock_fact_binding(
                key,
                value_shape,
                None,
                "trace_only",
                privacy,
                "unknown",
                true,
            )
        })
        .collect()
}

fn clock_fact_binding(
    key: &'static str,
    value_shape: &'static str,
    value: Option<&str>,
    visibility: &'static str,
    privacy: &'static str,
    freshness: &'static str,
    missing: bool,
) -> MainChatRuntimeFactBinding {
    MainChatRuntimeFactBinding {
        key,
        value_shape,
        value: value.map(str::to_string),
        source: vec!["local_clock"],
        authority: "runtime",
        freshness,
        visibility,
        privacy,
        missing,
    }
}

pub(crate) fn classify_provider_route_query(
    user_text: &str,
) -> Option<MainChatProviderRouteIntent> {
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

    if matches_exact_clock_phrase(
        compact,
        &[
            "刚才回答今天星期几用了什么模型",
            "刚才回答今天星期几时用了什么模型",
            "上一轮用了什么模型",
            "上次回答用了什么模型",
            "刚刚用了什么模型",
        ],
    ) || matches_exact_clock_phrase(
        english_phrase,
        &[
            "what model did you use last turn",
            "what model did you use for the last answer",
            "which model answered the previous turn",
        ],
    ) {
        return Some(MainChatProviderRouteIntent::AskPreviousTurnModelRoute);
    }

    if matches_exact_clock_phrase(
        compact,
        &[
            "你现在用什么模型",
            "你当前用什么模型",
            "当前用什么模型",
            "现在用什么模型",
            "你现在用哪个模型",
            "你当前用哪个模型",
            "你现在走什么模型路线",
            "当前模型路线是什么",
        ],
    ) || matches_exact_clock_phrase(
        english_phrase,
        &[
            "what model are you using",
            "what model are you using now",
            "which model are you using",
            "which provider are you using",
            "what provider are you using",
            "what is your current model route",
        ],
    ) {
        return Some(MainChatProviderRouteIntent::AskCurrentModelRoute);
    }

    None
}

fn provider_route_fact_keys() -> Vec<&'static str> {
    vec![
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_PROVIDER,
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL,
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_ROUTE_TYPE,
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED,
        RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_PROVIDER,
        RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_MODEL,
        RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_RUN_ID,
        RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER,
        RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_MODEL,
        RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER,
        RUNTIME_FACT_KEY_PROVIDER_PLANNED_MODEL,
        RUNTIME_FACT_KEY_PROVIDER_PLANNED_ROUTE_TYPE,
        RUNTIME_FACT_KEY_PROVIDER_PREFLIGHT_STATUS,
    ]
}

fn provider_route_fact_bindings(
    configured: &ProviderRouteFactSnapshot,
    current: &ProviderRouteFactSnapshot,
    last_completed: &Option<ProviderRouteFactSnapshot>,
    planned: &ProviderRouteFactSnapshot,
    current_model_generated: bool,
    preflight: &ProviderPreflightFactSnapshot,
) -> Vec<MainChatRuntimeFactBinding> {
    let mut facts = Vec::new();
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_PROVIDER,
        "bounded_label_or_none",
        current.provider.as_deref(),
        vec!["provider_route", "agent_run"],
        "run_trace",
        "run_trace",
        "ui_badge",
        current.provider.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL,
        "bounded_label_or_none",
        current.model.as_deref(),
        vec!["provider_route", "agent_run"],
        "run_trace",
        "run_trace",
        "ui_badge",
        current.model.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_ROUTE_TYPE,
        "local_cloud_direct_or_none",
        current.route_type.as_deref().or(Some("none")),
        vec!["provider_route"],
        "run_trace",
        "run_trace",
        "ui_badge",
        false,
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED,
        "boolean",
        Some(if current_model_generated {
            "true"
        } else {
            "false"
        }),
        vec!["generation_metadata"],
        "run_trace",
        "run_trace",
        "trace_only",
        false,
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_PROVIDER,
        "bounded_label",
        last_completed
            .as_ref()
            .and_then(|route| route.provider.as_deref()),
        vec!["agent_run"],
        "run_trace",
        "store_snapshot",
        "trace_only",
        last_completed.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_MODEL,
        "bounded_label",
        last_completed
            .as_ref()
            .and_then(|route| route.model.as_deref()),
        vec!["agent_run"],
        "run_trace",
        "store_snapshot",
        "trace_only",
        last_completed.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_RUN_ID,
        "bounded_id",
        last_completed
            .as_ref()
            .and_then(|route| route.run_id.as_deref()),
        vec!["agent_run"],
        "run_trace",
        "store_snapshot",
        "trace_only",
        last_completed.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER,
        "bounded_label",
        configured.provider.as_deref(),
        vec!["config"],
        "config",
        "turn_snapshot",
        "trace_only",
        configured.provider.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_MODEL,
        "bounded_label",
        configured.model.as_deref(),
        vec!["config"],
        "config",
        "turn_snapshot",
        "trace_only",
        configured.model.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER,
        "bounded_label",
        planned.provider.as_deref(),
        vec!["provider_route", "model_router"],
        "config",
        "turn_snapshot",
        "trace_only",
        planned.provider.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_PLANNED_MODEL,
        "bounded_label",
        planned.model.as_deref(),
        vec!["provider_route", "model_router"],
        "config",
        "turn_snapshot",
        "trace_only",
        planned.model.is_none(),
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_PLANNED_ROUTE_TYPE,
        "local_cloud_or_unknown",
        planned.route_type.as_deref().or(Some("unknown")),
        vec!["provider_route", "model_router"],
        "config",
        "turn_snapshot",
        "trace_only",
        false,
    ));
    facts.push(provider_fact_binding(
        RUNTIME_FACT_KEY_PROVIDER_PREFLIGHT_STATUS,
        "ready_or_blocker_labels",
        Some(preflight.status.as_str()),
        vec!["provider_preflight"],
        "policy",
        "turn_snapshot",
        "trace_only",
        false,
    ));
    facts
}

fn provider_fact_binding(
    key: &'static str,
    value_shape: &'static str,
    value: Option<&str>,
    source: Vec<&'static str>,
    authority: &'static str,
    freshness: &'static str,
    visibility: &'static str,
    missing: bool,
) -> MainChatRuntimeFactBinding {
    MainChatRuntimeFactBinding {
        key,
        value_shape,
        value: value.map(str::to_string),
        source,
        authority,
        freshness,
        visibility,
        privacy: "internal",
        missing,
    }
}

#[derive(Debug, Clone)]
struct ProviderRouteLabels {
    current: String,
    last_completed: String,
    configured: String,
    planned: String,
}

fn provider_route_labels(
    configured: &ProviderRouteFactSnapshot,
    current: &ProviderRouteFactSnapshot,
    last_completed: &Option<ProviderRouteFactSnapshot>,
    planned: &ProviderRouteFactSnapshot,
    current_model_generated: bool,
    preflight: &ProviderPreflightFactSnapshot,
) -> ProviderRouteLabels {
    let current_label = if current_model_generated {
        format!(
            "current_turn_generation: actual {} / {} ({})",
            label_or_unknown(current.provider.as_deref()),
            label_or_unknown(current.model.as_deref()),
            label_or_unknown(current.route_type.as_deref())
        )
    } else {
        "current_turn_generation: no model generated in this turn".into()
    };
    let last_label = last_completed
        .as_ref()
        .map(|route| {
            format!(
                "last_completed_generation: {} / {} ({}) run {}",
                label_or_unknown(route.provider.as_deref()),
                label_or_unknown(route.model.as_deref()),
                label_or_unknown(route.route_type.as_deref()),
                label_or_unknown(route.run_id.as_deref())
            )
        })
        .unwrap_or_else(|| "last_completed_generation: unknown".into());
    let configured_label = format!(
        "configured_default_route: {} / {}",
        label_or_unknown(configured.provider.as_deref()),
        label_or_unknown(configured.model.as_deref())
    );
    let planned_label = format!(
        "planned_route_if_model_needed: {} / {} ({}) preflight={}",
        label_or_unknown(planned.provider.as_deref()),
        label_or_unknown(planned.model.as_deref()),
        label_or_unknown(planned.route_type.as_deref()),
        preflight.status
    );

    ProviderRouteLabels {
        current: current_label,
        last_completed: last_label,
        configured: configured_label,
        planned: planned_label,
    }
}

fn provider_route_reply(
    intent: MainChatProviderRouteIntent,
    configured: &ProviderRouteFactSnapshot,
    current: &ProviderRouteFactSnapshot,
    last_completed: &Option<ProviderRouteFactSnapshot>,
    planned: &ProviderRouteFactSnapshot,
    last_turn: &Option<ProviderRouteFactSnapshot>,
    current_model_generated: bool,
    preflight: &ProviderPreflightFactSnapshot,
) -> String {
    let configured_label = format!(
        "{} / {}",
        label_or_unknown(configured.provider.as_deref()),
        label_or_unknown(configured.model.as_deref())
    );
    let planned_label = format!(
        "{} / {} ({})",
        label_or_unknown(planned.provider.as_deref()),
        label_or_unknown(planned.model.as_deref()),
        label_or_unknown(planned.route_type.as_deref())
    );
    let last_completed_label = last_completed
        .as_ref()
        .map(|route| {
            format!(
                "{} / {} ({})，run {}",
                label_or_unknown(route.provider.as_deref()),
                label_or_unknown(route.model.as_deref()),
                label_or_unknown(route.route_type.as_deref()),
                label_or_unknown(route.run_id.as_deref())
            )
        })
        .unwrap_or_else(|| "未知：本会话还没有已完成的模型生成记录".into());
    let preflight_label = if preflight.blockers.is_empty() {
        "provider.preflight.status=ready（这只是路由前置状态，不是实际调用证明）".into()
    } else {
        format!(
            "provider.preflight.status=blocked（{}）；这不是 readiness，也不会当作实际模型调用证明",
            preflight.blockers.join(", ")
        )
    };
    let last_turn_label = last_turn
        .as_ref()
        .map(|route| {
            if route_snapshot_is_model_generation(route) {
                format!(
                    "上一轮记录为模型生成：{} / {} ({})。",
                    label_or_unknown(route.provider.as_deref()),
                    label_or_unknown(route.model.as_deref()),
                    label_or_unknown(route.route_type.as_deref())
                )
            } else {
                "上一轮是确定性 runtime fact/direct 路径，没有调用模型。".into()
            }
        })
        .unwrap_or_else(|| "上一轮没有可用运行记录。".into());

    match intent {
        MainChatProviderRouteIntent::AskCurrentModelRoute if current_model_generated => format!(
            "current_turn_generation：本轮实际调用的是 {} / {}（{}）。configured_default_route：{}。planned_route_if_model_needed：{}。last_completed_generation：{}。{}",
            label_or_unknown(current.provider.as_deref()),
            label_or_unknown(current.model.as_deref()),
            label_or_unknown(current.route_type.as_deref()),
            configured_label,
            planned_label,
            last_completed_label,
            preflight_label
        ),
        MainChatProviderRouteIntent::AskCurrentModelRoute => format!(
            "current_turn_generation：本轮没有调用模型，因此没有 current-turn provider/model。configured_default_route：{}。planned_route_if_model_needed：{}。last_completed_generation：{}。{}",
            configured_label, planned_label, last_completed_label, preflight_label
        ),
        MainChatProviderRouteIntent::AskPreviousTurnModelRoute => format!(
            "{} current_turn_generation：本轮为了回答这个 runtime fact 问题没有调用模型，因此没有 current-turn provider/model。configured_default_route：{}。planned_route_if_model_needed：{}。last_completed_generation：{}。{}",
            last_turn_label, configured_label, planned_label, last_completed_label, preflight_label
        ),
    }
}

async fn last_turn_snapshot(
    state: &Arc<AppState>,
    session_id: &str,
) -> Option<ProviderRouteFactSnapshot> {
    let store_arc = state.agent_run_store.as_ref()?;
    let store = store_arc.lock().await;
    let runs = store.list_runs_for_session(session_id, 5).ok()?;
    runs.into_iter()
        .find(|run| run.status == AgentRunStatus::Completed)
        .and_then(|run| {
            run.model_route
                .as_ref()
                .map(|route| route_snapshot_from_trace(route, Some(run.id.as_str())))
        })
}

async fn last_completed_generation_snapshot(
    state: &Arc<AppState>,
    session_id: &str,
) -> Option<ProviderRouteFactSnapshot> {
    let store_arc = state.agent_run_store.as_ref()?;
    let store = store_arc.lock().await;
    let runs = store.list_runs_for_session(session_id, 20).ok()?;
    runs.into_iter()
        .filter(|run| run.status == AgentRunStatus::Completed)
        .find_map(|run| {
            let route = run.model_route.as_ref()?;
            let snapshot = route_snapshot_from_trace(route, Some(run.id.as_str()));
            route_snapshot_is_model_generation(&snapshot).then_some(snapshot)
        })
}

fn route_snapshot_from_trace(
    route: &ModelRouteTrace,
    run_id: Option<&str>,
) -> ProviderRouteFactSnapshot {
    ProviderRouteFactSnapshot {
        provider: Some(bounded_runtime_fact_label(&route.provider)),
        model: Some(bounded_runtime_fact_label(&route.model)),
        route_type: Some(bounded_runtime_fact_label(&route.route_type)),
        run_id: run_id.map(bounded_runtime_fact_label),
    }
}

fn no_current_generation_snapshot() -> ProviderRouteFactSnapshot {
    ProviderRouteFactSnapshot {
        provider: None,
        model: None,
        route_type: Some("none".into()),
        run_id: None,
    }
}

fn route_snapshot_is_model_generation(route: &ProviderRouteFactSnapshot) -> bool {
    let provider = route.provider.as_deref().unwrap_or_default();
    let model = route.model.as_deref().unwrap_or_default();
    let route_type = route.route_type.as_deref().unwrap_or_default();
    !provider.is_empty()
        && provider != "direct"
        && !matches!(model, "runtime_fact" | "L1_reflex")
        && !matches!(route_type, "direct" | "none")
}

fn provider_preflight_snapshot(
    config: &AppConfig,
    scheduler: &InferenceScheduler,
    planned: &ModelRouteTrace,
) -> ProviderPreflightFactSnapshot {
    let mut blockers = Vec::new();
    let planned_route_type = planned.route_type.trim().to_ascii_lowercase();
    let planned_provider = planned.provider.trim().to_ascii_lowercase();
    let configured_provider = config.llm.provider.trim().to_ascii_lowercase();
    if configured_provider.is_empty() || configured_provider == "none" {
        blockers.push("configured_provider_missing".into());
    }
    let configured_cloud_provider = is_cloud_route_provider(&configured_provider);
    let planned_cloud_route = planned_route_type == "cloud";
    let cloud_route_needs_preflight = configured_cloud_provider || planned_cloud_route;
    if cloud_route_needs_preflight && !config.system.network_policy.enabled {
        blockers.push("network_disabled".into());
    }
    if ((configured_cloud_provider && config.effective_cloud_api_key().trim().is_empty())
        || (planned_cloud_route && scheduler.effective_api_key().trim().is_empty()))
        && scheduler.scripted_generation_response.is_none()
    {
        blockers.push("provider_api_key_missing".into());
    }
    if planned_provider.is_empty() || planned_provider == "none" || planned_route_type == "fallback"
    {
        blockers.push("provider_route_unavailable".into());
    }
    blockers.sort();
    blockers.dedup();
    ProviderPreflightFactSnapshot {
        status: if blockers.is_empty() {
            "ready".into()
        } else {
            "blocked".into()
        },
        blockers,
    }
}

fn is_cloud_route_provider(provider: &str) -> bool {
    let provider = provider.trim().to_ascii_lowercase();
    !matches!(
        provider.as_str(),
        "" | "none" | "ollama" | "local" | "direct" | "runtime_fact"
    )
}

fn planned_route_without_probe(scheduler: &InferenceScheduler) -> ModelRouteTrace {
    if let Some(router) = scheduler.model_router.as_ref() {
        if let Ok(decision) = router.route_chat(None, scheduler.prefer_local) {
            return decision.to_trace();
        }
    }

    let has_remote_key = !scheduler.effective_api_key().trim().is_empty();
    let use_configured_local = scheduler.prefer_local && !has_remote_key;
    let (provider, model, route_type, reason) = if use_configured_local {
        (
            "ollama".to_string(),
            scheduler.local_model.clone(),
            "local".to_string(),
            "configured_local_preference_without_probe".to_string(),
        )
    } else if scheduler.provider.trim().is_empty() || scheduler.provider == "none" {
        (
            "none".to_string(),
            scheduler.chat_model.clone(),
            "unknown".to_string(),
            "configured_provider_missing_without_probe".to_string(),
        )
    } else {
        (
            scheduler.provider.clone(),
            scheduler.chat_model.clone(),
            "cloud".to_string(),
            "configured_cloud_route_without_probe".to_string(),
        )
    };

    ModelRouteTrace {
        provider,
        model,
        route_type,
        prefer_local: scheduler.prefer_local,
        local_model: scheduler.local_model.clone(),
        reason,
        privacy_level: openlife_core::agent::RedactionLevel::None,
        latency_ms: None,
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(true),
    }
}

fn bounded_runtime_fact_label(value: &str) -> String {
    let mut label = value
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(96)
        .collect::<String>();
    if label.is_empty() {
        label = "unknown".into();
    }
    label
}

fn label_or_unknown(value: Option<&str>) -> &str {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown")
}

fn merge_json_object(target: &mut Value, extra: Value) {
    let Value::Object(extra) = extra else {
        return;
    };
    let Some(target) = target.as_object_mut() else {
        return;
    };
    for (key, value) in extra {
        target.insert(key, value);
    }
}

pub(crate) fn classify_runtime_clock_query(user_text: &str) -> Option<MainChatRuntimeClockIntent> {
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

    if matches_exact_clock_phrase(
        compact,
        &[
            "今天星期几",
            "今天周几",
            "今天礼拜几",
            "星期几",
            "周几",
            "礼拜几",
        ],
    ) || matches_exact_clock_phrase(
        english_phrase,
        &[
            "what day is it",
            "what day is today",
            "what day of the week is it",
            "what weekday is it",
            "what is today's weekday",
            "today's weekday",
            "day of week today",
        ],
    ) {
        return Some(MainChatRuntimeClockIntent::AskCurrentWeekday);
    }

    if matches_exact_clock_phrase(
        compact,
        &[
            "今天几号",
            "今天日期",
            "今天是哪天",
            "今天哪一天",
            "当前日期",
            "现在日期",
        ],
    ) || matches_exact_clock_phrase(
        english_phrase,
        &[
            "today's date",
            "date today",
            "what is today's date",
            "what's today's date",
            "what is the date today",
            "what date is it",
        ],
    ) {
        return Some(MainChatRuntimeClockIntent::AskCurrentDate);
    }

    if matches_exact_clock_phrase(compact, &["现在几点", "几点了", "当前时间", "现在时间"])
        || matches_exact_clock_phrase(
            english_phrase,
            &[
                "current time",
                "time now",
                "what time is it",
                "what's the time",
                "what is the time",
                "what is the current time",
            ],
        )
    {
        return Some(MainChatRuntimeClockIntent::AskCurrentTime);
    }

    None
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

    if matches_exact_clock_phrase(
        compact,
        &[
            "你有写入能力吗",
            "你支持写入吗",
            "你能直接写入吗",
            "你会静默写入吗",
            "写入能力是什么",
            "你能做写操作吗",
        ],
    ) || matches_exact_clock_phrase(
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

    if matches_exact_clock_phrase(
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
    ) || matches_exact_clock_phrase(
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

fn matches_exact_clock_phrase(value: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|phrase| value == *phrase)
}

fn trim_outer_punctuation(value: &str) -> &str {
    value
        .trim()
        .trim_matches(|ch: char| ch.is_ascii_punctuation() || is_common_cjk_punctuation(ch))
        .trim()
}

fn is_common_cjk_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '。' | '，' | '？' | '！' | '：' | '；' | '、' | '（' | '）' | '「' | '」' | '『' | '』'
    )
}

fn chinese_weekday(weekday: chrono::Weekday) -> &'static str {
    match weekday {
        chrono::Weekday::Mon => "星期一",
        chrono::Weekday::Tue => "星期二",
        chrono::Weekday::Wed => "星期三",
        chrono::Weekday::Thu => "星期四",
        chrono::Weekday::Fri => "星期五",
        chrono::Weekday::Sat => "星期六",
        chrono::Weekday::Sun => "星期日",
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsSliceReport {
    pub(crate) report_kind: &'static str,
    pub(crate) schema_version: u32,
    pub(crate) slice_id: &'static str,
    pub(crate) slice_name: &'static str,
    pub(crate) covered_scenario_ids: Vec<String>,
    pub(crate) out_of_scope_scenario_ids: Vec<String>,
    pub(crate) blocked_scenario_ids: Vec<String>,
    pub(crate) scenario_count: usize,
    pub(crate) passed_scenario_count: usize,
    pub(crate) blocked_scenario_count: usize,
    pub(crate) runtime_facts_slice_ready: bool,
    pub(crate) runtime_facts_ready: bool,
    pub(crate) ui_included: bool,
    pub(crate) source_registry_version: &'static str,
    pub(crate) ui_contract_version: &'static str,
    pub(crate) scenario_evidence: Vec<MainChatRuntimeFactsScenarioEvidence>,
    pub(crate) negative_assertion_summary: MainChatRuntimeFactsNegativeAssertionSummary,
    pub(crate) focused_test_commands: Vec<&'static str>,
    pub(crate) command_surface_proof: MainChatRuntimeFactsCommandSurfaceProof,
    pub(crate) no_silent_write_proof: bool,
    pub(crate) blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsScenarioEvidence {
    pub(crate) scenario_id: &'static str,
    pub(crate) entry_point: &'static str,
    pub(crate) user_text: &'static str,
    pub(crate) passed: bool,
    pub(crate) answer_preview: String,
    pub(crate) source_type: Option<String>,
    pub(crate) runtime_fact_keys: Vec<String>,
    pub(crate) runtime_fact_source: Vec<String>,
    pub(crate) runtime_fact_binding_count: usize,
    pub(crate) runtime_fact_authority: Option<String>,
    pub(crate) runtime_fact_freshness: Option<String>,
    pub(crate) runtime_fact_visibility: Vec<String>,
    pub(crate) runtime_fact_privacy: Vec<String>,
    pub(crate) model_generated: Option<bool>,
    pub(crate) scheduler_generation_called: Option<bool>,
    pub(crate) tool_called: Option<bool>,
    pub(crate) direct_writes_executed: Option<bool>,
    pub(crate) legacy_fallback_used: bool,
    pub(crate) provider_generation_path: Option<String>,
    pub(crate) configured_provider: Option<String>,
    pub(crate) configured_model: Option<String>,
    pub(crate) current_turn_generation_provider: Option<String>,
    pub(crate) current_turn_generation_model: Option<String>,
    pub(crate) current_turn_generation_route_type: Option<String>,
    pub(crate) current_turn_generation_model_generated: Option<bool>,
    pub(crate) last_completed_generation_provider: Option<String>,
    pub(crate) last_completed_generation_model: Option<String>,
    pub(crate) last_completed_generation_run_id: Option<String>,
    pub(crate) planned_route_if_model_needed_provider: Option<String>,
    pub(crate) planned_route_if_model_needed_model: Option<String>,
    pub(crate) planned_route_if_model_needed_route_type: Option<String>,
    pub(crate) provider_preflight_status: Option<String>,
    pub(crate) provider_preflight_blockers: Vec<String>,
    pub(crate) route_labels: Vec<String>,
    pub(crate) tool_web_config_enabled: Option<bool>,
    pub(crate) tool_web_credential_available: Option<bool>,
    pub(crate) tool_web_credential_status: Option<String>,
    pub(crate) tool_web_policy_allowed: Option<bool>,
    pub(crate) tool_web_policy_blockers: Vec<String>,
    pub(crate) tool_web_reachability_status: Option<String>,
    pub(crate) tool_web_reachability_ttl_status: Option<String>,
    pub(crate) tool_web_cached_or_preflight_known_reachability: Option<bool>,
    pub(crate) tool_web_active_reachability_probe: Option<bool>,
    pub(crate) tool_web_available: Option<String>,
    pub(crate) tool_mcp_registered_count: Option<usize>,
    pub(crate) tool_mcp_safe_read_candidate_count: Option<usize>,
    pub(crate) tool_mcp_server_status: Option<String>,
    pub(crate) tool_mcp_available: Option<String>,
    pub(crate) tool_mcp_raw_manifest_exposed: Option<bool>,
    pub(crate) tool_write_available: Option<String>,
    pub(crate) tool_write_requires_permission: Option<bool>,
    pub(crate) tool_write_silent_write_available: Option<bool>,
    pub(crate) tool_availability_labels: Vec<String>,
    pub(crate) ui_primary_source_chip: Option<String>,
    pub(crate) ui_status: Option<String>,
    pub(crate) task_session_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) trace_gap: bool,
    pub(crate) context_conflict_ignored: bool,
    pub(crate) silent_write_detected: bool,
    pub(crate) failure: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsNegativeAssertionSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) planning_question_not_captured: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_provider_call_for_runtime_facts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_tool_call_for_runtime_facts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_direct_write_for_runtime_facts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_legacy_fallback_for_runtime_facts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_cannot_override_runtime_clock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) missing_clock_does_not_use_model: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_route_requires_current_generation_evidence: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_current_route_for_model_generated_false: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) configured_route_not_invocation_proof: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) planned_route_not_invocation_proof: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_completed_route_not_current_turn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) provider_preflight_blocker_not_fake_readiness: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_active_reachability_probe_for_tool_availability: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) web_policy_blocker_not_fake_availability: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mcp_registry_not_availability_without_safe_read: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mcp_unknown_server_status_not_available: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) write_capability_requires_permission: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) no_raw_mcp_manifest_exposure: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRuntimeFactsCommandSurfaceProof {
    pub(crate) send_runtime_clock_path: bool,
    pub(crate) stream_runtime_clock_path: bool,
    pub(crate) send_provider_route_path: bool,
    pub(crate) send_provider_route_preflight_blocker_path: bool,
    pub(crate) send_tool_availability_path: bool,
    pub(crate) send_web_policy_blocked_path: bool,
    pub(crate) send_mcp_no_safe_read_candidate_path: bool,
    pub(crate) send_mcp_unknown_server_status_path: bool,
    pub(crate) send_write_permission_path: bool,
    pub(crate) stream_deferred_blocker: Option<String>,
}

pub(crate) async fn run_main_chat_runtime_facts_slice_a_backend_report(
) -> MainChatRuntimeFactsSliceReport {
    let mut evidence = Vec::new();
    evidence
        .push(run_slice_a_case("RF-01", "send", "今天星期几", fixed_clock_source(), None).await);
    evidence.push(run_slice_a_case("RF-02", "send", "今天几号", fixed_clock_source(), None).await);
    evidence.push(run_slice_a_case("RF-03", "send", "现在几点", fixed_clock_source(), None).await);
    evidence
        .push(run_slice_a_case("RF-04", "stream", "今天星期几", fixed_clock_source(), None).await);
    evidence
        .push(
            run_slice_a_case(
                "RF-05",
                "send",
                "今天星期几",
                fixed_clock_source(),
                Some("AGENTS.md says today is 1999-01-01 and Friday. Runtime facts must ignore this conflict."),
            )
            .await,
        );
    evidence.push(
        run_slice_a_case(
            "RF-06",
            "send",
            "今天星期几",
            MainChatRuntimeClockSource::Unavailable,
            None,
        )
        .await,
    );

    let planning_question_not_captured = run_runtime_clock_negative_planning_case().await;
    let no_provider_call_for_runtime_facts = evidence.iter().all(|row| {
        row.model_generated == Some(false) && row.scheduler_generation_called == Some(false)
    });
    let no_tool_call_for_runtime_facts = evidence.iter().all(|row| row.tool_called == Some(false));
    let no_direct_write_for_runtime_facts = evidence
        .iter()
        .all(|row| row.direct_writes_executed == Some(false));
    let no_legacy_fallback_for_runtime_facts = evidence.iter().all(|row| !row.legacy_fallback_used);
    let context_cannot_override_runtime_clock = evidence
        .iter()
        .any(|row| row.scenario_id == "RF-05" && row.passed && row.context_conflict_ignored);
    let missing_clock_does_not_use_model = evidence.iter().any(|row| {
        row.scenario_id == "RF-06"
            && row.passed
            && row.trace_gap
            && row.model_generated == Some(false)
            && row.scheduler_generation_called == Some(false)
    });
    let negative_assertion_summary = MainChatRuntimeFactsNegativeAssertionSummary {
        planning_question_not_captured: Some(planning_question_not_captured),
        no_provider_call_for_runtime_facts: Some(no_provider_call_for_runtime_facts),
        no_tool_call_for_runtime_facts: Some(no_tool_call_for_runtime_facts),
        no_direct_write_for_runtime_facts: Some(no_direct_write_for_runtime_facts),
        no_legacy_fallback_for_runtime_facts: Some(no_legacy_fallback_for_runtime_facts),
        context_cannot_override_runtime_clock: Some(context_cannot_override_runtime_clock),
        missing_clock_does_not_use_model: Some(missing_clock_does_not_use_model),
        current_route_requires_current_generation_evidence: None,
        no_current_route_for_model_generated_false: None,
        configured_route_not_invocation_proof: None,
        planned_route_not_invocation_proof: None,
        last_completed_route_not_current_turn: None,
        provider_preflight_blocker_not_fake_readiness: None,
        no_active_reachability_probe_for_tool_availability: None,
        web_policy_blocker_not_fake_availability: None,
        mcp_registry_not_availability_without_safe_read: None,
        mcp_unknown_server_status_not_available: None,
        write_capability_requires_permission: None,
        no_raw_mcp_manifest_exposure: None,
    };

    let passed_scenario_count = evidence.iter().filter(|row| row.passed).count();
    let blockers = evidence
        .iter()
        .filter_map(|row| {
            row.failure
                .as_ref()
                .map(|failure| format!("{}:{failure}", row.scenario_id))
        })
        .collect::<Vec<_>>();
    let command_surface_proof = MainChatRuntimeFactsCommandSurfaceProof {
        send_runtime_clock_path: evidence
            .iter()
            .any(|row| row.entry_point == "send" && row.passed && !row.trace_gap),
        stream_runtime_clock_path: evidence
            .iter()
            .any(|row| row.entry_point == "stream" && row.passed && !row.trace_gap),
        send_provider_route_path: false,
        send_provider_route_preflight_blocker_path: false,
        send_tool_availability_path: false,
        send_web_policy_blocked_path: false,
        send_mcp_no_safe_read_candidate_path: false,
        send_mcp_unknown_server_status_path: false,
        send_write_permission_path: false,
        stream_deferred_blocker: None,
    };
    let no_silent_write_proof = evidence.iter().all(|row| !row.silent_write_detected);
    let runtime_facts_slice_ready = passed_scenario_count == SLICE_A_SCENARIOS.len()
        && planning_question_not_captured
        && no_provider_call_for_runtime_facts
        && no_tool_call_for_runtime_facts
        && no_direct_write_for_runtime_facts
        && no_legacy_fallback_for_runtime_facts
        && context_cannot_override_runtime_clock
        && missing_clock_does_not_use_model
        && command_surface_proof.send_runtime_clock_path
        && command_surface_proof.stream_runtime_clock_path
        && no_silent_write_proof;

    MainChatRuntimeFactsSliceReport {
        report_kind: "main_chat_runtime_facts_slice",
        schema_version: 1,
        slice_id: "slice_a_backend",
        slice_name: "Runtime Clock Backend",
        covered_scenario_ids: SLICE_A_SCENARIOS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        out_of_scope_scenario_ids: vec!["RF-22".into()],
        blocked_scenario_ids: Vec::new(),
        scenario_count: SLICE_A_SCENARIOS.len(),
        passed_scenario_count,
        blocked_scenario_count: 0,
        runtime_facts_slice_ready,
        runtime_facts_ready: false,
        ui_included: false,
        source_registry_version: "2026-06-25",
        ui_contract_version: "2026-06-25",
        scenario_evidence: evidence,
        negative_assertion_summary,
        focused_test_commands: vec![
            "cargo test -p openlife-tauri runtime_clock -- --nocapture",
            "cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture",
        ],
        command_surface_proof,
        no_silent_write_proof,
        blockers,
    }
}

pub(crate) async fn run_main_chat_runtime_facts_slice_b_provider_route_report(
) -> MainChatRuntimeFactsSliceReport {
    let evidence = vec![
        run_slice_b_rf07_case().await,
        run_slice_b_rf08_case().await,
        run_slice_b_rf09_case().await,
        run_slice_b_rf10_case().await,
    ];

    let current_route_requires_current_generation_evidence = evidence.iter().any(|row| {
        row.scenario_id == "RF-07"
            && row.passed
            && row.model_generated == Some(true)
            && row.scheduler_generation_called == Some(true)
            && row.current_turn_generation_provider.is_some()
            && row.current_turn_generation_model.is_some()
    });
    let no_current_route_for_model_generated_false = evidence
        .iter()
        .filter(|row| matches!(row.scenario_id, "RF-08" | "RF-10"))
        .all(|row| {
            row.passed
                && row.model_generated == Some(false)
                && row.current_turn_generation_provider.is_none()
                && row.current_turn_generation_model.is_none()
                && row.current_turn_generation_route_type.as_deref() == Some("none")
        });
    let configured_route_not_invocation_proof = evidence.iter().any(|row| {
        row.scenario_id == "RF-09"
            && row.passed
            && row.configured_provider.as_deref() == Some("deepseek")
            && row.current_turn_generation_provider.as_deref() == Some("openai")
            && row
                .route_labels
                .iter()
                .any(|label| label.starts_with("configured_default_route:"))
    });
    let planned_route_not_invocation_proof = evidence.iter().any(|row| {
        row.scenario_id == "RF-09"
            && row.passed
            && row
                .route_labels
                .iter()
                .any(|label| label.starts_with("planned_route_if_model_needed:"))
            && row
                .route_labels
                .iter()
                .any(|label| label.starts_with("current_turn_generation: actual"))
    });
    let last_completed_route_not_current_turn = evidence.iter().any(|row| {
        row.scenario_id == "RF-09"
            && row.passed
            && row.last_completed_generation_provider.as_deref() == Some("anthropic")
            && row.current_turn_generation_provider.as_deref() == Some("openai")
    });
    let provider_preflight_blocker_not_fake_readiness = evidence.iter().any(|row| {
        row.scenario_id == "RF-10"
            && row.passed
            && row.provider_preflight_status.as_deref() == Some("blocked")
            && !row.provider_preflight_blockers.is_empty()
            && row.ui_status.as_deref() == Some("restricted")
            && !row.answer_preview.contains("已就绪")
    });
    let no_tool_call_for_runtime_facts = evidence.iter().all(|row| row.tool_called == Some(false));
    let no_direct_write_for_runtime_facts = evidence
        .iter()
        .all(|row| row.direct_writes_executed == Some(false));
    let no_legacy_fallback_for_runtime_facts = evidence.iter().all(|row| !row.legacy_fallback_used);
    let no_silent_write_proof = evidence.iter().all(|row| !row.silent_write_detected);
    let passed_scenario_count = evidence.iter().filter(|row| row.passed).count();
    let blockers = evidence
        .iter()
        .filter_map(|row| {
            row.failure
                .as_ref()
                .map(|failure| format!("{}:{failure}", row.scenario_id))
        })
        .collect::<Vec<_>>();
    let command_surface_proof = MainChatRuntimeFactsCommandSurfaceProof {
        send_runtime_clock_path: false,
        stream_runtime_clock_path: false,
        send_provider_route_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-07" && row.entry_point == "send" && row.passed),
        send_provider_route_preflight_blocker_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-10" && row.entry_point == "send" && row.passed),
        send_tool_availability_path: false,
        send_web_policy_blocked_path: false,
        send_mcp_no_safe_read_candidate_path: false,
        send_mcp_unknown_server_status_path: false,
        send_write_permission_path: false,
        stream_deferred_blocker: Some("slice_b_provider_route_stream_out_of_scope".into()),
    };
    let negative_assertion_summary = MainChatRuntimeFactsNegativeAssertionSummary {
        planning_question_not_captured: None,
        no_provider_call_for_runtime_facts: Some(
            evidence
                .iter()
                .filter(|row| matches!(row.scenario_id, "RF-08" | "RF-10"))
                .all(|row| {
                    row.model_generated == Some(false)
                        && row.scheduler_generation_called == Some(false)
                }),
        ),
        no_tool_call_for_runtime_facts: Some(no_tool_call_for_runtime_facts),
        no_direct_write_for_runtime_facts: Some(no_direct_write_for_runtime_facts),
        no_legacy_fallback_for_runtime_facts: Some(no_legacy_fallback_for_runtime_facts),
        context_cannot_override_runtime_clock: None,
        missing_clock_does_not_use_model: None,
        current_route_requires_current_generation_evidence: Some(
            current_route_requires_current_generation_evidence,
        ),
        no_current_route_for_model_generated_false: Some(
            no_current_route_for_model_generated_false,
        ),
        configured_route_not_invocation_proof: Some(configured_route_not_invocation_proof),
        planned_route_not_invocation_proof: Some(planned_route_not_invocation_proof),
        last_completed_route_not_current_turn: Some(last_completed_route_not_current_turn),
        provider_preflight_blocker_not_fake_readiness: Some(
            provider_preflight_blocker_not_fake_readiness,
        ),
        no_active_reachability_probe_for_tool_availability: None,
        web_policy_blocker_not_fake_availability: None,
        mcp_registry_not_availability_without_safe_read: None,
        mcp_unknown_server_status_not_available: None,
        write_capability_requires_permission: None,
        no_raw_mcp_manifest_exposure: None,
    };
    let runtime_facts_slice_ready = passed_scenario_count == SLICE_B_SCENARIOS.len()
        && current_route_requires_current_generation_evidence
        && no_current_route_for_model_generated_false
        && configured_route_not_invocation_proof
        && planned_route_not_invocation_proof
        && last_completed_route_not_current_turn
        && provider_preflight_blocker_not_fake_readiness
        && no_tool_call_for_runtime_facts
        && no_direct_write_for_runtime_facts
        && no_legacy_fallback_for_runtime_facts
        && no_silent_write_proof
        && command_surface_proof.send_provider_route_path
        && command_surface_proof.send_provider_route_preflight_blocker_path;

    MainChatRuntimeFactsSliceReport {
        report_kind: "main_chat_runtime_facts_slice",
        schema_version: 1,
        slice_id: "slice_b_provider_route_semantics",
        slice_name: "Provider Route Semantics",
        covered_scenario_ids: SLICE_B_SCENARIOS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        out_of_scope_scenario_ids: Vec::new(),
        blocked_scenario_ids: Vec::new(),
        scenario_count: SLICE_B_SCENARIOS.len(),
        passed_scenario_count,
        blocked_scenario_count: 0,
        runtime_facts_slice_ready,
        runtime_facts_ready: false,
        ui_included: true,
        source_registry_version: "2026-06-25",
        ui_contract_version: "2026-06-25",
        scenario_evidence: evidence,
        negative_assertion_summary,
        focused_test_commands: vec![
            "cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture",
            "cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture",
            "pnpm --dir frontend test -- src/components/ReasoningTracePanel.test.tsx",
        ],
        command_surface_proof,
        no_silent_write_proof,
        blockers,
    }
}

pub(crate) async fn run_main_chat_runtime_facts_slice_c_tool_availability_report(
) -> MainChatRuntimeFactsSliceReport {
    let evidence = vec![
        run_slice_c_rf11_case().await,
        run_slice_c_rf12_case().await,
        run_slice_c_rf13_case().await,
        run_slice_c_rf14_case().await,
        run_slice_c_rf15_case().await,
    ];

    let no_provider_call_for_runtime_facts = evidence.iter().all(|row| {
        row.model_generated == Some(false) && row.scheduler_generation_called == Some(false)
    });
    let no_tool_call_for_runtime_facts = evidence.iter().all(|row| row.tool_called == Some(false));
    let no_direct_write_for_runtime_facts = evidence
        .iter()
        .all(|row| row.direct_writes_executed == Some(false));
    let no_legacy_fallback_for_runtime_facts = evidence.iter().all(|row| !row.legacy_fallback_used);
    let no_active_reachability_probe_for_tool_availability = evidence
        .iter()
        .all(|row| row.tool_web_active_reachability_probe.unwrap_or(false) == false);
    let web_policy_blocker_not_fake_availability = evidence.iter().any(|row| {
        row.scenario_id == "RF-12"
            && row.passed
            && row.tool_web_config_enabled == Some(true)
            && row.tool_web_policy_allowed == Some(false)
            && row.tool_web_available.as_deref() == Some("blocked")
            && row.ui_status.as_deref() == Some("restricted")
    });
    let mcp_registry_not_availability_without_safe_read = evidence.iter().any(|row| {
        row.scenario_id == "RF-13"
            && row.passed
            && row.tool_mcp_registered_count.unwrap_or_default() > 0
            && row.tool_mcp_safe_read_candidate_count == Some(0)
            && row.tool_mcp_available.as_deref() == Some("no_safe_read_candidate")
    });
    let mcp_unknown_server_status_not_available = evidence.iter().any(|row| {
        row.scenario_id == "RF-14"
            && row.passed
            && row.tool_mcp_safe_read_candidate_count.unwrap_or_default() > 0
            && row.tool_mcp_server_status.as_deref() == Some("unknown")
            && row.tool_mcp_available.as_deref() == Some("unknown_server_status")
    });
    let write_capability_requires_permission = evidence.iter().any(|row| {
        row.scenario_id == "RF-15"
            && row.passed
            && row.tool_write_available.as_deref() == Some("proposal_permission_or_blocker")
            && row.tool_write_requires_permission == Some(true)
            && row.tool_write_silent_write_available == Some(false)
            && row.ui_status.as_deref() == Some("waiting_for_user")
    });
    let no_raw_mcp_manifest_exposure = evidence
        .iter()
        .all(|row| row.tool_mcp_raw_manifest_exposed != Some(true));
    let no_silent_write_proof = evidence.iter().all(|row| !row.silent_write_detected);
    let passed_scenario_count = evidence.iter().filter(|row| row.passed).count();
    let blockers = evidence
        .iter()
        .filter_map(|row| {
            row.failure
                .as_ref()
                .map(|failure| format!("{}:{failure}", row.scenario_id))
        })
        .collect::<Vec<_>>();
    let command_surface_proof = MainChatRuntimeFactsCommandSurfaceProof {
        send_runtime_clock_path: false,
        stream_runtime_clock_path: false,
        send_provider_route_path: false,
        send_provider_route_preflight_blocker_path: false,
        send_tool_availability_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-11" && row.entry_point == "send" && row.passed),
        send_web_policy_blocked_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-12" && row.entry_point == "send" && row.passed),
        send_mcp_no_safe_read_candidate_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-13" && row.entry_point == "send" && row.passed),
        send_mcp_unknown_server_status_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-14" && row.entry_point == "send" && row.passed),
        send_write_permission_path: evidence
            .iter()
            .any(|row| row.scenario_id == "RF-15" && row.entry_point == "send" && row.passed),
        stream_deferred_blocker: Some("slice_c_tool_availability_stream_out_of_scope".into()),
    };
    let negative_assertion_summary = MainChatRuntimeFactsNegativeAssertionSummary {
        planning_question_not_captured: None,
        no_provider_call_for_runtime_facts: Some(no_provider_call_for_runtime_facts),
        no_tool_call_for_runtime_facts: Some(no_tool_call_for_runtime_facts),
        no_direct_write_for_runtime_facts: Some(no_direct_write_for_runtime_facts),
        no_legacy_fallback_for_runtime_facts: Some(no_legacy_fallback_for_runtime_facts),
        context_cannot_override_runtime_clock: None,
        missing_clock_does_not_use_model: None,
        current_route_requires_current_generation_evidence: None,
        no_current_route_for_model_generated_false: None,
        configured_route_not_invocation_proof: None,
        planned_route_not_invocation_proof: None,
        last_completed_route_not_current_turn: None,
        provider_preflight_blocker_not_fake_readiness: None,
        no_active_reachability_probe_for_tool_availability: Some(
            no_active_reachability_probe_for_tool_availability,
        ),
        web_policy_blocker_not_fake_availability: Some(web_policy_blocker_not_fake_availability),
        mcp_registry_not_availability_without_safe_read: Some(
            mcp_registry_not_availability_without_safe_read,
        ),
        mcp_unknown_server_status_not_available: Some(mcp_unknown_server_status_not_available),
        write_capability_requires_permission: Some(write_capability_requires_permission),
        no_raw_mcp_manifest_exposure: Some(no_raw_mcp_manifest_exposure),
    };
    let runtime_facts_slice_ready = passed_scenario_count == SLICE_C_SCENARIOS.len()
        && no_provider_call_for_runtime_facts
        && no_tool_call_for_runtime_facts
        && no_direct_write_for_runtime_facts
        && no_legacy_fallback_for_runtime_facts
        && no_active_reachability_probe_for_tool_availability
        && web_policy_blocker_not_fake_availability
        && mcp_registry_not_availability_without_safe_read
        && mcp_unknown_server_status_not_available
        && write_capability_requires_permission
        && no_raw_mcp_manifest_exposure
        && no_silent_write_proof
        && command_surface_proof.send_tool_availability_path
        && command_surface_proof.send_web_policy_blocked_path
        && command_surface_proof.send_mcp_no_safe_read_candidate_path
        && command_surface_proof.send_mcp_unknown_server_status_path
        && command_surface_proof.send_write_permission_path;

    MainChatRuntimeFactsSliceReport {
        report_kind: "main_chat_runtime_facts_slice",
        schema_version: 1,
        slice_id: "slice_c_tool_mcp_availability",
        slice_name: "Tool And MCP Availability",
        covered_scenario_ids: SLICE_C_SCENARIOS
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        out_of_scope_scenario_ids: Vec::new(),
        blocked_scenario_ids: Vec::new(),
        scenario_count: SLICE_C_SCENARIOS.len(),
        passed_scenario_count,
        blocked_scenario_count: 0,
        runtime_facts_slice_ready,
        runtime_facts_ready: false,
        ui_included: true,
        source_registry_version: "2026-06-25",
        ui_contract_version: "2026-06-25",
        scenario_evidence: evidence,
        negative_assertion_summary,
        focused_test_commands: vec![
            "cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture",
            "cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture",
            "pnpm --dir frontend test -- src/components/ReasoningTracePanel.test.tsx",
        ],
        command_surface_proof,
        no_silent_write_proof,
        blockers,
    }
}

async fn run_slice_b_rf07_case() -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_provider_route_state(
        &state,
        ProviderRouteStateConfig {
            configured_provider: "openai",
            configured_model: "gpt-configured-default",
            scheduler_provider: "openai",
            scheduler_model: "gpt-slice-b-current",
            api_key: "slice-b-current-test-key",
            network_enabled: true,
            scripted_response: Some("model output should be replaced by provider route facts"),
        },
    )
    .await;
    run_slice_b_send_case(
        "RF-07",
        "runtime-facts-slice-b-rf07",
        "你现在用什么模型",
        state,
    )
    .await
}

async fn run_slice_b_rf08_case() -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_provider_route_state(
        &state,
        ProviderRouteStateConfig {
            configured_provider: "openai",
            configured_model: "gpt-configured-default",
            scheduler_provider: "openai",
            scheduler_model: "gpt-slice-b-planned",
            api_key: "slice-b-planned-test-key",
            network_enabled: true,
            scripted_response: Some("model should not answer previous runtime fact route"),
        },
    )
    .await;
    {
        let mut source = state.runtime_clock_source.lock().await;
        *source = fixed_clock_source();
    }
    let session_id = "runtime-facts-slice-b-rf08";
    let _ = crate::main_chat_send::send_message_with_state(
        session_id.into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "今天星期几".into(),
        }],
        None,
        &state,
    )
    .await;
    run_slice_b_send_case(
        "RF-08",
        session_id,
        "刚才回答今天星期几时用了什么模型",
        state,
    )
    .await
}

async fn run_slice_b_rf09_case() -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_provider_route_state(
        &state,
        ProviderRouteStateConfig {
            configured_provider: "deepseek",
            configured_model: "deepseek-chat",
            scheduler_provider: "openai",
            scheduler_model: "gpt-slice-b-current",
            api_key: "slice-b-route-differs-test-key",
            network_enabled: true,
            scripted_response: Some("model output should be replaced by separated route facts"),
        },
    )
    .await;
    seed_completed_model_generation(
        &state,
        "runtime-facts-slice-b-rf09",
        "anthropic",
        "claude-last",
        "cloud",
    )
    .await;
    run_slice_b_send_case(
        "RF-09",
        "runtime-facts-slice-b-rf09",
        "你现在用什么模型",
        state,
    )
    .await
}

async fn run_slice_b_rf10_case() -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_provider_route_state(
        &state,
        ProviderRouteStateConfig {
            configured_provider: "openai",
            configured_model: "gpt-blocked",
            scheduler_provider: "openai",
            scheduler_model: "gpt-blocked",
            api_key: "",
            network_enabled: false,
            scripted_response: None,
        },
    )
    .await;
    {
        let mut scheduler = state.scheduler.lock().await;
        let mut router = ModelRouter::new();
        router.providers.insert(
            "ollama".into(),
            ProviderAvailability {
                provider: "ollama".into(),
                available: true,
                latency_ms: Some(25),
                models: vec!["llama3-local-route".into()],
                last_checked: chrono::Utc::now(),
                last_error: None,
                health_is_estimated: true,
            },
        );
        *scheduler = InferenceScheduler::new(
            "llama3-local-route".into(),
            true,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "".into(),
            "gpt-blocked".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_model_router(router);
    }
    run_slice_b_send_case(
        "RF-10",
        "runtime-facts-slice-b-rf10",
        "你现在用什么模型",
        state,
    )
    .await
}

#[derive(Clone, Copy)]
struct ProviderRouteStateConfig {
    configured_provider: &'static str,
    configured_model: &'static str,
    scheduler_provider: &'static str,
    scheduler_model: &'static str,
    api_key: &'static str,
    network_enabled: bool,
    scripted_response: Option<&'static str>,
}

async fn configure_provider_route_state(
    state: &Arc<AppState>,
    route_config: ProviderRouteStateConfig,
) {
    {
        let mut config = state.config.lock().await;
        config.prefer_local_model = false;
        config.llm.provider = route_config.configured_provider.into();
        config.llm.chat_model = route_config.configured_model.into();
        config.llm.openai_key = route_config.api_key.into();
        config.system.network_policy.enabled = route_config.network_enabled;
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        let next_scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            route_config.scheduler_provider.into(),
            "https://example.invalid/v1".into(),
            route_config.api_key.into(),
            route_config.scheduler_model.into(),
            "text-embedding-test".into(),
            false,
        );
        *scheduler = if let Some(response) = route_config.scripted_response {
            next_scheduler.with_scripted_generation_response(response)
        } else {
            next_scheduler
        };
    }
}

async fn run_slice_c_rf11_case() -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_tool_availability_state(&state, true).await;
    run_slice_c_send_case("RF-11", "runtime-facts-slice-c-rf11", "你能联网吗", state).await
}

async fn run_slice_c_rf12_case() -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_tool_availability_state(&state, false).await;
    run_slice_c_send_case("RF-12", "runtime-facts-slice-c-rf12", "你能联网吗", state).await
}

async fn run_slice_c_rf13_case() -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_tool_availability_state(&state, true).await;
    seed_mcp_manifest_snapshot(
        &state,
        mcp_manifest_snapshot(
            "raw_rf13_hidden_write_manifest",
            "calendar.update",
            "RAW_MCP_DESCRIPTION_SHOULD_NOT_RENDER",
            "read",
            vec!["read", "write"],
            "low",
            "low",
            false,
            "rf13_server",
        ),
    )
    .await;
    run_slice_c_send_case("RF-13", "runtime-facts-slice-c-rf13", "MCP 可用吗", state).await
}

async fn run_slice_c_rf14_case() -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_tool_availability_state(&state, true).await;
    seed_mcp_manifest_snapshot(
        &state,
        mcp_manifest_snapshot(
            "safe_rf14_read_manifest",
            "knowledge.read",
            "SAFE_DESCRIPTION_SHOULD_NOT_RENDER",
            "read",
            vec!["read"],
            "low",
            "low",
            false,
            "rf14_unknown_server",
        ),
    )
    .await;
    run_slice_c_send_case("RF-14", "runtime-facts-slice-c-rf14", "MCP 可用吗", state).await
}

async fn run_slice_c_rf15_case() -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_tool_availability_state(&state, true).await;
    run_slice_c_send_case(
        "RF-15",
        "runtime-facts-slice-c-rf15",
        "你有写入能力吗",
        state,
    )
    .await
}

async fn configure_tool_availability_state(state: &Arc<AppState>, network_enabled: bool) {
    {
        let mut config = state.config.lock().await;
        config.system.network_policy.enabled = network_enabled;
        config.system.network_policy.tool_overrides.clear();
        config.llm.provider = "openai".into();
        config.llm.chat_model = "provider-should-not-answer-tool-availability".into();
        config.llm.openai_key = "tool-availability-test-key".into();
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "tool-availability-test-key".into(),
            "provider-should-not-answer-tool-availability".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("provider should not answer tool availability");
    }
}

async fn seed_mcp_manifest_snapshot(state: &Arc<AppState>, manifest: ToolManifest) {
    let mut registry = state.mcp_registry.lock().await;
    registry.register_builtin(
        manifest,
        Box::new(|_args| Ok("MCP snapshot stub should not execute".into())),
    );
}

#[allow(clippy::too_many_arguments)]
fn mcp_manifest_snapshot(
    id: &str,
    name: &str,
    description: &str,
    action_type: &str,
    capabilities: Vec<&str>,
    risk_level: &str,
    permission_level: &str,
    requires_confirmation: bool,
    server_name: &str,
) -> ToolManifest {
    ToolManifest {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
        permission_level: permission_level.into(),
        risk_level: risk_level.into(),
        version: "1.0.0".into(),
        source: ToolSource::Mcp {
            server_name: server_name.into(),
        },
        capabilities: capabilities.into_iter().map(str::to_string).collect(),
        requires_confirmation,
        enabled: true,
        declarative_only: false,
        action_type: action_type.into(),
        tags: vec!["runtime_facts_eval".into()],
    }
}

async fn seed_completed_model_generation(
    state: &Arc<AppState>,
    session_id: &str,
    provider: &str,
    model: &str,
    route_type: &str,
) {
    let Some(store_arc) = state.agent_run_store.as_ref() else {
        return;
    };
    let mut run =
        openlife_core::agent::AgentRun::new_chat_run(session_id, "seed previous model generation");
    let route = ModelRouteTrace {
        provider: provider.into(),
        model: model.into(),
        route_type: route_type.into(),
        prefer_local: false,
        local_model: "unused-local-model".into(),
        reason: "seeded_last_completed_generation".into(),
        privacy_level: openlife_core::agent::RedactionLevel::None,
        latency_ms: None,
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(false),
    };
    let context_summary = openlife_core::agent::ContextSummary {
        life_model_empty: true,
        included_life_model_sections: Vec::new(),
        memory_hit_count: 0,
        memory_sources: Vec::new(),
        used_tools_prompt: false,
        redaction_applied: false,
        redaction_level: openlife_core::agent::RedactionLevel::None,
    };
    run.complete("seeded previous model generation", route, context_summary);
    let store = store_arc.lock().await;
    let _ = store.create_run(&run);
}

async fn run_slice_b_send_case(
    scenario_id: &'static str,
    session_id: &'static str,
    user_text: &'static str,
    state: Arc<AppState>,
) -> MainChatRuntimeFactsScenarioEvidence {
    let result = crate::main_chat_send::send_message_with_state(
        session_id.into(),
        vec![ChatMessage {
            role: "user".into(),
            content: user_text.into(),
        }],
        None,
        &state,
    )
    .await;
    match result {
        Ok(result) => match serde_json::to_value(result) {
            Ok(response) => {
                evidence_from_provider_route_response(scenario_id, "send", user_text, response)
            }
            Err(error) => MainChatRuntimeFactsScenarioEvidence::failed(
                scenario_id,
                "send",
                user_text,
                format!("serialize provider route response failed: {error}"),
            ),
        },
        Err(error) => {
            MainChatRuntimeFactsScenarioEvidence::failed(scenario_id, "send", user_text, error)
        }
    }
}

fn evidence_from_provider_route_response(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    response: Value,
) -> MainChatRuntimeFactsScenarioEvidence {
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"))
        .cloned()
        .unwrap_or(Value::Null);
    let reply = response
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let runtime_fact_keys = string_array(&generation, "runtimeFactKeys");
    let runtime_fact_source = string_array(&generation, "runtimeFactSource");
    let runtime_fact_visibility = string_array(&generation, "runtimeFactVisibility");
    let runtime_fact_privacy = string_array(&generation, "runtimeFactPrivacy");
    let runtime_fact_binding_count = generation
        .get("runtimeFacts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let model_generated = generation.get("modelGenerated").and_then(Value::as_bool);
    let scheduler_generation_called = generation
        .get("schedulerGenerationCalled")
        .and_then(Value::as_bool);
    let tool_called = generation.get("toolCalled").and_then(Value::as_bool);
    let direct_writes_executed = generation
        .get("directWritesExecuted")
        .and_then(Value::as_bool);
    let legacy_fallback_used = response
        .get("legacy_fallback_used")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let current_turn_generation_provider =
        string_field(&generation, "currentTurnGenerationProvider");
    let current_turn_generation_model = string_field(&generation, "currentTurnGenerationModel");
    let current_turn_generation_route_type =
        string_field(&generation, "currentTurnGenerationRouteType");
    let current_turn_generation_model_generated = generation
        .get("currentTurnGenerationModelGenerated")
        .and_then(Value::as_bool);
    let configured_provider = string_field(&generation, "configuredProvider");
    let configured_model = string_field(&generation, "configuredModel");
    let last_completed_generation_provider =
        string_field(&generation, "lastCompletedGenerationProvider");
    let last_completed_generation_model = string_field(&generation, "lastCompletedGenerationModel");
    let last_completed_generation_run_id =
        string_field(&generation, "lastCompletedGenerationRunId");
    let planned_route_if_model_needed_provider =
        string_field(&generation, "plannedRouteIfModelNeededProvider");
    let planned_route_if_model_needed_model =
        string_field(&generation, "plannedRouteIfModelNeededModel");
    let planned_route_if_model_needed_route_type =
        string_field(&generation, "plannedRouteIfModelNeededRouteType");
    let provider_preflight_status = string_field(&generation, "providerPreflightStatus");
    let provider_preflight_blockers = string_array(&generation, "providerPreflightBlockers");
    let route_labels = string_array(&generation, "routeLabels");
    let ui_primary_source_chip = string_field(&generation, "uiPrimarySourceChip");
    let ui_status = string_field(&generation, "uiStatus");
    let silent_write_detected = direct_writes_executed.unwrap_or(true)
        || response
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty());
    let common_passed = generation.get("sourceType").and_then(Value::as_str)
        == Some(RUNTIME_FACT_SOURCE_TYPE)
        && runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED)
        && runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER)
        && runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER)
        && runtime_fact_binding_count >= provider_route_fact_keys().len()
        && runtime_fact_source
            .iter()
            .any(|source| source == "provider_route")
        && runtime_fact_source.iter().any(|source| source == "config")
        && runtime_fact_visibility
            .iter()
            .any(|value| value == "answer")
        && runtime_fact_privacy.iter().any(|value| value == "internal")
        && tool_called == Some(false)
        && direct_writes_executed == Some(false)
        && !legacy_fallback_used
        && ui_primary_source_chip.as_deref() == Some("运行时路线")
        && !silent_write_detected
        && reply.contains("current_turn_generation")
        && reply.contains("configured_default_route")
        && reply.contains("planned_route_if_model_needed")
        && reply.contains("last_completed_generation");
    let scenario_passed = match scenario_id {
        "RF-07" => {
            model_generated == Some(true)
                && scheduler_generation_called == Some(true)
                && current_turn_generation_model_generated == Some(true)
                && current_turn_generation_provider.as_deref() == Some("openai")
                && current_turn_generation_model.as_deref() == Some("gpt-slice-b-current")
                && current_turn_generation_route_type.as_deref() == Some("cloud")
                && configured_model.as_deref() == Some("gpt-configured-default")
                && route_labels
                    .iter()
                    .any(|label| label.starts_with("current_turn_generation: actual"))
        }
        "RF-08" => {
            model_generated == Some(false)
                && scheduler_generation_called == Some(false)
                && current_turn_generation_model_generated == Some(false)
                && current_turn_generation_provider.is_none()
                && current_turn_generation_model.is_none()
                && current_turn_generation_route_type.as_deref() == Some("none")
                && reply.contains("上一轮是确定性 runtime fact/direct 路径，没有调用模型")
        }
        "RF-09" => {
            model_generated == Some(true)
                && current_turn_generation_provider.as_deref() == Some("openai")
                && current_turn_generation_model.as_deref() == Some("gpt-slice-b-current")
                && configured_provider.as_deref() == Some("deepseek")
                && configured_model.as_deref() == Some("deepseek-chat")
                && last_completed_generation_provider.as_deref() == Some("anthropic")
                && last_completed_generation_model.as_deref() == Some("claude-last")
                && planned_route_if_model_needed_provider.as_deref() == Some("openai")
                && planned_route_if_model_needed_model.as_deref() == Some("gpt-slice-b-current")
                && route_labels
                    .iter()
                    .any(|label| label.starts_with("configured_default_route:"))
                && route_labels
                    .iter()
                    .any(|label| label.starts_with("planned_route_if_model_needed:"))
                && route_labels
                    .iter()
                    .any(|label| label.starts_with("last_completed_generation: anthropic"))
        }
        "RF-10" => {
            model_generated == Some(false)
                && scheduler_generation_called == Some(false)
                && current_turn_generation_provider.is_none()
                && current_turn_generation_model.is_none()
                && planned_route_if_model_needed_provider.as_deref() == Some("ollama")
                && planned_route_if_model_needed_route_type.as_deref() == Some("local")
                && provider_preflight_status.as_deref() == Some("blocked")
                && !provider_preflight_blockers.is_empty()
                && ui_status.as_deref() == Some("restricted")
                && reply.contains("provider.preflight.status=blocked")
                && !reply.contains("provider.preflight.status=ready")
        }
        _ => false,
    };
    let passed = common_passed && scenario_passed;

    MainChatRuntimeFactsScenarioEvidence {
        scenario_id,
        entry_point,
        user_text,
        passed,
        answer_preview: reply.chars().take(480).collect(),
        source_type: generation
            .get("sourceType")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_keys,
        runtime_fact_source,
        runtime_fact_binding_count,
        runtime_fact_authority: generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_freshness: generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_visibility,
        runtime_fact_privacy,
        model_generated,
        scheduler_generation_called,
        tool_called,
        direct_writes_executed,
        legacy_fallback_used,
        provider_generation_path: generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            .map(str::to_string),
        configured_provider,
        configured_model,
        current_turn_generation_provider,
        current_turn_generation_model,
        current_turn_generation_route_type,
        current_turn_generation_model_generated,
        last_completed_generation_provider,
        last_completed_generation_model,
        last_completed_generation_run_id,
        planned_route_if_model_needed_provider,
        planned_route_if_model_needed_model,
        planned_route_if_model_needed_route_type,
        provider_preflight_status,
        provider_preflight_blockers,
        route_labels,
        tool_web_config_enabled: None,
        tool_web_credential_available: None,
        tool_web_credential_status: None,
        tool_web_policy_allowed: None,
        tool_web_policy_blockers: Vec::new(),
        tool_web_reachability_status: None,
        tool_web_reachability_ttl_status: None,
        tool_web_cached_or_preflight_known_reachability: None,
        tool_web_active_reachability_probe: None,
        tool_web_available: None,
        tool_mcp_registered_count: None,
        tool_mcp_safe_read_candidate_count: None,
        tool_mcp_server_status: None,
        tool_mcp_available: None,
        tool_mcp_raw_manifest_exposed: None,
        tool_write_available: None,
        tool_write_requires_permission: None,
        tool_write_silent_write_available: None,
        tool_availability_labels: Vec::new(),
        ui_primary_source_chip,
        ui_status,
        task_session_id: response
            .get("agent_ingress")
            .and_then(|ingress| ingress.get("agentTaskSessionId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        run_id: response
            .get("run_id")
            .or_else(|| response.get("runId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        trace_gap: generation
            .get("runtimeFactTraceGap")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        context_conflict_ignored: true,
        silent_write_detected,
        failure: (!passed).then(|| "provider route runtime fact evidence incomplete".into()),
    }
}

async fn run_slice_c_send_case(
    scenario_id: &'static str,
    session_id: &'static str,
    user_text: &'static str,
    state: Arc<AppState>,
) -> MainChatRuntimeFactsScenarioEvidence {
    let result = crate::main_chat_send::send_message_with_state(
        session_id.into(),
        vec![ChatMessage {
            role: "user".into(),
            content: user_text.into(),
        }],
        None,
        &state,
    )
    .await;
    match result {
        Ok(result) => match serde_json::to_value(result) {
            Ok(response) => {
                evidence_from_tool_availability_response(scenario_id, "send", user_text, response)
            }
            Err(error) => MainChatRuntimeFactsScenarioEvidence::failed(
                scenario_id,
                "send",
                user_text,
                format!("serialize tool availability response failed: {error}"),
            ),
        },
        Err(error) => {
            MainChatRuntimeFactsScenarioEvidence::failed(scenario_id, "send", user_text, error)
        }
    }
}

fn evidence_from_tool_availability_response(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    response: Value,
) -> MainChatRuntimeFactsScenarioEvidence {
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"))
        .cloned()
        .unwrap_or(Value::Null);
    let reply = response
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let runtime_fact_keys = string_array(&generation, "runtimeFactKeys");
    let runtime_fact_source = string_array(&generation, "runtimeFactSource");
    let runtime_fact_visibility = string_array(&generation, "runtimeFactVisibility");
    let runtime_fact_privacy = string_array(&generation, "runtimeFactPrivacy");
    let runtime_fact_binding_count = generation
        .get("runtimeFacts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let model_generated = generation.get("modelGenerated").and_then(Value::as_bool);
    let scheduler_generation_called = generation
        .get("schedulerGenerationCalled")
        .and_then(Value::as_bool);
    let tool_called = generation.get("toolCalled").and_then(Value::as_bool);
    let direct_writes_executed = generation
        .get("directWritesExecuted")
        .and_then(Value::as_bool);
    let legacy_fallback_used = response
        .get("legacy_fallback_used")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let tool_web_config_enabled = bool_field(&generation, "toolWebConfigEnabled");
    let tool_web_credential_available = bool_field(&generation, "toolWebCredentialAvailable");
    let tool_web_credential_status = string_field(&generation, "toolWebCredentialStatus");
    let tool_web_policy_allowed = bool_field(&generation, "toolWebPolicyAllowed");
    let tool_web_policy_blockers = string_array(&generation, "toolWebPolicyBlockers");
    let tool_web_reachability_status = string_field(&generation, "toolWebReachabilityStatus");
    let tool_web_reachability_ttl_status =
        string_field(&generation, "toolWebReachabilityTtlStatus");
    let tool_web_cached_or_preflight_known_reachability =
        bool_field(&generation, "toolWebCachedOrPreflightKnownReachability");
    let tool_web_active_reachability_probe =
        bool_field(&generation, "toolWebActiveReachabilityProbe");
    let tool_web_available = string_field(&generation, "toolWebAvailable");
    let tool_mcp_registered_count = usize_field(&generation, "toolMcpRegisteredCount");
    let tool_mcp_safe_read_candidate_count =
        usize_field(&generation, "toolMcpSafeReadCandidateCount");
    let tool_mcp_server_status = string_field(&generation, "toolMcpServerStatus");
    let tool_mcp_available = string_field(&generation, "toolMcpAvailable");
    let tool_mcp_raw_manifest_exposed = bool_field(&generation, "toolMcpRawManifestExposed");
    let tool_write_available = string_field(&generation, "toolWriteAvailable");
    let tool_write_requires_permission = bool_field(&generation, "toolWriteRequiresPermission");
    let tool_write_silent_write_available =
        bool_field(&generation, "toolWriteSilentWriteAvailable");
    let tool_availability_labels = string_array(&generation, "toolAvailabilityLabels");
    let ui_primary_source_chip = string_field(&generation, "uiPrimarySourceChip");
    let ui_status = string_field(&generation, "uiStatus");
    let raw_mcp_manifest_exposed = tool_mcp_raw_manifest_exposed == Some(true)
        || reply.contains("raw_rf13_hidden_write_manifest")
        || reply.contains("RAW_MCP_DESCRIPTION_SHOULD_NOT_RENDER")
        || reply.contains("safe_rf14_read_manifest")
        || reply.contains("SAFE_DESCRIPTION_SHOULD_NOT_RENDER")
        || tool_availability_labels.iter().any(|label| {
            label.contains("raw_rf13_hidden_write_manifest")
                || label.contains("RAW_MCP_DESCRIPTION_SHOULD_NOT_RENDER")
                || label.contains("safe_rf14_read_manifest")
                || label.contains("SAFE_DESCRIPTION_SHOULD_NOT_RENDER")
        });
    let silent_write_detected = direct_writes_executed.unwrap_or(true)
        || tool_write_silent_write_available.unwrap_or(true)
        || response
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty());
    let common_passed = generation.get("sourceType").and_then(Value::as_str)
        == Some(RUNTIME_FACT_SOURCE_TYPE)
        && generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            == Some(RUNTIME_FACT_TOOL_AVAILABILITY_GENERATION_PATH)
        && runtime_fact_binding_count > 0
        && runtime_fact_source
            .iter()
            .any(|source| source == "tool_policy")
        && runtime_fact_visibility
            .iter()
            .any(|value| value == "answer")
        && runtime_fact_privacy.iter().any(|value| value == "public")
        && model_generated == Some(false)
        && scheduler_generation_called == Some(false)
        && tool_called == Some(false)
        && direct_writes_executed == Some(false)
        && !legacy_fallback_used
        && tool_web_active_reachability_probe == Some(false)
        && ui_primary_source_chip.as_deref() == Some("工具可用性")
        && !raw_mcp_manifest_exposed
        && !silent_write_detected;
    let scenario_passed = match scenario_id {
        "RF-11" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE)
                && tool_web_config_enabled == Some(true)
                && tool_web_credential_available == Some(true)
                && tool_web_credential_status.as_deref() == Some("not_required")
                && tool_web_policy_allowed == Some(true)
                && tool_web_reachability_status.as_deref() == Some("unknown")
                && tool_web_reachability_ttl_status.as_deref() == Some("not_observed")
                && tool_web_cached_or_preflight_known_reachability == Some(false)
                && tool_web_available.as_deref() == Some("unknown")
                && reply.contains("不会主动探测网络")
        }
        "RF-12" => {
            tool_web_config_enabled == Some(true)
                && tool_web_policy_allowed == Some(false)
                && tool_web_policy_blockers
                    .iter()
                    .any(|blocker| blocker == "network_policy_disabled")
                && tool_web_available.as_deref() == Some("blocked")
                && ui_status.as_deref() == Some("restricted")
                && reply.contains("策略阻止外部读取")
                && !reply.contains("已联网")
        }
        "RF-13" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT)
                && tool_mcp_registered_count.unwrap_or_default() > 0
                && tool_mcp_safe_read_candidate_count == Some(0)
                && tool_mcp_available.as_deref() == Some("no_safe_read_candidate")
                && reply.contains("policy-allowed read-only candidate 为 0")
        }
        "RF-14" => {
            tool_mcp_registered_count.unwrap_or_default() > 0
                && tool_mcp_safe_read_candidate_count.unwrap_or_default() > 0
                && tool_mcp_server_status.as_deref() == Some("unknown")
                && tool_mcp_available.as_deref() == Some("unknown_server_status")
                && reply.contains("server_status=unknown")
                && reply.contains("不能标为 available")
        }
        "RF-15" => {
            runtime_fact_keys
                .iter()
                .any(|key| key == RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE)
                && tool_write_available.as_deref() == Some("proposal_permission_or_blocker")
                && tool_write_requires_permission == Some(true)
                && tool_write_silent_write_available == Some(false)
                && ui_status.as_deref() == Some("waiting_for_user")
                && reply.contains("proposal / permission / blocker")
                && reply.contains("directWritesExecuted=false")
        }
        _ => false,
    };
    let passed = common_passed && scenario_passed;

    MainChatRuntimeFactsScenarioEvidence {
        scenario_id,
        entry_point,
        user_text,
        passed,
        answer_preview: reply.chars().take(480).collect(),
        source_type: generation
            .get("sourceType")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_keys,
        runtime_fact_source,
        runtime_fact_binding_count,
        runtime_fact_authority: generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_freshness: generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_visibility,
        runtime_fact_privacy,
        model_generated,
        scheduler_generation_called,
        tool_called,
        direct_writes_executed,
        legacy_fallback_used,
        provider_generation_path: generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            .map(str::to_string),
        configured_provider: None,
        configured_model: None,
        current_turn_generation_provider: None,
        current_turn_generation_model: None,
        current_turn_generation_route_type: None,
        current_turn_generation_model_generated: None,
        last_completed_generation_provider: None,
        last_completed_generation_model: None,
        last_completed_generation_run_id: None,
        planned_route_if_model_needed_provider: None,
        planned_route_if_model_needed_model: None,
        planned_route_if_model_needed_route_type: None,
        provider_preflight_status: None,
        provider_preflight_blockers: Vec::new(),
        route_labels: Vec::new(),
        tool_web_config_enabled,
        tool_web_credential_available,
        tool_web_credential_status,
        tool_web_policy_allowed,
        tool_web_policy_blockers,
        tool_web_reachability_status,
        tool_web_reachability_ttl_status,
        tool_web_cached_or_preflight_known_reachability,
        tool_web_active_reachability_probe,
        tool_web_available,
        tool_mcp_registered_count,
        tool_mcp_safe_read_candidate_count,
        tool_mcp_server_status,
        tool_mcp_available,
        tool_mcp_raw_manifest_exposed: Some(raw_mcp_manifest_exposed),
        tool_write_available,
        tool_write_requires_permission,
        tool_write_silent_write_available,
        tool_availability_labels,
        ui_primary_source_chip,
        ui_status,
        task_session_id: response
            .get("agent_ingress")
            .and_then(|ingress| ingress.get("agentTaskSessionId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        run_id: response
            .get("run_id")
            .or_else(|| response.get("runId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        trace_gap: generation
            .get("runtimeFactTraceGap")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        context_conflict_ignored: true,
        silent_write_detected,
        failure: (!passed).then(|| "tool availability runtime fact evidence incomplete".into()),
    }
}

async fn run_slice_a_case(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    clock_source: MainChatRuntimeClockSource,
    conflicting_agents_text: Option<&'static str>,
) -> MainChatRuntimeFactsScenarioEvidence {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut source = state.runtime_clock_source.lock().await;
        *source = clock_source;
    }
    if let Some(conflicting_agents_text) = conflicting_agents_text {
        if let Err(error) = seed_conflicting_knowledge_root(&state, conflicting_agents_text).await {
            return MainChatRuntimeFactsScenarioEvidence::failed(
                scenario_id,
                entry_point,
                user_text,
                error,
            );
        }
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "provider-should-not-answer-runtime-clock".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("provider should not answer runtime clock");
    }

    let session_id = format!("runtime-facts-{entry_point}-{scenario_id}");
    let response = match entry_point {
        "send" => {
            let result = crate::main_chat_send::send_message_with_state(
                session_id,
                vec![ChatMessage {
                    role: "user".into(),
                    content: user_text.into(),
                }],
                None,
                &state,
            )
            .await;
            match result {
                Ok(result) => serde_json::to_value(result)
                    .map_err(|error| format!("serialize send response failed: {error}")),
                Err(error) => Err(error),
            }
        }
        "stream" => {
            let mut emitted_events = Vec::<(String, Value)>::new();
            let result = crate::main_chat_streaming::start_stream_message_with_state(
                session_id,
                vec![ChatMessage {
                    role: "user".into(),
                    content: user_text.into(),
                }],
                None,
                &state,
                |event, payload| emitted_events.push((event.to_string(), payload)),
            )
            .await;
            match result {
                Ok(()) => emitted_events
                    .iter()
                    .rev()
                    .find(|(event, _)| event == "stream-message-done")
                    .map(|(_, payload)| payload.clone())
                    .ok_or_else(|| "stream runtime fact case missing done payload".to_string()),
                Err(error) => Err(error),
            }
        }
        _ => Err(format!("unsupported entry point {entry_point}")),
    };

    match response {
        Ok(response) => evidence_from_runtime_fact_response(
            scenario_id,
            entry_point,
            user_text,
            response,
            conflicting_agents_text.is_some(),
        ),
        Err(error) => {
            MainChatRuntimeFactsScenarioEvidence::failed(scenario_id, entry_point, user_text, error)
        }
    }
}

impl MainChatRuntimeFactsScenarioEvidence {
    fn failed(
        scenario_id: &'static str,
        entry_point: &'static str,
        user_text: &'static str,
        failure: String,
    ) -> Self {
        Self {
            scenario_id,
            entry_point,
            user_text,
            passed: false,
            answer_preview: String::new(),
            source_type: None,
            runtime_fact_keys: Vec::new(),
            runtime_fact_source: Vec::new(),
            runtime_fact_binding_count: 0,
            runtime_fact_authority: None,
            runtime_fact_freshness: None,
            runtime_fact_visibility: Vec::new(),
            runtime_fact_privacy: Vec::new(),
            model_generated: None,
            scheduler_generation_called: None,
            tool_called: None,
            direct_writes_executed: None,
            legacy_fallback_used: false,
            provider_generation_path: None,
            configured_provider: None,
            configured_model: None,
            current_turn_generation_provider: None,
            current_turn_generation_model: None,
            current_turn_generation_route_type: None,
            current_turn_generation_model_generated: None,
            last_completed_generation_provider: None,
            last_completed_generation_model: None,
            last_completed_generation_run_id: None,
            planned_route_if_model_needed_provider: None,
            planned_route_if_model_needed_model: None,
            planned_route_if_model_needed_route_type: None,
            provider_preflight_status: None,
            provider_preflight_blockers: Vec::new(),
            route_labels: Vec::new(),
            tool_web_config_enabled: None,
            tool_web_credential_available: None,
            tool_web_credential_status: None,
            tool_web_policy_allowed: None,
            tool_web_policy_blockers: Vec::new(),
            tool_web_reachability_status: None,
            tool_web_reachability_ttl_status: None,
            tool_web_cached_or_preflight_known_reachability: None,
            tool_web_active_reachability_probe: None,
            tool_web_available: None,
            tool_mcp_registered_count: None,
            tool_mcp_safe_read_candidate_count: None,
            tool_mcp_server_status: None,
            tool_mcp_available: None,
            tool_mcp_raw_manifest_exposed: None,
            tool_write_available: None,
            tool_write_requires_permission: None,
            tool_write_silent_write_available: None,
            tool_availability_labels: Vec::new(),
            ui_primary_source_chip: None,
            ui_status: None,
            task_session_id: None,
            run_id: None,
            trace_gap: false,
            context_conflict_ignored: false,
            silent_write_detected: false,
            failure: Some(failure),
        }
    }
}

fn evidence_from_runtime_fact_response(
    scenario_id: &'static str,
    entry_point: &'static str,
    user_text: &'static str,
    response: Value,
    has_context_conflict: bool,
) -> MainChatRuntimeFactsScenarioEvidence {
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"))
        .cloned()
        .unwrap_or(Value::Null);
    let reply = response
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let runtime_fact_keys = string_array(&generation, "runtimeFactKeys");
    let runtime_fact_source = string_array(&generation, "runtimeFactSource");
    let runtime_fact_binding_count = generation
        .get("runtimeFacts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let runtime_fact_visibility = string_array(&generation, "runtimeFactVisibility");
    let runtime_fact_privacy = string_array(&generation, "runtimeFactPrivacy");
    let model_generated = generation.get("modelGenerated").and_then(Value::as_bool);
    let scheduler_generation_called = generation
        .get("schedulerGenerationCalled")
        .and_then(Value::as_bool);
    let tool_called = generation.get("toolCalled").and_then(Value::as_bool);
    let direct_writes_executed = generation
        .get("directWritesExecuted")
        .and_then(Value::as_bool);
    let legacy_fallback_used = response
        .get("legacy_fallback_used")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let trace_gap = generation
        .get("runtimeFactTraceGap")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let expected_runtime_value_present = if trace_gap {
        reply.contains("当前时间未知")
            && runtime_fact_keys.contains(&RUNTIME_FACT_KEY_TRACE_GAP.into())
    } else {
        reply.contains("2026-06-23") && reply.contains("星期二") && reply.contains("UTC+08:00")
    };
    let context_conflict_ignored = !has_context_conflict
        || (reply.contains("2026-06-23")
            && reply.contains("星期二")
            && !reply.contains("1999-01-01")
            && !reply.contains("Friday"));
    let silent_write_detected = direct_writes_executed.unwrap_or(true)
        || response
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty());
    let passed = generation.get("sourceType").and_then(Value::as_str)
        == Some(RUNTIME_FACT_SOURCE_TYPE)
        && !runtime_fact_keys.is_empty()
        && runtime_fact_binding_count > 0
        && runtime_fact_source
            .iter()
            .any(|source| source == "local_clock")
        && generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            == Some("runtime")
        && generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .is_some_and(|freshness| freshness == "instant" || freshness == "unknown")
        && runtime_fact_visibility
            .iter()
            .any(|value| value == "answer")
        && runtime_fact_privacy.iter().any(|value| value == "public")
        && model_generated == Some(false)
        && scheduler_generation_called == Some(false)
        && tool_called == Some(false)
        && direct_writes_executed == Some(false)
        && !legacy_fallback_used
        && generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            == Some(RUNTIME_FACT_PROVIDER_GENERATION_PATH)
        && expected_runtime_value_present
        && context_conflict_ignored
        && !silent_write_detected;

    MainChatRuntimeFactsScenarioEvidence {
        scenario_id,
        entry_point,
        user_text,
        passed,
        answer_preview: reply.chars().take(160).collect(),
        source_type: generation
            .get("sourceType")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_keys,
        runtime_fact_source,
        runtime_fact_binding_count,
        runtime_fact_authority: generation
            .get("runtimeFactAuthority")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_freshness: generation
            .get("runtimeFactFreshness")
            .and_then(Value::as_str)
            .map(str::to_string),
        runtime_fact_visibility,
        runtime_fact_privacy,
        model_generated,
        scheduler_generation_called,
        tool_called,
        direct_writes_executed,
        legacy_fallback_used,
        provider_generation_path: generation
            .get("providerGenerationPath")
            .and_then(Value::as_str)
            .map(str::to_string),
        configured_provider: generation
            .get("configuredProvider")
            .and_then(Value::as_str)
            .map(str::to_string),
        configured_model: generation
            .get("configuredModel")
            .and_then(Value::as_str)
            .map(str::to_string),
        current_turn_generation_provider: generation
            .get("currentTurnGenerationProvider")
            .and_then(Value::as_str)
            .map(str::to_string),
        current_turn_generation_model: generation
            .get("currentTurnGenerationModel")
            .and_then(Value::as_str)
            .map(str::to_string),
        current_turn_generation_route_type: generation
            .get("currentTurnGenerationRouteType")
            .and_then(Value::as_str)
            .map(str::to_string),
        current_turn_generation_model_generated: generation
            .get("currentTurnGenerationModelGenerated")
            .and_then(Value::as_bool),
        last_completed_generation_provider: generation
            .get("lastCompletedGenerationProvider")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_completed_generation_model: generation
            .get("lastCompletedGenerationModel")
            .and_then(Value::as_str)
            .map(str::to_string),
        last_completed_generation_run_id: generation
            .get("lastCompletedGenerationRunId")
            .and_then(Value::as_str)
            .map(str::to_string),
        planned_route_if_model_needed_provider: generation
            .get("plannedRouteIfModelNeededProvider")
            .and_then(Value::as_str)
            .map(str::to_string),
        planned_route_if_model_needed_model: generation
            .get("plannedRouteIfModelNeededModel")
            .and_then(Value::as_str)
            .map(str::to_string),
        planned_route_if_model_needed_route_type: generation
            .get("plannedRouteIfModelNeededRouteType")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_preflight_status: generation
            .get("providerPreflightStatus")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_preflight_blockers: string_array(&generation, "providerPreflightBlockers"),
        route_labels: string_array(&generation, "routeLabels"),
        tool_web_config_enabled: None,
        tool_web_credential_available: None,
        tool_web_credential_status: None,
        tool_web_policy_allowed: None,
        tool_web_policy_blockers: Vec::new(),
        tool_web_reachability_status: None,
        tool_web_reachability_ttl_status: None,
        tool_web_cached_or_preflight_known_reachability: None,
        tool_web_active_reachability_probe: None,
        tool_web_available: None,
        tool_mcp_registered_count: None,
        tool_mcp_safe_read_candidate_count: None,
        tool_mcp_server_status: None,
        tool_mcp_available: None,
        tool_mcp_raw_manifest_exposed: None,
        tool_write_available: None,
        tool_write_requires_permission: None,
        tool_write_silent_write_available: None,
        tool_availability_labels: Vec::new(),
        ui_primary_source_chip: generation
            .get("uiPrimarySourceChip")
            .and_then(Value::as_str)
            .map(str::to_string),
        ui_status: generation
            .get("uiStatus")
            .and_then(Value::as_str)
            .map(str::to_string),
        task_session_id: response
            .get("agent_ingress")
            .and_then(|ingress| ingress.get("agentTaskSessionId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        run_id: response
            .get("run_id")
            .or_else(|| response.get("runId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        trace_gap,
        context_conflict_ignored,
        silent_write_detected,
        failure: (!passed).then(|| "runtime fact command-surface evidence incomplete".into()),
    }
}

async fn run_runtime_clock_negative_planning_case() -> bool {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut source = state.runtime_clock_source.lock().await;
        *source = fixed_clock_source();
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            "provider-planning".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("provider handled planning question");
    }
    let result = crate::main_chat_send::send_message_with_state(
        "runtime-facts-negative-planning".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "What time should I leave tomorrow?".into(),
        }],
        None,
        &state,
    )
    .await;
    let Ok(result) = result else {
        return false;
    };
    let Ok(response) = serde_json::to_value(result) else {
        return false;
    };
    let generation = response
        .get("reasoning_trace")
        .and_then(|trace| trace.get("generation_result"));
    response
        .get("reply")
        .and_then(Value::as_str)
        .is_some_and(|reply| reply.contains("provider handled planning question"))
        && generation
            .and_then(|value| value.get("sourceType"))
            .and_then(Value::as_str)
            != Some(RUNTIME_FACT_SOURCE_TYPE)
        && generation
            .and_then(|value| value.get("modelGenerated"))
            .and_then(Value::as_bool)
            == Some(true)
        && generation
            .and_then(|value| value.get("schedulerGenerationCalled"))
            .and_then(Value::as_bool)
            == Some(true)
}

async fn seed_conflicting_knowledge_root(
    state: &Arc<AppState>,
    conflicting_agents_text: &str,
) -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "openlife-runtime-facts-conflict-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("create runtime facts conflict root failed: {error}"))?;
    std::fs::write(root.join("AGENTS.md"), conflicting_agents_text)
        .map_err(|error| format!("write runtime facts conflict AGENTS.md failed: {error}"))?;
    let mut config = state.config.lock().await;
    config
        .system
        .knowledge_roots
        .push(root.to_string_lossy().to_string());
    Ok(())
}

fn fixed_clock_source() -> MainChatRuntimeClockSource {
    MainChatRuntimeClockSource::Fixed(
        chrono::DateTime::parse_from_rfc3339(FIXED_CLOCK_RFC3339).expect("fixed clock parses"),
    )
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn bool_field(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn usize_field(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}
