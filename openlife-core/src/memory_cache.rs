use crate::life_model::LifeModel;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Hot Memory Cache: a lightweight, always-in-memory summary of the user's
/// core identity, top values, current goals and recent state.
///
/// This cache is rebuilt whenever the LifeModel changes and injected into
/// every chat prompt so the model always has the most important context
/// without relying on vector retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotMemoryCache {
    /// Core identity summary (name, mission, philosophy)
    pub identity_summary: String,
    /// Top 3-5 values ranked by strength
    pub top_values: Vec<String>,
    /// Active short-term + daily goals
    pub current_goals: Vec<String>,
    /// Recent emotional state and focus areas
    pub recent_state: String,
    /// When the cache was last refreshed
    pub last_refreshed: String,
    /// LifeModel version for invalidation check
    pub life_model_version: String,
}

impl Default for HotMemoryCache {
    fn default() -> Self {
        Self {
            identity_summary: String::new(),
            top_values: Vec::new(),
            current_goals: Vec::new(),
            recent_state: String::new(),
            last_refreshed: chrono::Utc::now().to_rfc3339(),
            life_model_version: String::new(),
        }
    }
}

impl HotMemoryCache {
    /// Build a hot cache from the current LifeModel.
    pub fn from_life_model(model: &LifeModel) -> Self {
        let identity = &model.identity;
        let goals = &model.goals;
        let state = &model.state;

        // Identity summary: name + mission + key traits
        let identity_summary = format!(
            "你是{}，{}。你的核心哲学是：{}。",
            identity.name,
            if identity.mission_statement.is_empty() {
                "正在探索人生使命"
            } else {
                &identity.mission_statement
            },
            if identity.life_philosophy.is_empty() {
                "持续成长"
            } else {
                &identity.life_philosophy
            }
        );

        // Top values: sort by inferred strength (first ones are strongest)
        let mut top_values: Vec<String> = identity
            .values
            .iter()
            .take(5)
            .map(|v| format!("{} ({})", v.name, v.description))
            .collect();
        if top_values.is_empty() {
            top_values.push("尚未明确核心价值观".to_string());
        }

        // Current goals: active short-term + daily goals
        let mut current_goals: Vec<String> = goals
            .short_term
            .iter()
            .filter(|g| g.status != "completed")
            .take(3)
            .map(|g| {
                format!(
                    "{} (优先级: {}, 进度: {}%)",
                    g.name,
                    g.priority,
                    (g.progress * 100.0) as i32
                )
            })
            .collect();
        // Add daily goals if any
        for dg in goals.daily.iter().take(3) {
            let status = if dg.done { "✓" } else { "○" };
            current_goals.push(format!("{} 每日目标: {}", status, dg.name));
        }
        if current_goals.is_empty() {
            current_goals.push("当前没有活跃的短期目标".to_string());
        }

        // Recent state: emotional + focus + alerts
        let mut state_parts = Vec::new();
        if !state.emotional_state.current_mood.is_empty() {
            state_parts.push(format!("心情: {}", state.emotional_state.current_mood));
        }
        if !state.current_focus.is_empty() {
            state_parts.push(format!("当前专注: {}", state.current_focus));
        }
        let active_alerts: Vec<_> = state
            .alerts
            .iter()
            .take(2)
            .map(|a| format!("⚠ {:?}: {}", a.level, a.message))
            .collect();
        for alert in active_alerts {
            state_parts.push(alert);
        }
        let recent_state = if state_parts.is_empty() {
            "状态平稳".to_string()
        } else {
            state_parts.join("；")
        };

        Self {
            identity_summary,
            top_values,
            current_goals,
            recent_state,
            last_refreshed: chrono::Utc::now().to_rfc3339(),
            life_model_version: model.metadata.version.clone(),
        }
    }

    /// Convert the cache into a context string suitable for prompt injection.
    pub fn to_context_string(&self) -> String {
        let values_text = self.top_values.join("、");
        let goals_text = self.current_goals.join("\n- ");
        format!(
            "【核心记忆摘要】\n身份: {}\n核心价值观: {}\n当前目标:\n- {}\n近期状态: {}\n",
            self.identity_summary, values_text, goals_text, self.recent_state
        )
    }

    /// Check if the cache is stale (LifeModel version mismatch).
    pub fn is_stale(&self, model: &LifeModel) -> bool {
        self.life_model_version != model.metadata.version
    }

    /// Refresh the cache from a new LifeModel.
    pub fn refresh(&mut self, model: &LifeModel) {
        *self = Self::from_life_model(model);
    }
}

/// Thread-safe shared hot cache handle.
pub type SharedHotCache = Arc<Mutex<HotMemoryCache>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_model() -> LifeModel {
        let mut model = LifeModel::default_model();
        model.identity.name = "测试用户".to_string();
        model.identity.mission_statement = "成为优秀的工程师".to_string();
        model.identity.life_philosophy = "持续学习".to_string();
        model.identity.values.push(crate::life_model::ValueItem {
            name: "成长".to_string(),
            description: "不断进步".to_string(),
            weight: 9,
        });
        model.state.emotional_state.current_mood = "积极".to_string();
        model.state.current_focus = "学习Rust".to_string();
        model
    }

    #[test]
    fn hot_cache_builds_from_life_model() {
        let model = sample_model();
        let cache = HotMemoryCache::from_life_model(&model);
        assert!(cache.identity_summary.contains("测试用户"));
        assert!(!cache.top_values.is_empty());
        assert_eq!(cache.life_model_version, model.metadata.version);
    }

    #[test]
    fn hot_cache_context_string_includes_all_parts() {
        let model = sample_model();
        let cache = HotMemoryCache::from_life_model(&model);
        let ctx = cache.to_context_string();
        assert!(ctx.contains("核心记忆摘要"));
        assert!(ctx.contains("测试用户"));
        assert!(ctx.contains("成长"));
        assert!(ctx.contains("近期状态"));
    }

    #[test]
    fn hot_cache_detects_stale_version() {
        let model = sample_model();
        let cache = HotMemoryCache::from_life_model(&model);
        assert!(!cache.is_stale(&model));

        let mut new_model = model.clone();
        new_model.metadata.version = "different".to_string();
        assert!(cache.is_stale(&new_model));
    }

    #[test]
    fn hot_cache_refresh_updates_version() {
        let mut model = sample_model();
        let mut cache = HotMemoryCache::from_life_model(&model);
        model.metadata.version = "v2".to_string();
        model.identity.name = "新名字".to_string();
        cache.refresh(&model);
        assert_eq!(cache.life_model_version, "v2");
        assert!(cache.identity_summary.contains("新名字"));
    }
}
