use crate::errors::AppError;
use crate::main_chat_event_stream::MainChatAgentDurableEvent;
use crate::main_chat_kernel::{
    MainChatLifeModelProductReceipt, MainChatLifeModelSelectedItemReceipt,
};
use crate::AppState;
use openlife_core::life_model::v2::{
    LifeModelItemV2, LifeModelSectionV2, DEFAULT_LIFE_MODEL_V2_MODEL_ID,
};
use openlife_core::llm::ChatMessage;
use std::sync::Arc;
use tauri::State;

#[cfg(test)]
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatLifeModelInfluenceSnapshot {
    pub status: String,
    pub life_model_influence: MainChatLifeModelProductReceipt,
}

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

#[cfg(test)]
pub(crate) async fn get_chat_life_model_influence_with_state(
    session_id: &str,
    state: &Arc<AppState>,
) -> Result<Option<ChatLifeModelInfluenceSnapshot>, AppError> {
    // Canonical Chat no longer derives presentation from the legacy Work
    // AgentRun/Event owners. R1 does not yet persist the bounded LifeModel
    // influence receipt on Conversation items, so canonical conversations
    // truthfully report no durable influence snapshot here.
    if let Some(store) = state.conversation_store.as_ref() {
        state
            .persistence_coordinator
            .require_trusted_read("ConversationStore")
            .map_err(|error| {
                AppError::db_with_hint(error.to_string(), "canonical_state_unknown")
            })?;
        if store
            .lock()
            .await
            .get_conversation(session_id)
            .map_err(AppError::from)?
            .is_some()
        {
            return Ok(None);
        }
    }
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

pub(crate) async fn create_chat_session_with_state(
    session_id: &str,
    title: &str,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_allowed()
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
        .require_effects_allowed()
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
        .require_effects_allowed()
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
        .require_effects_allowed()
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
        .require_effects_allowed()
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
        assert!(state.agent_run_store.is_none());
        assert!(state.main_chat_agent_event_store.is_none());
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
