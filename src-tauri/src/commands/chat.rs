use crate::errors::AppError;
use crate::memory_gateway;
use crate::AppState;
use openlife_core::llm::ChatMessage;
use std::sync::Arc;
use tauri::State;

pub(crate) async fn get_chat_history_with_state(
    session_id: &str,
    state: &Arc<AppState>,
) -> Result<Vec<ChatMessage>, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("MemoryStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let store = state.memory_store.lock().await;
    store
        .load_recent_messages(session_id, 200)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn get_chat_history(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ChatMessage>, AppError> {
    get_chat_history_with_state(&session_id, &state.inner().clone()).await
}

pub(crate) async fn save_chat_message_with_state(
    session_id: &str,
    message: &ChatMessage,
    operation_id: &str,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    let parsed = uuid::Uuid::parse_str(operation_id)
        .map_err(|_| AppError::permission("conversation operation id must be a UUIDv4"))?;
    if parsed.get_version() != Some(uuid::Version::Random)
        || parsed.hyphenated().to_string() != operation_id
    {
        return Err(AppError::permission(
            "conversation operation id must be a canonical lowercase UUIDv4",
        ));
    }
    memory_gateway::save_conversation_message_idempotent_with_state(
        session_id,
        message,
        operation_id,
        state,
    )
    .await
    .map(|_| ())
    .map_err(AppError::internal)
}

#[tauri::command]
pub async fn save_chat_message(
    session_id: String,
    message: ChatMessage,
    operation_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    save_chat_message_with_state(&session_id, &message, &operation_id, &state.inner().clone()).await
}

pub(crate) async fn create_chat_session_with_state(
    session_id: &str,
    title: &str,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    memory_gateway::create_chat_session_with_state(session_id, title, state).await
}

#[tauri::command]
pub async fn create_chat_session(
    session_id: String,
    title: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    create_chat_session_with_state(&session_id, &title, &state.inner().clone()).await
}

#[tauri::command]
pub async fn rename_chat_session(
    session_id: String,
    title: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    memory_gateway::rename_chat_session_with_state(&session_id, &title, state.inner()).await
}

#[tauri::command]
pub async fn delete_chat_session(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    memory_gateway::delete_chat_session_with_state(&session_id, state.inner()).await
}

pub(crate) async fn list_chat_sessions_with_state(
    state: &Arc<AppState>,
) -> Result<Vec<openlife_core::memory::ChatSession>, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("MemoryStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let store = state.memory_store.lock().await;
    store.list_chat_sessions(200).map_err(AppError::from)
}

#[tauri::command]
pub async fn list_chat_sessions(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::memory::ChatSession>, AppError> {
    list_chat_sessions_with_state(&state.inner().clone()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = openlife_core::config::AppConfig::default();
        let hot_cache: openlife_core::memory_cache::SharedHotCache = Arc::new(
            tokio::sync::RwLock::new(openlife_core::memory_cache::HotMemoryCache::default()),
        );
        Arc::new(AppState {
            persistence_coordinator: Arc::new(
                crate::persistence_coordinator::PersistenceCoordinator::isolated_evaluation(),
            ),
            config: Arc::new(tokio::sync::Mutex::new(config.clone())),
            life_model_manager: Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::LifeModelManager::new(
                    temp_dir.path().join("life-model").join("current"),
                ),
            )),
            life_model_write_coordinator: Arc::new(tokio::sync::Mutex::new(())),
            memory_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::memory::MemoryStore::new_in_memory().unwrap(),
            )),
            mcp_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp::McpRegistry::new(),
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
            vector_persistence_mode: crate::state::VectorPersistenceMode::Enabled,
            builder_session_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::builder::BuilderSessionStore::new(
                    temp_dir.path().join("builder_sessions.json"),
                ),
            )),
            a2a_sidecar: Arc::new(tokio::sync::Mutex::new(
                crate::a2a_sidecar::A2ASidecar::new(crate::a2a_server::configured_a2a_port()),
            )),
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp_audit::McpAuditStore::new(temp_dir.path().join("mcp_audit.db")),
            )),
            mcp_audit_read_gateway: Arc::new(
                crate::mcp_audit_read_gateway::McpAuditReadGateway::default(),
            ),
            agent_run_store: None,
            evidence_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
            )),
            life_event_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::LifeEventStore::new_in_memory().unwrap(),
            ))),
            heuristic_store: Arc::new(tokio::sync::Mutex::new({
                let store = openlife_core::agent::HeuristicStore::new_in_memory().unwrap();
                store.seed_mvp_heuristics().unwrap();
                store
            })),
            policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
            proposal_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
            ))),
            memory_lifecycle_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap(),
            ))),
            plan_execute_session_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::PlanExecuteSessionStore::new_in_memory().unwrap(),
            ))),
            main_chat_agent_session_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStore::new_in_memory()
                    .unwrap(),
            ))),
            main_chat_action_queue_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::main_chat_agent_v1::ActionQueueStore::new_in_memory()
                    .unwrap(),
            ))),
            main_chat_agent_event_store: None,
            main_chat_selected_skill_ids: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
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
            startup_warnings: vec![],
            provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
            scheduled_task_store: Arc::new(
                openlife_core::tasks::TaskStore::new_in_memory().unwrap(),
            ),
            runtime_clock_source: Arc::new(tokio::sync::Mutex::new(
                crate::main_chat_runtime_facts::MainChatRuntimeClockSource::default(),
            )),
            web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
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
        let result = save_chat_message_with_state(
            "session-2",
            &msg,
            "a8af0116-1571-4918-93ac-79880ef1f783",
            &state,
        )
        .await;
        assert!(result.is_ok());

        // Get history
        let history = get_chat_history_with_state("session-2", &state)
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].content, "Hello world");
    }
}
