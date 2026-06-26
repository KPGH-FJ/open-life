use openlife_core::agent::ModelRouteTrace;
use openlife_core::scheduler::InferenceScheduler;
use std::sync::Arc;

use super::agent_self_state::resolve_agent_self_state_fact_answer;
use super::clock::{resolve_runtime_clock_fact_answer, MainChatRuntimeClockSource};
use super::contract::{MainChatProviderRouteIntent, MainChatRuntimeFactAnswer};
use super::provider_route::{
    classify_provider_route_query, provider_route_fact_should_block_before_model,
    resolve_provider_route_fact_answer,
};
use super::tool_availability::resolve_tool_availability_fact_answer;
use crate::AppState;

pub(crate) struct MainChatRuntimeFactPreModelRequest<'a> {
    pub(crate) user_text: &'a str,
    pub(crate) state: &'a Arc<AppState>,
    pub(crate) scheduler: &'a InferenceScheduler,
    pub(crate) session_id: &'a str,
    pub(crate) current_task_session_id: Option<&'a str>,
    pub(crate) clock_source: MainChatRuntimeClockSource,
    pub(crate) provider_generation_path: &'a str,
}

pub(crate) async fn resolve_pre_model_runtime_fact_answer(
    request: MainChatRuntimeFactPreModelRequest<'_>,
) -> Option<MainChatRuntimeFactAnswer> {
    if request.user_text.trim().is_empty() {
        return None;
    }

    if let Some(answer) =
        resolve_runtime_clock_fact_answer(request.user_text, &request.clock_source)
    {
        return Some(answer);
    }

    match classify_provider_route_query(request.user_text) {
        Some(MainChatProviderRouteIntent::AskPreviousTurnModelRoute) => {
            return resolve_provider_route_fact_answer(
                request.user_text,
                request.state,
                request.scheduler,
                request.session_id,
                None,
                false,
                false,
                request.provider_generation_path,
            )
            .await;
        }
        Some(MainChatProviderRouteIntent::AskCurrentModelRoute)
            if provider_route_fact_should_block_before_model(request.state, request.scheduler)
                .await =>
        {
            return resolve_provider_route_fact_answer(
                request.user_text,
                request.state,
                request.scheduler,
                request.session_id,
                None,
                false,
                false,
                request.provider_generation_path,
            )
            .await;
        }
        _ => {}
    }

    if let Some(answer) =
        resolve_tool_availability_fact_answer(request.user_text, request.state).await
    {
        return Some(answer);
    }

    resolve_agent_self_state_fact_answer(
        request.user_text,
        request.state,
        request.session_id,
        request.current_task_session_id,
    )
    .await
}

pub(crate) struct MainChatRuntimeFactPostModelRequest<'a> {
    pub(crate) user_text: &'a str,
    pub(crate) state: &'a Arc<AppState>,
    pub(crate) scheduler: &'a InferenceScheduler,
    pub(crate) session_id: &'a str,
    pub(crate) current_route: ModelRouteTrace,
    pub(crate) current_model_generated: bool,
    pub(crate) scheduler_generation_called: bool,
    pub(crate) provider_generation_path: &'a str,
}

pub(crate) async fn resolve_post_model_runtime_fact_answer(
    request: MainChatRuntimeFactPostModelRequest<'_>,
) -> Option<MainChatRuntimeFactAnswer> {
    if classify_provider_route_query(request.user_text)
        != Some(MainChatProviderRouteIntent::AskCurrentModelRoute)
    {
        return None;
    }

    resolve_provider_route_fact_answer(
        request.user_text,
        request.state,
        request.scheduler,
        request.session_id,
        Some(request.current_route),
        request.current_model_generated,
        request.scheduler_generation_called,
        request.provider_generation_path,
    )
    .await
}
