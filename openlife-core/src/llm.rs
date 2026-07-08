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
    if !configured_key.trim().is_empty() {
        return configured_key.to_string();
    }
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
        return Err(anyhow::anyhow!("{} 错误 ({}): {}", label, status, text));
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
    let yaml = life_model_prompt_yaml(life_model);
    let tool_section = tools_prompt.unwrap_or("");
    let time_context = build_local_time_context();
    let state_hint = format_state_hint(&life_model.state);
    let evolution_hint = if life_model.evolution_rules.is_empty() {
        "暂无进化规则".to_string()
    } else {
        life_model.evolution_rules.join("\n")
    };
    let tool_call_instruction = if tool_section.is_empty() {
        String::new()
    } else {
        r#"
【工具调用规范】
当你需要调用工具时，请严格按以下 JSON 格式输出（不要包含其他自然语言）：
```json
{
  "tool_calls": [
    {
      "name": "工具名称",
      "arguments": { "参数名": "参数值" }
    }
  ]
}
```
如果不需要工具，直接以自然语言回答用户。
"#
        .to_string()
    };
    format!(
        r#"你是 OpenLife，用户的终身成长合伙人。你的人设和行为必须严格基于下面这份「人生模型」。

{}

请记住以下关于用户的信息，所有建议都必须经过人生模型的价值观过滤：

```yaml
{}
```

【用户当前状态摘要】
{}

【自动进化规则（基于近期反馈与行为数据）】
{}

【事实边界】
- 默认模板值不是已确认事实；只有上方状态摘要、用户消息或明确记录中出现的信息，才能作为用户当前健康、精力、压力、心情或生活状态的事实。
- 如果缺少明确记录，请说“尚未确认”或直接基于用户当前输入回答，不要补全默认状态。

在每次回应时：
1. 优先考虑用户的核心价值观
2. 结合用户当前的目标和状态给出建议
3. 语气要符合用户定义的人格特质
4. 如果用户的请求与人生模型冲突，请温和地提醒并引导对齐
5. 如果用户的状态显示精力低、压力高或情绪低落，请主动表达关心并调整建议的强度和节奏
{}{}
"#,
        time_context, yaml, state_hint, evolution_hint, tool_section, tool_call_instruction
    )
}

fn life_model_prompt_yaml(life_model: &LifeModel) -> String {
    let mut value = match serde_yaml::to_value(life_model) {
        Ok(value) => value,
        Err(_) => return String::new(),
    };
    remove_default_prompt_state_fields(&mut value, &LifeModel::default_model());
    serde_yaml::to_string(&value).unwrap_or_default()
}

fn remove_default_prompt_state_fields(value: &mut serde_yaml::Value, default_model: &LifeModel) {
    let Some(root) = value.as_mapping_mut() else {
        return;
    };
    let state_key = serde_yaml::Value::String("state".into());
    let Some(state) = root
        .get_mut(&state_key)
        .and_then(serde_yaml::Value::as_mapping_mut)
    else {
        return;
    };

    remove_scalar_if_matches(
        state,
        "current_focus",
        &serde_yaml::Value::String(default_model.state.current_focus.clone()),
    );

    let health_key = serde_yaml::Value::String("health_status".into());
    if let Some(health) = state
        .get_mut(&health_key)
        .and_then(serde_yaml::Value::as_mapping_mut)
    {
        remove_scalar_if_matches(
            health,
            "physical",
            &serde_yaml::Value::String(default_model.state.health_status.physical.clone()),
        );
        remove_scalar_if_matches(
            health,
            "mental",
            &serde_yaml::Value::String(default_model.state.health_status.mental.clone()),
        );
        if let Ok(default_energy) =
            serde_yaml::to_value(default_model.state.health_status.energy_level)
        {
            remove_scalar_if_matches(health, "energy_level", &default_energy);
        }
        if let Ok(empty_energy) = serde_yaml::to_value(0u8) {
            remove_scalar_if_matches(health, "energy_level", &empty_energy);
        }
    }

    let emotional_key = serde_yaml::Value::String("emotional_state".into());
    if let Some(emotional) = state
        .get_mut(&emotional_key)
        .and_then(serde_yaml::Value::as_mapping_mut)
    {
        remove_scalar_if_matches(
            emotional,
            "current_mood",
            &serde_yaml::Value::String(default_model.state.emotional_state.current_mood.clone()),
        );
        if let Ok(default_stress) =
            serde_yaml::to_value(default_model.state.emotional_state.stress_level)
        {
            remove_scalar_if_matches(emotional, "stress_level", &default_stress);
        }
        if let Ok(empty_stress) = serde_yaml::to_value(0u8) {
            remove_scalar_if_matches(emotional, "stress_level", &empty_stress);
        }
        if let Ok(default_fulfillment) =
            serde_yaml::to_value(default_model.state.emotional_state.fulfillment_score)
        {
            remove_scalar_if_matches(emotional, "fulfillment_score", &default_fulfillment);
        }
        if let Ok(empty_fulfillment) = serde_yaml::to_value(0u8) {
            remove_scalar_if_matches(emotional, "fulfillment_score", &empty_fulfillment);
        }
    }
}

fn remove_scalar_if_matches(
    mapping: &mut serde_yaml::Mapping,
    key: &str,
    default_value: &serde_yaml::Value,
) {
    let yaml_key = serde_yaml::Value::String(key.into());
    if mapping.get(&yaml_key) == Some(default_value) {
        mapping.remove(&yaml_key);
    }
}

fn build_local_time_context() -> String {
    use chrono::Datelike;

    let now = chrono::Local::now();
    let timezone = std::env::var("TZ")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| now.format("%:z").to_string());
    format!(
        "【本地时间上下文】\n- 本地日期: {}\n- 本地星期: {}\n- 本地时间: {}\n- 本地时区: {}\n\n当用户询问今天、明天、昨天、星期几或当前日期等相对时间问题时，优先使用上述本地上下文直接回答；不要声称无法访问实时钟表。",
        now.format("%Y-%m-%d"),
        chinese_weekday(now.weekday()),
        now.format("%H:%M:%S"),
        timezone
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

fn format_state_hint(state: &crate::life_model::State) -> String {
    let default_state = LifeModel::default_model().state;
    let mut parts = Vec::new();
    if !state.current_focus.is_empty() && state.current_focus != default_state.current_focus {
        parts.push(format!("- 当前重心: {}", state.current_focus));
    }
    let mut emotional_parts = Vec::new();
    if !state.emotional_state.current_mood.is_empty()
        && state.emotional_state.current_mood != default_state.emotional_state.current_mood
    {
        emotional_parts.push(format!("当前心情: {}", state.emotional_state.current_mood));
    }
    if state.emotional_state.stress_level > 0
        && state.emotional_state.stress_level != default_state.emotional_state.stress_level
    {
        emotional_parts.push(format!("压力{}/10", state.emotional_state.stress_level));
    }
    if state.emotional_state.fulfillment_score > 0
        && state.emotional_state.fulfillment_score
            != default_state.emotional_state.fulfillment_score
    {
        emotional_parts.push(format!(
            "满足度{}/10",
            state.emotional_state.fulfillment_score
        ));
    }
    if !emotional_parts.is_empty() {
        parts.push(format!("- 情绪状态: {}", emotional_parts.join("，")));
    }
    let mut health_parts = Vec::new();
    if !state.health_status.physical.is_empty()
        && state.health_status.physical != default_state.health_status.physical
    {
        health_parts.push(format!("身体: {}", state.health_status.physical));
    }
    if !state.health_status.mental.is_empty()
        && state.health_status.mental != default_state.health_status.mental
    {
        health_parts.push(format!("心理: {}", state.health_status.mental));
    }
    if state.health_status.energy_level > 0
        && state.health_status.energy_level != default_state.health_status.energy_level
    {
        health_parts.push(format!("精力{}/10", state.health_status.energy_level));
    }
    if !health_parts.is_empty() {
        parts.push(format!("- 身心状态: {}", health_parts.join("，")));
    }
    if !state.focus_areas.is_empty() {
        parts.push(format!("- 关注领域: {}", state.focus_areas.join(", ")));
    }
    if !state.recent_events.is_empty() {
        parts.push(format!("- 近期事件: {}", state.recent_events.join(", ")));
    }
    if !state.habit_streaks.is_empty() {
        let streaks: Vec<String> = state
            .habit_streaks
            .iter()
            .map(|h| format!("{}({}天)", h.name, h.streak_days))
            .collect();
        parts.push(format!("- 习惯连续: {}", streaks.join(", ")));
    }
    if parts.is_empty() {
        "暂无状态记录".to_string()
    } else {
        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_system_prompt, chat_completions_url, default_base_for_provider, effective_api_key,
        extract_chat_content, extract_stream_content, has_reasoning_content, provider_label,
        resolve_stream_chat_model,
    };
    use crate::life_model::LifeModel;

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
    fn system_prompt_includes_local_date_and_weekday_context() {
        let prompt = build_system_prompt(&LifeModel::default(), None);

        assert!(prompt.contains("【本地时间上下文】"));
        assert!(prompt.contains("本地日期"));
        assert!(prompt.contains("星期"));
        assert!(prompt.contains("本地时区"));
        assert!(prompt.contains("不要声称无法访问实时钟表"));
    }

    #[test]
    fn system_prompt_omits_default_life_model_state_baseline_as_facts() {
        let prompt = build_system_prompt(&LifeModel::default_model(), None);

        assert!(prompt.contains("【事实边界】"));
        assert!(prompt.contains("默认模板值不是已确认事实"));
        assert!(prompt.contains("暂无状态记录"));
        assert!(!prompt.contains("身心健康"));
        assert!(!prompt.contains("精力7/10"));
        assert!(!prompt.contains("current_focus: 构建人生模型"));
        assert!(!prompt.contains("physical: 良好"));
        assert!(!prompt.contains("mental: 积极"));
        assert!(!prompt.contains("energy_level: 7"));
        assert!(!prompt.contains("current_mood: 期待"));
        assert!(!prompt.contains("stress_level: 3"));
        assert!(!prompt.contains("fulfillment_score: 6"));
    }

    #[test]
    fn system_prompt_omits_empty_numeric_state_values_as_facts() {
        let prompt = build_system_prompt(&LifeModel::default(), None);

        assert!(prompt.contains("暂无状态记录"));
        assert!(!prompt.contains("精力0/10"));
        assert!(!prompt.contains("压力0/10"));
        assert!(!prompt.contains("满足度0/10"));
        assert!(!prompt.contains("energy_level: 0"));
        assert!(!prompt.contains("stress_level: 0"));
        assert!(!prompt.contains("fulfillment_score: 0"));
    }

    #[test]
    fn system_prompt_preserves_non_default_life_model_state_facts() {
        let mut model = LifeModel::default_model();
        model.state.current_focus = "准备产品测试".into();
        model.state.health_status.physical = "疲惫".into();
        model.state.health_status.energy_level = 4;
        model.state.emotional_state.current_mood = "焦虑".into();
        let prompt = build_system_prompt(&model, None);

        assert!(prompt.contains("当前重心: 准备产品测试"));
        assert!(prompt.contains("身体: 疲惫"));
        assert!(prompt.contains("精力4/10"));
        assert!(prompt.contains("当前心情: 焦虑"));
        assert!(prompt.contains("current_focus: 准备产品测试"));
        assert!(prompt.contains("physical: 疲惫"));
        assert!(prompt.contains("energy_level: 4"));
        assert!(prompt.contains("current_mood: 焦虑"));
        assert!(!prompt.contains("mental: 积极"));
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
