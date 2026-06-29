#![allow(unused_imports)]

mod agent_self_state;
mod clock;
mod contract;
mod eval;
mod provider_route;
mod registry;
mod resolver;
mod tool_availability;

pub(crate) use agent_self_state::{
    classify_agent_self_state_query, resolve_agent_self_state_fact_answer,
};
pub(crate) use clock::{
    classify_runtime_clock_query, resolve_runtime_clock_fact_answer, MainChatRuntimeClockSource,
};
pub(crate) use contract::{
    MainChatAgentSelfStateIntent, MainChatProviderRouteIntent, MainChatRuntimeClockIntent,
    MainChatRuntimeFactAnswer, MainChatRuntimeFactBinding, MainChatToolAvailabilityIntent,
    RUNTIME_FACT_AGENT_SELF_STATE_GENERATION_PATH, RUNTIME_FACT_KEY_AGENT_BLOCKER_CODES,
    RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS, RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY,
    RUNTIME_FACT_KEY_AGENT_PENDING_PERMISSION_COUNT, RUNTIME_FACT_KEY_AGENT_TASK_STATUS,
    RUNTIME_FACT_KEY_AGENT_TRACE_GAP, RUNTIME_FACT_KEY_DATE,
    RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_MODEL,
    RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER, RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL,
    RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED, RUNTIME_FACT_KEY_PROVIDER_CURRENT_PROVIDER,
    RUNTIME_FACT_KEY_PROVIDER_CURRENT_ROUTE_TYPE, RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_MODEL,
    RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_PROVIDER,
    RUNTIME_FACT_KEY_PROVIDER_LAST_COMPLETED_RUN_ID, RUNTIME_FACT_KEY_PROVIDER_PLANNED_MODEL,
    RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER, RUNTIME_FACT_KEY_PROVIDER_PLANNED_ROUTE_TYPE,
    RUNTIME_FACT_KEY_PROVIDER_PREFLIGHT_STATUS, RUNTIME_FACT_KEY_TIME, RUNTIME_FACT_KEY_TIMEZONE,
    RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT, RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE,
    RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE, RUNTIME_FACT_KEY_TRACE_GAP, RUNTIME_FACT_KEY_WEEKDAY,
    RUNTIME_FACT_PROVIDER_GENERATION_PATH, RUNTIME_FACT_PROVIDER_ROUTE_GENERATION_PATH,
    RUNTIME_FACT_SOURCE_TYPE, RUNTIME_FACT_TOOL_AVAILABILITY_GENERATION_PATH,
};
pub(crate) use eval::{
    run_main_chat_runtime_facts_slice_a_backend_report,
    run_main_chat_runtime_facts_slice_b_provider_route_report,
    run_main_chat_runtime_facts_slice_c_tool_availability_report,
    run_main_chat_runtime_facts_slice_d_agent_self_state_report,
    MainChatRuntimeFactsCommandSurfaceProof, MainChatRuntimeFactsNegativeAssertionSummary,
    MainChatRuntimeFactsScenarioEvidence, MainChatRuntimeFactsSliceReport,
};
pub(crate) use provider_route::{
    build_settings_runtime_route_evidence, classify_provider_route_query,
    provider_route_fact_should_block_before_model, provider_route_query_has_followup_task,
    provider_transmission_history_from_runs, provider_transmission_history_item_from_run,
    resolve_provider_route_fact_answer, FallbackEvidence, ProviderReadiness,
    ProviderTransmissionHistoryItem, ProviderTransmissionSourceRef, RouteIdentity,
    RuntimeRouteEvidence,
};
pub(crate) use resolver::{
    resolve_post_model_runtime_fact_answer, resolve_pre_model_runtime_fact_answer,
    MainChatRuntimeFactPostModelRequest, MainChatRuntimeFactPreModelRequest,
};
pub(crate) use tool_availability::{
    classify_tool_availability_query, resolve_tool_availability_fact_answer,
};
