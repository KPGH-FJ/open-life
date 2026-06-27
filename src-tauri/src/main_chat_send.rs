use openlife_core::layer_router::Layer;
use openlife_core::llm::ChatMessage;
use std::sync::Arc;

use crate::legacy_write_convergence::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::main_chat_conversation_updates::{
    capture_conversation_signals, try_auto_checkin_daily_goals,
};
use crate::main_chat_event_stream::materialize_optional_main_chat_agent_events;
use crate::main_chat_kernel::{
    main_chat_kernel_supports_turn, main_chat_live_provider_eval_requires_provider_backed_react,
    main_chat_react_turn_requires_governed_agent_loop_candidate_selection,
    run_main_chat_kernel_direct_answer_with_state, BufferedMainChatEventSink,
};
use crate::main_chat_legacy_fallback::{
    ordinary_send_chat_execution_plan, send_message_with_legacy_generation,
};
use crate::main_chat_preprocess::{preprocess_chat_input, preprocess_chat_input_v2};
use crate::main_chat_runtime_support::start_main_chat_agent_turn;
use crate::main_chat_strategy::try_run_main_chat_agent_strategy;
use crate::{persist_life_model, AppState, SendMessageResult};

pub(crate) async fn send_message_with_state(
    session_id: String,
    messages: Vec<ChatMessage>,
    selected_skill_id: Option<String>,
    state: &Arc<AppState>,
) -> Result<SendMessageResult, String> {
    let user_msg = messages.last().cloned();
    let main_chat_agent_turn = start_main_chat_agent_turn(
        &session_id,
        user_msg.as_ref(),
        openlife_core::agent::AgentTaskKind::Conversation,
        state,
    )
    .await?;

    let kernel_supported =
        main_chat_kernel_supports_turn(&main_chat_agent_turn.decision.selected_strategy, &messages);
    let live_eval_provider_backed_react_required =
        main_chat_live_provider_eval_requires_provider_backed_react(
            &main_chat_agent_turn.decision.selected_strategy,
            state,
        )
        .await;
    let governed_agent_loop_candidate_selection_required =
        main_chat_react_turn_requires_governed_agent_loop_candidate_selection(
            &main_chat_agent_turn.decision.selected_strategy,
            &messages,
            state,
        )
        .await;
    if kernel_supported
        && !live_eval_provider_backed_react_required
        && !governed_agent_loop_candidate_selection_required
    {
        let mut event_sink = BufferedMainChatEventSink::default();
        let result = run_main_chat_kernel_direct_answer_with_state(
            &session_id,
            messages,
            selected_skill_id,
            state,
            &main_chat_agent_turn,
            &mut event_sink,
            "buffered",
        )
        .await?;
        return Ok(result.into_send_message_result());
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
