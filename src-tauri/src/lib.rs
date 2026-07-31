#[cfg(all(feature = "dev-extensions", not(debug_assertions)))]
compile_error!("dev-extensions are forbidden in non-debug OpenLife builds");

use openlife_core::llm::ChatMessage;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

pub mod a2a_server;
pub mod a2a_sidecar;
pub(crate) mod artifact_materializer;
pub mod bootstrap;
pub mod commands;
pub(crate) mod danger_action_confirmation;
pub mod errors;
#[allow(dead_code)]
pub(crate) mod life_model_materializer_guard;
pub(crate) mod life_model_write_gateway;
pub(crate) mod life_state_projection;
pub(crate) mod main_chat_agent_state_payload;
pub(crate) mod main_chat_cancellation;
#[allow(dead_code)]
pub(crate) mod main_chat_capability_eval;
#[allow(dead_code)]
pub(crate) mod main_chat_command_surface_eval;
pub(crate) mod main_chat_context_loader;
#[allow(dead_code)]
pub(crate) mod main_chat_eval_state;
#[allow(dead_code)]
pub(crate) mod main_chat_event_stream;
#[allow(dead_code)]
pub(crate) mod main_chat_final_gate;
#[allow(dead_code)]
pub(crate) mod main_chat_generation_support;
pub(crate) mod main_chat_hs_runtime;
#[allow(dead_code)]
pub(crate) mod main_chat_kernel;
#[allow(dead_code)]
pub(crate) mod main_chat_live_provider_harness;
pub(crate) mod main_chat_memory_proposals;
#[allow(dead_code)]
pub(crate) mod main_chat_preprocess;
pub(crate) mod main_chat_react_execution;
#[allow(dead_code)]
pub(crate) mod main_chat_react_runtime;
#[allow(dead_code)]
pub(crate) mod main_chat_react_tool_selection;
pub(crate) mod main_chat_replay_contract;
#[allow(dead_code)]
pub(crate) mod main_chat_runtime_facts;
#[allow(dead_code)]
pub(crate) mod main_chat_runtime_status;
pub(crate) mod main_chat_runtime_support;
pub(crate) mod main_chat_send;
pub(crate) mod main_chat_skills_tools;
pub(crate) mod main_chat_streaming;
pub(crate) mod main_chat_task_controls;
#[allow(dead_code)]
pub(crate) mod main_chat_turn_pipeline;
#[allow(dead_code)]
pub mod main_chat_turn_runtime;
#[allow(dead_code)]
pub(crate) mod memory_gateway;
pub(crate) mod persistence_coordinator;
pub(crate) mod product_agent_dto;
pub(crate) mod provider_network_consent;
pub(crate) mod provider_validation;
pub(crate) mod read_models;
pub(crate) mod resource_commands;
pub mod runtime_build_info;
pub mod scheduler_runner;
pub(crate) mod secret_store;
pub mod state;
pub(crate) mod state_projection;
pub mod storage;
pub(crate) mod terminal_owner_write_gateway;
pub(crate) mod tool_gateway_resources;
pub(crate) mod workspace_file_resolver;

#[cfg(test)]
mod main_chat_acceptance_test_support;

#[cfg(test)]
mod main_chat_live_provider_tests;

#[cfg(test)]
mod main_chat_command_surface_tests;

#[cfg(test)]
mod main_chat_capability_eval_tests;

#[cfg(test)]
mod main_chat_react_boundary_tests;

#[cfg(test)]
mod main_chat_react_unit_tests;

#[cfg(test)]
mod main_chat_hs_runtime_tests;

#[cfg(test)]
mod main_chat_task_control_tests;

#[cfg(test)]
mod main_chat_context_loader_tests;

#[cfg(test)]
mod main_chat_runtime_module_tests;

#[cfg(test)]
mod main_chat_runtime_facts_tests;

#[cfg(test)]
pub mod test_utils;

pub use state::AppState;

// Re-exports for test modules (imported as crate::...)
#[cfg(feature = "dev-extensions")]
use commands::a2a::{
    a2a_bridge_local, a2a_discover_agent, a2a_handle_task, a2a_local_agent_card,
    a2a_restart_sidecar, a2a_send_task, a2a_stop_sidecar,
};
use commands::agent::{
    delete_agent_run, get_agent_run, list_agent_runs, list_agent_runs_for_session,
    list_provider_transmission_history, restore_agent_run,
};
use commands::agent_runtime::{
    cancel_plan_execute_session, clear_main_chat_skill, create_plan_execute_session,
    execute_plan_execute_step, finalize_plan_execute_session, get_main_chat_skill_detail,
    get_plan_execute_session, list_main_chat_skills, list_main_chat_tool_candidates,
    list_plan_execute_sessions, review_plan_execute_session, select_main_chat_skill,
    skip_plan_execute_step, update_plan_execute_session_draft,
};

use commands::builder::{
    builder_create_proposals, builder_delete_session, builder_get_pending_signals,
    builder_list_unfinished, builder_start, builder_step, get_model_4d_completion,
    goal_capability_gap_analysis, goal_capability_gap_report, identity_goal_alignment_check,
    identity_goal_alignment_report,
};
use commands::calibration::{
    apply_calibration, calibration_create_proposals, generate_calibration_report,
    generate_micro_evolution_changes, mark_calibration_shown, run_micro_evolution,
    should_show_calibration,
};
use commands::chat::{
    create_chat_session, delete_chat_session, get_chat_history, list_chat_sessions,
    rename_chat_session, save_chat_message,
};
use commands::diagnostics::{
    check_ollama_status, get_policy_router_status, get_runtime_build_info, get_scheduler_config,
    get_system_diagnostics, set_scheduler_config,
};
use commands::execution::{check_tool_permission, list_tool_permissions, revoke_tool_permission};
#[cfg(feature = "dev-extensions")]
use commands::execution::{disable_plugin, enable_plugin, list_plugins, reload_plugins};
use commands::feedback::{
    apply_feedback_evolution, generate_evolution_report, get_feedback_summary, log_analytics_event,
    save_feedback,
};
pub use openlife_core::memory_cache::HotMemoryCache;
pub use openlife_core::memory_cache::SharedHotCache;
pub use openlife_core::privacy::PrivacyEngine;
// Hermes module removed: replaced by AgentRuntime
use commands::life_model::{get_life_model, get_life_model_current_view, save_life_model};
use commands::mcp::list_tool_manifests;
#[cfg(feature = "dev-extensions")]
use commands::mcp::{
    clear_mcp_audit_logs, list_mcp_audit_logs, list_mcp_servers, list_mcp_templates,
    list_mcp_tools, recommend_mcp_manifests, register_mcp_server, unregister_mcp_server,
};
use commands::memory::{
    archive_low_access_memories, cancel_memory_index_rebuild, count_memory_chunks,
    create_knowledge_note, get_hot_cache, get_memory_index_rebuild_progress, get_memory_tier_stats,
    list_archived_chunks, rebuild_memory_index, restore_archived_chunks,
    run_memory_tier_maintenance, search_memory, undo_explicit_memory,
};
use commands::metrics::{get_rollout_errors, get_rollout_metrics, get_rollout_summary};
use commands::proactive::get_proactive_suggestions;
use commands::proposal::{
    accept_proposal, batch_accept_low_risk_proposals, edit_proposal, get_memory_asset,
    get_memory_lifecycle_events, get_pending_proposals, list_memory_assets, list_proposals,
    postpone_proposal, rebuild_memory_materialized_view, reject_proposal, rollback_memory_asset,
};
use commands::router::get_model_router_status;
use commands::settings::{
    abandon_governed_data_import_recovery, export_all_data, get_config,
    get_danger_action_preflight, get_governed_data_import_status, get_last_model_error,
    get_privacy_policy, import_all_data, recover_required_credential_access, save_config,
    set_privacy_policy, test_llm_connection,
};
#[cfg(feature = "dev-extensions")]
use commands::settings::{cleanup_mcp_audit_logs, export_mcp_audit_logs, rotate_mcp_audit_key};
use commands::state::{get_daily_goals, get_state_alerts, get_state_history};
use commands::version::{create_snapshot, diff_snapshots, list_snapshots, restore_snapshot};
use life_state_projection::get_life_state_projection;
use main_chat_event_stream::{get_main_chat_agent_state_snapshot, list_main_chat_agent_events};
use main_chat_memory_proposals::draft_edit_memory_proposal;
use main_chat_runtime_status::get_main_chat_runtime_status;
use main_chat_task_controls::{
    cancel_main_chat_agent_task, get_main_chat_agent_task_detail, get_main_chat_agent_task_state,
    list_main_chat_agent_tasks, refresh_main_chat_agent_task_context, resume_main_chat_agent_task,
    retry_main_chat_agent_action,
};
use read_models::life_model::get_life_model_view_model;
use read_models::memory::get_memory_view_model;
use read_models::provider_privacy::get_provider_privacy_boundary_summary;
use read_models::review_center::get_review_center_view_model;
use read_models::tasks::{get_tasks_view_model, get_workspace_view_model};
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
    pub react_trace: Option<crate::product_agent_dto::ProductReactActionTrace>,
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
            .field("react_trace_present", &self.react_trace.is_some())
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
    pub reasoning_trace: openlife_core::agent::ReasoningTrace,
    pub tool_calls: Vec<ToolCallResult>,
    pub run_id: Option<String>,
    pub agent_ingress: Option<openlife_core::agent::main_chat_agent_v1::AgentIngressDecision>,
    #[serde(serialize_with = "crate::product_agent_dto::serialize_product_agent_state")]
    pub agent_state:
        Option<openlife_core::agent::main_chat_runtime_contract::MainChatAgentStateSnapshot>,
    #[serde(serialize_with = "crate::product_agent_dto::serialize_product_execution_transcript")]
    pub execution_transcript:
        Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>,
    pub legacy_fallback_used: bool,
    pub legacy_runtime_invoked: bool,
    pub provider_invocation_status: crate::main_chat_turn_runtime::ProviderInvocationState,
    pub model_invoked: bool,
    pub tool_invoked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_terminal: Option<crate::main_chat_turn_runtime::OpenLifeTurnTerminal>,
}

impl std::fmt::Debug for SendMessageResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SendMessageResult")
            .field("reply", &"[REDACTED]")
            .field("status", &self.status)
            .field("blocker_count", &self.blockers.len())
            .field("reasoning_trace", &"[REDACTED]")
            .field("tool_call_count", &self.tool_calls.len())
            .field("run_id", &self.run_id)
            .field("agent_ingress_present", &self.agent_ingress.is_some())
            .field("agent_state_present", &self.agent_state.is_some())
            .field(
                "execution_transcript_count",
                &self.execution_transcript.len(),
            )
            .field("legacy_fallback_used", &self.legacy_fallback_used)
            .field("legacy_runtime_invoked", &self.legacy_runtime_invoked)
            .field(
                "provider_invocation_status",
                &self.provider_invocation_status,
            )
            .field("model_invoked", &self.model_invoked)
            .field("tool_invoked", &self.tool_invoked)
            .field("turn_terminal_present", &self.turn_terminal.is_some())
            .finish()
    }
}

#[derive(serde::Serialize)]
pub struct BuilderCompletion {
    pub identity: f32,
    pub goals: f32,
    pub capabilities: f32,
    pub state: f32,
    pub overall: f32,
    pub lowest_dimension: Option<String>,
}

#[derive(serde::Serialize)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size_mb: u64,
}

#[derive(serde::Serialize)]
pub struct SystemDiagnostics {
    pub persistence_health: crate::persistence_coordinator::PersistenceHealthSnapshot,
    pub policy_router: crate::commands::diagnostics::PolicyRouterStatus,
    pub mcp_server_count: usize,
    pub mcp_tool_count: usize,
    pub mcp_recent_audit_count: usize,
    pub mcp_recent_pii_count: usize,
    pub memory_chunk_count: usize,
    pub vector_corrupt_embedding_count: usize,
    pub vector_unknown_profile_count: usize,
    pub vector_profile_dimension_mismatch_count: usize,
    pub unfinished_builder_sessions: usize,
    pub pending_builder_review_sessions: usize,
    pub ollama_service_online: bool,
    pub ollama_online: bool,
    pub local_model: String,
    pub resolved_local_model: Option<String>,
    pub prefer_local_model: bool,
    pub cloud_api_configured: bool,
    pub cloud_provider: String,
    pub cloud_api_validated: bool,
    pub cloud_api_last_error: Option<String>,
    pub cloud_api_validation_status: String,
    pub cloud_api_validated_at: Option<String>,
    pub cloud_api_failed_at: Option<String>,
    pub cloud_api_validation_source: Option<String>,
    pub chat_ready: bool,
    pub readiness_issues: Vec<String>,
    pub data_dir: String,
    pub active_data_dir: String,
    pub database_status: String,
    pub startup_warnings: Vec<String>,
    pub snapshot_count: usize,
    pub life_model_ready: bool,
    pub app_version: String,
    pub model_empty: bool,
    pub chat_session_count: usize,
    pub usage_ready: bool,
    pub usage_readiness_issues: Vec<String>,
    pub builder_completion: BuilderCompletion,
    pub ollama_models: Vec<OllamaModelInfo>,
    pub agent_run_count: usize,
    pub agent_run_store_status: String,
    pub pending_proposal_count: usize,
    pub high_risk_pending_proposal_count: usize,
    pub proposal_store_status: String,
    pub runtime_build_info: runtime_build_info::RuntimeBuildInfo,
    pub runtime_route_evidence: main_chat_runtime_facts::RuntimeRouteEvidence,
}

#[tauri::command]
async fn send_message(
    operation_id: String,
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<SendMessageResult, String> {
    let selected_skill_id = selected_skill_id.as_deref().map(str::to_owned);
    main_chat_send::send_message_with_operation_state(
        operation_id,
        session_id,
        messages,
        selected_skill_id,
        state.inner(),
    )
    .await
}

#[derive(serde::Deserialize, Clone)]
struct StartStreamMessageArgs {
    operation_id: String,
    session_id: String,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    selected_skill_id: Option<String>,
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
            .finish()
    }
}

#[tauri::command]
async fn start_stream_message<R: tauri::Runtime>(
    args: Option<StartStreamMessageArgs>,
    operation_id: Option<String>,
    session_id: Option<String>,
    messages: Option<Vec<ChatMessage>>,
    selected_skill_id: Option<String>,
    app_handle: tauri::AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let (operation_id, session_id, messages, selected_skill_id) = if let Some(args) = args {
        (
            args.operation_id,
            args.session_id,
            args.messages,
            args.selected_skill_id,
        )
    } else {
        (
            operation_id.ok_or_else(|| "start_stream_message 缺少 operation_id".to_string())?,
            session_id.ok_or_else(|| "start_stream_message 缺少 session_id".to_string())?,
            messages.ok_or_else(|| "start_stream_message 缺少 messages".to_string())?,
            selected_skill_id,
        )
    };

    let selected_skill_id = selected_skill_id.as_deref().map(str::to_owned);
    let app_handle = app_handle.clone();
    main_chat_streaming::start_stream_message_with_operation_state(
        operation_id,
        session_id,
        messages,
        selected_skill_id,
        state.inner(),
        move |event, payload| {
            let _ = app_handle.emit(event, payload);
        },
    )
    .await
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
async fn cancel_resource_import(
    operation_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<bool, String> {
    resource_commands::cancel_resource_import(&operation_id, state.inner())
}

#[tauri::command]
fn get_resource_import_status(
    operation_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<resource_commands::ResourceImportStatus, String> {
    resource_commands::get_resource_import_status(&operation_id, state.inner())
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

#[tauri::command]
#[cfg(feature = "dev-extensions")]
async fn execute_tool_call(
    name: String,
    arguments: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<ToolCallResult, String> {
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| error.to_string())?;
    let resources = crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_dev_command(
        state.inner(),
    )
    .await?;
    let safe_paths = resources.shared.safe_paths.clone();

    // Create an AgentRun for direct tool execution audit trail
    let mut run = openlife_core::agent::AgentRun::new_tool_execution_run(&name);
    let run_id = run.id.clone();

    let tool_gateway = openlife_core::agent::ToolGateway::from_executor_config(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let ctx = openlife_core::agent::ActionExecutionContext::new(
        &resources.shared.registry,
        &resources.shared.permission_store,
        &resources.shared.audit_store,
        &resources.shared.privacy_engine,
        &safe_paths,
    );
    let ctx = ctx
        .with_tool_audit_persistence_observer(resources.shared.persistence_coordinator.as_ref())
        .with_durable_store_failure_observer(resources.shared.persistence_coordinator.as_ref())
        .with_agent_run_store(&resources.agent_run_store);

    let request = openlife_core::agent::AgentActionRequest {
        action_type: "mcp_tool".to_string(),
        target: name.clone(),
        input: serde_json::json!({ "arguments": arguments }),
        source_run_id: Some(run_id.clone()),
        step_index: 0,
    };

    let result = tool_gateway
        .execute(request, &ctx)
        .await
        .map_err(|e| e.to_string())?;

    // Persist the AgentRun
    run.actions.push(result.action.clone());
    run.observations.push(result.observation.clone());
    run.status = match result.status {
        openlife_core::agent::ActionExecutionStatus::Succeeded => {
            openlife_core::agent::AgentRunStatus::Completed
        }
        _ => openlife_core::agent::AgentRunStatus::Failed,
    };
    run.finished_at = Some(chrono::Utc::now());

    let _ = resources.agent_run_store.create_run(&run);

    let product_projection =
        crate::product_agent_dto::VerifiedProductToolCallProjection::from_bound_action(
            &result.action,
            &result.execution_receipt,
            &run_id,
        );

    let tool_result = ToolCallResult {
        name: name.clone(),
        arguments: arguments.clone(),
        sanitized_arguments: Some(arguments),
        success: result.status == openlife_core::agent::ActionExecutionStatus::Succeeded,
        output: result
            .action
            .output
            .as_ref()
            .and_then(|o| o.get("text").and_then(|t| t.as_str()).map(String::from)),
        error: result.action.error.clone(),
        permission_level: result
            .action
            .tool_scope
            .as_ref()
            .map(|s| s.risk_level.clone())
            .unwrap_or_else(|| "medium".into()),
        status: match result.status {
            openlife_core::agent::ActionExecutionStatus::Succeeded => ToolCallStatus::Success,
            openlife_core::agent::ActionExecutionStatus::Failed => ToolCallStatus::Error,
            openlife_core::agent::ActionExecutionStatus::Blocked => ToolCallStatus::Blocked,
            openlife_core::agent::ActionExecutionStatus::NeedsConfirmation => {
                ToolCallStatus::NeedsConfirmation
            }
        },
        requires_confirmation: result.status
            == openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
        pii_found: false,
        privacy_warnings: vec![],
        action_id: Some(result.action.id),
        run_id: Some(run_id),
        permission_decision: result.action.permission_decision,
        react_trace: result
            .action
            .react_trace
            .map(crate::product_agent_dto::ProductReactActionTrace::from_transient_trace),
        execution_receipt: Some(result.execution_receipt),
        product_projection,
    };

    Ok(tool_result)
}
#[cfg(feature = "dev-extensions")]
#[tauri::command]
async fn inspect_mcp_call(
    name: String,
    arguments: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::mcp::McpArgumentInspection, String> {
    let reg = state.mcp_registry.lock().await;
    Ok(reg.inspect_call_arguments(&name, &arguments))
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
    scheduler_runner::start_scheduler_runner(app_state);
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
            if let Err(error) = tauri::async_runtime::block_on(
                bootstrap::reconcile_startup_orphaned_main_chat_runs(&app_state_for_setup),
            ) {
                log::error!("[setup] orphan Main Chat reconciliation degraded: {error}");
            }
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
            if app_state_for_setup
                .persistence_coordinator
                .startup_reconciliation_mutations_safe()
            {
                if let Err(error) = tauri::async_runtime::block_on(
                    bootstrap::reconcile_startup_terminal_owner_successors(&app_state_for_setup),
                ) {
                    log::error!("[setup] terminal-owner reconciliation degraded: {error}");
                    app_state_for_setup
                        .persistence_coordinator
                        .degrade_globally("startup_terminal_owner_reconciliation_failed");
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
                if let Err(error) = tauri::async_runtime::block_on(
                    state_projection::reconcile_state_store_lifemodel_projection(
                        &app_state_for_setup,
                    ),
                ) {
                    // StateStore remains the canonical product read owner. A
                    // failed YAML compatibility projection is explicitly
                    // degraded and retryable; it must not trigger a temp-store
                    // fallback or misreport the canonical effect as failed.
                    log::warn!("[setup] StateStore compatibility projection degraded: {error}");
                }
            }
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
            {
                if std::env::var("OPENLIFE_DEV_AUTOSTART_A2A").as_deref() == Ok("1") {
                    if let Err(reason) = a2a_server::require_authenticated_dev_a2a_opt_in() {
                        log::warn!(
                            "[setup] refusing A2A autostart without explicit pairing: {}",
                            reason
                        );
                    } else {
                        log::info!("[setup] launching explicitly enabled development A2A sidecar");
                        let a2a_sidecar = app_state_for_setup.a2a_sidecar.clone();
                        tauri::async_runtime::spawn(async move {
                            let sidecar = a2a_sidecar.lock().await.clone();
                            if let Err(e) = sidecar.start().await {
                                log::warn!("[setup] development a2a sidecar start failed: {}", e);
                            }
                        });
                    }
                }
            }
            #[cfg(feature = "dev-extensions")]
            start_dev_extension_background_workers(app_state_for_setup.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_life_model,
            get_life_model_current_view,
            get_life_state_projection,
            get_life_model_view_model,
            get_review_center_view_model,
            get_memory_view_model,
            get_provider_privacy_boundary_summary,
            get_tasks_view_model,
            get_workspace_view_model,
            save_life_model,
            get_config,
            save_config,
            recover_required_credential_access,
            get_agent_run,
            list_agent_runs,
            list_provider_transmission_history,
            list_agent_runs_for_session,
            delete_agent_run,
            restore_agent_run,
            get_main_chat_runtime_status,
            list_main_chat_skills,
            get_main_chat_skill_detail,
            select_main_chat_skill,
            clear_main_chat_skill,
            list_main_chat_tool_candidates,
            create_plan_execute_session,
            get_plan_execute_session,
            list_plan_execute_sessions,
            update_plan_execute_session_draft,
            finalize_plan_execute_session,
            cancel_plan_execute_session,
            review_plan_execute_session,
            execute_plan_execute_step,
            skip_plan_execute_step,
            get_pending_proposals,
            list_proposals,
            batch_accept_low_risk_proposals,
            accept_proposal,
            reject_proposal,
            edit_proposal,
            draft_edit_memory_proposal,
            postpone_proposal,
            rollback_memory_asset,
            list_memory_assets,
            get_memory_asset,
            get_memory_lifecycle_events,
            rebuild_memory_materialized_view,
            send_message,
            start_stream_message,
            pick_and_import_resources,
            cancel_resource_import,
            get_resource_import_status,
            detach_resource_from_turn,
            list_main_chat_agent_events,
            get_main_chat_agent_state_snapshot,
            list_main_chat_agent_tasks,
            get_main_chat_agent_task_detail,
            refresh_main_chat_agent_task_context,
            get_main_chat_agent_task_state,
            resume_main_chat_agent_task,
            cancel_main_chat_agent_task,
            retry_main_chat_agent_action,
            get_chat_history,
            save_chat_message,
            #[cfg(feature = "dev-extensions")]
            execute_tool_call,
            #[cfg(feature = "dev-extensions")]
            inspect_mcp_call,
            #[cfg(feature = "dev-extensions")]
            register_mcp_server,
            #[cfg(feature = "dev-extensions")]
            unregister_mcp_server,
            #[cfg(feature = "dev-extensions")]
            list_mcp_servers,
            #[cfg(feature = "dev-extensions")]
            list_mcp_tools,
            #[cfg(feature = "dev-extensions")]
            list_mcp_templates,
            #[cfg(feature = "dev-extensions")]
            recommend_mcp_manifests,
            list_tool_manifests,
            #[cfg(feature = "dev-extensions")]
            list_mcp_audit_logs,
            #[cfg(feature = "dev-extensions")]
            clear_mcp_audit_logs,
            get_system_diagnostics,
            get_runtime_build_info,
            check_ollama_status,
            get_policy_router_status,
            get_model_router_status,
            get_scheduler_config,
            set_scheduler_config,
            create_snapshot,
            list_snapshots,
            restore_snapshot,
            diff_snapshots,
            save_feedback,
            get_feedback_summary,
            apply_feedback_evolution,
            generate_evolution_report,
            run_memory_tier_maintenance,
            count_memory_chunks,
            log_analytics_event,
            create_knowledge_note,
            search_memory,
            undo_explicit_memory,
            #[cfg(feature = "dev-extensions")]
            a2a_discover_agent,
            #[cfg(feature = "dev-extensions")]
            a2a_send_task,
            #[cfg(feature = "dev-extensions")]
            a2a_local_agent_card,
            #[cfg(feature = "dev-extensions")]
            a2a_handle_task,
            #[cfg(feature = "dev-extensions")]
            a2a_bridge_local,
            #[cfg(feature = "dev-extensions")]
            a2a_restart_sidecar,
            #[cfg(feature = "dev-extensions")]
            a2a_stop_sidecar,
            builder_start,
            builder_step,
            builder_list_unfinished,
            builder_delete_session,
            builder_get_pending_signals,
            builder_create_proposals,
            get_model_4d_completion,
            goal_capability_gap_analysis,
            goal_capability_gap_report,
            identity_goal_alignment_check,
            identity_goal_alignment_report,
            export_all_data,
            get_danger_action_preflight,
            import_all_data,
            abandon_governed_data_import_recovery,
            get_governed_data_import_status,
            test_llm_connection,
            get_last_model_error,
            list_chat_sessions,
            create_chat_session,
            rename_chat_session,
            delete_chat_session,
            get_state_history,
            get_state_alerts,
            get_daily_goals,
            run_micro_evolution,
            generate_calibration_report,
            generate_micro_evolution_changes,
            apply_calibration,
            calibration_create_proposals,
            should_show_calibration,
            mark_calibration_shown,
            get_hot_cache,
            archive_low_access_memories,
            restore_archived_chunks,
            list_archived_chunks,
            get_memory_tier_stats,
            rebuild_memory_index,
            get_memory_index_rebuild_progress,
            cancel_memory_index_rebuild,
            #[cfg(feature = "dev-extensions")]
            export_mcp_audit_logs,
            #[cfg(feature = "dev-extensions")]
            cleanup_mcp_audit_logs,
            #[cfg(feature = "dev-extensions")]
            rotate_mcp_audit_key,
            get_privacy_policy,
            set_privacy_policy,
            get_rollout_metrics,
            get_rollout_summary,
            get_rollout_errors,
            get_proactive_suggestions,
            list_tool_permissions,
            revoke_tool_permission,
            check_tool_permission,
            #[cfg(feature = "dev-extensions")]
            list_plugins,
            #[cfg(feature = "dev-extensions")]
            reload_plugins,
            #[cfg(feature = "dev-extensions")]
            enable_plugin,
            #[cfg(feature = "dev-extensions")]
            disable_plugin,
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

/// Builds focused real shipped-command IPC handlers in the command owner
/// module, where Tauri's generated command macros are natively scoped.
/// Keeping each handler focused prevents the generated test dispatcher for
/// unrelated commands from sharing the invoked command's worker stack. These
/// remain after `run` so source guards cannot mistake them for the shipped
/// command handler.
#[cfg(test)]
fn main_chat_send_command_surface_test_handler<R: tauri::Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![send_message]
}

#[cfg(test)]
fn main_chat_stream_command_surface_test_handler<R: tauri::Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![start_stream_message]
}

#[cfg(test)]
fn main_chat_get_agent_run_command_surface_test_handler<R: tauri::Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![get_agent_run]
}
