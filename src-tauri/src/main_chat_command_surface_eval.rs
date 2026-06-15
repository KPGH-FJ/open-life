use crate::AppState;
use openlife_core::agent::main_chat_agent_v1::{
    MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence,
    MainChatAgentExecutionV1AcceptanceLiveEvidence,
};
use openlife_core::llm::ChatMessage;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatCommandSurfaceEvalEntryPoint {
    Send,
    Stream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatCommandSurfaceEvalScenario {
    DirectProviderTrace,
    FileReadSuccess,
    PlanExecuteDraft,
    ProposalPath,
    WebPolicyBlocker,
    WebPolicyAgentLoopBlocker,
    WebAgentLoopSuccess,
    MissingMcpBlocker,
    RegisteredMcpReadSuccess,
    RegisteredMcpAgentLoopSuccess,
    RegisteredMcpPermissionProposal,
    RegisteredMcpAgentLoopPermissionProposal,
}

pub(crate) const MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES: [(
    MainChatCommandSurfaceEvalEntryPoint,
    MainChatCommandSurfaceEvalScenario,
); 24] = [
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::DirectProviderTrace,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::DirectProviderTrace,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::FileReadSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::FileReadSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::PlanExecuteDraft,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::PlanExecuteDraft,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::ProposalPath,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::ProposalPath,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::WebPolicyBlocker,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::WebPolicyBlocker,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::WebPolicyAgentLoopBlocker,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::WebPolicyAgentLoopBlocker,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::MissingMcpBlocker,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::MissingMcpBlocker,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal,
    ),
];

pub(crate) async fn run_main_chat_command_surface_eval_report() -> MainChatCommandSurfaceEvalReport
{
    let mut evidence = Vec::new();
    let mut failures = Vec::new();
    for (entry_point, scenario) in MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES {
        match run_main_chat_command_surface_state_eval_case(entry_point, scenario).await {
            Ok(case_evidence) => evidence.push(case_evidence),
            Err(error) => failures.push(format!("{entry_point:?}/{scenario:?}: {error}")),
        }
    }

    MainChatCommandSurfaceEvalReport::from_case_evidence(
        MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES.len(),
        evidence,
        failures,
    )
}

async fn run_main_chat_command_surface_state_eval_case(
    entry_point: MainChatCommandSurfaceEvalEntryPoint,
    scenario: MainChatCommandSurfaceEvalScenario,
) -> Result<MainChatCommandSurfaceEvalEvidence, String> {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    configure_main_chat_command_surface_eval_state(&state, scenario).await?;
    let session_id = main_chat_command_surface_eval_session_id(entry_point, scenario);
    let user_text = main_chat_command_surface_eval_user_text(scenario);
    let messages = vec![ChatMessage {
        role: "user".into(),
        content: user_text.into(),
    }];
    let (response_value, task_session_id, legacy_fallback_used) = match entry_point {
        MainChatCommandSurfaceEvalEntryPoint::Send => {
            let result = crate::main_chat_send::send_message_with_state(
                session_id.clone(),
                messages,
                None,
                &state,
            )
            .await?;
            let task_session_id = result
                .agent_ingress
                .as_ref()
                .and_then(|decision| decision.agent_task_session_id.as_deref())
                .ok_or_else(|| "send eval missing task session id".to_string())?
                .to_string();
            let legacy_fallback_used = result.legacy_fallback_used;
            let response_value = serde_json::to_value(&result)
                .map_err(|error| format!("serialize send eval response failed: {error}"))?;
            (response_value, task_session_id, legacy_fallback_used)
        }
        MainChatCommandSurfaceEvalEntryPoint::Stream => {
            let mut emitted_events = Vec::<(String, serde_json::Value)>::new();
            crate::main_chat_streaming::start_stream_message_with_state(
                session_id.clone(),
                messages,
                None,
                &state,
                |event, payload| {
                    emitted_events.push((event.to_string(), payload));
                },
            )
            .await?;
            let response_value = emitted_events
                .iter()
                .rev()
                .find(|(event, _)| event == "stream-message-done")
                .map(|(_, payload)| payload.clone())
                .ok_or_else(|| "stream eval missing stream-message-done event".to_string())?;
            let task_session_id = response_value
                .get("agent_ingress")
                .and_then(|value| value.get("agentTaskSessionId"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "stream eval missing task session id".to_string())?
                .to_string();
            let legacy_fallback_used = response_value
                .get("legacy_fallback_used")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            (response_value, task_session_id, legacy_fallback_used)
        }
    };
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "command-surface eval missing main chat session store".to_string())?;
    let (session, transcript) = {
        let store = store_arc.lock().await;
        let session = store
            .load_session(&task_session_id)
            .map_err(|error| format!("load command-surface eval task session failed: {error}"))?
            .ok_or_else(|| {
                "command-surface eval task session missing after execution".to_string()
            })?;
        let transcript = store
            .list_transcript_entries(&task_session_id)
            .map_err(|error| format!("list command-surface eval transcript failed: {error}"))?;
        (session, transcript)
    };
    let actions = if let Some(ref queue_arc) = state.main_chat_action_queue_store {
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(&task_session_id)
            .map_err(|error| format!("list command-surface eval actions failed: {error}"))?
    } else {
        Vec::new()
    };
    let proposals = if let Some(ref proposal_arc) = state.proposal_store {
        let proposal_store = proposal_arc.lock().await;
        proposal_store
            .list_pending_proposals(20)
            .map_err(|error| format!("list command-surface eval proposals failed: {error}"))?
    } else {
        Vec::new()
    };
    let runs = if let Some(ref run_store_arc) = state.agent_run_store {
        let run_store = run_store_arc.lock().await;
        run_store
            .list_runs_for_session(&session_id, 20)
            .map_err(|error| format!("list command-surface eval runs failed: {error}"))?
    } else {
        Vec::new()
    };

    assert_main_chat_command_surface_eval_case(
        scenario,
        &state,
        &task_session_id,
        &session,
        &transcript,
        &actions,
        &proposals,
        &runs,
        Some(&response_value),
    )
    .await?;

    Ok(MainChatCommandSurfaceEvalEvidence::for_case(
        entry_point,
        scenario,
        legacy_fallback_used,
        main_chat_command_surface_eval_has_silent_write(
            Some(&response_value),
            &transcript,
            &actions,
            &runs,
        ),
    ))
}

pub(crate) async fn configure_main_chat_command_surface_eval_state(
    state: &Arc<AppState>,
    scenario: MainChatCommandSurfaceEvalScenario,
) -> Result<(), String> {
    match scenario {
        MainChatCommandSurfaceEvalScenario::DirectProviderTrace => {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-direct",
                "command-surface eval direct provider reply",
            );
        }
        MainChatCommandSurfaceEvalScenario::FileReadSuccess => {
            let workspace_root = std::env::current_dir()
                .map_err(|error| format!("resolve eval cwd failed: {error}"))?
                .canonicalize()
                .map_err(|error| format!("canonicalize eval cwd failed: {error}"))?;
            let workspace_root_label = workspace_root.to_string_lossy().to_string();
            {
                let mut config = state.config.lock().await;
                if !config
                    .system
                    .safe_paths
                    .iter()
                    .any(|path| path == &workspace_root_label)
                {
                    config.system.safe_paths.push(workspace_root_label);
                }
            }
            let cargo_toml_path = workspace_root
                .join("Cargo.toml")
                .to_string_lossy()
                .to_string();
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-file-read",
                serde_json::json!({
                    "final": "I will read the workspace file first.",
                    "actions": [{
                        "name": "file.read",
                        "action_type": "mcp_tool",
                        "arguments": {
                            "path": cargo_toml_path
                        }
                    }],
                    "thought_summary": "Need a governed workspace file observation.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        MainChatCommandSurfaceEvalScenario::PlanExecuteDraft => {}
        MainChatCommandSurfaceEvalScenario::WebPolicyBlocker => {
            let mut config = state.config.lock().await;
            config.system.network_policy.enabled = false;
        }
        MainChatCommandSurfaceEvalScenario::WebPolicyAgentLoopBlocker => {
            {
                let mut config = state.config.lock().await;
                config.system.network_policy.enabled = false;
            }
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-web-loop",
                serde_json::json!({
                    "final": "I will run the governed web read first.",
                    "actions": [{
                        "name": "web.search",
                        "action_type": "mcp_tool",
                        "arguments": {
                            "query": "OpenLife release notes",
                            "max_results": 3
                        }
                    }],
                    "thought_summary": "Need a governed network-policy checked web observation.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess => {
            {
                let mut config = state.config.lock().await;
                config.system.network_policy.enabled = true;
            }
            {
                let mut fixture = state.web_search_fixture_output.lock().await;
                *fixture = Some(
                    "Search results for \"OpenLife release notes\":\n1. OpenLife fixture result\n   URL: https://example.com/openlife-release-notes\n   Snippet: Governed web AgentLoop command-surface success fixture."
                        .into(),
                );
            }
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-web-loop-success",
                serde_json::json!({
                    "final": "I will run the governed web read first.",
                    "actions": [{
                        "name": "web.search",
                        "action_type": "mcp_tool",
                        "arguments": {
                            "query": "OpenLife release notes",
                            "max_results": 3
                        }
                    }],
                    "thought_summary": "Need a governed successful web observation.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess => {
            grant_builtin_echo_read_once(state).await?;
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-mcp-fallback",
                serde_json::json!({
                    "final": "I can answer without a tool.",
                    "actions": [],
                    "thought_summary": "No governed observation yet.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess => {
            grant_builtin_echo_read_once(state).await?;
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-mcp-loop",
                serde_json::json!({
                    "final": "I will run the registered MCP read first.",
                    "actions": [{
                        "name": "builtin_echo",
                        "action_type": "mcp_tool",
                        "arguments": {}
                    }],
                    "thought_summary": "Need a governed read-only MCP observation.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal => {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-mcp-permission-fallback",
                serde_json::json!({
                    "final": "I can answer only after permission is reviewed.",
                    "actions": [],
                    "thought_summary": "The deterministic fallback should request tool permission.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal => {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-mcp-permission-loop",
                serde_json::json!({
                    "final": "I will run the registered MCP read after permission review.",
                    "actions": [{
                        "name": "memory.search",
                        "action_type": "mcp_tool",
                        "arguments": {}
                    }],
                    "thought_summary": "Need a governed read-only MCP observation that requires permission.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        MainChatCommandSurfaceEvalScenario::ProposalPath
        | MainChatCommandSurfaceEvalScenario::MissingMcpBlocker => {}
    }
    Ok(())
}

pub(crate) fn main_chat_command_surface_eval_user_text(
    scenario: MainChatCommandSurfaceEvalScenario,
) -> &'static str {
    match scenario {
        MainChatCommandSurfaceEvalScenario::DirectProviderTrace => {
            "Explain focused work in one concise paragraph for a teammate."
        }
        MainChatCommandSurfaceEvalScenario::FileReadSuccess => {
            "Read Cargo.toml as a governed workspace file observation."
        }
        MainChatCommandSurfaceEvalScenario::PlanExecuteDraft => {
            "Draft a weekly plan and break this goal into steps."
        }
        MainChatCommandSurfaceEvalScenario::ProposalPath => {
            "Please remember that I prefer morning writing blocks."
        }
        MainChatCommandSurfaceEvalScenario::WebPolicyBlocker => {
            "Please web search OpenLife release notes."
        }
        MainChatCommandSurfaceEvalScenario::WebPolicyAgentLoopBlocker => {
            "Please web search OpenLife release notes."
        }
        MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess => {
            "Please web search OpenLife release notes."
        }
        MainChatCommandSurfaceEvalScenario::MissingMcpBlocker => {
            "Use mcp missing.status read-only now."
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess
        | MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess => {
            "Use mcp builtin_echo read-only now."
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal
        | MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal => {
            "Use mcp memory.search now."
        }
    }
}

pub(crate) fn main_chat_command_surface_eval_session_id(
    entry_point: MainChatCommandSurfaceEvalEntryPoint,
    scenario: MainChatCommandSurfaceEvalScenario,
) -> String {
    format!(
        "command-surface-eval-{}-{}",
        match entry_point {
            MainChatCommandSurfaceEvalEntryPoint::Send => "send",
            MainChatCommandSurfaceEvalEntryPoint::Stream => "stream",
        },
        match scenario {
            MainChatCommandSurfaceEvalScenario::DirectProviderTrace => "direct-provider",
            MainChatCommandSurfaceEvalScenario::FileReadSuccess => "file-read-success",
            MainChatCommandSurfaceEvalScenario::PlanExecuteDraft => "plan-execute-draft",
            MainChatCommandSurfaceEvalScenario::ProposalPath => "proposal",
            MainChatCommandSurfaceEvalScenario::WebPolicyBlocker => "web-blocker",
            MainChatCommandSurfaceEvalScenario::WebPolicyAgentLoopBlocker => {
                "web-agent-loop-blocker"
            }
            MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess => "web-agent-loop-success",
            MainChatCommandSurfaceEvalScenario::MissingMcpBlocker => "missing-mcp",
            MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess => "mcp-success",
            MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess => "mcp-agent-loop",
            MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal => {
                "mcp-permission-proposal"
            }
            MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal => {
                "mcp-agent-loop-permission-proposal"
            }
        }
    )
}

fn scripted_eval_scheduler(
    model: impl Into<String>,
    response: impl Into<String>,
) -> openlife_core::scheduler::InferenceScheduler {
    openlife_core::scheduler::InferenceScheduler::new(
        "unused-local-model".into(),
        false,
        "openai".into(),
        "https://example.invalid/v1".into(),
        "test-key".into(),
        model.into(),
        "text-embedding-test".into(),
        false,
    )
    .with_scripted_generation_response(response.into())
}

pub(crate) async fn grant_builtin_echo_read_once(state: &Arc<AppState>) -> Result<(), String> {
    let store = state.tool_permission_store.lock().await;
    store
        .grant(
            "builtin_echo",
            "builtin",
            "low",
            "read",
            openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
            None,
        )
        .map_err(|error| format!("grant builtin_echo read permission failed: {error}"))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn assert_main_chat_command_surface_eval_case(
    scenario: MainChatCommandSurfaceEvalScenario,
    state: &Arc<AppState>,
    task_session_id: &str,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    proposals: &[openlife_core::agent::AgentProposal],
    runs: &[openlife_core::agent::AgentRun],
    response: Option<&serde_json::Value>,
) -> Result<(), String> {
    use openlife_core::agent::main_chat_agent_v1::{
        AgentTaskSessionStatus, ExecutionQueueStatus, ExecutionTranscriptEntryKind,
    };

    match scenario {
        MainChatCommandSurfaceEvalScenario::DirectProviderTrace => {
            if session.status != AgentTaskSessionStatus::Completed {
                return Err(format!(
                    "direct provider session status {:?}",
                    session.status
                ));
            }
            let generation_entry = transcript
                .iter()
                .find(|entry| {
                    entry
                        .summary
                        .contains("DirectAnswer generated a model response")
                })
                .ok_or_else(|| "missing DirectAnswer generation transcript".to_string())?;
            if generation_entry
                .metadata
                .get("providerGenerationPath")
                .and_then(serde_json::Value::as_str)
                != Some("main_chat_direct_answer_scheduler")
            {
                return Err("missing provider generation path metadata".into());
            }
            let run = runs
                .iter()
                .find(|run| {
                    run.reasoning_strategy.as_deref() == Some("main_chat_agent_v1_direct_answer")
                })
                .ok_or_else(|| "missing DirectAnswer AgentRun".to_string())?;
            let route = run
                .model_route
                .as_ref()
                .ok_or_else(|| "missing DirectAnswer model route".to_string())?;
            if route.provider != "openai" || route.route_type != "cloud" {
                return Err(format!(
                    "unexpected DirectAnswer provider route {}/{}",
                    route.provider, route.route_type
                ));
            }
            let scripted = generation_entry
                .metadata
                .get("scriptedProviderResponse")
                .and_then(serde_json::Value::as_bool);
            let live = generation_entry
                .metadata
                .get("liveProviderInvoked")
                .and_then(serde_json::Value::as_bool);
            if scripted != Some(true) || live != Some(false) {
                return Err(format!(
                    "scripted DirectAnswer provider metadata scripted={scripted:?} live={live:?}"
                ));
            }
            if generation_entry
                .metadata
                .get("providerEndpointKind")
                .and_then(serde_json::Value::as_str)
                != Some("scripted_scheduler_response")
                || generation_entry
                    .metadata
                    .get("externalLiveProviderEvalPreflighted")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err("scripted DirectAnswer metadata must not be treated as external live-provider eval proof".into());
            }
            if let Some(response) = response {
                if response
                    .get("reasoning_trace")
                    .and_then(|trace| trace.get("generation_result"))
                    .and_then(|generation| generation.get("providerGenerationPath"))
                    .and_then(serde_json::Value::as_str)
                    != Some("main_chat_direct_answer_scheduler")
                {
                    return Err("send response missing provider generation metadata".into());
                }
                let generation = response
                    .get("reasoning_trace")
                    .and_then(|trace| trace.get("generation_result"))
                    .ok_or_else(|| "send response missing generation result".to_string())?;
                if generation
                    .get("liveProviderInvoked")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                    || generation
                        .get("scriptedProviderResponse")
                        .and_then(serde_json::Value::as_bool)
                        != Some(true)
                    || generation
                        .get("providerEndpointKind")
                        .and_then(serde_json::Value::as_str)
                        != Some("scripted_scheduler_response")
                    || generation
                        .get("externalLiveProviderEvalPreflighted")
                        .and_then(serde_json::Value::as_bool)
                        != Some(false)
                {
                    return Err("send response missing scripted provider metadata".into());
                }
            }
        }
        MainChatCommandSurfaceEvalScenario::FileReadSuccess => {
            if session.status != AgentTaskSessionStatus::Completed {
                let action_debug: Vec<String> = actions
                    .iter()
                    .map(|action| {
                        format!(
                            "{}:{}:{:?}:{:?}:{:?}",
                            action.action.action_type,
                            action.action.description,
                            action.status,
                            action.error,
                            action.observation_metadata
                        )
                    })
                    .collect();
                return Err(format!(
                    "file read session status {:?}, blockers {:?}, actions {:?}",
                    session.status, session.pending_blockers, action_debug
                ));
            }
            if !session.pending_blockers.is_empty() {
                return Err(format!(
                    "file read success kept blockers {:?}",
                    session.pending_blockers
                ));
            }
            let completed_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
                .ok_or_else(|| "missing file read AgentLoop completion transcript".to_string())?;
            if completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
                || completed_entry
                    .metadata
                    .get("singleStepFallbackUsed")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || completed_entry
                    .metadata
                    .get("agentLoopActionStatus")
                    .and_then(serde_json::Value::as_str)
                    != Some("succeeded")
                || completed_entry
                    .metadata
                    .get("directWritesExecuted")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err("file read AgentLoop metadata incomplete".into());
            }
            let file_action = actions
                .iter()
                .find(|action| action.action.action_type == "file.read")
                .ok_or_else(|| "missing file.read action".to_string())?;
            if file_action.status != ExecutionQueueStatus::Completed {
                return Err(format!("file.read action status {:?}", file_action.status));
            }
            let metadata = file_action
                .observation_metadata
                .as_ref()
                .ok_or_else(|| "missing file.read observation metadata".to_string())?;
            if metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
                || metadata
                    .get("singleStepFallbackUsed")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || metadata
                    .get("agentLoopActionStatus")
                    .and_then(serde_json::Value::as_str)
                    != Some("succeeded")
                || metadata
                    .get("directWritesExecuted")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err("file.read action metadata incomplete".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::PlanExecuteDraft => {
            if session.status != AgentTaskSessionStatus::Completed {
                return Err(format!("PlanExecute session status {:?}", session.status));
            }
            if !session.pending_blockers.is_empty() {
                return Err(format!(
                    "PlanExecute draft kept blockers {:?}",
                    session.pending_blockers
                ));
            }
            let plan_action = actions
                .iter()
                .find(|action| action.action.action_type == "plan_execute.create_session")
                .ok_or_else(|| "missing plan_execute.create_session action".to_string())?;
            if plan_action.status != ExecutionQueueStatus::Completed {
                return Err(format!(
                    "plan_execute.create_session action status {:?}",
                    plan_action.status
                ));
            }
            let metadata = plan_action
                .observation_metadata
                .as_ref()
                .ok_or_else(|| "missing PlanExecute observation metadata".to_string())?;
            let plan_session_id = metadata
                .get("planExecuteSessionId")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "PlanExecute observation missing session id".to_string())?;
            let step_count = metadata
                .get("stepCount")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| "PlanExecute observation missing step count".to_string())?;
            if step_count == 0 {
                return Err("PlanExecute draft has no steps".into());
            }
            let store_arc = state
                .plan_execute_session_store
                .as_ref()
                .ok_or_else(|| "missing PlanExecute session store".to_string())?;
            let store = store_arc.lock().await;
            let plan_session = store
                .get_session(plan_session_id)
                .map_err(|error| format!("load PlanExecute session failed: {error}"))?
                .ok_or_else(|| "PlanExecute session was not persisted".to_string())?;
            if plan_session.status != openlife_core::agent::PlanExecuteSessionStatus::Draft
                || plan_session.source_chat_session_id.as_deref()
                    != Some(session.chat_session_id.as_str())
                || plan_session.steps.len() != step_count as usize
            {
                return Err("persisted PlanExecute draft metadata mismatch".into());
            }
            if !transcript.iter().any(|entry| {
                entry
                    .summary
                    .contains("Governed PlanExecute draft session was created")
                    && entry
                        .metadata
                        .get("directWritesExecuted")
                        .and_then(serde_json::Value::as_bool)
                        == Some(false)
            }) {
                return Err("missing PlanExecute transcript metadata".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::ProposalPath => {
            if session.status != AgentTaskSessionStatus::WaitingPermission {
                return Err(format!("proposal session status {:?}", session.status));
            }
            if !transcript
                .iter()
                .any(|entry| entry.kind == ExecutionTranscriptEntryKind::ProposalRequest)
            {
                return Err("proposal transcript missing".into());
            }
            if !actions.iter().any(|action| {
                action.action.action_type == "proposal.create"
                    && action.status == ExecutionQueueStatus::Completed
            }) {
                return Err("proposal.create queue action did not complete".into());
            }
            if !proposals.iter().any(|proposal| {
                proposal.source == openlife_core::agent::ProposalSource::ChatConversation
                    && proposal.source_detail.as_deref()
                        == Some(format!("main_chat_agent_task_session:{task_session_id}").as_str())
            }) {
                return Err("pending Review Center proposal not linked to task".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::WebPolicyBlocker => {
            if session.status != AgentTaskSessionStatus::Blocked {
                return Err(format!("web blocker session status {:?}", session.status));
            }
            if !session
                .pending_blockers
                .iter()
                .any(|blocker| blocker.contains("network_policy_blocked"))
            {
                return Err("network policy blocker not preserved on session".into());
            }
            let web_action = actions
                .iter()
                .find(|action| action.action.action_type == "web.search")
                .ok_or_else(|| "missing web.search action".to_string())?;
            if web_action.status != ExecutionQueueStatus::Failed {
                return Err(format!("web action status {:?}", web_action.status));
            }
            if web_action
                .observation_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("structuredResult"))
                .and_then(|value| value.get("network_policy_blocked"))
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                return Err("web blocker observation missing network_policy_blocked".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::WebPolicyAgentLoopBlocker => {
            if session.status != AgentTaskSessionStatus::Blocked {
                return Err(format!(
                    "web AgentLoop blocker session status {:?}",
                    session.status
                ));
            }
            if !session
                .pending_blockers
                .iter()
                .any(|blocker| blocker.contains("network_policy_blocked"))
            {
                return Err("web AgentLoop blocker not preserved on session".into());
            }
            let completed_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
                .ok_or_else(|| "missing web AgentLoop completion transcript".to_string())?;
            if completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
                || completed_entry
                    .metadata
                    .get("singleStepFallbackUsed")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || completed_entry
                    .metadata
                    .get("agentLoopActionStatus")
                    .and_then(serde_json::Value::as_str)
                    != Some("blocked")
                || completed_entry
                    .metadata
                    .get("permissionDecision")
                    .and_then(serde_json::Value::as_str)
                    != Some("network_policy_blocked")
            {
                return Err("web AgentLoop blocker metadata incomplete".into());
            }
            let web_action = actions
                .iter()
                .find(|action| action.action.action_type == "web.search")
                .ok_or_else(|| "missing web.search AgentLoop action".to_string())?;
            if web_action.status != ExecutionQueueStatus::Failed {
                return Err(format!(
                    "web AgentLoop action status {:?}",
                    web_action.status
                ));
            }
            let metadata = web_action
                .observation_metadata
                .as_ref()
                .ok_or_else(|| "missing web AgentLoop observation metadata".to_string())?;
            if metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
                || metadata
                    .get("singleStepFallbackUsed")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || metadata
                    .get("agentLoopActionStatus")
                    .and_then(serde_json::Value::as_str)
                    != Some("blocked")
                || metadata
                    .get("permissionDecision")
                    .and_then(serde_json::Value::as_str)
                    != Some("network_policy_blocked")
            {
                return Err("web AgentLoop action metadata incomplete".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess => {
            if session.status != AgentTaskSessionStatus::Completed {
                return Err(format!(
                    "web AgentLoop success session status {:?}",
                    session.status
                ));
            }
            if !session.pending_blockers.is_empty() {
                return Err(format!(
                    "web AgentLoop success kept blockers {:?}",
                    session.pending_blockers
                ));
            }
            let completed_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
                .ok_or_else(|| "missing web AgentLoop success transcript".to_string())?;
            if completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
                || completed_entry
                    .metadata
                    .get("singleStepFallbackUsed")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || completed_entry
                    .metadata
                    .get("agentLoopActionStatus")
                    .and_then(serde_json::Value::as_str)
                    != Some("succeeded")
                || completed_entry
                    .metadata
                    .get("directWritesExecuted")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err("web AgentLoop success metadata incomplete".into());
            }
            let web_action = actions
                .iter()
                .find(|action| action.action.action_type == "web.search")
                .ok_or_else(|| "missing web.search success action".to_string())?;
            if web_action.status != ExecutionQueueStatus::Completed {
                return Err(format!(
                    "web AgentLoop success action status {:?}",
                    web_action.status
                ));
            }
            let metadata = web_action
                .observation_metadata
                .as_ref()
                .ok_or_else(|| "missing web AgentLoop success metadata".to_string())?;
            if metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
                || metadata
                    .get("singleStepFallbackUsed")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || metadata
                    .get("agentLoopActionStatus")
                    .and_then(serde_json::Value::as_str)
                    != Some("succeeded")
                || metadata
                    .get("directWritesExecuted")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err("web AgentLoop success action metadata incomplete".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::MissingMcpBlocker => {
            if session.status != AgentTaskSessionStatus::Blocked {
                return Err(format!("missing MCP session status {:?}", session.status));
            }
            if !session
                .pending_blockers
                .iter()
                .any(|blocker| blocker.contains("mcp_read_tool_not_registered"))
            {
                return Err("missing MCP blocker not preserved on session".into());
            }
            let mcp_action = actions
                .iter()
                .find(|action| action.action.action_type == "mcp.read_only")
                .ok_or_else(|| "missing mcp.read_only action".to_string())?;
            if mcp_action.status != ExecutionQueueStatus::Failed {
                return Err(format!("missing MCP action status {:?}", mcp_action.status));
            }
            if mcp_action
                .observation_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("blockerReason"))
                .and_then(serde_json::Value::as_str)
                != Some("mcp_read_tool_not_registered")
            {
                return Err("missing MCP observation did not keep blocker reason".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess => {
            assert_mcp_read_success_action(actions, false)?;
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess => {
            assert_mcp_read_success_action(actions, true)?;
            let completed_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
                .ok_or_else(|| "missing AgentLoop completion transcript".to_string())?;
            if completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
                || completed_entry
                    .metadata
                    .get("singleStepFallbackUsed")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || completed_entry
                    .metadata
                    .get("mcpReadTargetResolved")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true)
                || completed_entry
                    .metadata
                    .get("resolvedTarget")
                    .and_then(serde_json::Value::as_str)
                    != Some("builtin_echo")
            {
                return Err("AgentLoop MCP completion metadata incomplete".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal => {
            assert_mcp_tool_permission_proposal_action(
                task_session_id,
                session,
                actions,
                proposals,
                false,
            )?;
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal => {
            assert_mcp_tool_permission_proposal_action(
                task_session_id,
                session,
                actions,
                proposals,
                true,
            )?;
            let completed_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
                .ok_or_else(|| "missing AgentLoop permission transcript".to_string())?;
            if completed_entry
                .metadata
                .get("agentLoopSucceeded")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
                || completed_entry
                    .metadata
                    .get("singleStepFallbackUsed")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || completed_entry
                    .metadata
                    .get("agentLoopActionStatus")
                    .and_then(serde_json::Value::as_str)
                    != Some("needs_confirmation")
            {
                return Err("AgentLoop permission metadata incomplete".into());
            }
        }
    }
    Ok(())
}

fn assert_mcp_read_success_action(
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    require_agent_loop: bool,
) -> Result<(), String> {
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .ok_or_else(|| "missing mcp.read_only action".to_string())?;
    if mcp_action.status
        != openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
    {
        return Err(format!("MCP action status {:?}", mcp_action.status));
    }
    let metadata = mcp_action
        .observation_metadata
        .as_ref()
        .ok_or_else(|| "missing MCP observation metadata".to_string())?;
    if metadata
        .get("mcpReadTargetResolved")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
        || metadata
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
    {
        return Err("MCP read success metadata incomplete".into());
    }
    if !require_agent_loop
        && metadata.get("target").and_then(serde_json::Value::as_str) != Some("builtin_echo")
    {
        return Err("MCP fallback observation target metadata incomplete".into());
    }
    if require_agent_loop
        && (metadata
            .get("agentLoopSucceeded")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
            || metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool)
                != Some(false)
            || metadata
                .get("resolvedTarget")
                .and_then(serde_json::Value::as_str)
                != Some("builtin_echo"))
    {
        return Err("MCP AgentLoop observation metadata incomplete".into());
    }
    Ok(())
}

fn assert_mcp_tool_permission_proposal_action(
    task_session_id: &str,
    session: &openlife_core::agent::main_chat_agent_v1::AgentTaskSession,
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    proposals: &[openlife_core::agent::AgentProposal],
    require_agent_loop: bool,
) -> Result<(), String> {
    if session.status
        != openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
    {
        return Err(format!(
            "MCP permission session status {:?}",
            session.status
        ));
    }
    if !session
        .pending_blockers
        .iter()
        .any(|blocker| blocker.contains("tool_permission_required"))
    {
        return Err(format!(
            "MCP permission blocker not preserved on session: {:?}",
            session.pending_blockers
        ));
    }
    let mcp_action = actions
        .iter()
        .find(|action| action.action.action_type == "mcp.read_only")
        .ok_or_else(|| "missing mcp.read_only permission action".to_string())?;
    if mcp_action.status
        != openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
    {
        return Err(format!(
            "MCP permission action status {:?}",
            mcp_action.status
        ));
    }
    let metadata = mcp_action
        .observation_metadata
        .as_ref()
        .ok_or_else(|| "missing MCP permission observation metadata".to_string())?;
    if metadata
        .get("directWritesExecuted")
        .and_then(serde_json::Value::as_bool)
        != Some(false)
    {
        return Err("MCP permission metadata missing no-write proof".into());
    }
    if require_agent_loop
        && (metadata
            .get("agentLoopSucceeded")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
            || metadata
                .get("singleStepFallbackUsed")
                .and_then(serde_json::Value::as_bool)
                != Some(false)
            || metadata
                .get("agentLoopActionStatus")
                .and_then(serde_json::Value::as_str)
                != Some("needs_confirmation"))
    {
        return Err("MCP AgentLoop permission action metadata incomplete".into());
    }
    if !require_agent_loop
        && metadata
            .get("executorStatus")
            .and_then(serde_json::Value::as_str)
            != Some("needs_confirmation")
    {
        return Err("MCP fallback permission action metadata incomplete".into());
    }
    let proposal_id = metadata
        .get("proposalId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "MCP permission action missing linked proposalId".to_string())?;
    let proposal = proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| "MCP permission proposal is not pending in Review Center".to_string())?;
    if proposal.proposal_type != openlife_core::agent::ProposalType::ToolPermission {
        return Err(format!(
            "MCP permission proposal type {:?}",
            proposal.proposal_type
        ));
    }
    if proposal.source != openlife_core::agent::ProposalSource::ChatConversation {
        return Err(format!(
            "MCP permission proposal source {:?}",
            proposal.source
        ));
    }
    if proposal.source_detail.as_deref()
        != Some(format!("main_chat_agent_task_session:{task_session_id}").as_str())
    {
        return Err("MCP permission proposal not linked to task session".into());
    }
    if proposal.affected_path != "tool_permission.builtin.memory.search" {
        return Err(format!(
            "MCP permission proposal affected path {}",
            proposal.affected_path
        ));
    }
    if proposal
        .after
        .get("permission_action")
        .and_then(serde_json::Value::as_str)
        != Some("grant")
        || proposal
            .after
            .get("tool_name")
            .and_then(serde_json::Value::as_str)
            != Some("memory.search")
        || proposal
            .after
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
    {
        return Err("MCP permission proposal payload incomplete".into());
    }
    Ok(())
}

pub(crate) fn main_chat_command_surface_eval_has_silent_write(
    response: Option<&serde_json::Value>,
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    actions: &[openlife_core::agent::main_chat_agent_v1::QueuedExecutionAction],
    runs: &[openlife_core::agent::AgentRun],
) -> bool {
    response.is_some_and(json_contains_direct_write_true)
        || transcript
            .iter()
            .any(|entry| json_contains_direct_write_true(&entry.metadata))
        || actions.iter().any(|action| {
            action
                .observation_metadata
                .as_ref()
                .is_some_and(json_contains_direct_write_true)
        })
        || runs.iter().any(|run| {
            run.reasoning_trace
                .as_ref()
                .and_then(|trace| trace.generation_result.as_ref())
                .is_some_and(json_contains_direct_write_true)
        })
}

pub(crate) fn json_contains_direct_write_true(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(key, value)| {
            (key == "directWritesExecuted" && value.as_bool() == Some(true))
                || json_contains_direct_write_true(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(json_contains_direct_write_true),
        _ => false,
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatCommandSurfaceEvalReport {
    pub(crate) total_cases: usize,
    pub(crate) failed_cases: usize,
    pub(crate) send_coverage: f32,
    pub(crate) stream_coverage: f32,
    pub(crate) provider_generation_coverage: f32,
    pub(crate) file_read_coverage: f32,
    pub(crate) plan_execute_coverage: f32,
    pub(crate) proposal_coverage: f32,
    pub(crate) web_policy_blocker_coverage: f32,
    pub(crate) web_agent_loop_blocker_coverage: f32,
    pub(crate) web_agent_loop_success_coverage: f32,
    pub(crate) mcp_missing_read_target_blocker_coverage: f32,
    pub(crate) mcp_registered_read_success_coverage: f32,
    pub(crate) mcp_agent_loop_success_coverage: f32,
    pub(crate) mcp_tool_permission_proposal_coverage: f32,
    pub(crate) mcp_agent_loop_tool_permission_proposal_coverage: f32,
    pub(crate) live_provider_generation_coverage: f32,
    pub(crate) live_provider_web_mcp_agent_loop_coverage: f32,
    pub(crate) live_provider_web_agent_loop_coverage: f32,
    pub(crate) live_provider_mcp_agent_loop_coverage: f32,
    pub(crate) live_provider_proposal_permission_coverage: f32,
    pub(crate) final_completion_ready: bool,
    pub(crate) final_completion_blockers: Vec<String>,
    pub(crate) legacy_fallback_count: usize,
    pub(crate) silent_write_count: usize,
    pub(crate) failures: Vec<String>,
}

impl MainChatCommandSurfaceEvalReport {
    pub(crate) fn from_case_evidence(
        total_cases: usize,
        evidence: Vec<MainChatCommandSurfaceEvalEvidence>,
        failures: Vec<String>,
    ) -> Self {
        let ratio = |count: usize| -> f32 {
            if total_cases == 0 {
                0.0
            } else {
                count as f32 / total_cases as f32
            }
        };

        Self {
            total_cases,
            failed_cases: failures.len(),
            send_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.entry_point == MainChatCommandSurfaceEvalEntryPoint::Send)
                    .count(),
            ),
            stream_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.entry_point == MainChatCommandSurfaceEvalEntryPoint::Stream)
                    .count(),
            ),
            provider_generation_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.provider_generation)
                    .count(),
            ),
            file_read_coverage: ratio(evidence.iter().filter(|case| case.file_read).count()),
            plan_execute_coverage: ratio(evidence.iter().filter(|case| case.plan_execute).count()),
            proposal_coverage: ratio(evidence.iter().filter(|case| case.proposal).count()),
            web_policy_blocker_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.web_policy_blocker)
                    .count(),
            ),
            web_agent_loop_blocker_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.web_agent_loop_blocker)
                    .count(),
            ),
            web_agent_loop_success_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.web_agent_loop_success)
                    .count(),
            ),
            mcp_missing_read_target_blocker_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.mcp_missing_read_target_blocker)
                    .count(),
            ),
            mcp_registered_read_success_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.mcp_registered_read_success)
                    .count(),
            ),
            mcp_agent_loop_success_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.mcp_agent_loop_success)
                    .count(),
            ),
            mcp_tool_permission_proposal_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.mcp_tool_permission_proposal)
                    .count(),
            ),
            mcp_agent_loop_tool_permission_proposal_coverage: ratio(
                evidence
                    .iter()
                    .filter(|case| case.mcp_agent_loop_tool_permission_proposal)
                    .count(),
            ),
            live_provider_generation_coverage: 0.0,
            live_provider_web_mcp_agent_loop_coverage: 0.0,
            live_provider_web_agent_loop_coverage: 0.0,
            live_provider_mcp_agent_loop_coverage: 0.0,
            live_provider_proposal_permission_coverage: 0.0,
            final_completion_ready: false,
            final_completion_blockers: vec![
                "live_provider_generation_not_executed".into(),
                "provider_backed_web_mcp_agent_loop_not_executed".into(),
                "provider_backed_web_agent_loop_not_executed".into(),
                "provider_backed_mcp_agent_loop_not_executed".into(),
                "provider_live_proposal_permission_not_executed".into(),
            ],
            legacy_fallback_count: evidence
                .iter()
                .filter(|case| case.legacy_fallback_used)
                .count(),
            silent_write_count: evidence
                .iter()
                .filter(|case| case.silent_write_detected)
                .count(),
            failures,
        }
    }

    pub(crate) fn acceptance_evidence(
        &self,
    ) -> MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence {
        let required_scenario_coverage_present = self.provider_generation_coverage > 0.0
            && self.file_read_coverage > 0.0
            && self.plan_execute_coverage > 0.0
            && self.proposal_coverage > 0.0
            && self.web_policy_blocker_coverage > 0.0
            && self.web_agent_loop_blocker_coverage > 0.0
            && self.web_agent_loop_success_coverage > 0.0
            && self.mcp_missing_read_target_blocker_coverage > 0.0
            && self.mcp_registered_read_success_coverage > 0.0
            && self.mcp_agent_loop_success_coverage > 0.0
            && self.mcp_tool_permission_proposal_coverage > 0.0
            && self.mcp_agent_loop_tool_permission_proposal_coverage > 0.0;
        let send_stream_matrix_coverage = if self.failed_cases == 0
            && self.total_cases >= MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES.len()
            && self.send_coverage >= 0.45
            && self.stream_coverage >= 0.45
            && required_scenario_coverage_present
        {
            1.0
        } else {
            0.0
        };

        MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence {
            total_cases: self.total_cases,
            legacy_fallback_count: usize_to_u32_saturating(self.legacy_fallback_count),
            silent_write_count: usize_to_u32_saturating(self.silent_write_count),
            send_stream_matrix_coverage,
            final_completion_ready: self.final_completion_ready,
        }
    }

    pub(crate) fn acceptance_evidence_with_live_provider(
        &self,
        live_provider: &MainChatAgentExecutionV1AcceptanceLiveEvidence,
    ) -> MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence {
        crate::main_chat_final_gate::command_surface_evidence_with_live_provider(
            self.acceptance_evidence(),
            live_provider,
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MainChatCommandSurfaceEvalEvidence {
    pub(crate) entry_point: MainChatCommandSurfaceEvalEntryPoint,
    pub(crate) provider_generation: bool,
    pub(crate) file_read: bool,
    pub(crate) plan_execute: bool,
    pub(crate) proposal: bool,
    pub(crate) web_policy_blocker: bool,
    pub(crate) web_agent_loop_blocker: bool,
    pub(crate) web_agent_loop_success: bool,
    pub(crate) mcp_missing_read_target_blocker: bool,
    pub(crate) mcp_registered_read_success: bool,
    pub(crate) mcp_agent_loop_success: bool,
    pub(crate) mcp_tool_permission_proposal: bool,
    pub(crate) mcp_agent_loop_tool_permission_proposal: bool,
    pub(crate) legacy_fallback_used: bool,
    pub(crate) silent_write_detected: bool,
}

impl MainChatCommandSurfaceEvalEvidence {
    pub(crate) fn for_case(
        entry_point: MainChatCommandSurfaceEvalEntryPoint,
        scenario: MainChatCommandSurfaceEvalScenario,
        legacy_fallback_used: bool,
        silent_write_detected: bool,
    ) -> Self {
        Self {
            entry_point,
            provider_generation: scenario
                == MainChatCommandSurfaceEvalScenario::DirectProviderTrace,
            file_read: scenario == MainChatCommandSurfaceEvalScenario::FileReadSuccess,
            plan_execute: scenario == MainChatCommandSurfaceEvalScenario::PlanExecuteDraft,
            proposal: scenario == MainChatCommandSurfaceEvalScenario::ProposalPath,
            web_policy_blocker: scenario == MainChatCommandSurfaceEvalScenario::WebPolicyBlocker,
            web_agent_loop_blocker: scenario
                == MainChatCommandSurfaceEvalScenario::WebPolicyAgentLoopBlocker,
            web_agent_loop_success: scenario
                == MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess,
            mcp_missing_read_target_blocker: scenario
                == MainChatCommandSurfaceEvalScenario::MissingMcpBlocker,
            mcp_registered_read_success: matches!(
                scenario,
                MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess
                    | MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess
            ),
            mcp_agent_loop_success: scenario
                == MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess,
            mcp_tool_permission_proposal: matches!(
                scenario,
                MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal
                    | MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal
            ),
            mcp_agent_loop_tool_permission_proposal: scenario
                == MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopPermissionProposal,
            legacy_fallback_used,
            silent_write_detected,
        }
    }
}

fn usize_to_u32_saturating(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}
