use crate::life_model::LifeModel;
use anyhow::{Context, Result};
use async_stream::try_stream;
use futures::{Stream, StreamExt};
use reqwest::header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::pin::Pin;
use std::time::Duration;

pub type StreamResult = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

const CHAT_REQUEST_TIMEOUT_SECS: u64 = 120;
const STREAM_CONNECT_TIMEOUT_SECS: u64 = 20;
const REASONING_NOTICE: &str = "（模型正在推理中，可能需要稍等片刻...）\n\n";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub fn provider_label(provider: &str) -> String {
    match provider {
        "deepseek" => "DeepSeek".to_string(),
        "openrouter" => "OpenRouter".to_string(),
        "openai" => "OpenAI".to_string(),
        "siliconflow" => "SiliconFlow".to_string(),
        "moonshot" => "Moonshot/Kimi".to_string(),
        "dashscope" => "通义千问 DashScope".to_string(),
        "zhipu" => "智谱 GLM".to_string(),
        _ => "OpenAI-compatible".to_string(),
    }
}

pub fn effective_api_key(provider: &str, configured_key: &str) -> String {
    // 1. Check OS keyring first (most secure)
    if let Some(key) = crate::keyring_store::get_api_key(provider) {
        if !key.trim().is_empty() {
            return key;
        }
    }
    // 2. Fall back to configured key in config.yaml
    if !configured_key.trim().is_empty() {
        return configured_key.to_string();
    }
    // 3. Fall back to environment variable
    match provider {
        "deepseek" => std::env::var("DEEPSEEK_API_KEY").unwrap_or_default(),
        "openrouter" => std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
        "openai" => std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        "siliconflow" => std::env::var("SILICONFLOW_API_KEY").unwrap_or_default(),
        "moonshot" => std::env::var("MOONSHOT_API_KEY").unwrap_or_default(),
        "dashscope" => std::env::var("DASHSCOPE_API_KEY").unwrap_or_default(),
        "zhipu" => std::env::var("ZHIPU_API_KEY").unwrap_or_default(),
        _ => String::new(),
    }
}

pub fn default_base_for_provider(provider: &str) -> &'static str {
    match provider {
        "deepseek" => "https://api.deepseek.com",
        "openrouter" => "https://openrouter.ai/api/v1",
        "siliconflow" => "https://api.siliconflow.cn/v1",
        "moonshot" => "https://api.moonshot.cn/v1",
        "dashscope" => "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "zhipu" => "https://open.bigmodel.cn/api/paas/v4",
        _ => "https://api.openai.com/v1",
    }
}

pub fn chat_completions_url(provider: &str, openai_base: &str) -> String {
    let base = if openai_base.trim().is_empty() {
        default_base_for_provider(provider).to_string()
    } else {
        openai_base.trim().trim_end_matches('/').to_string()
    };
    if base.ends_with("/chat/completions") {
        base
    } else {
        format!("{}/chat/completions", base)
    }
}

pub fn resolve_stream_chat_model<'a>(provider: &str, chat_model: &'a str) -> &'a str {
    if provider == "deepseek" && chat_model.to_lowercase().contains("reasoner") {
        "deepseek-chat"
    } else {
        chat_model
    }
}

fn extract_chat_content(json: &serde_json::Value) -> Option<String> {
    json["choices"][0]["message"]["content"]
        .as_str()
        .or_else(|| json["choices"][0]["text"].as_str())
        .or_else(|| json["output_text"].as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn extract_stream_content(json: &serde_json::Value) -> Option<String> {
    json["choices"][0]["delta"]["content"]
        .as_str()
        .or_else(|| json["choices"][0]["message"]["content"].as_str())
        .or_else(|| json["choices"][0]["text"].as_str())
        .or_else(|| json["delta"]["content"].as_str())
        .or_else(|| json["content"].as_str())
        .map(ToString::to_string)
        .filter(|s| !s.is_empty())
}

fn has_reasoning_content(json: &serde_json::Value) -> bool {
    json["choices"][0]["delta"]["reasoning_content"]
        .as_str()
        .or_else(|| json["choices"][0]["message"]["reasoning_content"].as_str())
        .or_else(|| json["delta"]["reasoning_content"].as_str())
        .or_else(|| json["reasoning_content"].as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some()
}

pub async fn chat_with_openrouter(
    messages: Vec<ChatMessage>,
    life_model: &LifeModel,
    tools_prompt: Option<&str>,
    provider: &str,
    openai_base: &str,
    openai_key: &str,
    chat_model: &str,
) -> Result<String> {
    let system_prompt = build_system_prompt(life_model, tools_prompt);
    chat_with_openrouter_raw(
        messages,
        Some(&system_prompt),
        provider,
        openai_base,
        openai_key,
        chat_model,
    )
    .await
}

pub async fn chat_with_openrouter_raw(
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
    provider: &str,
    openai_base: &str,
    openai_key: &str,
    chat_model: &str,
) -> Result<String> {
    let api_key = effective_api_key(provider, openai_key);
    let label = provider_label(provider);

    if api_key.is_empty() {
        return Ok(format!(
            "请设置 {} API Key，或在设置页填写 API Key 以使用对话功能。",
            label
        ));
    }

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
        "model": chat_model,
        "messages": req_messages,
        "temperature": 0.7,
        "max_tokens": 2048,
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(CHAT_REQUEST_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(STREAM_CONNECT_TIMEOUT_SECS))
        .build()?;
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse()?);
    headers.insert(AUTHORIZATION, format!("Bearer {}", api_key).parse()?);

    let url = chat_completions_url(provider, openai_base);

    let res = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("{} 请求失败", label))?;

    let status = res.status();
    let text = res.text().await.with_context(|| "读取响应失败")?;

    if !status.is_success() {
        log::debug!("{} response body ({}): {}", label, status, text);
        return Err(anyhow::anyhow!("{} 错误 ({})", label, status));
    }

    let json: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("解析响应 JSON 失败: {}", text))?;

    let content = match extract_chat_content(&json) {
        Some(content) => content,
        None if has_reasoning_content(&json) => format!(
            "{} 返回了推理过程，但没有返回最终回答。建议切换到 deepseek-chat 等通用聊天模型，或重试并增加输出 token 上限。",
            label
        ),
        None => return Err(anyhow::anyhow!("{} 响应为空或格式不兼容: {}", label, text)),
    };

    Ok(content)
}

pub async fn chat_with_openrouter_stream(
    messages: Vec<ChatMessage>,
    life_model: &LifeModel,
    tools_prompt: Option<&str>,
    provider: &str,
    openai_base: &str,
    openai_key: &str,
    chat_model: &str,
) -> Result<StreamResult> {
    let system_prompt = build_system_prompt(life_model, tools_prompt);
    chat_with_openrouter_raw_stream(
        messages,
        Some(&system_prompt),
        provider,
        openai_base,
        openai_key,
        chat_model,
    )
    .await
}

pub async fn chat_with_openrouter_raw_stream(
    messages: Vec<ChatMessage>,
    system_prompt: Option<&str>,
    provider: &str,
    openai_base: &str,
    openai_key: &str,
    chat_model: &str,
) -> Result<StreamResult> {
    let api_key = effective_api_key(provider, openai_key);
    let label = provider_label(provider);
    let stream_model = resolve_stream_chat_model(provider, chat_model);

    if api_key.is_empty() {
        return Err(anyhow::anyhow!(
            "请设置 {} API Key，或在设置页填写 API Key 以使用对话功能。",
            label
        ));
    }

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
        "model": stream_model,
        "messages": req_messages,
        "temperature": 0.7,
        "max_tokens": 2048,
        "stream": true,
    });

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(STREAM_CONNECT_TIMEOUT_SECS))
        .build()?;
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/json".parse()?);
    headers.insert(AUTHORIZATION, format!("Bearer {}", api_key).parse()?);

    let url = chat_completions_url(provider, openai_base);

    let res = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("{} 请求失败", label))?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("{} 错误 ({}): {}", label, status, text));
    }

    let mut byte_stream = res.bytes_stream();
    let stream = try_stream! {
        let mut buffer = String::new();
        let mut emitted_reasoning_notice = false;
        while let Some(chunk) = byte_stream.next().await {
            let bytes = chunk.with_context(|| "stream read error")?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer.replace_range(..=pos, "");
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data == "[DONE]" { return; }
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(content) = extract_stream_content(&json) {
                            yield content;
                        } else if !emitted_reasoning_notice && has_reasoning_content(&json) {
                            emitted_reasoning_notice = true;
                            yield REASONING_NOTICE.to_string();
                        }
                    }
                }
            }
        }
        let remainder = buffer.trim();
        if let Some(data) = remainder.strip_prefix("data:") {
            let data = data.trim();
            if data != "[DONE]" {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(content) = extract_stream_content(&json) {
                        yield content;
                    } else if !emitted_reasoning_notice && has_reasoning_content(&json) {
                        yield REASONING_NOTICE.to_string();
                    }
                }
            }
        }
    };

    Ok(Box::pin(stream))
}

pub fn build_system_prompt(life_model: &LifeModel, tools_prompt: Option<&str>) -> String {
    let tools_text = tools_prompt.unwrap_or("");
    let tools_block = if tools_text.trim().is_empty() {
        None
    } else {
        Some(crate::agent::prompt_stack::PromptBlock::available_tools(
            tools_text.to_string(),
        ))
    };
    crate::agent::prompt_stack::PromptStack::chat_system_stack(life_model, tools_block).assemble()
}

#[cfg(test)]
mod tests {
    use super::{
        chat_completions_url, default_base_for_provider, effective_api_key, extract_chat_content,
        extract_stream_content, has_reasoning_content, provider_label, resolve_stream_chat_model,
    };

    #[test]
    fn deepseek_provider_uses_expected_label_and_base() {
        assert_eq!(provider_label("deepseek"), "DeepSeek");
        assert_eq!(
            default_base_for_provider("deepseek"),
            "https://api.deepseek.com"
        );
    }

    #[test]
    fn provider_specific_env_fallbacks_are_used_when_config_key_is_empty() {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        std::env::set_var("DEEPSEEK_API_KEY", "sk-deepseek-test");
        std::env::set_var("OPENROUTER_API_KEY", "sk-openrouter-test");
        std::env::set_var("OPENAI_API_KEY", "sk-openai-test");

        assert_eq!(effective_api_key("deepseek", ""), "sk-deepseek-test");
        assert_eq!(effective_api_key("openrouter", ""), "sk-openrouter-test");
        assert_eq!(effective_api_key("openai", ""), "sk-openai-test");
        assert_eq!(effective_api_key("deepseek", "sk-config"), "sk-config");
        assert_eq!(effective_api_key("custom", ""), "");

        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("OPENROUTER_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn chat_url_accepts_base_or_full_endpoint() {
        assert_eq!(
            chat_completions_url("deepseek", "https://api.deepseek.com"),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            chat_completions_url("openai", "https://api.openai.com/v1"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("custom", "http://localhost:1234/v1/chat/completions"),
            "http://localhost:1234/v1/chat/completions"
        );
    }

    #[test]
    fn extracts_content_from_common_openai_compatible_shapes() {
        let normal = serde_json::json!({
            "choices": [{"message": {"content": "hello"}}]
        });
        let text = serde_json::json!({
            "choices": [{"text": "hello text"}]
        });
        let stream = serde_json::json!({
            "choices": [{"delta": {"content": "hi"}}]
        });
        let stream_alt = serde_json::json!({
            "delta": {"content": "alt"}
        });
        let reasoning = serde_json::json!({
            "choices": [{"delta": {"reasoning_content": "thinking"}}]
        });
        let reasoning_message = serde_json::json!({
            "choices": [{"message": {"reasoning_content": "thinking", "content": ""}}]
        });
        assert_eq!(extract_chat_content(&normal).as_deref(), Some("hello"));
        assert_eq!(extract_chat_content(&text).as_deref(), Some("hello text"));
        assert_eq!(extract_stream_content(&stream).as_deref(), Some("hi"));
        assert_eq!(extract_stream_content(&stream_alt).as_deref(), Some("alt"));
        assert!(has_reasoning_content(&reasoning));
        assert!(has_reasoning_content(&reasoning_message));
        assert_eq!(extract_stream_content(&reasoning), None);
    }

    #[test]
    fn deepseek_reasoner_stream_is_downgraded_to_chat_model() {
        assert_eq!(
            resolve_stream_chat_model("deepseek", "deepseek-reasoner"),
            "deepseek-chat"
        );
        assert_eq!(
            resolve_stream_chat_model("deepseek", "deepseek-chat"),
            "deepseek-chat"
        );
        assert_eq!(
            resolve_stream_chat_model("openai", "gpt-4o-mini"),
            "gpt-4o-mini"
        );
    }
}
