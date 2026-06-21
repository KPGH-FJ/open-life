use crate::life_model::LifeModel;
use crate::llm::{ChatMessage, StreamResult};
use anyhow::{Context, Result};
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

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};

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

static OLLAMA_CACHE: Mutex<Option<OllamaCache>> = Mutex::new(None);
static OLLAMA_CACHE_TTL_SECONDS: AtomicU64 = AtomicU64::new(10);

/// Set the Ollama cache TTL in seconds.
pub fn set_ollama_cache_ttl_seconds(seconds: u64) {
    OLLAMA_CACHE_TTL_SECONDS.store(seconds, Ordering::Relaxed);
}

fn get_ollama_cache_ttl() -> Duration {
    Duration::from_secs(OLLAMA_CACHE_TTL_SECONDS.load(Ordering::Relaxed))
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
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => return None,
    };
    let res = client.get("http://localhost:11434/api/tags").send().await;
    match res {
        Ok(r) if r.status().is_success() => {
            let body = r.json::<serde_json::Value>().await.ok()?;
            Some(parse_ollama_models_from_tags_body(&body))
        }
        _ => None,
    }
}

/// Resolve the configured model name to an actually available Ollama tag.
/// Falls back to the first installed model so fresh trials can still proceed.
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
        return models.first().map(|(name, _)| name.clone());
    }

    models
        .iter()
        .find(|(name, _)| name.trim().eq_ignore_ascii_case(requested))
        .or_else(|| {
            models
                .iter()
                .find(|(name, _)| model_matches_requested(name, requested))
        })
        .or_else(|| models.first())
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

/// Chat with a local Ollama model.
pub async fn chat_with_ollama(
    model: &str,
    messages: Vec<ChatMessage>,
    life_model: &LifeModel,
) -> Result<String> {
    let system_prompt = {
        let yaml = serde_yaml::to_string(life_model).unwrap_or_default();
        format!(
            r#"你是 OpenLife 的本地战术层助手，用户的终身成长合伙人。你的人设和行为必须严格基于下面这份「人生模型」。

请记住以下关于用户的信息，所有建议都必须经过人生模型的价值观过滤：

```yaml
{}
```

在每次回应时：
1. 优先考虑用户的核心价值观
2. 结合用户当前的目标和状态给出建议
3. 语气要符合用户定义的人格特质
4. 如果用户的请求与人生模型冲突，请温和地提醒并引导对齐
"#,
            yaml
        )
    };
    chat_with_ollama_raw(model, messages, Some(&system_prompt)).await
}

/// Chat with a local Ollama model using a raw system prompt.
pub async fn chat_with_ollama_raw(
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
) -> Result<String> {
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

    let body = json!({
        "model": model,
        "messages": req_messages,
        "stream": false,
        "options": {
            "temperature": 0.7,
            "num_predict": 2048,
        }
    });

    let client = reqwest::Client::new();
    let res = client
        .post("http://localhost:11434/api/chat")
        .json(&body)
        .send()
        .await
        .with_context(|| "Ollama 请求失败")?;

    let status = res.status();
    let text = res.text().await.with_context(|| "读取 Ollama 响应失败")?;

    if !status.is_success() {
        return Err(anyhow::anyhow!("Ollama 错误 ({}): {}", status, text));
    }

    let json: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("解析 Ollama 响应 JSON 失败: {}", text))?;

    let content = json["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(content)
}

/// Stream chat with a local Ollama model using a raw system prompt.
pub async fn chat_with_ollama_raw_stream(
    model: &str,
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
) -> Result<StreamResult> {
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

    let body = json!({
        "model": model,
        "messages": req_messages,
        "stream": true,
        "options": {
            "temperature": 0.7,
            "num_predict": 2048,
        }
    });

    let client = reqwest::Client::new();
    let res = client
        .post("http://localhost:11434/api/chat")
        .json(&body)
        .send()
        .await
        .with_context(|| "Ollama stream request failed")?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Ollama stream error ({}): {}",
            status,
            text
        ));
    }

    let byte_stream = res.bytes_stream();

    let stream = async_stream::try_stream! {
        let mut buffer = String::new();
        for await chunk in byte_stream {
            let chunk = chunk.map_err(|e| anyhow::anyhow!("stream error: {}", e))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buffer.find('\n') {
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
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(remainder) {
                if let Some(content) = parsed["message"]["content"].as_str() {
                    if !content.is_empty() {
                        yield content.to_string();
                    }
                }
            }
        }
    };

    Ok(Box::pin(stream)
        as Pin<
            Box<dyn futures::Stream<Item = Result<String>> + Send>,
        >)
}

/// Generate embeddings via a local Ollama model.
/// Falls back to a simple deterministic hash-based embedding if Ollama is unavailable.
pub async fn ollama_embed(text: &str, model: &str) -> anyhow::Result<Vec<f32>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let body = json!({
        "model": model,
        "prompt": text,
    });
    let res = client
        .post("http://localhost:11434/api/embeddings")
        .json(&body)
        .send()
        .await
        .with_context(|| "Ollama embedding request failed")?;

    let status = res.status();
    let json: serde_json::Value = res
        .json()
        .await
        .with_context(|| "parse Ollama embedding response failed")?;

    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "Ollama embedding error ({}): {:?}",
            status,
            json
        ));
    }

    let embedding = json["embedding"]
        .as_array()
        .with_context(|| "missing embedding array in Ollama response")?
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect::<Vec<f32>>();

    if embedding.is_empty() {
        return Err(anyhow::anyhow!("empty embedding returned from Ollama"));
    }

    Ok(embedding)
}

/// Simple deterministic fallback embedding when no API or Ollama is available.
/// Uses character n-gram hashing into a fixed 384-dim vector.
pub fn fallback_embed(text: &str) -> Vec<f32> {
    const DIM: usize = 384;
    let mut vec = vec![0.0f32; DIM];
    let lower = text.to_lowercase();
    for window in lower.chars().collect::<Vec<_>>().windows(3) {
        let mut hash = 0u64;
        for ch in window {
            hash = hash.wrapping_mul(31).wrapping_add(*ch as u64);
        }
        let idx = (hash as usize) % DIM;
        vec[idx] += 1.0;
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
}
