use openlife_core::agent::{AgentRunError, ReasoningTrace};
use openlife_core::layer_router::Layer;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::privacy::PrivacyEngine;
use std::collections::HashMap;
use std::sync::Arc;

use crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn;
use crate::main_chat_runtime_support::{
    append_main_chat_agent_transcript, finalize_main_chat_task_failure, MainChatAgentTurn,
    MainChatTaskFailureKind,
};
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

/// Retired non-stream fallback delivery used only when Main Chat v1 returns no
/// governed result. It records the blocked fallback for audit without invoking
/// the old runtime, provider model, tools, or assistant success delivery.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_retired_buffered_fallback_delivery(
    session_id: String,
    user_msg: Option<ChatMessage>,
    _life_model: LifeModel,
    _tools_prompt: String,
    _privacy_engine: PrivacyEngine,
    _privacy_map: HashMap<String, String>,
    _desensitized_messages: Vec<ChatMessage>,
    embed_err: Option<String>,
    _auto_checkin_msg: Option<String>,
    _layer: Layer,
    _context_summary: openlife_core::agent::types::ContextSummary,
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

    let user_input_text = user_msg
        .as_ref()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(&session_id, &user_input_text);
    let legacy_fallback_used = true;
    let fallback_reason_code = legacy_route_decision.reason_code.clone();
    let blocker_code = "retired_buffered_runtime_fallback_blocked";

    crate::main_chat_runtime_status::record_main_chat_legacy_fallback(
        state,
        fallback_reason_code.clone(),
    )
    .await;
    crate::main_chat_runtime_status::record_main_chat_turn_route_evidence(
        state,
        &legacy_route_decision,
        crate::main_chat_turn_pipeline::MainChatTurnStreamMode::Buffered,
        false,
        legacy_fallback_used,
        None,
    )
    .await;

    let reply = "Main Chat buffered retired runtime fallback is blocked. This turn did not invoke the old AgentRuntime, tools, or provider model.".to_string();
    let mut reasoning_trace = ReasoningTrace {
        generation_result: Some(serde_json::json!({
            "status": "failed",
            "blockerCode": blocker_code,
            "legacyFallbackUsed": true,
            "legacyFallbackReasonCode": fallback_reason_code.clone(),
            "reasonCode": fallback_reason_code.clone(),
            "legacyRuntimeInvoked": false,
            "modelInvoked": false,
            "toolInvoked": false,
        })),
        ..Default::default()
    };
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }

    agent_run.reasoning_strategy = Some(format!(
        "main_chat_agent_v1_{}_retired_buffered_blocked",
        main_chat_agent_turn.decision.selected_strategy.as_str()
    ));
    agent_run.output_preview = Some(reply.clone());
    agent_run.reasoning_trace = Some(reasoning_trace.clone());
    agent_run.fail(AgentRunError {
        message: blocker_code.into(),
        phase: "fallback".into(),
        recoverable: true,
    });
    if let Some(ref store_arc) = state.agent_run_store {
        let store = store_arc.lock().await;
        if let Err(e) = store.create_run(&agent_run) {
            log::warn!("[AgentRun] retired buffered fallback run create failed: {}", e);
        }
    }

    let task_session_id = main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref();
    let mut execution_transcript = main_chat_agent_turn.transcript_entries.clone();
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            task_session_id,
            openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Fallback,
            "Retired buffered fallback was blocked for this Main Chat turn.",
            serde_json::json!({
                "runId": agent_run.id,
                "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                "executionPath": legacy_route_decision.execution_path_label(),
                "routeDecisionReasonCode": fallback_reason_code.clone(),
                "fallbackReason": blocker_code,
                "fallbackVisible": true,
                "legacyRuntimeInvoked": false,
                "modelInvoked": false,
                "toolInvoked": false,
            }),
        )
        .await,
    );
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            task_session_id,
            openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::Error,
            "Main Chat buffered fallback was blocked before invoking legacy runtime.",
            serde_json::json!({
                "runId": agent_run.id,
                "blockerCode": blocker_code,
                "legacyFallbackUsed": true,
                "legacyFallbackReasonCode": fallback_reason_code.clone(),
            }),
        )
        .await,
    );
    if let Err(e) = finalize_main_chat_task_failure(
        state,
        Some(&agent_run.id),
        task_session_id,
        MainChatTaskFailureKind::PolicyBlocker,
        blocker_code,
        "main_chat_legacy_fallback.run_retired_buffered_fallback_delivery",
    )
    .await
    {
        log::warn!(
            "[AgentRun] retired buffered fallback failure finalizer failed: {}",
            e
        );
    }

    let agent_state = assemble_main_chat_agent_state_for_turn(
        state,
        task_session_id,
        Some(&agent_run.id),
    )
    .await;

    Ok(SendMessageResult {
        reply,
        status: "failed".into(),
        blockers: vec![blocker_code.into()],
        reasoning_trace,
        tool_calls: Vec::new(),
        run_id: Some(agent_run.id.clone()),
        agent_ingress: Some(main_chat_agent_turn.decision),
        agent_state,
        execution_transcript,
        legacy_fallback_used,
        legacy_runtime_invoked: false,
        model_invoked: false,
        tool_invoked: false,
    })
}
