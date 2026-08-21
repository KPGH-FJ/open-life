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
    /// Runtime-only secret hydrated from the OS credential store or environment.
    /// Legacy plaintext is still accepted on read so startup can migrate it.
    #[serde(default, skip_serializing)]
    pub openai_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openai_key_ref: Option<String>,
    /// Non-secret identity version for invalidating provider validation when the bound
    /// credential or endpoint changes. The secret itself is never hashed into config.
    #[serde(default)]
    pub credential_version: u64,
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
            openai_key_ref: None,
            credential_version: 0,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPolicy {
    #[serde(default = "default_network_enabled")]
    pub enabled: bool,
    #[serde(default = "default_network_default_decision")]
    pub default_decision: String,
    #[serde(default)]
    pub domain_allowlist: Vec<String>,
    #[serde(default)]
    pub domain_denylist: Vec<String>,
    #[serde(default)]
    pub tool_overrides: std::collections::HashMap<String, String>,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            enabled: default_network_enabled(),
            default_decision: default_network_default_decision(),
            domain_allowlist: Vec::new(),
            domain_denylist: Vec::new(),
            tool_overrides: std::collections::HashMap::new(),
        }
    }
}

fn default_network_enabled() -> bool {
    true
}

fn default_network_default_decision() -> String {
    "ask".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    #[serde(default = "default_ollama_cache_ttl_seconds")]
    pub ollama_cache_ttl_seconds: u64,
    #[serde(default = "default_memory_search_top_k")]
    pub memory_search_top_k: usize,
    /// Product-level master switch for Agent Memory recall and learning.
    /// Explicit user management remains available while this is disabled.
    #[serde(default = "default_agent_memory_enabled")]
    pub agent_memory_enabled: bool,
    /// Safe paths for file.read tool (workspace directories allowed for file access)
    #[serde(default)]
    pub safe_paths: Vec<String>,
    /// Network access policy for web tools
    #[serde(default)]
    pub network_policy: NetworkPolicy,
    /// ICS calendar file paths for calendar.read tool
    #[serde(default)]
    pub calendar_ics_paths: Vec<String>,
    /// Web search provider: "duckduckgo" (default), "brave", "deepseek", or "searxng"
    #[serde(default = "default_search_provider")]
    pub search_provider: String,
    /// Runtime-only API key for the web search provider.
    #[serde(default, skip_serializing)]
    pub search_provider_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_provider_key_ref: Option<String>,
    /// Base URL for SearXNG instance (e.g. "https://searx.example.com")
    #[serde(default)]
    pub searxng_url: String,
    /// Additional bounded knowledge roots for Main Chat context loading.
    #[serde(default)]
    pub knowledge_roots: Vec<String>,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            ollama_cache_ttl_seconds: default_ollama_cache_ttl_seconds(),
            memory_search_top_k: default_memory_search_top_k(),
            agent_memory_enabled: default_agent_memory_enabled(),
            safe_paths: Vec::new(),
            network_policy: NetworkPolicy::default(),
            calendar_ics_paths: Vec::new(),
            search_provider: default_search_provider(),
            search_provider_key: String::new(),
            search_provider_key_ref: None,
            searxng_url: String::new(),
            knowledge_roots: Vec::new(),
        }
    }
}

fn default_ollama_cache_ttl_seconds() -> u64 {
    10
}

fn default_memory_search_top_k() -> usize {
    3
}

fn default_agent_memory_enabled() -> bool {
    true
}

fn default_search_provider() -> String {
    "duckduckgo".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub prefer_local_model: bool,
    #[serde(default = "default_local_model")]
    pub local_model: String,
    #[serde(default)]
    pub system: SystemConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            llm: LlmConfig::default(),
            prefer_local_model: true,
            local_model: default_local_model(),
            system: SystemConfig::default(),
        }
    }
}

fn default_local_model() -> String {
    "llama2".to_string()
}

impl AppConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut config: Self = serde_yaml::from_str(&content)?;
        config.normalize_provider_from_base();
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

    /// Load config with warning on parse failure.
    /// Returns (config, optional_warning).
    pub fn load_or_default_with_warning(path: impl AsRef<Path>) -> (Self, Option<String>) {
        let path = path.as_ref();
        if !path.exists() {
            return (Self::default(), None);
        }
        match Self::load(path) {
            Ok(config) => (config, None),
            Err(e) => (Self::default(), Some(format!("配置文件读取失败: {}", e))),
        }
    }

    pub fn normalize_provider_from_base(&mut self) {
        if self.llm.provider != default_provider() {
            self.normalize_provider_embedding_defaults();
            return;
        }
        let base = self.llm.openai_base.to_lowercase();
        self.llm.provider = if base.contains("api.deepseek.com") {
            "deepseek".to_string()
        } else if base.contains("openrouter.ai") {
            "openrouter".to_string()
        } else if base.contains("api.siliconflow.cn") {
            "siliconflow".to_string()
        } else if base.contains("api.moonshot.cn") {
            "moonshot".to_string()
        } else if base.contains("dashscope.aliyuncs.com") {
            "dashscope".to_string()
        } else if base.contains("open.bigmodel.cn") {
            "zhipu".to_string()
        } else {
            self.llm.provider.clone()
        };
        self.normalize_provider_embedding_defaults();
    }

    fn normalize_provider_embedding_defaults(&mut self) {
        if self.llm.provider == "deepseek" {
            self.llm.embedding_enabled = false;
        }
    }

    /// Get the configured provider base URL.
    ///
    /// `OPENAI_API_BASE` used to redirect provider execution independently of
    /// the canonical config and credential identity. That made it possible for
    /// an official environment credential to follow a custom endpoint. Custom
    /// endpoints remain supported through the explicit Settings/config field.
    pub fn effective_openai_base(&self) -> String {
        self.llm.openai_base.clone()
    }

    /// Get the explicit configured key, or the provider-specific environment
    /// key only when the configured endpoint is that provider's official one.
    pub fn effective_openai_key(&self) -> String {
        self.effective_cloud_api_key()
    }

    pub fn effective_cloud_api_key(&self) -> String {
        crate::llm::effective_api_key_for_endpoint(
            &self.llm.provider,
            &self.llm.openai_base,
            &self.llm.openai_key,
        )
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
        let mut config = AppConfig {
            llm: LlmConfig {
                provider: "custom".into(),
                openai_base: "https://custom.com/v1".into(),
                openai_key: "sk-test".into(),
                openai_key_ref: Some("keychain://com.openlife.desktop/provider-api-key".into()),
                credential_version: 1,
                embedding_model: "text-embedding-3-large".into(),
                chat_model: "gpt-4".into(),
                embedding_enabled: false,
            },
            prefer_local_model: true,
            local_model: "qwen2.5".into(),
            system: SystemConfig::default(),
        };
        config.system.search_provider_key = "sk-search-test".into();
        config.system.search_provider_key_ref =
            Some("keychain://com.openlife.desktop/search-provider-api-key".into());
        config.save(file.path()).unwrap();
        let loaded = AppConfig::load(file.path()).unwrap();
        assert_eq!(loaded.llm.openai_base, config.llm.openai_base);
        assert_eq!(loaded.llm.provider, config.llm.provider);
        assert!(loaded.llm.openai_key.is_empty());
        assert_eq!(loaded.llm.embedding_model, config.llm.embedding_model);
        assert_eq!(loaded.llm.chat_model, config.llm.chat_model);
        assert_eq!(loaded.llm.embedding_enabled, config.llm.embedding_enabled);
        assert_eq!(loaded.prefer_local_model, config.prefer_local_model);
        assert_eq!(loaded.local_model, config.local_model);

        let saved = fs::read_to_string(file.path()).unwrap();
        assert!(!saved.contains("sk-test"));
        assert!(!saved.contains("sk-search-test"));
        assert!(!saved.contains("openai_key:"));
        assert!(saved.contains("openai_key_ref:"));
        assert!(!saved.contains("search_provider_key:"));
        assert!(saved.contains("search_provider_key_ref:"));
    }

    #[test]
    fn legacy_chat_proposal_config_is_ignored_and_not_reserialized() {
        let file = NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            "chat_proposal:\n  enabled: true\n  confidence_threshold: 0.9\n  min_message_length: 5\n  cooldown_seconds: 30\n",
        )
        .unwrap();

        let loaded = AppConfig::load(file.path()).expect("legacy config remains readable");
        loaded.save(file.path()).expect("save current config shape");

        let saved = std::fs::read_to_string(file.path()).unwrap();
        assert!(!saved.contains("chat_proposal"));
        assert!(!saved.contains("confidence_threshold"));
        assert!(!saved.contains("cooldown_seconds"));
    }

    #[test]
    fn config_load_or_default_uses_default_when_missing() {
        let path = "/tmp/nonexistent_openlife_config.yaml";
        let config = AppConfig::load_or_default(path);
        assert_eq!(config.llm.chat_model, "gpt-4o-mini");
    }

    #[test]
    fn config_effective_openai_base_ignores_ambient_redirect() {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let config = AppConfig::default();
        std::env::set_var("OPENAI_API_BASE", "https://env.override.com/v1");
        assert_eq!(config.effective_openai_base(), config.llm.openai_base);
        std::env::remove_var("OPENAI_API_BASE");
        assert_eq!(config.effective_openai_base(), config.llm.openai_base);
    }

    #[test]
    fn config_effective_openai_key_env_override() {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let mut config = AppConfig::default();
        config.llm.openai_key = "from-config".into();
        std::env::set_var("OPENAI_API_KEY", "sk-env");
        assert_eq!(config.effective_openai_key(), "from-config");
        std::env::remove_var("OPENAI_API_KEY");
        assert_eq!(config.effective_openai_key(), "from-config");
    }

    #[test]
    fn config_deepseek_env_fallback() {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let mut config = AppConfig::default();
        config.llm.provider = "deepseek".into();
        config.llm.openai_base = crate::llm::default_base_for_provider("deepseek").into();
        std::env::set_var("DEEPSEEK_API_KEY", "sk-deepseek");
        assert_eq!(config.effective_cloud_api_key(), "sk-deepseek");
        std::env::remove_var("DEEPSEEK_API_KEY");
    }

    #[test]
    fn official_environment_credential_never_follows_custom_endpoint() {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let mut config = AppConfig::default();
        config.llm.provider = "openai".into();
        config.llm.openai_base = "https://custom.example/v1".into();
        std::env::set_var("OPENAI_API_KEY", "sk-official-only");

        assert!(config.effective_cloud_api_key().is_empty());

        config.llm.openai_key = "sk-explicit-custom".into();
        assert_eq!(config.effective_cloud_api_key(), "sk-explicit-custom");
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn config_custom_provider_requires_explicit_key() {
        let _guard = crate::ENV_TEST_LOCK.lock().unwrap();
        let mut config = AppConfig::default();
        config.llm.provider = "custom".into();
        std::env::set_var("DEEPSEEK_API_KEY", "sk-deepseek");
        std::env::set_var("OPENAI_API_KEY", "sk-openai");
        assert_eq!(config.effective_cloud_api_key(), "");
        config.llm.openai_key = "sk-custom".into();
        assert_eq!(config.effective_cloud_api_key(), "sk-custom");
        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn config_load_infers_provider_for_legacy_deepseek_base() {
        let file = NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            r#"
llm:
  openai_base: "https://api.deepseek.com/v1"
  openai_key: "sk-test"
  chat_model: "deepseek-chat"
prefer_local_model: false
local_model: ""
"#,
        )
        .unwrap();

        let config = AppConfig::load(file.path()).unwrap();
        assert_eq!(config.llm.provider, "deepseek");
        assert!(!config.llm.embedding_enabled);
        assert_eq!(config.effective_provider_label(), "DeepSeek");
    }
}
