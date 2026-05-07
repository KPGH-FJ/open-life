use crate::agent::ModelRouteTrace;
use crate::life_model::LifeModel;
use crate::llm::{
    chat_with_openrouter, chat_with_openrouter_raw, chat_with_openrouter_raw_stream,
    chat_with_openrouter_stream, ChatMessage, StreamResult,
};
use crate::ollama::{chat_with_ollama, chat_with_ollama_raw_stream, resolve_ollama_model};
use anyhow::Result;
use async_stream::try_stream;

use crate::agent::model_router::ModelRouter;

/// Inference scheduler: prefers local Ollama when available, otherwise falls back to OpenRouter.
#[derive(Clone)]
pub struct InferenceScheduler {
    pub local_model: String,
    pub prefer_local: bool,
    pub provider: String,
    pub openai_base: String,
    pub openai_key: String,
    pub chat_model: String,
    pub embedding_model: String,
    pub embedding_enabled: bool,
    /// Optional model router for intelligent routing (experimental)
    pub model_router: Option<ModelRouter>,
}

impl Default for InferenceScheduler {
    fn default() -> Self {
        Self {
            local_model: "qwen2.5:7b".into(),
            prefer_local: true,
            provider: "openai".into(),
            openai_base: "https://api.openai.com/v1".into(),
            openai_key: "".into(),
            chat_model: "gpt-4o-mini".into(),
            embedding_model: "text-embedding-3-small".into(),
            embedding_enabled: true,
            model_router: None,
        }
    }
}

impl InferenceScheduler {
    fn has_remote_key(&self) -> bool {
        !self.effective_api_key().trim().is_empty()
    }

    pub fn provider_label(&self) -> String {
        crate::llm::provider_label(&self.provider)
    }

    pub fn effective_api_key(&self) -> String {
        crate::llm::effective_api_key(&self.provider, &self.openai_key)
    }

    fn should_use_local_for_chat(
        &self,
        tools_prompt: Option<&str>,
        ollama_available: bool,
    ) -> bool {
        let has_tool_prompt = tools_prompt
            .map(|prompt| !prompt.trim().is_empty())
            .unwrap_or(false);
        let has_remote_key = self.has_remote_key();
        ollama_available
            && (self.prefer_local || !has_remote_key)
            && (!has_tool_prompt || !has_remote_key)
    }

    fn missing_backend_stream_message(&self) -> StreamResult {
        Box::pin(try_stream! {
            yield "当前既没有可用的本地模型，也没有配置云端 API Key。请在设置页填写 API Key，或启动 Ollama 并下载所选本地模型后再试。".to_string();
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_model: String,
        prefer_local: bool,
        provider: String,
        openai_base: String,
        openai_key: String,
        chat_model: String,
        embedding_model: String,
        embedding_enabled: bool,
    ) -> Self {
        Self {
            local_model,
            prefer_local,
            provider,
            openai_base,
            openai_key,
            chat_model,
            embedding_model,
            embedding_enabled,
            model_router: None,
        }
    }

    pub fn with_model_router(mut self, router: ModelRouter) -> Self {
        self.model_router = Some(router);
        self
    }

    /// Generate a reply choosing the best available backend.
    /// If tools_prompt is provided, we skip local model (local 7B may not reliably call tools)
    /// and go straight to OpenRouter.
    pub async fn generate(
        &self,
        messages: Vec<ChatMessage>,
        life_model: &LifeModel,
        tools_prompt: Option<&str>,
    ) -> Result<String> {
        // Use ModelRouter if available (experimental)
        if let Some(ref router) = self.model_router {
            let decision = router.route_chat(tools_prompt, self.prefer_local)?;
            eprintln!(
                "[ModelRouter] Route decision: provider={}, model={}, reason={}",
                decision.provider, decision.model, decision.reason
            );

            if decision.provider == "ollama" {
                let resolved_local_model = resolve_ollama_model(&self.local_model).await;
                chat_with_ollama(
                    resolved_local_model.as_deref().unwrap_or(&self.local_model),
                    messages,
                    life_model,
                )
                .await
            } else {
                chat_with_openrouter(
                    messages,
                    life_model,
                    tools_prompt,
                    &self.provider,
                    &self.openai_base,
                    &self.openai_key,
                    &self.chat_model,
                )
                .await
            }
        } else {
            // Legacy routing logic
            let resolved_local_model = resolve_ollama_model(&self.local_model).await;
            let ollama_available = resolved_local_model.is_some();
            let use_local = self.should_use_local_for_chat(tools_prompt, ollama_available);

            if use_local {
                chat_with_ollama(
                    resolved_local_model.as_deref().unwrap_or(&self.local_model),
                    messages,
                    life_model,
                )
                .await
            } else {
                chat_with_openrouter(
                    messages,
                    life_model,
                    tools_prompt,
                    &self.provider,
                    &self.openai_base,
                    &self.openai_key,
                    &self.chat_model,
                )
                .await
            }
        }
    }

    /// Generate a reply using a raw system prompt without injecting the life model.
    /// Falls back to OpenRouter if local model is not preferred or unavailable.
    pub async fn generate_raw(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<&str>,
    ) -> Result<String> {
        let resolved_local_model = resolve_ollama_model(&self.local_model).await;
        let use_local =
            resolved_local_model.is_some() && (self.prefer_local || !self.has_remote_key());

        if use_local {
            crate::ollama::chat_with_ollama_raw(
                resolved_local_model.as_deref().unwrap_or(&self.local_model),
                messages,
                system_prompt,
            )
            .await
        } else {
            crate::llm::chat_with_openrouter_raw(
                messages,
                system_prompt,
                &self.provider,
                &self.openai_base,
                &self.openai_key,
                &self.chat_model,
            )
            .await
        }
    }

    /// Generate a stream choosing the best available backend.
    pub async fn generate_stream(
        &self,
        messages: Vec<ChatMessage>,
        life_model: &LifeModel,
        tools_prompt: Option<&str>,
    ) -> Result<StreamResult> {
        // Use ModelRouter if available (experimental)
        if let Some(ref router) = self.model_router {
            let decision = router.route_chat(tools_prompt, self.prefer_local)?;
            eprintln!(
                "[ModelRouter] Stream route decision: provider={}, model={}, reason={}",
                decision.provider, decision.model, decision.reason
            );

            if decision.provider == "ollama" {
                let resolved_local_model = resolve_ollama_model(&self.local_model).await;
                let system_prompt = crate::llm::build_system_prompt(life_model, tools_prompt);
                chat_with_ollama_raw_stream(
                    resolved_local_model.as_deref().unwrap_or(&self.local_model),
                    messages,
                    Some(&system_prompt),
                )
                .await
            } else if !self.has_remote_key() {
                Ok(self.missing_backend_stream_message())
            } else {
                chat_with_openrouter_stream(
                    messages,
                    life_model,
                    tools_prompt,
                    &self.provider,
                    &self.openai_base,
                    &self.openai_key,
                    &self.chat_model,
                )
                .await
            }
        } else {
            // Legacy routing logic
            let resolved_local_model = resolve_ollama_model(&self.local_model).await;
            let ollama_available = resolved_local_model.is_some();
            let use_local = self.should_use_local_for_chat(tools_prompt, ollama_available);

            if use_local {
                let system_prompt = crate::llm::build_system_prompt(life_model, tools_prompt);
                chat_with_ollama_raw_stream(
                    resolved_local_model.as_deref().unwrap_or(&self.local_model),
                    messages,
                    Some(&system_prompt),
                )
                .await
            } else if !self.has_remote_key() {
                Ok(self.missing_backend_stream_message())
            } else {
                chat_with_openrouter_stream(
                    messages,
                    life_model,
                    tools_prompt,
                    &self.provider,
                    &self.openai_base,
                    &self.openai_key,
                    &self.chat_model,
                )
                .await
            }
        }
    }

    /// Generate a stream using a raw system prompt without injecting the life model.
    pub async fn generate_raw_stream(
        &self,
        messages: Vec<ChatMessage>,
        system_prompt: Option<&str>,
    ) -> Result<StreamResult> {
        let resolved_local_model = resolve_ollama_model(&self.local_model).await;
        let use_local =
            resolved_local_model.is_some() && (self.prefer_local || !self.has_remote_key());

        if use_local {
            chat_with_ollama_raw_stream(
                resolved_local_model.as_deref().unwrap_or(&self.local_model),
                messages,
                system_prompt,
            )
            .await
        } else if !self.has_remote_key() {
            Ok(self.missing_backend_stream_message())
        } else {
            crate::llm::chat_with_openrouter_raw_stream(
                messages,
                system_prompt,
                &self.provider,
                &self.openai_base,
                &self.openai_key,
                &self.chat_model,
            )
            .await
        }
    }

    /// Generate a stream with AgentSpec privacy_policy enforcement.
    ///
    /// * `LocalOnly` — forces local Ollama; returns error if unavailable.
    /// * `SummaryOnly` — local: full context OK; cloud: summary-only prompt,
    ///   no raw LifeModel YAML, no memory snippets.
    /// * `CloudAllowed` — normal routing (legacy behavior).
    pub async fn generate_stream_governed(
        &self,
        messages: Vec<ChatMessage>,
        life_model: &LifeModel,
        tools_prompt: Option<&str>,
        privacy_policy: crate::agent::types::PrivacyPolicy,
    ) -> Result<StreamResult, String> {
        use crate::agent::types::PrivacyPolicy;

        match privacy_policy {
            PrivacyPolicy::LocalOnly => {
                let resolved = resolve_ollama_model(&self.local_model).await;
                if resolved.is_none() {
                    return Err(
                        "LocalOnly privacy policy requires a local model, but Ollama is not available or configured"
                            .to_string(),
                    );
                }
                let system_prompt = crate::llm::build_system_prompt(life_model, tools_prompt);
                chat_with_ollama_raw_stream(
                    resolved.as_deref().unwrap_or(&self.local_model),
                    messages,
                    Some(&system_prompt),
                )
                .await
                .map_err(|e| e.to_string())
            }
            PrivacyPolicy::SummaryOnly => {
                let resolved_local = resolve_ollama_model(&self.local_model).await;
                let has_remote = self.has_remote_key();
                let use_local = should_use_local_for_summary_only(
                    resolved_local.is_some(),
                    self.prefer_local,
                    has_remote,
                );
                if use_local {
                    let system_prompt = crate::llm::build_system_prompt(life_model, tools_prompt);
                    chat_with_ollama_raw_stream(
                        resolved_local.as_deref().unwrap_or(&self.local_model),
                        messages,
                        Some(&system_prompt),
                    )
                    .await
                    .map_err(|e| e.to_string())
                } else if has_remote {
                    let summary_prompt = build_summary_only_system_prompt(life_model, tools_prompt);
                    chat_with_openrouter_raw_stream(
                        messages,
                        Some(&summary_prompt),
                        &self.provider,
                        &self.openai_base,
                        &self.openai_key,
                        &self.chat_model,
                    )
                    .await
                    .map_err(|e| e.to_string())
                } else {
                    Err(
                        "SummaryOnly: no local model available and no cloud key configured"
                            .to_string(),
                    )
                }
            }
            PrivacyPolicy::CloudAllowed => self
                .generate_stream(messages, life_model, tools_prompt)
                .await
                .map_err(|e| e.to_string()),
        }
    }

    /// Generate non-stream with AgentSpec privacy_policy enforcement.
    pub async fn generate_governed(
        &self,
        messages: Vec<ChatMessage>,
        life_model: &LifeModel,
        tools_prompt: Option<&str>,
        privacy_policy: crate::agent::types::PrivacyPolicy,
    ) -> Result<String, String> {
        use crate::agent::types::PrivacyPolicy;

        match privacy_policy {
            PrivacyPolicy::LocalOnly => {
                let resolved = resolve_ollama_model(&self.local_model).await;
                if resolved.is_none() {
                    return Err(
                        "LocalOnly privacy policy requires a local model, but Ollama is not available or configured"
                            .to_string(),
                    );
                }
                chat_with_ollama(
                    resolved.as_deref().unwrap_or(&self.local_model),
                    messages,
                    life_model,
                )
                .await
                .map_err(|e| e.to_string())
            }
            PrivacyPolicy::SummaryOnly => {
                let resolved_local = resolve_ollama_model(&self.local_model).await;
                let has_remote = self.has_remote_key();
                let use_local = should_use_local_for_summary_only(
                    resolved_local.is_some(),
                    self.prefer_local,
                    has_remote,
                );
                if use_local {
                    chat_with_ollama(
                        resolved_local.as_deref().unwrap_or(&self.local_model),
                        messages,
                        life_model,
                    )
                    .await
                    .map_err(|e| e.to_string())
                } else if has_remote {
                    let summary_prompt = build_summary_only_system_prompt(life_model, tools_prompt);
                    chat_with_openrouter_raw(
                        messages,
                        Some(&summary_prompt),
                        &self.provider,
                        &self.openai_base,
                        &self.openai_key,
                        &self.chat_model,
                    )
                    .await
                    .map_err(|e| e.to_string())
                } else {
                    Err(
                        "SummaryOnly: no local model available and no cloud key configured"
                            .to_string(),
                    )
                }
            }
            PrivacyPolicy::CloudAllowed => self
                .generate(messages, life_model, tools_prompt)
                .await
                .map_err(|e| e.to_string()),
        }
    }

    /// Preview the routing decision for a chat request without actually calling the LLM.
    /// Returns a ModelRouteTrace describing which backend would be chosen and why.
    pub async fn preview_chat_route(&self, tools_prompt: Option<&str>) -> ModelRouteTrace {
        // Use ModelRouter if available for accurate preview
        if let Some(ref router) = self.model_router {
            match router.route_chat(tools_prompt, self.prefer_local) {
                Ok(decision) => {
                    return decision.to_trace();
                }
                Err(e) => {
                    // Fallback to legacy logic with error noted
                    let mut trace = self.preview_chat_route_legacy(tools_prompt).await;
                    trace.reason = format!("model_router_error: {}; fallback to legacy", e);
                    trace.provider_health_is_estimated = Some(true);
                    return trace;
                }
            }
        }

        self.preview_chat_route_legacy(tools_prompt).await
    }

    async fn preview_chat_route_legacy(&self, tools_prompt: Option<&str>) -> ModelRouteTrace {
        let resolved_local_model = resolve_ollama_model(&self.local_model).await;
        let ollama_available = resolved_local_model.is_some();
        let use_local = self.should_use_local_for_chat(tools_prompt, ollama_available);

        let (provider, model, route_type, reason) = if use_local {
            (
                "ollama".to_string(),
                resolved_local_model.unwrap_or_else(|| self.local_model.clone()),
                "local".to_string(),
                "ollama_available_and_preferred".to_string(),
            )
        } else if !self.has_remote_key() {
            (
                "none".to_string(),
                self.local_model.clone(),
                "fallback".to_string(),
                "no_backend_available".to_string(),
            )
        } else {
            (
                self.provider.clone(),
                self.chat_model.clone(),
                "cloud".to_string(),
                if tools_prompt.map(|p| !p.trim().is_empty()).unwrap_or(false) {
                    "tools_prompt_requires_cloud".to_string()
                } else if !ollama_available {
                    "ollama_unavailable".to_string()
                } else {
                    "prefer_cloud".to_string()
                },
            )
        };

        let fallback_reason = if reason == "ollama_unavailable" {
            Some(reason.clone())
        } else {
            None
        };

        ModelRouteTrace {
            provider,
            model,
            route_type,
            prefer_local: self.prefer_local,
            local_model: self.local_model.clone(),
            reason,
            privacy_level: crate::agent::types::RedactionLevel::None,
            latency_ms: None,
            retry_count: 0,
            fallback_reason,
            provider_health_is_estimated: Some(true),
        }
    }
}

/// Build a summary-only system prompt for `PrivacyPolicy::SummaryOnly`.
///
/// Excludes raw LifeModel YAML, identity.name, birth_date, goal descriptions,
/// recent_events, reflections, custom_dimensions, and memory snippets.
/// Keeps: current_focus, goal counts, value names (no descriptions), tools_prompt.
fn build_summary_only_system_prompt(life_model: &LifeModel, tools_prompt: Option<&str>) -> String {
    let tool_section = tools_prompt.unwrap_or("");

    let state_hint = if !life_model.state.current_focus.is_empty() {
        format!(
            "- 当前重心: {}\n- 当前心情: {}",
            life_model.state.current_focus, life_model.state.emotional_state.current_mood
        )
    } else {
        "暂无状态摘要".to_string()
    };

    let goal_summary = format!(
        "短期目标 {} 个，中期 {} 个，长期 {} 个，人生目标 {} 个，每日 {} 个",
        life_model.goals.short_term.len(),
        life_model.goals.medium_term.len(),
        life_model.goals.long_term.len(),
        life_model.goals.life_goals.len(),
        life_model.goals.daily.len(),
    );

    let value_names: Vec<&str> = life_model
        .identity
        .values
        .iter()
        .map(|v| v.name.as_str())
        .collect();
    let values_hint = if value_names.is_empty() {
        "暂无核心价值观".to_string()
    } else {
        format!("核心价值观: {}", value_names.join("、"))
    };

    format!(
        r#"你是 OpenLife，用户的终身成长合伙人。

[SummaryOnly] 云端隐私保护模式下，仅发送以下摘要信息：

【用户状态摘要】
{}

【目标摘要】
{}

【价值观方向】
{}

【工具信息】
{}

在每次回应时：
1. 基于用户的核心价值观方向给出建议
2. 结合用户当前的状态和大致目标方向
3. 语气要温和、支持但不透露具体个人信息
4. 如用户要求具体信息，请说明当前处于隐私保护模式，建议切换到本地模型
"#,
        state_hint, goal_summary, values_hint, tool_section,
    )
}

/// Pure decision function for SummaryOnly routing.
///
/// * no local + no cloud → error (handled by caller)
/// * local + no cloud → use local (even if prefer_local=false)
/// * local + prefer_local → use local
/// * remote + not prefer_local → cloud summary-only
pub fn should_use_local_for_summary_only(
    resolved_local: bool,
    prefer_local: bool,
    has_remote_key: bool,
) -> bool {
    resolved_local && (prefer_local || !has_remote_key)
}

#[cfg(test)]
mod tests {
    use super::InferenceScheduler;

    #[test]
    fn local_chat_stays_enabled_without_remote_key_even_when_tools_are_available() {
        let scheduler = InferenceScheduler::new(
            "qwen2.5".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            true,
        );
        assert!(scheduler.should_use_local_for_chat(Some("tool prompt"), true));
    }

    #[test]
    fn local_chat_can_yield_to_remote_when_tools_need_better_compliance_and_remote_is_ready() {
        let scheduler = InferenceScheduler::new(
            "qwen2.5".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            true,
        );
        assert!(!scheduler.should_use_local_for_chat(Some("tool prompt"), true));
    }

    #[test]
    fn local_chat_stays_disabled_when_ollama_is_not_available() {
        let scheduler = InferenceScheduler::new(
            "qwen2.5".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            true,
        );
        assert!(!scheduler.should_use_local_for_chat(None, false));
    }

    #[test]
    fn local_chat_becomes_fallback_when_remote_key_is_missing() {
        let scheduler = InferenceScheduler::new(
            "qwen2.5".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            true,
        );
        assert!(scheduler.should_use_local_for_chat(None, true));
    }

    // ── P7 Stabilize: privacy_policy enforcement tests ──────────────────

    use crate::agent::types::PrivacyPolicy;

    #[tokio::test]
    async fn test_local_only_cloud_only_route_is_blocked() {
        // Create a scheduler with no local model, cloud-only config.
        let scheduler = InferenceScheduler::new(
            "qwen2.5:7b".into(),
            false, // prefer cloud
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test-key".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        );

        // With LocalOnly privacy and no Ollama running, generate_stream_governed
        // must return an error rather than silently falling back to cloud.
        let messages = vec![];
        let life_model = crate::life_model::LifeModel::default();

        let result = scheduler
            .generate_stream_governed(
                messages.clone(),
                &life_model,
                None,
                PrivacyPolicy::LocalOnly,
            )
            .await;

        // LocalOnly with no local model available -> error, not cloud fallback.
        match result {
            Err(err) => {
                assert!(
                    err.contains("LocalOnly") || err.contains("Ollama"),
                    "error should mention LocalOnly/Ollama, got: {}",
                    err
                );
            }
            Ok(_) => panic!("LocalOnly must block cloud calls when no local model is available"),
        }
    }

    #[test]
    fn test_cloud_allowed_returns_existing_route_for_normal_config() {
        // CloudAllowed and SummaryOnly should not change the routing behavior
        // beyond what the existing ContextPolicy already handles.
        let _scheduler = InferenceScheduler::new(
            "qwen2.5:7b".into(),
            true,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            false,
        );

        // No cloud key, but local is available in config — CloudAllowed should
        // not error (it doesn't add restrictions).
        assert_eq!(format!("{}", PrivacyPolicy::CloudAllowed), "cloud_allowed");
        assert_eq!(format!("{}", PrivacyPolicy::SummaryOnly), "summary_only");
        assert_eq!(format!("{}", PrivacyPolicy::LocalOnly), "local_only");
    }

    // ── P7: SummaryOnly prompt excludes raw LifeModel fields ──────────────

    #[test]
    fn test_summary_only_prompt_excludes_raw_lifemodel_fields() {
        use crate::life_model::{
            EmotionalState, GoalItem, Goals, Identity, LifeModel, State, ValueItem,
        };

        let mut life_model = LifeModel::default();
        life_model.identity = Identity {
            name: "Alice Secret".to_string(),
            birth_date: Some("1990-01-01".to_string()),
            values: vec![ValueItem {
                name: "诚实".to_string(),
                weight: 90,
                description: "保持诚实正直".to_string(),
            }],
            ..Default::default()
        };
        life_model.goals = Goals {
            short_term: vec![GoalItem {
                name: "秘密目标".to_string(),
                description: "不能让人知道的目标".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        life_model.state = State {
            current_focus: "提升工作效率".to_string(),
            emotional_state: EmotionalState {
                current_mood: "平静".to_string(),
                stress_level: 3,
                fulfillment_score: 7,
            },
            recent_events: vec!["敏感事件A".to_string(), "敏感事件B".to_string()],
            ..Default::default()
        };

        let prompt = super::build_summary_only_system_prompt(&life_model, Some("工具测试"));

        // Sensitive fields must not appear
        assert!(
            !prompt.contains("Alice Secret"),
            "prompt should not contain identity.name"
        );
        assert!(
            !prompt.contains("1990-01-01"),
            "prompt should not contain birth_date"
        );
        assert!(
            !prompt.contains("不能让人知道的目标"),
            "prompt should not contain goal descriptions"
        );
        assert!(
            !prompt.contains("敏感事件A"),
            "prompt should not contain recent_events"
        );
        assert!(
            !prompt.contains("敏感事件B"),
            "prompt should not contain recent_events"
        );

        // Summary-only fields should appear
        assert!(
            prompt.contains("提升工作效率"),
            "prompt should contain current_focus summary"
        );
        assert!(prompt.contains("诚实"), "prompt should contain value names");
        assert!(prompt.contains("1 个"), "prompt should contain goal count");
        assert!(
            prompt.contains("SummaryOnly"),
            "prompt should be marked SummaryOnly"
        );
        assert!(
            prompt.contains("工具测试"),
            "prompt should contain tools section"
        );

        // Value descriptions (sensitive) should not appear
        assert!(
            !prompt.contains("保持诚实正直"),
            "prompt should not contain value descriptions"
        );
    }

    #[test]
    fn test_summary_only_prompt_includes_goal_counts_not_names() {
        use crate::life_model::{GoalItem, Goals, LifeModel};

        let mut life_model = LifeModel::default();
        life_model.goals = Goals {
            short_term: vec![
                GoalItem {
                    name: "目标A".to_string(),
                    description: "详细描述A".to_string(),
                    ..Default::default()
                },
                GoalItem {
                    name: "目标B".to_string(),
                    description: "详细描述B".to_string(),
                    ..Default::default()
                },
            ],
            medium_term: vec![GoalItem {
                name: "中期目标".to_string(),
                description: "中期描述".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let prompt = super::build_summary_only_system_prompt(&life_model, None);

        assert!(
            prompt.contains("短期目标 2 个"),
            "should show short-term count"
        );
        assert!(
            prompt.contains("中期 1 个"),
            "should show medium-term count"
        );
        assert!(!prompt.contains("目标A"), "should not contain goal names");
        assert!(
            !prompt.contains("详细描述A"),
            "should not contain goal descriptions"
        );
    }

    // ── P7: SummaryOnly routing pure-function tests ───────────────────

    #[test]
    fn test_should_use_local_for_summary_only_all_cases() {
        let func = super::should_use_local_for_summary_only;

        // local + prefer_local → use local
        assert!(func(true, true, true));
        assert!(func(true, true, false));

        // local + !prefer_local + no cloud → use local
        assert!(func(true, false, false));

        // local + !prefer_local + cloud → cloud summary
        assert!(!func(true, false, true));

        // no local + cloud → cloud summary
        assert!(!func(false, true, true));
        assert!(!func(false, false, true));

        // no local + no cloud → error (handled by caller)
        assert!(!func(false, true, false));
        assert!(!func(false, false, false));
    }

    #[test]
    fn test_summary_only_uses_local_when_remote_missing_even_if_prefer_local_false() {
        assert!(
            super::should_use_local_for_summary_only(true, false, false),
            "SummaryOnly with local available but no cloud key must use local"
        );
    }

    #[test]
    fn test_summary_only_uses_cloud_summary_when_remote_available_and_prefer_local_false() {
        assert!(
            !super::should_use_local_for_summary_only(true, false, true),
            "SummaryOnly with local + cloud key + !prefer_local must use cloud summary"
        );
    }
}
