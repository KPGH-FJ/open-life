use crate::streaming::{
    emit_agent_status_update, generate_non_stream_fallback_governed, start_stream_message,
};
use openlife_core::agent::ReasoningTrace;
use openlife_core::layer_router::Layer;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

pub mod a2a_server;
pub mod a2a_sidecar;
pub(crate) mod auto_checkin;
pub mod bootstrap;
pub(crate) mod chat_persistence;
pub(crate) mod chat_preprocess;
pub mod commands;
pub(crate) mod conversation_signals;
pub mod errors;
pub mod execution_deps;
pub mod execution_facade;
pub(crate) mod memory_utils;
pub mod scheduler_runner;
pub mod state;
pub mod storage;
pub(crate) mod streaming;
pub mod types;
pub(crate) mod window;

#[cfg(test)]
pub mod test_utils;

pub use state::AppState;

// Re-exports for test modules (imported as crate::...)
use commands::a2a::{
    a2a_bridge_local, a2a_discover_agent, a2a_handle_task, a2a_local_agent_card,
    a2a_restart_sidecar, a2a_send_task, a2a_stop_sidecar,
};
use commands::agent::{
    delete_agent_run, get_agent_run, list_agent_run_events, list_agent_runs,
    list_agent_runs_for_session, replay_agent_action, restore_agent_run,
};
use commands::agent_spec::{
    get_agent_spec, get_default_agent_spec, list_agent_specs, set_default_agent_spec,
    update_agent_spec,
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
    check_ollama_status, get_router_status, get_scheduler_config, get_system_diagnostics,
    set_scheduler_config,
};
use commands::execution::{
    check_tool_permission, disable_plugin, enable_plugin, get_skill_run_status,
    grant_tool_permission, list_plugins, list_skills, list_tool_permissions, reload_plugins,
    revoke_tool_permission, run_skill,
};
use commands::feedback::{
    apply_feedback_evolution, generate_evolution_report, get_feedback_summary, log_analytics_event,
    save_feedback,
};
use commands::plan::{
    cancel_agent_plan, confirm_agent_plan, continue_agent_plan, edit_agent_plan,
    execute_agent_plan, get_agent_plan, list_agent_plans_for_run, list_agent_plans_for_session,
    reject_agent_plan, retry_agent_plan,
};
pub use openlife_core::memory_cache::HotMemoryCache;
pub use openlife_core::memory_cache::SharedHotCache;
pub use openlife_core::privacy::PrivacyEngine;
// Hermes module removed: replaced by AgentRuntime
use auto_checkin::run_auto_checkin_and_stream_signals;
use chat_persistence::{
    finalize_chat_agent_run, finalize_chat_agent_run_inner, persist_chat_message_if_needed,
    persist_life_model, persist_vector_memory_for_message,
};
use chat_preprocess::{preprocess_chat_input, preprocess_chat_input_v2};
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
    accept_proposal, batch_accept_low_risk_proposals, edit_proposal, get_pending_proposals,
    list_proposals, postpone_proposal, reject_proposal,
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
use conversation_signals::capture_conversation_signals;
pub(crate) use memory_utils::merge_memory_hits;
use storage::app_data_dir;
use types::agent_actions_to_tool_call_results;
use types::preview_text;
pub use types::{
    BuilderCompletion, SendMessageResult, SystemDiagnostics, ToolCallResult, ToolCallStatus,
};
use window::ensure_main_window_visible;

#[tauri::command]
async fn send_message(
    session_id: String,
    messages: Vec<ChatMessage>,
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<SendMessageResult, String> {
    let user_msg = messages.last().cloned();
    let intent = if let Some(ref m) = user_msg {
        if m.role == "user" {
            let router = state.intent_router.lock().await;
            Some(router.classify(&m.content))
        } else {
            None
        }
    } else {
        None
    };

    let layer = if let (Some(ref i), Some(ref m)) = (&intent, &user_msg) {
        let lr = state.layer_router.lock().await;
        lr.resolve(i, &m.content)
    } else {
        Layer::L2
    };

    // Layer 1: direct reflex response
    if layer == Layer::L1 {
        if let Some(ref i) = intent {
            if let Some(reply) = i.direct_response() {
                // Persist user message
                if let Some(ref user) = user_msg {
                    if user.role == "user" {
                        let inserted =
                            persist_chat_message_if_needed(&session_id, user, &state).await?;
                        if inserted {
                            persist_vector_memory_for_message(&session_id, user, &state).await;
                        }
                    }
                }
                let assistant_msg = ChatMessage {
                    role: "assistant".into(),
                    content: reply.clone(),
                };

                // Create and finalize AgentRun for L1
                let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(
                    &session_id,
                    &user_msg
                        .as_ref()
                        .map(|m| m.content.clone())
                        .unwrap_or_default(),
                );
                let model_route = openlife_core::agent::ModelRouteTrace {
                    provider: "direct".to_string(),
                    model: "L1_reflex".to_string(),
                    route_type: "direct".to_string(),
                    prefer_local: false,
                    local_model: "".to_string(),
                    reason: "layer_1_direct_response".to_string(),
                    privacy_level: openlife_core::agent::types::RedactionLevel::None,
                    latency_ms: None,
                    retry_count: 0,
                    fallback_reason: None,
                    provider_health_is_estimated: Some(false),
                };
                let context_summary = openlife_core::agent::ContextSummary {
                    life_model_empty: false,
                    included_life_model_sections: vec![],
                    memory_hit_count: 0,
                    memory_sources: vec![],
                    used_tools_prompt: false,
                    redaction_applied: false,
                    redaction_level: openlife_core::agent::types::RedactionLevel::None,
                };
                agent_run.complete(&preview_text(&reply, 200), model_route, context_summary);
                let life_model = {
                    let manager = state.life_model_manager.lock().await;
                    manager
                        .load()
                        .map_err(|e| format!("人生模型加载失败: {}", e))?
                };
                let mut reasoning_trace = ReasoningTrace::default();
                finalize_chat_agent_run(
                    &session_id,
                    &assistant_msg,
                    &reply,
                    &mut reasoning_trace,
                    &mut agent_run,
                    &life_model,
                    &state,
                )
                .await?;

                return Ok(SendMessageResult {
                    reply,
                    reasoning_trace,
                    tool_calls: vec![],
                    run_id: Some(agent_run.id.clone()),
                });
            }
        }
    }

    // Gradual rollout: use v2 if experimental flag is enabled
    let use_v2 = {
        let cfg = state.config.lock().await;
        cfg.experimental_context_assembler
    };

    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        _context_summary,
    ) = if use_v2 {
        preprocess_chat_input_v2(&session_id, &messages, &state).await?
    } else {
        preprocess_chat_input(&session_id, &messages, &state).await?
    };

    let auto_checkin_msg =
        run_auto_checkin_and_stream_signals(&user_msg, &mut life_model, &session_id, &state, None)
            .await?;

    return send_message_with_agent_loop(
        session_id,
        messages,
        user_msg,
        life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        auto_checkin_msg,
        layer,
        state,
        app_handle,
    )
    .await;
}

/// AgentLoop-based chat execution (primary path).
#[allow(clippy::too_many_arguments)]
async fn send_message_with_agent_loop(
    session_id: String,
    _messages: Vec<ChatMessage>,
    user_msg: Option<ChatMessage>,
    life_model: LifeModel,
    tools_prompt: String,
    privacy_engine: PrivacyEngine,
    privacy_map: HashMap<String, String>,
    desensitized_messages: Vec<ChatMessage>,
    embed_err: Option<String>,
    auto_checkin_msg: Option<String>,
    layer: Layer,
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<SendMessageResult, String> {
    send_message_with_agent_loop_inner(
        session_id,
        user_msg,
        life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        auto_checkin_msg,
        layer,
        state.inner(),
        Some(&app_handle),
    )
    .await
}

/// Inner AgentLoop-based chat execution path, kept AppHandle-optional so tests can
/// exercise production control flow without constructing a Tauri runtime.
#[allow(clippy::too_many_arguments)]
async fn send_message_with_agent_loop_inner(
    session_id: String,
    user_msg: Option<ChatMessage>,
    life_model: LifeModel,
    tools_prompt: String,
    privacy_engine: PrivacyEngine,
    privacy_map: HashMap<String, String>,
    desensitized_messages: Vec<ChatMessage>,
    embed_err: Option<String>,
    auto_checkin_msg: Option<String>,
    layer: Layer,
    state: &Arc<AppState>,
    app_handle: Option<&tauri::AppHandle>,
) -> Result<SendMessageResult, String> {
    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let runtime_assembly = execution_facade::build_runtime_assembly_config(
        &cfg,
        execution_facade::TauriAgentExecutionMode::Chat,
        state.shutdown_notify.clone(),
    );
    let agent_loop = execution_facade::build_governed_agent_loop(
        life_model.clone(),
        scheduler.clone(),
        &cfg,
        &runtime_assembly,
        &state.agent_run_event_store,
    );
    drop(cfg);

    let task = execution_deps::build_agent_task(
        openlife_core::agent::AgentTaskKind::Conversation,
        session_id.clone(),
        user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        desensitized_messages.clone(),
        layer,
    );

    // ── Resolve AgentSpec — fail closed, no fallback ──────────────────
    let agent_spec = match crate::commands::agent_spec::resolve_required_agent_spec(
        &state.agent_spec_store,
        None,
    )
    .await
    {
        Ok(spec) => spec,
        Err(e) => {
            log::error!("[AgentSpec] non-stream resolution failed: {}", e);
            return Err(format!("AgentSpec resolution failed: {}", e));
        }
    };
    let prompt_registry = execution_facade::build_prompt_registry();

    let action_ctx = execution_facade::build_governed_action_context(
        state,
        &runtime_assembly,
        Some(life_model.clone()),
        Some(state.memory_store.clone()),
        agent_spec.clone(),
    );

    let execution_input = execution_facade::TauriAgentExecutionInput {
        mode: execution_facade::TauriAgentExecutionMode::Chat,
        task,
        life_model: life_model.clone(),
        tools_prompt: tools_prompt.clone(),
        privacy_engine: privacy_engine.clone(),
        agent_spec: Some(agent_spec.clone()),
        prompt_registry: Some(prompt_registry),
        streaming_callback: None,
    };

    let execution_outcome =
        execution_facade::run_tauri_agent_task(&agent_loop, &action_ctx, execution_input).await;

    let (mut reply, mut agent_run, _status_updates) = match execution_outcome {
        Ok(outcome) => {
            // Emit AgentLoop status updates as Tauri events
            for update in &outcome.status_updates {
                if let Some(app_handle) = app_handle {
                    emit_agent_status_update(
                        app_handle,
                        &session_id,
                        &outcome.run.id,
                        &update.phase.to_string(),
                        &update.message,
                        update.step_index,
                        update.tool_call_index,
                    );
                }
            }
            (outcome.reply, outcome.run, outcome.status_updates)
        }
        Err(e) => {
            let user_input_text = user_msg
                .as_ref()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            let fallback_outcome = handle_execution_facade_chat_error_branch(&e, |decision| {
                let scheduler = &scheduler;
                let desensitized_messages = desensitized_messages.clone();
                let life_model = &life_model;
                let tools_prompt = &tools_prompt;
                let session_id = &session_id;
                let user_input_text = &user_input_text;
                let agent_run_store = state.agent_run_store.as_ref();
                let agent_run_event_store = state.agent_run_event_store.as_ref();
                let original_error = e.to_string();
                let privacy_policy = agent_spec.privacy_policy;
                async move {
                    if let Some(warning) = decision.warning_message.as_deref() {
                        eprintln!("[warn] {}", warning);
                    }
                    handle_agent_loop_fallback(
                        scheduler,
                        desensitized_messages,
                        life_model,
                        tools_prompt,
                        session_id,
                        user_input_text,
                        agent_run_store,
                        agent_run_event_store,
                        &original_error,
                        privacy_policy,
                    )
                    .await
                }
            })
            .await?;

            return Ok(SendMessageResult {
                reply: fallback_outcome.reply,
                reasoning_trace: ReasoningTrace::default(),
                tool_calls: Vec::new(),
                run_id: Some(fallback_outcome.agent_run.id),
            });
        }
    };

    // Store status_updates in agent_run for persistence
    // (This requires adding a field to AgentRun, which we'll skip for now
    // and just use the Tauri events for real-time UI updates)

    // Apply privacy reconstruction
    reply = privacy_engine.reconstruct(&reply, &privacy_map);

    // Apply auto checkin message
    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    let mut reasoning_trace = agent_run.reasoning_trace.clone().unwrap_or_default();
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }

    finalize_chat_agent_run_inner(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        state,
    )
    .await?;

    let tool_calls = agent_actions_to_tool_call_results(&agent_run.actions, &agent_run.id);

    Ok(SendMessageResult {
        reply,
        reasoning_trace,
        tool_calls,
        run_id: Some(agent_run.id.clone()),
    })
}

/// Handle AgentLoop failure: try non-stream fallback, create AgentRun with
/// error context, persist the run. Returns (reply, agent_run) on success, or
/// an error message string if both AgentLoop and fallback fail.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_agent_loop_fallback(
    scheduler: &InferenceScheduler,
    messages: Vec<ChatMessage>,
    life_model: &LifeModel,
    tools_prompt: &str,
    session_id: &str,
    user_input_text: &str,
    agent_run_store: Option<
        &std::sync::Arc<tokio::sync::Mutex<openlife_core::agent::AgentRunStore>>,
    >,
    event_store: Option<&std::sync::Arc<openlife_core::agent::event_store::AgentRunEventStore>>,
    original_error: &str,
    privacy_policy: openlife_core::agent::types::PrivacyPolicy,
) -> Result<(String, openlife_core::agent::AgentRun), String> {
    // Create AgentRun first so fallback.started has a real run_id
    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(session_id, user_input_text);
    let run_id = agent_run.id.clone();

    if let Some(es) = event_store {
        let ev = openlife_core::agent::AgentRunEvent::new(
            &run_id,
            openlife_core::agent::AgentRunEventType::FallbackStarted,
            openlife_core::agent::AgentEventActor::Runtime,
            format!("AgentLoop failed, attempting fallback: {}", original_error),
            serde_json::json!({"error": original_error}),
        );
        if let Err(e) = es.append_event(&ev) {
            log::error!("[AgentRun] Failed to append event: {}", e);
        }
    }
    let fallback_reply = generate_non_stream_fallback_governed(
        scheduler,
        messages,
        life_model,
        tools_prompt,
        privacy_policy,
    )
    .await
    .map_err(|fallback_err| {
        format!(
            "AgentLoop failed: {}. Fallback also failed: {}",
            original_error, fallback_err
        )
    })?;

    agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    agent_run.output_preview = Some(preview_text(&fallback_reply, 200));
    agent_run
        .warnings
        .push(agent_loop_fallback_warning(original_error));
    agent_run.finished_at = Some(chrono::Utc::now());

    if let Some(store_arc) = agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(&agent_run) {
            log::error!("[AgentRun] 创建运行记录失败: {}", e);
        }
    }

    if let Some(es) = event_store {
        let ev = openlife_core::agent::AgentRunEvent::new(
            &agent_run.id,
            openlife_core::agent::AgentRunEventType::FallbackCompleted,
            openlife_core::agent::AgentEventActor::Runtime,
            "Fallback generation completed",
            serde_json::json!({"reply_len": fallback_reply.len()}),
        );
        if let Err(e) = es.append_event(&ev) {
            log::error!("[AgentRun] Failed to append event: {}", e);
        }
    }

    Ok((fallback_reply, agent_run))
}

fn should_fallback_from_execution_facade_error(
    error: &execution_facade::TauriExecutionFacadeError,
) -> bool {
    error.is_runtime()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChatExecutionFacadeErrorDecision {
    should_fallback: bool,
    error_message: Option<String>,
    warning_message: Option<String>,
}

#[derive(Debug)]
struct ChatExecutionFacadeFallbackOutcome {
    reply: String,
    agent_run: openlife_core::agent::AgentRun,
}

fn agent_loop_fallback_warning(original_error: &str) -> String {
    format!("fallback: agent_loop_error: {}", original_error)
}

fn chat_execution_facade_error_decision(
    error: &execution_facade::TauriExecutionFacadeError,
) -> ChatExecutionFacadeErrorDecision {
    if should_fallback_from_execution_facade_error(error) {
        ChatExecutionFacadeErrorDecision {
            should_fallback: true,
            error_message: None,
            warning_message: Some(format!(
                "AgentLoop failed in send_message, falling back to legacy: {}",
                error
            )),
        }
    } else {
        ChatExecutionFacadeErrorDecision {
            should_fallback: false,
            error_message: Some(error.to_string()),
            warning_message: None,
        }
    }
}

async fn handle_execution_facade_chat_error_branch<F, Fut>(
    error: &execution_facade::TauriExecutionFacadeError,
    fallback: F,
) -> Result<ChatExecutionFacadeFallbackOutcome, String>
where
    F: FnOnce(ChatExecutionFacadeErrorDecision) -> Fut,
    Fut: std::future::Future<Output = Result<(String, openlife_core::agent::AgentRun), String>>,
{
    let decision = chat_execution_facade_error_decision(error);
    if !decision.should_fallback {
        return Err(decision.error_message.unwrap_or_else(|| error.to_string()));
    }

    let (reply, agent_run) = fallback(decision).await?;
    Ok(ChatExecutionFacadeFallbackOutcome { reply, agent_run })
}

#[tauri::command]
async fn execute_tool_call(
    name: String,
    arguments: serde_json::Value,
    state: State<'_, Arc<AppState>>,
) -> Result<ToolCallResult, String> {
    execute_tool_call_inner(name, arguments, state.inner()).await
}

async fn execute_tool_call_inner(
    name: String,
    arguments: serde_json::Value,
    state: &Arc<AppState>,
) -> Result<ToolCallResult, String> {
    let cfg = state.config.lock().await.clone();
    let runtime_assembly = execution_facade::build_runtime_assembly_config(
        &cfg,
        execution_facade::TauriAgentExecutionMode::ToolExecution,
        state.shutdown_notify.clone(),
    );
    let network_policy = runtime_assembly.network_policy.clone();

    let agent_spec =
        execution_facade::resolve_default_agent_spec_fail_closed(&state.agent_spec_store).await?;
    let mut action_ctx = execution_facade::build_governed_action_context(
        state,
        &runtime_assembly,
        None,
        None,
        agent_spec.clone(),
    );
    action_ctx.proposal_store = None;
    action_ctx.calendar_ics_paths = Vec::new();
    let outcome = execution_facade::run_tauri_direct_tool_execution(
        execution_facade::TauriDirectToolExecutionInput {
            name,
            arguments,
            action_ctx,
            agent_spec,
            network_policy,
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(outcome.tool_result)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = app_data_dir();
    let bootstrap = match bootstrap::bootstrap(data_dir.clone()) {
        Ok(b) => b,
        Err(fatal_msg) => {
            // Show a native dialog before exiting so the user sees what went wrong.
            eprintln!("[startup] FATAL: {}", fatal_msg);
            // On macOS, try to show a dialog via osascript as a last resort.
            #[cfg(target_os = "macos")]
            {
                let escaped = fatal_msg.replace('"', "'");
                let _ = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(format!(
                        "display dialog \"OpenLife 启动失败:\\n\\n{}\" buttons {{\"确定\"}} default button 1 with icon stop",
                        escaped
                    ))
                    .output();
            }
            std::process::exit(1);
        }
    };
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
                let sandbox_path = app_data_dir().join("mcp-filesystem-sandbox");
                tauri::async_runtime::spawn(async move {
                    let mut registry = mcp_registry.lock().await;
                    let sandbox_str = sandbox_path.to_string_lossy().to_string();
                    log::info!("[setup] autostart filesystem MCP with sandbox path: {}", sandbox_str);
                    if let Err(e) = std::fs::create_dir_all(&sandbox_path) {
                        log::warn!(
                            "[setup] failed to create MCP filesystem sandbox dir: {} - {}",
                            sandbox_str,
                            e
                        );
                    }
                    if let Err(e) = registry.register(
                        "filesystem",
                        "npx",
                        &["-y", "@modelcontextprotocol/server-filesystem", &sandbox_str],
                    )
                    .await
                    {
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
            list_agent_run_events,
            list_agent_runs_for_session,
            delete_agent_run,
            restore_agent_run,
            replay_agent_action,
            get_agent_spec,
            list_agent_specs,
            get_default_agent_spec,
            update_agent_spec,
            set_default_agent_spec,
            get_agent_plan,
            list_agent_plans_for_run,
            list_agent_plans_for_session,
            confirm_agent_plan,
            reject_agent_plan,
            cancel_agent_plan,
            execute_agent_plan,
            retry_agent_plan,
            continue_agent_plan,
            edit_agent_plan,
            get_pending_proposals,
            list_proposals,
            batch_accept_low_risk_proposals,
            accept_proposal,
            reject_proposal,
            edit_proposal,
            postpone_proposal,
            send_message,
            start_stream_message,
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
            tauri::RunEvent::Ready | tauri::RunEvent::Reopen { .. } => {
                if let Err(e) = ensure_main_window_visible(app_handle) {
                    log::warn!("[runtime] failed to show main window: {}", e);
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_checkin::try_auto_checkin_daily_goals;
    use crate::types::{included_life_model_sections, preview_text};

    fn make_test_life_model() -> LifeModel {
        let mut lm = LifeModel::default();
        lm.goals.daily = vec![openlife_core::life_model::DailyGoal {
            name: "运动30分钟".to_string(),
            done: false,
            time_block: None,
        }];
        lm
    }

    #[test]
    fn test_auto_checkin_triggers_on_match() {
        let mut lm = make_test_life_model();
        let result = try_auto_checkin_daily_goals("我今天完成了运动30分钟", &mut lm);
        assert!(result.is_some());
        assert!(lm.goals.daily[0].done);
    }

    #[test]
    fn test_auto_checkin_no_match() {
        let mut lm = make_test_life_model();
        let result = try_auto_checkin_daily_goals("今天天气真好", &mut lm);
        assert!(result.is_none());
        assert!(!lm.goals.daily[0].done);
    }

    #[test]
    fn test_auto_checkin_multiple_triggers() {
        let triggers = ["我完成了", "我搞定了", "已经打卡了"];
        for trigger in triggers {
            let mut lm = make_test_life_model();
            let result = try_auto_checkin_daily_goals(&format!("{trigger}运动30分钟"), &mut lm);
            assert!(result.is_some(), "trigger '{trigger}' should match");
            assert!(lm.goals.daily[0].done);
        }
    }

    #[test]
    fn test_auto_checkin_partial_match() {
        let mut lm = make_test_life_model();
        let result = try_auto_checkin_daily_goals("我今天完成了运动", &mut lm);
        // "运动" is a partial match of "运动30分钟" — depends on contains logic
        assert!(!lm.goals.daily[0].done || result.is_some());
    }

    #[test]
    fn test_preview_text_truncates() {
        assert_eq!(preview_text("hello", 3), "hel");
        assert_eq!(preview_text("hi", 200), "hi");
        assert_eq!(preview_text("", 10), "");
    }

    #[test]
    fn test_included_life_model_sections() {
        let mut lm = LifeModel::default();
        lm.identity.name = "Test".to_string();
        let sections = included_life_model_sections(&lm);
        assert_eq!(sections.len(), 4);
        assert!(sections.contains(&"identity".to_string()));
    }

    #[test]
    fn test_included_life_model_sections_empty() {
        let lm = LifeModel::default();
        let sections = included_life_model_sections(&lm);
        // Default LifeModel may or may not be effectively empty depending on default field values
        if lm.is_effectively_empty() {
            assert!(sections.is_empty());
        } else {
            assert!(!sections.is_empty());
        }
    }

    #[tokio::test]
    async fn send_message_with_agent_loop_missing_agentspec_fails_closed_without_fallback() {
        let state = crate::test_utils::test_app_state();
        {
            let store = state.agent_spec_store.lock().await;
            store.set_active("main.default", false).unwrap();
        }

        let user_msg = ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        };

        let result = send_message_with_agent_loop_inner(
            "session-1".to_string(),
            Some(user_msg.clone()),
            LifeModel::default(),
            String::new(),
            PrivacyEngine::new(),
            HashMap::new(),
            vec![user_msg],
            None,
            None,
            Layer::L2,
            &state,
            None,
        )
        .await;

        let err = match result {
            Ok(_) => panic!("missing AgentSpec must fail closed"),
            Err(err) => err,
        };
        assert!(
            err.contains("AgentSpec resolution failed"),
            "unexpected error: {err}"
        );

        let runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs(10, 0).unwrap()
        };
        assert!(
            runs.is_empty(),
            "missing AgentSpec must not create fallback runs: {runs:?}"
        );
        assert!(
            runs.iter().all(|run| !run
                .warnings
                .iter()
                .any(|warning| warning.contains("fallback"))),
            "missing AgentSpec must not write fallback warnings: {runs:?}"
        );

        let event_store = state.agent_run_event_store.as_ref().unwrap();
        assert_eq!(
            event_store
                .count_events_by_type(openlife_core::agent::AgentRunEventType::FallbackStarted)
                .unwrap(),
            0,
            "missing AgentSpec must not record FallbackStarted"
        );
        assert_eq!(
            event_store
                .count_events_by_type(openlife_core::agent::AgentRunEventType::FallbackCompleted)
                .unwrap(),
            0,
            "missing AgentSpec must not record FallbackCompleted"
        );
    }

    #[tokio::test]
    async fn non_stream_chat_governance_error_returns_without_fallback() {
        let state = crate::test_utils::test_app_state();
        let governance =
            crate::execution_facade::TauriExecutionFacadeError::governance("AgentSpec mismatch");

        let fallback_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let err = handle_execution_facade_chat_error_branch(&governance, {
            let fallback_calls = fallback_calls.clone();
            let run_store = state.agent_run_store.as_ref().unwrap().clone();
            move |_decision| async move {
                fallback_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut run = openlife_core::agent::AgentRun::new_chat_run("session-1", "hello");
                run.warnings
                    .push(agent_loop_fallback_warning("should not be called"));
                {
                    let store = run_store.lock().await;
                    store.create_run(&run).unwrap();
                }
                Ok(("fallback reply".to_string(), run))
            }
        })
        .await
        .expect_err("Governance errors must fail closed");
        assert!(
            err.contains("AgentSpec mismatch"),
            "Governance error should preserve original message: {err}"
        );
        assert_eq!(
            fallback_calls.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "Governance errors must not call the fallback branch"
        );

        let runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs(10, 0).unwrap()
        };
        assert!(
            runs.is_empty(),
            "Governance errors must not create fallback AgentRuns: {runs:?}"
        );
        assert!(
            runs.iter().all(|run| !run
                .warnings
                .iter()
                .any(|warning| warning.contains("fallback"))),
            "Governance errors must not persist fallback warnings: {runs:?}"
        );
    }

    #[tokio::test]
    async fn non_stream_chat_runtime_error_still_falls_back() {
        let runtime = crate::execution_facade::TauriExecutionFacadeError::runtime("model failed");

        let decision = chat_execution_facade_error_decision(&runtime);
        assert!(decision.should_fallback);
        assert!(decision.error_message.is_none());
        assert!(
            decision
                .warning_message
                .as_deref()
                .is_some_and(|warning| warning.contains("model failed")
                    && warning.contains("falling back to legacy")),
            "Runtime fallback warning should preserve the original error: {decision:?}"
        );

        let fallback_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let outcome = handle_execution_facade_chat_error_branch(&runtime, {
            let fallback_calls = fallback_calls.clone();
            move |_decision| async move {
                fallback_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut run = openlife_core::agent::AgentRun::new_chat_run("session-1", "hello");
                run.warnings.push(agent_loop_fallback_warning(
                    "ExecutionFacade Runtime error: model failed",
                ));
                Ok(("fallback reply".to_string(), run))
            }
        })
        .await
        .expect("Runtime errors should call fallback");

        assert_eq!(
            fallback_calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "Runtime errors must still call the fallback branch"
        );
        assert_eq!(outcome.reply, "fallback reply");
        assert!(
            outcome
                .agent_run
                .warnings
                .iter()
                .any(|warning| warning.contains("fallback: agent_loop_error")
                    && warning.contains("model failed")),
            "Runtime fallback should preserve fallback warning semantics: {:?}",
            outcome.agent_run.warnings
        );
    }

    #[tokio::test]
    async fn execute_tool_call_requires_agent_spec() {
        let state = crate::test_utils::test_app_state();
        {
            let store = state.agent_spec_store.lock().await;
            store.set_active("main.default", false).unwrap();
        }

        let result =
            execute_tool_call_inner("goal.read".into(), serde_json::json!({}), &state).await;

        let err = match result {
            Ok(_) => panic!("direct tool execution must fail closed"),
            Err(err) => err,
        };
        assert!(
            err.contains("AgentSpec resolution failed"),
            "unexpected error: {}",
            err
        );
        let runs = {
            let store = state.agent_run_store.as_ref().unwrap().lock().await;
            store.list_runs(10, 0).unwrap()
        };
        assert!(
            runs.is_empty(),
            "tool execution must not create an AgentRun when AgentSpec resolution fails"
        );
    }

    #[test]
    fn execute_tool_call_uses_direct_tool_facade() {
        let source = include_str!("lib.rs");
        let start = source
            .find("async fn execute_tool_call_inner")
            .expect("direct tool command helper should exist");
        let end = source[start..]
            .find("#[tauri::command]\nasync fn inspect_mcp_call")
            .map(|offset| start + offset)
            .expect("inspect_mcp_call should follow execute_tool_call_inner");
        let direct_tool_path = &source[start..end];
        let direct_execute_call = [".", "execute("].concat();

        assert!(
            direct_tool_path.contains("run_tauri_direct_tool_execution"),
            "execute_tool_call_inner must call the Tauri direct tool facade wrapper"
        );
        assert!(
            !direct_tool_path.contains("ActionExecutor::new"),
            "execute_tool_call_inner must not construct ActionExecutor directly"
        );
        assert!(
            !direct_tool_path.contains(&direct_execute_call),
            "execute_tool_call_inner must not call ActionExecutor::execute directly"
        );
    }
}
