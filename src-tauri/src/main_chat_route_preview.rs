use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::Utc;
use openlife_core::agent::main_chat_agent_v1::{AgentIngressDecision, MainChatAgentStrategy};
use openlife_core::config::{AgentRuntimeMode, AppConfig};
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::main_chat_turn_pipeline::{
    MainChatExecutionPath, MainChatTurnRouteDecision, MainChatTurnStreamMode,
};
use crate::provider_validation::{
    load_provider_validation_record_from_path, provider_validation_path,
    summarize_provider_validation, ProviderValidationRecord, ProviderValidationSummary,
};
use crate::AppState;

const MIN_ROUTE_PREVIEW_CONFIDENCE: f64 = 0.70;
const MAX_ROUTE_PREVIEW_REASON_CHARS: usize = 96;
const MAX_ROUTE_PREVIEW_LABEL_CHARS: usize = 96;
const MAX_ROUTE_PREVIEW_CONTEXT_CHARS: usize = 1_600;
const ROUTE_PREVIEW_SYSTEM_PROMPT: &str = r#"Return exactly one JSON object and no other text.
Schema:
{"route":"direct_answer|tool_loop|plan_execute|memory_proposal|permission_request|blocked","confidence":0.0,"requires_tools":false,"requires_write":false,"reason":"metadata-safe short reason"}
Do not include tool targets, candidate ids, arguments, file paths, memory text, LifeModel YAML, credentials, markdown fences, arrays, or extra fields."#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatRoutePreviewRoute {
    DirectAnswer,
    ToolLoop,
    PlanExecute,
    MemoryProposal,
    PermissionRequest,
    Blocked,
}

impl MainChatRoutePreviewRoute {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DirectAnswer => "direct_answer",
            Self::ToolLoop => "tool_loop",
            Self::PlanExecute => "plan_execute",
            Self::MemoryProposal => "memory_proposal",
            Self::PermissionRequest => "permission_request",
            Self::Blocked => "blocked",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "direct_answer" => Some(Self::DirectAnswer),
            "tool_loop" => Some(Self::ToolLoop),
            "plan_execute" => Some(Self::PlanExecute),
            "memory_proposal" => Some(Self::MemoryProposal),
            "permission_request" => Some(Self::PermissionRequest),
            "blocked" => Some(Self::Blocked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MainChatStructuredRoutePreview {
    pub(crate) route: MainChatRoutePreviewRoute,
    pub(crate) confidence: f64,
    pub(crate) requires_tools: bool,
    pub(crate) requires_write: bool,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatRoutePreviewParserStatus {
    NotAttempted,
    Valid,
    InvalidJson,
    MarkdownFence,
    NotObject,
    ExtraFields,
    MissingField,
    UnknownRoute,
    InvalidConfidence,
    UnsafeReason,
    InconsistentFlags,
    ProviderError,
}

impl MainChatRoutePreviewParserStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::Valid => "valid",
            Self::InvalidJson => "invalid_json",
            Self::MarkdownFence => "markdown_fence",
            Self::NotObject => "not_object",
            Self::ExtraFields => "extra_fields",
            Self::MissingField => "missing_field",
            Self::UnknownRoute => "unknown_route",
            Self::InvalidConfidence => "invalid_confidence",
            Self::UnsafeReason => "unsafe_reason",
            Self::InconsistentFlags => "inconsistent_flags",
            Self::ProviderError => "provider_error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatRoutePreviewTrace {
    pub(crate) attempted: bool,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) deterministic_route: String,
    pub(crate) deterministic_execution_path: String,
    pub(crate) accepted_route: Option<String>,
    pub(crate) effective_route: String,
    pub(crate) accepted_reason: Option<String>,
    pub(crate) ignored_reason: Option<String>,
    pub(crate) parser_status: String,
    pub(crate) response_digest: Option<String>,
    pub(crate) confidence: Option<f64>,
    pub(crate) requires_tools: Option<bool>,
    pub(crate) requires_write: Option<bool>,
    pub(crate) advisory_reason: Option<String>,
}

impl MainChatRoutePreviewTrace {
    fn not_attempted(
        route_decision: &MainChatTurnRouteDecision,
        agent_decision: &AgentIngressDecision,
        ignored_reason: impl Into<String>,
    ) -> Self {
        let deterministic_route = deterministic_route_label(route_decision, agent_decision)
            .as_str()
            .to_string();
        Self {
            attempted: false,
            provider: None,
            model: None,
            deterministic_route: deterministic_route.clone(),
            deterministic_execution_path: route_decision.execution_path_label().to_string(),
            accepted_route: None,
            effective_route: deterministic_route,
            accepted_reason: None,
            ignored_reason: Some(ignored_reason.into()),
            parser_status: MainChatRoutePreviewParserStatus::NotAttempted
                .as_str()
                .to_string(),
            response_digest: None,
            confidence: None,
            requires_tools: None,
            requires_write: None,
            advisory_reason: None,
        }
    }
}

pub(crate) async fn preview_main_chat_turn_route(
    state: &Arc<AppState>,
    messages: &[ChatMessage],
    agent_decision: &AgentIngressDecision,
    route_decision: &MainChatTurnRouteDecision,
    stream_mode: MainChatTurnStreamMode,
) -> MainChatRoutePreviewTrace {
    let validation_record = load_provider_validation_record_from_path(&provider_validation_path());
    preview_main_chat_turn_route_with_validation_record(
        state,
        messages,
        agent_decision,
        route_decision,
        stream_mode,
        validation_record.as_ref(),
    )
    .await
}

pub(crate) async fn preview_main_chat_turn_route_with_validation_record(
    state: &Arc<AppState>,
    messages: &[ChatMessage],
    agent_decision: &AgentIngressDecision,
    route_decision: &MainChatTurnRouteDecision,
    stream_mode: MainChatTurnStreamMode,
    validation_record: Option<&ProviderValidationRecord>,
) -> MainChatRoutePreviewTrace {
    let (config, scheduler) = {
        let config = state.config.lock().await.clone();
        let scheduler = state.scheduler.lock().await.clone();
        (config, scheduler)
    };
    let validation_summary = summarize_provider_validation(&config, validation_record, Utc::now());
    let gate = evaluate_route_preview_gate(
        &config,
        &scheduler,
        &validation_summary,
        messages,
        agent_decision,
        route_decision,
        stream_mode,
    );
    let routing_context = match gate {
        Ok(context) => context,
        Err(reason) => {
            return MainChatRoutePreviewTrace::not_attempted(route_decision, agent_decision, reason)
        }
    };

    invoke_route_preview_provider(scheduler, route_decision, agent_decision, routing_context).await
}

pub(crate) fn attach_route_preview_trace(
    reasoning_trace: &mut openlife_core::agent::ReasoningTrace,
    preview_trace: &MainChatRoutePreviewTrace,
) {
    let preview_value = serde_json::to_value(preview_trace).unwrap_or_else(|_| {
        serde_json::json!({
            "attempted": false,
            "parserStatus": "serialization_failed",
        })
    });
    match reasoning_trace.generation_result.as_mut() {
        Some(Value::Object(object)) => {
            object.insert("routePreview".into(), preview_value);
        }
        Some(existing) => {
            let previous = existing.clone();
            *existing = serde_json::json!({
                "routePreview": preview_value,
                "previousGenerationResultDigest": metadata_safe_digest(previous.to_string().as_bytes()),
            });
        }
        None => {
            reasoning_trace.generation_result = Some(serde_json::json!({
                "routePreview": preview_value,
            }));
        }
    }
}

pub(crate) fn parse_main_chat_structured_route_preview(
    raw: &str,
) -> Result<MainChatStructuredRoutePreview, MainChatRoutePreviewParserStatus> {
    if raw.contains("```") {
        return Err(MainChatRoutePreviewParserStatus::MarkdownFence);
    }
    let value: Value =
        serde_json::from_str(raw).map_err(|_| MainChatRoutePreviewParserStatus::InvalidJson)?;
    let object = value
        .as_object()
        .ok_or(MainChatRoutePreviewParserStatus::NotObject)?;
    let allowed = [
        "route",
        "confidence",
        "requires_tools",
        "requires_write",
        "reason",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if object.keys().any(|key| !allowed.contains(key.as_str())) {
        return Err(MainChatRoutePreviewParserStatus::ExtraFields);
    }
    if allowed.iter().any(|key| !object.contains_key(*key)) {
        return Err(MainChatRoutePreviewParserStatus::MissingField);
    }

    let route = object
        .get("route")
        .and_then(Value::as_str)
        .and_then(MainChatRoutePreviewRoute::parse)
        .ok_or(MainChatRoutePreviewParserStatus::UnknownRoute)?;
    let confidence = object
        .get("confidence")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
        .ok_or(MainChatRoutePreviewParserStatus::InvalidConfidence)?;
    let requires_tools = object
        .get("requires_tools")
        .and_then(Value::as_bool)
        .ok_or(MainChatRoutePreviewParserStatus::MissingField)?;
    let requires_write = object
        .get("requires_write")
        .and_then(Value::as_bool)
        .ok_or(MainChatRoutePreviewParserStatus::MissingField)?;
    let reason = object
        .get("reason")
        .and_then(Value::as_str)
        .ok_or(MainChatRoutePreviewParserStatus::UnsafeReason)?;
    if !route_preview_reason_is_safe(reason) {
        return Err(MainChatRoutePreviewParserStatus::UnsafeReason);
    }
    if !route_preview_flags_are_consistent(route, requires_tools, requires_write) {
        return Err(MainChatRoutePreviewParserStatus::InconsistentFlags);
    }

    Ok(MainChatStructuredRoutePreview {
        route,
        confidence,
        requires_tools,
        requires_write,
        reason: reason.to_string(),
    })
}

fn evaluate_route_preview_gate(
    config: &AppConfig,
    scheduler: &InferenceScheduler,
    validation_summary: &ProviderValidationSummary,
    messages: &[ChatMessage],
    agent_decision: &AgentIngressDecision,
    route_decision: &MainChatTurnRouteDecision,
    stream_mode: MainChatTurnStreamMode,
) -> Result<String, &'static str> {
    if config.runtime_mode != AgentRuntimeMode::CapabilityFirst {
        return Err("runtime_mode_not_capability_first");
    }
    if !validation_summary.configured {
        return Err("provider_unconfigured");
    }
    if !validation_summary.validated {
        return Err("provider_unvalidated");
    }
    if !scheduler_matches_validated_config(scheduler, config) {
        return Err("provider_scheduler_config_mismatch");
    }
    if agent_decision.privacy_risk.local_only_required
        || agent_decision.privacy_risk.privacy_class == "sensitive"
    {
        return Err("hs_sensitive_or_local_only");
    }
    if !network_policy_allows_provider_preview(config) {
        return Err("network_policy_blocked");
    }
    render_metadata_safe_routing_context(messages, agent_decision, route_decision, stream_mode)
}

fn scheduler_matches_validated_config(scheduler: &InferenceScheduler, config: &AppConfig) -> bool {
    scheduler.provider.trim() == config.llm.provider.trim()
        && scheduler.openai_base.trim_end_matches('/')
            == config.llm.openai_base.trim_end_matches('/')
        && scheduler.chat_model.trim() == config.llm.chat_model.trim()
        && !scheduler.effective_api_key().trim().is_empty()
        && !config.effective_cloud_api_key().trim().is_empty()
}

fn network_policy_allows_provider_preview(config: &AppConfig) -> bool {
    if !config.system.network_policy.enabled {
        return false;
    }
    !matches!(
        config
            .system
            .network_policy
            .default_decision
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "deny" | "block" | "blocked"
    )
}

async fn invoke_route_preview_provider(
    mut scheduler: InferenceScheduler,
    route_decision: &MainChatTurnRouteDecision,
    agent_decision: &AgentIngressDecision,
    routing_context: String,
) -> MainChatRoutePreviewTrace {
    let provider = metadata_safe_label(&scheduler.provider, MAX_ROUTE_PREVIEW_LABEL_CHARS)
        .unwrap_or_else(|| "unknown_provider".into());
    let model = metadata_safe_label(&scheduler.chat_model, MAX_ROUTE_PREVIEW_LABEL_CHARS)
        .unwrap_or_else(|| "unknown_model".into());
    scheduler.prefer_local = false;
    let response = scheduler
        .generate_raw(
            vec![ChatMessage {
                role: "user".into(),
                content: routing_context,
            }],
            Some(ROUTE_PREVIEW_SYSTEM_PROMPT),
        )
        .await;

    match response {
        Ok(raw) => route_preview_trace_from_raw_response(
            route_decision,
            agent_decision,
            Some(provider),
            Some(model),
            &raw,
        ),
        Err(_) => {
            let deterministic_route = deterministic_route_label(route_decision, agent_decision)
                .as_str()
                .to_string();
            MainChatRoutePreviewTrace {
                attempted: true,
                provider: Some(provider),
                model: Some(model),
                deterministic_route: deterministic_route.clone(),
                deterministic_execution_path: route_decision.execution_path_label().to_string(),
                accepted_route: None,
                effective_route: deterministic_route,
                accepted_reason: None,
                ignored_reason: Some("provider_preview_failed".into()),
                parser_status: MainChatRoutePreviewParserStatus::ProviderError
                    .as_str()
                    .to_string(),
                response_digest: None,
                confidence: None,
                requires_tools: None,
                requires_write: None,
                advisory_reason: None,
            }
        }
    }
}

fn route_preview_trace_from_raw_response(
    route_decision: &MainChatTurnRouteDecision,
    agent_decision: &AgentIngressDecision,
    provider: Option<String>,
    model: Option<String>,
    raw: &str,
) -> MainChatRoutePreviewTrace {
    let deterministic_route = deterministic_route_label(route_decision, agent_decision)
        .as_str()
        .to_string();
    let response_digest = Some(metadata_safe_digest(raw.as_bytes()));
    match parse_main_chat_structured_route_preview(raw) {
        Ok(parsed) if parsed.confidence >= MIN_ROUTE_PREVIEW_CONFIDENCE => {
            MainChatRoutePreviewTrace {
                attempted: true,
                provider,
                model,
                deterministic_route: deterministic_route.clone(),
                deterministic_execution_path: route_decision.execution_path_label().to_string(),
                accepted_route: Some(parsed.route.as_str().to_string()),
                effective_route: deterministic_route,
                accepted_reason: Some("accepted_high_confidence_advisory_preview".into()),
                ignored_reason: None,
                parser_status: MainChatRoutePreviewParserStatus::Valid.as_str().to_string(),
                response_digest,
                confidence: Some(parsed.confidence),
                requires_tools: Some(parsed.requires_tools),
                requires_write: Some(parsed.requires_write),
                advisory_reason: Some(parsed.reason),
            }
        }
        Ok(parsed) => MainChatRoutePreviewTrace {
            attempted: true,
            provider,
            model,
            deterministic_route: deterministic_route.clone(),
            deterministic_execution_path: route_decision.execution_path_label().to_string(),
            accepted_route: None,
            effective_route: deterministic_route,
            accepted_reason: None,
            ignored_reason: Some("low_confidence".into()),
            parser_status: MainChatRoutePreviewParserStatus::Valid.as_str().to_string(),
            response_digest,
            confidence: Some(parsed.confidence),
            requires_tools: Some(parsed.requires_tools),
            requires_write: Some(parsed.requires_write),
            advisory_reason: Some(parsed.reason),
        },
        Err(status) => MainChatRoutePreviewTrace {
            attempted: true,
            provider,
            model,
            deterministic_route: deterministic_route.clone(),
            deterministic_execution_path: route_decision.execution_path_label().to_string(),
            accepted_route: None,
            effective_route: deterministic_route,
            accepted_reason: None,
            ignored_reason: Some(status.as_str().into()),
            parser_status: status.as_str().to_string(),
            response_digest,
            confidence: None,
            requires_tools: None,
            requires_write: None,
            advisory_reason: None,
        },
    }
}

fn deterministic_route_label(
    route_decision: &MainChatTurnRouteDecision,
    agent_decision: &AgentIngressDecision,
) -> MainChatRoutePreviewRoute {
    match route_decision.path {
        MainChatExecutionPath::KernelDirect => MainChatRoutePreviewRoute::DirectAnswer,
        MainChatExecutionPath::KernelReadTool | MainChatExecutionPath::ToolLoop => {
            MainChatRoutePreviewRoute::ToolLoop
        }
        MainChatExecutionPath::PlanExecute => MainChatRoutePreviewRoute::PlanExecute,
        MainChatExecutionPath::KernelWriteOutcome => match agent_decision.selected_strategy {
            MainChatAgentStrategy::MemoryProposal | MainChatAgentStrategy::LifeModelProposal => {
                MainChatRoutePreviewRoute::MemoryProposal
            }
            MainChatAgentStrategy::BlockedConfirmation => {
                MainChatRoutePreviewRoute::PermissionRequest
            }
            _ => MainChatRoutePreviewRoute::Blocked,
        },
        MainChatExecutionPath::GovernedBlocker | MainChatExecutionPath::LegacyCompatFallback => {
            MainChatRoutePreviewRoute::Blocked
        }
    }
}

fn render_metadata_safe_routing_context(
    messages: &[ChatMessage],
    agent_decision: &AgentIngressDecision,
    route_decision: &MainChatTurnRouteDecision,
    stream_mode: MainChatTurnStreamMode,
) -> Result<String, &'static str> {
    let context = serde_json::json!({
        "schema": "main_chat_route_preview_context.v1",
        "metadataSafe": true,
        "streamMode": stream_mode.as_str(),
        "messageCount": messages.len(),
        "latestUserMessageChars": latest_user_message_chars(messages),
        "deterministicRoute": deterministic_route_label(route_decision, agent_decision).as_str(),
        "deterministicExecutionPath": route_decision.execution_path_label(),
        "deterministicReasonCode": route_decision.reason_code,
        "selectedStrategy": agent_decision.selected_strategy.as_str(),
        "agentConfidenceBucket": confidence_bucket(agent_decision.confidence as f64),
        "kernelSupported": route_decision.kernel_supported,
        "kernelSupportDisposition": route_decision.kernel_support_disposition,
        "fallbackAllowed": route_decision.fallback_allowed,
        "requiresProvider": route_decision.requires_provider,
        "requiresToolLoop": route_decision.requires_tool_loop,
        "privacy": {
            "riskLevel": metadata_safe_label(&agent_decision.privacy_risk.risk_level, MAX_ROUTE_PREVIEW_LABEL_CHARS)
                .ok_or("routing_context_unsafe")?,
            "privacyClass": metadata_safe_label(&agent_decision.privacy_risk.privacy_class, MAX_ROUTE_PREVIEW_LABEL_CHARS)
                .ok_or("routing_context_unsafe")?,
            "policyReasonCode": metadata_safe_label(&agent_decision.privacy_risk.policy_reason_code, MAX_ROUTE_PREVIEW_LABEL_CHARS)
                .ok_or("routing_context_unsafe")?,
            "localOnlyRequired": agent_decision.privacy_risk.local_only_required,
            "writeLike": agent_decision.privacy_risk.write_like,
            "externalWriteLike": agent_decision.privacy_risk.external_write_like,
        }
    });
    let text = serde_json::to_string(&context).map_err(|_| "routing_context_unsafe")?;
    if text.chars().count() > MAX_ROUTE_PREVIEW_CONTEXT_CHARS {
        return Err("routing_context_oversized");
    }
    if text.chars().any(|ch| ch.is_control()) {
        return Err("routing_context_unsafe");
    }
    Ok(text)
}

fn latest_user_message_chars(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| message.content.chars().count())
        .unwrap_or_default()
}

fn confidence_bucket(confidence: f64) -> &'static str {
    if confidence >= 0.9 {
        "high"
    } else if confidence >= 0.7 {
        "medium"
    } else {
        "low"
    }
}

fn route_preview_reason_is_safe(reason: &str) -> bool {
    let trimmed = reason.trim();
    if trimmed.is_empty()
        || trimmed != reason
        || reason.chars().count() > MAX_ROUTE_PREVIEW_REASON_CHARS
        || reason.chars().any(|ch| ch.is_control() || !ch.is_ascii())
    {
        return false;
    }
    if reason.chars().any(|ch| {
        matches!(
            ch,
            '`' | '{' | '}' | '[' | ']' | '<' | '>' | '|' | '\\' | '/'
        )
    }) {
        return false;
    }
    let lower = reason.to_ascii_lowercase();
    ![
        "sk-",
        "api key",
        "apikey",
        "token",
        "password",
        "secret",
        "private key",
        "authorization",
        "bearer ",
        "-----begin",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn route_preview_flags_are_consistent(
    route: MainChatRoutePreviewRoute,
    requires_tools: bool,
    requires_write: bool,
) -> bool {
    match route {
        MainChatRoutePreviewRoute::DirectAnswer => !requires_tools && !requires_write,
        MainChatRoutePreviewRoute::ToolLoop | MainChatRoutePreviewRoute::PlanExecute => {
            requires_tools && !requires_write
        }
        MainChatRoutePreviewRoute::MemoryProposal => requires_write,
        MainChatRoutePreviewRoute::PermissionRequest => requires_tools,
        MainChatRoutePreviewRoute::Blocked => !requires_tools,
    }
}

fn metadata_safe_label(value: &str, max_chars: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(|ch| ch.is_control() || !ch.is_ascii()) {
        return None;
    }
    let mut output = String::new();
    let mut last_was_space = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/') {
            output.push(ch);
            last_was_space = false;
        } else if ch.is_ascii_whitespace() {
            if !last_was_space {
                output.push(' ');
                last_was_space = true;
            }
        } else {
            return None;
        }
        if output.chars().count() >= max_chars {
            break;
        }
    }
    let output = output.trim().to_string();
    (!output.is_empty()).then_some(output)
}

fn metadata_safe_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("bytes:{} hash:sha256:{:x}", bytes.len(), hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::main_chat_agent_v1::AgentIngress;
    use openlife_core::agent::AgentTaskKind;
    use openlife_core::config::NetworkPolicy;

    use crate::main_chat_command_surface_eval::{
        configure_main_chat_command_surface_eval_state, main_chat_command_surface_eval_user_text,
        MainChatCommandSurfaceEvalScenario,
    };
    use crate::main_chat_send::send_message_with_state;

    fn user_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: "user".into(),
            content: content.into(),
        }
    }

    fn route_decision_for_text(
        user_text: &str,
    ) -> (AgentIngressDecision, MainChatTurnRouteDecision) {
        let ingress = AgentIngress::default();
        let decision = ingress.decide(
            "route-preview-test-session",
            user_text,
            None,
            AgentTaskKind::Conversation,
        );
        let disposition = crate::main_chat_kernel::main_chat_kernel_support_disposition(
            &decision.selected_strategy,
            &[user_message(user_text)],
        );
        let route_decision =
            crate::main_chat_turn_pipeline::decide_main_chat_turn_route_from_disposition(
                decision.selected_strategy,
                disposition,
                false,
                false,
            );
        (decision, route_decision)
    }

    fn configured_capability_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.runtime_mode = AgentRuntimeMode::CapabilityFirst;
        config.llm.provider = "openai".into();
        config.llm.openai_base = "https://api.openai.com/v1".into();
        config.llm.openai_key = "sk-route-preview-test".into();
        config.llm.chat_model = "gpt-route-preview".into();
        config.prefer_local_model = false;
        config.system.network_policy = NetworkPolicy {
            enabled: true,
            ..Default::default()
        };
        config
    }

    fn scheduler_for_config(config: &AppConfig, response: impl Into<String>) -> InferenceScheduler {
        InferenceScheduler::new(
            config.local_model.clone(),
            false,
            config.llm.provider.clone(),
            config.llm.openai_base.clone(),
            config.llm.openai_key.clone(),
            config.llm.chat_model.clone(),
            config.llm.embedding_model.clone(),
            config.llm.embedding_enabled,
        )
        .with_scripted_generation_response(response)
    }

    fn valid_json(route: &str, confidence: f64) -> String {
        serde_json::json!({
            "route": route,
            "confidence": confidence,
            "requires_tools": route != "direct_answer",
            "requires_write": route == "memory_proposal",
            "reason": "metadata safe route preview"
        })
        .to_string()
    }

    #[test]
    fn main_chat_route_preview_parser_accepts_exact_valid_json() {
        let parsed = parse_main_chat_structured_route_preview(&valid_json("tool_loop", 0.91))
            .expect("valid route preview");
        assert_eq!(parsed.route, MainChatRoutePreviewRoute::ToolLoop);
        assert_eq!(parsed.confidence, 0.91);
        assert!(parsed.requires_tools);
        assert!(!parsed.requires_write);
        assert_eq!(parsed.reason, "metadata safe route preview");
    }

    #[test]
    fn main_chat_route_preview_parser_rejects_invalid_json() {
        let status = parse_main_chat_structured_route_preview("{not-json").unwrap_err();
        assert_eq!(status, MainChatRoutePreviewParserStatus::InvalidJson);
    }

    #[test]
    fn main_chat_route_preview_parser_rejects_markdown_fenced_json() {
        let raw = format!("```json\n{}\n```", valid_json("direct_answer", 0.88));
        let status = parse_main_chat_structured_route_preview(&raw).unwrap_err();
        assert_eq!(status, MainChatRoutePreviewParserStatus::MarkdownFence);
    }

    #[test]
    fn main_chat_route_preview_parser_rejects_arrays() {
        let raw = format!("[{}]", valid_json("direct_answer", 0.88));
        let status = parse_main_chat_structured_route_preview(&raw).unwrap_err();
        assert_eq!(status, MainChatRoutePreviewParserStatus::NotObject);
    }

    #[test]
    fn main_chat_route_preview_parser_rejects_extra_fields() {
        let raw = serde_json::json!({
            "route": "direct_answer",
            "confidence": 0.86,
            "requires_tools": false,
            "requires_write": false,
            "reason": "metadata safe route preview",
            "target": "file.read"
        })
        .to_string();
        let status = parse_main_chat_structured_route_preview(&raw).unwrap_err();
        assert_eq!(status, MainChatRoutePreviewParserStatus::ExtraFields);
    }

    #[test]
    fn main_chat_route_preview_parser_rejects_missing_fields() {
        let raw = serde_json::json!({
            "route": "direct_answer",
            "confidence": 0.86,
            "requires_tools": false,
            "requires_write": false
        })
        .to_string();
        let status = parse_main_chat_structured_route_preview(&raw).unwrap_err();
        assert_eq!(status, MainChatRoutePreviewParserStatus::MissingField);
    }

    #[test]
    fn main_chat_route_preview_parser_rejects_unknown_route() {
        let raw = valid_json("magic_route", 0.86);
        let status = parse_main_chat_structured_route_preview(&raw).unwrap_err();
        assert_eq!(status, MainChatRoutePreviewParserStatus::UnknownRoute);
    }

    #[test]
    fn main_chat_route_preview_parser_rejects_invalid_confidence() {
        let raw = serde_json::json!({
            "route": "direct_answer",
            "confidence": 1.01,
            "requires_tools": false,
            "requires_write": false,
            "reason": "metadata safe route preview"
        })
        .to_string();
        let status = parse_main_chat_structured_route_preview(&raw).unwrap_err();
        assert_eq!(status, MainChatRoutePreviewParserStatus::InvalidConfidence);
    }

    #[test]
    fn main_chat_route_preview_parser_rejects_unsafe_reason() {
        let raw = serde_json::json!({
            "route": "direct_answer",
            "confidence": 0.86,
            "requires_tools": false,
            "requires_write": false,
            "reason": "contains sk-secret-token"
        })
        .to_string();
        let status = parse_main_chat_structured_route_preview(&raw).unwrap_err();
        assert_eq!(status, MainChatRoutePreviewParserStatus::UnsafeReason);
    }

    #[test]
    fn main_chat_route_preview_low_confidence_is_ignored() {
        let (agent_decision, route_decision) =
            route_decision_for_text("Explain route preview in one sentence.");
        let trace = route_preview_trace_from_raw_response(
            &route_decision,
            &agent_decision,
            Some("openai".into()),
            Some("gpt-route-preview".into()),
            &valid_json("direct_answer", 0.42),
        );
        assert!(trace.attempted);
        assert_eq!(trace.accepted_route, None);
        assert_eq!(trace.ignored_reason.as_deref(), Some("low_confidence"));
        assert_eq!(trace.effective_route, trace.deterministic_route);
    }

    #[tokio::test]
    async fn main_chat_route_preview_skips_local_only_sensitive_turn() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let config = configured_capability_config();
        let record = crate::provider_validation::successful_provider_validation_record(
            &config,
            "route_preview_test",
            Utc::now(),
        );
        {
            let mut state_config = state.config.lock().await;
            *state_config = config.clone();
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scheduler_for_config(&config, valid_json("direct_answer", 0.95));
        }
        let user_text = "This is a private health question.";
        let (agent_decision, route_decision) = route_decision_for_text(user_text);
        let trace = preview_main_chat_turn_route_with_validation_record(
            &state,
            &[user_message(user_text)],
            &agent_decision,
            &route_decision,
            MainChatTurnStreamMode::Buffered,
            Some(&record),
        )
        .await;
        assert!(!trace.attempted);
        assert_eq!(
            trace.ignored_reason.as_deref(),
            Some("hs_sensitive_or_local_only")
        );
        assert_eq!(trace.parser_status, "not_attempted");
    }

    #[tokio::test]
    async fn main_chat_route_preview_skips_unvalidated_provider() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let config = configured_capability_config();
        {
            let mut state_config = state.config.lock().await;
            *state_config = config.clone();
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scheduler_for_config(&config, valid_json("direct_answer", 0.95));
        }
        let user_text = "Explain route preview in one sentence.";
        let (agent_decision, route_decision) = route_decision_for_text(user_text);
        let trace = preview_main_chat_turn_route_with_validation_record(
            &state,
            &[user_message(user_text)],
            &agent_decision,
            &route_decision,
            MainChatTurnStreamMode::Buffered,
            None,
        )
        .await;
        assert!(!trace.attempted);
        assert_eq!(
            trace.ignored_reason.as_deref(),
            Some("provider_unvalidated")
        );
    }

    #[tokio::test]
    async fn main_chat_route_preview_deterministic_fallback_parity_is_preserved() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let config = configured_capability_config();
        let record = crate::provider_validation::successful_provider_validation_record(
            &config,
            "route_preview_test",
            Utc::now(),
        );
        {
            let mut state_config = state.config.lock().await;
            *state_config = config.clone();
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scheduler_for_config(&config, valid_json("tool_loop", 0.93));
        }
        let user_text = "Explain route preview in one sentence.";
        let (agent_decision, route_decision) = route_decision_for_text(user_text);
        assert_eq!(route_decision.path, MainChatExecutionPath::KernelDirect);

        let trace = preview_main_chat_turn_route_with_validation_record(
            &state,
            &[user_message(user_text)],
            &agent_decision,
            &route_decision,
            MainChatTurnStreamMode::Buffered,
            Some(&record),
        )
        .await;

        assert!(trace.attempted);
        assert_eq!(trace.accepted_route.as_deref(), Some("tool_loop"));
        assert_eq!(trace.deterministic_route, "direct_answer");
        assert_eq!(trace.effective_route, "direct_answer");
        assert_eq!(route_decision.path, MainChatExecutionPath::KernelDirect);
    }

    #[tokio::test]
    async fn main_chat_route_preview_disabled_keeps_command_surface_semantics() {
        let cases = [
            MainChatCommandSurfaceEvalScenario::DirectProviderTrace,
            MainChatCommandSurfaceEvalScenario::FileReadSuccess,
            MainChatCommandSurfaceEvalScenario::PlanExecuteDraft,
            MainChatCommandSurfaceEvalScenario::ProposalPath,
            MainChatCommandSurfaceEvalScenario::WebPolicyBlocker,
            MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess,
            MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal,
        ];

        for scenario in cases {
            let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
            configure_main_chat_command_surface_eval_state(&state, scenario)
                .await
                .expect("configure command-surface route preview case");
            let response = send_message_with_state(
                format!("route-preview-disabled-{}", scenario.as_label()),
                vec![user_message(main_chat_command_surface_eval_user_text(
                    scenario,
                ))],
                None,
                &state,
            )
            .await
            .expect("send command-surface route preview case");
            let generation = response
                .reasoning_trace
                .generation_result
                .as_ref()
                .and_then(Value::as_object)
                .expect("generation metadata object");
            let preview = generation
                .get("routePreview")
                .and_then(Value::as_object)
                .expect("route preview trace");
            assert_eq!(
                preview.get("attempted").and_then(Value::as_bool),
                Some(false)
            );
            assert_eq!(
                preview.get("ignoredReason").and_then(Value::as_str),
                Some("runtime_mode_not_capability_first")
            );
            assert_eq!(
                generation
                    .get("legacyFallbackUsed")
                    .and_then(Value::as_bool),
                Some(false),
                "{}",
                scenario.as_label()
            );
            assert_eq!(
                generation
                    .get("directWritesExecuted")
                    .and_then(Value::as_bool),
                Some(false),
                "{}",
                scenario.as_label()
            );
            match scenario {
                MainChatCommandSurfaceEvalScenario::DirectProviderTrace => {
                    assert_eq!(
                        generation
                            .get("kernelBackedDirectAnswer")
                            .and_then(Value::as_bool),
                        Some(true)
                    );
                }
                MainChatCommandSurfaceEvalScenario::FileReadSuccess
                | MainChatCommandSurfaceEvalScenario::WebPolicyBlocker
                | MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess
                | MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal => {
                    assert_eq!(
                        generation
                            .get("kernelBackedReadOnlyToolLoop")
                            .and_then(Value::as_bool),
                        Some(true),
                        "{}",
                        scenario.as_label()
                    );
                }
                MainChatCommandSurfaceEvalScenario::PlanExecuteDraft => {
                    assert_eq!(
                        generation
                            .get("kernelBackedPlanExecuteDraft")
                            .and_then(Value::as_bool),
                        Some(true)
                    );
                }
                MainChatCommandSurfaceEvalScenario::ProposalPath => {
                    assert_eq!(
                        generation
                            .get("kernelBackedProposalOnlyWrite")
                            .and_then(Value::as_bool),
                        Some(true)
                    );
                }
                _ => {}
            }
            assert!(!response.legacy_fallback_used, "{}", scenario.as_label());
        }
    }
}
