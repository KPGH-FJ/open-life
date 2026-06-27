use serde::{Deserialize, Serialize};
use serde_json::Value;

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
pub(crate) const RUNTIME_FACT_AGENT_SELF_STATE_GENERATION_PATH: &str =
    "main_chat_agent_self_state_runtime_fact";
pub(crate) const RUNTIME_FACT_KEY_AGENT_CHAT_SESSION_ID: &str = "agent.chat_session_id";
pub(crate) const RUNTIME_FACT_KEY_AGENT_TASK_SESSION_ID: &str = "agent.task_session_id";
pub(crate) const RUNTIME_FACT_KEY_AGENT_RUN_ID: &str = "agent.run_id";
pub(crate) const RUNTIME_FACT_KEY_AGENT_TASK_STATUS: &str = "agent.task_status";
pub(crate) const RUNTIME_FACT_KEY_AGENT_DELIVERY_STATUS: &str = "agent.delivery_status";
pub(crate) const RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY: &str = "agent.last_action.summary";
pub(crate) const RUNTIME_FACT_KEY_AGENT_PENDING_PERMISSION_COUNT: &str =
    "agent.pending_permission.count";
pub(crate) const RUNTIME_FACT_KEY_AGENT_BLOCKER_CODES: &str = "agent.blocker.codes";
pub(crate) const RUNTIME_FACT_KEY_AGENT_PENDING_PROPOSAL_COUNT: &str =
    "agent.pending_proposal.count";
pub(crate) const RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS: &str = "agent.durable_change.status";
pub(crate) const RUNTIME_FACT_KEY_AGENT_TRACE_GAP: &str = "agent.self_state.trace_gap";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum MainChatRuntimeClockIntent {
    AskCurrentWeekday,
    AskCurrentDate,
    AskCurrentTime,
}

impl MainChatRuntimeClockIntent {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AskCurrentWeekday => "ask_current_weekday",
            Self::AskCurrentDate => "ask_current_date",
            Self::AskCurrentTime => "ask_current_time",
        }
    }

    pub(crate) fn fact_keys(self) -> Vec<&'static str> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatProviderRouteIntent {
    AskCurrentModelRoute,
    AskPreviousTurnModelRoute,
}

impl MainChatProviderRouteIntent {
    pub(crate) fn as_str(self) -> &'static str {
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
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AskToolAvailability => "ask_tool_availability",
            Self::AskWriteCapability => "ask_write_capability",
        }
    }

    pub(crate) fn fact_keys(self) -> Vec<&'static str> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatAgentSelfStateIntent {
    AskTaskCompletion,
    AskLastActionSummary,
}

impl MainChatAgentSelfStateIntent {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AskTaskCompletion => "ask_task_completion",
            Self::AskLastActionSummary => "ask_last_action_summary",
        }
    }

    pub(crate) fn fact_keys(self) -> Vec<&'static str> {
        match self {
            Self::AskTaskCompletion => vec![
                RUNTIME_FACT_KEY_AGENT_CHAT_SESSION_ID,
                RUNTIME_FACT_KEY_AGENT_TASK_SESSION_ID,
                RUNTIME_FACT_KEY_AGENT_RUN_ID,
                RUNTIME_FACT_KEY_AGENT_TASK_STATUS,
                RUNTIME_FACT_KEY_AGENT_DELIVERY_STATUS,
                RUNTIME_FACT_KEY_AGENT_PENDING_PERMISSION_COUNT,
                RUNTIME_FACT_KEY_AGENT_BLOCKER_CODES,
                RUNTIME_FACT_KEY_AGENT_PENDING_PROPOSAL_COUNT,
                RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS,
            ],
            Self::AskLastActionSummary => vec![
                RUNTIME_FACT_KEY_AGENT_CHAT_SESSION_ID,
                RUNTIME_FACT_KEY_AGENT_TASK_SESSION_ID,
                RUNTIME_FACT_KEY_AGENT_RUN_ID,
                RUNTIME_FACT_KEY_AGENT_TASK_STATUS,
                RUNTIME_FACT_KEY_AGENT_DELIVERY_STATUS,
                RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY,
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

pub(crate) fn bounded_runtime_fact_label(value: &str) -> String {
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

pub(crate) fn label_or_unknown(value: Option<&str>) -> &str {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown")
}

pub(crate) fn merge_json_object(target: &mut Value, extra: Value) {
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

pub(crate) fn matches_exact_runtime_fact_phrase(value: &str, phrases: &[&str]) -> bool {
    phrases.contains(&value)
}

pub(crate) fn trim_outer_punctuation(value: &str) -> &str {
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
