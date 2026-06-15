use std::collections::HashMap;
use std::sync::Arc;

use openlife_core::agent::{ReasoningTrace, StreamingCallback};
use openlife_core::layer_router::Layer;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::privacy::PrivacyEngine;
use openlife_core::scheduler::InferenceScheduler;
use tauri::{Emitter, State};

use crate::legacy_write_convergence::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::main_chat_conversation_updates::try_auto_checkin_daily_goals;
use crate::main_chat_generation_support::{
    finalize_chat_agent_run, generate_non_stream_fallback, preview_text,
};
use crate::main_chat_hs_runtime::build_chat_runtime_hs_packet;
use crate::main_chat_preprocess::preprocess_chat_input_v2;
use crate::main_chat_react_runtime::agent_actions_to_tool_call_results;
use crate::{persist_life_model, AppState, SendMessageResult, ToolCallResult};

/// AgentLoop-based chat execution for explicit non-default / controlled paths only.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) async fn send_message_with_agent_loop(
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
    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    let calendar_ics_paths = cfg.system.calendar_ics_paths.clone();
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    let action_executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let loop_config = openlife_core::agent::AgentLoopConfig {
        max_steps: cfg.system.agent_loop_max_steps,
        max_tool_calls: cfg.system.agent_loop_max_tool_calls,
        timeout_seconds: cfg.system.agent_loop_timeout_seconds,
        allow_writes: true,
        allow_cloud: true,
        shutdown_notify: Some(state.inner().shutdown_notify.clone()),
        ..Default::default()
    };
    let agent_loop = openlife_core::agent::AgentLoop::new(
        agent_runtime,
        action_executor,
        scheduler.clone(),
        loop_config,
    );

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.clone(),
        user_text: user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages: desensitized_messages.clone(),
        layer,
    };

    let hs_packet =
        build_chat_runtime_hs_packet(state.inner(), &task, &life_model, &tools_prompt, None)
            .await?;
    let network_policy = cfg.system.network_policy.clone();

    let loop_result = {
        let (reg, audit) = state.get_mcp_state().await;
        let permission_store = state.tool_permission_store.lock().await;
        let memory_store = state.memory_store.lock().await;
        let proposal_store_guard = if let Some(ref store) = state.proposal_store {
            Some(store.lock().await)
        } else {
            None
        };
        let agent_run_store_guard = if let Some(ref store) = state.agent_run_store {
            Some(store.lock().await)
        } else {
            None
        };
        let mut action_ctx = openlife_core::agent::ActionExecutionContext::new(
            &reg,
            &permission_store,
            &audit,
            &privacy_engine,
            &safe_paths,
        )
        .with_life_model(&life_model)
        .with_memory_store(&memory_store)
        .with_calendar_ics_paths(&calendar_ics_paths)
        .with_network_policy(&network_policy);
        if let Some(ref store) = proposal_store_guard {
            action_ctx = action_ctx.with_proposal_store(store);
        }
        if let Some(ref store) = agent_run_store_guard {
            action_ctx = action_ctx.with_agent_run_store(store);
        }
        if let Some(ref packet) = hs_packet {
            action_ctx = action_ctx.with_hs_runtime_packet(packet);
        }

        agent_loop
            .run(
                &task,
                &life_model,
                &tools_prompt,
                None,
                privacy_engine.clone(),
                &action_ctx,
            )
            .await
    };

    let (mut reply, mut agent_run, _status_updates) = match loop_result {
        Ok(result) => {
            // Emit AgentLoop status updates as Tauri events.
            for update in &result.status_updates {
                emit_agent_status_update(
                    &app_handle,
                    &session_id,
                    &result.run.id,
                    &update.phase.to_string(),
                    &update.message,
                    update.step_index,
                    update.tool_call_index,
                );
            }
            (result.final_response, result.run, result.status_updates)
        }
        Err(e) => {
            eprintln!(
                "[warn] AgentLoop failed in send_message, falling back to legacy: {}",
                e
            );
            let user_input_text = user_msg
                .as_ref()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            let (fallback_reply, agent_run) = handle_agent_loop_fallback(
                &scheduler,
                desensitized_messages.clone(),
                &life_model,
                &tools_prompt,
                &session_id,
                &user_input_text,
                state.agent_run_store.as_ref(),
                &e.to_string(),
                hs_packet.clone(),
            )
            .await?;

            return Ok(SendMessageResult {
                reply: fallback_reply,
                reasoning_trace: ReasoningTrace::default(),
                tool_calls: Vec::new(),
                run_id: Some(agent_run.id),
                agent_ingress: None,
                execution_transcript: Vec::new(),
                legacy_fallback_used: true,
            });
        }
    };

    // Store status updates in Tauri events for real-time UI state.

    reply = privacy_engine.reconstruct(&reply, &privacy_map);

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

    finalize_chat_agent_run(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        &state,
    )
    .await?;

    let tool_calls = agent_actions_to_tool_call_results(&agent_run.actions, &agent_run.id);

    Ok(SendMessageResult {
        reply,
        reasoning_trace,
        tool_calls,
        run_id: Some(agent_run.id.clone()),
        agent_ingress: None,
        execution_transcript: Vec::new(),
        legacy_fallback_used: false,
    })
}

/// Emit a unified agent-status-update event for both streaming and non-streaming paths.
/// Frontend AgentStateIndicator expects: phase, message, step_index, tool_call_index, timestamp.
#[allow(dead_code)]
pub(crate) fn emit_agent_status_update(
    app_handle: &tauri::AppHandle,
    session_id: &str,
    run_id: &str,
    phase: &str,
    message: &str,
    step_index: u32,
    tool_call_index: Option<u32>,
) {
    let _ = app_handle.emit(
        "agent-status-update",
        serde_json::json!({
            "session_id": session_id,
            "run_id": run_id,
            "phase": phase,
            "message": message,
            "step_index": step_index,
            "tool_call_index": tool_call_index,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }),
    );
}

/// Handle AgentLoop failure: try non-stream fallback, create AgentRun with
/// error context, persist the run. Returns (reply, agent_run) on success, or
/// an error message string if both AgentLoop and fallback fail.
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
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
    original_error: &str,
    hs_packet: Option<openlife_core::agent::RuntimeHSPacket>,
) -> Result<(String, openlife_core::agent::AgentRun), String> {
    let fallback_reply =
        generate_non_stream_fallback(scheduler, messages, life_model, tools_prompt, hs_packet)
            .await
            .map_err(|fallback_err| {
                format!(
                    "AgentLoop failed: {}. Fallback also failed: {}",
                    original_error, fallback_err
                )
            })?;

    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(session_id, user_input_text);
    agent_run.status = openlife_core::agent::AgentRunStatus::Completed;
    agent_run.output_preview = Some(preview_text(&fallback_reply, 200));
    agent_run
        .warnings
        .push(format!("fallback: agent_loop_error: {}", original_error));
    agent_run.finished_at = Some(chrono::Utc::now());

    if let Some(store_arc) = agent_run_store {
        let store = store_arc.lock().await;
        let _ = store.create_run(&agent_run);
    }

    Ok((fallback_reply, agent_run))
}

/// Streaming callback that forwards AgentLoop events to Tauri frontend via emit().
#[allow(dead_code)]
struct TauriStreamingCallback {
    app_handle: tauri::AppHandle,
    session_id: String,
    run_id: String,
}

#[async_trait::async_trait]
impl StreamingCallback for TauriStreamingCallback {
    async fn on_chunk(&self, chunk: &str, _step: u32, _phase: &str) {
        let _ = self.app_handle.emit(
            "stream-message-chunk",
            serde_json::json!({
                "session_id": self.session_id,
                "run_id": self.run_id,
                "chunk": chunk,
            }),
        );
    }

    async fn on_tool_start(&self, tool_name: &str, _step: u32) {
        let _ = self.app_handle.emit(
            "tool-start",
            serde_json::json!({
                "session_id": self.session_id,
                "run_id": self.run_id,
                "tool_name": tool_name,
                "phase": "executing_tool",
            }),
        );
    }

    async fn on_tool_result(&self, tool_name: &str, success: bool, _step: u32) {
        let _ = self.app_handle.emit(
            "tool-result",
            serde_json::json!({
                "session_id": self.session_id,
                "run_id": self.run_id,
                "tool_name": tool_name,
                "success": success,
                "phase": "observing",
            }),
        );
    }

    async fn on_proposal(&self, proposal_type: &str, proposal_id: &str) {
        let _ = self.app_handle.emit(
            "proposal-created",
            serde_json::json!({
                "session_id": self.session_id,
                "run_id": self.run_id,
                "proposal_type": proposal_type,
                "proposal_id": proposal_id,
            }),
        );
    }

    async fn on_status(&self, status: &str, message: &str, step: u32) {
        emit_agent_status_update(
            &self.app_handle,
            &self.session_id,
            &self.run_id,
            status,
            message,
            step,
            None,
        );
    }
}

/// Stream-mode AgentLoop execution: runs AgentLoop and emits real token-level stream events.
/// This provides consistency when use_agent_loop=true in stream mode.
#[allow(dead_code)]
pub(crate) async fn start_stream_message_with_agent_loop(
    session_id: String,
    messages: Vec<ChatMessage>,
    user_msg: Option<ChatMessage>,
    _layer: Layer,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Non-persisted placeholder id for pre-run errors. The stream start/done
    // events use the authoritative AgentLoop run id after execution.
    let user_input_text = user_msg
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let placeholder_run_id =
        openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text).id;

    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        _context_summary,
    ) = match preprocess_chat_input_v2(&session_id, &messages, &state).await {
        Ok(result) => result,
        Err(message) => return Err(message),
    };

    let auto_checkin_msg = if let Some(ref m) = user_msg {
        let msg = try_auto_checkin_daily_goals(&m.content, &mut life_model);
        if msg.is_some() {
            let _ = persist_life_model(
                &state.inner().clone(),
                life_model.clone(),
                false,
                LifeModelMaterializerCallerContext::new(
                    "ordinary_stream_agent_loop_auto_checkin_source_data",
                    LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
                    LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
                ),
            )
            .await;
        }
        msg
    } else {
        None
    };

    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let safe_paths = cfg.system.safe_paths.clone();
    let calendar_ics_paths = cfg.system.calendar_ics_paths.clone();
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg);
    let action_executor = openlife_core::agent::ActionExecutor::new(
        openlife_core::agent::ActionExecutorConfig::default(),
    );
    let loop_config = openlife_core::agent::AgentLoopConfig {
        max_steps: cfg.system.agent_loop_max_steps,
        max_tool_calls: cfg.system.agent_loop_max_tool_calls,
        timeout_seconds: cfg.system.agent_loop_timeout_seconds,
        allow_writes: true,
        allow_cloud: true,
        shutdown_notify: Some(state.inner().shutdown_notify.clone()),
        ..Default::default()
    };
    let agent_loop = openlife_core::agent::AgentLoop::new(
        agent_runtime,
        action_executor,
        scheduler.clone(),
        loop_config,
    );

    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.clone(),
        user_text: user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages: desensitized_messages.clone(),
        layer: _layer,
    };

    let hs_packet =
        build_chat_runtime_hs_packet(state.inner(), &task, &life_model, &tools_prompt, None)
            .await?;
    let network_policy = cfg.system.network_policy.clone();

    let callback = Arc::new(TauriStreamingCallback {
        app_handle: app_handle.clone(),
        session_id: session_id.clone(),
        run_id: placeholder_run_id.clone(),
    });

    let _ = app_handle.emit(
        "stream-message-start",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": placeholder_run_id,
            "reasoning_trace": ReasoningTrace::default(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );

    let loop_result = {
        let (reg, audit) = state.get_mcp_state().await;
        let permission_store = state.tool_permission_store.lock().await;
        let memory_store = state.memory_store.lock().await;
        let proposal_store_guard = if let Some(ref store) = state.proposal_store {
            Some(store.lock().await)
        } else {
            None
        };
        let agent_run_store_guard = if let Some(ref store) = state.agent_run_store {
            Some(store.lock().await)
        } else {
            None
        };
        let mut action_ctx = openlife_core::agent::ActionExecutionContext::new(
            &reg,
            &permission_store,
            &audit,
            &privacy_engine,
            &safe_paths,
        )
        .with_life_model(&life_model)
        .with_memory_store(&memory_store)
        .with_calendar_ics_paths(&calendar_ics_paths)
        .with_network_policy(&network_policy);
        if let Some(ref store) = proposal_store_guard {
            action_ctx = action_ctx.with_proposal_store(store);
        }
        if let Some(ref store) = agent_run_store_guard {
            action_ctx = action_ctx.with_agent_run_store(store);
        }
        if let Some(ref packet) = hs_packet {
            action_ctx = action_ctx.with_hs_runtime_packet(packet);
        }

        agent_loop
            .run_streaming(
                &task,
                &life_model,
                &tools_prompt,
                None,
                privacy_engine.clone(),
                &action_ctx,
                callback,
            )
            .await
    };

    let (mut reply, mut agent_run) = match loop_result {
        Ok(result) => (result.final_response, result.run),
        Err(e) => {
            eprintln!(
                "[warn] AgentLoop streaming failed, falling back to legacy: {}",
                e
            );
            let user_input_txt = user_msg
                .as_ref()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            let (fallback_reply, agent_run) = match handle_agent_loop_fallback(
                &scheduler,
                desensitized_messages.clone(),
                &life_model,
                &tools_prompt,
                &session_id,
                &user_input_txt,
                state.agent_run_store.as_ref(),
                &e.to_string(),
                hs_packet.clone(),
            )
            .await
            {
                Ok(result) => result,
                Err(error_msg) => {
                    let _ = app_handle.emit(
                        "stream-message-error",
                        serde_json::json!({
                            "session_id": &session_id,
                            "run_id": placeholder_run_id,
                            "error": error_msg.clone(),
                        }),
                    );
                    return Err(error_msg);
                }
            };

            let _ = app_handle.emit(
                "stream-message-chunk",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": placeholder_run_id,
                    "chunk": fallback_reply,
                }),
            );

            (fallback_reply, agent_run)
        }
    };

    reply = privacy_engine.reconstruct(&reply, &privacy_map);

    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    let mut reasoning_trace = agent_run.reasoning_trace.clone().unwrap_or_default();
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    let result = finalize_chat_agent_run(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        &state,
    )
    .await;

    match result {
        Ok(_) => {
            let tool_calls = agent_actions_to_tool_call_results(&agent_run.actions, &agent_run.id);
            let _ = app_handle.emit(
                "stream-message-done",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": agent_run.id,
                    "reply": reply,
                    "reasoning_trace": reasoning_trace,
                    "tool_calls": tool_calls,
                }),
            );
            Ok(())
        }
        Err(e) => {
            let _ = app_handle.emit(
                "stream-message-error",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": agent_run.id,
                    "error": e,
                }),
            );
            Ok(())
        }
    }
}
