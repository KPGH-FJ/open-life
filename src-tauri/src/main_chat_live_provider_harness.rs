use crate::main_chat_generation_support::{main_chat_provider_endpoint_kind, preview_text};
use crate::main_chat_send::send_message_with_state;
use crate::{main_chat_command_surface_eval, main_chat_eval_state, main_chat_final_gate, AppState};
use openlife_core::llm::ChatMessage;
use std::sync::Arc;

const MAIN_CHAT_LIVE_PROVIDER_CONTRACT_SAFE_LABEL_MAX_LEN: usize = 96;
const MAIN_CHAT_LIVE_PROVIDER_RESPONSE_PREVIEW_MAX_CHARS: usize = 240;
const MAIN_CHAT_LIVE_PROVIDER_WRITE_LIKE_LABEL_TERMS: &[&str] = &[
    "write",
    "send",
    "delete",
    "remove",
    "update",
    "create",
    "modify",
    "mutate",
    "externalwrite",
    "externalsideeffect",
    "realwrite",
    "emailsend",
    "calendarsend",
    "calendarwrite",
    "providerwrite",
    "shellexec",
    "execute",
    "exec",
];

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
                    provider_model: None,
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
                    tool_selection_model_ranked: false,
                    tool_selection_ranking_source: None,
                    tool_selection_ranking_provider: None,
                    tool_selection_ranking_model: None,
                    tool_selection_ranking_route_type: None,
                    tool_selection_ranking_provider_backed: false,
                    tool_selection_model_ranking_ignored: false,
                    tool_selection_model_ranking_candidate_ids: Vec::new(),
                    tool_selection_model_ranking_response_digest: None,
                    model_selected_allowed_tool: false,
                    model_selected_execution_policy_validated: false,
                    model_selected_execution_allowed: false,
                    model_selected_governed_arguments: false,
                    model_selected_governed_arguments_digest: None,
                    model_selected_candidate_id: None,
                    model_selected_candidate_target: None,
                    model_selected_candidate_action_type: None,
                    model_selected_candidate_rank: None,
                    model_selected_candidate_source: None,
                    model_selected_candidate_capabilities_digest: None,
                    model_selected_candidate_capability_labels: None,
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
    let raw_provider = scheduler.provider.clone();
    let provider_endpoint_kind =
        main_chat_provider_endpoint_kind(&scheduler, scripted_provider_response_present)
            .to_string();
    let provider_model = scheduler.chat_model.clone();
    let preflight =
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_live_provider_eval_preflight(
            openlife_core::agent::main_chat_agent_v1::MainChatLiveProviderEvalPreflightInput {
                provider: raw_provider.clone(),
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
                raw_provider,
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
        .and_then(|entries| main_chat_live_provider_agent_loop_metadata_from_entries(entries));
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
    let raw_mcp_read_target_resolved = agent_loop_metadata
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
    let tool_selection_model_ranked = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolSelectionModelRanked"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let tool_selection_ranking_source =
        non_empty_string_metadata(&agent_loop_metadata, "toolSelectionRankingSource");
    let tool_selection_ranking_provider =
        non_empty_string_metadata(&agent_loop_metadata, "toolSelectionRankingProvider");
    let tool_selection_ranking_model =
        non_empty_string_metadata(&agent_loop_metadata, "toolSelectionRankingModel");
    let tool_selection_ranking_route_type =
        non_empty_string_metadata(&agent_loop_metadata, "toolSelectionRankingRouteType");
    let tool_selection_ranking_provider_backed = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolSelectionRankingProviderBacked"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let tool_selection_model_ranking_ignored = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolSelectionModelRankingIgnored"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let tool_selection_model_ranking_candidate_ids = string_array_metadata(
        &agent_loop_metadata,
        "toolSelectionModelRankingCandidateIds",
    );
    let tool_selection_model_ranking_response_digest = non_empty_string_metadata(
        &agent_loop_metadata,
        "toolSelectionModelRankingResponseDigest",
    );
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
    let model_selected_governed_arguments_digest =
        non_empty_string_metadata(&agent_loop_metadata, "modelSelectedGovernedArgumentsDigest");
    let model_selected_governed_arguments = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("modelSelectedArgumentsSource"))
        .and_then(serde_json::Value::as_str)
        == Some("governed_candidate_contract")
        && model_selected_governed_arguments_digest
            .as_deref()
            .is_some_and(metadata_safe_digest_label_present);
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
    let model_selected_candidate_capability_labels = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolSelectionCandidateCapabilityLabels"))
        .and_then(serde_json::Value::as_str)
        .filter(|labels| !labels.trim().is_empty())
        .map(str::to_string);
    let model_selected_candidate_match_reason = agent_loop_metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolSelectionCandidateMatchReason"))
        .and_then(serde_json::Value::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .map(str::to_string);
    let ranked_manifest_trace_present = ranked_manifest_metadata_trace_present(
        model_selected_candidate_rank,
        &tool_selection_candidate_ids,
        model_selected_candidate_id.as_deref(),
        model_selected_candidate_source.as_deref(),
        model_selected_candidate_capabilities_digest.as_deref(),
        model_selected_candidate_capability_labels.as_deref(),
        model_selected_candidate_match_reason.as_deref(),
    );
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
    let provider_ranked_registered_mcp_selection_trace_present =
        provider_ranked_registered_mcp_selection_metadata_trace_present(
            input.scenario,
            &preflight.provider,
            &provider_model,
            tool_selection_model_ranked,
            tool_selection_ranking_source.as_deref(),
            tool_selection_ranking_provider.as_deref(),
            tool_selection_ranking_model.as_deref(),
            tool_selection_ranking_route_type.as_deref(),
            tool_selection_ranking_provider_backed,
            tool_selection_model_ranking_ignored,
            &tool_selection_model_ranking_candidate_ids,
            tool_selection_model_ranking_response_digest.as_deref(),
            &tool_selection_candidate_ids,
            model_selected_candidate_id.as_deref(),
            model_selected_candidate_target.as_deref(),
            model_selected_candidate_action_type.as_deref(),
            model_selected_candidate_rank,
            model_selected_candidate_match_reason.as_deref(),
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
        .map(|reply| {
            live_provider_response_preview(
                reply,
                MAIN_CHAT_LIVE_PROVIDER_RESPONSE_PREVIEW_MAX_CHARS,
            )
        });
    let provider_model_invoked = model_invoked || react_model_invoked;
    let provider_identity_trace_present = live_provider_contract_safe_label(&raw_provider);
    let provider_model_trace_present = live_provider_contract_safe_label(&provider_model);
    let traceable_response = traceable_response_metadata_present(
        run_id.as_deref(),
        task_session_id.as_deref(),
        response_preview.as_deref(),
    );
    let mcp_read_target_resolved = raw_mcp_read_target_resolved
        && input.scenario
            == main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop
        && agent_loop_succeeded
        && !single_step_fallback_used
        && agent_loop_action_status.as_deref() == Some("succeeded")
        && !tool_permission_proposal_created
        && tool_permission_proposal_target.is_none();
    let direct_answer_generation_trace_present = !agent_loop_succeeded
        && !single_step_fallback_used
        && agent_loop_action_status.is_none()
        && !mcp_read_target_resolved
        && !tool_permission_proposal_created
        && tool_permission_proposal_target.is_none()
        && tool_selection_candidate_count == 0
        && tool_selection_candidate_ids.is_empty()
        && tool_selection_allowlist.is_empty()
        && tool_selection_allowed_actions.is_empty()
        && !tool_selection_model_ranked
        && tool_selection_ranking_source.is_none()
        && tool_selection_ranking_provider.is_none()
        && tool_selection_ranking_model.is_none()
        && tool_selection_ranking_route_type.is_none()
        && !tool_selection_ranking_provider_backed
        && !tool_selection_model_ranking_ignored
        && tool_selection_model_ranking_candidate_ids.is_empty()
        && tool_selection_model_ranking_response_digest.is_none()
        && !model_selected_allowed_tool
        && !model_selected_execution_policy_validated
        && !model_selected_execution_allowed
        && !model_selected_governed_arguments
        && model_selected_governed_arguments_digest.is_none()
        && model_selected_candidate_id.is_none()
        && model_selected_candidate_target.is_none()
        && model_selected_candidate_action_type.is_none()
        && model_selected_candidate_rank.is_none()
        && model_selected_candidate_source.is_none()
        && model_selected_candidate_capabilities_digest.is_none()
        && model_selected_candidate_capability_labels.is_none()
        && model_selected_candidate_match_reason.is_none();
    let completed = match input.scenario {
        main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::DirectAnswer => {
            traceable_response
                && model_invoked
                && provider_identity_trace_present
                && provider_model_trace_present
                && direct_answer_generation_trace_present
                && !direct_writes_executed
                && !legacy_fallback_used
        }
        main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::WebAgentLoop => {
            traceable_response
                && provider_model_invoked
                && provider_identity_trace_present
                && provider_model_trace_present
                && agent_loop_succeeded
                && !single_step_fallback_used
                && agent_loop_action_status.as_deref() == Some("succeeded")
                && !mcp_read_target_resolved
                && !tool_permission_proposal_created
                && tool_permission_proposal_target.is_none()
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
                && provider_identity_trace_present
                && provider_model_trace_present
                && agent_loop_succeeded
                && !single_step_fallback_used
                && agent_loop_action_status.as_deref() == Some("succeeded")
                && mcp_read_target_resolved
                && !tool_permission_proposal_created
                && tool_permission_proposal_target.is_none()
                && distinct_registered_mcp_candidate_trace_present
                && provider_ranked_registered_mcp_selection_trace_present
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
                && provider_identity_trace_present
                && provider_model_trace_present
                && agent_loop_succeeded
                && !single_step_fallback_used
                && agent_loop_action_status.as_deref() == Some("needs_confirmation")
                && !mcp_read_target_resolved
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
        provider: raw_provider,
        provider_model: (!provider_model.is_empty()).then_some(provider_model),
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
        tool_selection_model_ranked,
        tool_selection_ranking_source,
        tool_selection_ranking_provider,
        tool_selection_ranking_model,
        tool_selection_ranking_route_type,
        tool_selection_ranking_provider_backed,
        tool_selection_model_ranking_ignored,
        tool_selection_model_ranking_candidate_ids,
        tool_selection_model_ranking_response_digest,
        model_selected_allowed_tool,
        model_selected_execution_policy_validated,
        model_selected_execution_allowed,
        model_selected_governed_arguments,
        model_selected_governed_arguments_digest,
        model_selected_candidate_id,
        model_selected_candidate_target,
        model_selected_candidate_action_type,
        model_selected_candidate_rank,
        model_selected_candidate_source,
        model_selected_candidate_capabilities_digest,
        model_selected_candidate_capability_labels,
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

pub(crate) fn main_chat_live_provider_agent_loop_metadata_from_entries(
    entries: &[serde_json::Value],
) -> Option<serde_json::Value> {
    let mut attempted_metadata = None;
    for entry in entries {
        let summary = entry
            .get("summary")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let Some(metadata) = entry.get("metadata") else {
            continue;
        };
        if summary.contains("Governed ReAct AgentLoop completed") {
            return Some(metadata.clone());
        }
        if summary.contains("Governed ReAct AgentLoop")
            && metadata
                .get("agentLoopAttempted")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        {
            attempted_metadata = Some(metadata.clone());
        }
    }
    attempted_metadata
}

fn live_provider_response_preview(reply: &str, max_chars: usize) -> String {
    let printable = reply
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    let single_line = printable.split_whitespace().collect::<Vec<_>>().join(" ");
    preview_text(&single_line, max_chars)
}

fn traceable_response_metadata_present(
    run_id: Option<&str>,
    task_session_id: Option<&str>,
    response_preview: Option<&str>,
) -> bool {
    run_id.is_some_and(live_provider_contract_safe_label)
        && task_session_id.is_some_and(live_provider_contract_safe_label)
        && response_preview.is_some_and(live_provider_response_preview_trace_present)
}

fn live_provider_response_preview_trace_present(preview: &str) -> bool {
    let normalized_preview = preview.split_whitespace().collect::<Vec<_>>().join(" ");
    !preview.is_empty()
        && normalized_preview == preview
        && preview.chars().count() <= MAIN_CHAT_LIVE_PROVIDER_RESPONSE_PREVIEW_MAX_CHARS
        && preview.chars().all(|ch| !ch.is_control())
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
                .filter_map(allowed_action_exact_pair)
                .map(|(action_type, target)| {
                    serde_json::json!({
                        "actionType": action_type,
                        "target": target,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn allowed_action_exact_pair(action: &serde_json::Value) -> Option<(&str, &str)> {
    let object = action.as_object()?;
    if object.len() != 2 {
        return None;
    }
    let action_type = object.get("actionType")?.as_str()?;
    let target = object.get("target")?.as_str()?;
    if !live_provider_contract_safe_label(action_type) || !live_provider_contract_safe_label(target)
    {
        return None;
    }
    Some((action_type, target))
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
        Some(id) if live_provider_contract_safe_label(id) => id,
        _ => return false,
    };
    let selected_target = match selected_target {
        Some(target) if live_provider_contract_safe_label(target) => target,
        _ => return false,
    };
    let selected_action_type = match selected_action_type {
        Some(action_type) if live_provider_contract_safe_label(action_type) => action_type,
        _ => return false,
    };

    candidate_count > 0
        && candidate_ids.len() == candidate_count
        && allowlist.len() == candidate_count
        && allowed_actions.len() == candidate_count
        && exact_candidate_allowlist_sets_present(
            candidate_count,
            candidate_ids,
            allowlist,
            allowed_actions,
        )
        && allowed_action_types_match_selected(allowed_actions, selected_action_type)
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

fn exact_candidate_allowlist_sets_present(
    candidate_count: usize,
    candidate_ids: &[String],
    allowlist: &[String],
    allowed_actions: &[serde_json::Value],
) -> bool {
    let candidate_targets = candidate_ids
        .iter()
        .filter(|candidate_id| live_provider_contract_safe_label(candidate_id))
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let allowed_targets = allowlist
        .iter()
        .filter(|target| live_provider_contract_safe_label(target))
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let action_targets = allowed_actions
        .iter()
        .filter_map(allowed_action_exact_pair)
        .map(|(_, target)| target)
        .collect::<std::collections::BTreeSet<_>>();

    candidate_targets.len() == candidate_count
        && allowed_targets.len() == candidate_count
        && action_targets.len() == candidate_count
        && candidate_targets == allowed_targets
        && candidate_targets == action_targets
}

fn allowed_action_types_match_selected(
    allowed_actions: &[serde_json::Value],
    selected_action_type: &str,
) -> bool {
    if !live_provider_contract_safe_label(selected_action_type) {
        return false;
    }
    allowed_actions.iter().all(|action| {
        matches!(
            allowed_action_exact_pair(action),
            Some((action_type, _)) if action_type == selected_action_type
        )
    })
}

fn selected_candidate_rank_matches_candidate_order(
    candidate_ids: &[String],
    selected_candidate_id: Option<&str>,
    selected_rank: Option<usize>,
) -> bool {
    let selected_candidate_id = match selected_candidate_id {
        Some(candidate_id) if live_provider_contract_safe_label(candidate_id) => candidate_id,
        _ => return false,
    };
    let Some(selected_rank) = selected_rank.filter(|rank| *rank > 0) else {
        return false;
    };

    candidate_ids
        .iter()
        .position(|candidate_id| candidate_id == selected_candidate_id)
        .is_some_and(|index| index + 1 == selected_rank)
}

fn ranked_manifest_metadata_trace_present(
    selected_rank: Option<usize>,
    candidate_ids: &[String],
    selected_candidate_id: Option<&str>,
    selected_candidate_source: Option<&str>,
    selected_candidate_capabilities_digest: Option<&str>,
    selected_candidate_capability_labels: Option<&str>,
    selected_candidate_match_reason: Option<&str>,
) -> bool {
    selected_rank.is_some_and(|rank| rank > 0)
        && selected_candidate_rank_matches_candidate_order(
            candidate_ids,
            selected_candidate_id,
            selected_rank,
        )
        && selected_candidate_source.is_some_and(live_provider_contract_safe_label)
        && selected_candidate_capabilities_digest.is_some_and(metadata_safe_digest_label_present)
        && selected_candidate_capability_labels
            .is_some_and(live_provider_capability_labels_trace_present)
        && selected_candidate_match_reason.is_some_and(live_provider_contract_safe_label)
}

fn live_provider_capability_labels_trace_present(labels: &str) -> bool {
    live_provider_contract_safe_label(labels)
        && labels != "none"
        && labels
            .split('/')
            .any(|label| label.eq_ignore_ascii_case("read"))
        && labels.split('/').all(|label| {
            live_provider_contract_safe_label(label)
                && !live_provider_write_like_capability_label(label)
        })
}

fn live_provider_write_like_capability_label(label: &str) -> bool {
    let label = label.to_ascii_lowercase();
    MAIN_CHAT_LIVE_PROVIDER_WRITE_LIKE_LABEL_TERMS
        .iter()
        .any(|term| label.contains(term))
        || label.ends_with("write")
        || label.ends_with("send")
        || label.ends_with("delete")
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
        Some(id) if id.starts_with("web.") && live_provider_contract_safe_label(id) => id,
        _ => return false,
    };
    let selected_target = match selected_target {
        Some(target) if target.starts_with("web.") && live_provider_contract_safe_label(target) => {
            target
        }
        _ => return false,
    };
    if selected_id != selected_target {
        return false;
    }
    let selected_action_type = match selected_action_type {
        Some("mcp_tool") => "mcp_tool",
        _ => return false,
    };

    candidate_ids
        .iter()
        .any(|candidate_id| candidate_id == selected_id)
        && allowlist
            .iter()
            .any(|target| target == selected_target && target.starts_with("web."))
        && allowed_actions
            .iter()
            .filter_map(allowed_action_exact_pair)
            .any(|(action_type, target)| {
                action_type == selected_action_type
                    && target == selected_target
                    && target.starts_with("web.")
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
                && live_provider_contract_safe_label(target)
                && !target.starts_with("web.")
                && !target.starts_with("file.") =>
        {
            target
        }
        _ => return false,
    };
    let selected_id = match selected_id {
        Some(id) if live_provider_contract_safe_label(id) => id,
        _ => return false,
    };
    let selected_target = match selected_target {
        Some(target) if target == proposal_target => target,
        _ => return false,
    };
    if selected_id != selected_target {
        return false;
    }
    let selected_action_type = match selected_action_type {
        Some("mcp_tool") => "mcp_tool",
        _ => return false,
    };

    candidate_ids
        .iter()
        .any(|candidate_id| candidate_id == selected_id)
        && allowlist.iter().any(|target| target == selected_target)
        && allowed_actions
            .iter()
            .filter_map(allowed_action_exact_pair)
            .any(|(action_type, target)| {
                action_type == selected_action_type && target == selected_target
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
        .filter(|candidate_id| live_provider_contract_safe_label(candidate_id))
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_allowed_targets = allowlist
        .iter()
        .filter(|target| live_provider_contract_safe_label(target))
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_allowed_action_pairs = allowed_actions
        .iter()
        .filter_map(allowed_action_exact_pair)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let candidate_targets = candidate_ids
        .iter()
        .filter(|candidate_id| live_provider_contract_safe_label(candidate_id))
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let allowed_targets = allowlist
        .iter()
        .filter(|target| live_provider_contract_safe_label(target))
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let action_targets = allowed_actions
        .iter()
        .filter_map(allowed_action_exact_pair)
        .filter_map(|(action_type, target)| (action_type == "mcp_tool").then_some(target))
        .collect::<std::collections::BTreeSet<_>>();

    candidate_count >= 2
        && candidate_ids.len() == candidate_count
        && distinct_candidate_ids == candidate_count
        && allowlist.len() == candidate_count
        && distinct_allowed_targets == candidate_count
        && allowed_actions.len() == candidate_count
        && distinct_allowed_action_pairs == candidate_count
        && candidate_targets == allowed_targets
        && candidate_targets == action_targets
}

fn provider_ranked_registered_mcp_selection_metadata_trace_present(
    scenario: main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario,
    expected_provider: &str,
    expected_model: &str,
    model_ranked: bool,
    ranking_source: Option<&str>,
    ranking_provider: Option<&str>,
    ranking_model: Option<&str>,
    ranking_route_type: Option<&str>,
    ranking_provider_backed: bool,
    ranking_ignored: bool,
    ranked_candidate_ids: &[String],
    ranking_response_digest: Option<&str>,
    candidate_ids: &[String],
    selected_candidate_id: Option<&str>,
    selected_candidate_target: Option<&str>,
    selected_candidate_action_type: Option<&str>,
    selected_candidate_rank: Option<usize>,
    selected_candidate_match_reason: Option<&str>,
) -> bool {
    if scenario
        != main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop
    {
        return true;
    }
    if !model_ranked || ranking_ignored || ranking_source != Some("provider_model") {
        return false;
    }
    if !ranking_provider_backed || ranking_route_type != Some("cloud") {
        return false;
    }
    if normalized_external_provider_label(expected_provider).is_none() {
        return false;
    }
    let Some(ranking_provider) =
        ranking_provider.filter(|provider| normalized_external_provider_label(provider).is_some())
    else {
        return false;
    };
    if ranking_provider != expected_provider {
        return false;
    }
    let Some(ranking_model) =
        ranking_model.filter(|model| live_provider_contract_safe_label(model))
    else {
        return false;
    };
    if !live_provider_contract_safe_label(expected_model) || ranking_model != expected_model {
        return false;
    }
    let selected_candidate_id = match selected_candidate_id {
        Some(candidate_id) if live_provider_contract_safe_label(candidate_id) => candidate_id,
        _ => return false,
    };
    if selected_candidate_target != Some(selected_candidate_id) {
        return false;
    }
    if selected_candidate_action_type != Some("mcp_tool") {
        return false;
    }
    if selected_candidate_match_reason != Some("provider_model_ranked") {
        return false;
    }
    if !ranking_response_digest.is_some_and(metadata_safe_digest_label_present) {
        return false;
    }
    if ranked_candidate_ids.len() < 2 {
        return false;
    }
    let selected_provider_rank = ranked_candidate_ids
        .iter()
        .position(|candidate_id| candidate_id == selected_candidate_id)
        .map(|index| index + 1);
    if selected_provider_rank != selected_candidate_rank {
        return false;
    }
    let ranked_candidate_id_count = ranked_candidate_ids.len();
    let candidate_id_count = candidate_ids.len();
    let ranked_candidate_ids = ranked_candidate_ids
        .iter()
        .filter(|candidate_id| live_provider_contract_safe_label(candidate_id))
        .map(String::as_str)
        .collect::<Vec<_>>();
    let candidate_ids = candidate_ids
        .iter()
        .filter(|candidate_id| live_provider_contract_safe_label(candidate_id))
        .map(String::as_str)
        .collect::<Vec<_>>();
    let ranked_candidate_set = ranked_candidate_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let candidate_set = candidate_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();

    ranked_candidate_ids.len() >= 2
        && ranked_candidate_ids.len() == ranked_candidate_id_count
        && candidate_ids.len() == candidate_id_count
        && ranked_candidate_ids.len() == candidate_ids.len()
        && ranked_candidate_set.len() == ranked_candidate_ids.len()
        && candidate_set.len() == candidate_ids.len()
        && ranked_candidate_set == candidate_set
        && ranked_candidate_ids == candidate_ids
        && ranked_candidate_id_count == candidate_ids.len()
        && ranked_candidate_ids
            .iter()
            .any(|candidate_id| *candidate_id == selected_candidate_id)
}

fn metadata_safe_digest_label_present(digest: &str) -> bool {
    if digest.chars().any(|ch| ch.is_control()) {
        return false;
    }
    let Some((bytes_label, hex_digest)) = digest.split_once(" hash:sha256:") else {
        return false;
    };
    let bytes_label_present = bytes_label
        .strip_prefix("bytes:")
        .and_then(|byte_count| {
            if byte_count.is_empty() || !byte_count.chars().all(|ch| ch.is_ascii_digit()) {
                return None;
            }
            if byte_count.len() > 1 && byte_count.starts_with('0') {
                return None;
            }
            byte_count.parse::<usize>().ok()
        })
        .is_some_and(|byte_count| byte_count > 0);
    bytes_label_present
        && hex_digest.len() == 64
        && hex_digest.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn live_provider_contract_safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAIN_CHAT_LIVE_PROVIDER_CONTRACT_SAFE_LABEL_MAX_LEN
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'))
}

fn normalized_external_provider_label(provider: &str) -> Option<String> {
    if !live_provider_contract_safe_label(provider) {
        return None;
    }
    let provider = provider.to_ascii_lowercase();
    if matches!(
        provider.as_str(),
        "" | "none"
            | "ollama"
            | "local"
            | "localhost"
            | "127.0.0.1"
            | "::1"
            | "0.0.0.0"
            | "local_test_http"
            | "local-test-http"
            | "local_http"
            | "local-http"
            | "mock"
            | "fixture"
            | "synthetic"
            | "scripted"
    ) {
        return None;
    }
    if provider_label_is_local_network_alias(&provider) {
        return None;
    }
    let has_local_token = provider
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| {
            matches!(
                token,
                "local" | "localhost" | "mock" | "fixture" | "synthetic" | "scripted"
            )
        });
    if has_local_token {
        return None;
    }
    if provider_label_has_embedded_synthetic_provider_alias(&provider) {
        return None;
    }
    Some(provider)
}

fn provider_label_has_embedded_synthetic_provider_alias(provider: &str) -> bool {
    [
        "ollama",
        "local",
        "localhost",
        "mock",
        "fixture",
        "synthetic",
        "scripted",
    ]
    .iter()
    .any(|alias| provider.contains(alias))
}

fn provider_label_is_local_network_alias(provider: &str) -> bool {
    let normalized = provider
        .chars()
        .map(|ch| {
            if matches!(ch, '-' | '_' | '/') {
                '.'
            } else {
                ch
            }
        })
        .collect::<String>();
    let parts = normalized.split('.').collect::<Vec<_>>();
    if parts.len() < 4 {
        return false;
    }
    parts.windows(4).any(|octets| {
        if octets
            .iter()
            .any(|octet| octet.is_empty() || !octet.chars().all(|ch| ch.is_ascii_digit()))
        {
            return false;
        }
        let Some(first) = octets.first().and_then(|octet| octet.parse::<u8>().ok()) else {
            return false;
        };
        let Some(second) = octets.get(1).and_then(|octet| octet.parse::<u8>().ok()) else {
            return false;
        };

        first == 0
            || first == 10
            || first == 127
            || (first == 169 && second == 254)
            || (first == 172 && (16..=31).contains(&second))
            || (first == 192 && second == 168)
    }) || provider_label_has_embedded_local_network_alias(provider)
}

fn provider_label_has_embedded_local_network_alias(provider: &str) -> bool {
    let mut octets = Vec::new();
    let mut current = String::new();
    for ch in provider.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(octet) = current.parse::<u16>() {
                octets.push(octet);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(octet) = current.parse::<u16>() {
            octets.push(octet);
        }
    }

    octets.windows(4).any(|window| {
        if window.iter().any(|octet| *octet > 255) {
            return false;
        }
        let first = window[0];
        let second = window[1];

        first == 0
            || first == 10
            || first == 127
            || (first == 169 && second == 254)
            || (first == 172 && (16..=31).contains(&second))
            || (first == 192 && second == 168)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        allowed_action_array_metadata, metadata_safe_digest_label_present,
        normalized_external_provider_label, proposal_permission_target_metadata_trace_present,
        provider_ranked_registered_mcp_selection_metadata_trace_present,
        ranked_manifest_metadata_trace_present, traceable_response_metadata_present,
        web_agent_loop_target_metadata_trace_present,
    };
    use crate::main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario;

    #[test]
    fn main_chat_live_provider_harness_digest_predicate_rejects_noncanonical_labels() {
        let canonical_digest =
            "bytes:12 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000";
        assert!(metadata_safe_digest_label_present(canonical_digest));

        for digest in [
            " bytes:12 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "bytes:12 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000 ",
            "bytes:0 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "bytes:012 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "bytes:12 hash: sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "bytes:12 hash:raw provider ranking response",
        ] {
            assert!(
                !metadata_safe_digest_label_present(digest),
                "live harness digest predicate must reject noncanonical digest label: {digest:?}"
            );
        }
    }

    #[test]
    fn main_chat_live_provider_harness_provider_ranked_trace_rejects_wrapped_model_identity() {
        assert!(provider_ranked_registered_mcp_trace_with_models(
            "gpt-live",
            Some("gpt-live")
        ));

        for (expected_model, ranking_model) in [
            (" gpt-live", Some("gpt-live")),
            ("gpt-live", Some("gpt-live ")),
            ("gpt-live\n", Some("gpt-live")),
        ] {
            assert!(
                !provider_ranked_registered_mcp_trace_with_models(expected_model, ranking_model),
                "provider-ranked harness credit must reject model identities that only match after trimming"
            );
        }
    }

    #[test]
    fn main_chat_live_provider_harness_provider_ranked_trace_requires_raw_exact_provider_identity()
    {
        assert!(provider_ranked_registered_mcp_trace_with_providers(
            "openai",
            Some("openai")
        ));

        for (expected_provider, ranking_provider) in
            [("OpenAI", Some("openai")), ("openai", Some("OpenAI"))]
        {
            assert!(
                !provider_ranked_registered_mcp_trace_with_providers(
                    expected_provider,
                    ranking_provider
                ),
                "provider-ranked harness credit must reject provider identities that only match after case normalization"
            );
        }
    }

    #[test]
    fn main_chat_live_provider_harness_allowed_actions_reject_non_exact_raw_pairs() {
        let metadata = Some(serde_json::json!({
            "toolSelectionAllowedActions": [
                {
                    "actionType": "mcp_tool",
                    "target": "web.search"
                },
                {
                    "actionType": " mcp_tool",
                    "target": "web.fetch"
                },
                {
                    "actionType": "mcp_tool",
                    "target": "web.read ",
                },
                {
                    "actionType": "mcp_tool",
                    "target": "web.extra",
                    "arguments": {}
                }
            ]
        }));

        let allowed_actions =
            allowed_action_array_metadata(&metadata, "toolSelectionAllowedActions");

        assert_eq!(
            allowed_actions,
            vec![serde_json::json!({
                "actionType": "mcp_tool",
                "target": "web.search"
            })],
            "live harness must not trim or rewrite malformed allowed-action metadata before final-gate audit"
        );
    }

    #[test]
    fn main_chat_live_provider_harness_target_traces_reject_non_exact_raw_action_pairs() {
        let malformed_web_allowed_actions = vec![serde_json::json!({
            "actionType": "mcp_tool",
            "target": "web.search",
            "arguments": {}
        })];
        let malformed_mcp_allowed_actions = vec![serde_json::json!({
            "actionType": "mcp_tool",
            "target": "mcp.notes.search",
            "arguments": {}
        })];

        assert!(
            !web_agent_loop_target_metadata_trace_present(
                &["web.search".to_string()],
                &["web.search".to_string()],
                &malformed_web_allowed_actions,
                Some("web.search"),
                Some("web.search"),
                Some("mcp_tool"),
            ),
            "web live target proof must reject allowed-action objects with extra raw fields"
        );
        assert!(
            !proposal_permission_target_metadata_trace_present(
                &["mcp.notes.search".to_string()],
                &["mcp.notes.search".to_string()],
                &malformed_mcp_allowed_actions,
                Some("mcp.notes.search"),
                Some("mcp.notes.search"),
                Some("mcp_tool"),
                Some("mcp.notes.search"),
            ),
            "proposal live target proof must reject allowed-action objects with extra raw fields"
        );
    }

    #[test]
    fn main_chat_live_provider_harness_provider_label_rejects_wrapping_whitespace() {
        assert_eq!(
            normalized_external_provider_label("openai").as_deref(),
            Some("openai")
        );

        for provider in [" openai", "openai ", "\topenai"] {
            assert!(
                normalized_external_provider_label(provider).is_none(),
                "live harness provider identity must reject labels that only become metadata-safe after trimming: {provider:?}"
            );
        }
    }

    #[test]
    fn main_chat_live_provider_harness_ranked_manifest_trace_rejects_wrapping_whitespace_labels() {
        let canonical_digest =
            "bytes:12 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000";
        assert!(ranked_manifest_metadata_trace_present(
            Some(1),
            &["candidate.alpha".to_string()],
            Some("candidate.alpha"),
            Some("planned_action"),
            Some(canonical_digest),
            Some("read"),
            Some("provider_model_ranked"),
        ));
        assert!(
            !ranked_manifest_metadata_trace_present(
                Some(1),
                &["candidate.alpha".to_string()],
                Some("candidate.alpha"),
                Some("planned_action"),
                Some(canonical_digest),
                Some("utility"),
                Some("provider_model_ranked"),
            ),
            "live harness ranked-manifest predicate must prove a discrete read capability label"
        );

        for (source, reason) in [
            (Some(" planned_action"), Some("provider_model_ranked")),
            (Some("planned_action"), Some("provider_model_ranked ")),
        ] {
            assert!(
                !ranked_manifest_metadata_trace_present(
                    Some(1),
                    &["candidate.alpha".to_string()],
                    Some("candidate.alpha"),
                    source,
                    Some(canonical_digest),
                    Some("read"),
                    reason,
                ),
                "live harness ranked-manifest predicate must reject labels that only become metadata-safe after trimming"
            );
        }
    }

    #[test]
    fn main_chat_live_provider_harness_traceable_response_rejects_raw_wrapping_whitespace() {
        assert!(traceable_response_metadata_present(
            Some("run-live-1"),
            Some("task-session-1"),
            Some("Live provider response")
        ));

        for (run_id, task_session_id, response_preview) in [
            (
                Some(" run-live-1"),
                Some("task-session-1"),
                Some("Live provider response"),
            ),
            (
                Some("run-live-1"),
                Some("task-session-1 "),
                Some("Live provider response"),
            ),
            (
                Some("run-live-1\n"),
                Some("task-session-1"),
                Some("Live provider response"),
            ),
            (
                Some("run-live-1"),
                Some("task-session-1"),
                Some(" Live provider response"),
            ),
            (
                Some("run-live-1"),
                Some("task-session-1"),
                Some("Live  provider response"),
            ),
            (
                Some("run-live-1"),
                Some("task-session-1"),
                Some("Live provider response\n"),
            ),
        ] {
            assert!(
                !traceable_response_metadata_present(run_id, task_session_id, response_preview),
                "live harness traceable-response predicate must reject raw trace fields that only become valid after trimming or whitespace normalization"
            );
        }
    }

    fn provider_ranked_registered_mcp_trace_with_models(
        expected_model: &str,
        ranking_model: Option<&str>,
    ) -> bool {
        provider_ranked_registered_mcp_trace(
            expected_model,
            Some("openai"),
            "openai",
            ranking_model,
        )
    }

    fn provider_ranked_registered_mcp_trace_with_providers(
        expected_provider: &str,
        ranking_provider: Option<&str>,
    ) -> bool {
        provider_ranked_registered_mcp_trace(
            "gpt-live",
            ranking_provider,
            expected_provider,
            Some("gpt-live"),
        )
    }

    fn provider_ranked_registered_mcp_trace(
        expected_model: &str,
        ranking_provider: Option<&str>,
        expected_provider: &str,
        ranking_model: Option<&str>,
    ) -> bool {
        let candidate_ids = vec!["mcp.read.alpha".to_string(), "mcp.read.beta".to_string()];
        provider_ranked_registered_mcp_selection_metadata_trace_present(
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
            expected_provider,
            expected_model,
            true,
            Some("provider_model"),
            ranking_provider,
            ranking_model,
            Some("cloud"),
            true,
            false,
            &candidate_ids,
            Some("bytes:12 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000"),
            &candidate_ids,
            Some("mcp.read.alpha"),
            Some("mcp.read.alpha"),
            Some("mcp_tool"),
            Some(1),
            Some("provider_model_ranked"),
        )
    }
}
