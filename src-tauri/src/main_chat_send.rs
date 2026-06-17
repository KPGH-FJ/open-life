use openlife_core::agent::ReasoningTrace;
use openlife_core::layer_router::Layer;
use openlife_core::llm::ChatMessage;
use std::sync::Arc;

use crate::legacy_write_convergence::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn;
use crate::main_chat_conversation_updates::{
    capture_conversation_signals, try_auto_checkin_daily_goals,
};
use crate::main_chat_event_stream::materialize_optional_main_chat_agent_events;
use crate::main_chat_generation_support::{
    finalize_chat_agent_run, persist_chat_message_if_needed, persist_vector_memory_for_message,
    preview_text,
};
use crate::main_chat_legacy_fallback::{
    ordinary_send_chat_execution_plan, send_message_with_legacy_generation,
};
use crate::main_chat_preprocess::{preprocess_chat_input, preprocess_chat_input_v2};
use crate::main_chat_runtime_support::{
    append_main_chat_agent_transcript, append_main_chat_direct_answer_contract_transcript,
    complete_main_chat_agent_turn_session, start_main_chat_agent_turn,
};
use crate::main_chat_strategy::try_run_main_chat_agent_strategy;
use crate::{persist_life_model, AppState, SendMessageResult};

pub(crate) async fn send_message_with_state(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
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
    let main_chat_agent_turn = start_main_chat_agent_turn(
        &session_id,
        user_msg.as_ref(),
        openlife_core::agent::AgentTaskKind::Conversation,
        state,
    )
    .await?;

    // Layer 1: direct reflex response.
    if layer == Layer::L1 {
        if let Some(ref i) = intent {
            if let Some(reply) = i.direct_response() {
                let user_text = user_msg
                    .as_ref()
                    .map(|message| message.content.as_str())
                    .unwrap_or_default();
                let mut execution_transcript = main_chat_agent_turn.transcript_entries.clone();
                execution_transcript.extend(
                    append_main_chat_direct_answer_contract_transcript(
                        state,
                        &main_chat_agent_turn,
                        user_text,
                        selected_skill_id.as_deref(),
                    )
                    .await,
                );
                if let Some(ref user) = user_msg {
                    if user.role == "user" {
                        let inserted =
                            persist_chat_message_if_needed(&session_id, user, state).await?;
                        if inserted {
                            persist_vector_memory_for_message(&session_id, user, state).await;
                        }
                    }
                }
                let assistant_msg = ChatMessage {
                    role: "assistant".into(),
                    content: reply.clone(),
                };

                let mut agent_run = openlife_core::agent::AgentRun::new_chat_run(
                    &session_id,
                    &user_msg
                        .as_ref()
                        .map(|m| m.content.clone())
                        .unwrap_or_default(),
                );
                agent_run.reasoning_strategy = Some("main_chat_agent_v1_direct_answer".into());
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
                    state,
                )
                .await?;
                complete_main_chat_agent_turn_session(
                    state,
                    &main_chat_agent_turn,
                    "DirectAnswer completed without tool execution.",
                )
                .await;
                execution_transcript.extend(
                    append_main_chat_agent_transcript(
                        state,
                        main_chat_agent_turn
                            .decision
                            .agent_task_session_id
                            .as_deref(),
                        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind::FinalResult,
                        "DirectAnswer completed without tool execution.",
                        serde_json::json!({
                            "runId": agent_run.id,
                            "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                            "legacyFallbackUsed": false,
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
                materialize_optional_main_chat_agent_events(state, agent_state.as_ref()).await?;

                return Ok(SendMessageResult {
                    reply,
                    reasoning_trace,
                    tool_calls: vec![],
                    run_id: Some(agent_run.id.clone()),
                    agent_ingress: Some(main_chat_agent_turn.decision),
                    agent_state,
                    execution_transcript,
                    legacy_fallback_used: false,
                });
            }
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
    ) = if use_v2 {
        preprocess_chat_input_v2(&session_id, &messages, state).await?
    } else {
        preprocess_chat_input(&session_id, &messages, state).await?
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

    if let Some(result) = try_run_main_chat_agent_strategy(
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
        materialize_optional_main_chat_agent_events(state, result.agent_state.as_ref()).await?;
        return Ok(result);
    }

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
        state,
    )
    .await?;
    materialize_optional_main_chat_agent_events(state, result.agent_state.as_ref()).await?;
    Ok(result)
}
