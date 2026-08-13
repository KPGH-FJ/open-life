use crate::errors::AppError;
use crate::main_chat_event_stream::MainChatAgentDurableEvent;
use crate::main_chat_kernel::{
    MainChatLifeModelProductReceipt, MainChatLifeModelSelectedItemReceipt,
};
use crate::memory_gateway;
use crate::AppState;
use openlife_core::life_model::v2::{
    LifeModelItemV2, LifeModelSectionV2, DEFAULT_LIFE_MODEL_V2_MODEL_ID,
};
use openlife_core::llm::ChatMessage;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatLifeModelInfluenceSnapshot {
    pub status: String,
    pub life_model_influence: MainChatLifeModelProductReceipt,
}

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

fn required_final_bool(event: &MainChatAgentDurableEvent, field: &str) -> Result<bool, AppError> {
    event
        .payload
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| AppError::internal(format!("life_model_influence_receipt_invalid:{field}")))
}

fn final_string_array(
    event: &MainChatAgentDurableEvent,
    field: &str,
) -> Result<Vec<String>, AppError> {
    event
        .payload
        .get(field)
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AppError::internal(format!("life_model_influence_receipt_invalid:{field}")))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
                .ok_or_else(|| {
                    AppError::internal(format!("life_model_influence_receipt_invalid:{field}"))
                })
        })
        .collect()
}

fn life_model_section_from_label(value: &str) -> Option<LifeModelSectionV2> {
    match value {
        "identity" => Some(LifeModelSectionV2::Identity),
        "values" => Some(LifeModelSectionV2::Values),
        "long_term_goals" => Some(LifeModelSectionV2::LongTermGoals),
        "stable_preferences" => Some(LifeModelSectionV2::StablePreferences),
        "personal_boundaries" => Some(LifeModelSectionV2::PersonalBoundaries),
        "important_relationships" => Some(LifeModelSectionV2::ImportantRelationships),
        "capabilities" => Some(LifeModelSectionV2::Capabilities),
        "resources" => Some(LifeModelSectionV2::Resources),
        "decision_principles" => Some(LifeModelSectionV2::DecisionPrinciples),
        "collaboration_preferences" => Some(LifeModelSectionV2::CollaborationPreferences),
        _ => None,
    }
}

fn life_model_item_statement(item: &LifeModelItemV2) -> String {
    let value = match item {
        LifeModelItemV2::Statement(item) => item.statement.clone(),
        LifeModelItemV2::LongTermGoal(item) => format!("{}: {}", item.direction, item.meaning),
        LifeModelItemV2::Relationship(item) => format!(
            "{}: {}; {}",
            item.person_label, item.relationship, item.significance
        ),
        LifeModelItemV2::Capability(item) => format!("{}: {}", item.name, item.description),
        LifeModelItemV2::Resource(item) => format!("{}: {}", item.name, item.description),
    };
    value.chars().take(320).collect()
}

fn life_model_item_provenance(item: &LifeModelItemV2) -> (&[String], &str) {
    match item {
        LifeModelItemV2::Statement(item) => (&item.source_refs, &item.confirmed_at),
        LifeModelItemV2::LongTermGoal(item) => (&item.source_refs, &item.confirmed_at),
        LifeModelItemV2::Relationship(item) => (&item.source_refs, &item.confirmed_at),
        LifeModelItemV2::Capability(item) => (&item.source_refs, &item.confirmed_at),
        LifeModelItemV2::Resource(item) => (&item.source_refs, &item.confirmed_at),
    }
}

pub(crate) async fn life_model_influence_for_final_event_with_state(
    event: &MainChatAgentDurableEvent,
    state: &Arc<AppState>,
) -> Result<Option<MainChatLifeModelProductReceipt>, AppError> {
    let Some(status) = event
        .payload
        .get("lifeModelInfluenceStatus")
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(None);
    };
    if !matches!(
        status,
        "eligible_for_context"
            | "not_task_relevant"
            | "canonical_model_unavailable"
            | "current_instruction_override"
            | "applied_memory_rerank"
            | "applied_equivalent_tool_preference"
            | "applied_context_building"
            | "applied_context_and_memory_rerank"
            | "applied_memory_rerank_without_direct_context"
            | "eligible_not_selected_by_context_budget"
            | "applied_planning"
    ) {
        return Err(AppError::internal(
            "life_model_influence_receipt_invalid:status",
        ));
    }
    let current_instruction_priority_preserved =
        required_final_bool(event, "lifeModelCurrentInstructionPriorityPreserved")?;
    let policy_priority_preserved = required_final_bool(event, "lifeModelPolicyPriorityPreserved")?;
    let permission_granted = required_final_bool(event, "lifeModelPermissionGranted")?;
    let durable_write_authorized = required_final_bool(event, "lifeModelDurableWriteAuthorized")?;
    if !current_instruction_priority_preserved
        || !policy_priority_preserved
        || permission_granted
        || durable_write_authorized
    {
        return Err(AppError::internal(
            "life_model_influence_receipt_security_boundary_invalid",
        ));
    }
    let selected_item_refs = final_string_array(event, "lifeModelSelectedItemRefs")?;
    let selection_reason_codes = final_string_array(event, "lifeModelSelectionReasonCodes")?;
    let applied_surfaces = final_string_array(event, "lifeModelAppliedSurfaces")?;
    if selected_item_refs.len() != selection_reason_codes.len() || selected_item_refs.len() > 4 {
        return Err(AppError::internal(
            "life_model_influence_receipt_selection_binding_invalid",
        ));
    }
    if selected_item_refs
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != selected_item_refs.len()
        || applied_surfaces
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != applied_surfaces.len()
    {
        return Err(AppError::internal(
            "life_model_influence_receipt_duplicate_binding",
        ));
    }
    if !applied_surfaces.iter().all(|surface| {
        matches!(
            surface.as_str(),
            "context_building"
                | "communication_style"
                | "memory_retrieval_rerank"
                | "equivalent_tool_ranking"
                | "planning"
        )
    }) {
        return Err(AppError::internal(
            "life_model_influence_receipt_surface_invalid",
        ));
    }

    let source_id = event
        .payload
        .get("lifeModelSourceId")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    let model_version = event
        .payload
        .get("lifeModelVersion")
        .and_then(serde_json::Value::as_u64);
    let version_digest = event
        .payload
        .get("lifeModelVersionDigest")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    let document_digest = event
        .payload
        .get("lifeModelDocumentDigest")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    if status == "current_instruction_override"
        && (!selected_item_refs.is_empty()
            || !applied_surfaces.is_empty()
            || source_id.is_some()
            || model_version.is_some()
            || version_digest.is_some()
            || document_digest.is_some())
    {
        return Err(AppError::internal(
            "life_model_influence_override_binding_invalid",
        ));
    }
    if status.starts_with("applied_") && applied_surfaces.is_empty() {
        return Err(AppError::internal(
            "life_model_influence_applied_surface_missing",
        ));
    }
    let selected_items = if selected_item_refs.is_empty() {
        Vec::new()
    } else {
        if source_id.as_deref() != Some("lifemodel.v2.runtime") {
            return Err(AppError::internal(
                "life_model_influence_receipt_source_invalid",
            ));
        }
        let version_number = model_version.ok_or_else(|| {
            AppError::internal("life_model_influence_receipt_model_version_missing")
        })?;
        let version = state
            .life_model_manager
            .lock()
            .await
            .load_v2_version(DEFAULT_LIFE_MODEL_V2_MODEL_ID, version_number)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::internal("life_model_influence_version_unavailable"))?;
        if version_digest.as_deref() != Some(version.version_digest.as_str())
            || document_digest.as_deref() != Some(version.document_digest.as_str())
        {
            return Err(AppError::internal(
                "life_model_influence_version_binding_invalid",
            ));
        }
        selected_item_refs
            .iter()
            .zip(selection_reason_codes.iter())
            .map(|(item_ref, reason_code)| {
                let (section_label, item_id) = item_ref
                    .split_once(':')
                    .ok_or_else(|| AppError::internal("life_model_influence_item_ref_invalid"))?;
                let section = life_model_section_from_label(section_label)
                    .ok_or_else(|| AppError::internal("life_model_influence_item_ref_invalid"))?;
                let item = version
                    .document
                    .item(section, item_id)
                    .ok_or_else(|| AppError::internal("life_model_influence_item_unavailable"))?;
                let (source_refs, confirmed_at) = life_model_item_provenance(&item);
                Ok(MainChatLifeModelSelectedItemReceipt {
                    item_ref: item_ref.clone(),
                    statement: life_model_item_statement(&item),
                    source_refs: source_refs.to_vec(),
                    confirmed_at: confirmed_at.to_string(),
                    reason_code: reason_code.clone(),
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?
    };

    Ok(Some(MainChatLifeModelProductReceipt {
        status: status.to_string(),
        source_id,
        model_version,
        version_digest,
        document_digest,
        selected_items,
        applied_surfaces,
        current_instruction_priority_preserved,
        policy_priority_preserved,
        permission_granted,
        durable_write_authorized,
    }))
}

pub(crate) async fn get_chat_life_model_influence_with_state(
    session_id: &str,
    state: &Arc<AppState>,
) -> Result<Option<ChatLifeModelInfluenceSnapshot>, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("AgentRunStore")
        .and_then(|_| {
            state
                .persistence_coordinator
                .require_trusted_read("MainChatAgentEventStore")
        })
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let latest_run = {
        let store = state
            .agent_run_store
            .as_ref()
            .ok_or_else(|| AppError::internal("agent_run_store_unavailable"))?
            .lock()
            .await;
        crate::terminal_owner_write_gateway::register_agent_run_store_result(
            state,
            store
                .list_runs_for_session(session_id, 1)
                .map_err(|error| error.to_string()),
        )
        .map_err(AppError::internal)?
        .into_iter()
        .next()
    };
    let Some(run) = latest_run else {
        return Ok(None);
    };
    if run.session_id.as_deref() != Some(session_id) || run.task_id.trim().is_empty() {
        return Err(AppError::internal(
            "life_model_influence_run_identity_invalid",
        ));
    }
    let final_event = {
        let store = state
            .main_chat_agent_event_store
            .as_ref()
            .ok_or_else(|| AppError::internal("main_chat_agent_event_store_unavailable"))?
            .lock()
            .await;
        store
            .terminal_owner_final_event(&run.task_id)
            .map_err(AppError::from)?
    };
    let Some(event) = final_event else {
        return Ok(None);
    };
    if event.task_session_id != run.task_id
        || event.run_id != run.id
        || event.event_type != "final_delivery.created"
        || event.object_type != "final_delivery"
    {
        return Err(AppError::internal(
            "life_model_influence_final_identity_invalid",
        ));
    }
    let Some(life_model_influence) =
        life_model_influence_for_final_event_with_state(&event, state).await?
    else {
        return Ok(None);
    };
    let status = event
        .payload
        .get("status")
        .and_then(serde_json::Value::as_str)
        .filter(|status| {
            matches!(
                *status,
                "completed"
                    | "completed_with_pending_items"
                    | "blocked"
                    | "failed"
                    | "cancelled"
                    | "interrupted"
            )
        })
        .ok_or_else(|| AppError::internal("life_model_influence_final_status_invalid"))?;
    Ok(Some(ChatLifeModelInfluenceSnapshot {
        status: status.to_string(),
        life_model_influence,
    }))
}

#[tauri::command]
pub async fn get_chat_life_model_influence(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<ChatLifeModelInfluenceSnapshot>, AppError> {
    get_chat_life_model_influence_with_state(&session_id, state.inner()).await
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
            a2a_sidecar: Arc::new(tokio::sync::Mutex::new(
                crate::a2a_sidecar::A2ASidecar::new(crate::a2a_server::configured_a2a_port()),
            )),
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp_audit::McpAuditStore::new(temp_dir.path().join("mcp_audit.db")),
            )),
            agent_run_store: None,
            canonical_task_runtime_store: None,
            evidence_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
            )),
            life_event_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::LifeEventStore::new_in_memory().unwrap(),
            ))),
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
            main_chat_agent_session_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStore::new_in_memory()
                    .unwrap(),
            ))),
            main_chat_action_queue_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::main_chat_agent_v1::ActionQueueStore::new_in_memory()
                    .unwrap(),
            ))),
            main_chat_agent_event_store: None,
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
            credential_bootstrap_snapshot: Default::default(),
            provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
            scheduled_task_store: Arc::new(
                openlife_core::tasks::TaskStore::new_in_memory().unwrap(),
            ),
            runtime_clock_source: Arc::new(tokio::sync::Mutex::new(
                crate::main_chat_runtime_facts::MainChatRuntimeClockSource::default(),
            )),
            web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            resource_runtime: None,
            state_store: None,
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

    fn influence_event(payload: serde_json::Value) -> MainChatAgentDurableEvent {
        MainChatAgentDurableEvent {
            event_id: "event-1".into(),
            task_session_id: "task-1".into(),
            run_id: "run-1".into(),
            sequence: 1,
            event_type: "final_delivery.created".into(),
            object_type: "final_delivery".into(),
            object_id: "delivery-1".into(),
            created_at: chrono::Utc::now(),
            source: "openlife_turn_runtime.final_delivery_owner".into(),
            payload_digest: "sha256:test".into(),
            payload,
            backfilled: false,
        }
    }

    #[tokio::test]
    async fn restored_lifemodel_override_receipt_preserves_the_security_boundary() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let event = influence_event(serde_json::json!({
            "lifeModelInfluenceStatus": "current_instruction_override",
            "lifeModelSourceId": null,
            "lifeModelSelectedItemRefs": [],
            "lifeModelSelectionReasonCodes": [],
            "lifeModelAppliedSurfaces": [],
            "lifeModelCurrentInstructionPriorityPreserved": true,
            "lifeModelPolicyPriorityPreserved": true,
            "lifeModelPermissionGranted": false,
            "lifeModelDurableWriteAuthorized": false,
        }));

        let receipt = life_model_influence_for_final_event_with_state(&event, &state)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(receipt.status, "current_instruction_override");
        assert!(receipt.selected_items.is_empty());
        assert!(!receipt.permission_granted);
        assert!(!receipt.durable_write_authorized);
    }

    #[tokio::test]
    async fn restored_lifemodel_receipt_rejects_fabricated_permission() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let event = influence_event(serde_json::json!({
            "lifeModelInfluenceStatus": "current_instruction_override",
            "lifeModelSourceId": null,
            "lifeModelSelectedItemRefs": [],
            "lifeModelSelectionReasonCodes": [],
            "lifeModelAppliedSurfaces": [],
            "lifeModelCurrentInstructionPriorityPreserved": true,
            "lifeModelPolicyPriorityPreserved": true,
            "lifeModelPermissionGranted": true,
            "lifeModelDurableWriteAuthorized": false,
        }));

        let error = life_model_influence_for_final_event_with_state(&event, &state)
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("life_model_influence_receipt_security_boundary_invalid"));
    }
}
