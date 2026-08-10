use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::llm::ContextManifest;
use crate::network_client::{NetworkPolicyDecision, NetworkPolicyDisposition};

const A2A_MAX_REASONING_TRACE_BYTES: usize = 64 * 1024;
const A2A_MAX_RESULT_BYTES: usize = 128 * 1024;

// ========================================
// A2A Protocol Types (based on Google A2A draft)
// ========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub url: String,
    pub provider: Option<AgentProvider>,
    pub version: String,
    pub documentation_url: Option<String>,
    pub capabilities: AgentCapabilities,
    pub authentication: Option<AgentAuthentication>,
    pub default_input_modes: Vec<String>,
    pub default_output_modes: Vec<String>,
    pub skills: Vec<AgentSkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProvider {
    pub organization: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    pub streaming: bool,
    pub push_notifications: bool,
    pub state_transition_history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAuthentication {
    pub schemes: Vec<String>,
    pub credentials: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub examples: Option<Vec<String>>,
    pub input_modes: Option<Vec<String>>,
    pub output_modes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub session_id: Option<String>,
    #[serde(flatten)]
    pub status: Option<TaskStatus>,
    pub history: Option<Vec<Message>>,
    pub artifacts: Option<Vec<Artifact>>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatus {
    pub state: TaskState,
    pub message: Option<Message>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Cancelled,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: String, // "user" | "agent"
    pub parts: Vec<Part>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Part {
    Text { text: String },
    File { file: FileData },
    Data { data: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileData {
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub bytes: Option<String>, // base64
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parts: Vec<Part>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub index: Option<u32>,
    pub append: Option<bool>,
    pub last_chunk: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTaskRequest {
    pub id: String,
    pub session_id: Option<String>,
    pub message: Message,
    pub accepted_output_modes: Option<Vec<String>>,
    pub push_notification: Option<PushNotificationConfig>,
    pub history_length: Option<i32>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushNotificationConfig {
    pub url: String,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTaskResponse {
    pub id: String,
    pub status: TaskStatus,
    pub artifacts: Option<Vec<Artifact>>,
    pub history: Option<Vec<Message>>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2AErrorResponse {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// ========================================
// A2A Client
// ========================================

pub struct A2AClient {
    network: crate::network_client::NetworkClient,
    network_policy: crate::config::NetworkPolicy,
    network_policy_decision: Option<NetworkPolicyDecision>,
    bearer_token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A2AEndpointTransport {
    RemoteHttps,
    PairedLoopback,
}

impl A2AEndpointTransport {
    pub fn for_base_url(base_url: &str) -> anyhow::Result<Self> {
        let parsed = reqwest::Url::parse(base_url).context("a2a_invalid_base_url")?;
        if !parsed.username().is_empty() || parsed.password().is_some() {
            anyhow::bail!("a2a_url_userinfo_blocked");
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("a2a_url_host_missing"))?;
        let explicit_loopback = host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback());
        match (parsed.scheme(), explicit_loopback) {
            ("https", false) => Ok(Self::RemoteHttps),
            ("http" | "https", true) => Ok(Self::PairedLoopback),
            ("http", false) => anyhow::bail!("a2a_remote_https_required"),
            _ => anyhow::bail!("a2a_url_scheme_blocked"),
        }
    }
}

impl A2AClient {
    pub fn new() -> Self {
        Self::with_policy(crate::config::NetworkPolicy::default(), None)
    }

    pub fn with_policy(
        network_policy: crate::config::NetworkPolicy,
        bearer_token: Option<String>,
    ) -> Self {
        Self {
            network: crate::network_client::NetworkClient::new(
                crate::network_client::NetworkClientPolicy {
                    require_https: true,
                    max_body_bytes: 512 * 1024,
                    ..Default::default()
                },
            ),
            network_policy,
            network_policy_decision: None,
            bearer_token: bearer_token
                .map(|token| token.trim().to_string())
                .filter(|token| !token.is_empty()),
        }
    }

    pub fn with_authorized_edge(
        network_policy: crate::config::NetworkPolicy,
        network_policy_decision: NetworkPolicyDecision,
        bearer_token: Option<String>,
        transport: A2AEndpointTransport,
    ) -> anyhow::Result<Self> {
        if network_policy_decision.disposition != NetworkPolicyDisposition::Allow {
            anyhow::bail!("a2a_network_edge_not_authorized");
        }
        let bearer_token = bearer_token
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty());
        if bearer_token.as_ref().is_some_and(|token| {
            !(32..=4096).contains(&token.len()) || token.chars().any(char::is_control)
        }) {
            anyhow::bail!("a2a_bearer_token_invalid");
        }
        let paired_loopback = transport == A2AEndpointTransport::PairedLoopback;
        Ok(Self {
            network: crate::network_client::NetworkClient::new(
                crate::network_client::NetworkClientPolicy {
                    require_https: !paired_loopback,
                    allow_loopback: paired_loopback,
                    max_redirects: 0,
                    max_body_bytes: 512 * 1024,
                    ..Default::default()
                },
            ),
            network_policy,
            network_policy_decision: Some(network_policy_decision),
            bearer_token,
        })
    }

    pub fn public_card_url(base_url: &str) -> anyhow::Result<String> {
        a2a_endpoint_url(base_url, "/.well-known/agent.json")
    }

    pub fn private_card_url(base_url: &str) -> anyhow::Result<String> {
        a2a_endpoint_url(base_url, "/agent.json")
    }

    pub fn task_url(base_url: &str) -> anyhow::Result<String> {
        a2a_endpoint_url(base_url, "/tasks/send")
    }

    fn authorized_decision(&self) -> anyhow::Result<&NetworkPolicyDecision> {
        self.network_policy_decision
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("a2a_network_authorization_required"))
    }

    pub async fn discover_agent_card(&self, base_url: &str) -> anyhow::Result<AgentCard> {
        let url = Self::public_card_url(base_url)?;
        let decision = self.authorized_decision()?;
        let response = self
            .network
            .get_text_with_headers_for_decision(
                &url,
                &self.network_policy,
                decision,
                reqwest::header::HeaderMap::new(),
            )
            .await?;
        if !response.status.is_success() {
            anyhow::bail!("Failed to fetch agent card: {}", response.status);
        }
        let card: AgentCard = serde_json::from_str(&response.body)?;
        Ok(card)
    }

    pub async fn discover_private_agent_card(&self, base_url: &str) -> anyhow::Result<AgentCard> {
        let url = Self::private_card_url(base_url)?;
        let token = self
            .bearer_token
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("A2A private card authentication is required"))?;
        let decision = self.authorized_decision()?;
        let response = self
            .network
            .get_text_with_headers_for_decision(
                &url,
                &self.network_policy,
                decision,
                bearer_headers(token)?,
            )
            .await?;
        if !response.status.is_success() {
            anyhow::bail!("A2A private card failed with status {}", response.status);
        }
        serde_json::from_str(&response.body).context("a2a_private_card_decode_failed")
    }

    pub async fn send_task(
        &self,
        base_url: &str,
        req: &SendTaskRequest,
    ) -> anyhow::Result<SendTaskResponse> {
        self.send_task_with_start_observer(base_url, req, |_| async { Ok(()) })
            .await
    }

    /// Send one non-idempotent A2A task while exposing the exact HTTP dispatch
    /// edge. Validation, authentication and policy failures do not invoke the
    /// observer.
    pub async fn send_task_with_start_observer<F, Fut>(
        &self,
        base_url: &str,
        req: &SendTaskRequest,
        on_started: F,
    ) -> anyhow::Result<SendTaskResponse>
    where
        F: FnMut(crate::network_client::NetworkDispatchAttemptPhase) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<()>>,
    {
        let url = Self::task_url(base_url)?;
        let token = self
            .bearer_token
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("A2A task authentication is required"))?;
        validate_external_task_request(req)?;
        let decision = self.authorized_decision()?;
        let response = self
            .network
            .post_json_text_with_decision_and_start_observer(
                &url,
                &self.network_policy,
                decision,
                bearer_headers(token)?,
                &serde_json::to_value(req)?,
                on_started,
            )
            .await?;
        if !response.status.is_success() {
            anyhow::bail!("A2A task failed with status {}", response.status);
        }
        let resp: SendTaskResponse = serde_json::from_str(&response.body)?;
        validate_outbound_a2a_response(&resp, Some(&req.id)).map_err(anyhow::Error::msg)?;
        Ok(resp)
    }

    /// Convenience: build a text-only task request
    pub fn build_text_task(session_id: Option<String>, text: &str) -> SendTaskRequest {
        SendTaskRequest {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            message: Message {
                role: "user".into(),
                parts: vec![Part::Text { text: text.into() }],
                metadata: None,
            },
            accepted_output_modes: Some(vec!["text".into()]),
            push_notification: None,
            history_length: None,
            metadata: None,
        }
    }

    pub fn attach_context_manifest(
        req: &mut SendTaskRequest,
        context_manifest: ContextManifest,
    ) -> anyhow::Result<()> {
        if context_manifest.request_id != req.id {
            anyhow::bail!("a2a_context_manifest_request_mismatch");
        }
        validate_context_manifest(&context_manifest)?;
        req.metadata.get_or_insert_with(HashMap::new).insert(
            "contextManifest".into(),
            serde_json::to_value(context_manifest)
                .context("a2a_context_manifest_serialize_failed")?,
        );
        Ok(())
    }
}

impl Default for A2AClient {
    fn default() -> Self {
        Self::new()
    }
}

fn a2a_endpoint_url(base_url: &str, path: &str) -> anyhow::Result<String> {
    A2AEndpointTransport::for_base_url(base_url)?;
    let mut parsed = reqwest::Url::parse(base_url).context("a2a_invalid_base_url")?;
    if parsed.query().is_some() || parsed.fragment().is_some() {
        anyhow::bail!("a2a_base_url_query_or_fragment_blocked");
    }
    parsed.set_path(path);
    parsed.set_query(None);
    parsed.set_fragment(None);
    Ok(parsed.into())
}

fn bearer_headers(token: &str) -> anyhow::Result<reqwest::header::HeaderMap> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}")
            .parse()
            .context("invalid A2A authorization header")?,
    );
    Ok(headers)
}

fn validate_context_manifest(manifest: &ContextManifest) -> anyhow::Result<()> {
    if manifest.request_id.trim().is_empty() || manifest.privacy_decision_id.trim().is_empty() {
        anyhow::bail!("a2a_context_manifest_identity_missing");
    }
    if manifest.raw_life_model_included || manifest.raw_unbounded_memory_included {
        anyhow::bail!("a2a_context_manifest_unbounded_private_context_blocked");
    }
    if manifest.included_context_categories.is_empty() {
        anyhow::bail!("a2a_context_manifest_category_missing");
    }
    Ok(())
}

fn validate_a2a_context_manifest(req: &SendTaskRequest) -> anyhow::Result<ContextManifest> {
    let manifest: ContextManifest = serde_json::from_value(
        req.metadata
            .as_ref()
            .and_then(|metadata| metadata.get("contextManifest"))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("a2a_context_manifest_required"))?,
    )
    .context("a2a_context_manifest_invalid")?;
    if manifest.request_id != req.id {
        anyhow::bail!("a2a_context_manifest_request_mismatch");
    }
    validate_context_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_text_task_envelope(req: &SendTaskRequest) -> anyhow::Result<()> {
    if req.message.role != "user" {
        anyhow::bail!("a2a_task_role_must_be_user");
    }
    if req.message.metadata.is_some() {
        anyhow::bail!("a2a_message_metadata_blocked");
    }
    if req.message.parts.is_empty() {
        anyhow::bail!("a2a_task_text_missing");
    }
    let mut text_chars = 0usize;
    for part in &req.message.parts {
        match part {
            Part::Text { text } => {
                text_chars = text_chars.saturating_add(text.chars().count());
            }
            Part::File { .. } | Part::Data { .. } => {
                anyhow::bail!("a2a_non_text_task_part_blocked");
            }
        }
    }
    if text_chars == 0 || text_chars > 65_536 {
        anyhow::bail!("a2a_task_text_size_invalid");
    }
    if req.push_notification.is_some() {
        anyhow::bail!("a2a_push_notification_not_supported");
    }
    if req
        .accepted_output_modes
        .as_ref()
        .is_some_and(|modes| modes.iter().any(|mode| mode != "text"))
    {
        anyhow::bail!("a2a_output_mode_not_supported");
    }
    if req.history_length.is_some() {
        anyhow::bail!("a2a_history_request_not_supported");
    }
    if req.metadata.as_ref().is_some_and(|metadata| {
        metadata
            .keys()
            .any(|key| !matches!(key.as_str(), "skill" | "contextManifest"))
    }) {
        anyhow::bail!("a2a_task_metadata_key_blocked");
    }
    Ok(())
}

pub fn validate_external_task_request(req: &SendTaskRequest) -> anyhow::Result<()> {
    validate_text_task_envelope(req)?;
    validate_a2a_context_manifest(req)?;
    Ok(())
}

// ========================================
// A2A Server (in-process handlers, no HTTP server yet)
// ========================================

use crate::agent::context_assembler::AssembleOutput;
use crate::agent::types::{AgentTaskKind, ContextSummary, RedactionLevel};
use crate::agent::{LayeredReasoner, ReasoningInput, ReasoningStrategy, ReasoningTrace};
use crate::llm::ChatMessage;
use crate::privacy::PrivacyEngine;

pub struct A2AServerHandler {
    pub privacy_engine: PrivacyEngine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum A2AProviderTerminalState {
    NotAttempted,
    Completed,
    ConfirmedFailed,
    RemoteUnknown,
}

impl A2AProviderTerminalState {
    fn from_summary(summary: &crate::scheduler::ProviderReceiptSummary) -> Self {
        if summary.confirmed_failed_count > 0 {
            Self::ConfirmedFailed
        } else if summary.remote_unknown_count > 0 || summary.in_flight_count > 0 {
            Self::RemoteUnknown
        } else if summary.started_attempt_count == 0 {
            Self::NotAttempted
        } else if summary.completed_count == summary.started_attempt_count {
            Self::Completed
        } else {
            // A started attempt without a terminal is never completion.
            Self::RemoteUnknown
        }
    }
}

struct A2ATaskExecution {
    state: TaskState,
    text: String,
    reasoning_trace: Option<serde_json::Value>,
    provider_summary: crate::scheduler::ProviderReceiptSummary,
}

impl A2ATaskExecution {
    fn failed(reason_code: &str) -> Self {
        Self {
            state: TaskState::Failed,
            text: reason_code.to_string(),
            reasoning_trace: None,
            provider_summary: crate::scheduler::ProviderReceiptSummary::default(),
        }
    }

    fn reasoning_succeeded(
        reasoning_trace: serde_json::Value,
        provider_summary: crate::scheduler::ProviderReceiptSummary,
    ) -> Self {
        let provider_terminal_state = A2AProviderTerminalState::from_summary(&provider_summary);
        let state = match provider_terminal_state {
            A2AProviderTerminalState::ConfirmedFailed => TaskState::Failed,
            A2AProviderTerminalState::RemoteUnknown => TaskState::Unknown,
            A2AProviderTerminalState::NotAttempted | A2AProviderTerminalState::Completed => {
                TaskState::Completed
            }
        };
        Self {
            state,
            text: "structured_reasoning_result".into(),
            reasoning_trace: Some(reasoning_trace),
            provider_summary,
        }
    }

    fn reasoning_failed(
        reasoning_trace: serde_json::Value,
        provider_summary: crate::scheduler::ProviderReceiptSummary,
    ) -> Self {
        Self {
            state: TaskState::Failed,
            text: "structured_reasoning_failure".into(),
            reasoning_trace: Some(reasoning_trace),
            provider_summary,
        }
    }

    fn reasoning_unknown(
        reasoning_trace: serde_json::Value,
        provider_summary: crate::scheduler::ProviderReceiptSummary,
    ) -> Self {
        Self {
            state: TaskState::Unknown,
            text: "structured_reasoning_unknown".into(),
            reasoning_trace: Some(reasoning_trace),
            provider_summary,
        }
    }

    fn into_response(self, id: String) -> SendTaskResponse {
        let provider_terminal_state =
            A2AProviderTerminalState::from_summary(&self.provider_summary);
        let state = match (self.state, provider_terminal_state) {
            (TaskState::Completed, A2AProviderTerminalState::ConfirmedFailed) => TaskState::Failed,
            (TaskState::Completed, A2AProviderTerminalState::RemoteUnknown) => TaskState::Unknown,
            (state, _) => state,
        };
        let status_text = match state {
            TaskState::Completed => "a2a_task_completed".to_string(),
            TaskState::Failed => "reasoning_bridge_failed".to_string(),
            TaskState::Cancelled => "reasoning_bridge_cancelled".to_string(),
            _ => "reasoning_bridge_state_unknown".to_string(),
        };
        let mut evidence = HashMap::new();
        if let Some(trace) = self.reasoning_trace.clone() {
            evidence.insert("reasoningTrace".into(), trace);
        }
        evidence.insert(
            "providerReceiptSummary".into(),
            provider_receipt_summary_value(&self.provider_summary),
        );

        let artifact_parts = if self.reasoning_trace.is_some() {
            vec![
                Part::Text {
                    text: "structured_reasoning_result".into(),
                },
                Part::Data {
                    data: serde_json::json!({
                        "reasoningTraceRef": "response.metadata.reasoningTrace",
                        "providerReceiptSummaryRef": "response.metadata.providerReceiptSummary",
                    }),
                },
            ]
        } else {
            vec![Part::Text { text: self.text }]
        };
        SendTaskResponse {
            id,
            status: TaskStatus {
                state,
                message: Some(Message {
                    role: "agent".into(),
                    parts: vec![Part::Text { text: status_text }],
                    metadata: None,
                }),
            },
            artifacts: Some(vec![Artifact {
                name: Some("result".into()),
                description: None,
                parts: artifact_parts,
                metadata: None,
                index: None,
                append: None,
                last_chunk: Some(true),
            }]),
            history: None,
            metadata: Some(evidence),
        }
    }
}

fn metadata_ref_digest(value: &str) -> String {
    crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::Value::String(
        value.to_string(),
    ))
    .1
}

fn provider_receipt_summary_value(
    summary: &crate::scheduler::ProviderReceiptSummary,
) -> serde_json::Value {
    let details = summary
        .retained_receipts
        .iter()
        .take(16)
        .map(|receipt| {
            serde_json::json!({
                "requestIdDigest": metadata_ref_digest(&receipt.request_id),
                "providerDigest": metadata_ref_digest(&receipt.provider),
                "modelDigest": metadata_ref_digest(&receipt.model),
                "status": receipt.status,
                "startedAt": receipt.started_at,
                "finishedAt": receipt.finished_at,
                "errorDigest": receipt.error_digest,
                "simulated": receipt.simulated,
                "policyEvidenceDigest": receipt
                    .policy_evidence
                    .as_ref()
                    .and_then(|evidence| evidence.evidence_digest().ok()),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "startedAttemptCount": summary.started_attempt_count,
        "completedCount": summary.completed_count,
        "confirmedFailedCount": summary.confirmed_failed_count,
        "remoteUnknownCount": summary.remote_unknown_count,
        "inFlightCount": summary.in_flight_count,
        "retainedDetailCount": details.len(),
        "retainedDetails": details,
        "overflowCount": summary.overflow_count,
        "overflowDigest": summary.overflow_digest,
    })
}

impl A2AServerHandler {
    pub fn public_agent_card(port: u16) -> AgentCard {
        AgentCard {
            name: "OpenLife".into(),
            description: "Private A2A endpoint. Pairing is required for capabilities and tasks."
                .into(),
            url: format!("http://127.0.0.1:{port}"),
            provider: Some(AgentProvider {
                organization: "OpenLife".into(),
                url: "https://openlife.app".into(),
            }),
            version: "0.1.0".into(),
            documentation_url: None,
            capabilities: AgentCapabilities {
                streaming: false,
                push_notifications: false,
                state_transition_history: false,
            },
            authentication: Some(AgentAuthentication {
                schemes: vec!["bearer".into()],
                credentials: None,
            }),
            default_input_modes: vec!["text".into()],
            default_output_modes: vec!["text".into()],
            skills: Vec::new(),
        }
    }

    pub fn default_agent_card(port: u16) -> AgentCard {
        AgentCard {
            name: "OpenLife".into(),
            description: "Private OpenLife agent endpoint. Pairing is required for tasks.".into(),
            url: format!("http://127.0.0.1:{}", port),
            provider: Some(AgentProvider {
                organization: "OpenLife".into(),
                url: "https://openlife.app".into(),
            }),
            version: "0.1.0".into(),
            documentation_url: None,
            capabilities: AgentCapabilities {
                streaming: false,
                push_notifications: false,
                state_transition_history: false,
            },
            authentication: Some(AgentAuthentication {
                schemes: vec!["bearer".into()],
                credentials: None,
            }),
            default_input_modes: vec!["text".into()],
            default_output_modes: vec!["text".into()],
            skills: vec![AgentSkill {
                    id: "openlife.reasoning_bridge".into(),
                    name: "Reasoning Bridge".into(),
                    description: "Runs the Layered Reasoning Meaning→Strategy→Generation pipeline and returns a structured trace".into(),
                    tags: vec!["reasoning".into(), "decision".into()],
                    examples: None,
                    input_modes: None,
                    output_modes: None,
                }],
        }
    }

    pub async fn handle_task(&self, req: SendTaskRequest) -> SendTaskResponse {
        let skill_hint = req
            .metadata
            .as_ref()
            .and_then(|m| m.get("skill"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let execution = match skill_hint {
            "openlife.reasoning_bridge" => self.reasoning_bridge(&req).await,
            _ => A2ATaskExecution::failed("a2a_skill_not_authorized"),
        };
        execution.into_response(req.id)
    }

    async fn reasoning_bridge(&self, req: &SendTaskRequest) -> A2ATaskExecution {
        self.reasoning_bridge_with_runtime(
            req,
            crate::scheduler::InferenceScheduler::default(),
            crate::config::NetworkPolicy::default(),
        )
        .await
    }

    fn sanitized_trace_value(&self, trace: &ReasoningTrace) -> serde_json::Value {
        let serialized = serde_json::to_string(trace).unwrap_or_else(|_| "{}".into());
        let (sanitized, _) = self.privacy_engine.desensitize(&serialized);
        if sanitized.len() > A2A_MAX_REASONING_TRACE_BYTES {
            let (_, digest) = crate::agent::metadata_safe::metadata_safe_value_digest(
                &serde_json::Value::String(sanitized),
            );
            return serde_json::json!({
                "truncated": true,
                "traceDigest": digest,
                "errorCount": trace.errors.len(),
                "reason": "reasoning_trace_exceeded_response_limit",
            });
        }
        serde_json::from_str(&sanitized).unwrap_or_else(|_| {
            serde_json::json!({
                "errors": ["reasoning_trace_serialization_failed"],
            })
        })
    }

    async fn reasoning_bridge_with_runtime(
        &self,
        req: &SendTaskRequest,
        scheduler: crate::scheduler::InferenceScheduler,
        network_policy: crate::config::NetworkPolicy,
    ) -> A2ATaskExecution {
        let user_text = extract_text_from_message(&req.message);
        let session_id = req
            .session_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let reasoning_input = ReasoningInput {
            task_kind: AgentTaskKind::Conversation,
            user_text: user_text.clone(),
            session_id: session_id.clone(),
        };

        let (desensitized_user_text, _) = self.privacy_engine.desensitize(&user_text);
        let assemble_output = AssembleOutput {
            tools_prompt: String::new(),
            privacy_map: HashMap::new(),
            desensitized_messages: std::sync::Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: desensitized_user_text,
            }]),
            memory_context: String::new(),
            context_summary: ContextSummary {
                life_model_empty: true,
                included_life_model_sections: vec![],
                memory_hit_count: 0,
                memory_sources: vec![],
                used_tools_prompt: false,
                redaction_applied: false,
                redaction_level: RedactionLevel::None,
            },
            embed_error: None,
        };

        let run_id = uuid::Uuid::new_v4().to_string();
        let collector = crate::scheduler::ProviderReceiptCollector::default();
        let scheduler = scheduler.with_provider_receipt_collector(collector.clone());
        let policy_context = crate::agent::RuntimePolicyContext::fail_closed();
        let reasoner = LayeredReasoner::new(scheduler)
            .with_network_policy(network_policy)
            .with_provider_policy_context(
                policy_context.provider_authorization().clone(),
                policy_context.policy_provenance_refs().to_vec(),
            )
            .with_privacy_engine(self.privacy_engine.clone());
        match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            reasoner.reason(&reasoning_input, &assemble_output, &run_id),
        )
        .await
        {
            Ok(Ok(output)) => {
                let summary = collector.summary();
                let trace = self.sanitized_trace_value(&output.trace);
                A2ATaskExecution::reasoning_succeeded(trace, summary)
            }
            Ok(Err(error)) => {
                let summary = collector.summary();
                let (_, error_digest) = crate::agent::metadata_safe::metadata_safe_value_digest(
                    &serde_json::json!({ "error": error.to_string() }),
                );
                let mut trace = ReasoningTrace {
                    input: Some(reasoning_input.user_text.clone()),
                    ..ReasoningTrace::default()
                };
                trace.errors.push("reasoning_bridge_failed".into());
                trace.generation_result = Some(serde_json::json!({
                    "reasoningErrorDigest": error_digest,
                    "providerAttemptCount": summary.started_attempt_count,
                    "providerConfirmedFailedCount": summary.confirmed_failed_count,
                    "providerRemoteUnknownCount": summary.remote_unknown_count,
                }));
                let trace = self.sanitized_trace_value(&trace);
                A2ATaskExecution::reasoning_failed(trace, summary)
            }
            Err(_) => {
                collector.mark_in_flight_remote_unknown("reasoning_bridge_timeout");
                let summary = collector.summary();
                let mut trace = ReasoningTrace {
                    input: Some(reasoning_input.user_text),
                    ..ReasoningTrace::default()
                };
                trace.errors.push("reasoning_bridge_timeout".into());
                trace.generation_result = Some(serde_json::json!({
                    "providerAttemptCount": summary.started_attempt_count,
                    "providerConfirmedFailedCount": summary.confirmed_failed_count,
                    "providerRemoteUnknownCount": summary.remote_unknown_count,
                }));
                let trace = self.sanitized_trace_value(&trace);
                // The local timeout dropped an in-flight reasoning future.
                // Earlier phase receipts cannot prove the current remote
                // attempt stopped, so the A2A terminal state remains unknown.
                A2ATaskExecution::reasoning_unknown(trace, summary)
            }
        }
    }
}

fn extract_text_from_message(msg: &Message) -> String {
    msg.parts
        .iter()
        .filter_map(|p| match p {
            Part::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ========================================
// Reasoning <-> A2A Bridge helpers
// ========================================

pub fn reasoning_input_to_a2a_task(
    req: &ReasoningInput,
    skill: Option<&str>,
    tool_calls: Option<&serde_json::Value>,
) -> SendTaskRequest {
    let text = &req.user_text;
    let mut task = A2AClient::build_text_task(Some(req.session_id.clone()), text);
    if let Some(skill) = skill {
        task.metadata = Some({
            let mut m = HashMap::new();
            m.insert(
                "skill".to_string(),
                serde_json::Value::String(skill.to_string()),
            );
            m
        });
    }
    if let Some(tools) = tool_calls {
        task.metadata
            .get_or_insert_with(HashMap::new)
            .insert("tool_calls".to_string(), tools.clone());
    }
    task
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A2ACompletionTruth {
    /// The authenticated peer reported completion. OpenLife did not
    /// independently confirm the peer's downstream side effects.
    RemoteReportedCompleted,
}

#[derive(Debug, Clone)]
pub struct ValidatedA2AResponse {
    pub text: String,
    pub completion_truth: A2ACompletionTruth,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// The single outbound response authority used by both reasoning and tool
/// projections. A peer's terminal report is never upgraded to locally
/// confirmed completion.
pub fn validate_outbound_a2a_response(
    resp: &SendTaskResponse,
    expected_id: Option<&str>,
) -> Result<ValidatedA2AResponse, String> {
    if expected_id.is_some_and(|expected| expected != resp.id) {
        return Err("a2a_response_request_id_mismatch".into());
    }
    if resp.status.state != TaskState::Completed {
        let state = serde_json::to_value(resp.status.state)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "UNKNOWN".into());
        return Err(format!(
            "A2A task is not a completed reasoning success (state={state})"
        ));
    }

    let metadata = resp.metadata.clone().unwrap_or_default();
    let legacy_terminal_conflict = metadata
        .get("providerTerminalState")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|state| matches!(state, "failed" | "confirmed_failed" | "remote_unknown"));
    let summary_terminal_conflict = metadata
        .get("providerReceiptSummary")
        .is_some_and(|summary| {
            [
                "confirmedFailedCount",
                "remoteUnknownCount",
                "inFlightCount",
            ]
            .iter()
            .any(|key| {
                summary
                    .get(*key)
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    > 0
            })
        });
    let legacy_receipt_conflict = metadata
        .get("providerInvocationReceipts")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|receipts| {
            receipts.iter().any(|receipt| {
                receipt
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|status| matches!(status, "failed" | "remote_unknown"))
            })
        });
    if legacy_terminal_conflict || summary_terminal_conflict || legacy_receipt_conflict {
        return Err("a2a_completed_response_conflicts_with_terminal_evidence".into());
    }

    // Aggregate text from artifacts (primary) and status message (fallback)
    let artifact_text: String = resp
        .artifacts
        .as_ref()
        .map(|arts| {
            arts.iter()
                .flat_map(|art| &art.parts)
                .filter_map(|part| match part {
                    Part::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    let status_text = resp
        .status
        .message
        .as_ref()
        .and_then(|m| m.parts.first())
        .and_then(|part| match part {
            Part::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let text = if artifact_text.is_empty() {
        status_text
    } else {
        artifact_text
    };

    let text = if text.len() > A2A_MAX_RESULT_BYTES {
        let (_, digest) = crate::agent::metadata_safe::metadata_safe_value_digest(
            &serde_json::Value::String(text),
        );
        serde_json::json!({
            "truncated": true,
            "resultDigest": digest,
            "reason": "a2a_result_exceeded_response_limit",
        })
        .to_string()
    } else {
        text
    };

    Ok(ValidatedA2AResponse {
        text,
        completion_truth: A2ACompletionTruth::RemoteReportedCompleted,
        metadata,
    })
}

pub fn a2a_response_to_reasoning_result(
    resp: &SendTaskResponse,
) -> Result<serde_json::Value, String> {
    let validated = validate_outbound_a2a_response(resp, None)?;

    // Build a reasoning-compatible result object
    let result = serde_json::json!({
        "text": validated.text,
        "state": "remote_reported_completed",
        "remoteState": resp.status.state,
        "status": {
            "state": resp.status.state,
            "message": resp.status.message.as_ref().and_then(|m| m.parts.first()).and_then(|part| match part {
                Part::Text { text } => Some(text.clone()),
                _ => None,
            }),
        },
        "metadata": validated.metadata,
    });

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentTaskKind, ReasoningInput};

    fn allowed_a2a_policy(capability: &str) -> crate::config::NetworkPolicy {
        crate::config::NetworkPolicy {
            tool_overrides: HashMap::from([(capability.to_string(), "allow".into())]),
            ..Default::default()
        }
    }

    fn bounded_task_context(request_id: &str) -> ContextManifest {
        ContextManifest {
            request_id: request_id.into(),
            privacy_decision_id: format!("a2a-context:{request_id}"),
            selected_context_refs: vec![format!("a2a-task:{request_id}")],
            included_context_categories: vec!["current_authenticated_user_message".into()],
            declared_payload_categories: vec![
                crate::llm::ProviderPayloadCategory::A2aAuthenticatedUserMessage,
            ],
            policy_provenance_refs: Vec::new(),
            raw_life_model_included: false,
            raw_unbounded_memory_included: false,
        }
    }

    #[tokio::test]
    async fn paired_loopback_task_requires_auth_and_transmits_bounded_context_manifest() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let task_url = A2AClient::task_url(&base_url).unwrap();
        let capability = "a2a.task";
        let policy = allowed_a2a_policy(capability);
        let decision =
            crate::network_client::resolve_network_policy_decision(&policy, &task_url, capability)
                .unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = socket.read(&mut buffer).await.unwrap();
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
                if let Some(headers_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let headers_end = headers_end + 4;
                    let header_text = String::from_utf8_lossy(&bytes[..headers_end]);
                    let content_length = header_text
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .or_else(|| line.strip_prefix("Content-Length: "))
                        })
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if bytes.len() >= headers_end + content_length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.contains("authorization: Bearer paired-secret-01234567890123456789"));
            assert!(request.contains("\"contextManifest\""));
            assert!(request.contains("current_authenticated_user_message"));
            assert!(!request.contains("rawLifeModel"));
            let body = serde_json::json!({
                "id": "task-authenticated",
                "status": {"state": "COMPLETED", "message": null},
                "artifacts": null,
                "history": null,
                "metadata": null
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let client = A2AClient::with_authorized_edge(
            policy,
            decision,
            Some("paired-secret-01234567890123456789".into()),
            A2AEndpointTransport::PairedLoopback,
        )
        .unwrap();
        let mut task = A2AClient::build_text_task(None, "hello paired agent");
        task.id = "task-authenticated".into();
        let request_id = task.id.clone();
        A2AClient::attach_context_manifest(&mut task, bounded_task_context(&request_id)).unwrap();
        let response = client.send_task(&base_url, &task).await.unwrap();

        assert_eq!(response.status.state, TaskState::Completed);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn unauthenticated_task_fails_before_loopback_dispatch() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let task_url = A2AClient::task_url(&base_url).unwrap();
        let capability = "a2a.task";
        let policy = allowed_a2a_policy(capability);
        let decision =
            crate::network_client::resolve_network_policy_decision(&policy, &task_url, capability)
                .unwrap();
        let client = A2AClient::with_authorized_edge(
            policy,
            decision,
            None,
            A2AEndpointTransport::PairedLoopback,
        )
        .unwrap();
        let mut task = A2AClient::build_text_task(None, "must not dispatch");
        let request_id = task.id.clone();
        A2AClient::attach_context_manifest(&mut task, bounded_task_context(&request_id)).unwrap();

        let error = client.send_task(&base_url, &task).await.unwrap_err();
        assert!(error.to_string().contains("authentication is required"));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    }

    #[test]
    fn remote_plain_http_and_unbounded_context_are_rejected() {
        assert!(A2AEndpointTransport::for_base_url("http://example.com")
            .unwrap_err()
            .to_string()
            .contains("https_required"));

        let mut task = A2AClient::build_text_task(None, "private context");
        let manifest = ContextManifest {
            raw_life_model_included: true,
            ..bounded_task_context(&task.id)
        };
        assert!(A2AClient::attach_context_manifest(&mut task, manifest)
            .unwrap_err()
            .to_string()
            .contains("unbounded_private_context_blocked"));
    }

    #[test]
    fn external_task_rejects_file_data_and_unlisted_metadata() {
        let mut task = A2AClient::build_text_task(None, "bounded text");
        task.message.parts.push(Part::Data {
            data: serde_json::json!({"workspace": "must-not-cross"}),
        });
        assert!(validate_text_task_envelope(&task)
            .unwrap_err()
            .to_string()
            .contains("non_text_task_part_blocked"));

        let mut task = A2AClient::build_text_task(None, "bounded text");
        task.metadata = Some(HashMap::from([(
            "rawWorkspace".into(),
            serde_json::json!({"private": true}),
        )]));
        assert!(validate_text_task_envelope(&task)
            .unwrap_err()
            .to_string()
            .contains("metadata_key_blocked"));
    }

    #[tokio::test]
    async fn missing_or_unknown_skill_fails_without_falling_back_to_life_model_query() {
        let handler = A2AServerHandler {
            privacy_engine: PrivacyEngine::new(),
        };
        let request = A2AClient::build_text_task(None, "untrusted peer request");
        let response = handler.handle_task(request).await;

        assert_eq!(response.status.state, TaskState::Failed);
        assert_eq!(
            response.metadata.unwrap()["providerReceiptSummary"]["startedAttemptCount"],
            0
        );
        let body = serde_json::to_string(&response.artifacts).unwrap();
        assert!(body.contains("a2a_skill_not_authorized"));
        assert!(!body.contains("identity"));
        assert!(!body.contains("goals"));
    }

    #[test]
    fn reasoning_trace_projection_is_bounded_and_digest_only_when_oversized() {
        let handler = A2AServerHandler {
            privacy_engine: PrivacyEngine::new(),
        };
        let mut trace = ReasoningTrace {
            input: Some("private-trace-content".repeat(10_000)),
            ..ReasoningTrace::default()
        };
        trace
            .errors
            .push("private-provider-error-body-must-not-return".into());
        let projected = handler.sanitized_trace_value(&trace);
        let serialized = serde_json::to_vec(&projected).unwrap();

        assert!(projected["truncated"].as_bool().unwrap_or(false));
        assert!(projected["traceDigest"].as_str().is_some());
        assert!(serialized.len() < 4096);
        assert!(!String::from_utf8(serialized)
            .unwrap()
            .contains("private-trace-content"));
        assert!(!serde_json::to_string(&projected)
            .unwrap()
            .contains("private-provider-error-body-must-not-return"));
        assert_eq!(projected["errorCount"], 1);
    }

    fn provider_receipt(
        request_id: &str,
        status: crate::llm::ProviderInvocationStatus,
    ) -> crate::llm::ProviderInvocationReceipt {
        let started_at = chrono::Utc::now();
        crate::llm::ProviderInvocationReceipt {
            request_id: request_id.into(),
            provider: "openai".into(),
            model: "gpt-test".into(),
            status,
            started_at,
            finished_at: started_at + chrono::Duration::milliseconds(1),
            error_digest: (status != crate::llm::ProviderInvocationStatus::Completed)
                .then(|| format!("sha256:{}", "0".repeat(64))),
            simulated: false,
            policy_evidence: None,
        }
    }

    #[test]
    fn earlier_failed_provider_attempt_cannot_be_hidden_by_later_completion() {
        let execution = A2ATaskExecution::reasoning_succeeded(
            serde_json::json!({"output": "bounded-success-trace"}),
            crate::scheduler::ProviderReceiptSummary::from_receipts(vec![
                provider_receipt(
                    "strategy-failed",
                    crate::llm::ProviderInvocationStatus::Failed,
                ),
                provider_receipt(
                    "generation-completed",
                    crate::llm::ProviderInvocationStatus::Completed,
                ),
            ]),
        );
        let response = execution.into_response("a2a-mixed-provider-outcome".into());

        assert_eq!(response.status.state, TaskState::Failed);
        let metadata = response.metadata.unwrap();
        assert_eq!(metadata["providerReceiptSummary"]["retainedDetailCount"], 2);
        assert_eq!(
            metadata["providerReceiptSummary"]["confirmedFailedCount"],
            1
        );
    }

    #[test]
    fn timed_out_reasoning_stays_unknown_after_an_earlier_completed_phase() {
        let execution = A2ATaskExecution::reasoning_unknown(
            serde_json::json!({"errors": ["reasoning_bridge_timeout"]}),
            crate::scheduler::ProviderReceiptSummary::from_receipts(vec![provider_receipt(
                "strategy-completed",
                crate::llm::ProviderInvocationStatus::Completed,
            )]),
        );
        let response = execution.into_response("a2a-timeout".into());

        assert_eq!(response.status.state, TaskState::Unknown);
        assert_eq!(
            response.metadata.unwrap()["providerReceiptSummary"]["completedCount"],
            1
        );
    }

    #[tokio::test]
    async fn reasoning_bridge_pre_dispatch_failure_retains_a_bounded_trace() {
        let scheduler = crate::scheduler::InferenceScheduler::new(
            String::new(),
            false,
            "openai".into(),
            "https://capture.invalid/v1".into(),
            "sk-test".into(),
            "gpt-test".into(),
            String::new(),
            false,
        )
        .with_scripted_generation_response("must-not-run");
        let handler = A2AServerHandler {
            privacy_engine: PrivacyEngine::new(),
        };
        let mut req = A2AClient::build_text_task(Some("a2a-failure".into()), "reason");
        req.metadata = Some(HashMap::from([(
            "skill".into(),
            serde_json::Value::String("openlife.reasoning_bridge".into()),
        )]));
        let execution = handler
            .reasoning_bridge_with_runtime(
                &req,
                scheduler,
                crate::config::NetworkPolicy {
                    default_decision: "allow".into(),
                    ..Default::default()
                },
            )
            .await;
        let response = execution.into_response(req.id.clone());

        assert!(matches!(response.status.state, TaskState::Failed));
        assert!(!matches!(response.status.state, TaskState::Completed));
        let metadata = response.metadata.expect("failure evidence metadata");
        assert!(metadata["reasoningTrace"]["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty()));
        assert_eq!(metadata["providerReceiptSummary"]["startedAttemptCount"], 0);
    }

    #[tokio::test]
    async fn scripted_reasoning_success_does_not_claim_a_provider_invocation() {
        let scheduler = crate::scheduler::InferenceScheduler::new(
            "local-model".into(),
            true,
            "openai".into(),
            "https://capture.invalid/v1".into(),
            "sk-test".into(),
            "gpt-test".into(),
            String::new(),
            false,
        )
        .with_scripted_generation_response("下一步 fixture");
        let handler = A2AServerHandler {
            privacy_engine: PrivacyEngine::new(),
        };
        let req = A2AClient::build_text_task(Some("a2a-scripted".into()), "reason");
        let execution = handler
            .reasoning_bridge_with_runtime(&req, scheduler, crate::config::NetworkPolicy::default())
            .await;
        let response = execution.into_response(req.id);

        assert!(matches!(response.status.state, TaskState::Completed));
        let metadata = response.metadata.unwrap();
        assert_eq!(metadata["providerReceiptSummary"]["startedAttemptCount"], 0);
    }

    #[test]
    fn build_text_task_basic() {
        let task = A2AClient::build_text_task(Some("sid-123".into()), "hello");
        assert_eq!(task.session_id, Some("sid-123".into()));
        assert_eq!(task.message.role, "user");
        let text = extract_text_from_message(&task.message);
        assert_eq!(text, "hello");
    }

    #[test]
    fn dev_agent_card_exposes_only_bounded_reasoning_skill() {
        let card = A2AServerHandler::default_agent_card(8766);
        assert_eq!(card.skills.len(), 1);
        assert_eq!(card.skills[0].id, "openlife.reasoning_bridge");
        let serialized = serde_json::to_string(&card).unwrap();
        assert!(!serialized.contains("query_life_model"));
        assert!(!serialized.contains("assess_values"));
    }

    #[test]
    fn reasoning_input_to_a2a_task_maps_text_and_metadata() {
        let req = ReasoningInput {
            task_kind: AgentTaskKind::Conversation,
            user_text: "do something".to_string(),
            session_id: "sess-42".to_string(),
        };
        let task = reasoning_input_to_a2a_task(
            &req,
            Some("coding"),
            Some(&serde_json::json!([{"name": "tool1"}])),
        );
        assert_eq!(task.session_id, Some("sess-42".into()));
        let meta = task.metadata.as_ref().unwrap();
        assert_eq!(meta.get("skill").unwrap().as_str().unwrap(), "coding");
        assert!(meta.get("tool_calls").is_some());
    }

    #[test]
    fn a2a_response_to_reasoning_result_prefers_artifacts() {
        let resp = SendTaskResponse {
            id: "task-1".into(),
            status: TaskStatus {
                state: TaskState::Completed,
                message: Some(Message {
                    role: "agent".into(),
                    parts: vec![Part::Text {
                        text: "status text".into(),
                    }],
                    metadata: None,
                }),
            },
            artifacts: Some(vec![Artifact {
                name: Some("result".into()),
                description: None,
                parts: vec![Part::Text {
                    text: "artifact text".into(),
                }],
                metadata: None,
                index: None,
                append: None,
                last_chunk: None,
            }]),
            history: None,
            metadata: Some({
                let mut m = HashMap::new();
                m.insert("key".into(), serde_json::Value::String("value".into()));
                m
            }),
        };
        let result = a2a_response_to_reasoning_result(&resp).unwrap();
        assert_eq!(result["text"].as_str().unwrap(), "artifact text");
        assert_eq!(result["state"], "remote_reported_completed");
        assert_eq!(result["metadata"]["key"].as_str().unwrap(), "value");
    }

    #[test]
    fn a2a_nonterminal_response_cannot_be_projected_as_reasoning_success() {
        let resp = SendTaskResponse {
            id: "task-2".into(),
            status: TaskStatus {
                state: TaskState::Working,
                message: Some(Message {
                    role: "agent".into(),
                    parts: vec![Part::Text {
                        text: "working on it".into(),
                    }],
                    metadata: None,
                }),
            },
            artifacts: None,
            history: None,
            metadata: None,
        };
        let error = a2a_response_to_reasoning_result(&resp).unwrap_err();
        assert!(error.contains("WORKING"));
    }

    #[test]
    fn a2a_failed_task_cannot_be_projected_as_reasoning_success() {
        let resp = SendTaskResponse {
            id: "task-failed".into(),
            status: TaskStatus {
                state: TaskState::Failed,
                message: Some(Message {
                    role: "agent".into(),
                    parts: vec![Part::Text {
                        text: "reasoning_bridge_failed".into(),
                    }],
                    metadata: None,
                }),
            },
            artifacts: None,
            history: None,
            metadata: Some(HashMap::from([(
                "providerTerminalState".into(),
                serde_json::Value::String("failed".into()),
            )])),
        };

        let error = a2a_response_to_reasoning_result(&resp).unwrap_err();
        assert!(error.contains("FAILED"));
    }

    #[test]
    fn completed_peer_response_with_unknown_evidence_is_rejected() {
        let resp = SendTaskResponse {
            id: "task-contradiction".into(),
            status: TaskStatus {
                state: TaskState::Completed,
                message: None,
            },
            artifacts: None,
            history: None,
            metadata: Some(HashMap::from([(
                "providerReceiptSummary".into(),
                serde_json::json!({
                    "startedAttemptCount": 1,
                    "completedCount": 0,
                    "confirmedFailedCount": 0,
                    "remoteUnknownCount": 1,
                    "inFlightCount": 0,
                }),
            )])),
        };

        let error = validate_outbound_a2a_response(&resp, Some("task-contradiction"))
            .expect_err("completed cannot override remote-unknown evidence");
        assert!(error.contains("conflicts_with_terminal_evidence"));
    }

    #[test]
    fn failed_unknown_and_cancelled_peer_states_never_validate_as_tool_success() {
        for state in [TaskState::Failed, TaskState::Unknown, TaskState::Cancelled] {
            let resp = SendTaskResponse {
                id: "task-terminal-not-success".into(),
                status: TaskStatus {
                    state,
                    message: None,
                },
                artifacts: None,
                history: None,
                metadata: None,
            };
            assert!(
                validate_outbound_a2a_response(&resp, Some("task-terminal-not-success")).is_err()
            );
        }
    }

    #[test]
    fn receipt_summary_is_bounded_and_keeps_early_failure_sticky() {
        let mut receipts = vec![provider_receipt(
            "early-failure",
            crate::llm::ProviderInvocationStatus::Failed,
        )];
        receipts.extend((0..40).map(|index| {
            provider_receipt(
                &format!("later-completion-{index}"),
                crate::llm::ProviderInvocationStatus::Completed,
            )
        }));
        let summary = crate::scheduler::ProviderReceiptSummary::from_receipts(receipts);
        let projected = provider_receipt_summary_value(&summary);

        assert_eq!(projected["confirmedFailedCount"], 1);
        assert_eq!(projected["retainedDetailCount"], 16);
        assert_eq!(projected["overflowCount"], 25);
        assert!(projected["overflowDigest"].as_str().is_some());
        assert!(serde_json::to_vec(&projected).unwrap().len() < 32 * 1024);
    }

    #[test]
    fn extract_text_from_message_joins_parts() {
        let msg = Message {
            role: "user".into(),
            parts: vec![
                Part::Text {
                    text: "hello".into(),
                },
                Part::Text {
                    text: "world".into(),
                },
            ],
            metadata: None,
        };
        assert_eq!(extract_text_from_message(&msg), "hello world");
    }
}
