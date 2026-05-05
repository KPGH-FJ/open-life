use crate::errors::AppError;
use crate::{persist_life_model, AppState};
use openlife_core::life_model::LifeModel;
use std::sync::Arc;
use tauri::State;

pub(crate) async fn get_life_model_with_state(state: &Arc<AppState>) -> Result<LifeModel, AppError> {
    let manager = state.life_model_manager.lock().await;
    manager.load().map_err(AppError::from)
}

#[tauri::command]
pub async fn get_life_model(state: State<'_, Arc<AppState>>) -> Result<LifeModel, AppError> {
    get_life_model_with_state(&state.inner().clone()).await
}

pub(crate) async fn save_life_model_with_state(
    life_model: LifeModel,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    persist_life_model(&state.clone(), life_model, true)
        .await
        .map_err(AppError::from)
        .map(|_| ())
}

#[tauri::command]
pub async fn save_life_model(
    life_model: LifeModel,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    save_life_model_with_state(life_model, &state.inner().clone()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = openlife_core::config::AppConfig::default();
        let hot_cache: openlife_core::memory_cache::SharedHotCache = Arc::new(
            tokio::sync::RwLock::new(openlife_core::memory_cache::HotMemoryCache::default()),
        );
        Arc::new(AppState {
            config: Arc::new(tokio::sync::Mutex::new(config.clone())),
            life_model_manager: Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::LifeModelManager::new(
                    temp_dir.path().join("life-model").join("current"),
                ),
            )),
            memory_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::memory::MemoryStore::new_in_memory().unwrap(),
            )),
            mcp_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp::McpRegistry::new(),
            )),
            intent_router: Arc::new(tokio::sync::Mutex::new(
                openlife_core::router::IntentRouter::new(),
            )),
            layer_router: Arc::new(tokio::sync::Mutex::new(
                openlife_core::layer_router::LayerRouter::new(),
            )),
            scheduler: Arc::new(tokio::sync::Mutex::new(
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
            privacy_engine: Arc::new(tokio::sync::Mutex::new(
                openlife_core::privacy::PrivacyEngine::new(),
            )),
            version_manager: Arc::new(tokio::sync::Mutex::new(
                openlife_core::versioning::VersionManager::new(
                    temp_dir.path().join("life-model").join("versions"),
                ),
            )),
            feedback_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::feedback::FeedbackStore::new_in_memory().unwrap(),
            )),
            vector_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::vectors::VectorStore::new_in_memory().unwrap(),
            )),
            builder_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            builder_session_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::builder::BuilderSessionStore::new(
                    temp_dir.path().join("builder_sessions.json"),
                ),
            )),
            a2a_sidecar: Arc::new(tokio::sync::Mutex::new(
                crate::a2a_sidecar::A2ASidecar::new(8765),
            )),
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp_audit::McpAuditStore::new(temp_dir.path().join("mcp_audit.db")),
            )),
            agent_run_store: None,
            proposal_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
            ))),
            patch_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::patch_store::PatchStore::new_in_memory().unwrap(),
            ))),
            rollout_metrics_store: None,
            tool_permission_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            )),
            skill_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::skills::SkillRegistry::built_in(),
            )),
            plugin_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::plugins::PluginRegistry::new(temp_dir.path().join("plugins")),
            )),
            hot_cache,
            proposal_engine: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalEngine::new(),
            )),
            startup_warnings: vec![],
            provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
            scheduled_task_mutex: Arc::new(tokio::sync::Mutex::new(())),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
        })
    }

    #[tokio::test]
    async fn get_life_model_returns_default_when_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let result = get_life_model_with_state(&state).await;
        assert!(result.is_ok());
        let model = result.unwrap();
        assert!(model.is_effectively_empty());
    }

    #[tokio::test]
    async fn save_and_get_life_model_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        // Create a life model with some data
        let mut model = LifeModel::default();
        model.identity.name = "TestUser".to_string();
        model
            .identity
            .values
            .push(openlife_core::life_model::ValueItem {
                name: "Honesty".to_string(),
                weight: 9,
                description: "Being truthful".to_string(),
            });

        // Save
        let save_result = save_life_model_with_state(model.clone(), &state).await;
        assert!(save_result.is_ok());

        // Get back
        let result = get_life_model_with_state(&state).await;
        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.identity.name, "TestUser");
        assert_eq!(retrieved.identity.values.len(), 1);
        assert_eq!(retrieved.identity.values[0].name, "Honesty");
    }
}
