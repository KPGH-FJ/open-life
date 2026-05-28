use crate::agent::types::RedactionLevel;
use crate::agent::ModelRouteTrace;
use crate::agent::RuntimeHSPacket;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Task type for routing decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskType {
    Chat,
    Planner,
    ToolUse,
    Summarizer,
    Extractor,
    Embedding,
}

/// Privacy requirement level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivacyRequirement {
    Low,      // Can use any provider
    Medium,   // Prefer local or trusted providers
    High,     // Must use local
    Critical, // Must use local + no network
}

/// Scoring result for a single provider.
#[derive(Debug, Clone)]
pub struct ModelRouteScore {
    pub provider: String,
    pub model: String,
    pub score: f32,
    pub latency_ms: Option<u64>,
    pub cost_per_1k_tokens: Option<f32>,
    pub capability_score: u8, // 0-10
    pub privacy_level: PrivacyRequirement,
}

/// Final routing decision.
#[derive(Debug, Clone)]
pub struct ModelRouteDecision {
    pub provider: String,
    pub model: String,
    pub route_type: String,
    pub prefer_local: bool,
    pub reason: String,
    pub privacy_level: RedactionLevel,
    pub estimated_latency_ms: Option<u64>,
    pub fallback_provider: Option<String>,
    pub fallback_model: Option<String>,
}

impl ModelRouteDecision {
    pub fn to_trace(&self) -> ModelRouteTrace {
        ModelRouteTrace {
            provider: self.provider.clone(),
            model: self.model.clone(),
            route_type: self.route_type.clone(),
            prefer_local: self.prefer_local,
            local_model: if self.prefer_local {
                self.model.clone()
            } else {
                String::new()
            },
            reason: self.reason.clone(),
            privacy_level: self.privacy_level,
            latency_ms: self.estimated_latency_ms,
            retry_count: 0,
            fallback_reason: self
                .fallback_provider
                .as_ref()
                .map(|provider| format!("fallback_provider_selected:{}", provider)),
            provider_health_is_estimated: None,
        }
    }
}

/// Provider health status with failure tracking.
#[derive(Debug, Clone)]
pub struct ProviderHealth {
    pub available: bool,
    pub latency_ms: Option<u64>,
    pub last_error: Option<String>,
    pub last_check_at: std::time::Instant,
    pub consecutive_failures: u32,
}

impl Default for ProviderHealth {
    fn default() -> Self {
        Self {
            available: false,
            latency_ms: None,
            last_error: None,
            last_check_at: std::time::Instant::now(),
            consecutive_failures: 0,
        }
    }
}

/// Model availability status.
#[derive(Debug, Clone)]
pub struct ProviderAvailability {
    pub provider: String,
    pub available: bool,
    pub latency_ms: Option<u64>,
    pub models: Vec<String>,
    pub last_checked: chrono::DateTime<chrono::Utc>,
    pub last_error: Option<String>,
    pub health_is_estimated: bool,
}

/// Intelligent model router with provider-agnostic, role-aware, privacy-aware routing.
#[derive(Clone)]
pub struct ModelRouter {
    /// Available providers and their status
    pub providers: HashMap<String, ProviderAvailability>,
    /// Provider health tracking with failure counting
    pub provider_health: HashMap<String, ProviderHealth>,
    /// Default provider preferences by task type
    pub task_preferences: HashMap<TaskType, Vec<String>>,
    /// Privacy policy: minimum privacy level per task type
    pub privacy_policies: HashMap<TaskType, PrivacyRequirement>,
    /// Cost budget per task type (optional)
    pub cost_budgets: HashMap<TaskType, f32>,
    /// Latency thresholds per task type (ms)
    pub latency_thresholds: HashMap<TaskType, u64>,
    /// Last availability check timestamp
    last_availability_check: Option<chrono::DateTime<chrono::Utc>>,
    /// Cache TTL for availability checks (seconds)
    availability_cache_ttl: i64,
    /// Minimum interval between health checks
    health_check_interval: std::time::Duration,
    /// Last health check time
    last_health_check: Option<std::time::Instant>,
}

impl Default for ModelRouter {
    fn default() -> Self {
        let mut task_preferences = HashMap::new();
        task_preferences.insert(
            TaskType::Chat,
            vec!["ollama".into(), "deepseek".into(), "openrouter".into()],
        );
        task_preferences.insert(
            TaskType::Planner,
            vec!["deepseek".into(), "openrouter".into(), "ollama".into()],
        );
        task_preferences.insert(
            TaskType::ToolUse,
            vec!["deepseek".into(), "openrouter".into()],
        );
        task_preferences.insert(
            TaskType::Summarizer,
            vec!["ollama".into(), "deepseek".into()],
        );
        task_preferences.insert(
            TaskType::Extractor,
            vec!["deepseek".into(), "openrouter".into()],
        );
        task_preferences.insert(TaskType::Embedding, vec!["ollama".into(), "openai".into()]);

        let mut privacy_policies = HashMap::new();
        privacy_policies.insert(TaskType::Chat, PrivacyRequirement::Low);
        privacy_policies.insert(TaskType::Planner, PrivacyRequirement::Medium);
        privacy_policies.insert(TaskType::ToolUse, PrivacyRequirement::Medium);
        privacy_policies.insert(TaskType::Summarizer, PrivacyRequirement::Low);
        privacy_policies.insert(TaskType::Extractor, PrivacyRequirement::High);
        privacy_policies.insert(TaskType::Embedding, PrivacyRequirement::High);

        Self {
            providers: HashMap::new(),
            provider_health: HashMap::new(),
            task_preferences,
            privacy_policies,
            cost_budgets: HashMap::new(),
            latency_thresholds: HashMap::new(),
            last_availability_check: None,
            availability_cache_ttl: 60, // 1 minute default
            health_check_interval: std::time::Duration::from_secs(60),
            last_health_check: None,
        }
    }
}

impl ModelRouter {
    const DEFAULT_BASE_SCORE: f32 = 50.0;
    const DEFAULT_CAPABILITY: u8 = 5;
    const PRIVACY_CRITICAL_BONUS: f32 = 30.0;
    const PRIVACY_HIGH_BONUS: f32 = 20.0;
    const PRIVACY_MEDIUM_BONUS: f32 = 10.0;
    const PRIVACY_CRITICAL_PENALTY: f32 = -50.0;
    const PRIVACY_HIGH_PENALTY: f32 = -20.0;
    const TOOL_USE_BONUS: f32 = 25.0;
    const TOOL_USE_PENALTY: f32 = -15.0;
    const PLANNER_BONUS: f32 = 15.0;
    const EMBEDDING_BONUS: f32 = 20.0;
    const TOOLS_NEEDED_PENALTY: f32 = -20.0;
    const LATENCY_FAST_BONUS: f32 = 10.0;
    const LATENCY_SLOW_PENALTY: f32 = -10.0;
    const PREFERENCE_MULTIPLIER: f32 = 5.0;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_availability_cache_ttl(mut self, ttl_seconds: i64) -> Self {
        self.availability_cache_ttl = ttl_seconds;
        self
    }

    fn provider_env_key(provider: &str) -> Option<String> {
        let candidates: &[&str] = match provider {
            "deepseek" => &["DEEPSEEK_API_KEY"],
            "openrouter" => &["OPENROUTER_API_KEY"],
            "openai" => &["OPENAI_API_KEY"],
            _ => &[],
        };
        candidates.iter().find_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
    }

    /// Check and update provider availability.
    pub async fn check_availability(&mut self) -> Result<()> {
        let now = chrono::Utc::now();

        // Check Ollama
        let ollama_available = crate::ollama::is_ollama_available("").await;
        let ollama_latency = if ollama_available {
            Some(100) // Estimated 100ms for local
        } else {
            None
        };

        self.providers.insert(
            "ollama".into(),
            ProviderAvailability {
                provider: "ollama".into(),
                available: ollama_available,
                latency_ms: ollama_latency,
                models: vec![], // Could populate with installed models
                last_checked: now,
                last_error: if ollama_available {
                    None
                } else {
                    Some("ollama_unavailable".into())
                },
                health_is_estimated: false,
            },
        );

        // Cloud providers are only considered available when a key is configured and
        // lightweight probing succeeds. Without a key, they are explicitly unavailable.
        // Probes run in parallel via tokio::join! to reduce latency.
        let cloud_providers = vec!["deepseek", "openrouter", "openai"];
        let probes = cloud_providers.into_iter().map(|provider| async move {
            let has_key = Self::provider_env_key(provider).is_some();
            let result = if has_key {
                match Self::probe_provider_lightweight(provider).await {
                    Ok(latency) => (true, Some(latency), None, false),
                    Err(e) => (false, None, Some(e.to_string()), false),
                }
            } else {
                (
                    false,
                    None,
                    Some(format!("{}_api_key_missing", provider)),
                    false,
                )
            };
            (provider.to_string(), result)
        });

        let results = futures::future::join_all(probes).await;
        for (provider, (available, latency_ms, last_error, estimated)) in results {
            self.providers.insert(
                provider.clone(),
                ProviderAvailability {
                    provider,
                    available,
                    latency_ms,
                    models: vec![],
                    last_checked: now,
                    last_error,
                    health_is_estimated: estimated,
                },
            );
        }

        self.last_availability_check = Some(now);
        Ok(())
    }

    /// Non-blocking health check: only executes if interval has passed.
    pub async fn check_availability_if_needed(&mut self) -> Result<()> {
        if let Some(last_check) = self.last_health_check {
            if last_check.elapsed() < self.health_check_interval {
                return Ok(()); // Skip check
            }
        }

        // Run health probes in parallel, then update state sequentially.
        let providers = vec!["deepseek", "openrouter"];
        let probes = providers.into_iter().map(|provider| async move {
            match ModelRouter::probe_provider_lightweight(provider).await {
                Ok(latency) => (provider.to_string(), true, Some(latency), None),
                Err(e) => (provider.to_string(), false, None, Some(e.to_string())),
            }
        });

        let results = futures::future::join_all(probes).await;
        for (provider, available, latency_ms, last_error) in results {
            let entry = self.provider_health.entry(provider).or_default();
            if available {
                entry.available = true;
                entry.latency_ms = latency_ms;
                entry.last_error = None;
                entry.consecutive_failures = 0;
            } else {
                entry.available = false;
                entry.latency_ms = None;
                entry.last_error = last_error;
                entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
                // Mark as unavailable after 3 consecutive failures
                if entry.consecutive_failures >= 3 {
                    entry.available = false;
                }
            }
            entry.last_check_at = std::time::Instant::now();
        }

        self.last_health_check = Some(std::time::Instant::now());
        Ok(())
    }

    /// Lightweight probe using HEAD request to provider's model list API.
    async fn probe_provider_lightweight(provider: &str) -> Result<u64> {
        let url = match provider {
            "deepseek" => "https://api.deepseek.com/models",
            "openrouter" => "https://openrouter.ai/api/v1/models",
            "openai" => "https://api.openai.com/v1/models",
            _ => return Err(anyhow::anyhow!("unknown provider: {}", provider)),
        };

        let start = std::time::Instant::now();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;

        let res = client.head(url).send().await?;

        // Accept 2xx and 404 as "available" (API exists even if auth fails)
        if res.status().is_success() || res.status() == 404 {
            Ok(start.elapsed().as_millis() as u64)
        } else {
            Err(anyhow::anyhow!(
                "provider returned status: {}",
                res.status()
            ))
        }
    }

    /// Check if availability cache is stale.
    pub fn is_availability_stale(&self) -> bool {
        if let Some(last_check) = self.last_availability_check {
            let elapsed = chrono::Utc::now().signed_duration_since(last_check);
            elapsed.num_seconds() > self.availability_cache_ttl
        } else {
            true
        }
    }

    /// Score a provider for a given task.
    fn score_provider(
        &self,
        provider: &str,
        task_type: TaskType,
        privacy_requirement: PrivacyRequirement,
        tools_needed: bool,
    ) -> Option<ModelRouteScore> {
        // Check provider health first (if available), fallback to providers map
        let (is_available, latency_ms) = if let Some(health) = self.provider_health.get(provider) {
            (health.available, health.latency_ms)
        } else if let Some(availability) = self.providers.get(provider) {
            (availability.available, availability.latency_ms)
        } else {
            return None;
        };

        if !is_available {
            return None;
        }

        let mut score = Self::DEFAULT_BASE_SCORE;
        let mut capability = Self::DEFAULT_CAPABILITY;

        // Privacy matching
        match (privacy_requirement, provider) {
            (PrivacyRequirement::Critical, "ollama") => {
                score += Self::PRIVACY_CRITICAL_BONUS;
                capability += 3;
            }
            (PrivacyRequirement::High, "ollama") => {
                score += Self::PRIVACY_HIGH_BONUS;
                capability += 2;
            }
            (PrivacyRequirement::Medium, "ollama") => {
                score += Self::PRIVACY_MEDIUM_BONUS;
            }
            (PrivacyRequirement::Critical, _) => {
                score += Self::PRIVACY_CRITICAL_PENALTY;
            }
            (PrivacyRequirement::High, _) => {
                score += Self::PRIVACY_HIGH_PENALTY;
            }
            _ => {}
        }

        // Task-specific capabilities
        match task_type {
            TaskType::ToolUse if provider == "deepseek" || provider == "openrouter" => {
                score += Self::TOOL_USE_BONUS;
                capability += 3;
            }
            TaskType::ToolUse if provider == "ollama" => {
                score += Self::TOOL_USE_PENALTY;
                capability -= 2;
            }
            TaskType::Planner if provider == "deepseek" || provider == "openrouter" => {
                score += Self::PLANNER_BONUS;
                capability += 2;
            }
            TaskType::Embedding if provider == "ollama" || provider == "openai" => {
                score += Self::EMBEDDING_BONUS;
                capability += 3;
            }
            _ => {}
        }

        // Tools requirement
        if tools_needed && provider == "ollama" {
            score += Self::TOOLS_NEEDED_PENALTY;
        }

        // Latency bonus
        if let Some(latency) = latency_ms {
            if latency < 200 {
                score += Self::LATENCY_FAST_BONUS;
            } else if latency > 1000 {
                score += Self::LATENCY_SLOW_PENALTY;
            }
        }

        // Provider preference from task preferences
        if let Some(preferences) = self.task_preferences.get(&task_type) {
            if let Some(pos) = preferences.iter().position(|p| p == provider) {
                score += (5 - pos.min(4)) as f32 * Self::PREFERENCE_MULTIPLIER;
            }
        }

        Some(ModelRouteScore {
            provider: provider.to_string(),
            model: self
                .providers
                .get(provider)
                .and_then(|a| a.models.first().cloned())
                .unwrap_or_else(|| "default".to_string()),
            score: score.clamp(0.0, 100.0),
            latency_ms,
            cost_per_1k_tokens: None,
            capability_score: capability.min(10),
            privacy_level: privacy_requirement,
        })
    }

    /// Route a task to the best available provider.
    pub fn route(
        &self,
        task_type: TaskType,
        tools_needed: bool,
        custom_privacy: Option<PrivacyRequirement>,
    ) -> Result<ModelRouteDecision> {
        let privacy_requirement = custom_privacy
            .or_else(|| self.privacy_policies.get(&task_type).copied())
            .unwrap_or(PrivacyRequirement::Low);

        if matches!(
            privacy_requirement,
            PrivacyRequirement::High | PrivacyRequirement::Critical
        ) {
            let local_available =
                self.score_provider("ollama", task_type, privacy_requirement, tools_needed);
            if local_available.is_none() {
                return Err(anyhow::anyhow!(
                    "No local provider available for {:?} privacy task {:?}",
                    privacy_requirement,
                    task_type
                ));
            }
        }

        let mut scores = Vec::new();
        for provider in self.providers.keys() {
            if let Some(score) =
                self.score_provider(provider, task_type, privacy_requirement, tools_needed)
            {
                scores.push(score);
            }
        }

        if scores.is_empty() {
            return Err(anyhow::anyhow!(
                "No available providers for task {:?}",
                task_type
            ));
        }

        // Sort by score descending
        scores.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let best = &scores[0];
        let fallback = scores.get(1);

        let route_type = if best.provider == "ollama" {
            "local"
        } else {
            "cloud"
        };

        let reason = format!(
            "Task: {:?}, Privacy: {:?}, Tools: {}, Best provider: {} (score: {:.1})",
            task_type, privacy_requirement, tools_needed, best.provider, best.score
        );

        Ok(ModelRouteDecision {
            provider: best.provider.clone(),
            model: best.model.clone(),
            route_type: route_type.to_string(),
            prefer_local: best.provider == "ollama",
            reason,
            privacy_level: match privacy_requirement {
                PrivacyRequirement::Low => RedactionLevel::None,
                PrivacyRequirement::Medium => RedactionLevel::Light,
                PrivacyRequirement::High => RedactionLevel::Strict,
                PrivacyRequirement::Critical => RedactionLevel::LocalOnly,
            },
            estimated_latency_ms: best.latency_ms,
            fallback_provider: fallback.map(|s| s.provider.clone()),
            fallback_model: fallback.map(|s| s.model.clone()),
        })
    }

    pub fn route_with_hs_packet(
        &self,
        task_type: TaskType,
        tools_needed: bool,
        hs_packet: &RuntimeHSPacket,
    ) -> Result<ModelRouteDecision> {
        let hs_requires_local_only = hs_packet
            .selected_policies
            .iter()
            .any(|policy| policy.route == Some(crate::agent::ModelRoutePolicy::LocalOnly));
        let mut decision = self.route(
            task_type,
            tools_needed,
            hs_requires_local_only.then_some(PrivacyRequirement::Critical),
        )?;
        if hs_requires_local_only {
            decision.reason = format!(
                "{}; HS policy enforced LocalOnly via {:?}",
                decision.reason, hs_packet.audit.selected_policy_ids
            );
        }
        Ok(decision)
    }

    /// Quick route for chat messages (backward compatible with existing scheduler logic).
    pub fn route_chat(
        &self,
        tools_prompt: Option<&str>,
        _prefer_local: bool,
    ) -> Result<ModelRouteDecision> {
        let tools_needed = tools_prompt.map(|p| !p.trim().is_empty()).unwrap_or(false);

        // If tools are needed and we have cloud providers, prefer cloud
        if tools_needed {
            if let Ok(decision) = self.route(TaskType::ToolUse, true, None) {
                if decision.provider != "ollama" {
                    return Ok(decision);
                }
            }
        }

        // Otherwise use normal chat routing
        self.route(TaskType::Chat, tools_needed, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_router() -> ModelRouter {
        let mut router = ModelRouter::new();
        router.providers.insert(
            "ollama".into(),
            ProviderAvailability {
                provider: "ollama".into(),
                available: true,
                latency_ms: Some(100),
                models: vec!["qwen2.5:7b".into()],
                last_checked: chrono::Utc::now(),
                last_error: None,
                health_is_estimated: false,
            },
        );
        router.providers.insert(
            "deepseek".into(),
            ProviderAvailability {
                provider: "deepseek".into(),
                available: true,
                latency_ms: Some(500),
                models: vec!["deepseek-chat".into()],
                last_checked: chrono::Utc::now(),
                last_error: None,
                health_is_estimated: false,
            },
        );
        router.providers.insert(
            "openrouter".into(),
            ProviderAvailability {
                provider: "openrouter".into(),
                available: true,
                latency_ms: Some(600),
                models: vec!["openai/gpt-4o".into()],
                last_checked: chrono::Utc::now(),
                last_error: None,
                health_is_estimated: false,
            },
        );
        router
    }

    #[test]
    fn test_route_chat_local_preferred() {
        let router = create_test_router();
        let decision = router.route_chat(None, true).unwrap();

        // Should prefer ollama when no tools and prefer_local
        assert_eq!(decision.provider, "ollama");
        assert!(decision.prefer_local);
    }

    #[test]
    fn test_route_chat_with_tools() {
        let router = create_test_router();
        let decision = router.route_chat(Some("tools available"), true).unwrap();

        // Should prefer cloud when tools are needed
        assert_ne!(decision.provider, "ollama");
    }

    #[test]
    fn test_route_extractor_high_privacy() {
        let router = create_test_router();
        let decision = router
            .route(TaskType::Extractor, false, Some(PrivacyRequirement::High))
            .unwrap();

        // High privacy should prefer local
        assert_eq!(decision.provider, "ollama");
    }

    #[test]
    fn test_availability_stale() {
        let router = ModelRouter::new();
        assert!(router.is_availability_stale());
    }

    #[test]
    fn test_no_available_providers() {
        let router = ModelRouter::new();
        let result = router.route(TaskType::Chat, false, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_unavailable_triggers_fallback() {
        let mut router = create_test_router();
        // Mark deepseek as unavailable via provider_health
        router.provider_health.insert(
            "deepseek".into(),
            ProviderHealth {
                available: false,
                latency_ms: None,
                last_error: Some("connection refused".into()),
                last_check_at: std::time::Instant::now(),
                consecutive_failures: 3,
            },
        );

        // Route should not pick deepseek
        let decision = router.route_chat(None, true).unwrap();
        assert_ne!(decision.provider, "deepseek");
        // Fallback should be the second-best available provider
        assert!(decision.fallback_provider.is_some());
    }

    #[test]
    fn test_provider_health_overrides_availability() {
        let mut router = create_test_router();
        // providers says available=true, but health says false
        router.provider_health.insert(
            "ollama".into(),
            ProviderHealth {
                available: false,
                latency_ms: None,
                last_error: Some("ollama not running".into()),
                last_check_at: std::time::Instant::now(),
                consecutive_failures: 3,
            },
        );

        let decision = router.route_chat(None, true).unwrap();
        // Should not pick ollama even though providers map says available
        assert_ne!(decision.provider, "ollama");
    }

    #[test]
    fn test_high_privacy_requires_local_provider() {
        let mut router = create_test_router();
        router.provider_health.insert(
            "ollama".into(),
            ProviderHealth {
                available: false,
                latency_ms: None,
                last_error: Some("ollama not running".into()),
                last_check_at: std::time::Instant::now(),
                consecutive_failures: 3,
            },
        );

        let result = router.route(TaskType::Extractor, false, Some(PrivacyRequirement::High));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No local provider"));
    }
}
