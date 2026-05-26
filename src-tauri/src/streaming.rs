use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use tauri::Emitter;
use tauri::State;
use tokio::time::{timeout, Duration};

use openlife_core::agent::trace_payloads;
use openlife_core::agent::types::{PrivacyPolicy, RedactionLevel};
use openlife_core::agent::{
    AgentEventActor, AgentRun, AgentRunError, AgentRunEvent, AgentRunEventType, AgentRunStore,
    AgentTask, AgentTaskKind, ContextSummary, ModelRouteTrace, ReasoningTrace, StreamingCallback,
};
use openlife_core::layer_router::Layer;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::scheduler::InferenceScheduler;

use crate::auto_checkin::run_auto_checkin_and_stream_signals;
use crate::chat_persistence::{
    finalize_chat_agent_run, persist_chat_message_if_needed, persist_vector_memory_for_message,
};
use crate::commands::agent_spec::resolve_required_agent_spec;
use crate::execution_deps;
use crate::state::AppState;
use crate::types::{
    agent_actions_to_tool_call_results, included_life_model_sections, preview_text, ToolCallResult,
};

// ── Timeout constants ──────────────────────────────────────────────────────

const STREAM_INIT_TIMEOUT_SECS: u64 = 45;
const STREAM_CHUNK_TIMEOUT_SECS: u64 = 90;
const NON_STREAM_FALLBACK_TIMEOUT_SECS: u64 = 120;

// ── Structs ────────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone, Debug)]
pub struct StartStreamMessageArgs {
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
}

/// Streaming callback that forwards AgentLoop events to Tauri frontend via emit().
struct TauriStreamingCallback {
    app_handle: tauri::AppHandle,
    session_id: String,
    run_id: String,
}

// ── Callback trait impl ────────────────────────────────────────────────────

#[async_trait]
impl StreamingCallback for TauriStreamingCallback {
    async fn on_chunk(&self, chunk: &str, _step: u32, _phase: &str) {
        let _ = self.app_handle.emit(
            "stream-message-chunk",
            json!({
                "session_id": self.session_id,
                "run_id": self.run_id,
                "chunk": chunk,
            }),
        );
    }

    async fn on_tool_start(&self, tool_name: &str, _step: u32) {
        let _ = self.app_handle.emit(
            "tool-start",
            json!({
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
            json!({
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
            json!({
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

// ── Event emitters ─────────────────────────────────────────────────────────

/// Emit a unified agent-status-update event for both streaming and non-streaming paths.
/// Frontend AgentStateIndicator expects: phase, message, step_index, tool_call_index, timestamp.
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
        json!({
            "session_id": session_id,
            "run_id": run_id,
            "phase": phase,
            "message": message,
            "step_index": step_index,
            "tool_call_index": tool_call_index,
            "timestamp": Utc::now().to_rfc3339(),
        }),
    );
}

fn emit_stream_error(
    app_handle: &tauri::AppHandle,
    session_id: &str,
    run_id: &str,
    error: impl Into<String>,
) {
    let _ = app_handle.emit(
        "stream-message-error",
        json!({
            "session_id": session_id,
            "run_id": run_id,
            "error": error.into(),
        }),
    );
}

// ── L1 direct stream handler ───────────────────────────────────────────────

/// Handle L1 direct reflex response in streaming mode — persist user message,
/// emit stream events, finalize AgentRun. Returns early via Ok(()) if a direct
/// response was handled.
async fn handle_l1_direct_stream_response(
    app_handle: &tauri::AppHandle,
    session_id: &str,
    reply: String,
    user_msg: &Option<ChatMessage>,
    state: &State<'_, Arc<AppState>>,
    agent_run: &mut AgentRun,
) -> Result<(), String> {
    if let Some(ref user) = user_msg {
        if user.role == "user" {
            let user_inserted = persist_chat_message_if_needed(session_id, user, state).await?;
            if user_inserted {
                persist_vector_memory_for_message(session_id, user, state).await;
            }
        }
    }

    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(|e| e.to_string())?
    };

    let assistant_msg = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    let _ = app_handle.emit(
        "stream-message-start",
        json!({
            "session_id": session_id,
            "run_id": agent_run.id,
            "reasoning_trace": ReasoningTrace::default(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );
    let _ = app_handle.emit(
        "stream-message-chunk",
        json!({
            "session_id": session_id,
            "run_id": agent_run.id,
            "chunk": reply.clone(),
        }),
    );

    let mut reasoning_trace = ReasoningTrace::default();
    let model_route = ModelRouteTrace {
        provider: "direct".to_string(),
        model: "L1_reflex".to_string(),
        route_type: "direct".to_string(),
        prefer_local: false,
        local_model: "".to_string(),
        reason: "layer_1_direct_response".to_string(),
        privacy_level: RedactionLevel::None,
        latency_ms: None,
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(false),
    };
    let context_summary = ContextSummary {
        life_model_empty: true,
        included_life_model_sections: vec![],
        memory_hit_count: 0,
        memory_sources: vec![],
        used_tools_prompt: false,
        redaction_applied: false,
        redaction_level: RedactionLevel::None,
    };
    let preview = preview_text(&reply, 200);
    agent_run.complete(&preview, model_route, context_summary);

    if let Err(e) = finalize_chat_agent_run(
        session_id,
        &assistant_msg,
        &reply,
        &mut reasoning_trace,
        agent_run,
        &life_model,
        state,
    )
    .await
    {
        log::warn!("[L1 Stream] finalize_chat_agent_run failed: {}", e);
        let _ = app_handle.emit(
            "stream-message-error",
            json!({
                "session_id": session_id,
                "run_id": agent_run.id,
                "error": format!("AgentRun 持久化失败: {}", e),
            }),
        );
        return Err(e);
    }

    let _ = app_handle.emit(
        "stream-message-done",
        json!({
            "session_id": session_id,
            "run_id": agent_run.id,
            "reply": reply,
            "reasoning_trace": ReasoningTrace::default(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );
    Ok(())
}

// ── Governed streaming + fallback ──────────────────────────────────────────

/// Run governed streaming with four fallback scenarios.
/// On success, full_reply and reasoning_trace.errors are populated.
/// On total failure, agent_run is marked failed and the error is returned.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_governed_stream_with_fallbacks(
    scheduler: &InferenceScheduler,
    messages_with_reasoning: Vec<ChatMessage>,
    life_model: &LifeModel,
    tools_prompt: &str,
    privacy_policy: PrivacyPolicy,
    app_handle: &tauri::AppHandle,
    session_id: &str,
    existing_reply: String,
    reasoning_trace: &mut ReasoningTrace,
    agent_run: &mut AgentRun,
    agent_run_store: Option<&Arc<tokio::sync::Mutex<AgentRunStore>>>,
) -> Result<String, String> {
    let mut full_reply = existing_reply;

    let mut stream = match timeout(
        Duration::from_secs(STREAM_INIT_TIMEOUT_SECS),
        scheduler.generate_stream_governed(
            messages_with_reasoning.clone(),
            life_model,
            Some(tools_prompt),
            privacy_policy,
        ),
    )
    .await
    .map_err(|_| format!("流式响应初始化超时（{} 秒）", STREAM_INIT_TIMEOUT_SECS))
    .and_then(|result| result.map_err(|e| e.to_string()))
    {
        Ok(s) => s,
        Err(stream_error) => {
            match generate_non_stream_fallback_governed(
                scheduler,
                messages_with_reasoning,
                life_model,
                tools_prompt,
                privacy_policy,
            )
            .await
            {
                Ok(reply) => {
                    reasoning_trace.errors.push(format!(
                        "流式响应初始化失败，已降级为非流式响应：{}",
                        stream_error
                    ));
                    full_reply = reply.clone();
                    let _ = app_handle.emit(
                        "stream-message-chunk",
                        json!({
                            "session_id": session_id,
                            "chunk": reply,
                        }),
                    );
                }
                Err(fallback_error) => {
                    let message = format!(
                        "流式响应初始化失败，非流式重试也失败：{}；重试错误：{}",
                        stream_error, fallback_error
                    );
                    emit_stream_error(app_handle, session_id, &agent_run.id, message.clone());
                    let error = AgentRunError {
                        message: message.clone(),
                        phase: "stream".to_string(),
                        recoverable: true,
                    };
                    agent_run.fail(error);
                    if let Some(store_arc) = agent_run_store {
                        let store = store_arc.lock().await;
                        if let Err(e) = store.update_run(agent_run) {
                            log::warn!("[AgentRun] 更新运行记录失败: {}", e);
                        }
                    }
                    return Err(message);
                }
            }
            return Ok(full_reply);
        }
    };

    loop {
        let next_chunk = match timeout(
            Duration::from_secs(STREAM_CHUNK_TIMEOUT_SECS),
            stream.next(),
        )
        .await
        {
            Ok(next) => next,
            Err(_) => {
                let stream_error = format!("超过 {} 秒没有收到模型输出", STREAM_CHUNK_TIMEOUT_SECS);
                match generate_non_stream_fallback_governed(
                    scheduler,
                    messages_with_reasoning.clone(),
                    life_model,
                    tools_prompt,
                    privacy_policy,
                )
                .await
                {
                    Ok(reply) => {
                        let fallback_text = if full_reply.is_empty() {
                            reply
                        } else {
                            format!(
                                "\n\n[系统] 流式连接长时间无输出，已自动用非流式请求重试并补全回复：\n\n{}",
                                reply
                            )
                        };
                        reasoning_trace.errors.push(format!(
                            "流式响应超时，已降级为非流式响应：{}",
                            stream_error
                        ));
                        full_reply.push_str(&fallback_text);
                        let _ = app_handle.emit(
                            "stream-message-chunk",
                            json!({
                                "session_id": session_id,
                                "chunk": fallback_text,
                            }),
                        );
                        break;
                    }
                    Err(fallback_error) => {
                        let message = format!(
                            "流式响应超时，非流式重试也失败：{}；重试错误：{}",
                            stream_error, fallback_error
                        );
                        emit_stream_error(app_handle, session_id, &agent_run.id, message.clone());
                        let error = AgentRunError {
                            message: message.clone(),
                            phase: "stream".to_string(),
                            recoverable: true,
                        };
                        agent_run.fail(error);
                        if let Some(store_arc) = agent_run_store {
                            let store = store_arc.lock().await;
                            if let Err(e) = store.update_run(agent_run) {
                                log::warn!("[AgentRun] 更新运行记录失败: {}", e);
                            }
                        }
                        return Err(message);
                    }
                }
            }
        };

        let Some(chunk_result) = next_chunk else {
            break;
        };

        match chunk_result {
            Ok(chunk) => {
                if !chunk.is_empty() {
                    full_reply.push_str(&chunk);
                    let _ = app_handle.emit(
                        "stream-message-chunk",
                        json!({
                            "session_id": session_id,
                            "chunk": chunk,
                        }),
                    );
                }
            }
            Err(e) => {
                let stream_error = e.to_string();
                match generate_non_stream_fallback_governed(
                    scheduler,
                    messages_with_reasoning.clone(),
                    life_model,
                    tools_prompt,
                    privacy_policy,
                )
                .await
                {
                    Ok(reply) => {
                        let fallback_text = if full_reply.is_empty() {
                            reply
                        } else {
                            format!(
                                "\n\n[系统] 流式连接中断，已自动用非流式请求重试并补全回复：\n\n{}",
                                reply
                            )
                        };
                        reasoning_trace.errors.push(format!(
                            "流式响应中断，已降级为非流式响应：{}",
                            stream_error
                        ));
                        full_reply.push_str(&fallback_text);
                        let _ = app_handle.emit(
                            "stream-message-chunk",
                            json!({
                                "session_id": session_id,
                                "chunk": fallback_text,
                            }),
                        );
                        break;
                    }
                    Err(fallback_error) => {
                        let message = format!(
                            "流式响应失败，非流式重试也失败：{}；重试错误：{}",
                            stream_error, fallback_error
                        );
                        emit_stream_error(app_handle, session_id, &agent_run.id, message.clone());
                        let error = AgentRunError {
                            message: message.clone(),
                            phase: "stream".to_string(),
                            recoverable: true,
                        };
                        agent_run.fail(error);
                        if let Some(store_arc) = agent_run_store {
                            let store = store_arc.lock().await;
                            if let Err(e) = store.update_run(agent_run) {
                                log::warn!("[AgentRun] 更新运行记录失败: {}", e);
                            }
                        }
                        return Err(message);
                    }
                }
            }
        }
    }

    // Post-loop: empty stream fallback
    if full_reply.trim().is_empty() {
        let stream_error = "流式响应已结束，但没有收到可显示内容".to_string();
        match generate_non_stream_fallback_governed(
            scheduler,
            messages_with_reasoning.clone(),
            life_model,
            tools_prompt,
            privacy_policy,
        )
        .await
        {
            Ok(reply) => {
                reasoning_trace.errors.push(format!(
                    "流式响应为空，已降级为非流式响应：{}",
                    stream_error
                ));
                full_reply = reply.clone();
                let _ = app_handle.emit(
                    "stream-message-chunk",
                    json!({
                        "session_id": session_id,
                        "chunk": reply,
                    }),
                );
            }
            Err(fallback_error) => {
                let message = format!(
                    "流式响应为空，非流式重试也失败：{}；重试错误：{}",
                    stream_error, fallback_error
                );
                emit_stream_error(app_handle, session_id, &agent_run.id, message.clone());
                let error = AgentRunError {
                    message: message.clone(),
                    phase: "stream".to_string(),
                    recoverable: true,
                };
                agent_run.fail(error);
                if let Some(store_arc) = agent_run_store {
                    let store = store_arc.lock().await;
                    if let Err(e) = store.update_run(agent_run) {
                        log::warn!("[AgentRun] 更新运行记录失败: {}", e);
                    }
                }
                return Err(message);
            }
        }
    }

    Ok(full_reply)
}

pub(crate) async fn generate_non_stream_fallback_governed(
    scheduler: &InferenceScheduler,
    messages: Vec<ChatMessage>,
    life_model: &LifeModel,
    tools_prompt: &str,
    privacy_policy: PrivacyPolicy,
) -> Result<String, String> {
    timeout(
        Duration::from_secs(NON_STREAM_FALLBACK_TIMEOUT_SECS),
        scheduler.generate_governed(messages, life_model, Some(tools_prompt), privacy_policy),
    )
    .await
    .map_err(|_| {
        format!(
            "非流式重试超时（{} 秒），请检查模型服务或切换后端。",
            NON_STREAM_FALLBACK_TIMEOUT_SECS
        )
    })?
    .map_err(|e| e.to_string())
}

// ── AgentLoop streaming ────────────────────────────────────────────────────

/// Stream-mode AgentLoop execution: runs AgentLoop and emits real token-level stream events.
/// This provides consistency when use_agent_loop=true in stream mode.
async fn start_stream_message_with_agent_loop(
    session_id: String,
    messages: Vec<ChatMessage>,
    user_msg: Option<ChatMessage>,
    _layer: Layer,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let user_input_text = user_msg
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let placeholder_run_id = AgentRun::new_chat_run(&session_id, &user_input_text).id;

    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        _context_summary,
    ) = match crate::chat_preprocess::preprocess_chat_input_v2(&session_id, &messages, &state).await
    {
        Ok(result) => result,
        Err(message) => return Err(message),
    };

    let auto_checkin_msg =
        run_auto_checkin_and_stream_signals(&user_msg, &mut life_model, &session_id, &state, None)
            .await?;

    let agent_spec = match resolve_required_agent_spec(&state.agent_spec_store, None).await {
        Ok(spec) => spec,
        Err(e) => {
            log::error!("[AgentSpec] AgentLoop stream resolution failed: {}", e);
            emit_stream_error(
                &app_handle,
                &session_id,
                &placeholder_run_id,
                format!("AgentSpec resolution failed: {}", e),
            );
            return Err(format!("AgentSpec resolution failed: {}", e));
        }
    };
    let agent_spec_id = agent_spec.id.clone();
    let prompt_registry = crate::execution_facade::build_prompt_registry();
    let fallback_prompt_registry = crate::execution_facade::build_prompt_registry();

    let scheduler = state.scheduler.lock().await.clone();
    let cfg = state.config.lock().await;
    let runtime_assembly = crate::execution_facade::build_runtime_assembly_config(
        &cfg,
        crate::execution_facade::TauriAgentExecutionMode::StreamChat,
        state.inner().shutdown_notify.clone(),
    );
    let agent_loop = crate::execution_facade::build_governed_agent_loop(
        life_model.clone(),
        scheduler.clone(),
        &cfg,
        &runtime_assembly,
        &state.agent_run_event_store,
    );
    drop(cfg);

    let task = execution_deps::build_agent_task(
        AgentTaskKind::Conversation,
        session_id.clone(),
        user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        desensitized_messages.clone(),
        _layer,
    );

    let callback = Arc::new(TauriStreamingCallback {
        app_handle: app_handle.clone(),
        session_id: session_id.clone(),
        run_id: placeholder_run_id.clone(),
    });

    let _ = app_handle.emit(
        "stream-message-start",
        json!({
            "session_id": &session_id,
            "run_id": placeholder_run_id,
            "agent_spec_id": agent_spec_id,
            "reasoning_trace": ReasoningTrace::default(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );

    let action_ctx = crate::execution_facade::build_governed_action_context(
        state.inner(),
        &runtime_assembly,
        Some(life_model.clone()),
        Some(state.memory_store.clone()),
        agent_spec.clone(),
    );
    let execution_input = crate::execution_facade::TauriAgentExecutionInput {
        mode: crate::execution_facade::TauriAgentExecutionMode::StreamChat,
        task,
        life_model: life_model.clone(),
        tools_prompt: tools_prompt.clone(),
        privacy_engine: privacy_engine.clone(),
        agent_spec: Some(agent_spec.clone()),
        prompt_registry: Some(prompt_registry),
        streaming_callback: Some(callback),
    };

    let execution_outcome =
        crate::execution_facade::run_tauri_agent_task(&agent_loop, &action_ctx, execution_input)
            .await;

    let (mut reply, mut agent_run) = match execution_outcome {
        Ok(outcome) => (outcome.reply, outcome.run),
        Err(e) => {
            let error_decision =
                handle_execution_facade_stream_error(&e, &session_id, &placeholder_run_id);
            for (event, payload) in &error_decision.emitted_events {
                let _ = app_handle.emit(event, payload.clone());
            }
            if !error_decision.should_fallback {
                let error_msg = error_decision
                    .error_message
                    .unwrap_or_else(|| e.to_string());
                return Err(error_msg);
            }
            eprintln!(
                "[warn] ExecutionFacade streaming failed, attempting governed compatibility fallback: {}",
                e
            );
            let user_input_txt = user_msg
                .as_ref()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            let (fallback_reply, agent_run) = match crate::handle_agent_loop_fallback(
                &scheduler,
                desensitized_messages.clone(),
                &life_model,
                &tools_prompt,
                &session_id,
                &user_input_txt,
                state.agent_run_store.as_ref(),
                state.agent_run_event_store.as_ref(),
                &e.to_string(),
                agent_spec.privacy_policy,
                &agent_spec,
                &fallback_prompt_registry,
            )
            .await
            {
                Ok(result) => result,
                Err(error_msg) => {
                    let _ = app_handle.emit(
                        "stream-message-error",
                        json!({
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
                json!({
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
                json!({
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
                json!({
                    "session_id": &session_id,
                    "run_id": agent_run.id,
                    "error": e,
                }),
            );
            Ok(())
        }
    }
}

fn should_fallback_from_execution_facade_error(
    error: &crate::execution_facade::TauriExecutionFacadeError,
) -> bool {
    error.is_runtime()
}

#[derive(Debug, Clone)]
struct ExecutionFacadeStreamErrorDecision {
    should_fallback: bool,
    emitted_events: Vec<(&'static str, serde_json::Value)>,
    error_message: Option<String>,
}

fn handle_execution_facade_stream_error(
    error: &crate::execution_facade::TauriExecutionFacadeError,
    session_id: &str,
    run_id: &str,
) -> ExecutionFacadeStreamErrorDecision {
    if should_fallback_from_execution_facade_error(error) {
        return ExecutionFacadeStreamErrorDecision {
            should_fallback: true,
            emitted_events: Vec::new(),
            error_message: None,
        };
    }

    let error_msg = error.to_string();
    ExecutionFacadeStreamErrorDecision {
        should_fallback: false,
        emitted_events: vec![(
            "stream-message-error",
            json!({
                "session_id": session_id,
                "run_id": run_id,
                "error": error_msg.clone(),
            }),
        )],
        error_message: Some(error_msg),
    }
}

// ── Main streaming command ─────────────────────────────────────────────────

#[tauri::command]
pub(crate) async fn start_stream_message(
    args: Option<StartStreamMessageArgs>,
    session_id: Option<String>,
    messages: Option<Vec<ChatMessage>>,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let (session_id, messages) = if let Some(args) = args {
        (args.session_id, args.messages)
    } else {
        (
            session_id.ok_or_else(|| "start_stream_message 缺少 session_id".to_string())?,
            messages.ok_or_else(|| "start_stream_message 缺少 messages".to_string())?,
        )
    };

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

    // Route L2/L3 to AgentLoop streaming path; L1 uses direct reflex below
    if layer != Layer::L1 {
        return start_stream_message_with_agent_loop(
            session_id, messages, user_msg, layer, app_handle, state,
        )
        .await;
    }

    // L1 direct reflex — no AgentLoop needed
    let user_input_text = messages
        .last()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let mut agent_run = AgentRun::new_chat_run(&session_id, &user_input_text);
    let _agent_run_id = agent_run.id.clone();
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(&agent_run) {
            log::warn!("[AgentRun] 保存运行记录失败: {}", e);
        }
    }

    // Layer 1: direct reflex response (non-streaming, emit as single chunk)
    if layer == Layer::L1 {
        if let Some(ref i) = intent {
            if let Some(reply) = i.direct_response() {
                return handle_l1_direct_stream_response(
                    &app_handle,
                    &session_id,
                    reply,
                    &user_msg,
                    &state,
                    &mut agent_run,
                )
                .await;
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
        _embed_err,
        context_summary,
    ) = match if use_v2 {
        crate::chat_preprocess::preprocess_chat_input_v2(&session_id, &messages, &state).await
    } else {
        crate::chat_preprocess::preprocess_chat_input(&session_id, &messages, &state).await
    } {
        Ok(result) => result,
        Err(message) => {
            let error = AgentRunError {
                message: message.clone(),
                phase: "preprocess".to_string(),
                recoverable: true,
            };
            agent_run.fail(error);
            if let Some(ref store_arc) = state.agent_run_store {
                let store = store_arc.lock().await;
                if let Err(e) = store.update_run(&agent_run) {
                    log::warn!("[AgentRun] 更新运行记录失败: {}", e);
                }
            }
            return Err(message);
        }
    };

    let auto_checkin_msg_stream = run_auto_checkin_and_stream_signals(
        &user_msg,
        &mut life_model,
        &session_id,
        &state,
        Some(&mut agent_run),
    )
    .await?;

    // ── Resolve AgentSpec — fail closed, no fallback ──────────────────
    let agent_spec = match resolve_required_agent_spec(&state.agent_spec_store, None).await {
        Ok(spec) => spec,
        Err(e) => {
            log::error!("[AgentSpec] Chat resolution failed: {}", e);
            emit_stream_error(
                &app_handle,
                &session_id,
                &agent_run.id,
                format!("AgentSpec 解析失败：{}", e),
            );
            agent_run.fail(AgentRunError {
                message: format!("AgentSpec resolution failed: {}", e),
                phase: "preprocess".to_string(),
                recoverable: false,
            });
            if let Some(ref store_arc) = state.agent_run_store {
                let store = store_arc.lock().await;
                if let Err(err) = store.update_run(&agent_run) {
                    log::warn!("[AgentRun] update_run failed: {}", err);
                }
            }
            return Err(e.to_string());
        }
    };

    // Record AgentSpecSelected event — fail-closed governance metadata
    if let Some(ref es) = state.agent_run_event_store {
        if let Err(e) = es.append_event(&AgentRunEvent::new(
            &agent_run.id,
            AgentRunEventType::AgentSpecSelected,
            AgentEventActor::Runtime,
            format!(
                "AgentSpec {} selected for governed execution",
                agent_spec.id
            ),
            trace_payloads::build_agent_spec_selected_payload(
                &agent_spec.id,
                agent_spec.role.to_string(),
                agent_spec.privacy_policy.to_string(),
            ),
        )) {
            log::error!("[AgentRun] Failed to append AgentSpecSelected event: {}", e);
        }
    }

    let scheduler_clone = state.scheduler.lock().await.clone();
    let model_route = scheduler_clone
        .preview_chat_route(Some(&tools_prompt))
        .await;
    let cfg = state.config.lock().await;
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler_clone.clone(), &cfg);
    drop(cfg);

    let prompt_registry = openlife_core::agent::prompt_stack::PromptBlockRegistry::built_in();

    let task = AgentTask {
        kind: AgentTaskKind::Conversation,
        session_id: session_id.clone(),
        user_text: user_msg
            .as_ref()
            .map(|m| m.content.clone())
            .unwrap_or_default(),
        messages: desensitized_messages.clone(),
        layer,
        agent_spec_id: Some(agent_spec.id.clone()),
        ..Default::default()
    };

    let mut reasoning_trace = ReasoningTrace::default();
    let mut messages_with_reasoning = desensitized_messages.clone();

    let runtime_output = agent_runtime
        .execute_task_with_spec(
            &task,
            &life_model,
            &tools_prompt,
            None,
            vec![],
            privacy_engine.clone(),
            &agent_spec,
            &prompt_registry,
        )
        .await;

    let _actual_layer = match runtime_output {
        Ok(output) => {
            messages_with_reasoning = output.final_messages;
            reasoning_trace = output.reasoning_trace;
            if let Some(ref es) = state.agent_run_event_store {
                if let Err(e) = es.append_event(&AgentRunEvent::new(
                    &agent_run.id,
                    AgentRunEventType::PromptStackAssembled,
                    AgentEventActor::Runtime,
                    format!(
                        "PromptStack assembled with {} blocks from AgentSpec {}",
                        output.prompt_block_trace.len(),
                        agent_spec.id
                    ),
                    trace_payloads::build_prompt_stack_assembled_payload(
                        &agent_spec.id,
                        &output.prompt_block_trace,
                    ),
                )) {
                    log::error!(
                        "[AgentRun] Failed to append PromptStackAssembled event: {}",
                        e
                    );
                }
                if let Err(e) = es.append_event(&AgentRunEvent::new(
                    &agent_run.id,
                    AgentRunEventType::ContextGovernanceApplied,
                    AgentEventActor::Runtime,
                    format!("Context governance applied by AgentSpec {}", agent_spec.id),
                    trace_payloads::build_context_governance_applied_payload(
                        &agent_spec.id,
                        output
                            .governed_context_summary
                            .as_ref()
                            .map(|g| g.included.clone())
                            .unwrap_or_default(),
                        output
                            .governed_context_summary
                            .as_ref()
                            .map(|g| g.excluded.clone())
                            .unwrap_or_default(),
                        agent_spec.privacy_policy.to_string(),
                        trace_payloads::ContextGovernanceEmitter::StreamingExecution,
                    ),
                )) {
                    log::error!(
                        "[AgentRun] Failed to append ContextGovernanceApplied event: {}",
                        e
                    );
                }
            }
            if layer == Layer::L3 {
                agent_run.reasoning_strategy = Some("layered".to_string());
            } else {
                agent_run.reasoning_strategy = Some("direct".to_string());
            }
            agent_run.reasoning_trace = Some(reasoning_trace.clone());
            layer
        }
        Err(e) => {
            if e.is_governance_failure() {
                log::error!(
                    "[AgentRuntime] Governance failure (prompt stack / context policy): {}",
                    e
                );
                emit_stream_error(
                    &app_handle,
                    &session_id,
                    &agent_run.id,
                    format!("AgentSpec governance failure: {}", e),
                );
                if let Some(ref es) = state.agent_run_event_store {
                    if let Err(e) = es.append_event(&AgentRunEvent::new(
                        &agent_run.id,
                        AgentRunEventType::ModelFailed,
                        AgentEventActor::Runtime,
                        format!("Governance failure: {}", e),
                        json!({
                            "agent_spec_id": agent_spec.id,
                            "error": e.to_string(),
                        }),
                    )) {
                        log::error!("[AgentRun] Failed to append ModelFailed event: {}", e);
                    }
                }
                agent_run.fail(AgentRunError {
                    message: format!("Governance failure: {}", e),
                    phase: "governance".to_string(),
                    recoverable: false,
                });
                if let Some(ref store_arc) = state.agent_run_store {
                    let store = store_arc.lock().await;
                    if let Err(err) = store.update_run(&agent_run) {
                        log::warn!("[AgentRun] update_run failed: {}", err);
                    }
                }
                return Err(format!("Governance failure: {}", e));
            }
            log::warn!("[AgentRuntime] Reasoning failed: {}, falling back to L2", e);
            agent_run.reasoning_strategy = Some("direct".to_string());
            if layer == Layer::L3 {
                let lr = state.layer_router.lock().await;
                lr.fallback(Layer::L3).unwrap_or(Layer::L2)
            } else {
                layer
            }
        }
    };

    let _ = app_handle.emit(
        "stream-message-start",
        json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "agent_spec_id": agent_spec.id,
            "reasoning_trace": reasoning_trace.clone(),
            "tool_calls": Vec::<ToolCallResult>::new(),
        }),
    );

    let mut full_reply = String::new();
    if let Some(ref ex) = reasoning_trace.generation_result {
        if let Some(text) = ex.get("text").and_then(|t| t.as_str()) {
            full_reply = text.to_string();
            let _ = app_handle.emit(
                "stream-message-chunk",
                json!({
                    "session_id": &session_id,
                    "chunk": text,
                }),
            );
        }
    }

    let full_reply = if full_reply.is_empty() {
        run_governed_stream_with_fallbacks(
            &scheduler_clone,
            messages_with_reasoning,
            &life_model,
            &tools_prompt,
            agent_spec.privacy_policy,
            &app_handle,
            &session_id,
            full_reply,
            &mut reasoning_trace,
            &mut agent_run,
            state.agent_run_store.as_ref(),
        )
        .await?
    } else {
        full_reply
    };

    let mut first_reply = privacy_engine.reconstruct(&full_reply, &privacy_map);
    if let Some(msg) = auto_checkin_msg_stream {
        if !first_reply.contains(&msg) {
            first_reply = format!("{}\n\n[系统] {}", first_reply, msg);
        }
    }

    let reply = first_reply;
    let tool_calls: Vec<ToolCallResult> = vec![];

    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };

    let context_summary = ContextSummary {
        life_model_empty: life_model.is_effectively_empty(),
        included_life_model_sections: included_life_model_sections(&life_model),
        memory_hit_count: context_summary.memory_hit_count,
        memory_sources: context_summary.memory_sources,
        used_tools_prompt: !tools_prompt.is_empty(),
        redaction_applied: !privacy_map.is_empty(),
        redaction_level: if privacy_map.is_empty() {
            RedactionLevel::None
        } else {
            RedactionLevel::Light
        },
    };
    let preview = preview_text(&reply, 200);
    agent_run.complete(&preview, model_route, context_summary);
    if let Err(e) = finalize_chat_agent_run(
        &session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        &life_model,
        &state,
    )
    .await
    {
        log::warn!("[Stream] finalize_chat_agent_run failed: {}", e);
        let _ = app_handle.emit(
            "stream-message-error",
            json!({
                "session_id": &session_id,
                "run_id": agent_run.id,
                "error": format!("AgentRun 持久化失败: {}", e),
            }),
        );
        return Err(e);
    }

    let _ = app_handle.emit(
        "stream-message-done",
        json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reply": reply,
            "reasoning_trace": reasoning_trace,
            "tool_calls": tool_calls,
        }),
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn streaming_path_uses_execution_facade() {
        let source = include_str!("streaming.rs");
        let direct_streaming_call = [".run_", "streaming("].concat();

        assert!(
            !source.contains(&direct_streaming_call),
            "streaming.rs must route AgentLoop streaming through ExecutionFacade"
        );
        assert!(
            source.contains("run_tauri_agent_task"),
            "streaming.rs should call the Tauri ExecutionFacade entrypoint"
        );
    }

    #[test]
    fn stream_chat_governance_error_emits_error_without_fallback_events() {
        let governance =
            crate::execution_facade::TauriExecutionFacadeError::governance("AgentSpec mismatch");
        let runtime = crate::execution_facade::TauriExecutionFacadeError::runtime("model failed");

        assert!(!super::should_fallback_from_execution_facade_error(
            &governance
        ));
        assert!(super::should_fallback_from_execution_facade_error(&runtime));

        let decision =
            super::handle_execution_facade_stream_error(&governance, "session-1", "run-1");
        assert!(!decision.should_fallback);
        assert_eq!(decision.emitted_events.len(), 1);
        assert_eq!(decision.emitted_events[0].0, "stream-message-error");
        let payload = &decision.emitted_events[0].1;
        assert_eq!(payload["session_id"], "session-1");
        assert_eq!(payload["run_id"], "run-1");
        assert!(
            payload["error"]
                .as_str()
                .is_some_and(|error: &str| error.contains("AgentSpec mismatch")),
            "error payload should preserve Governance message: {payload:?}"
        );
        assert!(
            decision
                .emitted_events
                .iter()
                .all(|(event, _)| *event != "stream-message-chunk"
                    && *event != "stream-message-done"),
            "Governance errors must only emit stream-message-error"
        );

        let runtime_decision =
            super::handle_execution_facade_stream_error(&runtime, "session-1", "run-1");
        assert!(runtime_decision.should_fallback);
        assert!(runtime_decision.emitted_events.is_empty());
    }

    #[test]
    fn stream_chat_runtime_error_fallback_uses_chat_governed_boundary() {
        let source = include_str!("streaming.rs");
        let start = source
            .find("async fn start_stream_message_with_agent_loop")
            .expect("stream chat entrypoint should exist");
        let end = source[start..]
            .find("fn should_fallback_from_execution_facade_error")
            .map(|offset| start + offset)
            .expect("stream fallback helper should follow entrypoint");
        let stream_path = &source[start..end];

        assert!(stream_path.contains("handle_agent_loop_fallback"));
        assert!(stream_path.contains("agent_spec.privacy_policy"));
        assert!(
            !stream_path.contains(".generate("),
            "StreamChat compatibility fallback must not call legacy scheduler generation"
        );
        assert!(
            !stream_path.contains(".generate_stream("),
            "StreamChat compatibility fallback must not call legacy scheduler streaming generation"
        );
    }
}
