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

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct OllamaCache {
    checked_at: Instant,
    model: String,
    resolved_model: Option<String>,
}

static OLLAMA_CACHE: parking_lot::Mutex<Option<OllamaCache>> = parking_lot::Mutex::new(None);
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

/// Fetch the list of installed Ollama models for UI display.
pub async fn list_ollama_models() -> Vec<(String, u64)> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let res = client.get("http://localhost:11434/api/tags").send().await;
    match res {
        Ok(r) if r.status().is_success() => {
            if let Ok(body) = r.json::<serde_json::Value>().await {
                if let Some(models) = body.get("models").and_then(|m| m.as_array()) {
                    return models
                        .iter()
                        .filter_map(|m| {
                            let name = m.get("name")?.as_str()?.to_string();
                            let size = m.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                            Some((name, size))
                        })
                        .collect();
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Resolve the configured model name to an actually available Ollama tag.
/// Falls back to the first installed model so fresh trials can still proceed.
pub async fn resolve_ollama_model(model: &str) -> Option<String> {
    {
        let guard = OLLAMA_CACHE.lock();
        if let Some(ref c) = *guard {
            if c.model == model && c.checked_at.elapsed() < get_ollama_cache_ttl() {
                return c.resolved_model.clone();
            }
        }
    }
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(_) => return None,
    };
    let res = client.get("http://localhost:11434/api/tags").send().await;
    let resolved_model = match res {
        Ok(r) if r.status().is_success() => {
            if let Ok(body) = r.json::<serde_json::Value>().await {
                if let Some(models) = body.get("models").and_then(|m| m.as_array()) {
                    let requested = model.trim();
                    let matched = models.iter().find_map(|m| {
                        m.get("name")
                            .and_then(|n| n.as_str())
                            .filter(|n| *n == requested || n.starts_with(requested))
                            .map(|n| n.to_string())
                    });
                    matched.or_else(|| {
                        models.iter().find_map(|m| {
                            m.get("name")
                                .and_then(|n| n.as_str())
                                .map(|n| n.to_string())
                        })
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    };
    let mut guard = OLLAMA_CACHE.lock();
    *guard = Some(OllamaCache {
        checked_at: Instant::now(),
        model: model.to_string(),
        resolved_model: resolved_model.clone(),
    });
    resolved_model
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
        log::debug!("Ollama response body ({}): {}", status, text);
        return Err(anyhow::anyhow!("Ollama 错误 ({})", status));
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
/// Uses character n-gram hashing with random-projection-like multi-dimension
/// scattering to produce a 384-dim vector with reasonable discrimination.
pub fn fallback_embed(text: &str) -> Vec<f32> {
    const DIM: usize = 384;
    let mut vec = vec![0.0f32; DIM];
    let lower = text.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.is_empty() {
        return vec;
    }
    // Unigrams + bigrams + trigrams for coverage (single CJK chars still contribute)
    let mut seen = 0u64;
    for window in chars
        .windows(3)
        .chain(chars.windows(2))
        .chain(chars.windows(1))
    {
        // Seed-specific hashing: each dimension gets a different hash contribution
        let base = {
            let mut h = 0u64;
            for ch in window {
                h = h
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(*ch as u64 ^ seen);
            }
            h
        };
        // Spread this ngram across ALL dimensions via a cheap LCG
        let mut state = base;
        for item in vec.iter_mut() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            // Use sign bit to produce bipolar contribution
            let val = ((state >> 32) as u32) as f32 / (u32::MAX as f32);
            let sign = if (state & 1) != 0 { 1.0 } else { -1.0 };
            *item += sign * val;
        }
        seen = seen.wrapping_add(1);
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
