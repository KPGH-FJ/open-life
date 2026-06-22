use futures::StreamExt;
use openlife_core::agent::ReasoningTrace;
use openlife_core::layer_router::Layer;
use openlife_core::llm::ChatMessage;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

use crate::legacy_write_convergence::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::main_chat_conversation_updates::{
    capture_conversation_signals, try_auto_checkin_daily_goals,
};
use crate::main_chat_event_stream::materialize_optional_main_chat_agent_events;
use crate::main_chat_generation_support::{
    finalize_chat_agent_run, generate_non_stream_fallback, preview_text,
};
use crate::main_chat_hs_runtime::{build_chat_runtime_hs_packet, included_life_model_sections};
use crate::main_chat_kernel::{
    run_main_chat_kernel_direct_answer_with_state, StreamingMainChatEventSink,
};
use crate::main_chat_legacy_fallback::ordinary_stream_chat_execution_plan;
use crate::main_chat_preprocess::{preprocess_chat_input, preprocess_chat_input_v2};
use crate::main_chat_runtime_support::{
    append_main_chat_agent_transcript, start_main_chat_agent_turn,
};
use crate::main_chat_strategy::try_run_main_chat_agent_strategy;
use crate::{persist_life_model, AppState, ToolCallResult};

const STREAM_INIT_TIMEOUT_SECS: u64 = 45;
const STREAM_CHUNK_TIMEOUT_SECS: u64 = 90;

pub(crate) async fn start_stream_message_with_state(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
    mut emit_stream_event: impl FnMut(&str, serde_json::Value) + Send,
) -> Result<(), String> {
    let user_msg = messages.last().cloned();
    let main_chat_agent_turn = start_main_chat_agent_turn(
        &session_id,
        user_msg.as_ref(),
        openlife_core::agent::AgentTaskKind::Conversation,
        state,
    )
    .await?;

    if main_chat_agent_turn.decision.selected_strategy
        == openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer
    {
        let result = {
            let mut event_sink = StreamingMainChatEventSink::new(&mut emit_stream_event);
            run_main_chat_kernel_direct_answer_with_state(
                &session_id,
                messages,
                selected_skill_id,
                state,
                &main_chat_agent_turn,
                &mut event_sink,
                "streaming",
            )
            .await?
        };
        let run_id = result.run_id.clone().unwrap_or_default();
        let agent_state = result.agent_state.clone();
        let kernel_event_count = result.kernel_events.len();
        emit_stream_event(
            "stream-message-start",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": run_id,
                "reasoning_trace": result.reasoning_trace.clone(),
                "tool_calls": result.tool_calls.clone(),
                "agent_ingress": result.agent_ingress.clone(),
                "agent_state": agent_state.clone(),
                "execution_transcript": result.execution_transcript.clone(),
                "legacy_fallback_used": result.legacy_fallback_used,
                "kernel_event_count": kernel_event_count,
            }),
        );
        if !result.reply.is_empty() {
            emit_stream_event(
                "stream-message-chunk",
                serde_json::json!({
                    "session_id": &session_id,
                    "run_id": run_id,
                    "chunk": result.reply.clone(),
                }),
            );
        }
        emit_stream_event(
            "stream-message-done",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": run_id,
                "reply": result.reply,
                "reasoning_trace": result.reasoning_trace,
                "tool_calls": result.tool_calls,
                "agent_ingress": result.agent_ingress,
                "agent_state": agent_state,
                "execution_transcript": result.execution_transcript,
                "legacy_fallback_used": result.legacy_fallback_used,
                "kernel_event_count": kernel_event_count,
            }),
        );
        for event in result.durable_events {
            emit_stream_event(
                "main-chat-agent-event",
                serde_json::to_value(event).map_err(|err| err.to_string())?,
            );
        }
        return Ok(());
    }

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
    let user_input_text = messages
        .last()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text);
    let _agent_run_id = agent_run.id.clone();
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(&agent_run) {
            log::warn!("[AgentRun] 保存运行记录失败: {}", e);
        }
    }

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
        context_summary,
    ) = match if use_v2 {
        preprocess_chat_input_v2(&session_id, &messages, state).await
    } else {
        preprocess_chat_input(&session_id, &messages, state).await
    } {
        Ok(result) => result,
        Err(message) => {
            let error = openlife_core::agent::AgentRunError {
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

    let auto_checkin_msg_stream = if let Some(ref m) = user_msg {
        let msg = try_auto_checkin_daily_goals(&m.content, &mut life_model);
        capture_conversation_signals(&session_id, &m.content, &life_model, state).await;
        if msg.is_some() {
            if let Err(message) = persist_life_model(
                &state.clone(),
                life_model.clone(),
                false,
                LifeModelMaterializerCallerContext::new(
                    "ordinary_stream_legacy_auto_checkin_source_data",
                    LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
                    LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
                ),
            )
            .await
            {
                let error = openlife_core::agent::AgentRunError {
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
        }
        msg
    } else {
        None
    };

    if let Some(result) = try_run_main_chat_agent_strategy(
        &session_id,
        user_msg.as_ref(),
        &desensitized_messages,
        &life_model,
        context_summary.clone(),
        embed_err.clone(),
        auto_checkin_msg_stream.clone(),
        &main_chat_agent_turn,
        state,
        &privacy_engine,
        &privacy_map,
        Some(agent_run.clone()),
        selected_skill_id.as_deref(),
    )
    .await?
    {
        let run_id = result.run_id.clone().unwrap_or_default();
        let agent_state = result.agent_state.clone();
        let durable_events =
            materialize_optional_main_chat_agent_events(state, agent_state.as_ref()).await?;
        emit_stream_event(
            "stream-message-start",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": run_id,
                "reasoning_trace": result.reasoning_trace.clone(),
                "tool_calls": result.tool_calls.clone(),
                "agent_ingress": result.agent_ingress.clone(),
                "agent_state": agent_state.clone(),
                "execution_transcript": result.execution_transcript.clone(),
                "legacy_fallback_used": result.legacy_fallback_used,
            }),
        );
        emit_stream_event(
            "stream-message-chunk",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": run_id,
                "chunk": result.reply.clone(),
            }),
        );
        emit_stream_event(
            "stream-message-done",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": run_id,
                "reply": result.reply,
                "reasoning_trace": result.reasoning_trace,
                "tool_calls": result.tool_calls,
                "agent_ingress": result.agent_ingress,
                "agent_state": agent_state,
                "execution_transcript": result.execution_transcript,
                "legacy_fallback_used": result.legacy_fallback_used,
            }),
        );
        for event in durable_events {
            emit_stream_event(
                "main-chat-agent-event",
                serde_json::to_value(event).map_err(|err| err.to_string())?,
            );
        }
        return Ok(());
    }

    let ordinary_plan = ordinary_stream_chat_execution_plan(layer);
    debug_assert!(!ordinary_plan.constructs_agent_loop);
    debug_assert!(!ordinary_plan.constructs_action_executor);
    debug_assert!(!ordinary_plan.tool_execution_allowed);

    let scheduler_clone = state.scheduler.lock().await.clone();
    let model_route = scheduler_clone
        .preview_chat_route(Some(&tools_prompt))
        .await;
    let cfg = state.config.lock().await;
    let agent_runtime =
        openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler_clone.clone(), &cfg);
    drop(cfg);

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
        build_chat_runtime_hs_packet(state, &task, &life_model, &tools_prompt, None).await?;

    let mut reasoning_trace = ReasoningTrace::default();
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }
    let mut messages_with_reasoning = desensitized_messages.clone();

    let _actual_layer = if layer == Layer::L3 {
        let runtime_output = agent_runtime
            .execute_task(
                &task,
                &life_model,
                &tools_prompt,
                None,
                vec![],
                privacy_engine.clone(),
            )
            .await;

        match runtime_output {
            Ok(output) => {
                messages_with_reasoning = output.final_messages;
                reasoning_trace = output.reasoning_trace;
                agent_run.reasoning_strategy = Some("layered".to_string());
                agent_run.reasoning_trace = Some(reasoning_trace.clone());
                Layer::L3
            }
            Err(e) => {
                log::warn!("[AgentRuntime] Reasoning failed: {}, falling back to L2", e);
                agent_run.reasoning_strategy = Some("direct".to_string());
                let lr = state.layer_router.lock().await;
                lr.fallback(Layer::L3).unwrap_or(Layer::L2)
            }
        }
    } else {
        agent_run.reasoning_strategy = Some("direct".to_string());
        layer
    };
    let legacy_fallback_used = main_chat_agent_turn.decision.selected_strategy
        != openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer;
    agent_run.reasoning_strategy = Some(if legacy_fallback_used {
        format!(
            "main_chat_agent_v1_{}_legacy_stream_fallback",
            main_chat_agent_turn.decision.selected_strategy.as_str()
        )
    } else {
        "main_chat_agent_v1_direct_answer_stream".to_string()
    });

    emit_stream_event(
        "stream-message-start",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reasoning_trace": reasoning_trace.clone(),
            "tool_calls": Vec::<ToolCallResult>::new(),
            "agent_ingress": main_chat_agent_turn.decision.clone(),
            "execution_transcript": main_chat_agent_turn.transcript_entries.clone(),
            "legacy_fallback_used": legacy_fallback_used,
        }),
    );

    let mut full_reply = String::new();
    if let Some(ref ex) = reasoning_trace.generation_result {
        if let Some(text) = ex.get("text").and_then(|t| t.as_str()) {
            full_reply = text.to_string();
            emit_stream_event(
                "stream-message-chunk",
                serde_json::json!({
                    "session_id": &session_id,
                    "chunk": text,
                }),
            );
        }
    }

    if full_reply.is_empty() {
        let stream_init = if let Some(ref packet) = hs_packet {
            timeout(
                Duration::from_secs(STREAM_INIT_TIMEOUT_SECS),
                scheduler_clone.generate_stream_with_hs_packet(
                    messages_with_reasoning.clone(),
                    &life_model,
                    Some(&tools_prompt),
                    packet,
                ),
            )
            .await
        } else {
            timeout(
                Duration::from_secs(STREAM_INIT_TIMEOUT_SECS),
                scheduler_clone.generate_stream(
                    messages_with_reasoning.clone(),
                    &life_model,
                    Some(&tools_prompt),
                ),
            )
            .await
        };

        match stream_init
            .map_err(|_| format!("流式响应初始化超时（{} 秒）", STREAM_INIT_TIMEOUT_SECS))
            .and_then(|result| result.map_err(|e| e.to_string()))
        {
            Ok(mut stream) => loop {
                let next_chunk = match timeout(
                    Duration::from_secs(STREAM_CHUNK_TIMEOUT_SECS),
                    stream.next(),
                )
                .await
                {
                    Ok(next) => next,
                    Err(_) => {
                        let stream_error =
                            format!("超过 {} 秒没有收到模型输出", STREAM_CHUNK_TIMEOUT_SECS);
                        match generate_non_stream_fallback(
                            &scheduler_clone,
                            messages_with_reasoning.clone(),
                            &life_model,
                            &tools_prompt,
                            hs_packet.clone(),
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
                                emit_stream_event(
                                    "stream-message-chunk",
                                    serde_json::json!({
                                        "session_id": &session_id,
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
                                emit_stream_event(
                                    "stream-message-error",
                                    serde_json::json!({
                                        "session_id": &session_id,
                                        "run_id": agent_run.id,
                                        "error": message.clone(),
                                    }),
                                );
                                let error = openlife_core::agent::AgentRunError {
                                    message: message.clone(),
                                    phase: "stream".to_string(),
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
                            emit_stream_event(
                                "stream-message-chunk",
                                serde_json::json!({
                                    "session_id": &session_id,
                                    "chunk": chunk,
                                }),
                            );
                        }
                    }
                    Err(e) => {
                        let stream_error = e.to_string();
                        match generate_non_stream_fallback(
                            &scheduler_clone,
                            messages_with_reasoning.clone(),
                            &life_model,
                            &tools_prompt,
                            hs_packet.clone(),
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
                                emit_stream_event(
                                    "stream-message-chunk",
                                    serde_json::json!({
                                        "session_id": &session_id,
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
                                emit_stream_event(
                                    "stream-message-error",
                                    serde_json::json!({
                                        "session_id": &session_id,
                                        "run_id": agent_run.id,
                                        "error": message.clone(),
                                    }),
                                );
                                let error = openlife_core::agent::AgentRunError {
                                    message: message.clone(),
                                    phase: "stream".to_string(),
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
                        }
                    }
                }
            },
            Err(stream_error) => {
                let stream_error = stream_error.to_string();
                match generate_non_stream_fallback(
                    &scheduler_clone,
                    messages_with_reasoning.clone(),
                    &life_model,
                    &tools_prompt,
                    hs_packet.clone(),
                )
                .await
                {
                    Ok(reply) => {
                        reasoning_trace.errors.push(format!(
                            "流式响应初始化失败，已降级为非流式响应：{}",
                            stream_error
                        ));
                        full_reply = reply.clone();
                        emit_stream_event(
                            "stream-message-chunk",
                            serde_json::json!({
                                "session_id": &session_id,
                                "chunk": reply,
                            }),
                        );
                    }
                    Err(fallback_error) => {
                        let message = format!(
                            "流式响应初始化失败，非流式重试也失败：{}；重试错误：{}",
                            stream_error, fallback_error
                        );
                        emit_stream_event(
                            "stream-message-error",
                            serde_json::json!({
                                "session_id": &session_id,
                                "run_id": agent_run.id,
                                "error": message.clone(),
                            }),
                        );
                        let error = openlife_core::agent::AgentRunError {
                            message: message.clone(),
                            phase: "stream".to_string(),
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
                }
            }
        }
        if full_reply.trim().is_empty() {
            let stream_error = "流式响应已结束，但没有收到可显示内容".to_string();
            match generate_non_stream_fallback(
                &scheduler_clone,
                messages_with_reasoning.clone(),
                &life_model,
                &tools_prompt,
                hs_packet.clone(),
            )
            .await
            {
                Ok(reply) => {
                    reasoning_trace.errors.push(format!(
                        "流式响应为空，已降级为非流式响应：{}",
                        stream_error
                    ));
                    full_reply = reply.clone();
                    emit_stream_event(
                        "stream-message-chunk",
                        serde_json::json!({
                            "session_id": &session_id,
                            "chunk": reply,
                        }),
                    );
                }
                Err(fallback_error) => {
                    let message = format!(
                        "流式响应为空，非流式重试也失败：{}；重试错误：{}",
                        stream_error, fallback_error
                    );
                    emit_stream_event(
                        "stream-message-error",
                        serde_json::json!({
                            "session_id": &session_id,
                            "run_id": agent_run.id,
                            "error": message.clone(),
                        }),
                    );
                    let error = openlife_core::agent::AgentRunError {
                        message: message.clone(),
                        phase: "stream".to_string(),
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
            }
        }
    }

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

    let context_summary = openlife_core::agent::ContextSummary {
        life_model_empty: life_model.is_effectively_empty(),
        included_life_model_sections: included_life_model_sections(&life_model),
        memory_hit_count: context_summary.memory_hit_count,
        memory_sources: context_summary.memory_sources,
        used_tools_prompt: !tools_prompt.is_empty(),
        redaction_applied: !privacy_map.is_empty(),
        redaction_level: if privacy_map.is_empty() {
            openlife_core::agent::types::RedactionLevel::None
        } else {
            openlife_core::agent::types::RedactionLevel::Light
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
        state,
    )
    .await
    {
        log::warn!("[Stream] finalize_chat_agent_run failed: {}", e);
        emit_stream_event(
            "stream-message-error",
            serde_json::json!({
                "session_id": &session_id,
                "run_id": agent_run.id,
                "error": format!("AgentRun 持久化失败: {}", e),
            }),
        );
        return Err(e);
    }
    let mut execution_transcript = main_chat_agent_turn.transcript_entries.clone();
    if legacy_fallback_used {
        execution_transcript.extend(
            append_main_chat_agent_transcript(
                state,
                main_chat_agent_turn
                    .decision
                    .agent_task_session_id
                    .as_deref(),
                openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Fallback,
                "Legacy streaming fallback was used for this Main Chat turn.",
                serde_json::json!({
                    "runId": agent_run.id,
                    "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                    "fallbackReason": "strategy_stream_executor_not_yet_available_for_this_path",
                    "fallbackVisible": true,
                }),
            )
            .await,
        );
    }
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            main_chat_agent_turn
                .decision
                .agent_task_session_id
                .as_deref(),
            openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::FinalResult,
            "Assistant response was delivered.",
            serde_json::json!({
                "runId": agent_run.id,
                "legacyFallbackUsed": legacy_fallback_used,
            }),
        )
        .await,
    );

    let agent_state =
        crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn(
            state,
            main_chat_agent_turn
                .decision
                .agent_task_session_id
                .as_deref(),
            Some(&agent_run.id),
        )
        .await;
    let durable_events =
        materialize_optional_main_chat_agent_events(state, agent_state.as_ref()).await?;

    emit_stream_event(
        "stream-message-done",
        serde_json::json!({
            "session_id": &session_id,
            "run_id": agent_run.id,
            "reply": reply,
            "reasoning_trace": reasoning_trace,
            "tool_calls": tool_calls,
            "agent_ingress": main_chat_agent_turn.decision.clone(),
            "agent_state": agent_state,
            "execution_transcript": execution_transcript,
            "legacy_fallback_used": legacy_fallback_used,
        }),
    );
    for event in durable_events {
        emit_stream_event(
            "main-chat-agent-event",
            serde_json::to_value(event).map_err(|err| err.to_string())?,
        );
    }

    Ok(())
}
