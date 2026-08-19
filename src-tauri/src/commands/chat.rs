use crate::errors::AppError;
use crate::AppState;
use openlife_core::llm::ChatMessage;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurnViewModel {
    pub turn_id: String,
    pub status: openlife_core::conversation::TurnStatus,
    pub provider_profile_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub endpoint_class: String,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationViewModel {
    pub status: String,
    pub conversations: Vec<openlife_core::memory::ChatSession>,
    pub projects: Vec<openlife_core::conversation::ProjectRecord>,
    pub selected_project_id: Option<String>,
    pub selected_conversation_id: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub latest_turn: Option<ConversationTurnViewModel>,
    pub provider_status: String,
    pub provider_profiles: Vec<crate::provider_registry::ProviderProfileViewModel>,
    pub selected_provider_profile_id: Option<String>,
    pub provider_error_code: Option<String>,
    /// R1 intentionally keeps the retired Work runtime out of the product UI.
    /// R2 changes this only after Task/Run/ItemAttempt is canonical end to end.
    pub work_status: String,
}

pub(crate) async fn get_conversation_view_model_with_state(
    requested_conversation_id: Option<&str>,
    state: &Arc<AppState>,
) -> Result<ConversationViewModel, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("ConversationStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?
        .lock()
        .await;
    let records = store
        .list_conversations(false, 200)
        .map_err(AppError::from)?;
    let selected = match requested_conversation_id {
        Some(requested) => {
            if records.iter().any(|record| record.id == requested) {
                Some(requested.to_string())
            } else {
                return Err(AppError::not_found("conversation_not_found"));
            }
        }
        None => records.first().map(|record| record.id.clone()),
    };
    let conversations = records
        .into_iter()
        .map(|record| openlife_core::memory::ChatSession {
            session_id: record.id,
            title: record.title,
            created_at: record.created_at.to_rfc3339(),
            updated_at: record.updated_at.to_rfc3339(),
        })
        .collect::<Vec<_>>();
    let projects = store.list_projects(200).map_err(AppError::from)?;
    let selected_project_id = selected
        .as_deref()
        .map(|conversation_id| store.get_conversation(conversation_id))
        .transpose()
        .map_err(AppError::from)?
        .flatten()
        .and_then(|conversation| conversation.project_id);
    let (messages, latest_turn) = if let Some(conversation_id) = selected.as_deref() {
        let messages = store
            .list_items(conversation_id, 200)
            .map_err(AppError::from)?
            .into_iter()
            .filter_map(|item| match item.kind {
                openlife_core::conversation::ConversationItemKind::UserMessage => {
                    Some(ChatMessage {
                        role: "user".into(),
                        content: item.content,
                    })
                }
                openlife_core::conversation::ConversationItemKind::AssistantMessage => {
                    Some(ChatMessage {
                        role: "assistant".into(),
                        content: item.content,
                    })
                }
                openlife_core::conversation::ConversationItemKind::UserSteering
                | openlife_core::conversation::ConversationItemKind::SystemNotice => None,
            })
            .collect();
        let latest_turn = store
            .latest_turn(conversation_id)
            .map_err(AppError::from)?
            .map(|turn| ConversationTurnViewModel {
                turn_id: turn.id,
                status: turn.status,
                provider_profile_id: turn.provider.profile_id,
                provider_id: turn.provider.provider_id,
                model_id: turn.provider.model_id,
                endpoint_class: turn.provider.endpoint_class,
                error_code: turn.error_code,
            });
        (messages, latest_turn)
    } else {
        (Vec::new(), None)
    };
    drop(store);
    let (provider_status, provider_profiles, selected_provider_profile_id, provider_error_code) =
        match crate::provider_registry::selected_provider_profile(state).await {
            Ok(provider) => (
                "ready".to_string(),
                provider.profiles,
                Some(provider.binding.profile_id),
                None,
            ),
            Err(error) => ("unavailable".to_string(), Vec::new(), None, Some(error)),
        };
    Ok(ConversationViewModel {
        status: if conversations.is_empty() {
            "empty"
        } else {
            "ready"
        }
        .into(),
        conversations,
        projects,
        selected_project_id,
        selected_conversation_id: selected,
        messages,
        latest_turn,
        provider_status,
        provider_profiles,
        selected_provider_profile_id,
        provider_error_code,
        work_status: if state.canonical_task_runtime_store.is_some() {
            "ready".into()
        } else {
            "reconstructing".into()
        },
    })
}

#[tauri::command]
pub async fn get_conversation_view_model(
    conversation_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<ConversationViewModel, AppError> {
    get_conversation_view_model_with_state(conversation_id.as_deref(), state.inner()).await
}

pub(crate) async fn create_chat_session_with_state(
    session_id: &str,
    title: &str,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?
        .lock()
        .await
        .create_conversation(session_id, title)
        .map(|_| ())
        .map_err(AppError::from)
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
pub async fn create_project(
    project_id: String,
    name: String,
    workspace_root: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::conversation::ProjectRecord, AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?
        .lock()
        .await
        .create_project(&project_id, &name, workspace_root.as_deref())
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn assign_conversation_project(
    conversation_id: String,
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?
        .lock()
        .await
        .assign_conversation_project(&conversation_id, project_id.as_deref())
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn rename_chat_session(
    session_id: String,
    title: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?
        .lock()
        .await
        .rename_conversation(&session_id, &title)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn delete_chat_session(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?
        .lock()
        .await
        .delete_conversation(&session_id)
        .map_err(AppError::from)
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
            governed_data_import_journal: None,
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
            conversation_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::conversation::ConversationStore::new_in_memory().unwrap(),
            ))),
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
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp_audit::McpAuditStore::new(temp_dir.path().join("mcp_audit.db")),
            )),
            canonical_task_runtime_store: None,
            evidence_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
            )),
            policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
            proposal_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
            ))),
            memory_lifecycle_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap(),
            ))),
            life_model_learning_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::LifeModelLearningStore::new_in_memory().unwrap(),
            ))),
            main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
            patch_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::patch_store::PatchStore::new_in_memory().unwrap(),
            ))),
            tool_permission_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            )),
            skill_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::skills::SkillRegistry::built_in(),
            )),
            hot_cache,
            startup_warnings: vec![],
            credential_bootstrap_snapshot: Default::default(),
            scheduled_task_store: Arc::new(
                openlife_core::tasks::TaskStore::new_in_memory().unwrap(),
            ),
            web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            resource_runtime: None,
            state_store: None,
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
        })
    }

    #[tokio::test]
    async fn create_chat_session_is_visible_through_the_conversation_view_model() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        // Create session
        let session_id = uuid::Uuid::new_v4().to_string();
        let result = create_chat_session_with_state(&session_id, "Test Session", &state).await;
        assert!(result.is_ok());

        let view = get_conversation_view_model_with_state(Some(&session_id), &state)
            .await
            .unwrap();
        assert_eq!(view.conversations.len(), 1);
        assert_eq!(view.conversations[0].session_id, session_id);
        assert_eq!(view.conversations[0].title, "Test Session");
    }

    #[tokio::test]
    async fn canonical_conversation_creation_ignores_unrelated_retired_store_health() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        let persistence = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        persistence.register_read_write("ConversationStore");
        persistence.register_read_write("CanonicalTaskRuntimeStore");
        persistence.seal();
        assert!(persistence.require_effects_allowed().is_err());
        Arc::get_mut(&mut state).unwrap().persistence_coordinator = persistence;

        let session_id = uuid::Uuid::new_v4().to_string();
        create_chat_session_with_state(&session_id, "Scoped Session", &state)
            .await
            .expect("an unrelated retired store must not disable canonical Conversation writes");

        let view = get_conversation_view_model_with_state(Some(&session_id), &state)
            .await
            .unwrap();
        assert_eq!(
            view.selected_conversation_id.as_deref(),
            Some(session_id.as_str())
        );
    }

    #[tokio::test]
    async fn project_creation_and_assignment_are_visible_through_the_same_view_model() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let project_id = uuid::Uuid::new_v4().to_string();
        create_chat_session_with_state(&conversation_id, "Scoped Session", &state)
            .await
            .unwrap();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_project(&project_id, "Research Project", None)
            .unwrap();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .assign_conversation_project(&conversation_id, Some(&project_id))
            .unwrap();

        let view = get_conversation_view_model_with_state(Some(&conversation_id), &state)
            .await
            .unwrap();
        assert_eq!(view.projects.len(), 1);
        assert_eq!(view.projects[0].name, "Research Project");
        assert_eq!(
            view.selected_project_id.as_deref(),
            Some(project_id.as_str())
        );
    }

    #[tokio::test]
    async fn conversation_view_model_is_canonical_and_provider_bound() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        create_chat_session_with_state(&conversation_id, "Canonical", &state)
            .await
            .unwrap();
        let provider = openlife_core::conversation::ProviderBinding {
            profile_id: "provider-profile:test".into(),
            provider_id: "openai".into(),
            model_id: "gpt-test".into(),
            endpoint_class: "cloud".into(),
            config_generation: "test-generation".into(),
        };
        let store = state.conversation_store.as_ref().unwrap().lock().await;
        store
            .begin_chat_turn(openlife_core::conversation::BeginChatTurn {
                turn_id: &turn_id,
                conversation_id: &conversation_id,
                user_message: "你好",
                provider: &provider,
            })
            .unwrap();
        store.complete_chat_turn(&turn_id, "你好，我在。").unwrap();
        drop(store);

        let view = get_conversation_view_model_with_state(Some(&conversation_id), &state)
            .await
            .unwrap();
        assert_eq!(view.status, "ready");
        assert_eq!(
            view.selected_conversation_id.as_deref(),
            Some(conversation_id.as_str())
        );
        assert_eq!(view.messages.len(), 2);
        assert_eq!(view.latest_turn.unwrap().turn_id, turn_id);
        assert_eq!(view.provider_status, "unavailable");
        assert!(view.provider_profiles.is_empty());
        assert!(view.selected_provider_profile_id.is_none());
        assert_eq!(
            view.provider_error_code.as_deref(),
            Some("configured_provider_unavailable")
        );
    }

    #[tokio::test]
    async fn conversation_view_model_rejects_an_unknown_requested_identity() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let existing_id = uuid::Uuid::new_v4().to_string();
        create_chat_session_with_state(&existing_id, "Existing", &state)
            .await
            .unwrap();

        let error =
            get_conversation_view_model_with_state(Some(&uuid::Uuid::new_v4().to_string()), &state)
                .await
                .unwrap_err();
        assert!(error.to_string().contains("conversation_not_found"));
    }
}
