use std::sync::Arc;

use futures::StreamExt;
use openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy;
use openlife_core::agent::ReasoningTrace;
use openlife_core::layer_router::Layer;
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};
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
    main_chat_kernel_support_disposition,
    main_chat_live_provider_eval_requires_provider_backed_react,
    main_chat_react_turn_requires_governed_agent_loop_candidate_selection,
    run_main_chat_kernel_direct_answer_with_state, BufferedMainChatEventSink,
    MainChatKernelSupportDisposition, StreamingMainChatEventSink,
};
use crate::main_chat_legacy_fallback::{
    ordinary_send_chat_execution_plan, ordinary_stream_chat_execution_plan,
    send_message_with_legacy_generation,
};
use crate::main_chat_preprocess::{
    preprocess_chat_input_v2_with_options, preprocess_chat_input_with_options,
    MainChatPreprocessOptions,
};
use crate::main_chat_route_preview::{
    attach_route_preview_trace, preview_main_chat_turn_route, MainChatRoutePreviewTrace,
};
use crate::main_chat_runtime_support::{
    append_main_chat_agent_transcript, start_main_chat_agent_turn,
};
use crate::main_chat_strategy::try_run_main_chat_agent_strategy;
use crate::main_chat_streaming::{STREAM_CHUNK_TIMEOUT_SECS, STREAM_INIT_TIMEOUT_SECS};
use crate::main_chat_tool_loop::{
    run_main_chat_tool_loop_adapter, MainChatToolLoopInput, MainChatToolLoopOutcome,
};
use crate::{persist_life_model, AppState, SendMessageResult, ToolCallResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MainChatExecutionPath {
    KernelDirect,
    KernelReadTool,
    KernelWriteOutcome,
    ToolLoop,
    PlanExecute,
    GovernedBlocker,
    LegacyCompatFallback,
}

impl MainChatExecutionPath {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::KernelDirect => "KernelDirect",
            Self::KernelReadTool => "KernelReadTool",
            Self::KernelWriteOutcome => "KernelWriteOutcome",
            Self::ToolLoop => "ToolLoop",
            Self::PlanExecute => "PlanExecute",
            Self::GovernedBlocker => "GovernedBlocker",
            Self::LegacyCompatFallback => "LegacyCompatFallback",
        }
    }

    pub(crate) fn is_kernel_dispatch(self) -> bool {
        matches!(
            self,
            Self::KernelDirect
                | Self::KernelReadTool
                | Self::KernelWriteOutcome
                | Self::PlanExecute
                | Self::GovernedBlocker
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatTurnRouteDecision {
    pub(crate) path: MainChatExecutionPath,
    pub(crate) strategy_label: String,
    pub(crate) reason_code: String,
    pub(crate) kernel_supported: bool,
    pub(crate) kernel_support_disposition: String,
    pub(crate) fallback_allowed: bool,
    pub(crate) requires_provider: bool,
    pub(crate) requires_tool_loop: bool,
    pub(crate) live_provider_backed_react_required: bool,
    pub(crate) governed_agent_loop_candidate_selection_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MainChatTurnStreamMode {
    Buffered,
    Streaming,
}

impl MainChatTurnStreamMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Buffered => "buffered",
            Self::Streaming => "streaming",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatTurnPipelineInput {
    pub(crate) session_id: String,
    pub(crate) messages: Vec<ChatMessage>,
    #[serde(default)]
    pub(crate) selected_skill_id: Option<String>,
    pub(crate) stream_mode: MainChatTurnStreamMode,
}

pub(crate) struct MainChatTurnPipelineOutput {
    pub(crate) route_decision: MainChatTurnRouteDecision,
    pub(crate) delivery: MainChatTurnDelivery,
}

pub(crate) enum MainChatTurnDelivery {
    Buffered {
        result: Box<SendMessageResult>,
    },
    Streamed {
        run_id: Option<String>,
        legacy_fallback_used: bool,
        kernel_event_count: Option<usize>,
        durable_event_count: usize,
        done_payload: serde_json::Value,
    },
}

impl MainChatTurnRouteDecision {
    pub(crate) fn execution_path_label(&self) -> &'static str {
        self.path.as_str()
    }

    pub(crate) fn legacy_compat_fallback(&self) -> Self {
        self.legacy_compat_fallback_with_reason("legacy_compat_after_strategy_no_result")
    }

    pub(crate) fn legacy_compat_fallback_with_reason(
        &self,
        reason_code: impl Into<String>,
    ) -> Self {
        Self {
            path: MainChatExecutionPath::LegacyCompatFallback,
            strategy_label: self.strategy_label.clone(),
            reason_code: reason_code.into(),
            kernel_supported: self.kernel_supported,
            kernel_support_disposition: self.kernel_support_disposition.clone(),
            fallback_allowed: true,
            requires_provider: false,
            requires_tool_loop: false,
            live_provider_backed_react_required: self.live_provider_backed_react_required,
            governed_agent_loop_candidate_selection_required: self
                .governed_agent_loop_candidate_selection_required,
        }
    }
}

pub(crate) async fn decide_main_chat_turn_route(
    selected_strategy: &MainChatAgentStrategy,
    messages: &[ChatMessage],
    state: &Arc<AppState>,
) -> MainChatTurnRouteDecision {
    let kernel_support_disposition =
        main_chat_kernel_support_disposition(selected_strategy, messages);
    let live_provider_backed_react_required =
        main_chat_live_provider_eval_requires_provider_backed_react(selected_strategy, state).await;
    let governed_agent_loop_candidate_selection_required =
        main_chat_react_turn_requires_governed_agent_loop_candidate_selection(
            selected_strategy,
            messages,
            state,
        )
        .await;

    decide_main_chat_turn_route_from_disposition(
        *selected_strategy,
        kernel_support_disposition,
        live_provider_backed_react_required,
        governed_agent_loop_candidate_selection_required,
    )
}

pub(crate) fn decide_main_chat_turn_route_from_disposition(
    selected_strategy: MainChatAgentStrategy,
    kernel_support_disposition: MainChatKernelSupportDisposition,
    live_provider_backed_react_required: bool,
    governed_agent_loop_candidate_selection_required: bool,
) -> MainChatTurnRouteDecision {
    let kernel_supported = matches!(
        kernel_support_disposition,
        MainChatKernelSupportDisposition::KernelSupported
            | MainChatKernelSupportDisposition::GovernedBlocker
    );
    let requires_tool_loop =
        live_provider_backed_react_required || governed_agent_loop_candidate_selection_required;

    let (path, reason_code, fallback_allowed, requires_provider) = if requires_tool_loop {
        (
            MainChatExecutionPath::ToolLoop,
            if live_provider_backed_react_required {
                "provider_backed_react_required"
            } else {
                "governed_agent_loop_candidate_selection_required"
            },
            true,
            live_provider_backed_react_required,
        )
    } else if !kernel_supported {
        (
            MainChatExecutionPath::LegacyCompatFallback,
            "kernel_support_unavailable",
            true,
            false,
        )
    } else {
        match kernel_support_disposition {
            MainChatKernelSupportDisposition::GovernedBlocker => (
                MainChatExecutionPath::GovernedBlocker,
                "kernel_governed_blocker",
                false,
                false,
            ),
            MainChatKernelSupportDisposition::KernelSupported => match selected_strategy {
                MainChatAgentStrategy::DirectAnswer => (
                    MainChatExecutionPath::KernelDirect,
                    "kernel_supported_direct_answer",
                    false,
                    false,
                ),
                MainChatAgentStrategy::ReActToolExecution => (
                    MainChatExecutionPath::KernelReadTool,
                    "kernel_supported_read_tool",
                    false,
                    false,
                ),
                MainChatAgentStrategy::PlanExecute => (
                    MainChatExecutionPath::PlanExecute,
                    "kernel_supported_plan_execute",
                    false,
                    false,
                ),
                MainChatAgentStrategy::MemoryProposal
                | MainChatAgentStrategy::LifeModelProposal
                | MainChatAgentStrategy::BlockedConfirmation => (
                    MainChatExecutionPath::KernelWriteOutcome,
                    "kernel_supported_write_outcome",
                    false,
                    false,
                ),
                MainChatAgentStrategy::ReviewMaturation => (
                    MainChatExecutionPath::GovernedBlocker,
                    "kernel_governed_blocker",
                    false,
                    false,
                ),
            },
        }
    };

    MainChatTurnRouteDecision {
        path,
        strategy_label: selected_strategy.as_str().into(),
        reason_code: reason_code.into(),
        kernel_supported,
        kernel_support_disposition: kernel_support_disposition.as_str().into(),
        fallback_allowed,
        requires_provider,
        requires_tool_loop,
        live_provider_backed_react_required,
        governed_agent_loop_candidate_selection_required,
    }
}

pub(crate) async fn run_main_chat_turn_pipeline_buffered(
    input: MainChatTurnPipelineInput,
    state: &Arc<AppState>,
) -> Result<MainChatTurnPipelineOutput, String> {
    debug_assert_eq!(input.stream_mode, MainChatTurnStreamMode::Buffered);
    let MainChatTurnPipelineInput {
        session_id,
        messages,
        selected_skill_id,
        stream_mode: _,
    } = input;
    let user_msg = messages.last().cloned();
    let main_chat_agent_turn = start_main_chat_agent_turn(
        &session_id,
        user_msg.as_ref(),
        openlife_core::agent::AgentTaskKind::Conversation,
        state,
    )
    .await?;

    let route_decision = decide_main_chat_turn_route(
        &main_chat_agent_turn.decision.selected_strategy,
        &messages,
        state,
    )
    .await;
    let route_preview_trace = preview_main_chat_turn_route(
        state,
        &messages,
        &main_chat_agent_turn.decision,
        &route_decision,
        MainChatTurnStreamMode::Buffered,
    )
    .await;
    if route_decision.path.is_kernel_dispatch() {
        let mut event_sink = BufferedMainChatEventSink::default();
        let result = run_main_chat_kernel_direct_answer_with_state(
            &session_id,
            messages,
            selected_skill_id,
            state,
            &main_chat_agent_turn,
            &mut event_sink,
            MainChatTurnStreamMode::Buffered.as_str(),
        )
        .await?;
        let mut result = result.into_send_message_result();
        attach_route_preview_trace(&mut result.reasoning_trace, &route_preview_trace);
        return Ok(MainChatTurnPipelineOutput {
            route_decision,
            delivery: MainChatTurnDelivery::Buffered {
                result: Box::new(result),
            },
        });
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

    let (use_v2, preprocess_options) = {
        let cfg = state.config.lock().await;
        (
            cfg.experimental_context_assembler,
            MainChatPreprocessOptions::from_runtime_mode(&cfg.runtime_mode),
        )
    };

    let (
        mut life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        context_summary,
    ) = if use_v2 {
        preprocess_chat_input_v2_with_options(&session_id, &messages, state, preprocess_options)
            .await?
    } else {
        preprocess_chat_input_with_options(&session_id, &messages, state, preprocess_options)
            .await?
    };

    let auto_checkin_msg = if let Some(ref m) = user_msg {
        let msg = try_auto_checkin_daily_goals(&m.content, &mut life_model);
        capture_conversation_signals(&session_id, &m.content, &life_model, state).await;
        if msg.is_some() {
            let _ = persist_life_model(
                state,
                life_model.clone(),
                false,
                LifeModelMaterializerCallerContext::new(
                    "ordinary_chat_auto_checkin_source_data",
                    LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
                    LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
                ),
            )
            .await?;
        }
        msg
    } else {
        None
    };

    if route_decision.path == MainChatExecutionPath::ToolLoop {
        let outcome = run_main_chat_tool_loop_adapter(
            MainChatToolLoopInput {
                session_id: &session_id,
                user_msg: user_msg.as_ref(),
                desensitized_messages: &desensitized_messages,
                life_model: &life_model,
                context_summary: context_summary.clone(),
                embed_err: embed_err.clone(),
                auto_checkin_msg: auto_checkin_msg.clone(),
                main_chat_agent_turn: &main_chat_agent_turn,
                privacy_engine: &privacy_engine,
                privacy_map: &privacy_map,
                existing_agent_run: None,
                selected_skill_id: selected_skill_id.as_deref(),
            },
            state,
        )
        .await?;
        match outcome {
            MainChatToolLoopOutcome::AgentLoopSuccess(mut result)
            | MainChatToolLoopOutcome::GovernedBlocker(mut result)
            | MainChatToolLoopOutcome::ToolPermissionProposal(mut result)
            | MainChatToolLoopOutcome::SingleStepFallback(mut result) => {
                attach_route_preview_trace(&mut result.reasoning_trace, &route_preview_trace);
                materialize_optional_main_chat_agent_events(state, result.agent_state.as_ref())
                    .await?;
                return Ok(MainChatTurnPipelineOutput {
                    route_decision,
                    delivery: MainChatTurnDelivery::Buffered {
                        result: Box::new(result),
                    },
                });
            }
            MainChatToolLoopOutcome::ExplicitFallbackAvailable { reason_code }
            | MainChatToolLoopOutcome::NoResult { reason_code } => {
                if !route_decision.fallback_allowed {
                    return Err(format!(
                        "ToolLoop adapter returned {reason_code}, but legacy fallback is not allowed"
                    ));
                }
                let legacy_route_decision =
                    route_decision.legacy_compat_fallback_with_reason(reason_code);
                let ordinary_plan = ordinary_send_chat_execution_plan(layer);
                let result = send_message_with_legacy_generation(
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
                    context_summary,
                    ordinary_plan,
                    main_chat_agent_turn,
                    legacy_route_decision.clone(),
                    state,
                )
                .await?;
                let mut result = result;
                attach_route_preview_trace(&mut result.reasoning_trace, &route_preview_trace);
                materialize_optional_main_chat_agent_events(state, result.agent_state.as_ref())
                    .await?;
                return Ok(MainChatTurnPipelineOutput {
                    route_decision: legacy_route_decision,
                    delivery: MainChatTurnDelivery::Buffered {
                        result: Box::new(result),
                    },
                });
            }
        }
    }

    if let Some(mut result) = try_run_main_chat_agent_strategy(
        &session_id,
        user_msg.as_ref(),
        &desensitized_messages,
        &life_model,
        context_summary.clone(),
        embed_err.clone(),
        auto_checkin_msg.clone(),
        &main_chat_agent_turn,
        state,
        &privacy_engine,
        &privacy_map,
        None,
        selected_skill_id.as_deref(),
    )
    .await?
    {
        attach_route_preview_trace(&mut result.reasoning_trace, &route_preview_trace);
        materialize_optional_main_chat_agent_events(state, result.agent_state.as_ref()).await?;
        return Ok(MainChatTurnPipelineOutput {
            route_decision,
            delivery: MainChatTurnDelivery::Buffered {
                result: Box::new(result),
            },
        });
    }

    let legacy_route_decision = route_decision.legacy_compat_fallback();
    let ordinary_plan = ordinary_send_chat_execution_plan(layer);
    let result = send_message_with_legacy_generation(
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
        context_summary,
        ordinary_plan,
        main_chat_agent_turn,
        legacy_route_decision.clone(),
        state,
    )
    .await?;
    let mut result = result;
    attach_route_preview_trace(&mut result.reasoning_trace, &route_preview_trace);
    materialize_optional_main_chat_agent_events(state, result.agent_state.as_ref()).await?;
    Ok(MainChatTurnPipelineOutput {
        route_decision: legacy_route_decision,
        delivery: MainChatTurnDelivery::Buffered {
            result: Box::new(result),
        },
    })
}

pub(crate) async fn run_main_chat_turn_pipeline_streaming(
    input: MainChatTurnPipelineInput,
    state: &Arc<AppState>,
    emit_stream_event: &mut (impl FnMut(&str, serde_json::Value) + Send),
) -> Result<MainChatTurnPipelineOutput, String> {
    debug_assert_eq!(input.stream_mode, MainChatTurnStreamMode::Streaming);
    let MainChatTurnPipelineInput {
        session_id,
        messages,
        selected_skill_id,
        stream_mode: _,
    } = input;
    let user_msg = messages.last().cloned();
    let main_chat_agent_turn = start_main_chat_agent_turn(
        &session_id,
        user_msg.as_ref(),
        openlife_core::agent::AgentTaskKind::Conversation,
        state,
    )
    .await?;

    let route_decision = decide_main_chat_turn_route(
        &main_chat_agent_turn.decision.selected_strategy,
        &messages,
        state,
    )
    .await;
    let route_preview_trace = preview_main_chat_turn_route(
        state,
        &messages,
        &main_chat_agent_turn.decision,
        &route_decision,
        MainChatTurnStreamMode::Streaming,
    )
    .await;
    if route_decision.path.is_kernel_dispatch() {
        let result = {
            let mut event_sink = StreamingMainChatEventSink::new(emit_stream_event);
            run_main_chat_kernel_direct_answer_with_state(
                &session_id,
                messages,
                selected_skill_id,
                state,
                &main_chat_agent_turn,
                &mut event_sink,
                MainChatTurnStreamMode::Streaming.as_str(),
            )
            .await?
        };
        let kernel_event_count = result.kernel_events.len();
        let durable_events = result.durable_events.clone();
        let durable_event_count = durable_events.len();
        let run_id = result.run_id.clone();
        let legacy_fallback_used = result.legacy_fallback_used;
        let mut result = result.into_send_message_result();
        attach_route_preview_trace(&mut result.reasoning_trace, &route_preview_trace);
        let done_payload = emit_stream_send_message_result(
            &session_id,
            result,
            Some(kernel_event_count),
            durable_events,
            false,
            emit_stream_event,
        )?;
        return Ok(MainChatTurnPipelineOutput {
            route_decision,
            delivery: MainChatTurnDelivery::Streamed {
                run_id,
                legacy_fallback_used,
                kernel_event_count: Some(kernel_event_count),
                durable_event_count,
                done_payload,
            },
        });
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
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(&agent_run) {
            log::warn!("[AgentRun] 保存运行记录失败: {}", e);
        }
    }

    let (use_v2, preprocess_options) = {
        let cfg = state.config.lock().await;
        (
            cfg.experimental_context_assembler,
            MainChatPreprocessOptions::from_runtime_mode(&cfg.runtime_mode),
        )
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
        preprocess_chat_input_v2_with_options(&session_id, &messages, state, preprocess_options)
            .await
    } else {
        preprocess_chat_input_with_options(&session_id, &messages, state, preprocess_options).await
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

    if route_decision.path == MainChatExecutionPath::ToolLoop {
        let outcome = run_main_chat_tool_loop_adapter(
            MainChatToolLoopInput {
                session_id: &session_id,
                user_msg: user_msg.as_ref(),
                desensitized_messages: &desensitized_messages,
                life_model: &life_model,
                context_summary: context_summary.clone(),
                embed_err: embed_err.clone(),
                auto_checkin_msg: auto_checkin_msg_stream.clone(),
                main_chat_agent_turn: &main_chat_agent_turn,
                privacy_engine: &privacy_engine,
                privacy_map: &privacy_map,
                existing_agent_run: Some(agent_run.clone()),
                selected_skill_id: selected_skill_id.as_deref(),
            },
            state,
        )
        .await?;
        match outcome {
            MainChatToolLoopOutcome::AgentLoopSuccess(mut result)
            | MainChatToolLoopOutcome::GovernedBlocker(mut result)
            | MainChatToolLoopOutcome::ToolPermissionProposal(mut result)
            | MainChatToolLoopOutcome::SingleStepFallback(mut result) => {
                attach_route_preview_trace(&mut result.reasoning_trace, &route_preview_trace);
                let durable_events =
                    materialize_optional_main_chat_agent_events(state, result.agent_state.as_ref())
                        .await?;
                let durable_event_count = durable_events.len();
                let run_id = result.run_id.clone();
                let legacy_fallback_used = result.legacy_fallback_used;
                let done_payload = emit_stream_send_message_result(
                    &session_id,
                    result,
                    None,
                    durable_events,
                    true,
                    emit_stream_event,
                )?;
                return Ok(MainChatTurnPipelineOutput {
                    route_decision,
                    delivery: MainChatTurnDelivery::Streamed {
                        run_id,
                        legacy_fallback_used,
                        kernel_event_count: None,
                        durable_event_count,
                        done_payload,
                    },
                });
            }
            MainChatToolLoopOutcome::ExplicitFallbackAvailable { reason_code }
            | MainChatToolLoopOutcome::NoResult { reason_code } => {
                if !route_decision.fallback_allowed {
                    return Err(format!(
                        "ToolLoop adapter returned {reason_code}, but legacy fallback is not allowed"
                    ));
                }
                let legacy_route_decision =
                    route_decision.legacy_compat_fallback_with_reason(reason_code);
                return run_legacy_streaming_delivery(
                    &session_id,
                    user_msg,
                    life_model,
                    tools_prompt,
                    privacy_engine,
                    privacy_map,
                    desensitized_messages,
                    embed_err,
                    auto_checkin_msg_stream,
                    layer,
                    context_summary,
                    agent_run,
                    main_chat_agent_turn,
                    legacy_route_decision,
                    state,
                    route_preview_trace.clone(),
                    emit_stream_event,
                )
                .await;
            }
        }
    }

    if let Some(mut result) = try_run_main_chat_agent_strategy(
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
        attach_route_preview_trace(&mut result.reasoning_trace, &route_preview_trace);
        let durable_events =
            materialize_optional_main_chat_agent_events(state, result.agent_state.as_ref()).await?;
        let durable_event_count = durable_events.len();
        let run_id = result.run_id.clone();
        let legacy_fallback_used = result.legacy_fallback_used;
        let done_payload = emit_stream_send_message_result(
            &session_id,
            result,
            None,
            durable_events,
            true,
            emit_stream_event,
        )?;
        return Ok(MainChatTurnPipelineOutput {
            route_decision,
            delivery: MainChatTurnDelivery::Streamed {
                run_id,
                legacy_fallback_used,
                kernel_event_count: None,
                durable_event_count,
                done_payload,
            },
        });
    }

    let legacy_route_decision = route_decision.legacy_compat_fallback();
    run_legacy_streaming_delivery(
        &session_id,
        user_msg,
        life_model,
        tools_prompt,
        privacy_engine,
        privacy_map,
        desensitized_messages,
        embed_err,
        auto_checkin_msg_stream,
        layer,
        context_summary,
        agent_run,
        main_chat_agent_turn,
        legacy_route_decision,
        state,
        route_preview_trace,
        emit_stream_event,
    )
    .await
}

fn emit_stream_send_message_result(
    session_id: &str,
    result: SendMessageResult,
    kernel_event_count: Option<usize>,
    durable_events: Vec<crate::main_chat_event_stream::MainChatAgentDurableEvent>,
    emit_empty_chunk: bool,
    emit_stream_event: &mut (impl FnMut(&str, serde_json::Value) + Send),
) -> Result<serde_json::Value, String> {
    let run_id = result.run_id.clone().unwrap_or_default();
    let agent_state = result.agent_state.clone();
    let mut start_payload = serde_json::json!({
        "session_id": session_id,
        "run_id": run_id,
        "reasoning_trace": result.reasoning_trace.clone(),
        "tool_calls": result.tool_calls.clone(),
        "agent_ingress": result.agent_ingress.clone(),
        "agent_state": agent_state.clone(),
        "execution_transcript": result.execution_transcript.clone(),
        "legacy_fallback_used": result.legacy_fallback_used,
    });
    if let Some(count) = kernel_event_count {
        start_payload["kernel_event_count"] = serde_json::json!(count);
    }
    emit_stream_event("stream-message-start", start_payload);
    if emit_empty_chunk || !result.reply.is_empty() {
        emit_stream_event(
            "stream-message-chunk",
            serde_json::json!({
                "session_id": session_id,
                "run_id": run_id,
                "chunk": result.reply.clone(),
            }),
        );
    }
    let mut done_payload = serde_json::json!({
        "session_id": session_id,
        "run_id": run_id,
        "reply": result.reply,
        "reasoning_trace": result.reasoning_trace,
        "tool_calls": result.tool_calls,
        "agent_ingress": result.agent_ingress,
        "agent_state": agent_state,
        "execution_transcript": result.execution_transcript,
        "legacy_fallback_used": result.legacy_fallback_used,
    });
    if let Some(count) = kernel_event_count {
        done_payload["kernel_event_count"] = serde_json::json!(count);
    }
    emit_stream_event("stream-message-done", done_payload.clone());
    for event in durable_events {
        emit_stream_event(
            "main-chat-agent-event",
            serde_json::to_value(event).map_err(|err| err.to_string())?,
        );
    }
    Ok(done_payload)
}

#[allow(clippy::too_many_arguments)]
async fn run_legacy_streaming_delivery(
    session_id: &str,
    user_msg: Option<ChatMessage>,
    life_model: openlife_core::life_model::LifeModel,
    tools_prompt: String,
    privacy_engine: openlife_core::privacy::PrivacyEngine,
    privacy_map: std::collections::HashMap<String, String>,
    desensitized_messages: Vec<ChatMessage>,
    embed_err: Option<String>,
    auto_checkin_msg_stream: Option<String>,
    layer: Layer,
    context_summary: openlife_core::agent::ContextSummary,
    mut agent_run: openlife_core::agent::AgentRun,
    main_chat_agent_turn: crate::main_chat_runtime_support::MainChatAgentTurn,
    legacy_route_decision: MainChatTurnRouteDecision,
    state: &Arc<AppState>,
    route_preview_trace: MainChatRoutePreviewTrace,
    emit_stream_event: &mut (impl FnMut(&str, serde_json::Value) + Send),
) -> Result<MainChatTurnPipelineOutput, String> {
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
        session_id: session_id.to_string(),
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
            "session_id": session_id,
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
                    "session_id": session_id,
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
                                emit_stream_error(
                                    session_id,
                                    &agent_run.id,
                                    &message,
                                    emit_stream_event,
                                );
                                fail_stream_agent_run(state, &mut agent_run, &message).await;
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
                                    "session_id": session_id,
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
                                emit_stream_error(
                                    session_id,
                                    &agent_run.id,
                                    &message,
                                    emit_stream_event,
                                );
                                fail_stream_agent_run(state, &mut agent_run, &message).await;
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
                        emit_stream_error(session_id, &agent_run.id, &message, emit_stream_event);
                        fail_stream_agent_run(state, &mut agent_run, &message).await;
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
                    emit_stream_error(session_id, &agent_run.id, &message, emit_stream_event);
                    fail_stream_agent_run(state, &mut agent_run, &message).await;
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
    attach_route_preview_trace(&mut reasoning_trace, &route_preview_trace);
    if let Err(e) = finalize_chat_agent_run(
        session_id,
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
        emit_stream_error(
            session_id,
            &agent_run.id,
            &format!("AgentRun 持久化失败: {}", e),
            emit_stream_event,
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
                    "executionPath": legacy_route_decision.execution_path_label(),
                    "routeDecisionReasonCode": legacy_route_decision.reason_code,
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
    let durable_event_count = durable_events.len();

    let done_payload = serde_json::json!({
            "session_id": session_id,
            "run_id": agent_run.id,
            "reply": reply,
            "reasoning_trace": reasoning_trace,
            "tool_calls": tool_calls,
            "agent_ingress": main_chat_agent_turn.decision.clone(),
            "agent_state": agent_state,
            "execution_transcript": execution_transcript,
            "legacy_fallback_used": legacy_fallback_used,
    });
    emit_stream_event("stream-message-done", done_payload.clone());
    for event in durable_events {
        emit_stream_event(
            "main-chat-agent-event",
            serde_json::to_value(event).map_err(|err| err.to_string())?,
        );
    }

    Ok(MainChatTurnPipelineOutput {
        route_decision: legacy_route_decision,
        delivery: MainChatTurnDelivery::Streamed {
            run_id: Some(agent_run.id),
            legacy_fallback_used,
            kernel_event_count: None,
            durable_event_count,
            done_payload,
        },
    })
}

fn emit_stream_error(
    session_id: &str,
    run_id: &str,
    message: &str,
    emit_stream_event: &mut (impl FnMut(&str, serde_json::Value) + Send),
) {
    emit_stream_event(
        "stream-message-error",
        serde_json::json!({
            "session_id": session_id,
            "run_id": run_id,
            "error": message,
        }),
    );
}

async fn fail_stream_agent_run(
    state: &Arc<AppState>,
    agent_run: &mut openlife_core::agent::AgentRun,
    message: &str,
) {
    let error = openlife_core::agent::AgentRunError {
        message: message.to_string(),
        phase: "stream".to_string(),
        recoverable: true,
    };
    agent_run.fail(error);
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.update_run(agent_run) {
            log::warn!("[AgentRun] 更新运行记录失败: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::main_chat_agent_v1::{AgentIngress, MainChatAgentStrategy};
    use openlife_core::agent::AgentTaskKind;

    fn user_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: "user".into(),
            content: content.into(),
        }
    }

    fn decision_for_user_text(user_text: &str) -> MainChatTurnRouteDecision {
        let ingress = AgentIngress::default();
        let ingress_decision = ingress.decide(
            "route-decision-test-session",
            user_text,
            None,
            AgentTaskKind::Conversation,
        );
        let messages = vec![user_message(user_text)];
        let disposition =
            main_chat_kernel_support_disposition(&ingress_decision.selected_strategy, &messages);
        decide_main_chat_turn_route_from_disposition(
            ingress_decision.selected_strategy,
            disposition,
            false,
            false,
        )
    }

    #[test]
    fn main_chat_turn_route_decision_maps_current_kernel_paths() {
        let cases = [
            (
                "Explain focused work in one concise paragraph.",
                MainChatAgentStrategy::DirectAnswer,
                MainChatExecutionPath::KernelDirect,
                "kernel_supported_direct_answer",
            ),
            (
                "Read Cargo.toml as a governed workspace file observation.",
                MainChatAgentStrategy::ReActToolExecution,
                MainChatExecutionPath::KernelReadTool,
                "kernel_supported_read_tool",
            ),
            (
                "Draft a weekly plan and break this goal into steps.",
                MainChatAgentStrategy::PlanExecute,
                MainChatExecutionPath::PlanExecute,
                "kernel_supported_plan_execute",
            ),
            (
                "Please remember that I prefer morning writing blocks.",
                MainChatAgentStrategy::MemoryProposal,
                MainChatExecutionPath::KernelWriteOutcome,
                "kernel_supported_write_outcome",
            ),
            (
                "Send this private medical update to my coworker.",
                MainChatAgentStrategy::BlockedConfirmation,
                MainChatExecutionPath::KernelWriteOutcome,
                "kernel_supported_write_outcome",
            ),
        ];

        for (user_text, expected_strategy, expected_path, expected_reason) in cases {
            let decision = decision_for_user_text(user_text);
            assert_eq!(decision.strategy_label, expected_strategy.as_str());
            assert_eq!(decision.path, expected_path, "{user_text}");
            assert_eq!(decision.reason_code, expected_reason, "{user_text}");
            assert!(decision.kernel_supported, "{user_text}");
            assert!(!decision.fallback_allowed, "{user_text}");
            assert!(!decision.requires_provider, "{user_text}");
            assert!(!decision.requires_tool_loop, "{user_text}");
        }
    }

    #[test]
    fn main_chat_turn_route_decision_keeps_tool_loop_and_legacy_explicit() {
        let provider_backed = decide_main_chat_turn_route_from_disposition(
            MainChatAgentStrategy::ReActToolExecution,
            MainChatKernelSupportDisposition::KernelSupported,
            true,
            false,
        );
        assert_eq!(provider_backed.path, MainChatExecutionPath::ToolLoop);
        assert_eq!(
            provider_backed.reason_code,
            "provider_backed_react_required"
        );
        assert!(provider_backed.fallback_allowed);
        assert!(provider_backed.requires_provider);
        assert!(provider_backed.requires_tool_loop);

        let governed_selection = decide_main_chat_turn_route_from_disposition(
            MainChatAgentStrategy::ReActToolExecution,
            MainChatKernelSupportDisposition::KernelSupported,
            false,
            true,
        );
        assert_eq!(governed_selection.path, MainChatExecutionPath::ToolLoop);
        assert_eq!(
            governed_selection.reason_code,
            "governed_agent_loop_candidate_selection_required"
        );
        assert!(governed_selection.fallback_allowed);
        assert!(!governed_selection.requires_provider);
        assert!(governed_selection.requires_tool_loop);

        let legacy = governed_selection.legacy_compat_fallback();
        assert_eq!(legacy.path, MainChatExecutionPath::LegacyCompatFallback);
        assert_eq!(
            legacy.execution_path_label(),
            MainChatExecutionPath::LegacyCompatFallback.as_str()
        );
        assert_eq!(legacy.reason_code, "legacy_compat_after_strategy_no_result");
        assert!(legacy.fallback_allowed);
        assert!(!legacy.requires_tool_loop);
    }

    #[test]
    fn main_chat_send_stream_route_parity_table_uses_single_decision_object() {
        use crate::main_chat_command_surface_eval::{
            main_chat_command_surface_eval_user_text, MainChatCommandSurfaceEvalScenario,
        };

        let cases = [
            (
                "direct_answer",
                MainChatCommandSurfaceEvalScenario::DirectProviderTrace,
                MainChatExecutionPath::KernelDirect,
            ),
            (
                "read_tool_file",
                MainChatCommandSurfaceEvalScenario::FileReadSuccess,
                MainChatExecutionPath::KernelReadTool,
            ),
            (
                "plan_execute_draft",
                MainChatCommandSurfaceEvalScenario::PlanExecuteDraft,
                MainChatExecutionPath::PlanExecute,
            ),
            (
                "proposal_path",
                MainChatCommandSurfaceEvalScenario::ProposalPath,
                MainChatExecutionPath::KernelWriteOutcome,
            ),
            (
                "web_blocker",
                MainChatCommandSurfaceEvalScenario::WebPolicyBlocker,
                MainChatExecutionPath::KernelReadTool,
            ),
            (
                "registered_mcp_success",
                MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess,
                MainChatExecutionPath::KernelReadTool,
            ),
            (
                "tool_permission_proposal",
                MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal,
                MainChatExecutionPath::KernelReadTool,
            ),
        ];

        for (label, scenario, expected_path) in cases {
            let user_text = main_chat_command_surface_eval_user_text(scenario);
            let send_decision = decision_for_user_text(user_text);
            let stream_decision = decision_for_user_text(user_text);
            assert_eq!(send_decision, stream_decision, "{label}");
            assert_eq!(send_decision.path, expected_path, "{label}");
            assert!(send_decision.path.is_kernel_dispatch(), "{label}");
            assert!(!send_decision.fallback_allowed, "{label}");
        }

        let legacy_eligible = decide_main_chat_turn_route_from_disposition(
            MainChatAgentStrategy::ReActToolExecution,
            MainChatKernelSupportDisposition::KernelSupported,
            false,
            true,
        );
        assert_eq!(legacy_eligible.path, MainChatExecutionPath::ToolLoop);
        assert!(legacy_eligible.fallback_allowed);
        assert_eq!(
            legacy_eligible.legacy_compat_fallback().path,
            MainChatExecutionPath::LegacyCompatFallback
        );
    }

    #[test]
    fn main_chat_send_stream_route_parity_source_guard_prevents_local_branch_reimplementation() {
        let send_source = include_str!("main_chat_send.rs");
        let stream_source = include_str!("main_chat_streaming.rs");
        let pipeline_source = include_str!("main_chat_turn_pipeline.rs");

        for (label, source) in [("send", send_source), ("stream", stream_source)] {
            assert!(
                source.contains("run_main_chat_turn_pipeline_"),
                "{label} must delegate to the MainChatTurnPipeline wrapper"
            );
            assert_eq!(
                source.matches("decide_main_chat_turn_route(").count(),
                0,
                "{label} must not own route-decision branching after Phase 3"
            );
            assert_eq!(
                source.matches("try_run_main_chat_agent_strategy(").count(),
                0,
                "{label} must not own strategy fallback selection after Phase 3"
            );
            assert_eq!(
                source.matches("legacy_compat_fallback()").count(),
                0,
                "{label} must not own explicit legacy fallback selection after Phase 3"
            );
            assert_eq!(
                source.matches("main_chat_kernel_supports_turn(").count(),
                0,
                "{label} must not reimplement kernel support branching"
            );
            assert_eq!(
                source
                    .matches("main_chat_live_provider_eval_requires_provider_backed_react(")
                    .count(),
                0,
                "{label} must not reimplement live-provider ReAct branching"
            );
            assert_eq!(
                source
                    .matches(
                        "main_chat_react_turn_requires_governed_agent_loop_candidate_selection("
                    )
                    .count(),
                0,
                "{label} must not reimplement governed AgentLoop candidate branching"
            );
        }

        assert!(
            pipeline_source.contains("main_chat_kernel_support_disposition("),
            "the shared pipeline helper owns kernel support disposition"
        );
        assert!(
            pipeline_source
                .contains("main_chat_live_provider_eval_requires_provider_backed_react("),
            "the shared pipeline helper owns live-provider ReAct routing"
        );
        assert!(
            pipeline_source
                .contains("main_chat_react_turn_requires_governed_agent_loop_candidate_selection("),
            "the shared pipeline helper owns governed candidate-selection routing"
        );
        assert!(
            pipeline_source.contains("pub(crate) struct MainChatTurnPipelineInput"),
            "the pipeline exposes a typed input boundary"
        );
        assert!(
            pipeline_source.contains("pub(crate) struct MainChatTurnPipelineOutput"),
            "the pipeline exposes a typed output boundary"
        );
        assert!(
            pipeline_source.contains("pub(crate) enum MainChatTurnDelivery"),
            "the pipeline exposes typed delivery evidence"
        );
        assert!(
            pipeline_source.contains("run_main_chat_tool_loop_adapter("),
            "the pipeline must call the ToolLoop adapter for ToolLoop route decisions"
        );
        assert!(
            pipeline_source.contains("route_decision.path == MainChatExecutionPath::ToolLoop"),
            "ToolLoop dispatch must be gated by the typed route decision"
        );
        assert!(
            pipeline_source.contains("MainChatToolLoopOutcome::ExplicitFallbackAvailable")
                && pipeline_source.contains("MainChatToolLoopOutcome::NoResult"),
            "legacy fallback after ToolLoop must be explicit outcome handling"
        );
        assert!(
            pipeline_source.contains("route_decision: legacy_route_decision"),
            "explicit legacy fallback must be reflected in the pipeline output route evidence"
        );
    }
}
