use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_openai_base")]
    pub openai_base: String,
    #[serde(default)]
    pub openai_key: String,
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    #[serde(default = "default_chat_model")]
    pub chat_model: String,
    #[serde(default = "default_embedding_enabled")]
    pub embedding_enabled: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            openai_base: default_openai_base(),
            openai_key: String::new(),
            embedding_model: default_embedding_model(),
            chat_model: default_chat_model(),
            embedding_enabled: default_embedding_enabled(),
        }
    }
}

fn default_provider() -> String {
    "openai".to_string()
}

fn default_openai_base() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_embedding_model() -> String {
    "text-embedding-3-small".to_string()
}

fn default_chat_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_embedding_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub prefer_local_model: bool,
    #[serde(default = "default_local_model")]
    pub local_model: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            llm: LlmConfig::default(),
            prefer_local_model: true,
            local_model: default_local_model(),
        }
    }
}

fn default_local_model() -> String {
    "llama2".to_string()
}

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Self = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        Self::load(&path).unwrap_or_default()
    }

    /// Get effective OpenAI base URL (env var overrides config file)
    pub fn effective_openai_base(&self) -> String {
        std::env::var("OPENAI_API_BASE").unwrap_or_else(|_| self.llm.openai_base.clone())
    }

    /// Get effective OpenAI API key (env var overrides config file)
    pub fn effective_openai_key(&self) -> String {
        self.effective_cloud_api_key()
    }

    pub fn effective_cloud_api_key(&self) -> String {
        if !self.llm.openai_key.trim().is_empty() {
            return self.llm.openai_key.clone();
        }
        match self.llm.provider.as_str() {
            "deepseek" => std::env::var("DEEPSEEK_API_KEY").unwrap_or_default(),
            "openrouter" => std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
            "openai" => std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            "siliconflow" => std::env::var("SILICONFLOW_API_KEY").unwrap_or_default(),
            "moonshot" => std::env::var("MOONSHOT_API_KEY").unwrap_or_default(),
            "dashscope" => std::env::var("DASHSCOPE_API_KEY").unwrap_or_default(),
            "zhipu" => std::env::var("ZHIPU_API_KEY").unwrap_or_default(),
            _ => std::env::var("DEEPSEEK_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .or_else(|_| std::env::var("OPENROUTER_API_KEY"))
                .unwrap_or_default(),
        }
    }

    pub fn effective_provider_label(&self) -> String {
        match self.llm.provider.as_str() {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn config_default_values() {
        let config = AppConfig::default();
        assert_eq!(config.llm.openai_base, "https://api.openai.com/v1");
        assert_eq!(config.llm.provider, "openai");
        assert_eq!(config.llm.embedding_model, "text-embedding-3-small");
        assert_eq!(config.llm.chat_model, "gpt-4o-mini");
        assert!(config.llm.embedding_enabled);
        assert_eq!(config.local_model, "llama2");
        assert!(config.prefer_local_model);
    }

    #[test]
    fn config_save_and_load_roundtrip() {
        let file = NamedTempFile::new().unwrap();
        let config = AppConfig {
            llm: LlmConfig {
                provider: "custom".into(),
                openai_base: "https://custom.com/v1".into(),
                openai_key: "sk-test".into(),
                embedding_model: "text-embedding-3-large".into(),
                chat_model: "gpt-4".into(),
                embedding_enabled: false,
            },
            prefer_local_model: true,
            local_model: "qwen2.5".into(),
        };
        config.save(file.path()).unwrap();
        let loaded = AppConfig::load(file.path()).unwrap();
        assert_eq!(loaded.llm.openai_base, config.llm.openai_base);
        assert_eq!(loaded.llm.provider, config.llm.provider);
        assert_eq!(loaded.llm.openai_key, config.llm.openai_key);
        assert_eq!(loaded.llm.embedding_model, config.llm.embedding_model);
        assert_eq!(loaded.llm.chat_model, config.llm.chat_model);
        assert_eq!(loaded.llm.embedding_enabled, config.llm.embedding_enabled);
        assert_eq!(loaded.prefer_local_model, config.prefer_local_model);
        assert_eq!(loaded.local_model, config.local_model);
    }

    #[test]
    fn config_load_or_default_uses_default_when_missing() {
        let path = "/tmp/nonexistent_openlife_config.yaml";
        let config = AppConfig::load_or_default(path);
        assert_eq!(config.llm.chat_model, "gpt-4o-mini");
    }

    #[test]
    fn config_effective_openai_base_env_override() {
        let config = AppConfig::default();
        std::env::set_var("OPENAI_API_BASE", "https://env.override.com/v1");
        assert_eq!(
            config.effective_openai_base(),
            "https://env.override.com/v1"
        );
        std::env::remove_var("OPENAI_API_BASE");
        assert_eq!(config.effective_openai_base(), config.llm.openai_base);
    }

    #[test]
    fn config_effective_openai_key_env_override() {
        let mut config = AppConfig::default();
        config.llm.openai_key = "from-config".into();
        std::env::set_var("OPENAI_API_KEY", "sk-env");
        assert_eq!(config.effective_openai_key(), "from-config");
        std::env::remove_var("OPENAI_API_KEY");
        assert_eq!(config.effective_openai_key(), "from-config");
    }

    #[test]
    fn config_deepseek_env_fallback() {
        let mut config = AppConfig::default();
        config.llm.provider = "deepseek".into();
        std::env::set_var("DEEPSEEK_API_KEY", "sk-deepseek");
        assert_eq!(config.effective_cloud_api_key(), "sk-deepseek");
        std::env::remove_var("DEEPSEEK_API_KEY");
    }
}
