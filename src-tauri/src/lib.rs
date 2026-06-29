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
pub(crate) mod default_chat_adapter;
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
pub(crate) mod main_chat_legacy_fallback;
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
    replay_agent_action, restore_agent_run,
};
use commands::agent_runtime::{
    cancel_plan_execute_session, check_controlled_chat_cutover_candidate_promotion_readiness,
    check_controlled_chat_cutover_readiness, check_controlled_chat_migration_implementation_gate,
    check_controlled_chat_pilot_eligibility, check_controlled_pilot_promotion_readiness,
    check_default_chat_adapter_activation_implementation_gate,
    check_default_chat_adapter_contract_harness,
    check_default_chat_adapter_controlled_preview_approval_readiness,
    check_default_chat_adapter_cutover_plan_approval_readiness,
    check_default_chat_adapter_implementation_readiness,
    check_default_chat_adapter_narrow_implementation_discussion_gate,
    check_default_chat_adapter_narrow_implementation_plan_approval_readiness,
    check_runtime_migration_gate, clear_main_chat_skill, create_plan_execute_session,
    draft_controlled_chat_migration_plan, draft_default_chat_adapter_activation_plan,
    draft_default_chat_adapter_cutover_implementation_plan,
    draft_default_chat_adapter_narrow_implementation_plan, execute_plan_execute_step,
    finalize_plan_execute_session, get_controlled_chat_cutover_candidate_review_summary,
    get_controlled_chat_migration_review_decision_summary,
    get_controlled_chat_migration_shadow_review_summary,
    get_controlled_pilot_promotion_evidence_summary,
    get_default_chat_adapter_activation_review_summary,
    get_default_chat_adapter_controlled_preview_review_summary,
    get_default_chat_adapter_cutover_plan_review_summary,
    get_default_chat_adapter_dry_run_review_summary,
    get_default_chat_adapter_narrow_implementation_plan_review_summary,
    get_default_chat_adapter_ordinary_entry_preflight_status,
    get_default_chat_adapter_routing_status, get_default_chat_runtime_boundary_status,
    get_main_chat_skill_detail, get_plan_execute_session, get_react_beta_execution_status,
    get_runtime_strategy_registry_status, list_main_chat_skills, list_main_chat_tool_candidates,
    list_plan_execute_sessions, prepare_main_chat_agent_stage1_browser_dogfood_state,
    prepare_main_chat_step6_live_provider_eval_state,
    record_controlled_chat_cutover_candidate_review_decision,
    record_controlled_chat_migration_review_decision,
    record_controlled_chat_migration_shadow_review_decision,
    record_controlled_pilot_promotion_evidence,
    record_default_chat_adapter_activation_review_decision,
    record_default_chat_adapter_controlled_preview_review_decision,
    record_default_chat_adapter_cutover_plan_review_decision,
    record_default_chat_adapter_dry_run_review_decision,
    record_default_chat_adapter_narrow_implementation_plan_review_decision,
    review_plan_execute_session, run_controlled_chat_cutover_candidate,
    run_controlled_chat_migration_shadow_run, run_default_chat_adapter_controlled_preview,
    run_default_chat_adapter_dry_run, run_main_chat_agent_beta_v1_readiness_gate,
    run_main_chat_agent_execution_v1_eval_gate,
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
use commands::life_model::{get_life_model, save_life_model};
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
    get_last_model_error, get_privacy_policy, has_completed_onboarding, import_all_data,
    mark_onboarding_completed, rotate_mcp_audit_key, save_config, set_privacy_policy, test_api_key,
    test_llm_connection,
};
use commands::state::{
    add_daily_goal, delete_daily_goal, get_daily_goals, get_state_alerts, get_state_history,
    record_state, toggle_daily_goal, update_daily_goal,
};
use commands::version::{create_snapshot, diff_snapshots, list_snapshots, restore_snapshot};
use main_chat_event_stream::{get_main_chat_agent_state_snapshot, list_main_chat_agent_events};
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
    pub reasoning_trace: openlife_core::agent::ReasoningTrace,
    pub tool_calls: Vec<ToolCallResult>,
    pub run_id: Option<String>,
    pub agent_ingress: Option<openlife_core::agent::main_chat_agent_v1::AgentIngressDecision>,
    pub agent_state:
        Option<openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentStateSnapshot>,
    pub execution_transcript:
        Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>,
    pub legacy_fallback_used: bool,
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
    pub legacy_data_dir: Option<String>,
    pub database_status: String,
    pub startup_warnings: Vec<String>,
    pub snapshot_count: usize,
    pub life_model_ready: bool,
    pub app_version: String,
    pub model_empty: bool,
    pub chat_session_count: usize,
    pub onboarding_completed: bool,
    pub beta_ready: bool,
    pub beta_readiness_issues: Vec<String>,
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

fn ensure_main_window_visible<R: tauri::Runtime, M: Manager<R>>(manager: &M) -> tauri::Result<()> {
    let window = if let Some(window) = manager.get_webview_window("main") {
        window
    } else {
        let main_window_config = manager
            .config()
            .app
            .windows
            .iter()
            .find(|config| config.label == "main")
            .ok_or_else(|| anyhow::anyhow!("tauri config is missing the main window"))?;
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
            save_life_model,
            get_config,
            save_config,
            get_agent_run,
            list_agent_runs,
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
            get_default_chat_runtime_boundary_status,
            draft_default_chat_adapter_activation_plan,
            record_default_chat_adapter_activation_review_decision,
            get_default_chat_adapter_activation_review_summary,
            check_default_chat_adapter_activation_implementation_gate,
            get_default_chat_adapter_routing_status,
            check_default_chat_adapter_contract_harness,
            get_default_chat_adapter_ordinary_entry_preflight_status,
            check_default_chat_adapter_narrow_implementation_discussion_gate,
            draft_default_chat_adapter_narrow_implementation_plan,
            record_default_chat_adapter_narrow_implementation_plan_review_decision,
            get_default_chat_adapter_narrow_implementation_plan_review_summary,
            check_default_chat_adapter_narrow_implementation_plan_approval_readiness,
            run_default_chat_adapter_dry_run,
            record_default_chat_adapter_dry_run_review_decision,
            get_default_chat_adapter_dry_run_review_summary,
            check_default_chat_adapter_implementation_readiness,
            run_default_chat_adapter_controlled_preview,
            record_default_chat_adapter_controlled_preview_review_decision,
            get_default_chat_adapter_controlled_preview_review_summary,
            check_default_chat_adapter_controlled_preview_approval_readiness,
            draft_default_chat_adapter_cutover_implementation_plan,
            record_default_chat_adapter_cutover_plan_review_decision,
            get_default_chat_adapter_cutover_plan_review_summary,
            check_default_chat_adapter_cutover_plan_approval_readiness,
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
            has_completed_onboarding,
            mark_onboarding_completed,
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

#[cfg(test)]
mod hs_runtime_tests {
    #[test]
    fn default_chat_adapter_cutover_route_guard_defaults_to_disabled_legacy_stream() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        assert_eq!(route.current_mode, "legacy_stream");
        assert!(route.adapter_scaffold_present);
        assert!(!route.controlled_adapter_enabled);
        assert!(!route.automatic_migration_enabled);
        assert_eq!(route.default_send_path, "legacy_stream");
        assert_eq!(route.start_stream_path, "legacy_stream");
        assert!(route.requires_separate_cutover_implementation);
        crate::default_chat_adapter::ensure_default_chat_cutover_harness("send_message", &route)
            .expect("disabled scaffold must allow legacy send path");
    }

    #[test]
    fn default_chat_adapter_cutover_route_guard_fails_closed_for_enabled_route() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.controlled_adapter_enabled = true;
        route.default_send_path = "controlled_adapter".into();

        let error = crate::default_chat_adapter::ensure_default_chat_cutover_harness(
            "send_message",
            &route,
        )
        .expect_err("enabled adapter route must fail closed until cutover is implemented");

        assert!(error.contains("send_message"));
        assert!(error.contains("controlled_adapter_enabled"));
        assert!(error.contains("default_send_path_not_legacy_stream"));
    }

    #[test]
    fn default_chat_adapter_cutover_harness_is_legacy_guarded_and_side_effect_free() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let harness = crate::default_chat_adapter::evaluate_default_chat_adapter_cutover_harness(
            "send_message",
            &route,
        );

        assert!(harness.harness_ready);
        assert!(harness.route_guard_passed);
        assert_eq!(harness.invocation_mode, "legacy_guarded");
        assert_eq!(harness.default_send_path, "legacy_stream");
        assert_eq!(harness.start_stream_path, "legacy_stream");
        assert!(!harness.controlled_adapter_invocation_allowed);
        assert!(!harness.runtime_call_enabled);
        assert!(!harness.model_call_enabled);
        assert!(!harness.tool_call_enabled);
        assert!(!harness.allow_writes);
        assert_eq!(harness.max_tool_calls, 0);
        assert!(!harness.chat_message_saved);
        assert!(!harness.agent_run_recorded);
        assert!(!harness.evidence_recorded);
        assert!(harness.default_chat_path_unchanged);
        assert!(harness.requires_separate_cutover_implementation);
        assert!(harness.blocking_reasons.is_empty());
        crate::default_chat_adapter::ensure_default_chat_cutover_harness("send_message", &route)
            .expect("default route must satisfy the legacy guarded cutover harness");
    }

    #[test]
    fn default_chat_adapter_cutover_harness_fails_closed_for_route_drift() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.current_mode = "controlled_adapter".into();
        route.adapter_scaffold_present = false;
        route.controlled_adapter_enabled = true;
        route.automatic_migration_enabled = true;
        route.default_send_path = "controlled_adapter".into();
        route.start_stream_path = "controlled_adapter".into();
        route.requires_separate_cutover_implementation = false;

        let harness = crate::default_chat_adapter::evaluate_default_chat_adapter_cutover_harness(
            "start_stream_message",
            &route,
        );

        assert!(!harness.harness_ready);
        assert!(!harness.route_guard_passed);
        assert_eq!(harness.invocation_mode, "blocked");
        assert!(!harness.default_chat_path_unchanged);
        assert!(harness
            .blocking_reasons
            .contains(&"adapter_scaffold_missing".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"separate_cutover_implementation_not_required".to_string()));

        let error = crate::default_chat_adapter::ensure_default_chat_cutover_harness(
            "start_stream_message",
            &route,
        )
        .expect_err("route drift must fail closed before default Chat can cut over");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("adapter_scaffold_missing"));
        assert!(error.contains("separate_cutover_implementation_not_required"));
    }

    #[test]
    fn default_chat_adapter_invocation_plan_selects_legacy_with_controlled_candidate_disabled() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let plan = crate::default_chat_adapter::plan_default_chat_adapter_invocation(
            "send_message",
            &route,
        );

        assert!(plan.plan_ready);
        assert!(plan.harness_ready);
        assert_eq!(plan.selected_adapter_path, "legacy_stream");
        assert_eq!(plan.fallback_adapter_path, "legacy_stream");
        assert_eq!(plan.controlled_adapter_candidate_path, "controlled_adapter");
        assert!(!plan.controlled_adapter_invocation_allowed);
        assert!(!plan.controlled_adapter_executor_attached);
        assert_eq!(plan.send_contract_shape, "send_message_compatible");
        assert_eq!(plan.stream_contract_shape, "stream_message_compatible");
        assert!(!plan.runtime_call_enabled);
        assert!(!plan.model_call_enabled);
        assert!(!plan.tool_call_enabled);
        assert!(!plan.allow_writes);
        assert_eq!(plan.max_tool_calls, 0);
        assert!(!plan.chat_message_saved);
        assert!(!plan.agent_run_recorded);
        assert!(!plan.evidence_recorded);
        assert!(plan.default_chat_path_unchanged);
        assert!(plan.blocking_reasons.is_empty());
        crate::default_chat_adapter::ensure_default_chat_adapter_invocation_plan(
            "send_message",
            &route,
        )
        .expect("default route must keep the invocation plan on legacy stream");
    }

    #[test]
    fn default_chat_adapter_invocation_plan_blocks_when_harness_blocks() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.controlled_adapter_enabled = true;
        route.default_send_path = "controlled_adapter".into();

        let plan = crate::default_chat_adapter::plan_default_chat_adapter_invocation(
            "send_message",
            &route,
        );

        assert!(!plan.plan_ready);
        assert!(!plan.harness_ready);
        assert_eq!(plan.selected_adapter_path, "blocked");
        assert_eq!(plan.fallback_adapter_path, "legacy_stream");
        assert!(!plan.default_chat_path_unchanged);
        assert!(plan
            .blocking_reasons
            .contains(&"cutover_harness_not_ready".to_string()));
        assert!(plan
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));
        assert!(plan
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));

        let error = crate::default_chat_adapter::ensure_default_chat_adapter_invocation_plan(
            "send_message",
            &route,
        )
        .expect_err("blocked harness must prevent default Chat invocation planning");
        assert!(error.contains("send_message"));
        assert!(error.contains("cutover_harness_not_ready"));
        assert!(error.contains("controlled_adapter_enabled"));
    }

    #[test]
    fn default_chat_adapter_invocation_boundary_requires_legacy_path_only() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let boundary =
            crate::default_chat_adapter::evaluate_default_chat_adapter_invocation_boundary(
                "send_message",
                &route,
            );

        assert!(boundary.boundary_ready);
        assert!(boundary.plan_ready);
        assert_eq!(boundary.selected_adapter_path, "legacy_stream");
        assert_eq!(boundary.required_callsite_path, "legacy_stream");
        assert_eq!(boundary.fallback_adapter_path, "legacy_stream");
        assert_eq!(
            boundary.controlled_adapter_candidate_path,
            "controlled_adapter"
        );
        assert!(boundary.legacy_adapter_invocation_required);
        assert!(!boundary.controlled_adapter_invocation_allowed);
        assert!(!boundary.controlled_adapter_executor_attached);
        assert!(boundary.side_effect_free_before_legacy_entry);
        assert!(!boundary.runtime_call_enabled);
        assert!(!boundary.model_call_enabled);
        assert!(!boundary.tool_call_enabled);
        assert!(!boundary.allow_writes);
        assert_eq!(boundary.max_tool_calls, 0);
        assert!(!boundary.chat_message_saved);
        assert!(!boundary.agent_run_recorded);
        assert!(!boundary.evidence_recorded);
        assert!(boundary.blocking_reasons.is_empty());

        let decision =
            crate::default_chat_adapter::ensure_default_chat_adapter_invocation_boundary(
                "send_message",
                &route,
            )
            .expect("default invocation boundary must select the legacy adapter path");
        assert_eq!(decision.selected_adapter_path, "legacy_stream");
    }

    #[test]
    fn default_chat_adapter_invocation_boundary_blocks_when_plan_blocks() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.start_stream_path = "controlled_adapter".into();

        let boundary =
            crate::default_chat_adapter::evaluate_default_chat_adapter_invocation_boundary(
                "start_stream_message",
                &route,
            );

        assert!(!boundary.boundary_ready);
        assert!(!boundary.plan_ready);
        assert_eq!(boundary.selected_adapter_path, "blocked");
        assert_eq!(boundary.required_callsite_path, "legacy_stream");
        assert!(!boundary.legacy_adapter_invocation_required);
        assert!(boundary
            .blocking_reasons
            .contains(&"invocation_plan_not_ready".to_string()));
        assert!(boundary
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));

        let error = crate::default_chat_adapter::ensure_default_chat_adapter_invocation_boundary(
            "start_stream_message",
            &route,
        )
        .expect_err("blocked invocation plan must prevent default adapter boundary entry");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("invocation_plan_not_ready"));
        assert!(error.contains("start_stream_path_not_legacy_stream"));
    }

    #[test]
    fn default_chat_adapter_callsite_contract_selects_typed_legacy_paths() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let send_contract =
            crate::default_chat_adapter::evaluate_default_chat_adapter_callsite_contract(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
            );
        let stream_contract =
            crate::default_chat_adapter::evaluate_default_chat_adapter_callsite_contract(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
            );

        assert!(send_contract.contract_ready);
        assert!(send_contract.boundary_ready);
        assert_eq!(send_contract.callsite, "send_message");
        assert_eq!(send_contract.contract_shape, "send_message_compatible");
        assert_eq!(send_contract.actual_callsite_path, "legacy_stream");
        assert_eq!(send_contract.required_callsite_path, "legacy_stream");
        assert_eq!(send_contract.selected_adapter_path, "legacy_stream");
        assert!(!send_contract.controlled_adapter_executor_attached);
        assert!(send_contract.side_effect_free_before_legacy_entry);
        assert!(send_contract.blocking_reasons.is_empty());

        assert!(stream_contract.contract_ready);
        assert!(stream_contract.boundary_ready);
        assert_eq!(stream_contract.callsite, "start_stream_message");
        assert_eq!(stream_contract.contract_shape, "stream_message_compatible");
        assert_eq!(stream_contract.actual_callsite_path, "legacy_stream");
        assert_eq!(stream_contract.required_callsite_path, "legacy_stream");
        assert_eq!(stream_contract.selected_adapter_path, "legacy_stream");
        assert!(!stream_contract.controlled_adapter_executor_attached);
        assert!(stream_contract.side_effect_free_before_legacy_entry);
        assert!(stream_contract.blocking_reasons.is_empty());
    }

    #[test]
    fn default_chat_adapter_callsite_contract_blocks_when_callsite_route_drifts() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.default_send_path = "controlled_adapter".into();

        let contract = crate::default_chat_adapter::evaluate_default_chat_adapter_callsite_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
        );

        assert!(!contract.contract_ready);
        assert!(!contract.boundary_ready);
        assert_eq!(contract.callsite, "send_message");
        assert_eq!(contract.actual_callsite_path, "controlled_adapter");
        assert_eq!(contract.required_callsite_path, "legacy_stream");
        assert_eq!(contract.selected_adapter_path, "blocked");
        assert!(contract
            .blocking_reasons
            .contains(&"invocation_boundary_not_ready".to_string()));
        assert!(contract
            .blocking_reasons
            .contains(&"callsite_path_not_legacy_stream".to_string()));
        assert!(contract
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));

        let error = crate::default_chat_adapter::ensure_default_chat_adapter_callsite_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
        )
        .expect_err("route drift must block the typed default Chat adapter callsite");
        assert!(error.contains("send_message"));
        assert!(error.contains("callsite_path_not_legacy_stream"));
    }

    #[test]
    fn default_chat_adapter_ordinary_entry_preflight_locks_zero_side_effect_budget() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let send_preflight =
            crate::default_chat_adapter::evaluate_default_chat_adapter_ordinary_entry_preflight(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
            );
        let stream_preflight =
            crate::default_chat_adapter::evaluate_default_chat_adapter_ordinary_entry_preflight(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
            );

        assert!(send_preflight.preflight_ready);
        assert!(send_preflight.contract_ready);
        assert!(send_preflight.legacy_entry_allowed);
        assert_eq!(send_preflight.callsite, "send_message");
        assert_eq!(send_preflight.contract_shape, "send_message_compatible");
        assert_eq!(send_preflight.ordinary_entry_path, "legacy_stream");
        assert_eq!(send_preflight.required_entry_path, "legacy_stream");
        assert!(send_preflight.side_effect_lock_engaged);
        assert!(!send_preflight.default_chat_migration_allowed);
        assert!(!send_preflight.controlled_adapter_executor_attached);
        assert!(!send_preflight.runtime_call_enabled);
        assert!(!send_preflight.model_call_enabled);
        assert!(!send_preflight.tool_call_enabled);
        assert!(!send_preflight.allow_writes);
        assert_eq!(send_preflight.max_tool_calls, 0);
        assert!(!send_preflight.chat_message_saved);
        assert!(!send_preflight.agent_run_recorded);
        assert!(!send_preflight.evidence_recorded);
        assert!(send_preflight.blocking_reasons.is_empty());

        assert!(stream_preflight.preflight_ready);
        assert!(stream_preflight.contract_ready);
        assert!(stream_preflight.legacy_entry_allowed);
        assert_eq!(stream_preflight.callsite, "start_stream_message");
        assert_eq!(stream_preflight.contract_shape, "stream_message_compatible");
        assert_eq!(stream_preflight.ordinary_entry_path, "legacy_stream");
        assert_eq!(stream_preflight.required_entry_path, "legacy_stream");
        assert!(stream_preflight.side_effect_lock_engaged);
        assert!(!stream_preflight.default_chat_migration_allowed);
        assert!(!stream_preflight.controlled_adapter_executor_attached);
        assert!(!stream_preflight.runtime_call_enabled);
        assert!(!stream_preflight.model_call_enabled);
        assert!(!stream_preflight.tool_call_enabled);
        assert!(!stream_preflight.allow_writes);
        assert_eq!(stream_preflight.max_tool_calls, 0);
        assert!(!stream_preflight.chat_message_saved);
        assert!(!stream_preflight.agent_run_recorded);
        assert!(!stream_preflight.evidence_recorded);
        assert!(stream_preflight.blocking_reasons.is_empty());
    }

    #[test]
    fn default_chat_adapter_ordinary_entry_preflight_blocks_route_drift() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.start_stream_path = "controlled_adapter".into();

        let preflight =
            crate::default_chat_adapter::evaluate_default_chat_adapter_ordinary_entry_preflight(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
            );

        assert!(!preflight.preflight_ready);
        assert!(!preflight.contract_ready);
        assert!(!preflight.legacy_entry_allowed);
        assert_eq!(preflight.callsite, "start_stream_message");
        assert_eq!(preflight.ordinary_entry_path, "blocked");
        assert_eq!(preflight.required_entry_path, "legacy_stream");
        assert!(preflight.side_effect_lock_engaged);
        assert!(preflight
            .blocking_reasons
            .contains(&"callsite_contract_not_ready".to_string()));
        assert!(preflight
            .blocking_reasons
            .contains(&"callsite_path_not_legacy_stream".to_string()));
        assert!(preflight
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_adapter_ordinary_entry_preflight(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
            )
            .expect_err("route drift must block the ordinary default Chat adapter entry preflight");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("callsite_contract_not_ready"));
    }

    #[test]
    fn default_chat_adapter_descriptor_is_metadata_safe_and_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input =
            "raw-user-secret prompt-token assistant-output tool-payload lifemodel-memory";

        let descriptor =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        assert!(descriptor.metadata_safe);
        assert!(!descriptor.contains_raw_content);
        assert_eq!(descriptor.callsite_kind, "send_message");
        assert_eq!(descriptor.contract_shape, "send_message_compatible");
        assert_eq!(descriptor.input_length_bytes, raw_input.len());
        assert_eq!(descriptor.input_length_chars, raw_input.chars().count());
        assert!(descriptor.input_sha256.starts_with("sha256:"));
        assert!(!descriptor.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{descriptor:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "lifemodel-memory",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "descriptor leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_descriptor_keeps_controlled_executor_disabled_unattached() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let descriptor =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "metadata only input",
            );

        assert!(descriptor.descriptor_ready);
        assert!(!descriptor.fail_closed);
        assert_eq!(descriptor.route_mode, "legacy_stream");
        assert_eq!(
            descriptor.controlled_adapter_candidate_path,
            "controlled_adapter"
        );
        assert!(!descriptor.controlled_adapter_enabled);
        assert!(!descriptor.controlled_adapter_invocation_allowed);
        assert!(!descriptor.controlled_adapter_executor_enabled);
        assert!(!descriptor.controlled_adapter_executor_attached);
        assert_eq!(
            descriptor.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!descriptor.allow_writes);
        assert_eq!(descriptor.max_tool_calls, 0);
        assert_eq!(descriptor.side_effect_budget.runtime_calls, 0);
        assert_eq!(descriptor.side_effect_budget.model_calls, 0);
        assert_eq!(descriptor.side_effect_budget.tool_calls, 0);
        assert_eq!(descriptor.side_effect_budget.store_writes, 0);
        assert_eq!(descriptor.side_effect_budget.chat_message_writes, 0);
        assert_eq!(descriptor.side_effect_budget.agent_run_writes, 0);
        assert_eq!(descriptor.side_effect_budget.evidence_writes, 0);
        assert_eq!(descriptor.side_effect_budget.proposal_writes, 0);
        assert_eq!(descriptor.side_effect_budget.memory_writes, 0);
        assert_eq!(descriptor.side_effect_budget.life_model_writes, 0);
        assert_eq!(descriptor.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(descriptor.side_effect_budget.external_writes, 0);
    }

    #[test]
    fn default_chat_adapter_descriptor_default_send_stream_routes_remain_legacy_stream() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let send_descriptor =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "send input",
            );
        let stream_descriptor =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "stream input",
            );

        assert!(send_descriptor.descriptor_ready);
        assert!(stream_descriptor.descriptor_ready);
        assert_eq!(send_descriptor.selected_adapter_path, "legacy_stream");
        assert_eq!(stream_descriptor.selected_adapter_path, "legacy_stream");
        assert_eq!(send_descriptor.default_send_path, "legacy_stream");
        assert_eq!(send_descriptor.start_stream_path, "legacy_stream");
        assert_eq!(stream_descriptor.default_send_path, "legacy_stream");
        assert_eq!(stream_descriptor.start_stream_path, "legacy_stream");
        assert!(!send_descriptor.migration_permission);
        assert!(!stream_descriptor.migration_permission);
        assert!(send_descriptor.blocking_reasons.is_empty());
        assert!(stream_descriptor.blocking_reasons.is_empty());
    }

    #[test]
    fn default_chat_adapter_descriptor_fails_closed_for_route_drift_enabled_and_auto_migration() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.current_mode = "controlled_adapter".into();
        route.controlled_adapter_enabled = true;
        route.automatic_migration_enabled = true;
        route.default_send_path = "controlled_adapter".into();
        route.requires_separate_cutover_implementation = false;

        let descriptor =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "blocked input",
            );

        assert!(!descriptor.descriptor_ready);
        assert!(descriptor.fail_closed);
        assert!(!descriptor.route_guard_passed);
        assert_eq!(descriptor.selected_adapter_path, "blocked");
        assert!(!descriptor.migration_permission);
        assert!(!descriptor.controlled_adapter_invocation_allowed);
        assert!(!descriptor.controlled_adapter_executor_enabled);
        assert!(!descriptor.controlled_adapter_executor_attached);
        assert!(descriptor
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(descriptor
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));
        assert!(descriptor
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
        assert!(descriptor
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));
        assert!(descriptor
            .blocking_reasons
            .contains(&"separate_cutover_implementation_not_required".to_string()));
    }

    #[test]
    fn default_chat_adapter_descriptor_mapper_is_side_effect_free_and_stable() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let first = crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            "stable side-effect-free input",
        );
        let second =
            crate::default_chat_adapter::describe_default_chat_controlled_adapter_candidate(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "stable side-effect-free input",
            );

        assert_eq!(first, second);
        assert!(first.mapper_side_effect_free);
        assert!(first.side_effect_budget.is_zero());
        assert!(!first.allow_writes);
        assert_eq!(first.max_tool_calls, 0);
        assert!(!first.migration_permission);
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_contract_send_ready_without_migration_permission() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report = crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            "send raw prompt should be hashed only",
        );

        assert!(report.contract_ready);
        assert!(report.descriptor_ready);
        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert!(report.mapper_side_effect_free);
        assert_eq!(report.callsite_kind, "send_message");
        assert_eq!(report.contract_shape, "send_message_compatible");
        assert_eq!(report.selected_adapter_path, "legacy_stream");
        assert_eq!(report.required_callsite_path, "legacy_stream");
        assert_eq!(report.actual_callsite_path, "legacy_stream");
        assert_eq!(report.default_send_path, "legacy_stream");
        assert_eq!(report.start_stream_path, "legacy_stream");
        assert_eq!(
            report.controlled_adapter_candidate_path,
            "controlled_adapter"
        );
        assert!(!report.controlled_adapter_enabled);
        assert!(!report.automatic_migration_enabled);
        assert!(!report.controlled_adapter_invocation_allowed);
        assert!(!report.controlled_adapter_executor_enabled);
        assert!(!report.controlled_adapter_executor_attached);
        assert_eq!(
            report.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!report.allow_writes);
        assert_eq!(report.max_tool_calls, 0);
        assert!(report.side_effect_budget.is_zero());
        assert!(!report.migration_permission);
        assert!(report.default_chat_unchanged);
        assert!(report.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            "send raw prompt should be hashed only",
        )
        .expect("clean send route should produce a ready metadata-only contract report");
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_contract_stream_ready_without_migration_permission()
    {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report = crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
            &route,
            "stream raw prompt should be hashed only",
        );

        assert!(report.contract_ready);
        assert!(report.descriptor_ready);
        assert_eq!(report.callsite_kind, "start_stream_message");
        assert_eq!(report.contract_shape, "stream_message_compatible");
        assert_eq!(report.selected_adapter_path, "legacy_stream");
        assert_eq!(report.required_callsite_path, "legacy_stream");
        assert_eq!(report.actual_callsite_path, "legacy_stream");
        assert!(!report.controlled_adapter_invocation_allowed);
        assert!(!report.controlled_adapter_executor_enabled);
        assert!(!report.controlled_adapter_executor_attached);
        assert!(!report.migration_permission);
        assert!(report.default_chat_unchanged);
        assert!(report.side_effect_budget.is_zero());
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_contract_fails_closed_for_route_drift() {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.current_mode = "controlled_adapter".into();
        route.controlled_adapter_enabled = true;
        route.automatic_migration_enabled = true;
        route.start_stream_path = "controlled_adapter".into();
        route.requires_separate_cutover_implementation = false;

        let report = crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
            &route,
            "blocked input",
        );

        assert!(!report.contract_ready);
        assert!(!report.descriptor_ready);
        assert!(!report.default_chat_unchanged);
        assert_eq!(report.selected_adapter_path, "blocked");
        assert!(!report.controlled_adapter_invocation_allowed);
        assert!(!report.controlled_adapter_executor_enabled);
        assert!(!report.controlled_adapter_executor_attached);
        assert!(!report.migration_permission);
        assert!(report
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"callsite_path_not_legacy_stream".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"separate_cutover_implementation_not_required".to_string()));

        let error = crate::default_chat_adapter::ensure_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
            &route,
            "blocked input",
        )
        .expect_err("route drift must fail closed");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("controlled_adapter_contract_not_ready"));
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_contract_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input =
            "raw-user-secret prompt-token assistant-output tool-payload lifemodel-memory";

        let report = crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_contract(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            raw_input,
        );

        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert!(report.input_sha256.starts_with("sha256:"));
        assert!(!report.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{report:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "lifemodel-memory",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "contract report leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_send_ready_keeps_legacy_without_migration_permission(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "send raw prompt should be metadata only";
        let contract =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_contract(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        let harness =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        assert!(harness.harness_ready);
        assert_eq!(
            harness.harness_kind,
            "default_chat_controlled_adapter_non_default_invocation_harness"
        );
        assert_eq!(harness.callsite_kind, "send_message");
        assert_eq!(harness.contract_shape, contract.contract_shape);
        assert!(harness.contract_ready);
        assert!(harness.metadata_safe);
        assert!(!harness.contains_raw_content);
        assert!(harness.non_default);
        assert!(harness.ordinary_default_chat_path_unchanged);
        assert_eq!(harness.selected_adapter_path, "legacy_stream");
        assert_eq!(harness.candidate_adapter_path, "controlled_adapter");
        assert!(!harness.controlled_adapter_invocation_allowed);
        assert!(!harness.controlled_adapter_executor_enabled);
        assert!(!harness.controlled_adapter_executor_attached);
        assert_eq!(
            harness.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!harness.allow_writes);
        assert_eq!(harness.max_tool_calls, 0);
        assert!(!harness.migration_permission);
        assert!(harness.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_invocation_harness(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            raw_input,
        )
        .expect("send harness should prove only the non-default invocation shape");
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_stream_ready_keeps_legacy_without_migration_permission(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let harness =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "stream raw prompt should be metadata only",
            );

        assert!(harness.harness_ready);
        assert_eq!(harness.callsite_kind, "start_stream_message");
        assert_eq!(harness.contract_shape, "stream_message_compatible");
        assert!(harness.contract_ready);
        assert!(harness.metadata_safe);
        assert!(!harness.contains_raw_content);
        assert!(harness.non_default);
        assert!(harness.ordinary_default_chat_path_unchanged);
        assert_eq!(harness.selected_adapter_path, "legacy_stream");
        assert_eq!(harness.candidate_adapter_path, "controlled_adapter");
        assert!(!harness.controlled_adapter_invocation_allowed);
        assert!(!harness.controlled_adapter_executor_enabled);
        assert!(!harness.controlled_adapter_executor_attached);
        assert_eq!(
            harness.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!harness.migration_permission);
        assert!(harness.blocking_reasons.is_empty());
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_fails_closed_for_route_drift_controlled_adapter_and_auto_migration(
    ) {
        let mut route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        route.current_mode = "controlled_adapter".into();
        route.controlled_adapter_enabled = true;
        route.automatic_migration_enabled = true;
        route.default_send_path = "controlled_adapter".into();
        route.start_stream_path = "controlled_adapter".into();
        route.requires_separate_cutover_implementation = false;

        let harness =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "blocked raw input",
            );

        assert!(!harness.harness_ready);
        assert!(!harness.contract_ready);
        assert!(!harness.ordinary_default_chat_path_unchanged);
        assert_eq!(harness.selected_adapter_path, "blocked");
        assert_eq!(harness.candidate_adapter_path, "controlled_adapter");
        assert!(!harness.controlled_adapter_invocation_allowed);
        assert!(!harness.controlled_adapter_executor_enabled);
        assert!(!harness.controlled_adapter_executor_attached);
        assert_eq!(
            harness.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!harness.migration_permission);
        assert!(harness
            .blocking_reasons
            .contains(&"contract_not_ready".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"callsite_path_not_legacy_stream".to_string()));
        assert!(harness
            .blocking_reasons
            .contains(&"separate_cutover_implementation_not_required".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "blocked raw input",
            )
            .expect_err("route drift must fail the non-default invocation harness closed");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("controlled_adapter_invocation_harness_not_ready"));
        assert!(error.contains("contract_not_ready"));
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_keeps_executor_unattached_and_side_effect_budget_zero(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let harness =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "metadata only",
            );

        assert!(harness.harness_ready);
        assert!(!harness.controlled_adapter_executor_enabled);
        assert!(!harness.controlled_adapter_executor_attached);
        assert_eq!(
            harness.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!harness.runtime_call_enabled);
        assert!(!harness.model_call_enabled);
        assert!(!harness.tool_call_enabled);
        assert!(harness.business_write_disabled);
        assert!(!harness.allow_writes);
        assert_eq!(harness.max_tool_calls, 0);
        assert!(harness.side_effect_budget_zero);
        assert_eq!(harness.side_effect_budget.runtime_calls, 0);
        assert_eq!(harness.side_effect_budget.model_calls, 0);
        assert_eq!(harness.side_effect_budget.tool_calls, 0);
        assert_eq!(harness.side_effect_budget.store_writes, 0);
        assert_eq!(harness.side_effect_budget.chat_message_writes, 0);
        assert_eq!(harness.side_effect_budget.agent_run_writes, 0);
        assert_eq!(harness.side_effect_budget.evidence_writes, 0);
        assert_eq!(harness.side_effect_budget.proposal_writes, 0);
        assert_eq!(harness.side_effect_budget.memory_writes, 0);
        assert_eq!(harness.side_effect_budget.life_model_writes, 0);
        assert_eq!(harness.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(harness.side_effect_budget.external_writes, 0);
    }

    #[test]
    fn default_chat_adapter_controlled_adapter_invocation_harness_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input =
            "raw-user-secret prompt-token assistant-output tool-payload lifemodel-memory";

        let harness =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_invocation_harness(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        assert!(harness.metadata_safe);
        assert!(!harness.contains_raw_content);
        assert!(harness.input_sha256.starts_with("sha256:"));
        assert!(!harness.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{harness:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "lifemodel-memory",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "invocation harness leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_send_ready_keeps_selected_legacy_and_no_writes() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "send raw prompt should be metadata only";

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        assert!(proof.proof_ready);
        assert!(proof.send_message_result_compatible);
        assert!(proof.descriptor_ready);
        assert!(proof.contract_ready);
        assert!(proof.harness_ready);
        assert!(proof.metadata_safe);
        assert!(!proof.contains_raw_content);
        assert_eq!(
            proof.proof_kind,
            "default_chat_controlled_adapter_send_compatible_proof"
        );
        assert_eq!(proof.callsite_kind, "send_message");
        assert_eq!(proof.contract_shape, "send_message_compatible");
        assert_eq!(proof.selected_adapter_path, "legacy_stream");
        assert_eq!(proof.candidate_adapter_path, "controlled_adapter");
        assert_eq!(proof.required_callsite_path, "legacy_stream");
        assert_eq!(proof.actual_callsite_path, "legacy_stream");
        assert_eq!(proof.default_send_path, "legacy_stream");
        assert_eq!(proof.start_stream_path, "legacy_stream");
        assert!(!proof.controlled_adapter_enabled);
        assert!(!proof.automatic_migration_enabled);
        assert!(!proof.controlled_adapter_invocation_allowed);
        assert!(!proof.controlled_adapter_executor_enabled);
        assert!(!proof.controlled_adapter_executor_attached);
        assert_eq!(
            proof.controlled_adapter_executor_state,
            "disabled_unattached"
        );
        assert!(!proof.allow_writes);
        assert_eq!(proof.max_tool_calls, 0);
        assert!(proof.side_effect_budget_zero);
        assert!(!proof.runtime_call_enabled);
        assert!(!proof.model_call_enabled);
        assert!(!proof.tool_call_enabled);
        assert!(proof.business_write_disabled);
        assert!(!proof.migration_permission);
        assert!(!proof.chat_message_saved);
        assert!(!proof.agent_run_recorded);
        assert!(!proof.evidence_recorded);
        assert!(!proof.proposal_created);
        assert!(!proof.memory_written);
        assert!(!proof.life_model_written);
        assert!(!proof.external_write_recorded);
        assert!(proof.default_chat_unchanged);
        assert!(proof.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_send_compatible_proof(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
            raw_input,
        )
        .expect("clean send route should produce only a send-compatible proof");
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_stream_callsite_fails_closed() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "stream raw prompt should be metadata only",
            );

        assert!(!proof.proof_ready);
        assert!(!proof.send_message_result_compatible);
        assert!(proof.harness_ready);
        assert_eq!(proof.callsite_kind, "start_stream_message");
        assert_eq!(proof.contract_shape, "stream_message_compatible");
        assert_eq!(proof.selected_adapter_path, "legacy_stream");
        assert_eq!(proof.candidate_adapter_path, "controlled_adapter");
        assert!(!proof.controlled_adapter_invocation_allowed);
        assert!(!proof.controlled_adapter_executor_enabled);
        assert!(!proof.migration_permission);
        assert!(proof.default_chat_unchanged);
        assert!(proof
            .blocking_reasons
            .contains(&"callsite_not_send_message".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "stream raw prompt should be metadata only",
            )
            .expect_err("stream callsite must fail closed for send-compatible proof");
        assert!(error.contains("start_stream_message"));
        assert!(error.contains("send_compatible_proof_not_ready"));
        assert!(error.contains("callsite_not_send_message"));
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_fails_closed_for_route_drift_controlled_adapter_and_auto_migration(
    ) {
        let mut drift_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        drift_route.current_mode = "controlled_adapter".into();
        drift_route.default_send_path = "controlled_adapter".into();
        let drift_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &drift_route,
                "blocked raw prompt",
            );

        assert!(!drift_proof.proof_ready);
        assert!(!drift_proof.send_message_result_compatible);
        assert!(!drift_proof.default_chat_unchanged);
        assert_eq!(drift_proof.selected_adapter_path, "blocked");
        assert!(!drift_proof.migration_permission);
        assert!(drift_proof
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(drift_proof
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));

        let mut enabled_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        enabled_route.controlled_adapter_enabled = true;
        let enabled_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &enabled_route,
                "blocked raw prompt",
            );

        assert!(!enabled_proof.proof_ready);
        assert!(!enabled_proof.send_message_result_compatible);
        assert!(!enabled_proof.default_chat_unchanged);
        assert!(!enabled_proof.controlled_adapter_invocation_allowed);
        assert!(enabled_proof
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));

        let mut auto_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        auto_route.automatic_migration_enabled = true;
        let auto_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &auto_route,
                "blocked raw prompt",
            );

        assert!(!auto_proof.proof_ready);
        assert!(!auto_proof.send_message_result_compatible);
        assert!(!auto_proof.default_chat_unchanged);
        assert!(!auto_proof.migration_permission);
        assert!(auto_proof
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw-user-secret prompt-token assistant-output tool-payload LifeModel-memory memory-raw-content";

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
            );

        assert!(proof.metadata_safe);
        assert!(!proof.contains_raw_content);
        assert!(proof.input_sha256.starts_with("sha256:"));
        assert!(!proof.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{proof:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "LifeModel-memory",
            "memory-raw-content",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "send-compatible proof leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_send_compatible_proof_side_effect_budget_is_all_zero() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_send_compatible_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "metadata only",
            );

        assert!(proof.proof_ready);
        assert!(proof.side_effect_budget_zero);
        assert_eq!(proof.side_effect_budget.runtime_calls, 0);
        assert_eq!(proof.side_effect_budget.model_calls, 0);
        assert_eq!(proof.side_effect_budget.tool_calls, 0);
        assert_eq!(proof.side_effect_budget.store_writes, 0);
        assert_eq!(proof.side_effect_budget.chat_message_writes, 0);
        assert_eq!(proof.side_effect_budget.agent_run_writes, 0);
        assert_eq!(proof.side_effect_budget.evidence_writes, 0);
        assert_eq!(proof.side_effect_budget.proposal_writes, 0);
        assert_eq!(proof.side_effect_budget.memory_writes, 0);
        assert_eq!(proof.side_effect_budget.life_model_writes, 0);
        assert_eq!(proof.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(proof.side_effect_budget.external_writes, 0);
        assert!(!proof.runtime_call_enabled);
        assert!(!proof.model_call_enabled);
        assert!(!proof.tool_call_enabled);
        assert!(proof.business_write_disabled);
        assert!(!proof.chat_message_saved);
        assert!(!proof.agent_run_recorded);
        assert!(!proof.evidence_recorded);
        assert!(!proof.proposal_created);
        assert!(!proof.memory_written);
        assert!(!proof.life_model_written);
        assert!(!proof.external_write_recorded);
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_ready_keeps_selected_legacy_without_stream_or_writes(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "stream raw prompt should be metadata only";

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                raw_input,
            );

        assert!(proof.proof_ready);
        assert!(proof.stream_message_compatible);
        assert!(proof.descriptor_ready);
        assert!(proof.contract_ready);
        assert!(proof.harness_ready);
        assert!(proof.metadata_safe);
        assert!(!proof.contains_raw_content);
        assert_eq!(
            proof.proof_kind,
            "default_chat_controlled_adapter_stream_boundary_proof"
        );
        assert_eq!(proof.callsite_kind, "start_stream_message");
        assert_eq!(proof.contract_shape, "stream_message_compatible");
        assert_eq!(proof.selected_adapter_path, "legacy_stream");
        assert_eq!(proof.candidate_adapter_path, "controlled_adapter");
        assert_eq!(proof.required_callsite_path, "legacy_stream");
        assert_eq!(proof.actual_callsite_path, "legacy_stream");
        assert_eq!(proof.default_send_path, "legacy_stream");
        assert_eq!(proof.start_stream_path, "legacy_stream");
        assert!(!proof.controlled_adapter_enabled);
        assert!(!proof.automatic_migration_enabled);
        assert!(!proof.controlled_adapter_invocation_allowed);
        assert!(!proof.stream_started);
        assert!(!proof.stream_events_emitted);
        assert!(!proof.event_channel_opened);
        assert!(!proof.executor_enabled);
        assert!(!proof.executor_attached);
        assert_eq!(proof.executor_state, "disabled_unattached");
        assert!(!proof.allow_writes);
        assert_eq!(proof.max_tool_calls, 0);
        assert!(proof.side_effect_budget_zero);
        assert!(!proof.runtime_call_enabled);
        assert!(!proof.model_call_enabled);
        assert!(!proof.tool_call_enabled);
        assert!(proof.business_write_disabled);
        assert!(!proof.migration_permission);
        assert!(!proof.chat_message_saved);
        assert!(!proof.agent_run_recorded);
        assert!(!proof.evidence_recorded);
        assert!(!proof.proposal_created);
        assert!(!proof.memory_written);
        assert!(!proof.life_model_written);
        assert!(!proof.mcp_audit_written);
        assert!(!proof.external_write_recorded);
        assert!(proof.default_chat_unchanged);
        assert!(proof.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_stream_boundary_proof(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
            &route,
            raw_input,
        )
        .expect("clean stream route should produce only a stream-compatible boundary proof");
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_send_callsite_fails_closed() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "send raw prompt should be metadata only",
            );

        assert!(!proof.proof_ready);
        assert!(!proof.stream_message_compatible);
        assert!(proof.harness_ready);
        assert_eq!(proof.callsite_kind, "send_message");
        assert_eq!(proof.contract_shape, "send_message_compatible");
        assert_eq!(proof.selected_adapter_path, "legacy_stream");
        assert_eq!(proof.candidate_adapter_path, "controlled_adapter");
        assert!(!proof.controlled_adapter_invocation_allowed);
        assert!(!proof.stream_started);
        assert!(!proof.event_channel_opened);
        assert!(!proof.stream_events_emitted);
        assert!(!proof.migration_permission);
        assert!(proof.default_chat_unchanged);
        assert!(proof
            .blocking_reasons
            .contains(&"callsite_not_start_stream_message".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "send raw prompt should be metadata only",
            )
            .expect_err("send callsite must fail closed for stream boundary proof");
        assert!(error.contains("send_message"));
        assert!(error.contains("stream_boundary_proof_not_ready"));
        assert!(error.contains("callsite_not_start_stream_message"));
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_fails_closed_for_route_drift_controlled_adapter_and_auto_migration(
    ) {
        let mut drift_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        drift_route.current_mode = "controlled_adapter".into();
        drift_route.start_stream_path = "controlled_adapter".into();
        let drift_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &drift_route,
                "blocked raw prompt",
            );

        assert!(!drift_proof.proof_ready);
        assert!(!drift_proof.stream_message_compatible);
        assert!(!drift_proof.default_chat_unchanged);
        assert_eq!(drift_proof.selected_adapter_path, "blocked");
        assert!(!drift_proof.migration_permission);
        assert!(drift_proof
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(drift_proof
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));

        let mut enabled_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        enabled_route.controlled_adapter_enabled = true;
        let enabled_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &enabled_route,
                "blocked raw prompt",
            );

        assert!(!enabled_proof.proof_ready);
        assert!(!enabled_proof.stream_message_compatible);
        assert!(!enabled_proof.default_chat_unchanged);
        assert!(!enabled_proof.controlled_adapter_invocation_allowed);
        assert!(enabled_proof
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));

        let mut auto_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        auto_route.automatic_migration_enabled = true;
        let auto_proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &auto_route,
                "blocked raw prompt",
            );

        assert!(!auto_proof.proof_ready);
        assert!(!auto_proof.stream_message_compatible);
        assert!(!auto_proof.default_chat_unchanged);
        assert!(!auto_proof.migration_permission);
        assert!(auto_proof
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw-user-secret prompt-token assistant-output tool-payload LifeModel-memory memory-raw-content";

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                raw_input,
            );

        assert!(proof.metadata_safe);
        assert!(!proof.contains_raw_content);
        assert!(proof.input_sha256.starts_with("sha256:"));
        assert!(!proof.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{proof:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "LifeModel-memory",
            "memory-raw-content",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "stream boundary proof leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_stream_boundary_proof_side_effect_budget_is_all_zero_without_stream_emit(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let proof =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_stream_boundary_proof(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                "metadata only",
            );

        assert!(proof.proof_ready);
        assert!(proof.side_effect_budget_zero);
        assert_eq!(proof.side_effect_budget.runtime_calls, 0);
        assert_eq!(proof.side_effect_budget.model_calls, 0);
        assert_eq!(proof.side_effect_budget.tool_calls, 0);
        assert_eq!(proof.side_effect_budget.store_writes, 0);
        assert_eq!(proof.side_effect_budget.chat_message_writes, 0);
        assert_eq!(proof.side_effect_budget.agent_run_writes, 0);
        assert_eq!(proof.side_effect_budget.evidence_writes, 0);
        assert_eq!(proof.side_effect_budget.proposal_writes, 0);
        assert_eq!(proof.side_effect_budget.memory_writes, 0);
        assert_eq!(proof.side_effect_budget.life_model_writes, 0);
        assert_eq!(proof.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(proof.side_effect_budget.external_writes, 0);
        assert!(!proof.runtime_call_enabled);
        assert!(!proof.model_call_enabled);
        assert!(!proof.tool_call_enabled);
        assert!(proof.business_write_disabled);
        assert!(!proof.chat_message_saved);
        assert!(!proof.agent_run_recorded);
        assert!(!proof.evidence_recorded);
        assert!(!proof.proposal_created);
        assert!(!proof.memory_written);
        assert!(!proof.life_model_written);
        assert!(!proof.mcp_audit_written);
        assert!(!proof.external_write_recorded);
        assert!(!proof.stream_started);
        assert!(!proof.event_channel_opened);
        assert!(!proof.stream_events_emitted);
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_report_generates_under_clean_legacy_route_without_permissions(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "raw prompt should remain metadata only",
            );

        assert_eq!(
            report.report_kind,
            "default_chat_controlled_adapter_executor_attachment_gate_report"
        );
        assert!(report.gate_report_metadata_ready);
        assert!(report.executor_skeleton_discussion_ready);
        assert!(!report.executor_attachment_allowed);
        assert!(!report.executor_attached);
        assert!(!report.executor_enabled);
        assert!(!report.route_cutover_permission);
        assert!(!report.migration_permission);
        assert!(report.ordinary_default_chat_unchanged);
        assert_eq!(report.selected_adapter_path, "legacy_stream");
        assert!(!report.controlled_adapter_invocation_allowed);
        assert!(report.send_proof_ready);
        assert!(report.stream_boundary_proof_ready);
        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert!(!report.runtime_call_enabled);
        assert!(!report.model_call_enabled);
        assert!(!report.tool_call_enabled);
        assert!(!report.stream_started);
        assert!(!report.event_channel_opened);
        assert!(!report.stream_events_emitted);
        assert!(report.side_effect_budget_zero);
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_reuses_send_stream_and_metadata_safe_layers() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "metadata only",
            );

        assert!(report.send_proof_ready);
        assert!(report.send_message_result_compatible);
        assert!(report.stream_boundary_proof_ready);
        assert!(report.stream_message_compatible);
        assert!(report.send_descriptor_ready);
        assert!(report.send_contract_ready);
        assert!(report.send_harness_ready);
        assert!(report.stream_descriptor_ready);
        assert!(report.stream_contract_ready);
        assert!(report.stream_harness_ready);
        assert!(report.w65_w67_metadata_safe);
        assert!(report.w68_send_compatible_proof_ready);
        assert!(report.w69_stream_boundary_proof_ready);
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_fails_closed_for_route_drift_controlled_adapter_and_auto_migration(
    ) {
        let mut drift_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        drift_route.current_mode = "controlled_adapter".into();
        drift_route.default_send_path = "controlled_adapter".into();
        drift_route.start_stream_path = "controlled_adapter".into();
        let drift_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &drift_route,
                "blocked raw prompt",
            );

        assert!(!drift_report.gate_report_metadata_ready);
        assert!(!drift_report.executor_skeleton_discussion_ready);
        assert!(!drift_report.executor_attachment_allowed);
        assert!(!drift_report.migration_permission);
        assert!(!drift_report.ordinary_default_chat_unchanged);
        assert!(drift_report
            .blocking_reasons
            .contains(&"current_mode_not_legacy_stream".to_string()));
        assert!(drift_report
            .blocking_reasons
            .contains(&"default_send_path_not_legacy_stream".to_string()));
        assert!(drift_report
            .blocking_reasons
            .contains(&"start_stream_path_not_legacy_stream".to_string()));
        assert!(drift_report
            .blocking_reasons
            .contains(&"send_proof_not_ready".to_string()));
        assert!(drift_report
            .blocking_reasons
            .contains(&"stream_boundary_proof_not_ready".to_string()));

        let mut enabled_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        enabled_route.controlled_adapter_enabled = true;
        let enabled_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &enabled_route,
                "blocked raw prompt",
            );

        assert!(!enabled_report.gate_report_metadata_ready);
        assert!(!enabled_report.executor_attachment_allowed);
        assert!(!enabled_report.controlled_adapter_invocation_allowed);
        assert!(enabled_report
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));

        let mut auto_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        auto_route.automatic_migration_enabled = true;
        let auto_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &auto_route,
                "blocked raw prompt",
            );

        assert!(!auto_report.gate_report_metadata_ready);
        assert!(!auto_report.executor_attachment_allowed);
        assert!(!auto_report.migration_permission);
        assert!(auto_report
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_blocks_missing_executor_review_and_cutover_authorization(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "metadata only",
            );

        for blocker in [
            "executor_implementation_missing",
            "human_review_missing",
            "route_cutover_not_authorized",
        ] {
            assert!(
                report.blocking_reasons.contains(&blocker.to_string()),
                "expected W70 attachment blocker: {blocker}"
            );
        }
        assert!(!report.executor_attachment_allowed);
        assert!(!report.route_cutover_permission);

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "metadata only",
            )
            .expect_err("W70 must fail closed because it is not executor attachment");
        assert!(error.contains("executor_attachment_gate_not_ready"));
        assert!(error.contains("executor_implementation_missing"));
        assert!(error.contains("human_review_missing"));
        assert!(error.contains("route_cutover_not_authorized"));
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw-user-secret prompt-token assistant-output tool-payload LifeModel-memory memory-raw-content";

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );

        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert!(report.input_sha256.starts_with("sha256:"));
        assert!(!report.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{report:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "LifeModel-memory",
            "memory-raw-content",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "executor attachment gate report leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_executor_attachment_gate_side_effect_budget_is_all_zero() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "metadata only",
            );

        assert!(report.side_effect_budget_zero);
        assert_eq!(report.side_effect_budget.runtime_calls, 0);
        assert_eq!(report.side_effect_budget.model_calls, 0);
        assert_eq!(report.side_effect_budget.tool_calls, 0);
        assert_eq!(report.side_effect_budget.store_writes, 0);
        assert_eq!(report.side_effect_budget.chat_message_writes, 0);
        assert_eq!(report.side_effect_budget.agent_run_writes, 0);
        assert_eq!(report.side_effect_budget.evidence_writes, 0);
        assert_eq!(report.side_effect_budget.proposal_writes, 0);
        assert_eq!(report.side_effect_budget.memory_writes, 0);
        assert_eq!(report.side_effect_budget.life_model_writes, 0);
        assert_eq!(report.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(report.side_effect_budget.external_writes, 0);
        assert!(!report.runtime_call_enabled);
        assert!(!report.model_call_enabled);
        assert!(!report.tool_call_enabled);
        assert!(!report.chat_message_saved);
        assert!(!report.agent_run_recorded);
        assert!(!report.evidence_recorded);
        assert!(!report.proposal_created);
        assert!(!report.memory_written);
        assert!(!report.life_model_written);
        assert!(!report.mcp_audit_written);
        assert!(!report.external_write_recorded);
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_clean_legacy_route_is_metadata_safe_and_disabled(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw prompt should only become skeleton metadata";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(skeleton.skeleton_contract_ready);
        assert!(skeleton.metadata_safe);
        assert!(!skeleton.contains_raw_content);
        assert!(skeleton.executor_skeleton_present);
        assert!(!skeleton.executor_enabled);
        assert!(!skeleton.executor_attached);
        assert!(!skeleton.executor_runnable);
        assert!(!skeleton.invocation_allowed);
        assert!(!skeleton.route_cutover_permission);
        assert!(!skeleton.migration_permission);
        assert_eq!(skeleton.selected_adapter_path, "legacy_stream");
        assert!(skeleton.ordinary_default_chat_unchanged);
        assert!(skeleton.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_disabled_executor_skeleton(
            &input,
            &gate,
        )
        .expect("clean legacy metadata should satisfy the disabled skeleton contract");
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_send_shape_returns_send_result_placeholder()
    {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "send raw prompt should not appear in the skeleton";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(skeleton.skeleton_contract_ready);
        assert_eq!(
            skeleton.output.output_kind,
            "default_chat_controlled_adapter_disabled_executor_skeleton_output"
        );
        assert_eq!(skeleton.output.compatible_shape, "send_message_result");
        assert_eq!(skeleton.output.executor_state, "disabled_unattached");
        assert!(skeleton.output.no_user_visible_output);
        assert!(!skeleton.output.raw_output_present);
        assert!(skeleton.output.blocking_reasons.is_empty());
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_stream_shape_returns_stream_boundary_placeholder_without_stream_emit(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "stream raw prompt should not open event channels";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                raw_input,
                "stream_boundary",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(skeleton.skeleton_contract_ready);
        assert_eq!(skeleton.output.compatible_shape, "stream_boundary");
        assert_eq!(skeleton.output.executor_state, "disabled_unattached");
        assert!(skeleton.output.no_user_visible_output);
        assert!(!skeleton.output.raw_output_present);
        assert!(!skeleton.stream_started);
        assert!(!skeleton.event_channel_opened);
        assert!(!skeleton.stream_events_emitted);
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_unknown_shape_fails_closed() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "unknown shape raw prompt should not leak";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "unexpected_shape",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(!skeleton.skeleton_contract_ready);
        assert_eq!(skeleton.output.compatible_shape, "blocked");
        assert!(skeleton
            .blocking_reasons
            .contains(&"unknown_requested_shape".to_string()));
        assert!(skeleton
            .output
            .blocking_reasons
            .contains(&"unknown_requested_shape".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            )
            .expect_err("unknown shape must fail closed");
        assert!(error.contains("disabled_executor_skeleton_not_ready"));
        assert!(error.contains("unknown_requested_shape"));
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_route_drift_controlled_adapter_and_auto_migration_fail_closed(
    ) {
        let mut drift_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        drift_route.current_mode = "controlled_adapter".into();
        drift_route.default_send_path = "controlled_adapter".into();
        drift_route.start_stream_path = "controlled_adapter".into();
        let drift_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &drift_route,
                "drift raw prompt",
            );
        let drift_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &drift_route,
                "drift raw prompt",
                "send_message_result",
            );
        let drift_skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &drift_input,
                &drift_gate,
            );

        assert!(!drift_skeleton.skeleton_contract_ready);
        assert!(!drift_skeleton.ordinary_default_chat_unchanged);
        assert!(drift_skeleton
            .blocking_reasons
            .contains(&"w70_gate_report_not_metadata_ready".to_string()));
        assert!(drift_skeleton
            .blocking_reasons
            .contains(&"route_drift_from_legacy_stream".to_string()));
        assert!(!drift_skeleton.executor_enabled);
        assert!(!drift_skeleton.executor_attached);
        assert!(!drift_skeleton.invocation_allowed);

        let mut enabled_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        enabled_route.controlled_adapter_enabled = true;
        let enabled_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &enabled_route,
                "enabled raw prompt",
            );
        let enabled_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &enabled_route,
                "enabled raw prompt",
                "send_message_result",
            );
        let enabled_skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &enabled_input,
                &enabled_gate,
            );
        assert!(!enabled_skeleton.skeleton_contract_ready);
        assert!(enabled_skeleton
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));

        let mut auto_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        auto_route.automatic_migration_enabled = true;
        let auto_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &auto_route,
                "auto migration raw prompt",
            );
        let auto_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &auto_route,
                "auto migration raw prompt",
                "send_message_result",
            );
        let auto_skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &auto_input,
                &auto_gate,
            );
        assert!(!auto_skeleton.skeleton_contract_ready);
        assert!(!auto_skeleton.migration_permission);
        assert!(auto_skeleton
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw-user-secret prompt-token assistant-output tool-payload LifeModel-memory memory-raw-content";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(skeleton.metadata_safe);
        assert!(!skeleton.contains_raw_content);
        assert_eq!(skeleton.input_length_bytes, raw_input.len());
        assert_eq!(skeleton.input_length_chars, raw_input.chars().count());
        assert!(skeleton.input_sha256.starts_with("sha256:"));
        assert!(!skeleton.input_sha256.contains("raw-user-secret"));
        assert!(input.input_sha256.starts_with("sha256:"));
        assert!(!input.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{input:?} {skeleton:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "LifeModel-memory",
            "memory-raw-content",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "disabled executor skeleton leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_disabled_executor_skeleton_side_effect_budget_is_all_zero() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "metadata only",
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "metadata only",
                "send_message_result",
            );

        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(skeleton.side_effect_budget_zero);
        assert_eq!(skeleton.side_effect_budget.runtime_calls, 0);
        assert_eq!(skeleton.side_effect_budget.model_calls, 0);
        assert_eq!(skeleton.side_effect_budget.tool_calls, 0);
        assert_eq!(skeleton.side_effect_budget.store_writes, 0);
        assert_eq!(skeleton.side_effect_budget.chat_message_writes, 0);
        assert_eq!(skeleton.side_effect_budget.agent_run_writes, 0);
        assert_eq!(skeleton.side_effect_budget.evidence_writes, 0);
        assert_eq!(skeleton.side_effect_budget.proposal_writes, 0);
        assert_eq!(skeleton.side_effect_budget.memory_writes, 0);
        assert_eq!(skeleton.side_effect_budget.life_model_writes, 0);
        assert_eq!(skeleton.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(skeleton.side_effect_budget.external_writes, 0);
        assert!(!skeleton.runtime_call_enabled);
        assert!(!skeleton.model_call_enabled);
        assert!(!skeleton.tool_call_enabled);
        assert!(skeleton.business_write_disabled);
        assert!(!skeleton.chat_message_saved);
        assert!(!skeleton.agent_run_recorded);
        assert!(!skeleton.evidence_recorded);
        assert!(!skeleton.proposal_created);
        assert!(!skeleton.memory_written);
        assert!(!skeleton.life_model_written);
        assert!(!skeleton.mcp_audit_written);
        assert!(!skeleton.external_write_recorded);
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_clean_send_ready_but_no_run_no_permission_no_write(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "send binding raw prompt must remain metadata only";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert_eq!(
            report.report_kind,
            "default_chat_controlled_adapter_skeleton_binding_integrity_report"
        );
        assert!(report.binding_metadata_ready);
        assert!(report.binding_integrity_ready);
        assert!(report.skeleton_contract_ready);
        assert_eq!(report.callsite_kind, "send_message");
        assert_eq!(report.requested_shape, "send_message_result");
        assert_eq!(
            report.skeleton_output_compatible_shape,
            "send_message_result"
        );
        assert_eq!(report.selected_adapter_path, "legacy_stream");
        assert!(!report.executor_enabled);
        assert!(!report.executor_attached);
        assert!(!report.executor_runnable);
        assert!(!report.invocation_allowed);
        assert!(!report.route_cutover_permission);
        assert!(!report.migration_permission);
        assert!(!report.runtime_call_enabled);
        assert!(!report.model_call_enabled);
        assert!(!report.tool_call_enabled);
        assert!(report.business_write_disabled);
        assert!(report.side_effect_budget_zero);
        assert!(report.blocking_reasons.is_empty());

        crate::default_chat_adapter::ensure_default_chat_controlled_adapter_skeleton_binding_integrity(
            &input,
            &gate,
        )
        .expect("clean send binding metadata should be integrity-ready");
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_clean_stream_ready_but_no_stream_or_event_channel(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "stream binding raw prompt must not emit";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                raw_input,
                "stream_boundary",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(report.binding_integrity_ready);
        assert!(report.binding_metadata_ready);
        assert_eq!(report.callsite_kind, "start_stream_message");
        assert_eq!(report.requested_shape, "stream_boundary");
        assert_eq!(report.skeleton_output_compatible_shape, "stream_boundary");
        assert!(!report.stream_started);
        assert!(!report.event_channel_opened);
        assert!(!report.stream_events_emitted);
        assert!(!report.executor_runnable);
        assert!(!report.invocation_allowed);
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_input_gate_hash_mismatch_fails_closed() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                "gate prompt",
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                "different input prompt",
                "send_message_result",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(!report.binding_integrity_ready);
        assert!(!report.binding_metadata_ready);
        assert!(report
            .blocking_reasons
            .contains(&"input_gate_hash_mismatch".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"input_gate_length_mismatch".to_string()));

        let error =
            crate::default_chat_adapter::ensure_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            )
            .expect_err("mismatched input/gate metadata must fail closed");
        assert!(error.contains("skeleton_binding_integrity_not_ready"));
        assert!(error.contains("input_gate_hash_mismatch"));
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_route_metadata_mismatch_fails_closed() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "route metadata prompt";
        let mut gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        gate.default_send_path = "controlled_adapter".into();
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(!report.binding_integrity_ready);
        assert!(report
            .blocking_reasons
            .contains(&"route_metadata_mismatch".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"route_drift_from_legacy_stream".to_string()));
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_requested_shape_callsite_mismatch_fails_closed(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "callsite shape prompt";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let send_bound_to_stream =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "stream_boundary",
            );
        let stream_bound_to_send =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        for input in [send_bound_to_stream, stream_bound_to_send] {
            let report =
                crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                    &input,
                    &gate,
                );

            assert!(!report.binding_integrity_ready);
            assert!(!report.binding_metadata_ready);
            assert!(report
                .blocking_reasons
                .contains(&"requested_shape_callsite_mismatch".to_string()));
        }
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_unknown_shape_fails_closed_through_skeleton_and_binding(
    ) {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "unknown shape binding prompt";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "unknown_future_shape",
            );
        let skeleton =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
                &input,
                &gate,
            );

        assert!(!skeleton.skeleton_contract_ready);
        assert!(skeleton
            .blocking_reasons
            .contains(&"unknown_requested_shape".to_string()));

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(!report.binding_integrity_ready);
        assert!(!report.binding_metadata_ready);
        assert_eq!(report.skeleton_output_compatible_shape, "blocked");
        assert!(report
            .blocking_reasons
            .contains(&"skeleton_contract_not_ready".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"unknown_requested_shape".to_string()));
        assert!(report
            .blocking_reasons
            .contains(&"output_shape_mismatch".to_string()));
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_route_drift_controlled_adapter_and_auto_migration_fail_closed(
    ) {
        let mut drift_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        drift_route.current_mode = "controlled_adapter".into();
        drift_route.default_send_path = "controlled_adapter".into();
        drift_route.start_stream_path = "controlled_adapter".into();
        let drift_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &drift_route,
                "drift prompt",
            );
        let drift_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &drift_route,
                "drift prompt",
                "send_message_result",
            );
        let drift_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &drift_input,
                &drift_gate,
            );
        assert!(!drift_report.binding_integrity_ready);
        assert!(drift_report
            .blocking_reasons
            .contains(&"route_drift_from_legacy_stream".to_string()));

        let mut enabled_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        enabled_route.controlled_adapter_enabled = true;
        let enabled_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &enabled_route,
                "enabled prompt",
            );
        let enabled_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &enabled_route,
                "enabled prompt",
                "send_message_result",
            );
        let enabled_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &enabled_input,
                &enabled_gate,
            );
        assert!(!enabled_report.binding_integrity_ready);
        assert!(enabled_report
            .blocking_reasons
            .contains(&"controlled_adapter_enabled".to_string()));

        let mut auto_route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        auto_route.automatic_migration_enabled = true;
        let auto_gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &auto_route,
                "auto prompt",
            );
        let auto_input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &auto_route,
                "auto prompt",
                "send_message_result",
            );
        let auto_report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &auto_input,
                &auto_gate,
            );
        assert!(!auto_report.binding_integrity_ready);
        assert!(!auto_report.migration_permission);
        assert!(auto_report
            .blocking_reasons
            .contains(&"automatic_migration_enabled".to_string()));
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_debug_dump_omits_raw_content() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "raw-user-secret prompt-token assistant-output tool-payload LifeModel-memory memory-raw-content";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert_eq!(report.input_length_bytes, raw_input.len());
        assert_eq!(report.input_length_chars, raw_input.chars().count());
        assert!(report.input_sha256.starts_with("sha256:"));
        assert!(!report.input_sha256.contains("raw-user-secret"));

        let debug_dump = format!("{input:?} {report:?}");
        for forbidden in [
            "raw-user-secret",
            "prompt-token",
            "assistant-output",
            "tool-payload",
            "LifeModel-memory",
            "memory-raw-content",
        ] {
            assert!(
                !debug_dump.contains(forbidden),
                "skeleton binding integrity report leaked forbidden raw content: {forbidden}"
            );
        }
    }

    #[test]
    fn default_chat_adapter_skeleton_binding_integrity_side_effect_budget_is_all_zero() {
        let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
        let raw_input = "budget binding prompt";
        let gate =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_executor_attachment_gate(
                &route,
                raw_input,
            );
        let input =
            crate::default_chat_adapter::DefaultChatControlledAdapterExecutorSkeletonInput::from_request_metadata(
                crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
                &route,
                raw_input,
                "send_message_result",
            );

        let report =
            crate::default_chat_adapter::evaluate_default_chat_controlled_adapter_skeleton_binding_integrity(
                &input,
                &gate,
            );

        assert!(report.side_effect_budget_zero);
        assert_eq!(report.side_effect_budget.runtime_calls, 0);
        assert_eq!(report.side_effect_budget.model_calls, 0);
        assert_eq!(report.side_effect_budget.tool_calls, 0);
        assert_eq!(report.side_effect_budget.store_writes, 0);
        assert_eq!(report.side_effect_budget.chat_message_writes, 0);
        assert_eq!(report.side_effect_budget.agent_run_writes, 0);
        assert_eq!(report.side_effect_budget.evidence_writes, 0);
        assert_eq!(report.side_effect_budget.proposal_writes, 0);
        assert_eq!(report.side_effect_budget.memory_writes, 0);
        assert_eq!(report.side_effect_budget.life_model_writes, 0);
        assert_eq!(report.side_effect_budget.mcp_audit_writes, 0);
        assert_eq!(report.side_effect_budget.external_writes, 0);
        assert!(!report.runtime_call_enabled);
        assert!(!report.model_call_enabled);
        assert!(!report.tool_call_enabled);
        assert!(report.business_write_disabled);
        assert!(!report.chat_message_saved);
        assert!(!report.agent_run_recorded);
        assert!(!report.evidence_recorded);
        assert!(!report.proposal_created);
        assert!(!report.memory_written);
        assert!(!report.life_model_written);
        assert!(!report.mcp_audit_written);
        assert!(!report.external_write_recorded);
    }
}
