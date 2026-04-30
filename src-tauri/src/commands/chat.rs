use crate::AppState;
use openlife_core::llm::ChatMessage;
use std::sync::Arc;
use tauri::State;

pub(crate) async fn get_chat_history_with_state(
    session_id: &str,
    state: &Arc<AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let store = state.memory_store.lock().await;
    store
        .load_recent_messages(session_id, 200)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_chat_history(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ChatMessage>, String> {
    get_chat_history_with_state(&session_id, &state.inner().clone()).await
}

pub(crate) async fn save_chat_message_with_state(
    session_id: &str,
    message: &ChatMessage,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let store = state.memory_store.lock().await;
    store
        .save_message(session_id, message)
        .map_err(|e| e.to_string())?;
    store
        .touch_chat_session(session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_chat_message(
    session_id: String,
    message: ChatMessage,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    save_chat_message_with_state(&session_id, &message, &state.inner().clone()).await
}

pub(crate) async fn create_chat_session_with_state(
    session_id: &str,
    title: &str,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let store = state.memory_store.lock().await;
    store
        .create_chat_session(session_id, title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_chat_session(
    session_id: String,
    title: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    create_chat_session_with_state(&session_id, &title, &state.inner().clone()).await
}

#[tauri::command]
pub async fn rename_chat_session(
    session_id: String,
    title: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let store = state.memory_store.lock().await;
    store
        .rename_chat_session(&session_id, &title)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_chat_session(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let store = state.memory_store.lock().await;
    store
        .delete_chat_session(&session_id)
        .map_err(|e| e.to_string())
}

pub(crate) async fn list_chat_sessions_with_state(
    state: &Arc<AppState>,
) -> Result<Vec<openlife_core::memory::ChatSession>, String> {
    let store = state.memory_store.lock().await;
    store.list_chat_sessions(200).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_chat_sessions(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::memory::ChatSession>, String> {
    list_chat_sessions_with_state(&state.inner().clone()).await
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
        })
    }

    #[tokio::test]
    async fn create_and_list_chat_session() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        // Create session
        let result = create_chat_session_with_state("session-1", "Test Session", &state).await;
        assert!(result.is_ok());

        // List sessions
        let sessions = list_chat_sessions_with_state(&state).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "session-1");
        assert_eq!(sessions[0].title, "Test Session");
    }

    #[tokio::test]
    async fn save_and_get_chat_message() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        // Create session first
        create_chat_session_with_state("session-2", "Msg Test", &state)
            .await
            .unwrap();

        // Save message
        let msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello world".to_string(),
        };
        let result = save_chat_message_with_state("session-2", &msg, &state).await;
        assert!(result.is_ok());

        // Get history
        let history = get_chat_history_with_state("session-2", &state)
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].content, "Hello world");
    }
}
