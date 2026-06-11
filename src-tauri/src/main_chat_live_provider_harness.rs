use crate::{
    main_chat_command_surface_eval, main_chat_eval_state, main_chat_final_gate,
    main_chat_provider_endpoint_kind, preview_text, send_message_with_state, AppState,
};
use openlife_core::llm::ChatMessage;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct MainChatLiveProviderEvalHarnessInput {
    pub(crate) scenario: main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario,
    pub(crate) session_id: String,
    pub(crate) prompt: String,
    pub(crate) explicit_live_eval_requested: bool,
    pub(crate) local_only_required: bool,
}

pub(crate) fn main_chat_live_provider_eval_opt_in_from_env() -> bool {
    std::env::var("OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL").is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

pub(crate) async fn run_main_chat_live_provider_eval_harness_suite_from_state(
    source_state: &Arc<AppState>,
    explicit_live_eval_requested: bool,
) -> Result<
    (
        openlife_core::agent::main_chat_agent_v1::MainChatLiveProviderEvalPreflightReport,
        Vec<main_chat_final_gate::MainChatLiveProviderEvalHarnessReport>,
    ),
    String,
> {
    let source_config = source_state.config.lock().await.clone();
    let source_scheduler = source_state.scheduler.lock().await.clone();
    let scripted_provider_response_present =
        source_scheduler.scripted_generation_response.is_some();
    let preflight =
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_live_provider_eval_preflight(
            openlife_core::agent::main_chat_agent_v1::MainChatLiveProviderEvalPreflightInput {
                provider: source_scheduler.provider.clone(),
                api_key_present: !source_scheduler.effective_api_key().trim().is_empty(),
                network_enabled: source_config.system.network_policy.enabled,
                explicit_live_eval_requested,
                scripted_provider_response_present,
                local_only_required: false,
            },
        );
    if !explicit_live_eval_requested {
        return Ok((preflight, Vec::new()));
    }

    let mut reports = Vec::new();
    for scenario in [
        main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
        main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
        main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    ] {
        let state = main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut config = state.config.lock().await;
            *config = source_config.clone();
        }
        {
            let mut scheduler = state.scheduler.lock().await;
            *scheduler = source_scheduler.clone();
        }
        match run_main_chat_live_provider_eval_harness(
            state,
            MainChatLiveProviderEvalHarnessInput {
                scenario,
                session_id: format!("final-acceptance-live-{}", scenario.as_str()),
                prompt: scenario.prompt().into(),
                explicit_live_eval_requested: true,
                local_only_required: false,
            },
        )
        .await
        {
            Ok(report) => reports.push(report),
            Err(error) => reports.push(
                main_chat_final_gate::MainChatLiveProviderEvalHarnessReport {
                    scenario,
                    ready: false,
                    status: "failed".into(),
                    provider: preflight.provider.clone(),
                    provider_endpoint_kind: "error".into(),
                    blockers: vec![error],
                    required_evidence:
                        main_chat_final_gate::main_chat_live_provider_required_evidence(),
                    live_provider_invocation_allowed: false,
                    main_chat_invoked: false,
                    model_invoked: false,
                    direct_writes_executed: false,
                    legacy_fallback_used: false,
                    agent_loop_succeeded: false,
                    single_step_fallback_used: false,
                    agent_loop_action_status: None,
                    mcp_read_target_resolved: false,
                    tool_permission_proposal_created: false,
                    run_id: None,
                    task_session_id: None,
                    response_preview: None,
                },
            ),
        }
    }

    Ok((preflight, reports))
}

pub(crate) async fn run_main_chat_live_provider_eval_harness(
    state: Arc<AppState>,
    input: MainChatLiveProviderEvalHarnessInput,
) -> Result<main_chat_final_gate::MainChatLiveProviderEvalHarnessReport, String> {
    let config = state.config.lock().await.clone();
    let scheduler = state.scheduler.lock().await.clone();
    let scripted_provider_response_present = scheduler.scripted_generation_response.is_some();
    let provider_endpoint_kind =
        main_chat_provider_endpoint_kind(&scheduler, scripted_provider_response_present)
            .to_string();
    let preflight =
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_live_provider_eval_preflight(
            openlife_core::agent::main_chat_agent_v1::MainChatLiveProviderEvalPreflightInput {
                provider: scheduler.provider.clone(),
                api_key_present: !scheduler.effective_api_key().trim().is_empty(),
                network_enabled: config.system.network_policy.enabled,
                explicit_live_eval_requested: input.explicit_live_eval_requested,
                scripted_provider_response_present,
                local_only_required: input.local_only_required,
            },
        );
    let mut blockers = preflight.blockers.clone();
    if !matches!(
        provider_endpoint_kind.as_str(),
        "external_provider" | "local_test_http"
    ) {
        blockers.push("external_provider_endpoint_required".into());
    }
    let live_provider_invocation_allowed =
        preflight.live_provider_invocation_allowed && blockers.is_empty();

    if !live_provider_invocation_allowed {
        return Ok(
            main_chat_final_gate::blocked_main_chat_live_provider_eval_harness_report(
                input.scenario,
                preflight.provider,
                provider_endpoint_kind,
                blockers,
                preflight.required_evidence,
            ),
        );
    }

    if input.scenario
        == main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop
    {
        main_chat_command_surface_eval::grant_builtin_echo_read_once(&state).await?;
    }

    let result = send_message_with_state(
        input.session_id.clone(),
        vec![ChatMessage {
            role: "user".into(),
            content: input.prompt.clone(),
        }],
        None,
        &state,
    )
    .await?;
    let response = serde_json::to_value(&result)
        .map_err(|error| format!("serialize live provider eval response failed: {error}"))?;

    let run_id = response
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let task_session_id = response
        .get("agent_ingress")
        .and_then(|value| value.get("agentTaskSessionId"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let legacy_fallback_used = response
        .get("legacy_fallback_used")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let model_invoked = response
        .get("execution_transcript")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry
                    .get("metadata")
                    .and_then(|metadata| metadata.get("liveProviderInvoked"))
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                    && entry
                        .get("metadata")
                        .and_then(|metadata| metadata.get("providerEndpointKind"))
                        .and_then(serde_json::Value::as_str)
                        == Some(provider_endpoint_kind.as_str())
            })
        });
    let agent_loop_metadata = response
        .get("execution_transcript")
        .and_then(serde_json::Value::as_array)
        .and_then(|entries| {
            entries.iter().find_map(|entry| {
                let summary_matches = entry
                    .get("summary")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|summary| summary.contains("Governed ReAct AgentLoop completed"));
                if summary_matches {
                    entry.get("metadata").cloned()
                } else {
                    None
                }
            })
        });
    let agent_loop_succeeded = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("agentLoopSucceeded"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let single_step_fallback_used = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("singleStepFallbackUsed"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let agent_loop_action_status = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("agentLoopActionStatus"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let mcp_read_target_resolved = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("mcpReadTargetResolved"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let react_model_invoked = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("liveProviderInvoked"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let tool_permission_proposal_created = if input.scenario
        == main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal
    {
        if let Some(ref task_session_id) = task_session_id {
            let actions = if let Some(ref queue_arc) = state.main_chat_action_queue_store {
                let queue = queue_arc.lock().await;
                queue.list_for_session(task_session_id).unwrap_or_default()
            } else {
                Vec::new()
            };
            let proposal_id = actions.iter().find_map(|action| {
                action
                    .observation_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.get("proposalId"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            });
            if let (Some(proposal_id), Some(ref proposal_arc)) =
                (proposal_id, state.proposal_store.as_ref())
            {
                let proposal_store = proposal_arc.lock().await;
                proposal_store
                    .list_pending_proposals(20)
                    .unwrap_or_default()
                    .iter()
                    .any(|proposal| {
                        proposal.id == proposal_id
                            && proposal.proposal_type
                                == openlife_core::agent::ProposalType::ToolPermission
                    })
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };
    let direct_writes_executed =
        main_chat_command_surface_eval::json_contains_direct_write_true(&response);
    let response_preview = response
        .get("reply")
        .and_then(serde_json::Value::as_str)
        .map(|reply| preview_text(reply, 240));
    let provider_model_invoked = model_invoked || react_model_invoked;
    let traceable_response = run_id
        .as_ref()
        .is_some_and(|run_id| !run_id.trim().is_empty())
        && task_session_id
            .as_ref()
            .is_some_and(|task_session_id| !task_session_id.trim().is_empty())
        && response_preview
            .as_ref()
            .is_some_and(|preview| !preview.trim().is_empty());
    let completed = match input.scenario {
        main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::DirectAnswer => {
            traceable_response
                && model_invoked
                && !direct_writes_executed
                && !legacy_fallback_used
        }
        main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::WebAgentLoop => {
            traceable_response
                && provider_model_invoked
                && agent_loop_succeeded
                && !single_step_fallback_used
                && agent_loop_action_status.as_deref() == Some("succeeded")
                && !direct_writes_executed
                && !legacy_fallback_used
        }
        main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => {
            traceable_response
                && provider_model_invoked
                && agent_loop_succeeded
                && !single_step_fallback_used
                && agent_loop_action_status.as_deref() == Some("succeeded")
                && mcp_read_target_resolved
                && !direct_writes_executed
                && !legacy_fallback_used
        }
        main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
            traceable_response
                && provider_model_invoked
                && agent_loop_succeeded
                && !single_step_fallback_used
                && agent_loop_action_status.as_deref() == Some("needs_confirmation")
                && tool_permission_proposal_created
                && !direct_writes_executed
                && !legacy_fallback_used
        }
    };

    let mut report = main_chat_final_gate::MainChatLiveProviderEvalHarnessReport {
        scenario: input.scenario,
        ready: completed,
        status: if completed { "completed" } else { "failed" }.into(),
        provider: preflight.provider,
        provider_endpoint_kind,
        blockers: Vec::new(),
        required_evidence: preflight.required_evidence,
        live_provider_invocation_allowed,
        main_chat_invoked: true,
        model_invoked: provider_model_invoked,
        direct_writes_executed,
        legacy_fallback_used,
        agent_loop_succeeded,
        single_step_fallback_used,
        agent_loop_action_status,
        mcp_read_target_resolved,
        tool_permission_proposal_created,
        run_id,
        task_session_id,
        response_preview,
    };
    if !report.ready {
        report.blockers = main_chat_final_gate::main_chat_live_provider_report_blockers(&report);
    }
    Ok(report)
}
