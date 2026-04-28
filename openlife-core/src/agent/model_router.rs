use crate::agent::ModelRouteTrace;
use crate::agent::types::RedactionLevel;
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
            local_model: if self.prefer_local { self.model.clone() } else { String::new() },
            reason: self.reason.clone(),
            privacy_level: self.privacy_level,
            latency_ms: self.estimated_latency_ms,
            retry_count: 0,
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
}

/// Intelligent model router with provider-agnostic, role-aware, privacy-aware routing.
pub struct ModelRouter {
    /// Available providers and their status
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
        task_preferences.insert(TaskType::Chat, vec!["ollama".into(), "deepseek".into(), "openrouter".into()]);
        task_preferences.insert(TaskType::Planner, vec!["deepseek".into(), "openrouter".into(), "ollama".into()]);
        task_preferences.insert(TaskType::ToolUse, vec!["deepseek".into(), "openrouter".into()]);
        task_preferences.insert(TaskType::Summarizer, vec!["ollama".into(), "deepseek".into()]);
        task_preferences.insert(TaskType::Extractor, vec!["deepseek".into(), "openrouter".into()]);
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
        
        self.providers.insert("ollama".into(), ProviderAvailability {
            provider: "ollama".into(),
            available: ollama_available,
            latency_ms: ollama_latency,
            models: vec![], // Could populate with installed models
            last_checked: now,
        });

        // Check cloud providers (basic connectivity check)
        // In production, this would do actual health checks
        for provider in &["deepseek", "openrouter", "openai"] {
            self.providers.insert(provider.to_string(), ProviderAvailability {
                provider: provider.to_string(),
                available: true, // Assume available unless proven otherwise
                latency_ms: Some(500), // Estimated 500ms for cloud
                models: vec![],
                last_checked: now,
            });
        }

        self.last_availability_check = Some(now);
        Ok(())
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
        let availability = self.providers.get(provider)?;
        if !availability.available {
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
            TaskType::ToolUse => {
                if provider == "deepseek" || provider == "openrouter" {
                    score += Self::TOOL_USE_BONUS;
                    capability += 3;
                } else if provider == "ollama" {
                    score += Self::TOOL_USE_PENALTY;
                    capability -= 2;
                }
            }
            TaskType::Planner => {
                if provider == "deepseek" || provider == "openrouter" {
                    score += Self::PLANNER_BONUS;
                    capability += 2;
                }
            }
            TaskType::Embedding => {
                if provider == "ollama" || provider == "openai" {
                    score += Self::EMBEDDING_BONUS;
                    capability += 3;
                }
            }
            _ => {}
        }

        // Tools requirement
        if tools_needed && provider == "ollama" {
            score += Self::TOOLS_NEEDED_PENALTY;
        }

        // Latency bonus
        if let Some(latency) = availability.latency_ms {
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
            model: availability.models.first().cloned().unwrap_or_else(|| "default".to_string()),
            score: score.max(0.0).min(100.0),
            latency_ms: availability.latency_ms,
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

        let mut scores = Vec::new();
        for provider in self.providers.keys() {
            if let Some(score) = self.score_provider(provider, task_type, privacy_requirement, tools_needed) {
                scores.push(score);
            }
        }

        if scores.is_empty() {
            return Err(anyhow::anyhow!("No available providers for task {:?}", task_type));
        }

        // Sort by score descending
        scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

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

        // Otherwise use normal chat routing
        self.route(TaskType::Chat, tools_needed, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_router() -> ModelRouter {
        let mut router = ModelRouter::new();
        router.providers.insert("ollama".into(), ProviderAvailability {
            provider: "ollama".into(),
            available: true,
            latency_ms: Some(100),
            models: vec!["qwen2.5:7b".into()],
            last_checked: chrono::Utc::now(),
        });
        router.providers.insert("deepseek".into(), ProviderAvailability {
            provider: "deepseek".into(),
            available: true,
            latency_ms: Some(500),
            models: vec!["deepseek-chat".into()],
            last_checked: chrono::Utc::now(),
        });
        router.providers.insert("openrouter".into(), ProviderAvailability {
            provider: "openrouter".into(),
            available: true,
            latency_ms: Some(600),
            models: vec!["openai/gpt-4o".into()],
            last_checked: chrono::Utc::now(),
        });
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
        let decision = router.route(TaskType::Extractor, false, Some(PrivacyRequirement::High)).unwrap();
        
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
}
