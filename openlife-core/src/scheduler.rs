use crate::agent::{ModelRouteTrace, RuntimeHSPacket};
use crate::life_model::LifeModel;
use crate::llm::{chat_with_openrouter, chat_with_openrouter_stream, ChatMessage, StreamResult};
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

    pub async fn generate_with_hs_packet(
        &self,
        messages: Vec<ChatMessage>,
        life_model: &LifeModel,
        tools_prompt: Option<&str>,
        hs_packet: &RuntimeHSPacket,
    ) -> Result<String> {
        if !hs_requires_local_only(hs_packet) {
            return self.generate(messages, life_model, tools_prompt).await;
        }

        if let Some(ref router) = self.model_router {
            router.route_with_hs_packet(
                crate::agent::TaskType::Chat,
                tools_prompt.map(|p| !p.trim().is_empty()).unwrap_or(false),
                hs_packet,
            )?;
        }

        let resolved_local_model = resolve_ollama_model(&self.local_model)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "LocalOnly HS policy requires a local model, but Ollama is not available or configured"
                )
            })?;
        chat_with_ollama(&resolved_local_model, messages, life_model).await
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

    pub async fn generate_stream_with_hs_packet(
        &self,
        messages: Vec<ChatMessage>,
        life_model: &LifeModel,
        tools_prompt: Option<&str>,
        hs_packet: &RuntimeHSPacket,
    ) -> Result<StreamResult> {
        if !hs_requires_local_only(hs_packet) {
            return self
                .generate_stream(messages, life_model, tools_prompt)
                .await;
        }

        if let Some(ref router) = self.model_router {
            router.route_with_hs_packet(
                crate::agent::TaskType::Chat,
                tools_prompt.map(|p| !p.trim().is_empty()).unwrap_or(false),
                hs_packet,
            )?;
        }

        let resolved_local_model = resolve_ollama_model(&self.local_model)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "LocalOnly HS policy requires a local model, but Ollama is not available or configured"
                )
            })?;
        let system_prompt = crate::llm::build_system_prompt(life_model, tools_prompt);
        chat_with_ollama_raw_stream(&resolved_local_model, messages, Some(&system_prompt)).await
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

fn hs_requires_local_only(packet: &RuntimeHSPacket) -> bool {
    packet
        .selected_policies
        .iter()
        .any(|policy| policy.route == Some(crate::agent::ModelRoutePolicy::LocalOnly))
}

#[cfg(test)]
mod tests {
    use super::InferenceScheduler;
    use crate::agent::hs_selector::{HSSelectionAudit, RuntimeHSPacket, SelectedPolicyRef};
    use crate::agent::{
        ModelRoutePolicy, ModelRouter, ProviderAvailability,
        BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY,
    };

    fn local_only_packet() -> RuntimeHSPacket {
        RuntimeHSPacket {
            selected_policies: vec![SelectedPolicyRef {
                policy_id: BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into(),
                reason: "test_sensitive_topic".into(),
                route: Some(ModelRoutePolicy::LocalOnly),
                digest: "digest".into(),
            }],
            selected_heuristics: vec![],
            guidance_refs: vec![],
            estimated_tokens: 0,
            audit: HSSelectionAudit {
                agent_task_id: None,
                agent_run_id: Some("run-scheduler-hs".into()),
                input_digest: "input-digest".into(),
                selected_policy_ids: vec![BUILTIN_POLICY_SENSITIVE_TOPICS_LOCAL_ONLY.into()],
                selected_heuristic_ids: vec![],
                selected_guidance_ids: vec![],
                selected_guidance_refs: vec![],
                excluded_assets: vec![],
                estimated_tokens: 0,
                token_budget: 128,
            },
        }
    }

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

    #[tokio::test]
    async fn hs_local_only_policy_fails_closed_before_cloud_generation_without_local_provider() {
        let mut router = ModelRouter::new();
        router.providers.insert(
            "ollama".into(),
            ProviderAvailability {
                provider: "ollama".into(),
                available: false,
                latency_ms: None,
                models: vec![],
                last_checked: chrono::Utc::now(),
                last_error: Some("not running".into()),
                health_is_estimated: false,
            },
        );
        router.providers.insert(
            "openai".into(),
            ProviderAvailability {
                provider: "openai".into(),
                available: true,
                latency_ms: Some(200),
                models: vec!["gpt-4o-mini".into()],
                last_checked: chrono::Utc::now(),
                last_error: None,
                health_is_estimated: false,
            },
        );
        let scheduler = InferenceScheduler::new(
            "qwen2.5".into(),
            false,
            "openai".into(),
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o-mini".into(),
            "text-embedding-3-small".into(),
            true,
        )
        .with_model_router(router);

        let err = scheduler
            .generate_with_hs_packet(
                vec![],
                &crate::life_model::LifeModel::default(),
                None,
                &local_only_packet(),
            )
            .await
            .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("LocalOnly") || message.contains("local"));
    }
}
