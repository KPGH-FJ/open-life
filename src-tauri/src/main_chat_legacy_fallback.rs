use openlife_core::layer_router::Layer;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::privacy::PrivacyEngine;
use std::collections::HashMap;
use std::sync::Arc;

use crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn;
use crate::main_chat_generation_support::{
    finalize_chat_agent_run, generate_non_stream_fallback, preview_text,
};
use crate::main_chat_hs_runtime::build_chat_runtime_hs_packet;
use crate::main_chat_runtime_support::{append_main_chat_agent_transcript, MainChatAgentTurn};
use crate::main_chat_turn_pipeline::MainChatTurnRouteDecision;
use crate::{AppState, SendMessageResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrdinaryChatRouteKind {
    LegacyNonStream,
    LegacyStream,
    DirectReflex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OrdinaryChatExecutionPlan {
    pub(crate) route_kind: OrdinaryChatRouteKind,
    pub(crate) constructs_agent_loop: bool,
    pub(crate) constructs_action_executor: bool,
    pub(crate) tool_execution_allowed: bool,
    pub(crate) agent_actions_allowed: bool,
    pub(crate) agent_observations_allowed: bool,
    pub(crate) mcp_audit_write_allowed: bool,
    pub(crate) external_write_allowed: bool,
    pub(crate) plan_execute_allowed: bool,
    pub(crate) golden_path_allowed: bool,
    pub(crate) final_gate_allowed: bool,
    pub(crate) guidance_consumption_enabled: bool,
}

impl OrdinaryChatExecutionPlan {
    fn legacy(route_kind: OrdinaryChatRouteKind) -> Self {
        Self {
            route_kind,
            constructs_agent_loop: false,
            constructs_action_executor: false,
            tool_execution_allowed: false,
            agent_actions_allowed: false,
            agent_observations_allowed: false,
            mcp_audit_write_allowed: false,
            external_write_allowed: false,
            plan_execute_allowed: false,
            golden_path_allowed: false,
            final_gate_allowed: false,
            guidance_consumption_enabled: false,
        }
    }
}

pub(crate) fn ordinary_send_chat_execution_plan(_layer: Layer) -> OrdinaryChatExecutionPlan {
    OrdinaryChatExecutionPlan::legacy(OrdinaryChatRouteKind::LegacyNonStream)
}

pub(crate) fn ordinary_stream_chat_execution_plan(layer: Layer) -> OrdinaryChatExecutionPlan {
    if layer == Layer::L1 {
        OrdinaryChatExecutionPlan::legacy(OrdinaryChatRouteKind::DirectReflex)
    } else {
        OrdinaryChatExecutionPlan::legacy(OrdinaryChatRouteKind::LegacyStream)
    }
}

/// Legacy non-stream fallback used only when Main Chat v1 returns no governed
/// result. It intentionally does not construct AgentLoop, ActionExecutor, or
/// tool actions.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn send_message_with_legacy_generation(
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
    context_summary: openlife_core::agent::types::ContextSummary,
    ordinary_plan: OrdinaryChatExecutionPlan,
    main_chat_agent_turn: MainChatAgentTurn,
    legacy_route_decision: MainChatTurnRouteDecision,
    state: &Arc<AppState>,
) -> Result<SendMessageResult, String> {
    debug_assert_eq!(
        ordinary_plan.route_kind,
        OrdinaryChatRouteKind::LegacyNonStream
    );
    debug_assert!(!ordinary_plan.constructs_agent_loop);
    debug_assert!(!ordinary_plan.constructs_action_executor);
    debug_assert!(!ordinary_plan.tool_execution_allowed);

    let scheduler = state.scheduler.lock().await.clone();
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

    let mut reply = generate_non_stream_fallback(
        &scheduler,
        desensitized_messages,
        &life_model,
        &tools_prompt,
        hs_packet.clone(),
    )
    .await?;
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
    let user_input_text = user_msg
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text);
    let legacy_fallback_used = main_chat_agent_turn.decision.selected_strategy
        != openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer;
    agent_run.reasoning_strategy = Some(if legacy_fallback_used {
        format!(
            "main_chat_agent_v1_{}_legacy_fallback",
            main_chat_agent_turn.decision.selected_strategy.as_str()
        )
    } else {
        "main_chat_agent_v1_direct_answer".to_string()
    });
    agent_run.hs_selection_audit = hs_packet.as_ref().map(|packet| packet.audit.clone());
    let model_route = scheduler.preview_chat_route(Some(&tools_prompt)).await;
    agent_run.complete(&preview_text(&reply, 200), model_route, context_summary);

    let mut reasoning_trace = openlife_core::agent::ReasoningTrace {
        generation_result: Some(serde_json::json!({ "text": reply })),
        ..Default::default()
    };
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
        state,
    )
    .await?;
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
                "Legacy generation fallback was used for this Main Chat turn.",
                serde_json::json!({
                    "runId": agent_run.id,
                    "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                    "executionPath": legacy_route_decision.execution_path_label(),
                    "routeDecisionReasonCode": legacy_route_decision.reason_code,
                    "fallbackReason": "strategy_executor_not_yet_available_for_this_path",
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
    let agent_state = assemble_main_chat_agent_state_for_turn(
        state,
        main_chat_agent_turn
            .decision
            .agent_task_session_id
            .as_deref(),
        Some(&agent_run.id),
    )
    .await;

    Ok(SendMessageResult {
        reply,
        reasoning_trace,
        tool_calls: Vec::new(),
        run_id: Some(agent_run.id.clone()),
        agent_ingress: Some(main_chat_agent_turn.decision),
        agent_state,
        execution_transcript,
        legacy_fallback_used,
    })
}
