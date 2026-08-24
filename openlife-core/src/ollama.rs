use crate::llm::{ChatMessage, StreamResult};
use anyhow::{Context, Result};
use futures::StreamExt;
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::pin::Pin;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaChatResponse {
    pub message: OllamaMessage,
    pub done: bool,
}

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, OnceLock,
};
use std::time::{Duration, Instant};

const DEFAULT_OLLAMA_IPV4_BASE_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_OLLAMA_LOCALHOST_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_OLLAMA_IPV6_BASE_URL: &str = "http://[::1]:11434";
const OLLAMA_BASE_ENV_KEYS: [&str; 2] = ["OPENLIFE_OLLAMA_BASE_URL", "OLLAMA_HOST"];
const OLLAMA_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const OLLAMA_CHAT_TIMEOUT: Duration = Duration::from_secs(120);
// The bounded conversation projection plus Main Chat's system/context prompt
// must fit without relying on Ollama's smaller process default, which silently
// drops the oldest messages (including the conversation summary).
const OLLAMA_CHAT_CONTEXT_TOKENS: usize = 32_768;
const OLLAMA_STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const OLLAMA_MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const OLLAMA_MAX_STREAM_FRAME_BYTES: usize = 256 * 1024;
const OLLAMA_MAX_STREAM_TOTAL_BYTES: usize = 4 * 1024 * 1024;
const OLLAMA_INSPECTION_DEDUP_WINDOW: Duration = Duration::from_secs(1);

struct OllamaCache {
    checked_at: Instant,
    model: String,
    resolved_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaStatus {
    pub server_online: bool,
    pub resolved_model: Option<String>,
    pub models: Vec<(String, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OllamaEmbeddingModelIdentity {
    pub model: String,
    pub digest: String,
}

/// One immutable local chat deployment selected before content dispatch.
/// Scheduler-owned prepared requests retain the exact final `/api/chat` URL;
/// the adapter must not rediscover it from process environment or cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedOllamaChatTarget {
    pub model: String,
    pub endpoint: String,
}

struct OllamaDeploymentSnapshot {
    base_url: String,
    models: Vec<(String, u64)>,
}

static OLLAMA_CACHE: Mutex<Option<OllamaCache>> = Mutex::new(None);
static OLLAMA_CACHE_TTL_SECONDS: AtomicU64 = AtomicU64::new(10);

struct OllamaInspectionFlight {
    result: tokio::sync::Mutex<Option<OllamaStatus>>,
    notify: tokio::sync::Notify,
}

struct OllamaInspectionCompleted {
    key: String,
    checked_at: Instant,
    status: OllamaStatus,
}

#[derive(Default)]
struct OllamaInspectionCoordinator {
    in_flight: HashMap<String, Arc<OllamaInspectionFlight>>,
    completed: Option<OllamaInspectionCompleted>,
}

static OLLAMA_INSPECTION_COORDINATOR: OnceLock<tokio::sync::Mutex<OllamaInspectionCoordinator>> =
    OnceLock::new();

fn ollama_inspection_coordinator() -> &'static tokio::sync::Mutex<OllamaInspectionCoordinator> {
    OLLAMA_INSPECTION_COORDINATOR
        .get_or_init(|| tokio::sync::Mutex::new(OllamaInspectionCoordinator::default()))
}

/// Set the Ollama cache TTL in seconds.
pub fn set_ollama_cache_ttl_seconds(seconds: u64) {
    OLLAMA_CACHE_TTL_SECONDS.store(seconds, Ordering::Relaxed);
}

fn get_ollama_cache_ttl() -> Duration {
    Duration::from_secs(OLLAMA_CACHE_TTL_SECONDS.load(Ordering::Relaxed))
}

fn ollama_base_url_candidates() -> Vec<String> {
    for key in OLLAMA_BASE_ENV_KEYS {
        match std::env::var(key) {
            Ok(value) => return ollama_base_url_candidates_for(Some(&value)),
            Err(std::env::VarError::NotPresent) => continue,
            Err(std::env::VarError::NotUnicode(_)) => return Vec::new(),
        }
    }
    ollama_base_url_candidates_for(None)
}

fn ollama_base_url_candidates_for(explicit_base: Option<&str>) -> Vec<String> {
    if let Some(explicit_base) = explicit_base {
        return normalize_ollama_base_url(explicit_base)
            .into_iter()
            .collect();
    }

    vec![
        DEFAULT_OLLAMA_IPV4_BASE_URL.to_string(),
        DEFAULT_OLLAMA_LOCALHOST_BASE_URL.to_string(),
        DEFAULT_OLLAMA_IPV6_BASE_URL.to_string(),
    ]
}

fn normalize_ollama_base_url(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let candidate = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let parsed = reqwest::Url::parse(&candidate).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return None;
    }
    let host = parsed.host_str()?;
    let normalized_host = host.trim_start_matches('[').trim_end_matches(']');
    let is_loopback = normalized_host.eq_ignore_ascii_case("localhost")
        || normalized_host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !is_loopback {
        return None;
    }
    Some(candidate)
}

/// Resolve exactly one embedding deployment. An explicitly configured invalid
/// value fails closed and never falls through to an implicit localhost route.
pub(crate) fn configured_ollama_embedding_base_url() -> Result<String> {
    for key in OLLAMA_BASE_ENV_KEYS {
        match std::env::var(key) {
            Ok(value) => {
                return normalize_ollama_base_url(&value)
                    .ok_or_else(|| anyhow::anyhow!("ollama_embedding_endpoint_invalid"));
            }
            Err(std::env::VarError::NotPresent) => continue,
            Err(std::env::VarError::NotUnicode(_)) => {
                anyhow::bail!("ollama_embedding_endpoint_invalid")
            }
        }
    }
    Ok(DEFAULT_OLLAMA_IPV4_BASE_URL.to_string())
}

fn ollama_api_url(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn ollama_http_client(timeout: Duration) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(OLLAMA_CONNECT_TIMEOUT)
        .timeout(timeout)
        .build()
        .context("build loopback-only Ollama HTTP client")
}

async fn read_bounded_ollama_response(response: reqwest::Response) -> Result<String> {
    if response
        .content_length()
        .is_some_and(|length| length > OLLAMA_MAX_RESPONSE_BYTES as u64)
    {
        anyhow::bail!("ollama_response_body_too_large");
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("ollama_response_read_failed")?;
        if body.len().saturating_add(chunk.len()) > OLLAMA_MAX_RESPONSE_BYTES {
            anyhow::bail!("ollama_response_body_too_large");
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).context("ollama_response_utf8_invalid")
}

fn ollama_error_body_digest(body: &str) -> String {
    let hash = digest(&SHA256, body.as_bytes());
    let hex = hash
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

/// Check if Ollama is reachable and the requested model is available.
pub async fn is_ollama_available(model: &str) -> bool {
    resolve_ollama_model(model).await.is_some()
}

/// Check if the Ollama HTTP service is reachable, regardless of the selected model.
pub async fn is_ollama_server_online() -> bool {
    fetch_ollama_models_from_server().await.is_some()
}

/// Fetch the list of installed Ollama models for UI display.
pub async fn list_ollama_models() -> Vec<(String, u64)> {
    fetch_ollama_models_from_server().await.unwrap_or_default()
}

pub async fn inspect_ollama_status(model: &str) -> OllamaStatus {
    inspect_ollama_status_for_generation(model, "standalone").await
}

/// Coalesce passive status probes only inside one immutable product runtime
/// generation. A config replacement cannot inherit an earlier generation's
/// completed or in-flight observation even when the model label is unchanged.
pub async fn inspect_ollama_status_for_generation(
    model: &str,
    runtime_generation: &str,
) -> OllamaStatus {
    let model = model.to_string();
    let key = serde_json::to_string(&(
        runtime_generation.trim(),
        model.trim(),
        ollama_base_url_candidates(),
    ))
    .expect("Ollama inspection generation key");
    let flight = {
        let mut coordinator = ollama_inspection_coordinator().lock().await;
        if let Some(completed) = coordinator.completed.as_ref() {
            if completed.key == key
                && completed.checked_at.elapsed() < OLLAMA_INSPECTION_DEDUP_WINDOW
            {
                return completed.status.clone();
            }
        }
        if let Some(flight) = coordinator.in_flight.get(&key) {
            Arc::clone(flight)
        } else {
            let flight = Arc::new(OllamaInspectionFlight {
                result: tokio::sync::Mutex::new(None),
                notify: tokio::sync::Notify::new(),
            });
            coordinator
                .in_flight
                .insert(key.clone(), Arc::clone(&flight));
            let task_flight = Arc::clone(&flight);
            let task_key = key.clone();
            tokio::spawn(async move {
                let status = inspect_ollama_status_uncached(&model).await;
                *task_flight.result.lock().await = Some(status.clone());
                let mut coordinator = ollama_inspection_coordinator().lock().await;
                coordinator.completed = Some(OllamaInspectionCompleted {
                    key: task_key.clone(),
                    checked_at: Instant::now(),
                    status,
                });
                if coordinator
                    .in_flight
                    .get(&task_key)
                    .is_some_and(|current| Arc::ptr_eq(current, &task_flight))
                {
                    coordinator.in_flight.remove(&task_key);
                }
                drop(coordinator);
                task_flight.notify.notify_waiters();
            });
            flight
        }
    };
    loop {
        let notified = flight.notify.notified();
        if let Some(status) = flight.result.lock().await.clone() {
            return status;
        }
        notified.await;
    }
}

async fn inspect_ollama_status_uncached(model: &str) -> OllamaStatus {
    match fetch_ollama_models_from_server().await {
        Some(models) => OllamaStatus {
            server_online: true,
            resolved_model: resolve_ollama_model_from_models(model, &models),
            models,
        },
        None => OllamaStatus {
            server_online: false,
            resolved_model: None,
            models: Vec::new(),
        },
    }
}

async fn fetch_ollama_models_from_server() -> Option<Vec<(String, u64)>> {
    fetch_ollama_deployment_from_candidates(ollama_base_url_candidates(), None)
        .await
        .map(|deployment| deployment.models)
}

async fn fetch_ollama_deployment_from_candidates(
    candidates: Vec<String>,
    required_model: Option<&str>,
) -> Option<OllamaDeploymentSnapshot> {
    let client = match ollama_http_client(Duration::from_secs(2)) {
        Ok(c) => c,
        Err(_) => return None,
    };
    for base_url in candidates {
        let res = client
            .get(ollama_api_url(&base_url, "api/tags"))
            .send()
            .await;
        if let Ok(r) = res {
            if r.status().is_success() {
                if let Ok(text) = read_bounded_ollama_response(r).await {
                    if let Ok(body) = serde_json::from_str::<serde_json::Value>(&text) {
                        if body.get("models").is_some_and(serde_json::Value::is_array) {
                            let models = parse_ollama_models_from_tags_body(&body);
                            if required_model.is_some_and(|required_model| {
                                resolve_ollama_model_from_models(required_model, &models).is_none()
                            }) {
                                continue;
                            }
                            return Some(OllamaDeploymentSnapshot { base_url, models });
                        }
                    }
                }
            }
        }
    }
    None
}

/// Resolve model and endpoint from one captured candidate set. This path is
/// intentionally uncached: the returned endpoint is part of the turn's
/// execution snapshot, whereas the passive availability cache stores only UI
/// diagnostics and must never authorize adapter routing.
pub(crate) async fn prepare_ollama_chat_target(
    requested_model: &str,
) -> Option<PreparedOllamaChatTarget> {
    let candidates = ollama_base_url_candidates();
    let deployment =
        fetch_ollama_deployment_from_candidates(candidates, Some(requested_model)).await?;
    let model = resolve_ollama_model_from_models(requested_model, &deployment.models)?;
    Some(PreparedOllamaChatTarget {
        model,
        endpoint: ollama_api_url(&deployment.base_url, "api/chat"),
    })
}

pub(crate) fn validate_prepared_ollama_chat_endpoint(endpoint: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(endpoint)
        .context("prepared Ollama chat endpoint is not a valid URL")?;
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("prepared Ollama chat endpoint has no host"))?;
    let normalized_host = host.trim_start_matches('[').trim_end_matches(']');
    let is_loopback = normalized_host.eq_ignore_ascii_case("localhost")
        || normalized_host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !matches!(parsed.scheme(), "http" | "https")
        || !is_loopback
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path().trim_end_matches('/') != "/api/chat"
    {
        anyhow::bail!("prepared Ollama chat endpoint is outside the loopback chat boundary");
    }
    Ok(())
}

/// Resolve one loopback deployment before any content-bearing chat request is
/// built. Discovery may probe multiple loopback addresses with `/api/tags`, but
/// a logical chat attempt is forever bound to the first valid deployment. A
/// failed or ambiguous `/api/chat` dispatch must never switch endpoints.
async fn resolve_ollama_chat_base_url_from_candidates(
    client: &reqwest::Client,
    candidates: Vec<String>,
) -> Result<String> {
    let mut last_error = None;
    for base_url in candidates {
        let response = match client
            .get(ollama_api_url(&base_url, "api/tags"))
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                last_error = Some(anyhow::anyhow!("ollama_endpoint_discovery_failed: {error}"));
                continue;
            }
        };
        let status = response.status();
        let body = match read_bounded_ollama_response(response).await {
            Ok(body) => body,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        if !status.is_success() {
            last_error = Some(anyhow::anyhow!(
                "ollama_endpoint_discovery_http_error status={} body_digest={}",
                status.as_u16(),
                ollama_error_body_digest(&body)
            ));
            continue;
        }
        let parsed: serde_json::Value = match serde_json::from_str(&body) {
            Ok(parsed) => parsed,
            Err(error) => {
                last_error = Some(anyhow::anyhow!(
                    "ollama_endpoint_discovery_json_invalid: {error}"
                ));
                continue;
            }
        };
        if !parsed
            .get("models")
            .is_some_and(serde_json::Value::is_array)
        {
            last_error = Some(anyhow::anyhow!("ollama_endpoint_discovery_models_missing"));
            continue;
        }
        return Ok(base_url);
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("ollama_endpoint_unavailable")))
}

/// Resolve the configured model name to an actually available Ollama tag.
/// Unrelated installed models are never substituted for the configured target.
pub async fn resolve_ollama_model(model: &str) -> Option<String> {
    {
        let guard = OLLAMA_CACHE.lock().unwrap();
        if let Some(ref c) = *guard {
            if c.model == model && c.checked_at.elapsed() < get_ollama_cache_ttl() {
                return c.resolved_model.clone();
            }
        }
    }
    let resolved_model = fetch_ollama_models_from_server()
        .await
        .and_then(|models| resolve_ollama_model_from_models(model, &models));
    let mut guard = OLLAMA_CACHE.lock().unwrap();
    *guard = Some(OllamaCache {
        checked_at: Instant::now(),
        model: model.to_string(),
        resolved_model: resolved_model.clone(),
    });
    resolved_model
}

fn parse_ollama_models_from_tags_body(body: &serde_json::Value) -> Vec<(String, u64)> {
    body.get("models")
        .and_then(|models| models.as_array())
        .map(|models| {
            models
                .iter()
                .filter_map(|model| {
                    let name = ollama_model_name(model)?;
                    let size = model.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                    Some((name.to_string(), size))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn ollama_model_name(model: &serde_json::Value) -> Option<&str> {
    ["name", "model"]
        .iter()
        .filter_map(|key| model.get(*key).and_then(|value| value.as_str()))
        .map(str::trim)
        .find(|name| !name.is_empty())
}

fn resolve_ollama_model_from_models(model: &str, models: &[(String, u64)]) -> Option<String> {
    let requested = model.trim();
    if models.is_empty() {
        return None;
    }
    if requested.is_empty() {
        return None;
    }

    models
        .iter()
        .find(|(name, _)| name.trim().eq_ignore_ascii_case(requested))
        .or_else(|| {
            models
                .iter()
                .find(|(name, _)| model_matches_requested(name, requested))
        })
        .map(|(name, _)| name.clone())
}

fn model_matches_requested(available: &str, requested: &str) -> bool {
    let available = available.trim();
    let requested = requested.trim();
    if requested.is_empty() || available.is_empty() {
        return false;
    }

    let available_tokens = model_family_tokens(available);
    let requested_tokens = model_family_tokens(requested);
    !requested_tokens.is_empty()
        && available_tokens.len() >= requested_tokens.len()
        && available_tokens
            .iter()
            .zip(requested_tokens.iter())
            .all(|(available, requested)| available == requested)
}

fn model_family_tokens(value: &str) -> Vec<String> {
    let family = value.split(':').next().unwrap_or(value);
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_kind: Option<ModelTokenKind> = None;

    for ch in family.chars() {
        let kind = if ch.is_ascii_alphabetic() {
            Some(ModelTokenKind::Alpha)
        } else if ch.is_ascii_digit() {
            Some(ModelTokenKind::Digit)
        } else {
            None
        };

        let Some(kind) = kind else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            current_kind = None;
            continue;
        };

        if current_kind.is_some_and(|existing| existing != kind) && !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
        current_kind = Some(kind);
        current.push(ch.to_ascii_lowercase());
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModelTokenKind {
    Alpha,
    Digit,
}

/// Chat with a local Ollama model using a raw system prompt.
pub async fn chat_with_ollama_raw(
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
) -> Result<String> {
    chat_with_ollama_raw_with_start_observer(model, messages, system_prompt, None, || Ok(())).await
}

pub async fn chat_with_ollama_raw_with_start_observer<F>(
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
    request_id: Option<&str>,
    on_started: F,
) -> Result<String>
where
    F: FnOnce() -> Result<()>,
{
    chat_with_ollama_raw_with_candidate_set(
        model,
        messages,
        system_prompt,
        request_id,
        on_started,
        ollama_base_url_candidates(),
    )
    .await
}

async fn chat_with_ollama_raw_with_candidate_set<F>(
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
    request_id: Option<&str>,
    on_started: F,
    candidates: Vec<String>,
) -> Result<String>
where
    F: FnOnce() -> Result<()>,
{
    let client = ollama_http_client(OLLAMA_CHAT_TIMEOUT)?;
    let base_url = resolve_ollama_chat_base_url_from_candidates(&client, candidates).await?;
    let endpoint = ollama_api_url(&base_url, "api/chat");
    chat_with_ollama_raw_at_endpoint_with_start_observer(
        &endpoint,
        model,
        messages,
        system_prompt,
        OllamaOutputContract::default(),
        request_id,
        on_started,
    )
    .await
}

/// Dispatch to the exact loopback endpoint selected during preparation.
/// This function deliberately has no environment/cache/discovery fallback.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OllamaOutputContract {
    pub(crate) structured_format: Option<serde_json::Value>,
    pub(crate) deterministic: bool,
    pub(crate) reasoning_effort: Option<crate::conversation::ReasoningEffort>,
}

pub(crate) fn main_chat_work_semantic_verification_json_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "schemaVersion": {
                "type": "string",
                "const": "openlife.work-semantic-verification.v1"
            },
            "status": {
                "type": "string",
                "enum": ["complete", "needs_more_evidence"]
            },
            "gaps": {
                "type": "array",
                "maxItems": 8,
                "items": { "type": "string" }
            }
        },
        "required": ["schemaVersion", "status", "gaps"],
        "additionalProperties": false
    })
}

pub(crate) fn agent_memory_extraction_json_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "keep": { "type": "boolean" },
            "confidence": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
            "source_span": { "type": "string", "maxLength": 320 }
        },
        "required": ["keep", "confidence", "source_span"],
        "additionalProperties": false
    })
}

/// Provider-side grammar for the model-authored Work plan. This schema is
/// intentionally broader than the per-turn Policy decision: it guarantees the
/// complete typed shape and prevents fixed capabilities from carrying a model
/// minted target, while `StructuredWorkPlan::validate` still narrows kinds and
/// exact MCP target ids to the current authorized scope.
pub(crate) fn main_chat_work_plan_json_schema() -> serde_json::Value {
    let common_properties = json!({
        "id": {
            "type": "string",
            "pattern": "^[a-z][a-z0-9_]{0,31}$"
        },
        "required": { "type": "boolean" },
        "dependsOn": {
            "type": "array",
            "maxItems": 8,
            "items": { "type": "string" }
        }
    });
    let mut fixed_properties = common_properties.clone();
    fixed_properties
        .as_object_mut()
        .expect("schema object")
        .insert(
            "kind".into(),
            json!({
                "type": "string",
                "enum": [
                    "analyze",
                    "read_imported_document",
                    "read_workspace_file",
                    "web_search",
                    "web_fetch",
                    "use_selected_skill",
                    "draft_artifact",
                    "verify",
                    "deliver_result"
                ]
            }),
        );
    let mut mcp_properties = common_properties;
    let mcp_properties = mcp_properties.as_object_mut().expect("schema object");
    mcp_properties.insert(
        "kind".into(),
        json!({ "type": "string", "const": "read_mcp" }),
    );
    mcp_properties.insert("targetId".into(), json!({ "type": "string" }));

    json!({
        "type": "object",
        "properties": {
            "schemaVersion": {
                "type": "string",
                "const": "openlife.work-plan.v3"
            },
            "steps": {
                "type": "array",
                "minItems": 1,
                "maxItems": 8,
                "items": {
                    "oneOf": [
                        {
                            "type": "object",
                            "properties": fixed_properties,
                            "required": ["id", "kind", "required", "dependsOn"],
                            "additionalProperties": false
                        },
                        {
                            "type": "object",
                            "properties": mcp_properties,
                            "required": [
                                "id",
                                "kind",
                                "required",
                                "dependsOn",
                                "targetId"
                            ],
                            "additionalProperties": false
                        }
                    ]
                }
            },
            "completion": {
                "type": "object",
                "properties": {
                    "resultKind": {
                        "type": "string",
                        "enum": ["answer", "artifact"]
                    },
                    "requiresVerification": { "type": "boolean" },
                    "requirements": {
                        "type": "array",
                        "maxItems": 8,
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "description": { "type": "string" },
                                "evidenceKind": {
                                    "type": "string",
                                    "enum": ["result", "source"]
                                }
                            },
                            "required": ["id", "description", "evidenceKind"],
                            "additionalProperties": false
                        }
                    },
                    "requiresReviewBeforeWrite": { "type": "boolean" }
                },
                "required": [
                    "resultKind",
                    "requiresVerification",
                    "requirements",
                    "requiresReviewBeforeWrite"
                ],
                "additionalProperties": false
            },
            "sourceConstraints": {
                "type": "object",
                "properties": {
                    "requiredWebDomains": {
                        "type": "array",
                        "maxItems": 8,
                        "items": { "type": "string" }
                    }
                },
                "required": ["requiredWebDomains"],
                "additionalProperties": false
            }
        },
        "required": ["schemaVersion", "steps", "completion", "sourceConstraints"],
        "additionalProperties": false
    })
}

fn agent_step_envelope_schema(step: serde_json::Value) -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "schemaVersion": {
                "type": "string",
                "const": "openlife.agent-step.v1"
            },
            "step": step
        },
        "required": ["schemaVersion", "step"],
        "additionalProperties": false
    })
}

fn agent_tool_call_step_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "const": "tool_call" },
            "payload": {
                "type": "object",
                "properties": {
                    "capabilityId": { "type": "string" },
                    "arguments": { "type": "object" }
                },
                "required": ["capabilityId", "arguments"],
                "additionalProperties": false
            }
        },
        "required": ["kind", "payload"],
        "additionalProperties": false
    })
}

fn agent_tool_calls_step_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "const": "tool_calls" },
            "payload": {
                "type": "object",
                "properties": {
                    "calls": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 4,
                        "items": {
                            "type": "object",
                            "properties": {
                                "capabilityId": { "type": "string" },
                                "arguments": { "type": "object" }
                            },
                            "required": ["capabilityId", "arguments"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["calls"],
                "additionalProperties": false
            }
        },
        "required": ["kind", "payload"],
        "additionalProperties": false
    })
}

fn agent_source_block_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "enum": ["heading", "claim"] },
            "text": { "type": "string" },
            "sourceRefs": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["kind", "text", "sourceRefs"],
        "additionalProperties": false
    })
}

fn agent_final_answer_step_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "const": "final_answer" },
            "payload": {
                "type": "object",
                "properties": {
                    "content": { "type": "string" },
                    "evidenceRefs": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "artifactRefs": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "sourceBlocks": {
                        "type": "array",
                        "items": agent_source_block_schema()
                    }
                },
                "required": ["content", "evidenceRefs", "artifactRefs"],
                "additionalProperties": false
            }
        },
        "required": ["kind", "payload"],
        "additionalProperties": false
    })
}

fn agent_artifact_step_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "const": "draft_artifact" },
            "payload": {
                "type": "object",
                "properties": {
                    "artifacts": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 5,
                        "items": {
                            "type": "object",
                            "properties": {
                                "format": { "type": "string" },
                                "suggestedName": { "type": "string" },
                                "content": {},
                                "sourceBlocks": {
                                    "type": "array",
                                    "items": agent_source_block_schema()
                                }
                            },
                            "required": ["format", "suggestedName", "content"],
                            "additionalProperties": false
                        }
                    },
                    "reviewBeforeWrite": { "type": "boolean" }
                },
                "required": ["artifacts"],
                "additionalProperties": false
            }
        },
        "required": ["kind", "payload"],
        "additionalProperties": false
    })
}

fn agent_personal_intelligence_step_schemas() -> Vec<serde_json::Value> {
    vec![
        json!({
            "type": "object",
            "properties": {
                "kind": { "type": "string", "const": "personal_intelligence" },
                "payload": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "const": "remember" },
                        "sourceSpan": { "type": "string", "maxLength": 320 },
                        "memoryKind": {
                            "type": "string",
                            "enum": ["fact", "preference", "procedure", "life_event"]
                        },
                        "scope": { "type": "string", "enum": ["personal", "project"] }
                    },
                    "required": ["action", "sourceSpan", "memoryKind", "scope"],
                    "additionalProperties": false
                }
            },
            "required": ["kind", "payload"],
            "additionalProperties": false
        }),
        json!({
            "type": "object",
            "properties": {
                "kind": { "type": "string", "const": "personal_intelligence" },
                "payload": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "const": "forget" },
                        "query": { "type": "string", "maxLength": 320 }
                    },
                    "required": ["action", "query"],
                    "additionalProperties": false
                }
            },
            "required": ["kind", "payload"],
            "additionalProperties": false
        }),
        json!({
            "type": "object",
            "properties": {
                "kind": { "type": "string", "const": "personal_intelligence" },
                "payload": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "const": "suggest_life_model" },
                        "sourceSpan": { "type": "string", "maxLength": 320 },
                        "lifeModelSection": {
                            "type": "string",
                            "enum": [
                                "identity",
                                "values",
                                "stable_preferences",
                                "personal_boundaries",
                                "decision_principles",
                                "collaboration_preferences"
                            ]
                        },
                        "lifeModelStatement": { "type": "string", "maxLength": 320 }
                    },
                    "required": [
                        "action",
                        "sourceSpan",
                        "lifeModelSection",
                        "lifeModelStatement"
                    ],
                    "additionalProperties": false
                }
            },
            "required": ["kind", "payload"],
            "additionalProperties": false
        }),
    ]
}

/// Provider-side grammar for one model-selected tool action. Runtime
/// validation still narrows `capabilityId` to the exact current capability and
/// validates the capability-specific argument schema before dispatch.
pub(crate) fn main_chat_agent_tool_step_json_schema() -> serde_json::Value {
    agent_step_envelope_schema(agent_tool_call_step_schema())
}

pub(crate) fn main_chat_agent_artifact_or_tool_step_json_schema() -> serde_json::Value {
    agent_step_envelope_schema(json!({
        "oneOf": [
            agent_tool_call_step_schema(),
            agent_tool_calls_step_schema(),
            agent_artifact_step_schema()
        ]
    }))
}

pub(crate) fn main_chat_agent_answer_or_tool_step_json_schema() -> serde_json::Value {
    agent_step_envelope_schema(json!({
        "oneOf": [
            agent_tool_call_step_schema(),
            agent_tool_calls_step_schema(),
            agent_final_answer_step_schema()
        ]
    }))
}

pub(crate) fn main_chat_personal_intelligence_step_json_schema() -> serde_json::Value {
    agent_step_envelope_schema(json!({
        "oneOf": agent_personal_intelligence_step_schemas()
    }))
}

pub(crate) fn main_chat_tool_arguments_json_schema() -> serde_json::Value {
    json!({ "type": "object" })
}

pub(crate) fn main_chat_initial_work_decision_json_schema() -> serde_json::Value {
    let mut direct_steps = vec![
        agent_final_answer_step_schema(),
        agent_artifact_step_schema(),
    ];
    direct_steps.extend(agent_personal_intelligence_step_schemas());
    json!({
        "oneOf": [
            main_chat_work_plan_json_schema(),
            agent_step_envelope_schema(json!({ "oneOf": direct_steps }))
        ]
    })
}

/// Provider-side grammar for an ordinary Chat decision. Chat may either
/// return the user-visible answer or propose one bounded personal-intelligence
/// action. The runtime still binds any action to the authenticated user Item
/// and applies scope, sensitivity, dedupe, Review, and write policy itself.
pub(crate) fn main_chat_conversation_step_json_schema() -> serde_json::Value {
    let mut steps = vec![agent_final_answer_step_schema()];
    steps.extend(agent_personal_intelligence_step_schemas());
    agent_step_envelope_schema(json!({ "oneOf": steps }))
}

/// Provider-side grammar for one model-authored Artifact draft. The runtime
/// still validates the selected format, filename extension, semantic content,
/// citations, safe path, and Review checkpoint before any file write.
pub(crate) fn main_chat_agent_artifact_step_json_schema() -> serde_json::Value {
    agent_step_envelope_schema(agent_artifact_step_schema())
}

/// Provider-side grammar for the terminal answer of one canonical Agent loop.
/// Evidence and Artifact references are still checked against runtime-owned
/// identities after parsing; the model cannot mint completion proof.
pub(crate) fn main_chat_agent_final_step_json_schema() -> serde_json::Value {
    agent_step_envelope_schema(agent_final_answer_step_schema())
}

pub(crate) async fn chat_with_ollama_raw_at_endpoint_with_start_observer<F>(
    endpoint: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
    output_contract: OllamaOutputContract,
    request_id: Option<&str>,
    on_started: F,
) -> Result<String>
where
    F: FnOnce() -> Result<()>,
{
    validate_prepared_ollama_chat_endpoint(endpoint)?;
    let mut req_messages: Vec<serde_json::Value> = Vec::new();
    if let Some(sp) = system_prompt {
        req_messages.push(json!({
            "role": "system",
            "content": sp
        }));
    }

    for msg in messages {
        req_messages.push(json!({
            "role": msg.role,
            "content": msg.content
        }));
    }
    if let Some(effort) = output_contract.reasoning_effort {
        let capability = crate::llm::built_in_reasoning_capability("ollama", model)
            .ok_or_else(|| anyhow::anyhow!("ollama reasoning capability is unavailable"))?;
        if !capability.supported_efforts.contains(&effort) {
            anyhow::bail!("ollama reasoning effort is unsupported");
        }
    }

    let body = ollama_chat_request_body(
        model,
        req_messages,
        output_contract.structured_format,
        output_contract.deterministic,
        output_contract.reasoning_effort,
    );

    let client = ollama_http_client(OLLAMA_CHAT_TIMEOUT)?;
    let mut request = client.post(endpoint).json(&body);
    if let Some(request_id) = request_id {
        request = request.header("x-openlife-request-id", request_id);
    }
    on_started().context("ollama_pre_dispatch_observer_rejected")?;
    let response = request
        .send()
        .await
        .context("ollama_chat_bound_endpoint_request_failed")?;
    let status = response.status();
    let text = read_bounded_ollama_response(response).await?;
    if !status.is_success() {
        return Err(crate::llm::confirmed_provider_terminal_failure(
            "ollama_http_terminal_failed",
            anyhow::anyhow!(
                "ollama_http_error status={} body_digest={}",
                status.as_u16(),
                ollama_error_body_digest(&text)
            ),
        ));
    }
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        crate::llm::confirmed_provider_terminal_failure(
            "ollama_response_json_invalid",
            anyhow::Error::new(error).context("ollama_response_json_invalid"),
        )
    })?;
    let content = json["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if content.trim().is_empty() {
        return Err(crate::llm::confirmed_provider_terminal_failure(
            "ollama_final_content_missing",
            anyhow::anyhow!("ollama_final_content_missing"),
        ));
    }
    Ok(content)
}

fn ollama_chat_request_body(
    model: &str,
    req_messages: Vec<serde_json::Value>,
    structured_format: Option<serde_json::Value>,
    deterministic_output: bool,
    reasoning_effort: Option<crate::conversation::ReasoningEffort>,
) -> serde_json::Value {
    let structured_json_output = structured_format.is_some();
    let mut body = json!({
        "model": model,
        "messages": req_messages,
        "stream": false,
        "options": {
            "temperature": if structured_json_output || deterministic_output { 0.0 } else { 0.7 },
            "num_predict": 2048,
            "num_ctx": OLLAMA_CHAT_CONTEXT_TOKENS,
        }
    });
    if let Some(format) = structured_format {
        body["format"] = format;
    }
    if let Some(effort) = reasoning_effort {
        body["think"] = json!(effort.as_str());
    }
    body
}

/// Stream chat with a local Ollama model using a raw system prompt.
pub async fn chat_with_ollama_raw_stream(
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
) -> Result<StreamResult> {
    chat_with_ollama_raw_stream_with_start_observer(model, messages, system_prompt, None, || Ok(()))
        .await
}

pub async fn chat_with_ollama_raw_stream_with_start_observer<F>(
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
    request_id: Option<&str>,
    on_started: F,
) -> Result<StreamResult>
where
    F: FnOnce() -> Result<()>,
{
    let client = ollama_http_client(OLLAMA_CHAT_TIMEOUT)?;
    let base_url =
        resolve_ollama_chat_base_url_from_candidates(&client, ollama_base_url_candidates()).await?;
    let endpoint = ollama_api_url(&base_url, "api/chat");
    chat_with_ollama_raw_stream_at_endpoint_with_start_observer(
        &endpoint,
        model,
        messages,
        system_prompt,
        None,
        request_id,
        on_started,
    )
    .await
}

/// Streaming counterpart of the exact prepared-endpoint adapter. It never
/// performs endpoint discovery after the prepared turn has been sealed.
pub(crate) async fn chat_with_ollama_raw_stream_at_endpoint_with_start_observer<F>(
    endpoint: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
    reasoning_effort: Option<crate::conversation::ReasoningEffort>,
    request_id: Option<&str>,
    on_started: F,
) -> Result<StreamResult>
where
    F: FnOnce() -> Result<()>,
{
    validate_prepared_ollama_chat_endpoint(endpoint)?;
    let mut req_messages: Vec<serde_json::Value> = Vec::new();
    if let Some(sp) = system_prompt {
        req_messages.push(json!({
            "role": "system",
            "content": sp
        }));
    }

    for msg in messages {
        req_messages.push(json!({
            "role": msg.role,
            "content": msg.content
        }));
    }

    let mut body = json!({
        "model": model,
        "messages": req_messages,
        "stream": true,
        "options": {
            "temperature": 0.7,
            "num_predict": 2048,
            "num_ctx": OLLAMA_CHAT_CONTEXT_TOKENS,
        }
    });
    if let Some(effort) = reasoning_effort {
        let capability = crate::llm::built_in_reasoning_capability("ollama", model)
            .ok_or_else(|| anyhow::anyhow!("ollama reasoning capability is unavailable"))?;
        if !capability.supported_efforts.contains(&effort) {
            anyhow::bail!("ollama reasoning effort is unsupported");
        }
        body["think"] = json!(effort.as_str());
    }

    let client = ollama_http_client(OLLAMA_CHAT_TIMEOUT)?;
    let mut request = client.post(endpoint).json(&body);
    if let Some(request_id) = request_id {
        request = request.header("x-openlife-request-id", request_id);
    }
    on_started().context("ollama_pre_dispatch_observer_rejected")?;
    let res = request
        .send()
        .await
        .context("ollama_stream_bound_endpoint_request_failed")?;
    let status = res.status();
    if !status.is_success() {
        let text = read_bounded_ollama_response(res).await.unwrap_or_default();
        return Err(crate::llm::confirmed_provider_terminal_failure(
            "ollama_stream_http_terminal_failed",
            anyhow::anyhow!(
                "ollama_stream_http_error status={} body_digest={}",
                status.as_u16(),
                ollama_error_body_digest(&text)
            ),
        ));
    }

    let mut byte_stream = res.bytes_stream();

    let stream = async_stream::try_stream! {
        let mut buffer = String::new();
        let mut total_bytes = 0_usize;
        loop {
            let next = tokio::time::timeout(OLLAMA_STREAM_IDLE_TIMEOUT, byte_stream.next())
                .await
                .map_err(|_| anyhow::anyhow!("ollama_stream_idle_timeout"))?;
            let Some(chunk) = next else { break; };
            let chunk = chunk.map_err(|_| anyhow::anyhow!("ollama_stream_read_failed"))?;
            total_bytes = total_bytes.saturating_add(chunk.len());
            if total_bytes > OLLAMA_MAX_STREAM_TOTAL_BYTES {
                Err(anyhow::anyhow!("ollama_stream_total_limit_exceeded"))?;
            }
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            if buffer.len() > OLLAMA_MAX_STREAM_FRAME_BYTES && !buffer.contains('\n') {
                Err(anyhow::anyhow!("ollama_stream_frame_limit_exceeded"))?;
            }
            while let Some(pos) = buffer.find('\n') {
                if pos > OLLAMA_MAX_STREAM_FRAME_BYTES {
                    Err(anyhow::anyhow!("ollama_stream_frame_limit_exceeded"))?;
                }
                let line = buffer[..pos].trim().to_string();
                buffer.replace_range(..=pos, "");
                if line.is_empty() { continue; }
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(content) = parsed["message"]["content"].as_str() {
                        if !content.is_empty() {
                            yield content.to_string();
                        }
                    }
                    if parsed["done"].as_bool() == Some(true) {
                        return;
                    }
                }
            }
        }
        let remainder = buffer.trim();
        if !remainder.is_empty() {
            if remainder.len() > OLLAMA_MAX_STREAM_FRAME_BYTES {
                Err(anyhow::anyhow!("ollama_stream_frame_limit_exceeded"))?;
            }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(remainder) {
                if let Some(content) = parsed["message"]["content"].as_str() {
                    if !content.is_empty() {
                        yield content.to_string();
                    }
                }
                if parsed["done"].as_bool() == Some(true) {
                    return;
                }
            }
        }
        Err(anyhow::anyhow!("ollama_stream_incomplete"))?;
    };

    Ok(Box::pin(stream)
        as Pin<
            Box<dyn futures::Stream<Item = Result<String>> + Send>,
        >)
}

/// Generate embeddings via the explicitly selected local Ollama profile.
/// The mutable tag must resolve to the same model and digest immediately before
/// and after `/api/embed`; otherwise the returned vector has no stable artifact
/// identity and is rejected.
pub async fn ollama_embed(text: &str, model: &str) -> anyhow::Result<Vec<f32>> {
    let base_url = configured_ollama_embedding_base_url()?;
    let pre_identity =
        resolve_ollama_embedding_model_at_with_start_observer(&base_url, model, || {}).await?;
    let embedding =
        ollama_embed_resolved_at_with_start_observer(&base_url, text, &pre_identity.model, || {})
            .await?;
    let post_identity =
        resolve_ollama_embedding_model_at_with_start_observer(&base_url, model, || {}).await?;
    if post_identity != pre_identity {
        anyhow::bail!("ollama_embedding_artifact_changed_during_dispatch");
    }
    Ok(embedding)
}

/// Resolve one Ollama tag/digest observation from `/api/tags`.
/// A caller must not treat this observation as stable across `/api/embed`
/// without a matching post-dispatch observation from the same endpoint.
pub(crate) async fn resolve_ollama_embedding_model_at_with_start_observer<F>(
    base_url: &str,
    model: &str,
    on_started: F,
) -> anyhow::Result<OllamaEmbeddingModelIdentity>
where
    F: FnOnce(),
{
    let base_url = normalize_ollama_base_url(base_url)
        .ok_or_else(|| anyhow::anyhow!("ollama_embedding_endpoint_invalid"))?;
    let client = ollama_http_client(Duration::from_secs(30))?;
    let request = client.get(ollama_api_url(&base_url, "api/tags"));
    on_started();
    let response = request
        .send()
        .await
        .context("ollama_embedding_manifest_request_failed")?;
    let status = response.status();
    let response_text = read_bounded_ollama_response(response).await?;
    if !status.is_success() {
        anyhow::bail!(
            "ollama_embedding_manifest_http_error status={} body_digest={}",
            status.as_u16(),
            ollama_error_body_digest(&response_text)
        );
    }
    parse_ollama_embedding_model_identity(&response_text, model)
}

/// Execute the current Ollama `/api/embed` contract for one manifest-resolved
/// model tag. The observer fires immediately before the real HTTP dispatch.
pub(crate) async fn ollama_embed_resolved_at_with_start_observer<F>(
    base_url: &str,
    text: &str,
    model: &str,
    on_started: F,
) -> anyhow::Result<Vec<f32>>
where
    F: FnOnce(),
{
    let base_url = normalize_ollama_base_url(base_url)
        .ok_or_else(|| anyhow::anyhow!("ollama_embedding_endpoint_invalid"))?;
    let client = ollama_http_client(Duration::from_secs(30))?;
    let body = json!({
        "model": model,
        "input": text,
    });
    let request = client
        .post(ollama_api_url(&base_url, "api/embed"))
        .json(&body);
    on_started();
    let response = request
        .send()
        .await
        .context("ollama_embedding_request_failed")?;
    let status = response.status();
    let response_text = read_bounded_ollama_response(response).await?;
    if !status.is_success() {
        anyhow::bail!(
            "ollama_embedding_http_error status={} body_digest={}",
            status.as_u16(),
            ollama_error_body_digest(&response_text)
        );
    }
    parse_ollama_embedding_response(&response_text, model)
}

fn parse_ollama_embedding_model_identity(
    response_text: &str,
    requested_model: &str,
) -> Result<OllamaEmbeddingModelIdentity> {
    let body: serde_json::Value = serde_json::from_str(response_text)
        .context("parse Ollama embedding manifest response failed")?;
    let models = body
        .get("models")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("ollama_embedding_manifest_models_missing"))?;
    let available = models
        .iter()
        .filter_map(|model| {
            Some((
                ollama_model_name(model)?.to_string(),
                model
                    .get("size")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
            ))
        })
        .collect::<Vec<_>>();
    let resolved = resolve_ollama_model_from_models(requested_model, &available)
        .ok_or_else(|| anyhow::anyhow!("ollama_embedding_model_not_installed"))?;
    let model = models
        .iter()
        .find(|model| {
            ollama_model_name(model).is_some_and(|name| name.eq_ignore_ascii_case(&resolved))
        })
        .ok_or_else(|| anyhow::anyhow!("ollama_embedding_manifest_model_missing"))?;
    let digest = model
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .and_then(normalize_ollama_model_digest)
        .ok_or_else(|| anyhow::anyhow!("ollama_embedding_manifest_digest_invalid"))?;
    Ok(OllamaEmbeddingModelIdentity {
        model: resolved,
        digest,
    })
}

fn normalize_ollama_model_digest(value: &str) -> Option<String> {
    let value = value.trim();
    let hex = value.strip_prefix("sha256:").unwrap_or(value);
    (hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit()))
        .then(|| format!("sha256:{}", hex.to_ascii_lowercase()))
}

fn parse_ollama_embedding_response(response_text: &str, expected_model: &str) -> Result<Vec<f32>> {
    let json: serde_json::Value =
        serde_json::from_str(response_text).context("parse Ollama embedding response failed")?;
    let response_model = json
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .ok_or_else(|| anyhow::anyhow!("ollama_embedding_response_model_missing"))?;
    if !response_model.eq_ignore_ascii_case(expected_model.trim()) {
        anyhow::bail!("ollama_embedding_response_model_mismatch");
    }
    let embeddings = json["embeddings"]
        .as_array()
        .with_context(|| "missing embeddings array in Ollama response")?;
    if embeddings.len() != 1 {
        anyhow::bail!("ollama_embedding_response_count_mismatch");
    }
    let values = embeddings[0]
        .as_array()
        .with_context(|| "missing embedding vector in Ollama response")?;
    if values.is_empty() {
        anyhow::bail!("empty embedding returned from Ollama");
    }
    if values.len() > crate::embedding::MAX_EMBEDDING_DIMENSION {
        anyhow::bail!("ollama_embedding_dimension_limit_exceeded");
    }
    values
        .iter()
        .map(|value| {
            let number = value
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("ollama_embedding_value_invalid"))?;
            if !number.is_finite() || number > f32::MAX as f64 || number < f32::MIN as f64 {
                anyhow::bail!("ollama_embedding_value_invalid");
            }
            Ok(number as f32)
        })
        .collect()
}

/// Explicit deterministic embedding profile using character n-gram hashing.
pub fn deterministic_hash_embed_v1(text: &str) -> Vec<f32> {
    const DIM: usize = 384;
    let mut vec = vec![0.0f32; DIM];
    let lower = text.to_lowercase();
    let characters = lower.chars().collect::<Vec<_>>();
    for width in 1..=characters.len().min(3) {
        for window in characters.windows(width) {
            let mut hash = width as u64;
            for ch in window {
                hash = hash.wrapping_mul(31).wrapping_add(*ch as u64);
            }
            let idx = (hash as usize) % DIM;
            vec[idx] += 1.0;
        }
    }
    // L2 normalize
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vec {
            *v /= norm;
        }
    }
    vec
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed_step_kinds(schema: &serde_json::Value) -> Vec<&str> {
        schema["properties"]["step"]["oneOf"]
            .as_array()
            .expect("step oneOf")
            .iter()
            .map(|step| {
                step["properties"]["kind"]["const"]
                    .as_str()
                    .expect("step kind const")
            })
            .collect()
    }

    #[test]
    fn dynamic_agent_schemas_allow_one_terminal_kind_plus_tools() {
        assert_eq!(
            allowed_step_kinds(&main_chat_agent_artifact_or_tool_step_json_schema()),
            vec!["tool_call", "tool_calls", "draft_artifact"]
        );
        assert_eq!(
            allowed_step_kinds(&main_chat_agent_answer_or_tool_step_json_schema()),
            vec!["tool_call", "tool_calls", "final_answer"]
        );
    }

    #[test]
    fn artifact_schema_matches_source_bound_runtime_contract() {
        let schema = main_chat_agent_artifact_step_json_schema();
        let payload = &schema["properties"]["step"]["properties"]["payload"];
        assert!(payload["properties"].get("reviewBeforeWrite").is_some());
        let artifact = &payload["properties"]["artifacts"]["items"];
        assert!(artifact["properties"].get("sourceBlocks").is_some());
    }

    #[test]
    fn initial_decision_schema_supports_plan_or_direct_step() {
        let schema = main_chat_initial_work_decision_json_schema();
        let choices = schema["oneOf"].as_array().expect("top-level oneOf");
        assert_eq!(choices.len(), 2);
        assert_eq!(
            choices[0]["properties"]["schemaVersion"]["const"],
            "openlife.work-plan.v3"
        );
        assert_eq!(
            choices[1]["properties"]["schemaVersion"]["const"],
            "openlife.agent-step.v1"
        );
    }

    #[test]
    fn resolves_llama3_preset_to_installed_llama31_tag() {
        let models = vec![
            ("qwen2.5:7b".to_string(), 4_000),
            ("llama3.1:8b".to_string(), 8_000),
        ];

        assert_eq!(
            resolve_ollama_model_from_models("llama3", &models),
            Some("llama3.1:8b".to_string())
        );
    }

    #[test]
    fn resolves_display_style_llama3_name_to_installed_llama31_tag() {
        let models = vec![
            ("qwen2.5:7b".to_string(), 4_000),
            ("llama3.1:8b".to_string(), 8_000),
        ];

        assert_eq!(
            resolve_ollama_model_from_models("Llama 3", &models),
            Some("llama3.1:8b".to_string())
        );
    }

    #[test]
    fn does_not_match_unrelated_longer_prefix_before_family_version_match() {
        let models = vec![
            ("llama30:latest".to_string(), 30_000),
            ("llama3.1:8b".to_string(), 8_000),
        ];

        assert_eq!(
            resolve_ollama_model_from_models("llama3", &models),
            Some("llama3.1:8b".to_string())
        );
    }

    #[test]
    fn resolves_display_style_names_for_other_model_families() {
        let models = vec![
            ("gemma2:9b".to_string(), 9_000),
            ("qwen2.5:7b".to_string(), 7_000),
            ("deepseek-r1:8b".to_string(), 8_000),
        ];

        assert_eq!(
            resolve_ollama_model_from_models("Qwen 2.5", &models),
            Some("qwen2.5:7b".to_string())
        );
        assert_eq!(
            resolve_ollama_model_from_models("DeepSeek R1", &models),
            Some("deepseek-r1:8b".to_string())
        );
    }

    #[test]
    fn missing_configured_model_does_not_substitute_first_installed_model() {
        let models = vec![("llama3.1:8b".to_string(), 8_000)];
        assert_eq!(
            resolve_ollama_model_from_models("not-installed:latest", &models),
            None
        );
        assert_eq!(resolve_ollama_model_from_models("", &models), None);
    }

    #[test]
    fn parses_ollama_tags_model_field_when_name_is_missing() {
        let body = serde_json::json!({
            "models": [
                {
                    "model": "llama3.1:8b",
                    "size": 8_000
                }
            ]
        });

        assert_eq!(
            parse_ollama_models_from_tags_body(&body),
            vec![("llama3.1:8b".to_string(), 8_000)]
        );
    }

    #[test]
    fn defaults_to_ipv4_loopback_before_localhost_for_ollama() {
        assert_eq!(
            ollama_base_url_candidates_for(None),
            vec![
                "http://127.0.0.1:11434".to_string(),
                "http://localhost:11434".to_string(),
                "http://[::1]:11434".to_string(),
            ]
        );
    }

    #[test]
    fn explicit_ollama_base_url_uses_single_normalized_candidate() {
        assert_eq!(
            ollama_base_url_candidates_for(Some("127.0.0.1:11435/")),
            vec!["http://127.0.0.1:11435".to_string()]
        );
        assert_eq!(
            ollama_base_url_candidates_for(Some("http://localhost:11435/")),
            vec!["http://localhost:11435".to_string()]
        );
    }

    #[test]
    fn explicit_ollama_base_url_rejects_every_non_loopback_endpoint() {
        for endpoint in [
            "https://models.example.com:11434",
            "http://192.168.1.20:11434",
            "http://10.0.0.20:11434",
            "http://localhost.evil.example:11434",
            "http://0.0.0.0:11434",
        ] {
            assert!(
                ollama_base_url_candidates_for(Some(endpoint)).is_empty(),
                "LocalOnly Ollama endpoint must reject {endpoint}"
            );
        }
    }

    #[test]
    fn prepared_ollama_chat_endpoint_is_exact_loopback_api_chat_only() {
        for endpoint in [
            "http://127.0.0.1:11434/api/chat",
            "http://localhost:11434/api/chat",
            "http://[::1]:11434/api/chat",
        ] {
            validate_prepared_ollama_chat_endpoint(endpoint)
                .unwrap_or_else(|error| {
                    let parsed = reqwest::Url::parse(endpoint).unwrap();
                    let host = parsed.host_str().unwrap();
                    let normalized = host.trim_start_matches('[').trim_end_matches(']');
                    panic!(
                        "{endpoint}: {error:#}; scheme={:?}; host={host:?}; normalized={normalized:?}; ip={:?}; loopback={}; path={:?}; username={:?}; password={:?}; query={:?}; fragment={:?}",
                        parsed.scheme(),
                        normalized.parse::<std::net::IpAddr>(),
                        normalized.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()),
                        parsed.path(),
                        parsed.username(),
                        parsed.password(),
                        parsed.query(),
                        parsed.fragment(),
                    )
                });
        }
        for endpoint in [
            "http://127.0.0.1:11434/api/tags",
            "http://127.0.0.1:11434/api/chat?target=other",
            "https://models.example.com/api/chat",
            "local://ollama",
        ] {
            assert!(
                validate_prepared_ollama_chat_endpoint(endpoint).is_err(),
                "prepared execution must reject {endpoint}"
            );
        }
    }

    #[test]
    fn invalid_explicit_ollama_environment_fails_closed_without_local_fallback() {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        for key in OLLAMA_BASE_ENV_KEYS {
            std::env::remove_var(key);
        }
        std::env::set_var(
            "OPENLIFE_OLLAMA_BASE_URL",
            "https://remote-model.example:11434",
        );

        let candidates = ollama_base_url_candidates();

        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        assert!(
            candidates.is_empty(),
            "an explicit invalid LocalOnly endpoint must not silently fall back to another route"
        );
    }

    #[test]
    fn builds_ollama_api_urls_without_duplicate_slashes() {
        assert_eq!(
            ollama_api_url("http://127.0.0.1:11434/", "/api/tags"),
            "http://127.0.0.1:11434/api/tags"
        );
    }

    // These tests mutate a process-global endpoint and must retain the shared
    // environment lock until every async request and server task completes.
    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test(flavor = "current_thread")]
    async fn concurrent_diagnostics_share_one_generation_keyed_ollama_inspection() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", format!("http://{address}"));
        std::env::remove_var("OLLAMA_HOST");
        *ollama_inspection_coordinator().lock().await = OllamaInspectionCoordinator::default();
        let hits = Arc::new(AtomicUsize::new(0));
        let server_hits = Arc::clone(&hits);
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0_u8; 4096];
                let _ = socket.read(&mut request).await.unwrap();
                server_hits.fetch_add(1, Ordering::SeqCst);
                let body =
                    r#"{"models":[{"name":"model-a","size":1},{"name":"model-b","size":1}]}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });

        let (first, duplicate) = tokio::join!(
            inspect_ollama_status_for_generation("model-a", "generation-a"),
            inspect_ollama_status_for_generation("model-a", "generation-a")
        );
        let next_generation = inspect_ollama_status_for_generation("model-a", "generation-b").await;

        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        server.await.unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        assert_eq!(first, duplicate);
        assert_eq!(first.resolved_model.as_deref(), Some("model-a"));
        assert_eq!(next_generation.resolved_model.as_deref(), Some("model-a"));
    }

    #[tokio::test]
    async fn chat_attempt_never_switches_endpoint_after_the_bound_post_dispatches() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let first_listener = Arc::new(tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap());
        let second_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let first_url = format!("http://{}", first_listener.local_addr().unwrap());
        let second_url = format!("http://{}", second_listener.local_addr().unwrap());
        let first_hits = Arc::new(AtomicUsize::new(0));
        let first_server = {
            let listener = Arc::clone(&first_listener);
            let hits = Arc::clone(&first_hits);
            tokio::spawn(async move {
                for (path, status, body) in [
                    ("/api/tags", "200 OK", r#"{"models":[]}"#),
                    (
                        "/api/chat",
                        "500 Internal Server Error",
                        r#"{"error":"failed"}"#,
                    ),
                ] {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut request = vec![0_u8; 16 * 1024];
                    let read = socket.read(&mut request).await.unwrap();
                    let request = String::from_utf8_lossy(&request[..read]);
                    assert!(
                        request.lines().next().unwrap_or_default().contains(path),
                        "expected {path}, got {request}"
                    );
                    hits.fetch_add(1, Ordering::SeqCst);
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    socket.write_all(response.as_bytes()).await.unwrap();
                }
            })
        };
        let second_hits = Arc::new(AtomicUsize::new(0));
        let second_server = {
            let hits = Arc::clone(&second_hits);
            tokio::spawn(async move {
                if let Ok(Ok((mut socket, _))) =
                    tokio::time::timeout(Duration::from_millis(500), second_listener.accept()).await
                {
                    hits.fetch_add(1, Ordering::SeqCst);
                    let mut request = [0_u8; 1024];
                    let _ = socket.read(&mut request).await;
                }
            })
        };
        let start_count = Arc::new(AtomicUsize::new(0));
        let observer_count = Arc::clone(&start_count);

        let error = chat_with_ollama_raw_with_candidate_set(
            "local-model",
            vec![ChatMessage {
                role: "user".into(),
                content: "bound request".into(),
            }],
            None,
            Some("bound-request-id"),
            move || {
                observer_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
            vec![first_url, second_url],
        )
        .await
        .unwrap_err();

        first_server.await.unwrap();
        second_server.await.unwrap();
        assert!(error.to_string().contains("ollama_http_terminal_failed"));
        assert_eq!(start_count.load(Ordering::SeqCst), 1);
        assert_eq!(first_hits.load(Ordering::SeqCst), 2);
        assert_eq!(
            second_hits.load(Ordering::SeqCst),
            0,
            "a possible content dispatch must never fall through to another endpoint"
        );
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test]
    async fn embedding_parser_rejects_non_numeric_elements_instead_of_filtering_them() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", format!("http://{address}"));
        std::env::remove_var("OLLAMA_HOST");
        let server = tokio::spawn(async move {
            for body in [
                format!(
                    r#"{{"models":[{{"name":"nomic-embed-text:latest","digest":"sha256:{}","size":1}}]}}"#,
                    "a".repeat(64)
                ),
                r#"{"model":"nomic-embed-text:latest","embeddings":[[0.1,"not-a-number",0.3]]}"#
                    .to_string(),
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0_u8; 4096];
                let _ = socket.read(&mut request).await.unwrap();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });

        let result = ollama_embed("strict parse", "nomic-embed-text:latest").await;
        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        server.await.unwrap();

        assert!(result.is_err());
    }

    #[test]
    fn embedding_parser_rejects_empty_and_out_of_range_vectors() {
        assert!(
            parse_ollama_embedding_response(r#"{"model":"m","embeddings":[[]]}"#, "m").is_err()
        );
        assert!(
            parse_ollama_embedding_response(r#"{"model":"m","embeddings":[[1e39]]}"#, "m").is_err()
        );
        assert!(
            parse_ollama_embedding_response(r#"{"model":"m","embeddings":[[1e999]]}"#, "m")
                .is_err()
        );
        assert!(
            parse_ollama_embedding_response(r#"{"model":"other","embeddings":[[1.0]]}"#, "m")
                .is_err()
        );
    }

    #[test]
    fn embedding_manifest_requires_a_real_digest_for_the_selected_model() {
        let digest = format!("sha256:{}", "b".repeat(64));
        let response = serde_json::json!({
            "models": [
                {"name": "other:latest", "digest": format!("sha256:{}", "c".repeat(64))},
                {"name": "nomic-embed-text:latest", "digest": digest.clone()}
            ]
        })
        .to_string();
        let identity =
            parse_ollama_embedding_model_identity(&response, "nomic-embed-text:latest").unwrap();

        assert_eq!(identity.model, "nomic-embed-text:latest");
        assert_eq!(identity.digest, digest);
        assert!(parse_ollama_embedding_model_identity(
            r#"{"models":[{"name":"nomic-embed-text:latest"}]}"#,
            "nomic-embed-text:latest"
        )
        .is_err());
    }

    #[test]
    fn embedding_transport_has_one_explicit_endpoint_not_candidate_fallbacks() {
        let source = include_str!("ollama.rs");
        let embedding_transport = source
            .split("pub async fn ollama_embed")
            .nth(1)
            .unwrap()
            .split("pub fn deterministic_hash_embed_v1")
            .next()
            .unwrap();
        assert!(!embedding_transport.contains("for base_url in ollama_base_url_candidates()"));
    }

    #[test]
    fn short_hash_embeddings_are_non_zero_and_self_similar() {
        let one = deterministic_hash_embed_v1("a");
        let two = deterministic_hash_embed_v1("字");
        let norm = |embedding: &[f32]| embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
        let dot = |embedding: &[f32]| embedding.iter().map(|v| v * v).sum::<f32>();

        assert!(norm(&one) > 0.99);
        assert!(norm(&two) > 0.99);
        assert!(dot(&one) > 0.99);
        assert!(dot(&two) > 0.99);
        assert_ne!(one, two);
    }

    #[test]
    fn artifact_request_uses_json_mode_and_deterministic_temperature() {
        let body = ollama_chat_request_body(
            "llama3.1:latest",
            vec![json!({"role": "user", "content": "draft artifacts"})],
            Some(serde_json::Value::String("json".into())),
            true,
            None,
        );

        assert_eq!(body["format"], "json");
        assert_eq!(body["options"]["temperature"], 0.0);
        assert_eq!(body["stream"], false);
    }

    #[test]
    fn semantic_verification_request_uses_its_exact_typed_json_schema() {
        let body = ollama_chat_request_body(
            "llama3.1:latest",
            vec![json!({"role": "user", "content": "verify the requested outcome"})],
            Some(main_chat_work_semantic_verification_json_schema()),
            true,
            None,
        );

        assert_eq!(body["format"]["type"], "object");
        assert_eq!(
            body["format"]["required"],
            json!(["schemaVersion", "status", "gaps"])
        );
        assert_eq!(
            body["format"]["properties"]["status"]["enum"],
            json!(["complete", "needs_more_evidence"])
        );
        assert_eq!(body["options"]["temperature"], 0.0);
    }

    #[test]
    fn work_plan_request_uses_typed_json_schema_without_fixed_targets() {
        let schema = main_chat_work_plan_json_schema();
        let body = ollama_chat_request_body(
            "llama3.1:latest",
            vec![json!({"role": "user", "content": "plan this work"})],
            Some(schema),
            true,
            None,
        );

        assert_eq!(body["format"]["type"], "object");
        assert_eq!(
            body["format"]["required"],
            json!(["schemaVersion", "steps", "completion", "sourceConstraints"])
        );
        assert_eq!(
            body["format"]["properties"]["sourceConstraints"]["required"],
            json!(["requiredWebDomains"])
        );
        let alternatives = body["format"]["properties"]["steps"]["items"]["oneOf"]
            .as_array()
            .expect("step alternatives");
        assert_eq!(alternatives.len(), 2);
        assert!(alternatives[0]["properties"].get("targetId").is_none());
        assert_eq!(alternatives[1]["properties"]["kind"]["const"], "read_mcp");
        assert_eq!(
            alternatives[1]["required"],
            json!(["id", "kind", "required", "dependsOn", "targetId"])
        );
        assert_eq!(body["options"]["temperature"], 0.0);
    }

    #[test]
    fn conversation_step_schema_allows_only_answer_or_bounded_personal_action() {
        let schema = main_chat_conversation_step_json_schema();
        let alternatives = schema["properties"]["step"]["oneOf"]
            .as_array()
            .expect("conversation step alternatives");
        assert_eq!(alternatives.len(), 4);
        assert_eq!(
            alternatives[0]["properties"]["kind"]["const"],
            "final_answer"
        );
        assert_eq!(
            alternatives[1]["properties"]["payload"]["properties"]["action"]["const"],
            "remember"
        );
        assert_eq!(
            alternatives[2]["properties"]["payload"]["properties"]["action"]["const"],
            "forget"
        );
        assert_eq!(
            alternatives[3]["properties"]["payload"]["properties"]["action"]["const"],
            "suggest_life_model"
        );
        assert!(alternatives.iter().all(|alternative| {
            alternative["properties"]["payload"]["additionalProperties"] == false
        }));
    }

    #[test]
    fn ordinary_chat_request_does_not_force_json_mode() {
        let body = ollama_chat_request_body(
            "llama3.1:latest",
            vec![json!({"role": "user", "content": "hello"})],
            None,
            false,
            None,
        );

        assert!(body.get("format").is_none());
        assert_eq!(body["options"]["temperature"], 0.7);
        assert_eq!(body["options"]["num_ctx"], 32_768);
    }

    #[test]
    fn gpt_oss_request_uses_ollama_think_level() {
        let body = ollama_chat_request_body(
            "gpt-oss:20b",
            vec![json!({"role": "user", "content": "reason carefully"})],
            None,
            false,
            Some(crate::conversation::ReasoningEffort::Medium),
        );

        assert_eq!(body["think"], "medium");
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn evidence_grounded_request_is_deterministic_without_forcing_json_mode() {
        let body = ollama_chat_request_body(
            "llama3.1:latest",
            vec![json!({"role": "user", "content": "summarize Web evidence"})],
            None,
            true,
            None,
        );

        assert!(body.get("format").is_none());
        assert_eq!(body["options"]["temperature"], 0.0);
    }

    #[expect(
        clippy::await_holding_lock,
        reason = "owner=backend-reliability; expires=2026-10-01; test serializes process-global provider configuration"
    )]
    #[tokio::test(flavor = "current_thread")]
    async fn bound_stream_endpoint_ignores_environment_and_eof_is_not_completion() {
        use futures::StreamExt as _;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let endpoint = format!("http://{address}/api/chat");
        std::env::set_var("OPENLIFE_OLLAMA_BASE_URL", "http://127.0.0.1:9");
        std::env::remove_var("OLLAMA_HOST");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let read = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request
                .lines()
                .next()
                .is_some_and(|line| line.contains("/api/chat")));
            let body = "{\"message\":{\"content\":\"partial\"},\"done\":false}\n";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let mut stream = chat_with_ollama_raw_stream_at_endpoint_with_start_observer(
            &endpoint,
            "qwen-local:latest",
            vec![ChatMessage {
                role: "user".into(),
                content: "test incomplete stream".into(),
            }],
            None,
            None,
            None,
            || Ok(()),
        )
        .await
        .unwrap();
        assert_eq!(stream.next().await.unwrap().unwrap(), "partial");
        let terminal = stream.next().await.expect("terminal stream error");

        std::env::remove_var("OPENLIFE_OLLAMA_BASE_URL");
        server.await.unwrap();
        assert!(terminal
            .unwrap_err()
            .to_string()
            .contains("ollama_stream_incomplete"));
    }
}
