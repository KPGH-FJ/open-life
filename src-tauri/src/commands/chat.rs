use crate::danger_action_confirmation::{
    require_native_danger_action_confirmation, NativeDangerActionRequest,
};
use crate::errors::AppError;
use crate::AppState;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{Runtime, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurnViewModel {
    pub turn_id: String,
    pub status: openlife_core::conversation::TurnStatus,
    pub provider_profile_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub endpoint_class: String,
    pub reasoning_effort: Option<openlife_core::conversation::ReasoningEffort>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConversationListItemViewModel {
    pub session_id: String,
    pub title: String,
    pub status: openlife_core::conversation::ConversationStatus,
    #[serde(rename = "turnCount")]
    pub turn_count: u64,
    #[serde(rename = "itemCount")]
    pub item_count: u64,
    #[serde(rename = "taskReferenceCount")]
    pub task_reference_count: Option<u64>,
    #[serde(rename = "activeTaskCount")]
    pub active_task_count: Option<u64>,
    #[serde(rename = "allowedControls")]
    pub allowed_controls: Vec<String>,
    #[serde(rename = "blockerCodes")]
    pub blocker_codes: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationAttachmentViewModel {
    pub resource_id: String,
    pub filename: String,
    pub detected_mime: String,
    pub format: openlife_core::resource::ResourceFormat,
    pub digest: String,
    pub byte_count: u64,
    pub chunk_count: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessageViewModel {
    pub turn_id: String,
    pub role: String,
    pub content: String,
    /// `ready` means the attachment list is authoritative, including when it
    /// is empty. `unavailable` never masquerades an unreadable owner as none.
    pub attachments_status: String,
    pub attachments: Vec<ConversationAttachmentViewModel>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLifecycleViewModel {
    #[serde(flatten)]
    pub project: openlife_core::conversation::ProjectRecord,
    pub active_conversation_count: u64,
    pub total_conversation_count: u64,
    pub task_run_reference_count: Option<u64>,
    pub selected_for_new_conversation: bool,
    pub allowed_controls: Vec<String>,
    pub blocker_codes: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationViewModel {
    pub status: String,
    pub conversations: Vec<ConversationListItemViewModel>,
    pub archived_conversations: Vec<ConversationListItemViewModel>,
    pub projects: Vec<ProjectLifecycleViewModel>,
    pub selected_project_id: Option<String>,
    pub selected_conversation_id: Option<String>,
    pub global_memory_enabled: bool,
    pub selected_memory_mode: openlife_core::conversation::ConversationMemoryMode,
    pub messages: Vec<ConversationMessageViewModel>,
    pub latest_turn: Option<ConversationTurnViewModel>,
    pub provider_status: String,
    pub provider_profiles: Vec<crate::provider_registry::ProviderProfileViewModel>,
    pub selected_provider_profile_id: Option<String>,
    pub provider_error_code: Option<String>,
    /// Product availability of the canonical Work runtime.
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
    let all_records = store
        .list_conversations(true, 400)
        .map_err(AppError::from)?;
    let records = all_records
        .iter()
        .filter(|record| record.status == openlife_core::conversation::ConversationStatus::Active)
        .cloned()
        .collect::<Vec<_>>();
    let selected = match requested_conversation_id {
        Some(requested) => {
            if all_records.iter().any(|record| record.id == requested) {
                Some(requested.to_string())
            } else {
                return Err(AppError::not_found("conversation_not_found"));
            }
        }
        None => records.first().map(|record| record.id.clone()),
    };
    let conversation_history_counts = all_records
        .iter()
        .map(|record| {
            store
                .conversation_history_counts(&record.id)
                .map(|counts| (record.id.clone(), counts))
        })
        .collect::<Result<std::collections::HashMap<_, _>, _>>()
        .map_err(AppError::from)?;
    let mut project_records = store.list_projects(200).map_err(AppError::from)?;
    project_records.extend(store.list_archived_projects(200).map_err(AppError::from)?);
    let project_facts = project_records
        .iter()
        .map(|project| store.project_lifecycle_facts(&project.id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from)?;
    let selected_record = selected
        .as_deref()
        .map(|conversation_id| store.get_conversation(conversation_id))
        .transpose()
        .map_err(AppError::from)?
        .flatten();
    let selected_project_id = match selected_record.as_ref() {
        Some(conversation) => conversation.project_id.clone(),
        None => store
            .new_conversation_project_id()
            .map_err(AppError::from)?,
    };
    let selected_memory_mode = selected_record
        .as_ref()
        .map(|conversation| conversation.memory_mode)
        .unwrap_or_default();
    let (messages, latest_turn) = if let Some(conversation_id) = selected.as_deref() {
        let messages = store
            .list_items(conversation_id, 200)
            .map_err(AppError::from)?
            .into_iter()
            .filter_map(|item| match item.kind {
                openlife_core::conversation::ConversationItemKind::UserMessage => {
                    Some(ConversationMessageViewModel {
                        turn_id: item.turn_id,
                        role: "user".into(),
                        content: item.content,
                        attachments_status: "unavailable".into(),
                        attachments: Vec::new(),
                    })
                }
                openlife_core::conversation::ConversationItemKind::AssistantMessage => {
                    Some(ConversationMessageViewModel {
                        turn_id: item.turn_id,
                        role: "assistant".into(),
                        content: item.content,
                        attachments_status: "not_applicable".into(),
                        attachments: Vec::new(),
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
                reasoning_effort: turn.provider.reasoning_effort,
                error_code: turn.error_code,
            });
        (messages, latest_turn)
    } else {
        (Vec::new(), None)
    };
    let persisted_provider_profile_id = latest_turn
        .as_ref()
        .map(|turn| turn.provider_profile_id.clone());
    drop(store);
    let messages = project_conversation_attachments(messages, state);
    let conversation_task_reference_counts = if state
        .persistence_coordinator
        .require_trusted_read("CanonicalTaskRuntimeStore")
        .is_ok()
    {
        if let Some(task_store) = state.canonical_task_runtime_store.as_ref() {
            let task_store = task_store.lock().await;
            all_records
                .iter()
                .map(|record| {
                    (
                        record.id.clone(),
                        task_store
                            .conversation_task_reference_counts(&record.id)
                            .ok(),
                    )
                })
                .collect::<std::collections::HashMap<_, _>>()
        } else {
            std::collections::HashMap::new()
        }
    } else {
        std::collections::HashMap::new()
    };
    let mut conversations = Vec::new();
    let mut archived_conversations = Vec::new();
    for record in all_records {
        let history_counts = conversation_history_counts
            .get(&record.id)
            .copied()
            .unwrap_or_default();
        let task_counts = conversation_task_reference_counts
            .get(&record.id)
            .copied()
            .flatten();
        let status = record.status;
        let item = conversation_lifecycle_view_model(record, history_counts, task_counts);
        if status == openlife_core::conversation::ConversationStatus::Active {
            conversations.push(item);
        } else {
            archived_conversations.push(item);
        }
    }
    let task_run_reference_counts = if state
        .persistence_coordinator
        .require_trusted_read("CanonicalTaskRuntimeStore")
        .is_ok()
    {
        if let Some(task_store) = state.canonical_task_runtime_store.as_ref() {
            let task_store = task_store.lock().await;
            project_facts
                .iter()
                .map(|facts| {
                    let count = task_store
                        .project_run_reference_count(&facts.project.id)
                        .ok();
                    (facts.project.id.clone(), count)
                })
                .collect::<std::collections::HashMap<_, _>>()
        } else {
            project_facts
                .iter()
                .map(|facts| (facts.project.id.clone(), None))
                .collect()
        }
    } else {
        project_facts
            .iter()
            .map(|facts| (facts.project.id.clone(), None))
            .collect()
    };
    let projects = project_facts
        .into_iter()
        .map(|facts| {
            let task_run_reference_count = task_run_reference_counts
                .get(&facts.project.id)
                .copied()
                .flatten();
            project_lifecycle_view_model(facts, task_run_reference_count)
        })
        .collect::<Vec<_>>();
    let global_memory_enabled = state.config.lock().await.system.agent_memory_enabled;
    let (provider_status, provider_profiles, selected_provider_profile_id, provider_error_code) =
        match crate::provider_registry::provider_profile_registry(state).await {
            Ok(registry) => {
                let selected_id = persisted_provider_profile_id
                    .filter(|id| {
                        registry
                            .profiles
                            .iter()
                            .any(|profile| &profile.profile_id == id)
                    })
                    .or_else(|| registry.default_profile_id.clone());
                let selected = selected_id.as_deref().and_then(|id| {
                    registry
                        .profiles
                        .iter()
                        .find(|profile| profile.profile_id == id)
                });
                let ready = selected.is_some_and(|profile| profile.availability == "ready");
                let error = selected
                    .and_then(|profile| profile.unavailable_reason.clone())
                    .or(registry.default_error_code);
                (
                    if ready { "ready" } else { "unavailable" }.to_string(),
                    registry.profiles,
                    selected_id,
                    error,
                )
            }
            Err(error) => ("unavailable".to_string(), Vec::new(), None, Some(error)),
        };
    Ok(ConversationViewModel {
        status: if conversations.is_empty() && selected.is_none() {
            "empty"
        } else {
            "ready"
        }
        .into(),
        conversations,
        archived_conversations,
        projects,
        selected_project_id,
        selected_conversation_id: selected,
        global_memory_enabled,
        selected_memory_mode,
        messages,
        latest_turn,
        provider_status,
        provider_profiles,
        selected_provider_profile_id,
        provider_error_code,
        work_status: if state.canonical_task_runtime_store.is_some() {
            "available".into()
        } else {
            "unavailable".into()
        },
    })
}

fn conversation_lifecycle_view_model(
    record: openlife_core::conversation::ConversationRecord,
    (turn_count, item_count): (u64, u64),
    task_counts: Option<(u64, u64)>,
) -> ConversationListItemViewModel {
    use openlife_core::conversation::ConversationStatus;
    let (task_reference_count, active_task_count) = task_counts
        .map(|(total, active)| (Some(total), Some(active)))
        .unwrap_or((None, None));
    let mut allowed_controls = Vec::new();
    let mut blocker_codes = Vec::new();
    match record.status {
        ConversationStatus::Active => {
            if active_task_count == Some(0) {
                allowed_controls.push("archive".into());
            } else if active_task_count.is_none() {
                blocker_codes.push("conversation_task_history_unknown".into());
            } else {
                blocker_codes.push("conversation_archive_active_task_present".into());
            }
        }
        ConversationStatus::Archived => {
            allowed_controls.push("restore".into());
            if turn_count == 0 && item_count == 0 && task_reference_count == Some(0) {
                allowed_controls.push("delete".into());
            } else {
                if turn_count > 0 || item_count > 0 {
                    blocker_codes.push("conversation_delete_history_present".into());
                }
                match task_reference_count {
                    Some(count) if count > 0 => {
                        blocker_codes.push("conversation_delete_task_history_present".into())
                    }
                    None => blocker_codes.push("conversation_task_history_unknown".into()),
                    _ => {}
                }
            }
        }
    }
    ConversationListItemViewModel {
        session_id: record.id,
        title: record.title,
        status: record.status,
        turn_count,
        item_count,
        task_reference_count,
        active_task_count,
        allowed_controls,
        blocker_codes,
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    }
}

fn project_conversation_attachments(
    mut messages: Vec<ConversationMessageViewModel>,
    state: &Arc<AppState>,
) -> Vec<ConversationMessageViewModel> {
    if state
        .persistence_coordinator
        .require_trusted_read("ResourceStore")
        .is_err()
    {
        return messages;
    }
    let Some(store) = state
        .resource_runtime
        .as_ref()
        .map(|runtime| runtime.gateway().store())
    else {
        return messages;
    };
    for message in messages.iter_mut().filter(|message| message.role == "user") {
        let Ok(resources) = store.list_resources_for_message(&message.turn_id) else {
            continue;
        };
        message.attachments = resources
            .into_iter()
            .map(|resource| ConversationAttachmentViewModel {
                resource_id: resource.resource_id,
                filename: resource.filename,
                detected_mime: resource.detected_mime,
                format: resource.format,
                digest: resource.digest,
                byte_count: resource.byte_count,
                chunk_count: resource.chunk_count,
            })
            .collect();
        message.attachments_status = "ready".into();
    }
    messages
}

fn project_lifecycle_view_model(
    facts: openlife_core::conversation::ProjectLifecycleFacts,
    task_run_reference_count: Option<u64>,
) -> ProjectLifecycleViewModel {
    use openlife_core::conversation::ProjectStatus;
    let mut allowed_controls = Vec::new();
    let mut blocker_codes = Vec::new();
    match facts.project.status {
        ProjectStatus::Active => {
            allowed_controls.push("update".into());
            if facts.active_conversation_count == 0 {
                allowed_controls.push("archive".into());
            } else {
                blocker_codes.push("project_archive_active_conversations_present".into());
            }
        }
        ProjectStatus::Archived => {
            allowed_controls.push("restore".into());
            if facts.total_conversation_count == 0
                && task_run_reference_count == Some(0)
                && !facts.selected_for_new_conversation
            {
                allowed_controls.push("delete".into());
            } else {
                if facts.total_conversation_count > 0 {
                    blocker_codes.push("project_delete_conversation_history_present".into());
                }
                if task_run_reference_count.is_some_and(|count| count > 0) {
                    blocker_codes.push("project_delete_task_history_present".into());
                } else if task_run_reference_count.is_none() {
                    blocker_codes.push("project_delete_task_history_unknown".into());
                }
                if facts.selected_for_new_conversation {
                    blocker_codes.push("project_delete_selected_for_new_conversation".into());
                }
            }
        }
    }
    ProjectLifecycleViewModel {
        project: facts.project,
        active_conversation_count: facts.active_conversation_count,
        total_conversation_count: facts.total_conversation_count,
        task_run_reference_count,
        selected_for_new_conversation: facts.selected_for_new_conversation,
        allowed_controls,
        blocker_codes,
    }
}

#[tauri::command]
pub async fn set_conversation_memory_mode(
    conversation_id: String,
    mode: openlife_core::conversation::ConversationMemoryMode,
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
        .set_memory_mode(&conversation_id, mode)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn get_conversation_view_model(
    conversation_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<ConversationViewModel, AppError> {
    get_conversation_view_model_with_state(conversation_id.as_deref(), state.inner()).await
}

#[cfg(test)]
pub(crate) async fn create_chat_session_with_state(
    session_id: &str,
    title: &str,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    create_chat_session_with_admission_with_state(
        session_id,
        title,
        None,
        None,
        openlife_core::conversation::ConversationMemoryMode::default(),
        state,
    )
    .await
}

pub(crate) async fn create_chat_session_with_admission_with_state(
    session_id: &str,
    title: &str,
    project_id: Option<&str>,
    selected_skill_id: Option<&str>,
    memory_mode: openlife_core::conversation::ConversationMemoryMode,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    if let Some(skill_id) = selected_skill_id {
        let detail =
            crate::main_chat_skills_tools::get_main_chat_skill_detail_with_state(state, skill_id)
                .await
                .map_err(|error| {
                    AppError::internal_with_code(error, "conversation_admission_skill_unavailable")
                })?;
        let available =
            crate::main_chat_skills_tools::list_main_chat_skills_with_state(state, None)
                .await
                .map_err(|error| {
                    AppError::internal_with_code(error, "conversation_admission_skill_unavailable")
                })?
                .into_iter()
                .any(|skill| skill.skill_id == detail.skill_id && skill.available);
        if !available {
            return Err(AppError::internal_with_code(
                "selected skill is not available for Main Chat",
                "conversation_admission_skill_unavailable",
            ));
        }
    }
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
        .create_conversation_with_admission(
            openlife_core::conversation::CreateConversationAdmission {
                id: session_id,
                title,
                project_id,
                selected_skill_id,
                memory_mode,
            },
        )
        .map(|_| ())
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn create_chat_session(
    session_id: String,
    title: String,
    project_id: Option<String>,
    selected_skill_id: Option<String>,
    memory_mode: openlife_core::conversation::ConversationMemoryMode,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    create_chat_session_with_admission_with_state(
        &session_id,
        &title,
        project_id.as_deref(),
        selected_skill_id.as_deref(),
        memory_mode,
        state.inner(),
    )
    .await
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDirectoryCreationResult {
    pub cancelled: bool,
    pub project: Option<openlife_core::conversation::ProjectRecord>,
}

fn validate_project_workspace_directory(path: &Path) -> Result<PathBuf, AppError> {
    let metadata = path.symlink_metadata().map_err(|error| {
        AppError::permission(format!(
            "project workspace directory cannot be inspected: {error}"
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AppError::permission(
            "project workspace selection must be a real directory",
        ));
    }
    let canonical = path.canonicalize().map_err(|error| {
        AppError::permission(format!(
            "project workspace directory cannot be canonicalized: {error}"
        ))
    })?;
    if canonical.parent().is_none() {
        return Err(AppError::permission(
            "filesystem root cannot be used as a project workspace",
        ));
    }
    Ok(canonical)
}

fn normalized_project_name(name: Option<&str>, workspace_root: &Path) -> Result<String, AppError> {
    let explicit = name
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|value| !value.is_empty());
    let derived = workspace_root
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    explicit
        .or(derived)
        .ok_or_else(|| AppError::permission("project workspace name is unavailable"))
}

pub(crate) async fn create_project_from_directory_with_state(
    project_id: &str,
    name: Option<&str>,
    selected_directory: &Path,
    state: &Arc<AppState>,
) -> Result<openlife_core::conversation::ProjectRecord, AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let canonical = validate_project_workspace_directory(selected_directory)?;
    let name = normalized_project_name(name, &canonical)?;
    let canonical = canonical.to_string_lossy().into_owned();
    state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?
        .lock()
        .await
        .create_project_as_new_conversation_scope(project_id, &name, &canonical)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn create_project_from_directory<R: Runtime>(
    project_id: String,
    name: Option<String>,
    app_handle: tauri::AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<ProjectDirectoryCreationResult, AppError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app_handle
        .dialog()
        .file()
        .set_title("选择 OpenLife Project 文件夹")
        .pick_folder(move |path| {
            let _ = sender.send(path);
        });
    let selected = receiver
        .await
        .map_err(|_| AppError::internal("project workspace picker closed without a result"))?;
    let Some(selected) = selected else {
        return Ok(ProjectDirectoryCreationResult {
            cancelled: true,
            project: None,
        });
    };
    let selected = selected
        .into_path()
        .map_err(|_| AppError::permission("project workspace picker returned an invalid path"))?;
    let project = create_project_from_directory_with_state(
        &project_id,
        name.as_deref(),
        &selected,
        state.inner(),
    )
    .await?;
    Ok(ProjectDirectoryCreationResult {
        cancelled: false,
        project: Some(project),
    })
}

pub(crate) async fn bind_project_directory_with_state(
    project_id: &str,
    expected_revision: u64,
    selected_directory: &Path,
    state: &Arc<AppState>,
) -> Result<openlife_core::conversation::ProjectRecord, AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let canonical = validate_project_workspace_directory(selected_directory)?;
    let canonical = canonical.to_string_lossy().into_owned();
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?
        .lock()
        .await;
    let project = store
        .get_project(project_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found("project_not_found"))?;
    if project.revision != expected_revision {
        return Err(AppError::internal_with_code(
            "project scope changed before directory selection completed",
            "project_scope_revision_conflict",
        ));
    }
    store
        .update_project_scope(
            project_id,
            &project.name,
            Some(&canonical),
            expected_revision,
        )
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn bind_project_directory<R: Runtime>(
    project_id: String,
    expected_revision: u64,
    app_handle: tauri::AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<ProjectDirectoryCreationResult, AppError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app_handle
        .dialog()
        .file()
        .set_title("选择 OpenLife Project 文件夹")
        .pick_folder(move |path| {
            let _ = sender.send(path);
        });
    let selected = receiver
        .await
        .map_err(|_| AppError::internal("project workspace picker closed without a result"))?;
    let Some(selected) = selected else {
        return Ok(ProjectDirectoryCreationResult {
            cancelled: true,
            project: None,
        });
    };
    let selected = selected
        .into_path()
        .map_err(|_| AppError::permission("project workspace picker returned an invalid path"))?;
    let project =
        bind_project_directory_with_state(&project_id, expected_revision, &selected, state.inner())
            .await?;
    Ok(ProjectDirectoryCreationResult {
        cancelled: false,
        project: Some(project),
    })
}

pub(crate) async fn add_project_read_root_with_state(
    project_id: &str,
    expected_revision: u64,
    selected_directory: &Path,
    state: &Arc<AppState>,
) -> Result<openlife_core::conversation::ProjectRecord, AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let canonical = validate_project_workspace_directory(selected_directory)?;
    let canonical_path = canonical.to_string_lossy().into_owned();
    let name = normalized_project_name(None, &canonical)?;
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?
        .lock()
        .await;
    let project = store
        .get_project(project_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found("project_not_found"))?;
    if project.revision != expected_revision {
        return Err(AppError::internal_with_code(
            "project scope changed before directory selection completed",
            "project_scope_revision_conflict",
        ));
    }
    let matches_selected = |existing: &str| {
        Path::new(existing)
            .canonicalize()
            .is_ok_and(|path| path == canonical)
    };
    if project
        .workspace_root
        .as_deref()
        .is_some_and(matches_selected)
        || project
            .additional_read_roots
            .iter()
            .any(|root| matches_selected(&root.path))
    {
        return Err(AppError::internal_with_code(
            "directory is already part of the Project read scope",
            "project_read_root_already_exists",
        ));
    }
    store
        .add_project_read_root(
            project_id,
            &uuid::Uuid::new_v4().to_string(),
            &name,
            &canonical_path,
            expected_revision,
        )
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn add_project_read_root<R: Runtime>(
    project_id: String,
    expected_revision: u64,
    app_handle: tauri::AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<ProjectDirectoryCreationResult, AppError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app_handle
        .dialog()
        .file()
        .set_title("选择 Project 附加读取文件夹")
        .pick_folder(move |path| {
            let _ = sender.send(path);
        });
    let selected = receiver
        .await
        .map_err(|_| AppError::internal("project read root picker closed without a result"))?;
    let Some(selected) = selected else {
        return Ok(ProjectDirectoryCreationResult {
            cancelled: true,
            project: None,
        });
    };
    let selected = selected
        .into_path()
        .map_err(|_| AppError::permission("project read root picker returned an invalid path"))?;
    let project =
        add_project_read_root_with_state(&project_id, expected_revision, &selected, state.inner())
            .await?;
    Ok(ProjectDirectoryCreationResult {
        cancelled: false,
        project: Some(project),
    })
}

pub(crate) async fn remove_project_read_root_with_state(
    project_id: &str,
    root_id: &str,
    expected_revision: u64,
    state: &Arc<AppState>,
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
        .remove_project_read_root(project_id, root_id, expected_revision)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn remove_project_read_root(
    project_id: String,
    root_id: String,
    expected_revision: u64,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::conversation::ProjectRecord, AppError> {
    remove_project_read_root_with_state(&project_id, &root_id, expected_revision, state.inner())
        .await
}

pub(crate) async fn update_project_name_with_state(
    project_id: &str,
    name: &str,
    expected_revision: u64,
    state: &Arc<AppState>,
) -> Result<openlife_core::conversation::ProjectRecord, AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?
        .lock()
        .await;
    let project = store
        .get_project(project_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found("project_not_found"))?;
    store
        .update_project_scope(
            project_id,
            name,
            project.workspace_root.as_deref(),
            expected_revision,
        )
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn update_project_name(
    project_id: String,
    name: String,
    expected_revision: u64,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::conversation::ProjectRecord, AppError> {
    update_project_name_with_state(&project_id, &name, expected_revision, state.inner()).await
}

pub(crate) async fn archive_project_with_state(
    project_id: &str,
    expected_revision: u64,
    state: &Arc<AppState>,
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
        .archive_project(project_id, expected_revision)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn archive_project(
    project_id: String,
    expected_revision: u64,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::conversation::ProjectRecord, AppError> {
    archive_project_with_state(&project_id, expected_revision, state.inner()).await
}

pub(crate) async fn restore_project_with_state(
    project_id: &str,
    expected_revision: u64,
    state: &Arc<AppState>,
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
        .restore_project(project_id, expected_revision)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn restore_project(
    project_id: String,
    expected_revision: u64,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::conversation::ProjectRecord, AppError> {
    restore_project_with_state(&project_id, expected_revision, state.inner()).await
}

fn ensure_project_delete_eligible(
    facts: &openlife_core::conversation::ProjectLifecycleFacts,
    task_run_reference_count: u64,
) -> Result<(), AppError> {
    use openlife_core::conversation::ProjectStatus;
    if facts.project.status != ProjectStatus::Archived {
        return Err(AppError::internal_with_code(
            "Project must be archived before permanent deletion",
            "project_delete_requires_archived",
        ));
    }
    if facts.total_conversation_count > 0 {
        return Err(AppError::internal_with_code(
            "Project is still referenced by Conversation history",
            "project_delete_conversation_history_present",
        ));
    }
    if task_run_reference_count > 0 {
        return Err(AppError::internal_with_code(
            "Project is still referenced by canonical Task history",
            "project_delete_task_history_present",
        ));
    }
    if facts.selected_for_new_conversation {
        return Err(AppError::internal_with_code(
            "Project is still selected for a new Conversation",
            "project_delete_selected_for_new_conversation",
        ));
    }
    Ok(())
}

async fn project_deletion_preflight(
    project_id: &str,
    expected_revision: u64,
    state: &Arc<AppState>,
) -> Result<openlife_core::conversation::ProjectRecord, AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore", "CanonicalTaskRuntimeStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let conversation_store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?;
    let task_store = state.canonical_task_runtime_store.as_ref().ok_or_else(|| {
        AppError::internal_with_code(
            "Task history is unavailable, so Project deletion cannot be proven safe",
            "project_delete_task_history_unknown",
        )
    })?;
    let facts = conversation_store
        .lock()
        .await
        .project_lifecycle_facts(project_id)
        .map_err(AppError::from)?;
    if facts.project.revision != expected_revision {
        return Err(AppError::internal_with_code(
            "Project changed before deletion eligibility was checked",
            "project_delete_revision_conflict",
        ));
    }
    let task_run_reference_count = task_store
        .lock()
        .await
        .project_run_reference_count(project_id)
        .map_err(AppError::from)?;
    ensure_project_delete_eligible(&facts, task_run_reference_count)?;
    Ok(facts.project)
}

pub(crate) async fn delete_project_metadata_with_state(
    project_id: &str,
    expected_revision: u64,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    let conversation_store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?;
    let task_store = state.canonical_task_runtime_store.as_ref().ok_or_else(|| {
        AppError::internal_with_code(
            "Task history is unavailable, so Project deletion cannot be proven safe",
            "project_delete_task_history_unknown",
        )
    })?;

    // The short final critical section closes the gap between the last reference
    // check and metadata deletion across the two canonical stores.
    let conversation_store = conversation_store.lock().await;
    let task_store = task_store.lock().await;
    let facts = conversation_store
        .project_lifecycle_facts(project_id)
        .map_err(AppError::from)?;
    if facts.project.revision != expected_revision {
        return Err(AppError::internal_with_code(
            "Project changed after native deletion confirmation",
            "project_delete_revision_conflict",
        ));
    }
    let task_run_reference_count = task_store
        .project_run_reference_count(project_id)
        .map_err(AppError::from)?;
    ensure_project_delete_eligible(&facts, task_run_reference_count)?;
    conversation_store
        .delete_archived_project(project_id, expected_revision)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn delete_project<R: Runtime>(
    project_id: String,
    expected_revision: u64,
    window: WebviewWindow<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let project = project_deletion_preflight(&project_id, expected_revision, state.inner()).await?;
    let arguments = serde_json::json!({
        "project_id": project.id,
        "expected_revision": expected_revision,
        "operation": "irreversible_project_metadata_delete",
    });
    require_native_danger_action_confirmation(
        &window,
        NativeDangerActionRequest {
            action_type: "project_metadata_delete",
            target_ids_for_new_challenge: std::slice::from_ref(&project_id),
            requested_target: Some(project_id.as_str()),
            affected_count: 1,
            arguments: &arguments,
            arguments_summary:
                "永久删除一个已归档且没有 Conversation 或 Task 引用的 Project 记录。",
            scope_summary: "不会删除本地文件夹或其中内容；Project 元数据删除后不可恢复。",
            challenge_id: None,
        },
    )
    .await?;
    delete_project_metadata_with_state(&project_id, expected_revision, state.inner()).await
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
pub async fn select_new_conversation_project(
    project_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    select_new_conversation_project_with_state(project_id.as_deref(), state.inner()).await
}

pub(crate) async fn select_new_conversation_project_with_state(
    project_id: Option<&str>,
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
        .set_new_conversation_project(project_id)
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
pub async fn archive_chat_session(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore", "CanonicalTaskRuntimeStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let conversation_store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?;
    let task_store = state.canonical_task_runtime_store.as_ref().ok_or_else(|| {
        AppError::internal_with_code(
            "Task history is unavailable, so Conversation archive safety is unknown",
            "conversation_task_history_unknown",
        )
    })?;
    let conversation_store = conversation_store.lock().await;
    let task_store = task_store.lock().await;
    let (_, active_task_count) = task_store
        .conversation_task_reference_counts(&session_id)
        .map_err(AppError::from)?;
    if active_task_count > 0 {
        return Err(AppError::internal_with_code(
            "An active Work Task still belongs to this Conversation",
            "conversation_archive_active_task_present",
        ));
    }
    conversation_store
        .archive_conversation(&session_id)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn restore_chat_session(
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
        .restore_conversation(&session_id)
        .map_err(AppError::from)
}

fn ensure_conversation_delete_eligible(
    conversation: &openlife_core::conversation::ConversationRecord,
    history_counts: (u64, u64),
    task_counts: (u64, u64),
) -> Result<(), AppError> {
    use openlife_core::conversation::ConversationStatus;
    if conversation.status != ConversationStatus::Archived {
        return Err(AppError::internal_with_code(
            "Conversation must be archived before permanent deletion",
            "conversation_delete_requires_archived",
        ));
    }
    if history_counts.0 > 0 || history_counts.1 > 0 {
        return Err(AppError::internal_with_code(
            "Conversation history must be retained by its canonical owner",
            "conversation_delete_history_present",
        ));
    }
    if task_counts.0 > 0 {
        return Err(AppError::internal_with_code(
            "Canonical Task history still references this Conversation",
            "conversation_delete_task_history_present",
        ));
    }
    Ok(())
}

async fn conversation_deletion_preflight(
    session_id: &str,
    state: &Arc<AppState>,
) -> Result<openlife_core::conversation::ConversationRecord, AppError> {
    state
        .persistence_coordinator
        .require_effects_for_stores(&["ConversationStore", "CanonicalTaskRuntimeStore"])
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let conversation_store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?;
    let task_store = state.canonical_task_runtime_store.as_ref().ok_or_else(|| {
        AppError::internal_with_code(
            "Task history is unavailable, so Conversation deletion cannot be proven safe",
            "conversation_task_history_unknown",
        )
    })?;
    let conversation_store = conversation_store.lock().await;
    let task_store = task_store.lock().await;
    let conversation = conversation_store
        .get_conversation(session_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found("conversation_not_found"))?;
    let history_counts = conversation_store
        .conversation_history_counts(session_id)
        .map_err(AppError::from)?;
    let task_counts = task_store
        .conversation_task_reference_counts(session_id)
        .map_err(AppError::from)?;
    ensure_conversation_delete_eligible(&conversation, history_counts, task_counts)?;
    Ok(conversation)
}

async fn delete_conversation_metadata_with_state(
    session_id: &str,
    state: &Arc<AppState>,
) -> Result<(), AppError> {
    let conversation_store = state
        .conversation_store
        .as_ref()
        .ok_or_else(|| AppError::internal("conversation_store_unavailable"))?;
    let task_store = state.canonical_task_runtime_store.as_ref().ok_or_else(|| {
        AppError::internal_with_code(
            "Task history is unavailable, so Conversation deletion cannot be proven safe",
            "conversation_task_history_unknown",
        )
    })?;
    let conversation_store = conversation_store.lock().await;
    let task_store = task_store.lock().await;
    let conversation = conversation_store
        .get_conversation(session_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found("conversation_not_found"))?;
    let history_counts = conversation_store
        .conversation_history_counts(session_id)
        .map_err(AppError::from)?;
    let task_counts = task_store
        .conversation_task_reference_counts(session_id)
        .map_err(AppError::from)?;
    ensure_conversation_delete_eligible(&conversation, history_counts, task_counts)?;
    conversation_store
        .delete_conversation(session_id)
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn delete_chat_session<R: Runtime>(
    session_id: String,
    window: WebviewWindow<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    let conversation = conversation_deletion_preflight(&session_id, state.inner()).await?;
    let arguments = serde_json::json!({
        "conversation_id": conversation.id,
        "operation": "irreversible_empty_conversation_metadata_delete",
    });
    require_native_danger_action_confirmation(
        &window,
        NativeDangerActionRequest {
            action_type: "conversation_metadata_delete",
            target_ids_for_new_challenge: std::slice::from_ref(&session_id),
            requested_target: Some(session_id.as_str()),
            affected_count: 1,
            arguments: &arguments,
            arguments_summary: "永久删除一个已归档且没有消息、Turn 或 Task 引用的空对话记录。",
            scope_summary: "不会删除任何任务、文件、审核、Artifact、记忆或证据；存在任一引用时操作会失败关闭。",
            challenge_id: None,
        },
    )
    .await?;
    delete_conversation_metadata_with_state(&session_id, state.inner()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = openlife_core::config::AppConfig::default();
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
                openlife_core::memory::KnowledgeNoteProjectionStore::new_in_memory().unwrap(),
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
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp_audit::McpAuditStore::new(temp_dir.path().join("mcp_audit.db")),
            )),
            canonical_task_runtime_store: None,
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
            tool_permission_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            )),
            skill_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::skills::SkillRegistry::built_in(),
            )),
            startup_warnings: vec![],
            credential_bootstrap_snapshot: Default::default(),
            web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            work_initial_decision_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            work_steering_replan_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            work_agent_step_fixture_outputs: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            work_semantic_verification_fixture_outputs: Arc::new(tokio::sync::Mutex::new(
                Vec::new(),
            )),
            resource_runtime: None,
        })
    }

    #[tokio::test]
    async fn create_chat_session_is_visible_through_the_conversation_view_model() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state)
            .unwrap()
            .canonical_task_runtime_store = Some(Arc::new(tokio::sync::Mutex::new(
            openlife_core::task_runtime::CanonicalTaskRuntimeStore::new_in_memory().unwrap(),
        )));

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
    async fn create_chat_session_admits_project_and_memory_before_first_turn() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let session_id = uuid::Uuid::new_v4().to_string();
        let project_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_project(&project_id, "Exact admission", None)
            .unwrap();

        create_chat_session_with_admission_with_state(
            &session_id,
            "Private first turn",
            Some(&project_id),
            Some("evidence_review"),
            openlife_core::conversation::ConversationMemoryMode::Off,
            &state,
        )
        .await
        .unwrap();

        let view = get_conversation_view_model_with_state(Some(&session_id), &state)
            .await
            .unwrap();
        assert_eq!(
            view.selected_project_id.as_deref(),
            Some(project_id.as_str())
        );
        assert_eq!(
            view.selected_memory_mode,
            openlife_core::conversation::ConversationMemoryMode::Off
        );
        assert!(state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_conversation(&session_id)
            .unwrap()
            .is_some_and(|conversation| {
                conversation.selected_skill_id.as_deref() == Some("evidence_review")
            }));
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
        assert_eq!(view.projects[0].project.name, "Research Project");
        assert_eq!(
            view.selected_project_id.as_deref(),
            Some(project_id.as_str())
        );
        assert!(view.global_memory_enabled);
        assert_eq!(
            view.selected_memory_mode,
            openlife_core::conversation::ConversationMemoryMode::UseAndLearn
        );
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .set_memory_mode(
                &conversation_id,
                openlife_core::conversation::ConversationMemoryMode::UseOnly,
            )
            .unwrap();
        let updated = get_conversation_view_model_with_state(Some(&conversation_id), &state)
            .await
            .unwrap();
        assert_eq!(
            updated.selected_memory_mode,
            openlife_core::conversation::ConversationMemoryMode::UseOnly
        );
    }

    #[tokio::test]
    async fn existing_project_can_be_selected_before_the_first_conversation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let project_id = uuid::Uuid::new_v4().to_string();
        state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_project(&project_id, "Next Conversation", Some("/tmp/next"))
            .unwrap();

        select_new_conversation_project_with_state(Some(&project_id), &state)
            .await
            .unwrap();

        let view = get_conversation_view_model_with_state(None, &state)
            .await
            .unwrap();
        assert_eq!(
            view.selected_project_id.as_deref(),
            Some(project_id.as_str())
        );
        assert!(view.projects[0].selected_for_new_conversation);
    }

    #[tokio::test]
    async fn project_delete_control_is_derived_from_conversation_and_task_references() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state)
            .unwrap()
            .canonical_task_runtime_store = Some(Arc::new(tokio::sync::Mutex::new(
            openlife_core::task_runtime::CanonicalTaskRuntimeStore::new_in_memory().unwrap(),
        )));
        let project_id = uuid::Uuid::new_v4().to_string();
        let project = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_project(&project_id, "Reference-safe Project", Some("/tmp/project"))
            .unwrap();
        let archived = archive_project_with_state(&project_id, project.revision, &state)
            .await
            .unwrap();

        let view = get_conversation_view_model_with_state(None, &state)
            .await
            .unwrap();
        assert_eq!(view.projects[0].task_run_reference_count, Some(0));
        assert!(view.projects[0]
            .allowed_controls
            .contains(&"delete".to_string()));

        let task_id = uuid::Uuid::new_v4().to_string();
        let conversation_id = uuid::Uuid::new_v4().to_string();
        let run_id = uuid::Uuid::new_v4().to_string();
        let instruction_digest = openlife_core::persistence_outbox::metadata_digest("instruction");
        let scope_digest =
            openlife_core::conversation::ConversationStore::project_scope_digest(&archived);
        state
            .canonical_task_runtime_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .begin_general_task_run(openlife_core::task_runtime::BeginGeneralTaskRunInput {
                task_id: &task_id,
                conversation_id: &conversation_id,
                run_id: &run_id,
                execution_session_id: &run_id,
                instruction_digest: &instruction_digest,
                plan_digest: None,
                project_id: Some(&project_id),
                project_revision: Some(archived.revision),
                scope_digest: Some(&scope_digest),
                execution_mode: openlife_core::task_runtime::WorkExecutionMode::ScopedAgent,
            })
            .unwrap();

        let view = get_conversation_view_model_with_state(None, &state)
            .await
            .unwrap();
        assert_eq!(view.projects[0].task_run_reference_count, Some(1));
        assert!(!view.projects[0]
            .allowed_controls
            .contains(&"delete".to_string()));
        assert!(view.projects[0]
            .blocker_codes
            .contains(&"project_delete_task_history_present".to_string()));
        let error = delete_project_metadata_with_state(&project_id, archived.revision, &state)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::Internal { code: Some(code), .. }
                if code == "project_delete_task_history_present"
        ));
    }

    #[tokio::test]
    async fn archived_unreferenced_project_metadata_can_be_deleted_without_touching_its_folder() {
        let temp_dir = tempfile::tempdir().unwrap();
        let workspace = temp_dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(workspace.join("user-file.txt"), "keep me").unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state)
            .unwrap()
            .canonical_task_runtime_store = Some(Arc::new(tokio::sync::Mutex::new(
            openlife_core::task_runtime::CanonicalTaskRuntimeStore::new_in_memory().unwrap(),
        )));
        let project_id = uuid::Uuid::new_v4().to_string();
        let project = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_project(&project_id, "Deletable Project", workspace.to_str())
            .unwrap();
        let archived = archive_project_with_state(&project_id, project.revision, &state)
            .await
            .unwrap();

        delete_project_metadata_with_state(&project_id, archived.revision, &state)
            .await
            .unwrap();

        assert!(state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_project(&project_id)
            .unwrap()
            .is_none());
        assert_eq!(
            std::fs::read_to_string(workspace.join("user-file.txt")).unwrap(),
            "keep me"
        );
    }

    #[tokio::test]
    async fn project_directory_creation_persists_the_exact_canonical_scope() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let selected = temp_dir.path().join("selected workspace");
        std::fs::create_dir(&selected).unwrap();
        let project_id = uuid::Uuid::new_v4().to_string();

        let project =
            create_project_from_directory_with_state(&project_id, None, &selected, &state)
                .await
                .unwrap();

        assert_eq!(project.name, "selected workspace");
        assert_eq!(
            project.workspace_root.as_deref(),
            selected.canonicalize().unwrap().to_str()
        );
        let persisted = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_project(&project_id)
            .unwrap()
            .unwrap();
        assert_eq!(persisted, project);
        let view = get_conversation_view_model_with_state(None, &state)
            .await
            .unwrap();
        assert_eq!(
            view.selected_project_id.as_deref(),
            Some(project_id.as_str())
        );
    }

    #[tokio::test]
    async fn project_directory_binding_advances_the_scope_revision() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let selected = temp_dir.path().join("bound workspace");
        std::fs::create_dir(&selected).unwrap();
        let project_id = uuid::Uuid::new_v4().to_string();
        let project = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_project(&project_id, "Existing Project", None)
            .unwrap();

        let updated =
            bind_project_directory_with_state(&project_id, project.revision, &selected, &state)
                .await
                .unwrap();

        assert_eq!(updated.name, project.name);
        assert_eq!(updated.revision, project.revision + 1);
        assert_eq!(
            updated.workspace_root.as_deref(),
            selected.canonicalize().unwrap().to_str()
        );
    }

    #[tokio::test]
    async fn project_additional_read_root_is_visible_and_removal_keeps_user_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let primary = temp_dir.path().join("primary");
        let additional = temp_dir.path().join("reference notes");
        std::fs::create_dir(&primary).unwrap();
        std::fs::create_dir(&additional).unwrap();
        std::fs::write(additional.join("keep.txt"), "user owned").unwrap();
        let project_id = uuid::Uuid::new_v4().to_string();
        let project = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_project(&project_id, "Project", Some(primary.to_str().unwrap()))
            .unwrap();

        let added =
            add_project_read_root_with_state(&project_id, project.revision, &additional, &state)
                .await
                .unwrap();

        assert_eq!(added.revision, project.revision + 1);
        assert_eq!(added.additional_read_roots.len(), 1);
        assert_eq!(added.additional_read_roots[0].name, "reference notes");
        assert_eq!(
            added.additional_read_roots[0].path,
            additional.canonicalize().unwrap().to_string_lossy()
        );
        let view = get_conversation_view_model_with_state(None, &state)
            .await
            .unwrap();
        assert_eq!(view.projects[0].project.additional_read_roots.len(), 1);

        let removed = remove_project_read_root_with_state(
            &project_id,
            &added.additional_read_roots[0].id,
            added.revision,
            &state,
        )
        .await
        .unwrap();

        assert_eq!(removed.revision, added.revision + 1);
        assert!(removed.additional_read_roots.is_empty());
        assert_eq!(
            std::fs::read_to_string(additional.join("keep.txt")).unwrap(),
            "user owned"
        );
    }

    #[tokio::test]
    async fn project_additional_read_root_rejects_the_primary_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let primary = temp_dir.path().join("primary");
        std::fs::create_dir(&primary).unwrap();
        let project_id = uuid::Uuid::new_v4().to_string();
        let project = state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_project(&project_id, "Project", Some(primary.to_str().unwrap()))
            .unwrap();

        let error =
            add_project_read_root_with_state(&project_id, project.revision, &primary, &state)
                .await
                .unwrap_err();

        assert!(matches!(
            error,
            AppError::Internal {
                code: Some(code),
                ..
            } if code == "project_read_root_already_exists"
        ));
    }

    #[tokio::test]
    async fn conversation_view_model_is_canonical_and_provider_bound() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
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
            reasoning_effort: None,
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

        let resource_store = openlife_core::resource::ResourceStore::new_in_memory().unwrap();
        resource_store
            .commit_import_batch(openlife_core::resource::ResourceImportBatch {
                operation_id: uuid::Uuid::new_v4().to_string(),
                message_id: turn_id.clone(),
                resources: vec![openlife_core::resource::ResourceImportCandidate {
                    resource_id: uuid::Uuid::new_v4().to_string(),
                    filename: "requirements.md".into(),
                    declared_mime: "text/markdown".into(),
                    detected_mime: "text/markdown".into(),
                    format: openlife_core::resource::ResourceFormat::Markdown,
                    bytes: b"durable attachment".to_vec(),
                    chunks: vec![openlife_core::resource::ResourceChunkDraft {
                        content: "durable attachment".into(),
                        provenance: openlife_core::resource::ResourceProvenance::Text {
                            start_line: 1,
                            end_line: 1,
                        },
                    }],
                }],
            })
            .unwrap();
        Arc::get_mut(&mut state).unwrap().resource_runtime =
            Some(Arc::new(crate::resource_commands::ResourceRuntime::new(
                openlife_core::resource_gateway::ResourceGateway::new(
                    resource_store,
                    openlife_core::resource_gateway::ResourceParserProcess::for_current_executable(
                    )
                    .unwrap(),
                ),
            )));

        let view = get_conversation_view_model_with_state(Some(&conversation_id), &state)
            .await
            .unwrap();
        assert_eq!(view.status, "ready");
        assert_eq!(
            view.selected_conversation_id.as_deref(),
            Some(conversation_id.as_str())
        );
        assert_eq!(view.messages.len(), 2);
        assert_eq!(view.messages[0].turn_id, turn_id);
        assert_eq!(view.messages[0].attachments_status, "ready");
        assert_eq!(view.messages[0].attachments.len(), 1);
        assert_eq!(view.messages[0].attachments[0].filename, "requirements.md");
        assert_eq!(view.messages[1].attachments_status, "not_applicable");
        assert_eq!(view.latest_turn.unwrap().turn_id, turn_id);
        assert_eq!(view.provider_status, "unavailable");
        assert_eq!(view.provider_profiles.len(), 1);
        assert_eq!(view.provider_profiles[0].availability, "offline");
        assert_eq!(
            view.selected_provider_profile_id.as_deref(),
            Some(view.provider_profiles[0].profile_id.as_str())
        );
        assert_eq!(
            view.provider_error_code.as_deref(),
            Some("provider_selected_local_route_unavailable")
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

    #[tokio::test]
    async fn conversation_deletion_keeps_history_and_deletes_only_an_empty_archived_record() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(&temp_dir);
        Arc::get_mut(&mut state)
            .unwrap()
            .canonical_task_runtime_store = Some(Arc::new(tokio::sync::Mutex::new(
            openlife_core::task_runtime::CanonicalTaskRuntimeStore::new_in_memory().unwrap(),
        )));
        let history_id = uuid::Uuid::new_v4().to_string();
        let empty_id = uuid::Uuid::new_v4().to_string();
        let turn_id = uuid::Uuid::new_v4().to_string();
        let provider = openlife_core::conversation::ProviderBinding {
            profile_id: "provider-profile:test".into(),
            provider_id: "openai".into(),
            model_id: "gpt-test".into(),
            endpoint_class: "cloud".into(),
            config_generation: "test-generation".into(),
            reasoning_effort: None,
        };
        {
            let store = state.conversation_store.as_ref().unwrap().lock().await;
            store.create_conversation(&history_id, "History").unwrap();
            store
                .begin_chat_turn(openlife_core::conversation::BeginChatTurn {
                    turn_id: &turn_id,
                    conversation_id: &history_id,
                    user_message: "retain me",
                    provider: &provider,
                })
                .unwrap();
            store.cancel_chat_turn(&turn_id).unwrap();
            store.archive_conversation(&history_id).unwrap();
            store.create_conversation(&empty_id, "Empty").unwrap();
            store.archive_conversation(&empty_id).unwrap();
        }

        assert!(delete_conversation_metadata_with_state(&history_id, &state)
            .await
            .is_err());
        let archived_view = get_conversation_view_model_with_state(Some(&history_id), &state)
            .await
            .unwrap();
        assert_eq!(archived_view.status, "ready");
        assert_eq!(
            archived_view.selected_conversation_id.as_deref(),
            Some(history_id.as_str())
        );
        assert_eq!(archived_view.archived_conversations.len(), 2);
        assert!(!archived_view.messages.is_empty());
        assert!(state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_conversation(&history_id)
            .unwrap()
            .is_some());

        delete_conversation_metadata_with_state(&empty_id, &state)
            .await
            .unwrap();
        assert!(state
            .conversation_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_conversation(&empty_id)
            .unwrap()
            .is_none());
    }
}
