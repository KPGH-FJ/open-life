use crate::agent::types::RedactionLevel;
use crate::agent::RuntimeHSPacket;
use crate::agent::{
    GovernanceDecisionKind, GovernorDecisionReport, LifeModelGovernor, ModelRouteGovernanceInput,
    ModelRouteTrace, RiskLevel,
};
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
    pub governance_report: Option<GovernorDecisionReport>,
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
    /// Canonical provider observations supplied by the product-owned provider boundary.
    pub providers: HashMap<String, ProviderAvailability>,
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
            task_preferences,
            privacy_policies,
            cost_budgets: HashMap::new(),
            latency_thresholds: HashMap::new(),
            last_availability_check: None,
            availability_cache_ttl: 60, // 1 minute default
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

    pub fn seed_configured_cloud_provider(
        &mut self,
        provider: &str,
        model: &str,
        has_configured_key: bool,
    ) {
        let provider = provider.trim();
        if provider.is_empty() || provider == "ollama" || self.providers.contains_key(provider) {
            return;
        }

        let model = model.trim();
        self.providers.insert(
            provider.to_string(),
            ProviderAvailability {
                provider: provider.to_string(),
                available: has_configured_key,
                latency_ms: None,
                models: if model.is_empty() {
                    vec![]
                } else {
                    vec![model.to_string()]
                },
                last_checked: chrono::Utc::now(),
                last_error: if has_configured_key {
                    None
                } else {
                    Some(format!("{}_api_key_missing", provider))
                },
                health_is_estimated: true,
            },
        );
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
        if matches!(
            privacy_requirement,
            PrivacyRequirement::High | PrivacyRequirement::Critical
        ) && provider != "ollama"
        {
            return None;
        }

        let availability = self.providers.get(provider)?;
        let (is_available, latency_ms) = (availability.available, availability.latency_ms);

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

        Ok(Self::decision_from_ranked_scores(
            task_type,
            privacy_requirement,
            tools_needed,
            &scores,
        ))
    }

    fn decision_from_ranked_scores(
        task_type: TaskType,
        privacy_requirement: PrivacyRequirement,
        tools_needed: bool,
        scores: &[ModelRouteScore],
    ) -> ModelRouteDecision {
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

        ModelRouteDecision {
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
            governance_report: None,
        }
    }

    pub fn route_with_hs_packet(
        &self,
        task_type: TaskType,
        tools_needed: bool,
        hs_packet: &RuntimeHSPacket,
    ) -> Result<ModelRouteDecision> {
        let local_model_available = self
            .score_provider(
                "ollama",
                task_type,
                PrivacyRequirement::Critical,
                tools_needed,
            )
            .is_some();
        let governor_decision = LifeModelGovernor.govern_model_route(ModelRouteGovernanceInput {
            hs_packet: Some(hs_packet.clone()),
            risk_level: if crate::agent::governor::packet_requires_local_only(Some(hs_packet)) {
                RiskLevel::High
            } else {
                RiskLevel::Low
            },
            local_model_available,
        });

        if governor_decision.kind == GovernanceDecisionKind::Block {
            return Err(anyhow::anyhow!("{}", governor_decision.reason));
        }

        let hs_requires_local_only =
            governor_decision.kind == GovernanceDecisionKind::RequireLocalOnly;
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
            decision.fallback_provider = None;
            decision.fallback_model = None;
            decision.privacy_level = RedactionLevel::LocalOnly;
        }
        decision.governance_report = Some(governor_decision.to_report());
        Ok(decision)
    }

    /// Quick route for chat messages (backward compatible with existing scheduler logic).
    pub fn route_chat(
        &self,
        tools_prompt: Option<&str>,
        prefer_local: bool,
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

        if !prefer_local {
            let privacy_requirement = self
                .privacy_policies
                .get(&TaskType::Chat)
                .copied()
                .unwrap_or(PrivacyRequirement::Low);
            let mut cloud_scores = self
                .providers
                .keys()
                .filter(|provider| provider.as_str() != "ollama")
                .filter_map(|provider| {
                    self.score_provider(provider, TaskType::Chat, privacy_requirement, tools_needed)
                })
                .collect::<Vec<_>>();
            cloud_scores.sort_by(|left, right| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            if !cloud_scores.is_empty() {
                return Ok(Self::decision_from_ranked_scores(
                    TaskType::Chat,
                    privacy_requirement,
                    tools_needed,
                    &cloud_scores,
                ));
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

    fn create_fast_cloud_slow_local_router() -> ModelRouter {
        let mut router = create_test_router();
        router.providers.get_mut("ollama").unwrap().latency_ms = Some(5_000);
        router.providers.get_mut("deepseek").unwrap().latency_ms = Some(10);
        router.providers.get_mut("openrouter").unwrap().latency_ms = Some(20);
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
    fn test_route_chat_cloud_preference_does_not_silently_select_local() {
        let router = create_test_router();
        let decision = router.route_chat(None, false).unwrap();

        assert_ne!(decision.provider, "ollama");
        assert_eq!(decision.route_type, "cloud");
        assert!(!decision.prefer_local);
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
    fn test_high_privacy_hard_filters_cloud_even_when_tool_use_prefers_fast_cloud() {
        let router = create_fast_cloud_slow_local_router();
        let decision = router
            .route(TaskType::ToolUse, true, Some(PrivacyRequirement::High))
            .unwrap();

        assert_eq!(decision.provider, "ollama");
        assert_eq!(decision.route_type, "local");
        assert!(decision.prefer_local);
        assert_eq!(decision.privacy_level, RedactionLevel::Strict);
        assert_eq!(decision.fallback_provider, None);
        assert_eq!(decision.fallback_model, None);
    }

    #[test]
    fn test_critical_privacy_hard_filters_cloud_even_when_planner_prefers_fast_cloud() {
        let router = create_fast_cloud_slow_local_router();
        let decision = router
            .route(TaskType::Planner, true, Some(PrivacyRequirement::Critical))
            .unwrap();

        assert_eq!(decision.provider, "ollama");
        assert_eq!(decision.route_type, "local");
        assert!(decision.prefer_local);
        assert_eq!(decision.privacy_level, RedactionLevel::LocalOnly);
        assert_eq!(decision.fallback_provider, None);
        assert_eq!(decision.fallback_model, None);
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
        router.providers.get_mut("deepseek").unwrap().available = false;
        router.providers.get_mut("deepseek").unwrap().last_error =
            Some("connection refused".into());

        // Route should not pick deepseek
        let decision = router.route_chat(None, true).unwrap();
        assert_ne!(decision.provider, "deepseek");
        // Fallback should be the second-best available provider
        assert!(decision.fallback_provider.is_some());
    }

    #[test]
    fn test_provider_observation_is_the_only_availability_authority() {
        let source = include_str!("model_router.rs");
        let removed_health_map_field = ["provider", "_health: HashMap"].concat();
        let removed_health_map_access = [".provider", "_health"].concat();
        let removed_probe = ["probe_provider", "_lightweight"].concat();
        let removed_env_lookup = ["provider", "_env_key"].concat();
        let removed_direct_http = ["reqwest", "::"].concat();
        let removed_environment_lookup = ["std::env", "::var"].concat();

        assert!(!source.contains(&removed_health_map_field));
        assert!(!source.contains(&removed_health_map_access));
        assert!(!source.contains(&removed_probe));
        assert!(!source.contains(&removed_env_lookup));
        assert!(!source.contains(&removed_direct_http));
        assert!(!source.contains(&removed_environment_lookup));
    }

    #[test]
    fn test_high_privacy_requires_local_provider() {
        let mut router = create_test_router();
        router.providers.get_mut("ollama").unwrap().available = false;
        router.providers.get_mut("ollama").unwrap().last_error = Some("ollama not running".into());

        let result = router.route(TaskType::Extractor, false, Some(PrivacyRequirement::High));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No local provider"));
    }

    #[test]
    fn test_critical_privacy_requires_local_provider_and_never_cloud_fallback() {
        let mut router = create_fast_cloud_slow_local_router();
        router.providers.get_mut("ollama").unwrap().available = false;
        router.providers.get_mut("ollama").unwrap().last_error = Some("ollama not running".into());

        let result = router.route(TaskType::Planner, true, Some(PrivacyRequirement::Critical));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No local provider"));
    }
}
