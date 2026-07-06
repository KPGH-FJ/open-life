use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::router::RouterStatus;
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

use crate::legacy_write_convergence::{
    ensure_lifemodel_materializer_caller_restriction, LifeModelMaterializerCallerContext,
};

pub mod a2a_server;
pub mod a2a_sidecar;
pub mod bootstrap;
pub mod commands;
pub mod errors;
pub(crate) mod legacy_write_convergence;
#[allow(dead_code)]
pub(crate) mod main_chat_agent_beta_v1_default_experience;
#[allow(dead_code)]
pub(crate) mod main_chat_agent_beta_v1_readiness;
#[allow(dead_code)]
pub(crate) mod main_chat_agent_beta_v1_real_tasks;
#[allow(dead_code)]
pub(crate) mod main_chat_agent_productization_eval;
#[allow(dead_code)]
pub(crate) mod main_chat_agent_stage1_dogfood;
pub(crate) mod main_chat_agent_stage2_readiness;
pub(crate) mod main_chat_agent_state_payload;
#[allow(dead_code)]
pub(crate) mod main_chat_capability_eval;
#[allow(dead_code)]
pub(crate) mod main_chat_command_surface_eval;
pub(crate) mod main_chat_context_loader;
pub(crate) mod main_chat_conversation_updates;
#[allow(dead_code)]
pub(crate) mod main_chat_eval_state;
pub(crate) mod main_chat_event_stream;
#[allow(dead_code)]
pub(crate) mod main_chat_final_gate;
pub(crate) mod main_chat_generation_support;
pub(crate) mod main_chat_hs_runtime;
#[allow(dead_code)]
pub(crate) mod main_chat_kernel;
pub(crate) mod main_chat_legacy_agent_loop;
#[allow(dead_code)]
pub(crate) mod main_chat_live_productization_eval;
pub(crate) mod main_chat_live_provider_harness;
#[allow(dead_code)]
pub(crate) mod main_chat_memory_lifecycle_eval;
#[allow(dead_code)]
pub(crate) mod main_chat_plan_interaction_eval;
pub(crate) mod main_chat_preprocess;
pub(crate) mod main_chat_product_maturity_v2_final_readiness;
pub(crate) mod main_chat_proposal_support;
pub(crate) mod main_chat_react_execution;
pub(crate) mod main_chat_react_runtime;
pub(crate) mod main_chat_react_tool_selection;
pub(crate) mod main_chat_route_preview;
#[allow(dead_code)]
pub(crate) mod main_chat_runtime_facts;
pub(crate) mod main_chat_runtime_status;
pub(crate) mod main_chat_runtime_support;
pub(crate) mod main_chat_send;
pub(crate) mod main_chat_skills_tools;
pub(crate) mod main_chat_stage3_execution_ux;
pub(crate) mod main_chat_stage4_memory_knowledge;
pub(crate) mod main_chat_stage5_release_debug;
#[allow(dead_code)]
pub(crate) mod main_chat_step6_product_acceptance;
pub(crate) mod main_chat_strategy;
pub(crate) mod main_chat_streaming;
#[allow(dead_code)]
pub(crate) mod main_chat_task_continuity_eval;
pub(crate) mod main_chat_task_controls;
pub(crate) mod main_chat_tool_loop;
pub(crate) mod main_chat_turn_pipeline;
pub(crate) mod provider_validation;
pub mod runtime_build_info;
pub mod scheduler_runner;
pub mod state;
pub mod storage;
pub(crate) mod workspace_file_resolver;

#[cfg(test)]
mod main_chat_final_acceptance_tests;

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
mod single_system_authority_tests;

#[cfg(test)]
mod legacy_surface_tests;

#[cfg(test)]
mod main_chat_agent_productization_tests;

#[cfg(test)]
mod main_chat_agent_stage1_dogfood_tests;

#[cfg(test)]
mod main_chat_agent_stage2_readiness_tests;

#[cfg(test)]
mod main_chat_stage3_execution_ux_tests;

#[cfg(test)]
mod main_chat_stage4_memory_knowledge_tests;

#[cfg(test)]
mod main_chat_stage5_release_debug_tests;

#[cfg(test)]
mod main_chat_step6_product_acceptance_tests;

#[cfg(test)]
mod main_chat_event_stream_tests;

#[cfg(test)]
mod main_chat_runtime_facts_tests;

#[cfg(test)]
pub mod test_utils;

pub use state::AppState;

// Re-exports for test modules (imported as crate::...)
use commands::a2a::{
    a2a_bridge_local, a2a_discover_agent, a2a_handle_task, a2a_local_agent_card,
    a2a_restart_sidecar, a2a_send_task, a2a_stop_sidecar,
};
use commands::agent::{
    delete_agent_run, get_agent_run, list_agent_runs, list_agent_runs_for_session,
    list_provider_transmission_history, replay_agent_action, restore_agent_run,
};
use commands::agent_runtime::{
    cancel_plan_execute_session, check_controlled_chat_cutover_candidate_promotion_readiness,
    check_controlled_chat_cutover_readiness, check_controlled_chat_migration_implementation_gate,
    check_controlled_chat_pilot_eligibility, check_controlled_pilot_promotion_readiness,
    check_runtime_migration_gate, clear_main_chat_skill, create_plan_execute_session,
    draft_controlled_chat_migration_plan, execute_plan_execute_step, finalize_plan_execute_session,
    get_controlled_chat_cutover_candidate_review_summary,
    get_controlled_chat_migration_review_decision_summary,
    get_controlled_chat_migration_shadow_review_summary,
    get_controlled_pilot_promotion_evidence_summary, get_main_chat_skill_detail,
    get_plan_execute_session, get_react_beta_execution_status,
    get_runtime_strategy_registry_status, list_main_chat_skills, list_main_chat_tool_candidates,
    list_plan_execute_sessions, prepare_main_chat_agent_stage1_browser_dogfood_state,
    prepare_main_chat_step6_live_provider_eval_state,
    record_controlled_chat_cutover_candidate_review_decision,
    record_controlled_chat_migration_review_decision,
    record_controlled_chat_migration_shadow_review_decision,
    record_controlled_pilot_promotion_evidence, review_plan_execute_session,
    run_controlled_chat_cutover_candidate, run_controlled_chat_migration_shadow_run,
    run_main_chat_agent_beta_v1_readiness_gate, run_main_chat_agent_execution_v1_eval_gate,
    run_main_chat_agent_execution_v1_final_acceptance_gate,
    run_main_chat_agent_product_maturity_v2_event_gate,
    run_main_chat_agent_product_maturity_v2_final_readiness_gate,
    run_main_chat_agent_product_maturity_v2_plan_gate,
    run_main_chat_agent_product_maturity_v2_skills_gate,
    run_main_chat_agent_productization_v1_gate, run_main_chat_agent_stage1_dogfood_gate,
    run_main_chat_agent_stage2_readiness_gate, run_main_chat_agent_step6_product_acceptance_gate,
    run_main_chat_capability_eval_gate, run_main_chat_external_live_productization_gate,
    run_main_chat_stage3_execution_ux_report, run_multi_strategy_agent_preview,
    select_main_chat_skill, set_main_chat_agent_stage1_browser_network_policy,
    set_main_chat_agent_stage1_browser_scripted_response,
    set_main_chat_agent_stage1_browser_web_fixture_output, skip_plan_execute_step,
    update_plan_execute_session_draft, validate_main_chat_agent_stage2_manual_dogfood_artifact,
};

use commands::builder::{
    builder_apply_signals, builder_create_proposals, builder_delete_session,
    builder_get_pending_signals, builder_list_unfinished, builder_start, builder_step,
    get_model_4d_completion, goal_capability_gap_analysis, goal_capability_gap_report,
    identity_goal_alignment_check, identity_goal_alignment_report,
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
    check_ollama_status, get_router_status, get_runtime_build_info, get_scheduler_config,
    get_system_diagnostics, set_scheduler_config,
};
use commands::execution::{
    check_tool_permission, disable_plugin, enable_plugin, get_skill_run_status,
    get_skill_runtime_status, grant_tool_permission, list_plugins, list_skills,
    list_tool_permissions, reload_plugins, revoke_tool_permission, run_skill,
};
use commands::feedback::{
    apply_feedback_evolution, generate_evolution_report, get_feedback_summary, log_analytics_event,
    save_feedback,
};
pub use openlife_core::memory_cache::HotMemoryCache;
pub use openlife_core::memory_cache::SharedHotCache;
pub use openlife_core::privacy::PrivacyEngine;
// Hermes module removed: replaced by AgentRuntime
use commands::life_model::{get_life_model, get_life_model_current_view, save_life_model};
use commands::mcp::{
    clear_mcp_audit_logs, list_mcp_audit_logs, list_mcp_servers, list_mcp_templates,
    list_mcp_tools, list_tool_manifests, recommend_mcp_manifests, register_mcp_server,
    unregister_mcp_server,
};
use commands::memory::{
    archive_low_access_memories, count_memory_chunks, get_hot_cache, get_memory_tier_stats,
    index_memory_chunk, list_archived_chunks, rebuild_memory_index, restore_archived_chunks,
    run_memory_tier_maintenance, search_memory,
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
    cleanup_mcp_audit_logs, export_all_data, export_mcp_audit_logs, get_config,
    get_danger_action_preflight, get_last_model_error, get_privacy_policy, import_all_data,
    rotate_mcp_audit_key, save_config, set_privacy_policy, test_api_key, test_llm_connection,
};
use commands::state::{
    add_daily_goal, delete_daily_goal, get_daily_goals, get_state_alerts, get_state_history,
    record_state, toggle_daily_goal, update_daily_goal,
};
use commands::version::{create_snapshot, diff_snapshots, list_snapshots, restore_snapshot};
use main_chat_event_stream::{get_main_chat_agent_state_snapshot, list_main_chat_agent_events};
use main_chat_runtime_status::get_main_chat_runtime_status;
use main_chat_stage4_memory_knowledge::{
    confirm_managed_knowledge_write, create_managed_knowledge_write_draft,
    draft_edit_memory_proposal, list_stage4_knowledge_asset_inventory,
    rollback_managed_knowledge_write, run_main_chat_stage4_memory_knowledge_report,
};
use main_chat_stage5_release_debug::{
    create_main_chat_internal_issue_report, delete_main_chat_debug_bundle,
    delete_main_chat_internal_issue_report, evaluate_main_chat_stage5_release_debug_preflight,
    export_main_chat_agent_debug_bundle, get_main_chat_debug_bundle,
    get_main_chat_internal_issue_report, list_main_chat_debug_bundles,
    list_main_chat_internal_issue_reports, run_main_chat_stage5_release_debug_report,
};
use main_chat_task_controls::{
    cancel_main_chat_agent_task, get_main_chat_agent_task_detail, get_main_chat_agent_task_state,
    list_main_chat_agent_tasks, refresh_main_chat_agent_task_context, resume_main_chat_agent_task,
    retry_main_chat_agent_action,
};
use storage::app_data_dir;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Success,
    Error,
    Pending,
    Blocked,
    NeedsConfirmation,
}

#[derive(Clone, serde::Serialize)]
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
    pub react_trace: Option<openlife_core::agent::ReactActionTraceEnvelope>,
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
    pub agent_state:
        Option<openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentStateSnapshot>,
    pub execution_transcript:
        Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>,
    pub legacy_fallback_used: bool,
    pub legacy_runtime_invoked: bool,
    pub model_invoked: bool,
    pub tool_invoked: bool,
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
    pub router: RouterStatus,
    pub mcp_server_count: usize,
    pub mcp_tool_count: usize,
    pub mcp_recent_audit_count: usize,
    pub mcp_recent_pii_count: usize,
    pub memory_chunk_count: usize,
    pub vector_corrupt_embedding_count: usize,
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

pub(crate) async fn persist_life_model(
    state: &Arc<AppState>,
    mut life_model: LifeModel,
    create_daily_snapshot: bool,
    caller_context: LifeModelMaterializerCallerContext,
) -> Result<LifeModel, String> {
    ensure_lifemodel_materializer_caller_restriction(&caller_context, "persist_life_model")?;
    let previous_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().ok()
    };
    openlife_core::versioning::prepare_model_for_save(previous_model.as_ref(), &mut life_model);
    {
        let manager = state.life_model_manager.lock().await;
        manager.save(&life_model).map_err(|e| e.to_string())?;
    }
    if create_daily_snapshot {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let should_snapshot = {
            let vm = state.version_manager.lock().await;
            !vm.has_snapshot_tag_on_date("auto:daily-save", &today)
                .map_err(|e| e.to_string())?
        };
        if should_snapshot {
            let vm = state.version_manager.lock().await;
            vm.snapshot(&life_model, "auto:daily-save", "当日首次保存自动快照")
                .map_err(|e| e.to_string())?;
            let mut last_snapshot_date = state.last_snapshot_date.lock().await;
            *last_snapshot_date = Some(today);
        }
    }
    Ok(life_model)
}
#[tauri::command]
async fn send_message(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<SendMessageResult, String> {
    let selected_skill_id = selected_skill_id.as_deref().map(str::to_owned);
    main_chat_send::send_message_with_state(session_id, messages, selected_skill_id, state.inner())
        .await
}

#[derive(serde::Deserialize, Clone, Debug)]
struct StartStreamMessageArgs {
    session_id: String,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    selected_skill_id: Option<String>,
}

#[tauri::command]
async fn start_stream_message<R: tauri::Runtime>(
    args: Option<StartStreamMessageArgs>,
    session_id: Option<String>,
    messages: Option<Vec<ChatMessage>>,
    selected_skill_id: Option<String>,
    app_handle: tauri::AppHandle<R>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let (session_id, messages, selected_skill_id) = if let Some(args) = args {
        (args.session_id, args.messages, args.selected_skill_id)
    } else {
        (
            session_id.ok_or_else(|| "start_stream_message 缺少 session_id".to_string())?,
            messages.ok_or_else(|| "start_stream_message 缺少 messages".to_string())?,
            selected_skill_id,
        )
    };

    let selected_skill_id = selected_skill_id.as_deref().map(str::to_owned);
    let app_handle = app_handle.clone();
    main_chat_streaming::start_stream_message_with_state(
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
async fn execute_tool_call(
    name: String,
    arguments: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<ToolCallResult, String> {
    let (reg, audit) = state.get_mcp_state().await;
    let permission_store = state.tool_permission_store.lock().await;
    let privacy_engine = state.privacy_engine.lock().await;
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();

    // Create an AgentRun for direct tool execution audit trail
    let mut run = openlife_core::agent::AgentRun::new_tool_execution_run(&name);
    let run_id = run.id.clone();

    let agent_run_store_guard = if let Some(ref store) = state.agent_run_store {
        Some(store.lock().await)
    } else {
        None
    };

    let executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let ctx = openlife_core::agent::ActionExecutionContext::new(
        &reg,
        &permission_store,
        &audit,
        &privacy_engine,
        &safe_paths,
    );
    let ctx = if let Some(ref store) = agent_run_store_guard {
        ctx.with_agent_run_store(store)
    } else {
        ctx
    };

    let request = openlife_core::agent::AgentActionRequest {
        action_type: "mcp_tool".to_string(),
        target: name.clone(),
        input: serde_json::json!({ "arguments": arguments }),
        source_run_id: Some(run_id.clone()),
        step_index: 0,
    };

    let result = executor.execute(request, &ctx).map_err(|e| e.to_string())?;

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

    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&run);
    }

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
        react_trace: result.action.react_trace,
    };

    Ok(tool_result)
}
#[tauri::command]
async fn inspect_mcp_call(
    name: String,
    arguments: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::mcp::McpArgumentInspection, String> {
    let reg = state.mcp_registry.lock().await;
    Ok(reg.inspect_call_arguments(&name, &arguments))
}

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
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_http::init())
        .manage(app_state.clone())
        .setup(move |app| {
            if let Err(e) = ensure_main_window_visible(app) {
                log::warn!("[setup] failed to show main window: {}", e);
                return Err(Box::new(e));
            }
            log::info!("[setup] launching a2a sidecar");
            let a2a_sidecar = app_state_for_setup.a2a_sidecar.clone();
            let state = app_state_for_setup.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = a2a_sidecar.lock().await.start().await {
                    log::warn!("[setup] a2a sidecar start failed: {}", e);
                    log::warn!("[setup] falling back to embedded a2a server");
                    a2a_server::start(state).await;
                }
            });
            if std::env::var("OPENLIFE_AUTOSTART_FILESYSTEM_MCP").as_deref() == Ok("1") {
                let mcp_registry = app_state_for_setup.mcp_registry.clone();
                tauri::async_runtime::spawn(async move {
                    let mut registry = mcp_registry.lock().await;
                    if let Err(e) = registry.register(
                        "filesystem",
                        "npx",
                        &["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
                    ) {
                        eprintln!(
                            "[setup] autoregister filesystem mcp failed: {} - lib.rs:2246",
                            e
                        );
                    }
                });
            }
            let vs = app_state_for_setup.vector_store.clone();
            tauri::async_runtime::spawn(async move {
                {
                    let store = vs.lock().await;
                    match store.run_tier_maintenance() {
                        Ok((upgraded, downgraded)) => {
                            log::info!("[tier] initial maintenance done: upgraded={} downgraded={} - lib.rs:2255", upgraded, downgraded);
                        }
                        Err(e) => {
                            log::warn!("[tier] initial maintenance failed: {} - lib.rs:2258", e);
                        }
                    }
                }
                let interval = std::time::Duration::from_secs(600);
                loop {
                    tokio::time::sleep(interval).await;
                    let store = vs.lock().await;
                    match store.run_tier_maintenance() {
                        Ok((upgraded, downgraded)) => {
                            log::info!("[tier] periodic maintenance done: upgraded={} downgraded={} - lib.rs:2268", upgraded, downgraded);
                        }
                        Err(e) => {
                            log::warn!("[tier] periodic maintenance failed: {} - lib.rs:2271", e);
                        }
                    }
                }
            });
            // Start scheduled task runner
            scheduler_runner::start_scheduler_runner(app_state_for_setup.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_life_model,
            get_life_model_current_view,
            save_life_model,
            get_config,
            save_config,
            get_agent_run,
            list_agent_runs,
            list_provider_transmission_history,
            list_agent_runs_for_session,
            delete_agent_run,
            restore_agent_run,
            replay_agent_action,
            run_multi_strategy_agent_preview,
            run_main_chat_agent_execution_v1_eval_gate,
            run_main_chat_capability_eval_gate,
            run_main_chat_agent_productization_v1_gate,
            run_main_chat_external_live_productization_gate,
            run_main_chat_agent_product_maturity_v2_event_gate,
            run_main_chat_agent_product_maturity_v2_plan_gate,
            run_main_chat_agent_product_maturity_v2_skills_gate,
            run_main_chat_agent_product_maturity_v2_final_readiness_gate,
            run_main_chat_agent_beta_v1_readiness_gate,
            run_main_chat_agent_stage1_dogfood_gate,
            run_main_chat_agent_stage2_readiness_gate,
            run_main_chat_agent_step6_product_acceptance_gate,
            prepare_main_chat_step6_live_provider_eval_state,
            run_main_chat_stage3_execution_ux_report,
            validate_main_chat_agent_stage2_manual_dogfood_artifact,
            prepare_main_chat_agent_stage1_browser_dogfood_state,
            set_main_chat_agent_stage1_browser_network_policy,
            set_main_chat_agent_stage1_browser_scripted_response,
            set_main_chat_agent_stage1_browser_web_fixture_output,
            run_main_chat_agent_execution_v1_final_acceptance_gate,
            get_runtime_strategy_registry_status,
            get_main_chat_runtime_status,
            get_react_beta_execution_status,
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
            check_runtime_migration_gate,
            check_controlled_chat_pilot_eligibility,
            check_controlled_pilot_promotion_readiness,
            draft_controlled_chat_migration_plan,
            record_controlled_chat_migration_review_decision,
            get_controlled_chat_migration_review_decision_summary,
            check_controlled_chat_migration_implementation_gate,
            run_controlled_chat_migration_shadow_run,
            record_controlled_chat_migration_shadow_review_decision,
            get_controlled_chat_migration_shadow_review_summary,
            check_controlled_chat_cutover_readiness,
            run_controlled_chat_cutover_candidate,
            record_controlled_chat_cutover_candidate_review_decision,
            get_controlled_chat_cutover_candidate_review_summary,
            check_controlled_chat_cutover_candidate_promotion_readiness,
            record_controlled_pilot_promotion_evidence,
            get_controlled_pilot_promotion_evidence_summary,
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
            list_stage4_knowledge_asset_inventory,
            create_managed_knowledge_write_draft,
            confirm_managed_knowledge_write,
            rollback_managed_knowledge_write,
            run_main_chat_stage4_memory_knowledge_report,
            evaluate_main_chat_stage5_release_debug_preflight,
            export_main_chat_agent_debug_bundle,
            create_main_chat_internal_issue_report,
            list_main_chat_debug_bundles,
            get_main_chat_debug_bundle,
            delete_main_chat_debug_bundle,
            list_main_chat_internal_issue_reports,
            get_main_chat_internal_issue_report,
            delete_main_chat_internal_issue_report,
            run_main_chat_stage5_release_debug_report,
            send_message,
            start_stream_message,
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
            execute_tool_call,
            inspect_mcp_call,
            register_mcp_server,
            unregister_mcp_server,
            list_mcp_servers,
            list_mcp_tools,
            list_mcp_templates,
            recommend_mcp_manifests,
            list_tool_manifests,
            list_mcp_audit_logs,
            clear_mcp_audit_logs,
            get_system_diagnostics,
            get_runtime_build_info,
            check_ollama_status,
            get_router_status,
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
            index_memory_chunk,
            search_memory,
            a2a_discover_agent,
            a2a_send_task,
            a2a_local_agent_card,
            a2a_handle_task,
            a2a_bridge_local,
            a2a_restart_sidecar,
            a2a_stop_sidecar,
            builder_start,
            builder_step,
            builder_list_unfinished,
            builder_delete_session,
            builder_get_pending_signals,
            builder_create_proposals,
            builder_apply_signals,
            get_model_4d_completion,
            goal_capability_gap_analysis,
            goal_capability_gap_report,
            identity_goal_alignment_check,
            identity_goal_alignment_report,
            export_all_data,
            get_danger_action_preflight,
            import_all_data,
            test_api_key,
            test_llm_connection,
            get_last_model_error,
            list_chat_sessions,
            create_chat_session,
            rename_chat_session,
            delete_chat_session,
            record_state,
            get_state_history,
            get_state_alerts,
            get_daily_goals,
            add_daily_goal,
            update_daily_goal,
            delete_daily_goal,
            toggle_daily_goal,
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
            export_mcp_audit_logs,
            cleanup_mcp_audit_logs,
            rotate_mcp_audit_key,
            get_privacy_policy,
            set_privacy_policy,
            get_rollout_metrics,
            get_rollout_summary,
            get_rollout_errors,
            get_proactive_suggestions,
            list_tool_permissions,
            grant_tool_permission,
            revoke_tool_permission,
            check_tool_permission,
            list_skills,
            get_skill_runtime_status,
            run_skill,
            get_skill_run_status,
            list_plugins,
            reload_plugins,
            enable_plugin,
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

#[cfg(test)]
mod legacy_write_convergence_tests;
