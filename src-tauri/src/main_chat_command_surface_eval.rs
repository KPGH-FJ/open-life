use crate::AppState;
use openlife_core::agent::main_chat_agent_v1::{
    MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence,
    MainChatAgentExecutionV1AcceptanceLiveEvidence,
};
use openlife_core::llm::ChatMessage;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatCommandSurfaceEvalEntryPoint {
    Send,
    Stream,
}

impl MainChatCommandSurfaceEvalEntryPoint {
    pub(crate) fn as_label(self) -> &'static str {
        match self {
            Self::Send => "send",
            Self::Stream => "stream",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatCommandSurfaceEvalScenario {
    DirectProviderTrace,
    MemoryContextDirectAnswerSuccess,
    MemoryConflictCompareSuccess,
    FileReadSuccess,
    SessionSearchSuccess,
    PlanExecuteDraft,
    SelectedSkillContextSuccess,
    KnowledgeAssetContextSuccess,
    KnowledgeAssetEditProposal,
    ProposalPath,
    WebPolicyBlocker,
    WebPolicyAgentLoopBlocker,
    WebAgentLoopSuccess,
    MissingMcpBlocker,
    RegisteredMcpReadSuccess,
    RegisteredMcpAgentLoopSuccess,
    MultiReadAgentLoopSuccess,
    RegisteredMcpPermissionProposal,
    RegisteredMcpAgentLoopPermissionProposal,
}

impl MainChatCommandSurfaceEvalScenario {
    pub(crate) fn as_label(self) -> &'static str {
        match self {
            Self::DirectProviderTrace => "direct_provider_trace",
            Self::MemoryContextDirectAnswerSuccess => "memory_context_direct_answer_success",
            Self::MemoryConflictCompareSuccess => "memory_conflict_compare_success",
            Self::FileReadSuccess => "file_read_success",
            Self::SessionSearchSuccess => "session_search_success",
            Self::PlanExecuteDraft => "plan_execute_draft",
            Self::SelectedSkillContextSuccess => "selected_skill_context_success",
            Self::KnowledgeAssetContextSuccess => "knowledge_asset_context_success",
            Self::KnowledgeAssetEditProposal => "knowledge_asset_edit_proposal",
            Self::ProposalPath => "proposal_path",
            Self::WebPolicyBlocker => "web_policy_blocker",
            Self::WebPolicyAgentLoopBlocker => "web_policy_agent_loop_blocker",
            Self::WebAgentLoopSuccess => "web_agent_loop_success",
            Self::MissingMcpBlocker => "missing_mcp_blocker",
            Self::RegisteredMcpReadSuccess => "registered_mcp_read_success",
            Self::RegisteredMcpAgentLoopSuccess => "registered_mcp_agent_loop_success",
            Self::MultiReadAgentLoopSuccess => "multi_read_agent_loop_success",
            Self::RegisteredMcpPermissionProposal => "registered_mcp_permission_proposal",
            Self::RegisteredMcpAgentLoopPermissionProposal => {
                "registered_mcp_agent_loop_permission_proposal"
            }
        }
    }
}

pub(crate) const MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES: [(
    MainChatCommandSurfaceEvalEntryPoint,
    MainChatCommandSurfaceEvalScenario,
); 38] = [
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
        MainChatCommandSurfaceEvalScenario::MemoryContextDirectAnswerSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::MemoryContextDirectAnswerSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::MemoryConflictCompareSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::MemoryConflictCompareSuccess,
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
        MainChatCommandSurfaceEvalScenario::SessionSearchSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::SessionSearchSuccess,
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
        MainChatCommandSurfaceEvalScenario::SelectedSkillContextSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::SelectedSkillContextSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::KnowledgeAssetContextSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::KnowledgeAssetContextSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Send,
        MainChatCommandSurfaceEvalScenario::KnowledgeAssetEditProposal,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::KnowledgeAssetEditProposal,
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
        MainChatCommandSurfaceEvalScenario::MultiReadAgentLoopSuccess,
    ),
    (
        MainChatCommandSurfaceEvalEntryPoint::Stream,
        MainChatCommandSurfaceEvalScenario::MultiReadAgentLoopSuccess,
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
    let selected_skill_id = main_chat_command_surface_eval_selected_skill_id(scenario);
    let (response_value, task_session_id, legacy_fallback_used) = match entry_point {
        MainChatCommandSurfaceEvalEntryPoint::Send => {
            let result = crate::main_chat_send::send_message_with_state(
                session_id.clone(),
                messages,
                selected_skill_id.map(str::to_string),
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
                selected_skill_id.map(str::to_string),
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
    let memory_conflict_evidence =
        main_chat_command_surface_eval_memory_conflict_evidence(&state).await?;
    let knowledge_asset_edit_evidence =
        main_chat_command_surface_eval_knowledge_asset_edit_evidence(&proposals);

    Ok(MainChatCommandSurfaceEvalEvidence::for_case(
        entry_point,
        scenario,
        task_session_id,
        runs.iter().map(|run| run.id.clone()).collect(),
        transcript.len(),
        actions.len(),
        proposals.len(),
        runs.len(),
        legacy_fallback_used,
        main_chat_command_surface_eval_has_silent_write(
            Some(&response_value),
            &transcript,
            &actions,
            &runs,
        ),
        selected_skill_id.map(str::to_string),
        main_chat_command_surface_eval_selected_skill_loaded(
            &transcript,
            "skills/phase_e_review/SKILL.md",
        ),
        main_chat_command_surface_eval_selected_skill_loaded(
            &transcript,
            "skills/unselected_context/SKILL.md",
        ),
        main_chat_command_surface_eval_memory_context_source_count(&transcript),
        main_chat_command_surface_eval_knowledge_asset_source_count(&transcript),
        main_chat_command_surface_eval_knowledge_asset_scope_digest_loaded(&transcript),
        main_chat_command_surface_eval_agent_loop_count(&transcript, "toolCallCount"),
        main_chat_command_surface_eval_agent_loop_count(&transcript, "agentLoopObservationCount"),
        memory_conflict_evidence.graph_conflict_count,
        memory_conflict_evidence.lifecycle_record_count,
        memory_conflict_evidence.distinct_conflict_id_count,
        knowledge_asset_edit_evidence.proposal_created,
        knowledge_asset_edit_evidence.proposed_diff_present,
        knowledge_asset_edit_evidence.direct_write_detected,
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
        MainChatCommandSurfaceEvalScenario::MemoryContextDirectAnswerSuccess => {
            seed_command_surface_memory_context(state).await?;
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-memory-context",
                "command-surface eval direct reply grounded in accepted memory context",
            );
        }
        MainChatCommandSurfaceEvalScenario::MemoryConflictCompareSuccess => {
            seed_command_surface_memory_conflict_context(state).await?;
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-memory-conflict",
                "command-surface eval direct reply comparing visible conflicting memory facts",
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
        MainChatCommandSurfaceEvalScenario::SessionSearchSuccess => {
            {
                let memory_store = state.memory_store.lock().await;
                memory_store
                    .save_message(
                        "prior-agent-memory-session",
                        &ChatMessage {
                            role: "user".into(),
                            content: "We discussed Agent memory needing source citations, bounded session search, and no silent promotion to durable truth.".into(),
                        },
                    )
                    .map_err(|error| {
                        format!("seed command-surface session search memory failed: {error}")
                    })?;
            }
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-session-search",
                serde_json::json!({
                    "final": "I will search prior session memory first.",
                    "actions": [{
                        "name": "session.search",
                        "action_type": "session_search",
                        "arguments": {
                            "query": "Agent memory",
                            "limit": 5
                        }
                    }],
                    "thought_summary": "Need a governed prior-session search observation.",
                    "warnings": []
                })
                .to_string(),
            );
        }
        MainChatCommandSurfaceEvalScenario::PlanExecuteDraft => {}
        MainChatCommandSurfaceEvalScenario::SelectedSkillContextSuccess => {}
        MainChatCommandSurfaceEvalScenario::KnowledgeAssetContextSuccess => {
            let root = create_command_surface_knowledge_asset_root()?;
            {
                let mut config = state.config.lock().await;
                config.system.knowledge_roots.push(root);
            }
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-knowledge-assets",
                "command-surface eval direct reply grounded in bounded knowledge assets",
            );
        }
        MainChatCommandSurfaceEvalScenario::KnowledgeAssetEditProposal => {
            let root = create_command_surface_knowledge_asset_root()?;
            let mut config = state.config.lock().await;
            config.system.knowledge_roots.push(root);
        }
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
        MainChatCommandSurfaceEvalScenario::MultiReadAgentLoopSuccess => {
            {
                let mut config = state.config.lock().await;
                config.system.agent_loop_max_steps = 1;
                config.system.agent_loop_max_tool_calls = 3;
            }
            {
                let memory_store = state.memory_store.lock().await;
                memory_store
                    .save_message(
                        "multi-read-fixture-source-a",
                        &ChatMessage {
                            role: "user".into(),
                            content: "Ask a task that needs multiple reads. Multi-read fixture alpha covers workspace notes and source evidence.".into(),
                        },
                    )
                    .map_err(|error| {
                        format!("seed command-surface multi-read memory A failed: {error}")
                    })?;
                memory_store
                    .save_message(
                        "multi-read-fixture-source-b",
                        &ChatMessage {
                            role: "assistant".into(),
                            content: "Ask a task that needs multiple reads. Multi-read fixture beta covers delivery evidence and follow-up synthesis.".into(),
                        },
                    )
                    .map_err(|error| {
                        format!("seed command-surface multi-read memory B failed: {error}")
                    })?;
            }
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = scripted_eval_scheduler(
                "gpt-command-surface-eval-multi-read-loop",
                serde_json::json!({
                    "final": "I will run two governed reads before answering.",
                    "actions": [
                        {
                            "name": "memory.search",
                            "action_type": "memory_search",
                            "arguments": {
                                "query": "multi-read fixture alpha",
                                "limit": 5
                            }
                        },
                        {
                            "name": "memory.search",
                            "action_type": "memory_search",
                            "arguments": {
                                "query": "multi-read fixture beta",
                                "limit": 5
                            }
                        }
                    ],
                    "thought_summary": "Need two bounded memory observations.",
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
        MainChatCommandSurfaceEvalScenario::MemoryContextDirectAnswerSuccess => {
            "Use my current memory/preferences when answering."
        }
        MainChatCommandSurfaceEvalScenario::MemoryConflictCompareSuccess => {
            "Compare two memory facts that conflict."
        }
        MainChatCommandSurfaceEvalScenario::FileReadSuccess => {
            "Read Cargo.toml as a governed workspace file observation."
        }
        MainChatCommandSurfaceEvalScenario::SessionSearchSuccess => {
            "Find what we discussed about Agent memory."
        }
        MainChatCommandSurfaceEvalScenario::PlanExecuteDraft => {
            "Draft a weekly plan and break this goal into steps."
        }
        MainChatCommandSurfaceEvalScenario::SelectedSkillContextSuccess => {
            "Use the selected skill to review this plan."
        }
        MainChatCommandSurfaceEvalScenario::KnowledgeAssetContextSuccess => {
            "Inspect loaded knowledge assets."
        }
        MainChatCommandSurfaceEvalScenario::KnowledgeAssetEditProposal => {
            "Propose an edit to AGENTS.md knowledge asset: add a bounded Beta evidence note."
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
        MainChatCommandSurfaceEvalScenario::MultiReadAgentLoopSuccess => {
            "Ask a task that needs multiple reads."
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

pub(crate) fn main_chat_command_surface_eval_selected_skill_id(
    scenario: MainChatCommandSurfaceEvalScenario,
) -> Option<&'static str> {
    match scenario {
        MainChatCommandSurfaceEvalScenario::SelectedSkillContextSuccess => Some("phase_e_review"),
        _ => None,
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
            MainChatCommandSurfaceEvalScenario::MemoryContextDirectAnswerSuccess => {
                "memory-context-direct"
            }
            MainChatCommandSurfaceEvalScenario::MemoryConflictCompareSuccess => {
                "memory-conflict-compare"
            }
            MainChatCommandSurfaceEvalScenario::FileReadSuccess => "file-read-success",
            MainChatCommandSurfaceEvalScenario::SessionSearchSuccess => "session-search-success",
            MainChatCommandSurfaceEvalScenario::PlanExecuteDraft => "plan-execute-draft",
            MainChatCommandSurfaceEvalScenario::SelectedSkillContextSuccess => {
                "selected-skill-context"
            }
            MainChatCommandSurfaceEvalScenario::KnowledgeAssetContextSuccess => {
                "knowledge-assets-context"
            }
            MainChatCommandSurfaceEvalScenario::KnowledgeAssetEditProposal => {
                "knowledge-assets-edit-proposal"
            }
            MainChatCommandSurfaceEvalScenario::ProposalPath => "proposal",
            MainChatCommandSurfaceEvalScenario::WebPolicyBlocker => "web-blocker",
            MainChatCommandSurfaceEvalScenario::WebPolicyAgentLoopBlocker => {
                "web-agent-loop-blocker"
            }
            MainChatCommandSurfaceEvalScenario::WebAgentLoopSuccess => "web-agent-loop-success",
            MainChatCommandSurfaceEvalScenario::MissingMcpBlocker => "missing-mcp",
            MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess => "mcp-success",
            MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess => "mcp-agent-loop",
            MainChatCommandSurfaceEvalScenario::MultiReadAgentLoopSuccess => {
                "multi-read-agent-loop"
            }
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

async fn seed_command_surface_memory_context(state: &Arc<AppState>) -> Result<(), String> {
    let mut proposal = openlife_core::agent::AgentProposal::new(
        openlife_core::agent::ProposalType::MemoryWrite,
        "memory.preferences.beta_b4",
        serde_json::json!({
            "content": "User prefers concise execution-first answers with source-backed caveats.",
            "scope": "global",
            "category": "preference"
        }),
        "Command-surface eval seeds one accepted memory preference for bounded context.",
        0.91,
        openlife_core::agent::RiskLevel::Low,
        openlife_core::agent::ProposalSource::ChatConversation,
    );
    proposal.id = "proposal-command-surface-memory-context".into();
    proposal.source_detail = Some("task-session-command-surface-memory-context".into());
    let input = openlife_core::agent::MemoryLifecycleAcceptanceInput::from_memory_proposal(
        &proposal,
        "User prefers concise execution-first answers with source-backed caveats.".into(),
    );
    let store_arc = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(|| "command-surface eval missing memory lifecycle store".to_string())?;
    let store = store_arc.lock().await;
    store
        .accept_memory_proposal(input)
        .map(|_| ())
        .map_err(|error| format!("seed command-surface memory context failed: {error}"))
}

async fn seed_command_surface_memory_conflict_context(state: &Arc<AppState>) -> Result<(), String> {
    let conflict_ids = {
        let evidence_store = state.evidence_store.lock().await;
        let support = evidence_store
            .create_evidence(command_surface_memory_conflict_evidence_draft(
                "run-command-surface-memory-conflict-support",
                openlife_core::agent::EvidenceType::Preference,
                0.82,
                Vec::new(),
            ))
            .map_err(|error| format!("seed memory conflict support evidence failed: {error}"))?;
        let contradiction = evidence_store
            .create_evidence(command_surface_memory_conflict_evidence_draft(
                "run-command-surface-memory-conflict-opposition",
                openlife_core::agent::EvidenceType::Contradiction,
                0.79,
                vec![support.id.clone()],
            ))
            .map_err(|error| {
                format!("seed memory conflict contradiction evidence failed: {error}")
            })?;
        let records = evidence_store
            .query(openlife_core::agent::EvidenceQuery::default())
            .map_err(|error| format!("query memory conflict evidence failed: {error}"))?;
        let graph = openlife_core::agent::evaluate_evidence_graph(
            openlife_core::agent::EvidenceGraphInput::new(records, chrono::Utc::now()),
        );
        if !graph.graph_ready
            || !graph.metadata_safe
            || graph.contains_raw_content
            || graph.conflict_count < 2
            || graph.opposition_link_count == 0
        {
            return Err(format!(
                "seeded memory conflict graph not ready: conflict_count={} opposition_link_count={}",
                graph.conflict_count, graph.opposition_link_count
            ));
        }
        vec![support.id, contradiction.id]
    };

    let store_arc = state
        .memory_lifecycle_store
        .as_ref()
        .ok_or_else(|| "command-surface eval missing memory lifecycle store".to_string())?;
    let store = store_arc.lock().await;
    for (suffix, content) in [
        ("a", "User prefers morning deep work."),
        ("b", "User prefers late-night deep work."),
    ] {
        let proposal = command_surface_memory_conflict_proposal(suffix, content, &conflict_ids);
        store
            .accept_memory_proposal(
                openlife_core::agent::MemoryLifecycleAcceptanceInput::from_memory_proposal(
                    &proposal,
                    content.into(),
                ),
            )
            .map_err(|error| format!("seed memory conflict lifecycle record failed: {error}"))?;
    }
    Ok(())
}

fn command_surface_memory_conflict_proposal(
    suffix: &str,
    content: &str,
    conflict_ids: &[String],
) -> openlife_core::agent::AgentProposal {
    let mut proposal = openlife_core::agent::AgentProposal::new(
        openlife_core::agent::ProposalType::MemoryWrite,
        "memory.preferences.deep_work_window",
        serde_json::json!({
            "content": content,
            "scope": "global",
            "category": "preference",
            "conflictIds": conflict_ids
        }),
        "Command-surface eval seeds conflicting accepted memory candidates for comparison.",
        0.74,
        openlife_core::agent::RiskLevel::Low,
        openlife_core::agent::ProposalSource::ChatConversation,
    );
    proposal.id = format!("proposal-command-surface-memory-conflict-{suffix}");
    proposal.source_detail = Some(format!(
        "task-session-command-surface-memory-conflict-{suffix}"
    ));
    proposal.run_id = Some(format!("run-command-surface-memory-conflict-{suffix}"));
    proposal
}

fn command_surface_memory_conflict_evidence_draft(
    run_id: &str,
    evidence_type: openlife_core::agent::EvidenceType,
    confidence: f32,
    opposing_refs: Vec<String>,
) -> openlife_core::agent::EvidenceDraft {
    let mut draft = openlife_core::agent::EvidenceDraft::new(
        evidence_type,
        "/preferences/deep_work/window",
        confidence,
        openlife_core::agent::RiskLevel::Low,
        openlife_core::agent::EvidencePrivacyLevel::Internal,
    )
    .with_summary("metadata safe command-surface memory conflict")
    .with_source_ref(openlife_core::agent::EvidenceSourceRef::from_digest(
        openlife_core::agent::EvidenceSourceType::AgentRun,
        run_id,
        Some("main_chat_command_surface_memory_conflict"),
        format!("{run_id}-digest"),
    ))
    .with_linked_agent_run(run_id);
    draft.opposing_refs = opposing_refs;
    draft.run_metadata = serde_json::json!({
        "schema": "mainChatCommandSurfaceMemoryConflict.v1",
        "metadataSafe": true,
        "containsRawContent": false
    });
    draft
}

fn create_command_surface_knowledge_asset_root() -> Result<String, String> {
    let root = std::env::temp_dir().join(format!(
        "openlife-command-surface-knowledge-assets-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("create command-surface knowledge root failed: {error}"))?;
    for (relative, content) in [
        (
            "AGENTS.md",
            "B27 scoped AGENTS instructions: use bounded context only; never override runtime policy.",
        ),
        (
            "SOUL.md",
            "B27 scoped SOUL surface: materialized identity context, not canonical truth.",
        ),
        (
            "USER.md",
            "B27 scoped USER surface: local user context for inspection evidence.",
        ),
        (
            "MEMORY.md",
            "B27 scoped MEMORY surface: bounded memory context, not trusted raw top-k memory.",
        ),
    ] {
        std::fs::write(root.join(relative), content).map_err(|error| {
            format!("write command-surface knowledge asset {relative} failed: {error}")
        })?;
    }

    Ok(root.to_string_lossy().to_string())
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
        MainChatCommandSurfaceEvalScenario::MemoryContextDirectAnswerSuccess => {
            if session.status != AgentTaskSessionStatus::Completed {
                return Err(format!(
                    "memory context session status {:?}",
                    session.status
                ));
            }
            if !session.pending_blockers.is_empty() {
                return Err(format!(
                    "memory context DirectAnswer kept blockers {:?}",
                    session.pending_blockers
                ));
            }
            if !actions.is_empty() {
                return Err("memory context DirectAnswer should not create tool actions".into());
            }
            let context_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Bounded context was selected"))
                .ok_or_else(|| "missing memory context transcript".to_string())?;
            if context_entry
                .metadata
                .get("rawTopKMemoryTrusted")
                .and_then(serde_json::Value::as_bool)
                != Some(false)
            {
                return Err("memory context must not trust raw top-k memory snippets".into());
            }
            let memory_source_count = context_entry_source_prefix_count(
                &context_entry.metadata,
                "selected_personal_context",
                "memory:",
            );
            if memory_source_count == 0
                || !context_entry_sources_contain_prefix(
                    &context_entry.metadata,
                    "selected_personal_context",
                    "memory:",
                )
            {
                return Err(
                    "accepted memory lifecycle source was not loaded as bounded context".into(),
                );
            }
            let generation_entry = transcript
                .iter()
                .find(|entry| {
                    entry
                        .summary
                        .contains("DirectAnswer generated a model response")
                })
                .ok_or_else(|| {
                    "missing memory-context DirectAnswer generation transcript".to_string()
                })?;
            if generation_entry
                .metadata
                .get("providerGenerationPath")
                .and_then(serde_json::Value::as_str)
                != Some("main_chat_direct_answer_scheduler")
                || generation_entry
                    .metadata
                    .get("directWritesExecuted")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err("memory-context DirectAnswer generation metadata incomplete".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::MemoryConflictCompareSuccess => {
            if session.status != AgentTaskSessionStatus::Completed {
                return Err(format!(
                    "memory conflict compare session status {:?}",
                    session.status
                ));
            }
            if !session.pending_blockers.is_empty() {
                return Err(format!(
                    "memory conflict compare kept blockers {:?}",
                    session.pending_blockers
                ));
            }
            if !actions.is_empty() {
                return Err("memory conflict compare should not create tool actions".into());
            }
            let context_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Bounded context was selected"))
                .ok_or_else(|| "missing memory conflict context transcript".to_string())?;
            if context_entry
                .metadata
                .get("rawTopKMemoryTrusted")
                .and_then(serde_json::Value::as_bool)
                != Some(false)
            {
                return Err("memory conflict must not trust raw top-k memory snippets".into());
            }
            let memory_source_count = context_entry_source_prefix_count(
                &context_entry.metadata,
                "selected_personal_context",
                "memory:",
            );
            if memory_source_count != 2 {
                return Err(format!(
                    "memory conflict should load exactly two accepted memory records, got {memory_source_count}"
                ));
            }
            let conflict_evidence =
                main_chat_command_surface_eval_memory_conflict_evidence(state).await?;
            if conflict_evidence.graph_conflict_count != 2
                || conflict_evidence.lifecycle_record_count != 2
                || conflict_evidence.distinct_conflict_id_count != 2
            {
                return Err(format!(
                    "memory conflict evidence incomplete: graph={} lifecycle={} conflict_ids={}",
                    conflict_evidence.graph_conflict_count,
                    conflict_evidence.lifecycle_record_count,
                    conflict_evidence.distinct_conflict_id_count
                ));
            }
            let generation_entry = transcript
                .iter()
                .find(|entry| {
                    entry
                        .summary
                        .contains("DirectAnswer generated a model response")
                })
                .ok_or_else(|| {
                    "missing memory-conflict DirectAnswer generation transcript".to_string()
                })?;
            if generation_entry
                .metadata
                .get("providerGenerationPath")
                .and_then(serde_json::Value::as_str)
                != Some("main_chat_direct_answer_scheduler")
                || generation_entry
                    .metadata
                    .get("directWritesExecuted")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err("memory-conflict DirectAnswer generation metadata incomplete".into());
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
                .get("sourceKind")
                .and_then(serde_json::Value::as_str)
                != Some("file")
                || metadata
                    .get("sourceLabel")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(str::is_empty)
                || metadata
                    .get("preview")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(str::is_empty)
            {
                return Err(
                    "file read observation must expose source metadata for the control plane"
                        .into(),
                );
            }
            let read_evidence = metadata
                .get("structuredResult")
                .and_then(|value| value.get("readExecutionEvidence"))
                .ok_or_else(|| {
                    "file read observation missing read execution evidence".to_string()
                })?;
            if read_evidence
                .get("kind")
                .and_then(serde_json::Value::as_str)
                != Some("file_system_read")
                || read_evidence
                    .get("realReadOnlyExecution")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true)
                || read_evidence
                    .get("fixtureBacked")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err(
                    "file read observation did not prove real read-only file execution".into(),
                );
            }
            assert_response_agent_state_read_execution(response, "file_system_read", true, false)?;
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
        MainChatCommandSurfaceEvalScenario::SessionSearchSuccess => {
            if session.status != AgentTaskSessionStatus::Completed {
                return Err(format!(
                    "session search session status {:?}",
                    session.status
                ));
            }
            if !session.pending_blockers.is_empty() {
                return Err(format!(
                    "session search kept blockers {:?}",
                    session.pending_blockers
                ));
            }
            let completed_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
                .ok_or_else(|| {
                    "missing session search AgentLoop completion transcript".to_string()
                })?;
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
                return Err("session search AgentLoop metadata incomplete".into());
            }
            let session_action = actions
                .iter()
                .find(|action| action.action.action_type == "session.search")
                .ok_or_else(|| "missing session.search action".to_string())?;
            if session_action.status != ExecutionQueueStatus::Completed {
                return Err(format!(
                    "session.search action status {:?}",
                    session_action.status
                ));
            }
            let metadata = session_action
                .observation_metadata
                .as_ref()
                .ok_or_else(|| "missing session.search observation metadata".to_string())?;
            if metadata
                .get("sourceKind")
                .and_then(serde_json::Value::as_str)
                != Some("session")
                || metadata
                    .get("sourceLabel")
                    .and_then(serde_json::Value::as_str)
                    != Some("session.search")
                || metadata
                    .get("preview")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(str::is_empty)
            {
                return Err(
                    "session search observation must expose source metadata for the control plane"
                        .into(),
                );
            }
            let structured = metadata
                .get("structuredResult")
                .ok_or_else(|| "session search missing structured result".to_string())?;
            if structured
                .get("hitCount")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default()
                == 0
                || structured
                    .get("directWritesExecuted")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || structured
                    .get("promotedToMemory")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err(
                    "session search must return hits without silent memory promotion".into(),
                );
            }
            let read_evidence = structured.get("readExecutionEvidence").ok_or_else(|| {
                "session search observation missing read execution evidence".to_string()
            })?;
            if read_evidence
                .get("kind")
                .and_then(serde_json::Value::as_str)
                != Some("session_read")
                || read_evidence
                    .get("realReadOnlyExecution")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true)
                || read_evidence
                    .get("fixtureBacked")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err(
                    "session search observation did not prove real read-only session execution"
                        .into(),
                );
            }
            assert_response_agent_state_read_execution(response, "session_read", true, false)?;
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
                return Err("session.search action metadata incomplete".into());
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
        MainChatCommandSurfaceEvalScenario::SelectedSkillContextSuccess => {
            if session.status != AgentTaskSessionStatus::Completed {
                return Err(format!(
                    "selected skill session status {:?}",
                    session.status
                ));
            }
            if !session.pending_blockers.is_empty() {
                return Err(format!(
                    "selected skill context kept blockers {:?}",
                    session.pending_blockers
                ));
            }
            let context_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Bounded context was selected"))
                .ok_or_else(|| "missing selected skill context transcript".to_string())?;
            if context_entry
                .metadata
                .get("selectedSkillInstructionLoaded")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                return Err("selected skill instruction was not marked loaded".into());
            }
            if !context_entry_sources_contain(
                &context_entry.metadata,
                "skill_instruction",
                "skills/phase_e_review/SKILL.md",
            ) {
                return Err("selected phase_e_review SKILL.md source missing".into());
            }
            if context_entry_sources_contain(
                &context_entry.metadata,
                "skill_instruction",
                "skills/unselected_context/SKILL.md",
            ) {
                return Err("unselected skill instruction was injected".into());
            }
            if !actions.iter().any(|action| {
                action.action.action_type == "plan_execute.create_session"
                    && action.status == ExecutionQueueStatus::Completed
            }) {
                return Err("selected skill plan review did not complete a governed action".into());
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
                return Err("selected skill plan review missing final governed delivery".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::KnowledgeAssetContextSuccess => {
            if session.status != AgentTaskSessionStatus::Completed {
                return Err(format!(
                    "knowledge asset context session status {:?}",
                    session.status
                ));
            }
            if !session.pending_blockers.is_empty() {
                return Err(format!(
                    "knowledge asset context kept blockers {:?}",
                    session.pending_blockers
                ));
            }
            if !actions.is_empty() {
                return Err(
                    "knowledge asset context inspection should not create tool actions".into(),
                );
            }
            let context_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Bounded context was selected"))
                .ok_or_else(|| "missing knowledge asset context transcript".to_string())?;
            if context_entry
                .metadata
                .get("workspacePolicyOverrideBlocked")
                .and_then(serde_json::Value::as_bool)
                != Some(true)
                || context_entry
                    .metadata
                    .get("rawLifeModelYamlIncluded")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || context_entry
                    .metadata
                    .get("rawTopKMemoryTrusted")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || context_entry
                    .metadata
                    .get("contextSnapshotRef")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(str::is_empty)
            {
                return Err(
                    "knowledge asset context metadata did not preserve runtime policy boundaries"
                        .into(),
                );
            }
            let required_sources = [
                ("workspace_instruction", "app_configured:AGENTS.md"),
                ("materialized_file", "app_configured:SOUL.md"),
                ("selected_personal_context", "app_configured:USER.md"),
                ("selected_personal_context", "app_configured:MEMORY.md"),
            ];
            for (source_kind, source_id) in required_sources {
                if !context_entry_sources_contain(&context_entry.metadata, source_kind, source_id) {
                    return Err(format!(
                        "knowledge asset source missing from bounded context: {source_kind}:{source_id}"
                    ));
                }
            }
            let loaded_count = context_entry_source_prefix_count(
                &context_entry.metadata,
                "workspace_instruction",
                "app_configured:",
            ) + context_entry_source_prefix_count(
                &context_entry.metadata,
                "materialized_file",
                "app_configured:",
            ) + context_entry_source_prefix_count(
                &context_entry.metadata,
                "selected_personal_context",
                "app_configured:",
            );
            if loaded_count != 4 {
                return Err(format!(
                    "knowledge asset context should load exactly four scoped configured assets, got {loaded_count}"
                ));
            }
            let generation_entry = transcript
                .iter()
                .find(|entry| {
                    entry
                        .summary
                        .contains("DirectAnswer generated a model response")
                })
                .ok_or_else(|| {
                    "missing knowledge-asset DirectAnswer generation transcript".to_string()
                })?;
            if generation_entry
                .metadata
                .get("providerGenerationPath")
                .and_then(serde_json::Value::as_str)
                != Some("main_chat_direct_answer_scheduler")
                || generation_entry
                    .metadata
                    .get("directWritesExecuted")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
            {
                return Err("knowledge-asset DirectAnswer generation metadata incomplete".into());
            }
        }
        MainChatCommandSurfaceEvalScenario::KnowledgeAssetEditProposal => {
            if session.status != AgentTaskSessionStatus::WaitingPermission {
                return Err(format!(
                    "knowledge asset edit proposal session status {:?}",
                    session.status
                ));
            }
            if !session
                .pending_blockers
                .iter()
                .any(|blocker| blocker.starts_with("proposal:"))
            {
                return Err("knowledge asset edit proposal blocker not preserved".into());
            }
            let edit_action = actions
                .iter()
                .find(|action| action.action.action_type == "knowledge.propose_edit")
                .ok_or_else(|| "missing knowledge.propose_edit action".to_string())?;
            if edit_action.status != ExecutionQueueStatus::Completed {
                return Err(format!(
                    "knowledge.propose_edit action status {:?}",
                    edit_action.status
                ));
            }
            let proposal = proposals
                .iter()
                .find(|proposal| {
                    proposal.source == openlife_core::agent::ProposalSource::ChatConversation
                        && proposal.source_detail.as_deref()
                            == Some(
                                format!("main_chat_agent_task_session:{task_session_id}").as_str(),
                            )
                        && proposal.affected_path == "knowledge_asset.AGENTS.md"
                })
                .ok_or_else(|| "knowledge asset edit proposal not linked to task".to_string())?;
            if proposal.proposal_type != openlife_core::agent::ProposalType::LifeModelUpdate
                || proposal
                    .after
                    .get("assetId")
                    .and_then(serde_json::Value::as_str)
                    != Some("AGENTS.md")
                || proposal.after.get("proposedDiff").is_none()
                || proposal
                    .after
                    .get("directKnowledgeFileWrite")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || proposal
                    .after
                    .get("requiresReviewCenterApproval")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true)
            {
                return Err(format!(
                    "knowledge asset edit proposal payload incomplete: {:?}",
                    proposal.after
                ));
            }
            if actions.iter().any(|action| {
                matches!(
                    action.action.action_type.as_str(),
                    "file.write" | "file.update" | "knowledge.write"
                )
            }) {
                return Err("knowledge asset edit must not execute a direct file write".into());
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
                .get("sourceKind")
                .and_then(serde_json::Value::as_str)
                != Some("web")
                || metadata
                    .get("sourceLabel")
                    .and_then(serde_json::Value::as_str)
                    != Some("web.search")
                || metadata
                    .get("preview")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(str::is_empty)
            {
                return Err(
                    "web AgentLoop observation must expose source metadata for the control plane"
                        .into(),
                );
            }
            let read_evidence = metadata
                .get("structuredResult")
                .and_then(|value| value.get("readExecutionEvidence"))
                .ok_or_else(|| {
                    "web AgentLoop observation missing read execution evidence".to_string()
                })?;
            if read_evidence
                .get("kind")
                .and_then(serde_json::Value::as_str)
                != Some("web_search_fixture")
                || read_evidence
                    .get("realReadOnlyExecution")
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
                || read_evidence
                    .get("fixtureBacked")
                    .and_then(serde_json::Value::as_bool)
                    != Some(true)
            {
                return Err("fixture-backed web AgentLoop success must not be counted as real web read evidence".into());
            }
            assert_response_agent_state_read_execution(
                response,
                "web_search_fixture",
                false,
                true,
            )?;
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
            assert_mcp_read_success_action(actions, response, false)?;
        }
        MainChatCommandSurfaceEvalScenario::RegisteredMcpAgentLoopSuccess => {
            assert_mcp_read_success_action(actions, response, true)?;
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
        MainChatCommandSurfaceEvalScenario::MultiReadAgentLoopSuccess => {
            if session.status != AgentTaskSessionStatus::Completed {
                return Err(format!("multi-read session status {:?}", session.status));
            }
            if !session.pending_blockers.is_empty() {
                return Err(format!(
                    "multi-read kept blockers {:?}",
                    session.pending_blockers
                ));
            }
            let completed_entry = transcript
                .iter()
                .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
                .ok_or_else(|| "missing multi-read AgentLoop completion transcript".to_string())?;
            let tool_call_count = metadata_usize(&completed_entry.metadata, "toolCallCount");
            let action_count = metadata_usize(&completed_entry.metadata, "agentLoopActionCount");
            let observation_count =
                metadata_usize(&completed_entry.metadata, "agentLoopObservationCount");
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
                || tool_call_count < 2
                || action_count < 2
                || observation_count < 2
            {
                return Err(format!(
                    "multi-read AgentLoop metadata incomplete: tool_calls={tool_call_count}, actions={action_count}, observations={observation_count}"
                ));
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
    response: Option<&serde_json::Value>,
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
        .get("sourceKind")
        .and_then(serde_json::Value::as_str)
        != Some("mcp")
        || metadata
            .get("sourceLabel")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        || metadata
            .get("preview")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err(
            "registered MCP read observation must expose source metadata for the control plane"
                .into(),
        );
    }
    let read_evidence = metadata
        .get("structuredResult")
        .and_then(|value| value.get("readExecutionEvidence"))
        .ok_or_else(|| {
            "registered MCP read observation missing read execution evidence".to_string()
        })?;
    if read_evidence
        .get("kind")
        .and_then(serde_json::Value::as_str)
        != Some("registered_mcp_read")
        || read_evidence
            .get("realReadOnlyExecution")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        || read_evidence
            .get("fixtureBacked")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
    {
        return Err("registered MCP read observation did not prove real MCP read execution".into());
    }
    assert_response_agent_state_read_execution(response, "registered_mcp_read", true, false)?;
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

fn assert_response_agent_state_read_execution(
    response: Option<&serde_json::Value>,
    expected_kind: &str,
    expected_real_read: bool,
    expected_fixture: bool,
) -> Result<(), String> {
    let response = response.ok_or_else(|| "missing command response payload".to_string())?;
    let observations = response
        .get("agent_state")
        .and_then(|value| value.get("observations"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "command response missing agent_state observations".to_string())?;
    let read_execution = observations
        .iter()
        .filter_map(|observation| observation.get("readExecution"))
        .find(|read_execution| {
            read_execution
                .get("kind")
                .and_then(serde_json::Value::as_str)
                == Some(expected_kind)
        })
        .ok_or_else(|| {
            format!(
                "agent_state observations missing readExecution kind {expected_kind}: {observations:?}"
            )
        })?;
    if read_execution
        .get("realReadOnlyExecution")
        .and_then(serde_json::Value::as_bool)
        != Some(expected_real_read)
        || read_execution
            .get("fixtureBacked")
            .and_then(serde_json::Value::as_bool)
            != Some(expected_fixture)
        || read_execution
            .get("directWritesExecuted")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
    {
        return Err(format!(
            "agent_state readExecution evidence mismatch for {expected_kind}: {read_execution:?}"
        ));
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

fn context_entry_sources_contain(
    metadata: &serde_json::Value,
    source_kind: &str,
    source_id: &str,
) -> bool {
    metadata
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|sources| {
            sources.iter().any(|source| {
                source.get("sourceKind").and_then(serde_json::Value::as_str) == Some(source_kind)
                    && source.get("sourceId").and_then(serde_json::Value::as_str) == Some(source_id)
            })
        })
}

fn context_entry_sources_contain_prefix(
    metadata: &serde_json::Value,
    source_kind: &str,
    source_id_prefix: &str,
) -> bool {
    metadata
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|sources| {
            sources.iter().any(|source| {
                source.get("sourceKind").and_then(serde_json::Value::as_str) == Some(source_kind)
                    && source
                        .get("sourceId")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|source_id| source_id.starts_with(source_id_prefix))
            })
        })
}

fn context_entry_source_count(metadata: &serde_json::Value, source_kind: &str) -> usize {
    metadata
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .map(|sources| {
            sources
                .iter()
                .filter(|source| {
                    source.get("sourceKind").and_then(serde_json::Value::as_str)
                        == Some(source_kind)
                })
                .count()
        })
        .unwrap_or_default()
}

fn context_entry_source_prefix_count(
    metadata: &serde_json::Value,
    source_kind: &str,
    source_id_prefix: &str,
) -> usize {
    metadata
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .map(|sources| {
            sources
                .iter()
                .filter(|source| {
                    source.get("sourceKind").and_then(serde_json::Value::as_str)
                        == Some(source_kind)
                        && source
                            .get("sourceId")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|source_id| source_id.starts_with(source_id_prefix))
                })
                .count()
        })
        .unwrap_or_default()
}

fn main_chat_command_surface_eval_selected_skill_loaded(
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    source_id: &str,
) -> bool {
    transcript
        .iter()
        .find(|entry| entry.summary.contains("Bounded context was selected"))
        .is_some_and(|entry| {
            context_entry_sources_contain(&entry.metadata, "skill_instruction", source_id)
        })
}

fn main_chat_command_surface_eval_memory_context_source_count(
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
) -> usize {
    transcript
        .iter()
        .find(|entry| entry.summary.contains("Bounded context was selected"))
        .map(|entry| {
            context_entry_source_prefix_count(
                &entry.metadata,
                "selected_personal_context",
                "memory:",
            )
        })
        .unwrap_or_default()
}

fn main_chat_command_surface_eval_knowledge_asset_source_count(
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
) -> usize {
    transcript
        .iter()
        .find(|entry| entry.summary.contains("Bounded context was selected"))
        .map(|entry| {
            context_entry_source_prefix_count(
                &entry.metadata,
                "workspace_instruction",
                "app_configured:",
            ) + context_entry_source_prefix_count(
                &entry.metadata,
                "materialized_file",
                "app_configured:",
            ) + context_entry_source_prefix_count(
                &entry.metadata,
                "selected_personal_context",
                "app_configured:",
            )
        })
        .unwrap_or_default()
}

fn main_chat_command_surface_eval_knowledge_asset_scope_digest_loaded(
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
) -> bool {
    transcript
        .iter()
        .find(|entry| entry.summary.contains("Bounded context was selected"))
        .is_some_and(|entry| {
            entry
                .metadata
                .get("workspacePolicyOverrideBlocked")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
                && entry
                    .metadata
                    .get("contextSnapshotRef")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
        })
}

fn main_chat_command_surface_eval_agent_loop_count(
    transcript: &[openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry],
    key: &str,
) -> usize {
    transcript
        .iter()
        .find(|entry| entry.summary.contains("Governed ReAct AgentLoop completed"))
        .map(|entry| metadata_usize(&entry.metadata, key))
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, Default)]
struct MainChatCommandSurfaceMemoryConflictEvidence {
    graph_conflict_count: usize,
    lifecycle_record_count: usize,
    distinct_conflict_id_count: usize,
}

async fn main_chat_command_surface_eval_memory_conflict_evidence(
    state: &Arc<AppState>,
) -> Result<MainChatCommandSurfaceMemoryConflictEvidence, String> {
    let records = {
        let evidence_store = state.evidence_store.lock().await;
        evidence_store
            .query(openlife_core::agent::EvidenceQuery::default())
            .map_err(|error| {
                format!("query command-surface memory conflict evidence failed: {error}")
            })?
    };
    let graph = openlife_core::agent::evaluate_evidence_graph(
        openlife_core::agent::EvidenceGraphInput::new(records, chrono::Utc::now()),
    );
    let (lifecycle_record_count, distinct_conflict_id_count) =
        if let Some(lifecycle_store) = state.memory_lifecycle_store.as_ref() {
            let store = lifecycle_store.lock().await;
            let records = store.list_active_records(None, 20).map_err(|error| {
                format!("list memory conflict lifecycle records failed: {error}")
            })?;
            let distinct_conflict_ids = records
                .iter()
                .flat_map(|record| record.conflict_ids.iter().cloned())
                .collect::<std::collections::BTreeSet<_>>();
            (
                records
                    .iter()
                    .filter(|record| !record.conflict_ids.is_empty())
                    .count(),
                distinct_conflict_ids.len(),
            )
        } else {
            (0, 0)
        };

    Ok(MainChatCommandSurfaceMemoryConflictEvidence {
        graph_conflict_count: graph.conflict_count,
        lifecycle_record_count,
        distinct_conflict_id_count,
    })
}

#[derive(Debug, Clone, Copy, Default)]
struct MainChatCommandSurfaceKnowledgeAssetEditEvidence {
    proposal_created: bool,
    proposed_diff_present: bool,
    direct_write_detected: bool,
}

fn main_chat_command_surface_eval_knowledge_asset_edit_evidence(
    proposals: &[openlife_core::agent::AgentProposal],
) -> MainChatCommandSurfaceKnowledgeAssetEditEvidence {
    let Some(proposal) = proposals
        .iter()
        .find(|proposal| proposal.affected_path == "knowledge_asset.AGENTS.md")
    else {
        return MainChatCommandSurfaceKnowledgeAssetEditEvidence::default();
    };
    MainChatCommandSurfaceKnowledgeAssetEditEvidence {
        proposal_created: true,
        proposed_diff_present: proposal.after.get("proposedDiff").is_some(),
        direct_write_detected: proposal
            .after
            .get("directKnowledgeFileWrite")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    }
}

fn metadata_usize(metadata: &serde_json::Value, key: &str) -> usize {
    metadata
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .map(|value| value.min(usize::MAX as u64) as usize)
        .unwrap_or_default()
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
    pub(crate) case_evidence: Vec<MainChatCommandSurfaceEvalEvidence>,
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
            case_evidence: evidence,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatCommandSurfaceEvalEvidence {
    pub(crate) entry_point: MainChatCommandSurfaceEvalEntryPoint,
    pub(crate) scenario: MainChatCommandSurfaceEvalScenario,
    pub(crate) task_session_id: String,
    pub(crate) run_ids: Vec<String>,
    pub(crate) transcript_entry_count: usize,
    pub(crate) action_count: usize,
    pub(crate) proposal_count: usize,
    pub(crate) run_count: usize,
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
    pub(crate) selected_skill_id: Option<String>,
    pub(crate) selected_skill_instruction_loaded: bool,
    pub(crate) unselected_skill_instruction_loaded: bool,
    pub(crate) memory_context_active_record_count: usize,
    pub(crate) knowledge_asset_context_source_count: usize,
    pub(crate) knowledge_asset_scope_digest_loaded: bool,
    pub(crate) agent_loop_tool_call_count: usize,
    pub(crate) agent_loop_observation_count: usize,
    pub(crate) memory_conflict_graph_conflict_count: usize,
    pub(crate) memory_conflict_lifecycle_record_count: usize,
    pub(crate) memory_conflict_distinct_conflict_id_count: usize,
    pub(crate) knowledge_asset_edit_proposal_created: bool,
    pub(crate) knowledge_asset_edit_proposed_diff_present: bool,
    pub(crate) knowledge_asset_edit_direct_write_detected: bool,
}

impl MainChatCommandSurfaceEvalEvidence {
    pub(crate) fn for_case(
        entry_point: MainChatCommandSurfaceEvalEntryPoint,
        scenario: MainChatCommandSurfaceEvalScenario,
        task_session_id: String,
        run_ids: Vec<String>,
        transcript_entry_count: usize,
        action_count: usize,
        proposal_count: usize,
        run_count: usize,
        legacy_fallback_used: bool,
        silent_write_detected: bool,
        selected_skill_id: Option<String>,
        selected_skill_instruction_loaded: bool,
        unselected_skill_instruction_loaded: bool,
        memory_context_active_record_count: usize,
        knowledge_asset_context_source_count: usize,
        knowledge_asset_scope_digest_loaded: bool,
        agent_loop_tool_call_count: usize,
        agent_loop_observation_count: usize,
        memory_conflict_graph_conflict_count: usize,
        memory_conflict_lifecycle_record_count: usize,
        memory_conflict_distinct_conflict_id_count: usize,
        knowledge_asset_edit_proposal_created: bool,
        knowledge_asset_edit_proposed_diff_present: bool,
        knowledge_asset_edit_direct_write_detected: bool,
    ) -> Self {
        Self {
            entry_point,
            scenario,
            task_session_id,
            run_ids,
            transcript_entry_count,
            action_count,
            proposal_count,
            run_count,
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
            selected_skill_id,
            selected_skill_instruction_loaded,
            unselected_skill_instruction_loaded,
            memory_context_active_record_count,
            knowledge_asset_context_source_count,
            knowledge_asset_scope_digest_loaded,
            agent_loop_tool_call_count,
            agent_loop_observation_count,
            memory_conflict_graph_conflict_count,
            memory_conflict_lifecycle_record_count,
            memory_conflict_distinct_conflict_id_count,
            knowledge_asset_edit_proposal_created,
            knowledge_asset_edit_proposed_diff_present,
            knowledge_asset_edit_direct_write_detected,
        }
    }
}

fn usize_to_u32_saturating(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}
