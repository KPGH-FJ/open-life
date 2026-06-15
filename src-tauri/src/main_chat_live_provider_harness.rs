use crate::main_chat_generation_support::{main_chat_provider_endpoint_kind, preview_text};
use crate::main_chat_send::send_message_with_state;
use crate::{main_chat_command_surface_eval, main_chat_eval_state, main_chat_final_gate, AppState};
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
                    tool_permission_proposal_target: None,
                    tool_selection_candidate_count: 0,
                    tool_selection_candidate_ids: Vec::new(),
                    tool_selection_allowlist: Vec::new(),
                    tool_selection_allowed_actions: Vec::new(),
                    model_selected_allowed_tool: false,
                    model_selected_execution_policy_validated: false,
                    model_selected_execution_allowed: false,
                    model_selected_governed_arguments: false,
                    model_selected_candidate_id: None,
                    model_selected_candidate_target: None,
                    model_selected_candidate_action_type: None,
                    model_selected_candidate_rank: None,
                    model_selected_candidate_source: None,
                    model_selected_candidate_capabilities_digest: None,
                    model_selected_candidate_match_reason: None,
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
    let tool_selection_candidate_count = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolSelectionCandidateCount"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0);
    let tool_selection_candidate_ids =
        string_array_metadata(&agent_loop_metadata, "toolSelectionCandidateIds");
    let tool_selection_allowlist =
        string_array_metadata(&agent_loop_metadata, "toolSelectionAllowlist");
    let tool_selection_allowed_actions =
        allowed_action_array_metadata(&agent_loop_metadata, "toolSelectionAllowedActions");
    let model_selected_allowed_tool = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("modelSelectedAllowedTool"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let model_selected_execution_policy_validated = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("modelSelectedExecutionPolicyValidated"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let model_selected_execution_allowed = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("modelSelectedExecutionAllowed"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let model_selected_governed_arguments = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("modelSelectedArgumentsSource"))
        .and_then(serde_json::Value::as_str)
        == Some("governed_candidate_contract");
    let model_selected_candidate_id =
        non_empty_string_metadata(&agent_loop_metadata, "toolSelectionCandidateId");
    let model_selected_candidate_target =
        non_empty_string_metadata(&agent_loop_metadata, "toolSelectionCandidateTarget");
    let model_selected_candidate_action_type =
        non_empty_string_metadata(&agent_loop_metadata, "toolSelectionCandidateActionType");
    let model_selected_candidate_rank = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolSelectionCandidateRank"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|rank| usize::try_from(rank).ok());
    let model_selected_candidate_source = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolSelectionCandidateSource"))
        .and_then(serde_json::Value::as_str)
        .filter(|source| !source.trim().is_empty())
        .map(str::to_string);
    let model_selected_candidate_capabilities_digest = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolSelectionCandidateCapabilitiesDigest"))
        .and_then(serde_json::Value::as_str)
        .filter(|digest| !digest.trim().is_empty())
        .map(str::to_string);
    let model_selected_candidate_match_reason = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolSelectionCandidateMatchReason"))
        .and_then(serde_json::Value::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .map(str::to_string);
    let ranked_manifest_trace_present = model_selected_candidate_rank.is_some_and(|rank| rank > 0)
        && model_selected_candidate_source.is_some()
        && model_selected_candidate_capabilities_digest.is_some()
        && model_selected_candidate_match_reason.is_some();
    let candidate_allowlist_trace_present = candidate_allowlist_metadata_trace_present(
        tool_selection_candidate_count,
        &tool_selection_candidate_ids,
        &tool_selection_allowlist,
        &tool_selection_allowed_actions,
        model_selected_candidate_id.as_deref(),
        model_selected_candidate_target.as_deref(),
        model_selected_candidate_action_type.as_deref(),
    );
    let distinct_registered_mcp_candidate_trace_present =
        distinct_registered_mcp_candidate_metadata_trace_present(
            tool_selection_candidate_count,
            &tool_selection_candidate_ids,
            &tool_selection_allowlist,
            &tool_selection_allowed_actions,
        );
    let web_agent_loop_target_trace_present = web_agent_loop_target_metadata_trace_present(
        &tool_selection_candidate_ids,
        &tool_selection_allowlist,
        &tool_selection_allowed_actions,
        model_selected_candidate_id.as_deref(),
        model_selected_candidate_target.as_deref(),
        model_selected_candidate_action_type.as_deref(),
    );
    let react_model_invoked = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("liveProviderInvoked"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let (tool_permission_proposal_created, tool_permission_proposal_target) = if input.scenario
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
                if let Some(proposal) = proposal_store
                    .list_pending_proposals(20)
                    .unwrap_or_default()
                    .into_iter()
                    .find(|proposal| proposal.id == proposal_id)
                {
                    let proposal_target = proposal
                        .after
                        .get("tool_name")
                        .and_then(serde_json::Value::as_str)
                        .filter(|target| !target.trim().is_empty())
                        .map(str::to_string);
                    (
                        proposal.proposal_type
                            == openlife_core::agent::ProposalType::ToolPermission,
                        proposal_target,
                    )
                } else {
                    (false, None)
                }
            } else {
                (false, None)
            }
        } else {
            (false, None)
        }
    } else {
        (false, None)
    };
    let proposal_permission_target_trace_present =
        proposal_permission_target_metadata_trace_present(
            &tool_selection_candidate_ids,
            &tool_selection_allowlist,
            &tool_selection_allowed_actions,
            model_selected_candidate_id.as_deref(),
            model_selected_candidate_target.as_deref(),
            model_selected_candidate_action_type.as_deref(),
            tool_permission_proposal_target.as_deref(),
        );
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
                && tool_selection_candidate_count > 0
                && model_selected_allowed_tool
                && model_selected_execution_policy_validated
                && model_selected_execution_allowed
                && model_selected_governed_arguments
                && ranked_manifest_trace_present
                && candidate_allowlist_trace_present
                && web_agent_loop_target_trace_present
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
                && distinct_registered_mcp_candidate_trace_present
                && model_selected_allowed_tool
                && model_selected_execution_policy_validated
                && model_selected_execution_allowed
                && model_selected_governed_arguments
                && ranked_manifest_trace_present
                && candidate_allowlist_trace_present
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
                && proposal_permission_target_trace_present
                && tool_selection_candidate_count > 0
                && model_selected_allowed_tool
                && model_selected_execution_policy_validated
                && model_selected_execution_allowed
                && model_selected_governed_arguments
                && ranked_manifest_trace_present
                && candidate_allowlist_trace_present
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
        tool_permission_proposal_target,
        tool_selection_candidate_count,
        tool_selection_candidate_ids,
        tool_selection_allowlist,
        tool_selection_allowed_actions,
        model_selected_allowed_tool,
        model_selected_execution_policy_validated,
        model_selected_execution_allowed,
        model_selected_governed_arguments,
        model_selected_candidate_id,
        model_selected_candidate_target,
        model_selected_candidate_action_type,
        model_selected_candidate_rank,
        model_selected_candidate_source,
        model_selected_candidate_capabilities_digest,
        model_selected_candidate_match_reason,
        run_id,
        task_session_id,
        response_preview,
    };
    if !report.ready {
        report.blockers = main_chat_final_gate::main_chat_live_provider_report_blockers(&report);
    }
    Ok(report)
}

fn non_empty_string_metadata(metadata: &Option<serde_json::Value>, key: &str) -> Option<String> {
    metadata
        .as_ref()
        .and_then(|metadata| metadata.get(key))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn string_array_metadata(metadata: &Option<serde_json::Value>, key: &str) -> Vec<String> {
    metadata
        .as_ref()
        .and_then(|metadata| metadata.get(key))
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn allowed_action_array_metadata(
    metadata: &Option<serde_json::Value>,
    key: &str,
) -> Vec<serde_json::Value> {
    metadata
        .as_ref()
        .and_then(|metadata| metadata.get(key))
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| {
                    let action_type = value.get("actionType")?.as_str()?.trim();
                    let target = value.get("target")?.as_str()?.trim();
                    if action_type.is_empty() || target.is_empty() {
                        return None;
                    }
                    Some(serde_json::json!({
                        "actionType": action_type,
                        "target": target,
                    }))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn candidate_allowlist_metadata_trace_present(
    candidate_count: usize,
    candidate_ids: &[String],
    allowlist: &[String],
    allowed_actions: &[serde_json::Value],
    selected_id: Option<&str>,
    selected_target: Option<&str>,
    selected_action_type: Option<&str>,
) -> bool {
    let selected_id = match selected_id {
        Some(id) if !id.trim().is_empty() => id,
        _ => return false,
    };
    let selected_target = match selected_target {
        Some(target) if !target.trim().is_empty() => target,
        _ => return false,
    };
    let selected_action_type = match selected_action_type {
        Some(action_type) if !action_type.trim().is_empty() => action_type,
        _ => return false,
    };

    candidate_count > 0
        && candidate_ids.len() == candidate_count
        && candidate_ids
            .iter()
            .any(|candidate_id| candidate_id == selected_id)
        && allowlist.iter().any(|target| target == selected_target)
        && allowed_actions.iter().any(|action| {
            action.get("actionType").and_then(serde_json::Value::as_str)
                == Some(selected_action_type)
                && action.get("target").and_then(serde_json::Value::as_str) == Some(selected_target)
        })
}

fn web_agent_loop_target_metadata_trace_present(
    candidate_ids: &[String],
    allowlist: &[String],
    allowed_actions: &[serde_json::Value],
    selected_id: Option<&str>,
    selected_target: Option<&str>,
    selected_action_type: Option<&str>,
) -> bool {
    let selected_id = match selected_id {
        Some(id) if id.starts_with("web.") => id,
        _ => return false,
    };
    let selected_target = match selected_target {
        Some(target) if target.starts_with("web.") => target,
        _ => return false,
    };
    let selected_action_type = match selected_action_type {
        Some(action_type) if !action_type.trim().is_empty() => action_type,
        _ => return false,
    };

    candidate_ids
        .iter()
        .any(|candidate_id| candidate_id == selected_id)
        && allowlist
            .iter()
            .any(|target| target == selected_target && target.starts_with("web."))
        && allowed_actions.iter().any(|action| {
            action.get("actionType").and_then(serde_json::Value::as_str)
                == Some(selected_action_type)
                && action
                    .get("target")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|target| target == selected_target && target.starts_with("web."))
        })
}

fn proposal_permission_target_metadata_trace_present(
    candidate_ids: &[String],
    allowlist: &[String],
    allowed_actions: &[serde_json::Value],
    selected_id: Option<&str>,
    selected_target: Option<&str>,
    selected_action_type: Option<&str>,
    proposal_target: Option<&str>,
) -> bool {
    let proposal_target = match proposal_target {
        Some(target)
            if !target.trim().is_empty()
                && !target.starts_with("web.")
                && !target.starts_with("file.") =>
        {
            target
        }
        _ => return false,
    };
    let selected_id = match selected_id {
        Some(id) if !id.trim().is_empty() => id,
        _ => return false,
    };
    let selected_target = match selected_target {
        Some(target) if target == proposal_target => target,
        _ => return false,
    };
    let selected_action_type = match selected_action_type {
        Some("mcp_tool") => "mcp_tool",
        _ => return false,
    };

    candidate_ids
        .iter()
        .any(|candidate_id| candidate_id == selected_id)
        && allowlist.iter().any(|target| target == selected_target)
        && allowed_actions.iter().any(|action| {
            action.get("actionType").and_then(serde_json::Value::as_str)
                == Some(selected_action_type)
                && action.get("target").and_then(serde_json::Value::as_str) == Some(selected_target)
        })
}

fn distinct_registered_mcp_candidate_metadata_trace_present(
    candidate_count: usize,
    candidate_ids: &[String],
    allowlist: &[String],
    allowed_actions: &[serde_json::Value],
) -> bool {
    let distinct_candidate_ids = candidate_ids
        .iter()
        .filter(|candidate_id| !candidate_id.trim().is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_allowed_targets = allowlist
        .iter()
        .filter(|target| !target.trim().is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_allowed_action_pairs = allowed_actions
        .iter()
        .filter_map(|action| {
            let action_type = action.get("actionType")?.as_str()?.trim();
            let target = action.get("target")?.as_str()?.trim();
            if action_type.is_empty() || target.is_empty() {
                return None;
            }
            Some((action_type, target))
        })
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    candidate_count >= 2
        && distinct_candidate_ids >= 2
        && distinct_allowed_targets >= 2
        && distinct_allowed_action_pairs >= 2
}
