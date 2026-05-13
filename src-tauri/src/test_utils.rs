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
        agent_run_event_store: Some(Arc::new(
            openlife_core::agent::event_store::AgentRunEventStore::new_in_memory().unwrap(),
        )),
        plan_store: Some(Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::PlanStore::new_in_memory().unwrap(),
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
        agent_spec_store: Arc::new(tokio::sync::Mutex::new(
            openlife_core::agent::AgentSpecStore::new_in_memory().unwrap(),
        )),
        startup_warnings: vec![],
        provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
        scheduled_task_mutex: Arc::new(tokio::sync::Mutex::new(())),
        shutdown_notify: Arc::new(tokio::sync::Notify::new()),
    })
}

#[cfg(test)]
#[tokio::test]
async fn test_event_store_accessible_through_app_state() {
    let state = test_app_state();
    let es = state
        .agent_run_event_store
        .as_ref()
        .expect("event_store should be set");

    let run_id = "app-state-test-run";
    let event = openlife_core::agent::AgentRunEvent::new(
        run_id,
        openlife_core::agent::AgentRunEventType::RunCreated,
        openlife_core::agent::AgentEventActor::Runtime,
        "test event through AppState",
        serde_json::json!({}),
    );

    es.append_event(&event).unwrap();

    let events = es.list_events_by_run(run_id).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].event_type,
        openlife_core::agent::AgentRunEventType::RunCreated
    );
    assert_eq!(events[0].summary, "test event through AppState");
}

#[cfg(test)]
#[tokio::test]
async fn test_default_agent_spec_available_after_bootstrap() {
    let state = test_app_state();
    let store = state.agent_spec_store.lock().await;
    let spec = store.get_default_spec().unwrap();
    assert!(
        spec.is_some(),
        "default AgentSpec should exist after bootstrap"
    );
    let spec = spec.unwrap();
    assert_eq!(spec.id, "main.default");
    assert_eq!(spec.role, openlife_core::agent::AgentRoleKind::Main);
    assert!(spec.active);
}

#[cfg(test)]
#[tokio::test]
async fn test_list_returns_default_main_spec() {
    let state = test_app_state();
    let store = state.agent_spec_store.lock().await;
    let specs = store.list_specs().unwrap();
    assert!(
        !specs.is_empty(),
        "list should return at least the default spec"
    );
    assert!(
        specs.iter().any(|s| s.id == "main.default"),
        "list should include main.default"
    );
}
