#[cfg(all(feature = "dev-extensions", not(debug_assertions)))]
compile_error!("dev-extensions are forbidden in non-debug OpenLife builds");

use openlife_core::llm::ChatMessage;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

mod agent_memory_learning;
pub(crate) mod artifact_materializer;
pub mod bootstrap;
mod canonical_chat_runtime;
mod canonical_work_runtime;
pub mod commands;
mod credential_bootstrap;
pub(crate) mod danger_action_confirmation;
pub mod errors;
pub(crate) mod life_model_learning;
pub(crate) mod life_model_write_gateway;
pub(crate) mod life_state_projection;
pub(crate) mod main_chat_cancellation;
pub(crate) mod main_chat_context_loader;
#[cfg(test)]
pub(crate) mod main_chat_eval_state;
pub(crate) mod main_chat_send;
pub(crate) mod main_chat_skills_tools;
pub(crate) mod main_chat_steering;
pub(crate) mod main_chat_streaming;
pub(crate) mod main_chat_tool_selection;
pub(crate) mod memory_gateway;
pub(crate) mod memory_retrieval_filter;
pub(crate) mod persistence_coordinator;
pub(crate) mod personal_intelligence_ports;
pub(crate) mod product_agent_dto;
pub(crate) mod provider_client;
pub(crate) mod provider_invocation_state;
pub(crate) mod provider_network_consent;
pub(crate) mod provider_registry;
pub(crate) mod provider_runtime;
pub(crate) mod provider_validation;
pub(crate) mod read_models;
pub(crate) mod resource_commands;
pub mod runtime_build_info;
pub(crate) mod runtime_events;
pub(crate) mod secret_store;
pub mod state;
pub mod storage;
pub(crate) mod tool_gateway_resources;
pub(crate) mod workspace_file_resolver;

#[cfg(test)]
mod main_chat_acceptance_test_support;

#[cfg(test)]
mod main_chat_context_loader_tests;

#[cfg(test)]
pub mod test_utils;

pub use state::AppState;

// Re-exports for test modules (imported as crate::...)
use commands::main_chat_tools::{
    clear_main_chat_skill, get_main_chat_skill_detail, list_main_chat_skills,
    list_main_chat_tool_candidates, select_main_chat_skill,
};

use commands::artifact::open_artifact_result;
use commands::chat::{
    add_project_read_root, archive_chat_session, archive_project, assign_conversation_project,
    bind_project_directory, create_chat_session, create_project_from_directory,
    delete_chat_session, delete_project, get_conversation_view_model, remove_project_read_root,
    rename_chat_session, restore_chat_session, restore_project, select_new_conversation_project,
    set_conversation_memory_mode, update_project_name,
};
use commands::life_model::{
    confirm_lifemodel_learning_candidate, delete_lifemodel_learning_candidate,
    draft_legacy_lifemodel_migration, draft_lifemodel_v2_change, draft_lifemodel_v2_export,
    draft_lifemodel_v2_rollback, edit_lifemodel_learning_proposal,
    pause_lifemodel_learning_suggestion_class, reject_lifemodel_learning_candidate,
    stage_lifemodel_learning_candidate,
};
use commands::memory::{
    archive_memory, correct_memory, privacy_erase_memory_asset, restore_memory,
};
use commands::proposal::{
    accept_proposal, postpone_proposal, reject_proposal, request_artifact_undo,
    rollback_memory_asset,
};
use commands::settings::{
    get_config, recover_required_credential_access, save_config, test_llm_connection,
};
use commands::tool_permissions::revoke_tool_permission;
use life_state_projection::get_life_state_projection;
use main_chat_steering::submit_main_chat_task_steering;
pub use openlife_core::privacy::PrivacyEngine;
use read_models::diagnostics::get_product_diagnostics_view_model;
use read_models::life_model::get_life_model_view_model;
use read_models::memory::get_memory_view_model;
use read_models::provider_privacy::get_provider_privacy_boundary_summary;
use read_models::review_center::get_review_center_view_model;
use read_models::tasks::get_workbench_view_model;
use read_models::tool_permissions::get_tool_permission_view_model;
use storage::app_data_dir;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Success,
    Error,
    Pending,
    Blocked,
    NeedsConfirmation,
}

#[derive(Clone)]
pub struct ToolCallResult {
    pub name: String,
    pub arguments: serde_json::Value,
    pub sanitized_arguments: Option<serde_json::Value>,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub permission_level: String,
    pub status: ToolCallStatus,
    pub requires_confirmation: bool,
    pub pii_found: bool,
    pub privacy_warnings: Vec<String>,
    pub action_id: Option<String>,
    pub run_id: Option<String>,
    pub permission_decision: Option<String>,
    pub tool_trace: Option<crate::product_agent_dto::ProductToolActionTrace>,
    /// Runtime-only tool execution authority. Product IPC receives an exact,
    /// body-free ProductToolCallResult projection instead of this receipt.
    pub execution_receipt: Option<openlife_core::tool_execution_receipt::ToolExecutionReceipt>,
    /// Runtime-only proof that the product projection came from the exact
    /// ToolGateway receipt bound to the exact AgentAction.
    pub(crate) product_projection:
        Option<crate::product_agent_dto::VerifiedProductToolCallProjection>,
}

impl serde::Serialize for ToolCallResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(
            &crate::product_agent_dto::ProductToolCallResult::from_internal(self),
            serializer,
        )
    }
}

impl std::fmt::Debug for ToolCallResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToolCallResult")
            .field("name", &self.name)
            .field("arguments", &"[REDACTED]")
            .field(
                "sanitized_arguments",
                &self.sanitized_arguments.as_ref().map(|_| "[REDACTED]"),
            )
            .field("success", &self.success)
            .field("output", &self.output.as_ref().map(|_| "[REDACTED]"))
            .field("error", &self.error.as_ref().map(|_| "[REDACTED]"))
            .field("permission_level", &self.permission_level)
            .field("status", &self.status)
            .field("requires_confirmation", &self.requires_confirmation)
            .field("pii_found", &self.pii_found)
            .field("privacy_warning_count", &self.privacy_warnings.len())
            .field("action_id", &self.action_id)
            .field("run_id", &self.run_id)
            .field("permission_decision", &self.permission_decision)
            .field("tool_trace_present", &self.tool_trace.is_some())
            .field(
                "execution_receipt_present",
                &self.execution_receipt.is_some(),
            )
            .field(
                "product_projection_present",
                &self.product_projection.is_some(),
            )
            .finish()
    }
}

#[derive(serde::Serialize)]
pub struct SendMessageResult {
    pub reply: String,
    pub status: String,
    pub blockers: Vec<String>,
    #[serde(skip_serializing)]
    pub tool_calls: Vec<ToolCallResult>,
    pub run_id: Option<String>,
    pub provider_invocation_status: crate::provider_invocation_state::ProviderInvocationState,
    pub model_invoked: bool,
    pub tool_invoked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub life_model_influence: Option<crate::personal_intelligence_ports::LifeModelProductReceipt>,
}

impl std::fmt::Debug for SendMessageResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SendMessageResult")
            .field("reply", &"[REDACTED]")
            .field("status", &self.status)
            .field("blocker_count", &self.blockers.len())
            .field("tool_call_count", &self.tool_calls.len())
            .field("run_id", &self.run_id)
            .field(
                "provider_invocation_status",
                &self.provider_invocation_status,
            )
            .field("model_invoked", &self.model_invoked)
            .field("tool_invoked", &self.tool_invoked)
            .finish()
    }
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "owner=canonical-chat-work-ipc; Tauri command keeps explicit wire fields while internal runtime uses typed inputs"
)]
async fn send_message(
    operation_id: String,
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    provider_profile_id: Option<String>,
    reasoning_effort: Option<openlife_core::conversation::ReasoningEffort>,
    execution_mode: Option<openlife_core::task_runtime::WorkExecutionMode>,
    mode: Option<String>,
    task_id: Option<String>,
    run_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<SendMessageResult, String> {
    let selected_skill_id = selected_skill_id.as_deref().map(str::to_owned);
    match mode.as_deref().unwrap_or("chat") {
        "chat" => {
            main_chat_send::send_canonical_chat_with_state(
                operation_id,
                session_id,
                messages,
                selected_skill_id,
                provider_profile_id,
                reasoning_effort,
                state.inner(),
            )
            .await
        }
        "work" => {
            main_chat_send::send_canonical_work_with_state(
                canonical_work_runtime::CanonicalWorkInput {
                    turn_id: operation_id,
                    task_id: task_id.ok_or_else(|| "canonical_work_task_id_missing".to_string())?,
                    run_id: run_id.ok_or_else(|| "canonical_work_run_id_missing".to_string())?,
                    conversation_id: session_id,
                    messages,
                    selected_skill_id,
                    provider_profile_id,
                    reasoning_effort,
                    execution_mode: execution_mode.unwrap_or_default(),
                    revision_context: None,
                    stream: false,
                },
                state.inner(),
            )
            .await
        }
        _ => Err("invalid_main_chat_mode".into()),
    }
}

#[derive(serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StartStreamMessageArgs {
    operation_id: String,
    session_id: String,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    selected_skill_id: Option<String>,
    #[serde(default)]
    provider_profile_id: Option<String>,
    #[serde(default)]
    reasoning_effort: Option<openlife_core::conversation::ReasoningEffort>,
    #[serde(default)]
    execution_mode: Option<openlife_core::task_runtime::WorkExecutionMode>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    run_id: Option<String>,
}

impl std::fmt::Debug for StartStreamMessageArgs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StartStreamMessageArgs")
            .field("operation_id_present", &!self.operation_id.is_empty())
            .field("session_id", &self.session_id)
            .field("message_count", &self.messages.len())
            .field("messages", &"[REDACTED]")
            .field(
                "selected_skill_id_present",
                &self.selected_skill_id.is_some(),
            )
            .field(
                "provider_profile_id_present",
                &self.provider_profile_id.is_some(),
            )
            .field("reasoning_effort", &self.reasoning_effort)
            .field("execution_mode", &self.execution_mode)
            .field("mode", &self.mode)
            .field("task_id_present", &self.task_id.is_some())
            .field("run_id_present", &self.run_id.is_some())
            .finish()
    }
}

#[tauri::command]
async fn start_stream_message<R: tauri::Runtime>(
    args: StartStreamMessageArgs,
    app_handle: tauri::AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let StartStreamMessageArgs {
        operation_id,
        session_id,
        messages,
        selected_skill_id,
        provider_profile_id,
        reasoning_effort,
        execution_mode,
        mode,
        task_id,
        run_id,
    } = args;

    let selected_skill_id = selected_skill_id.as_deref().map(str::to_owned);
    let app_handle = app_handle.clone();
    let emit = move |event: &str, payload| {
        let _ = app_handle.emit(event, payload);
    };
    match mode.as_deref().unwrap_or("chat") {
        "chat" => {
            main_chat_streaming::start_canonical_chat_stream_with_state(
                canonical_chat_runtime::CanonicalChatInput {
                    turn_id: operation_id,
                    conversation_id: session_id,
                    messages,
                    selected_skill_id,
                    provider_profile_id,
                    reasoning_effort,
                    stream: true,
                },
                state.inner(),
                emit,
            )
            .await
        }
        "work" => {
            main_chat_streaming::start_canonical_work_stream_with_state(
                canonical_work_runtime::CanonicalWorkInput {
                    turn_id: operation_id,
                    task_id: task_id.ok_or_else(|| "canonical_work_task_id_missing".to_string())?,
                    run_id: run_id.ok_or_else(|| "canonical_work_run_id_missing".to_string())?,
                    conversation_id: session_id,
                    messages,
                    selected_skill_id,
                    provider_profile_id,
                    reasoning_effort,
                    execution_mode: execution_mode.unwrap_or_default(),
                    revision_context: None,
                    stream: true,
                },
                state.inner(),
                emit,
            )
            .await
        }
        _ => Err("invalid_main_chat_mode".into()),
    }
}

#[tauri::command]
async fn cancel_chat_turn(
    conversation_id: String,
    turn_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<canonical_chat_runtime::CancelCanonicalChatResult, String> {
    canonical_chat_runtime::cancel_canonical_chat(&conversation_id, &turn_id, state.inner()).await
}

#[tauri::command]
async fn stop_work_run(
    task_id: String,
    run_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<canonical_work_runtime::CanonicalWorkControlResult, String> {
    canonical_work_runtime::stop_canonical_work_run(&task_id, &run_id, state.inner()).await
}

#[tauri::command]
async fn retry_work_task(
    task_id: String,
    prior_run_id: String,
    new_run_id: String,
    new_turn_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<SendMessageResult, String> {
    canonical_work_runtime::retry_canonical_work_task(
        task_id,
        prior_run_id,
        new_run_id,
        new_turn_id,
        state.inner(),
    )
    .await
    .map(|output| output.result)
}

#[tauri::command]
async fn resume_work_task(
    task_id: String,
    prior_run_id: String,
    new_run_id: String,
    new_turn_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<SendMessageResult, String> {
    canonical_work_runtime::resume_canonical_work_task(
        task_id,
        prior_run_id,
        new_run_id,
        new_turn_id,
        state.inner(),
    )
    .await
    .map(|output| output.result)
}

#[tauri::command]
async fn revise_work_artifact(
    task_id: String,
    artifact_id: String,
    base_version: u64,
    instruction: String,
    new_run_id: String,
    new_turn_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<SendMessageResult, String> {
    canonical_work_runtime::revise_canonical_work_artifact(
        task_id,
        artifact_id,
        base_version,
        instruction,
        new_run_id,
        new_turn_id,
        state.inner(),
    )
    .await
    .map(|output| output.result)
}

#[tauri::command]
async fn pick_and_import_resources<R: tauri::Runtime>(
    import_operation_id: String,
    turn_operation_id: String,
    app_handle: tauri::AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<resource_commands::ResourceImportSelectionResult, String> {
    resource_commands::pick_and_import_resources(
        import_operation_id,
        turn_operation_id,
        app_handle,
        state.inner(),
    )
    .await
}

#[tauri::command]
async fn select_artifact_output_directory<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<commands::settings::ArtifactOutputDirectorySelection, errors::AppError> {
    commands::settings::select_artifact_output_directory(app_handle, state.inner()).await
}

#[tauri::command]
async fn export_artifact_result<R: tauri::Runtime>(
    artifact_id: String,
    version: u64,
    app_handle: tauri::AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<commands::artifact::ExportArtifactResult, String> {
    commands::artifact::export_artifact_result(app_handle, state.inner(), &artifact_id, version)
        .await
}

#[tauri::command]
async fn detach_resource_from_turn(
    operation_id: String,
    turn_operation_id: String,
    resource_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::resource::ResourceDetachReceipt, String> {
    resource_commands::detach_resource_from_turn(
        operation_id,
        turn_operation_id,
        resource_id,
        state.inner(),
    )
    .await
}

#[cfg(feature = "dev-extensions")]
fn start_dev_extension_background_workers(app_state: Arc<AppState>) {
    if app_state
        .persistence_coordinator
        .require_effects_allowed()
        .is_ok()
    {
        let maintenance_state = Arc::clone(&app_state);
        tauri::async_runtime::spawn(async move {
            match memory_gateway::run_memory_tier_maintenance_with_state(&maintenance_state).await {
                Ok((upgraded, downgraded)) => {
                    log::info!(
                        "[tier] development maintenance done: upgraded={} downgraded={}",
                        upgraded,
                        downgraded
                    );
                }
                Err(error) => {
                    log::warn!(
                        "[tier] development maintenance skipped or failed: {}",
                        error
                    );
                }
            }
            let interval = std::time::Duration::from_secs(600);
            loop {
                tokio::time::sleep(interval).await;
                match memory_gateway::run_memory_tier_maintenance_with_state(&maintenance_state)
                    .await
                {
                    Ok((upgraded, downgraded)) => {
                        log::info!(
                            "[tier] periodic development maintenance done: upgraded={} downgraded={}",
                            upgraded,
                            downgraded
                        );
                    }
                    Err(error) => {
                        log::warn!(
                            "[tier] periodic development maintenance skipped or failed: {}",
                            error
                        );
                    }
                }
            }
        });
    }
}

#[cfg(debug_assertions)]
fn runtime_dev_url() -> Option<tauri::Url> {
    let value = std::env::var("OPENLIFE_DEV_URL").ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let Ok(url) = tauri::Url::parse(trimmed) else {
        log::warn!("[setup] ignoring invalid OPENLIFE_DEV_URL value");
        return None;
    };
    if !matches!(url.scheme(), "http" | "https") {
        log::warn!("[setup] ignoring OPENLIFE_DEV_URL with unsupported scheme");
        return None;
    }
    if !matches!(url.host_str(), Some("127.0.0.1" | "localhost")) {
        log::warn!("[setup] ignoring OPENLIFE_DEV_URL with non-loopback host");
        return None;
    }
    Some(url)
}

#[cfg(not(debug_assertions))]
fn runtime_dev_url() -> Option<tauri::Url> {
    None
}

fn ensure_main_window_visible<R: tauri::Runtime, M: Manager<R>>(manager: &M) -> tauri::Result<()> {
    let main_window_config = manager
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == "main")
        .ok_or_else(|| anyhow::anyhow!("tauri config is missing the main window"))?;
    let dev_url = runtime_dev_url().and_then(|url| {
        if manager.config().build.dev_url.as_ref() == Some(&url) {
            Some(url)
        } else {
            log::warn!("[setup] ignoring OPENLIFE_DEV_URL that does not match tauri build.devUrl");
            None
        }
    });

    let window = if let Some(dev_url) = dev_url {
        if let Some(window) = manager.get_webview_window("main") {
            if window
                .url()
                .map(|current_url| current_url != dev_url)
                .unwrap_or(false)
            {
                window.navigate(dev_url)?;
            }
            window
        } else {
            let mut dev_window_config = main_window_config.clone();
            dev_window_config.url = tauri::WebviewUrl::External(dev_url);
            tauri::WebviewWindowBuilder::from_config(manager, &dev_window_config)?.build()?
        }
    } else if let Some(window) = manager.get_webview_window("main") {
        window
    } else {
        tauri::WebviewWindowBuilder::from_config(manager, main_window_config)?.build()?
    };

    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    Ok(())
}

fn validated_external_https_source(url: &str) -> Result<tauri::Url, String> {
    let parsed = tauri::Url::parse(url).map_err(|_| "external_source_url_invalid".to_string())?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("external_source_url_not_https".into());
    }
    Ok(parsed)
}

#[tauri::command]
fn open_external_https_source(url: String) -> Result<(), String> {
    let parsed = validated_external_https_source(&url)?;
    open::that(parsed.as_str()).map_err(|_| "external_source_open_failed".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = app_data_dir();
    let bootstrap = bootstrap::bootstrap(data_dir.clone());
    let app_state = bootstrap.state;
    let app_state_for_setup = app_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state.clone())
        .setup(move |app| {
            if app_state_for_setup
                .persistence_coordinator
                .startup_reconciliation_mutations_safe()
            {
                if let Err(error) = tauri::async_runtime::block_on(
                    bootstrap::reconcile_startup_canonical_outboxes(&app_state_for_setup),
                ) {
                    log::error!("[setup] canonical outbox reconciliation degraded: {error}");
                    app_state_for_setup
                        .persistence_coordinator
                        .degrade_globally("startup_canonical_outbox_reconciliation_failed");
                }
            }
            let proposal_backlog = if app_state_for_setup
                .persistence_coordinator
                .startup_reconciliation_mutations_safe()
            {
                match tauri::async_runtime::block_on(
                    bootstrap::reconcile_startup_proposal_projections(&app_state_for_setup),
                ) {
                    Ok(backlog) => backlog,
                    Err(error) => {
                        log::error!("[setup] Proposal projection reconciliation degraded: {error}");
                        app_state_for_setup
                            .persistence_coordinator
                            .degrade_globally("startup_proposal_projection_reconciliation_failed");
                        false
                    }
                }
            } else {
                false
            };
            // Product effects remain blocked in Initializing mode until every
            // startup reconciliation above has either succeeded or degraded
            // the coordinator. Seal is the one-way enable point.
            app_state_for_setup.persistence_coordinator.seal();
            if app_state_for_setup
                .persistence_coordinator
                .require_effects_allowed()
                .is_ok()
            {
                memory_gateway::start_canonical_outbox_background_worker(Arc::clone(
                    &app_state_for_setup,
                ));
            }
            if proposal_backlog
                && app_state_for_setup
                    .persistence_coordinator
                    .require_effects_allowed()
                    .is_ok()
            {
                let reconciliation_state = Arc::clone(&app_state_for_setup);
                tauri::async_runtime::spawn(async move {
                    bootstrap::drain_startup_proposal_projection_backlog(reconciliation_state)
                        .await;
                });
            }
            if let Err(e) = ensure_main_window_visible(app) {
                log::warn!("[setup] failed to show main window: {}", e);
                return Err(Box::new(e));
            }
            #[cfg(feature = "dev-extensions")]
            start_dev_extension_background_workers(app_state_for_setup.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            draft_legacy_lifemodel_migration,
            draft_lifemodel_v2_change,
            draft_lifemodel_v2_rollback,
            draft_lifemodel_v2_export,
            confirm_lifemodel_learning_candidate,
            delete_lifemodel_learning_candidate,
            reject_lifemodel_learning_candidate,
            pause_lifemodel_learning_suggestion_class,
            stage_lifemodel_learning_candidate,
            edit_lifemodel_learning_proposal,
            get_life_state_projection,
            get_life_model_view_model,
            get_review_center_view_model,
            get_memory_view_model,
            get_provider_privacy_boundary_summary,
            get_workbench_view_model,
            get_product_diagnostics_view_model,
            get_tool_permission_view_model,
            get_config,
            save_config,
            select_artifact_output_directory,
            recover_required_credential_access,
            revoke_tool_permission,
            list_main_chat_skills,
            get_main_chat_skill_detail,
            select_main_chat_skill,
            clear_main_chat_skill,
            list_main_chat_tool_candidates,
            open_external_https_source,
            open_artifact_result,
            export_artifact_result,
            accept_proposal,
            reject_proposal,
            request_artifact_undo,
            postpone_proposal,
            rollback_memory_asset,
            correct_memory,
            archive_memory,
            restore_memory,
            privacy_erase_memory_asset,
            send_message,
            start_stream_message,
            cancel_chat_turn,
            stop_work_run,
            retry_work_task,
            resume_work_task,
            revise_work_artifact,
            pick_and_import_resources,
            detach_resource_from_turn,
            submit_main_chat_task_steering,
            get_conversation_view_model,
            test_llm_connection,
            create_chat_session,
            create_project_from_directory,
            bind_project_directory,
            add_project_read_root,
            remove_project_read_root,
            update_project_name,
            archive_project,
            restore_project,
            delete_project,
            assign_conversation_project,
            select_new_conversation_project,
            set_conversation_memory_mode,
            rename_chat_session,
            archive_chat_session,
            restore_chat_session,
            delete_chat_session,
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| panic!("Tauri build failed: {}", e))
        .run(|app_handle, event| match event {
            tauri::RunEvent::Ready => {
                if let Err(e) = ensure_main_window_visible(app_handle) {
                    log::warn!("[runtime] failed to show main window: {}", e);
                }
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => {
                if let Err(e) = ensure_main_window_visible(app_handle) {
                    log::warn!("[runtime] failed to show main window: {}", e);
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod external_source_tests {
    use super::validated_external_https_source;

    #[test]
    fn external_source_requires_credential_free_https_url() {
        assert!(validated_external_https_source("https://example.com/source").is_ok());
        for rejected in [
            "http://example.com/source",
            "javascript:alert(1)",
            "file:///tmp/source",
            "https://user:secret@example.com/source",
            "not-a-url",
        ] {
            assert!(
                validated_external_https_source(rejected).is_err(),
                "URL should be rejected: {rejected}"
            );
        }
    }
}

#[cfg(test)]
mod release_surface_tests {
    fn command_name_after_attribute(source: &str, offset: usize) -> Option<&str> {
        let tail = &source[offset + concat!("#[", "tauri::command", "]").len()..];
        let function = tail.find("fn ")?;
        let name = &tail[function + 3..];
        let end =
            name.find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))?;
        Some(&name[..end])
    }

    #[test]
    fn every_tauri_command_is_registered_in_the_release_handler() {
        let command_attribute = concat!("#[", "tauri::command", "]");
        let lib = include_str!("lib.rs");
        let start = lib
            .find(".invoke_handler(tauri::generate_handler![")
            .expect("shipped Tauri handler start");
        let end = lib[start..]
            .find("])\n        .build(")
            .map(|offset| start + offset)
            .expect("shipped Tauri handler end");
        let handler = &lib[start..end];
        let sources = [
            ("lib.rs", lib),
            ("commands/artifact.rs", include_str!("commands/artifact.rs")),
            ("commands/chat.rs", include_str!("commands/chat.rs")),
            (
                "commands/life_model.rs",
                include_str!("commands/life_model.rs"),
            ),
            (
                "commands/main_chat_tools.rs",
                include_str!("commands/main_chat_tools.rs"),
            ),
            ("commands/memory.rs", include_str!("commands/memory.rs")),
            ("commands/proposal.rs", include_str!("commands/proposal.rs")),
            ("commands/settings.rs", include_str!("commands/settings.rs")),
            (
                "life_state_projection.rs",
                include_str!("life_state_projection.rs"),
            ),
            (
                "main_chat_steering.rs",
                include_str!("main_chat_steering.rs"),
            ),
            (
                "read_models/diagnostics.rs",
                include_str!("read_models/diagnostics.rs"),
            ),
            (
                "read_models/life_model.rs",
                include_str!("read_models/life_model.rs"),
            ),
            (
                "read_models/memory.rs",
                include_str!("read_models/memory.rs"),
            ),
            (
                "read_models/provider_privacy.rs",
                include_str!("read_models/provider_privacy.rs"),
            ),
            (
                "read_models/review_center.rs",
                include_str!("read_models/review_center.rs"),
            ),
            ("read_models/tasks.rs", include_str!("read_models/tasks.rs")),
        ];

        for (path, source) in sources {
            let mut offset = 0;
            for line in source.split_inclusive('\n') {
                if line.trim() == command_attribute {
                    let name = command_name_after_attribute(source, offset)
                        .unwrap_or_else(|| panic!("could not parse Tauri command in {path}"));
                    assert!(
                        handler.contains(&format!("{name},")),
                        "Tauri command {name} in {path} is not registered in the release handler"
                    );
                }
                offset += line.len();
            }
        }
    }

    #[test]
    fn shipped_handler_excludes_retired_product_surfaces() {
        let source = include_str!("lib.rs");
        let start = source
            .find(".invoke_handler(tauri::generate_handler![")
            .expect("shipped Tauri handler start");
        let end = source[start..]
            .find("])\n        .build(")
            .map(|offset| start + offset)
            .expect("shipped Tauri handler end");
        let handler = &source[start..end];

        for required in [
            "get_life_model_view_model",
            "draft_lifemodel_v2_change",
            "correct_memory",
            "archive_memory",
            "restore_memory",
        ] {
            assert!(
                handler.contains(required),
                "missing current command {required}"
            );
        }
        for retired in [
            "get_life_model,",
            "save_life_model,",
            "get_life_model_current_view",
            "builder_start",
            "builder_create_proposals",
            "get_model_4d_completion",
            "recommend_mcp_manifests",
            "create_snapshot",
            "restore_snapshot",
            "calibration_create_proposals",
            "save_feedback",
            "get_feedback_summary",
            "generate_evolution_report",
            "log_analytics_event",
            "get_proactive_suggestions",
            "select_markdown_memory_root",
            "get_markdown_memory_view_model",
            "draft_markdown_memory_file_proposal",
            "deactivate_markdown_memory_file_proposal",
            "draft_memory_stop_recall_proposal",
            "draft_memory_correction_proposal",
            "draft_memory_archive_proposal",
            "restore_archived_chunks",
        ] {
            assert!(
                !handler.contains(retired),
                "retired LifeModel command is still shipped: {retired}"
            );
        }
    }
}
