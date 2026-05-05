//! Shared test utilities for Tauri command integration tests.
//! Provides a minimal AppState builder backed by in-memory stores,
//! avoiding filesystem dependencies for isolated unit tests.
//!
//! Note: Individual test modules define their own `test_app_state(temp_dir)`
//! factory. The zero-arg version below is kept for potential shared use.

use crate::AppState;
use openlife_core::config::AppConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_app_state() -> Arc<AppState> {
    let config = AppConfig::default();
    let base = std::env::temp_dir().join("test-openlife");
    Arc::new(AppState {
        config: Arc::new(Mutex::new(config.clone())),
        life_model_manager: Arc::new(Mutex::new(
            openlife_core::life_model::LifeModelManager::new(
                base.join("life-model").join("current"),
            ),
        )),
        memory_store: Arc::new(Mutex::new(
            openlife_core::memory::MemoryStore::new_in_memory().unwrap(),
        )),
        mcp_registry: Arc::new(Mutex::new(openlife_core::mcp::McpRegistry::new())),
        intent_router: Arc::new(Mutex::new(openlife_core::router::IntentRouter::new())),
        layer_router: Arc::new(Mutex::new(openlife_core::layer_router::LayerRouter::new())),
        scheduler: Arc::new(Mutex::new(
            openlife_core::scheduler::InferenceScheduler::new(
                config.local_model.clone(),
                config.prefer_local_model,
                config.llm.provider.clone(),
                config.llm.openai_base.clone(),
                config.llm.openai_key.clone(),
                config.llm.chat_model.clone(),
                config.llm.embedding_model.clone(),
                config.llm.embedding_enabled,
            ),
        )),
        privacy_engine: Arc::new(Mutex::new(openlife_core::privacy::PrivacyEngine::new())),
        version_manager: Arc::new(Mutex::new(openlife_core::versioning::VersionManager::new(
            base.join("life-model").join("versions"),
        ))),
        feedback_store: Arc::new(Mutex::new(
            openlife_core::feedback::FeedbackStore::new_in_memory().unwrap(),
        )),
        vector_store: Arc::new(Mutex::new(
            openlife_core::vectors::VectorStore::new_in_memory().unwrap(),
        )),
        builder_sessions: Arc::new(Mutex::new(HashMap::new())),
        builder_session_store: Arc::new(Mutex::new(
            openlife_core::builder::BuilderSessionStore::new(base.join("builder_sessions.json")),
        )),
        a2a_sidecar: Arc::new(Mutex::new(crate::a2a_sidecar::A2ASidecar::new(8765))),
        last_snapshot_date: Arc::new(Mutex::new(None)),
        mcp_audit_store: Arc::new(Mutex::new(openlife_core::mcp_audit::McpAuditStore::new(
            base.join("mcp_audit.db"),
        ))),
        agent_run_store: Some(Arc::new(Mutex::new(
            openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
        ))),
        proposal_store: Some(Arc::new(Mutex::new(
            openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
        ))),
        patch_store: Some(Arc::new(Mutex::new(
            openlife_core::life_model::patch_store::PatchStore::new_in_memory().unwrap(),
        ))),
        rollout_metrics_store: None,
        tool_permission_store: Arc::new(Mutex::new(
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
        )),
        skill_registry: Arc::new(Mutex::new(openlife_core::skills::SkillRegistry::built_in())),
        plugin_registry: Arc::new(Mutex::new(openlife_core::plugins::PluginRegistry::new(
            base.join("plugins"),
        ))),
        hot_cache: Arc::new(tokio::sync::RwLock::new(
            openlife_core::memory_cache::HotMemoryCache::default(),
        )),
        proposal_engine: Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::ProposalEngine::new(),
        )),
        startup_warnings: vec![],
        provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
        scheduled_task_mutex: Arc::new(tokio::sync::Mutex::new(())),
        shutdown_notify: Arc::new(tokio::sync::Notify::new()),
    })
}
