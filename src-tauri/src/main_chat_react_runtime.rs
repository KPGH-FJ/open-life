use std::collections::HashMap;
use std::sync::Arc;

use openlife_core::layer::Layer;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::privacy::PrivacyEngine;

use crate::main_chat_generation_support::{
    generate_non_stream_fallback, main_chat_provider_endpoint_kind, preview_text,
};
use crate::main_chat_hs_runtime::build_chat_runtime_hs_packet;
use crate::main_chat_react_tool_selection::{
    build_main_chat_react_agent_loop_messages, main_chat_react_agent_loop_execution_plan,
    rank_main_chat_react_tool_candidates_with_model, MainChatReactActionPlan,
    MainChatReactToolSelectionRanking,
};
use crate::main_chat_runtime_support::append_main_chat_agent_transcript;
use crate::{AppState, ToolCallResult, ToolCallStatus};

pub(crate) struct MainChatObservation {
    pub(crate) summary: String,
    pub(crate) output_preview: String,
    pub(crate) final_answer: String,
    pub(crate) metadata: serde_json::Value,
    pub(crate) executor_status: openlife_core::agent::ActionExecutionStatus,
    pub(crate) blocker_reason: Option<String>,
}

pub(crate) struct MainChatReactFollowUp {
    pub(crate) reply: String,
    pub(crate) model_route: Option<openlife_core::agent::ModelRouteTrace>,
    pub(crate) transcript_entries:
        Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>,
}

pub(crate) struct MainChatReactAgentLoopAttempt {
    pub(crate) reply: Option<String>,
    pub(crate) tool_calls: Vec<ToolCallResult>,
    pub(crate) model_route: Option<openlife_core::agent::ModelRouteTrace>,
    pub(crate) transcript_entries:
        Vec<openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry>,
    pub(crate) metadata: serde_json::Value,
    pub(crate) queue_status: Option<openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus>,
    pub(crate) blocker_reason: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn attach_main_chat_read_observation_metadata(
    metadata: &mut serde_json::Value,
    queue_action_type: &str,
    execution_target: &str,
    arguments: &serde_json::Value,
    output_preview: &str,
    structured_result: Option<serde_json::Value>,
    fixture_backed: bool,
    succeeded: bool,
) {
    let source_kind = match queue_action_type {
        "file.read" => "file",
        "web.search" | "web.fetch" | "web.read" => "web",
        "mcp.read_only" => "mcp",
        "memory.search" => "memory",
        "session.search" => "session",
        _ => "tool",
    };
    let source_label = match queue_action_type {
        "file.read" => arguments
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(execution_target),
        "mcp.read_only" => execution_target,
        _ => execution_target,
    };
    let evidence_kind = match queue_action_type {
        "file.read" => "file_system_read",
        "web.search" if fixture_backed => "web_search_fixture",
        "web.search" => "web_search_network",
        "web.fetch" => "web_fetch_network",
        "web.read" => "governed_read",
        "mcp.read_only" => "registered_mcp_read",
        "memory.search" => "memory_read",
        "session.search" => "session_read",
        _ => "governed_read",
    };
    let network_read_attempted =
        matches!(queue_action_type, "web.search" | "web.fetch") && !fixture_backed;
    let real_read_only_execution = succeeded && !fixture_backed;
    let preview = if output_preview.trim().is_empty() {
        format!("{source_kind} read completed from {source_label}")
    } else {
        preview_text(output_preview, 500)
    };
    let read_evidence = serde_json::json!({
        "kind": evidence_kind,
        "sourceKind": source_kind,
        "sourceLabel": source_label,
        "target": execution_target,
        "realReadOnlyExecution": real_read_only_execution,
        "fixtureBacked": fixture_backed,
        "networkReadAttempted": network_read_attempted,
        "directWritesExecuted": false,
    });

    if let Some(object) = metadata.as_object_mut() {
        object.insert("sourceKind".into(), serde_json::json!(source_kind));
        object.insert("sourceLabel".into(), serde_json::json!(source_label));
        object.insert("preview".into(), serde_json::json!(preview));
        let mut structured = structured_result.unwrap_or_else(|| serde_json::json!({}));
        if let Some(structured_object) = structured.as_object_mut() {
            structured_object.insert("readExecutionEvidence".into(), read_evidence);
            structured_object.insert("directWritesExecuted".into(), serde_json::json!(false));
        } else {
            structured = serde_json::json!({
                "readExecutionEvidence": read_evidence,
                "directWritesExecuted": false,
            });
        }
        object.insert("structuredResult".into(), structured);
    }
}

pub(crate) fn bind_main_chat_observation_metadata_to_queue_action(
    metadata: &mut serde_json::Value,
    queued_action_id: &str,
) {
    if let Some(object) = metadata.as_object_mut() {
        if let Some(executor_action_id) = object.get("actionId").cloned() {
            object.insert("executorActionId".into(), executor_action_id);
        }
        object.insert("actionId".into(), serde_json::json!(queued_action_id));
    }
}

fn attach_tool_selection_ranking_metadata(
    metadata: &mut serde_json::Value,
    ranking: &MainChatReactToolSelectionRanking,
) {
    if let Some(object) = metadata.as_object_mut() {
        object.insert(
            "toolSelectionModelRanked".into(),
            serde_json::json!(ranking.model_ranked),
        );
        object.insert(
            "toolSelectionRankingSource".into(),
            serde_json::json!(ranking.ranking_source),
        );
        object.insert(
            "toolSelectionRankingProvider".into(),
            ranking
                .ranking_provider
                .as_ref()
                .map(|provider| serde_json::Value::String(provider.clone()))
                .unwrap_or(serde_json::Value::Null),
        );
        object.insert(
            "toolSelectionRankingModel".into(),
            ranking
                .ranking_model
                .as_ref()
                .map(|model| serde_json::Value::String(model.clone()))
                .unwrap_or(serde_json::Value::Null),
        );
        object.insert(
            "toolSelectionRankingRouteType".into(),
            ranking
                .ranking_route_type
                .as_ref()
                .map(|route_type| serde_json::Value::String(route_type.clone()))
                .unwrap_or(serde_json::Value::Null),
        );
        object.insert(
            "toolSelectionRankingProviderBacked".into(),
            serde_json::json!(ranking.provider_backed),
        );
        object.insert(
            "toolSelectionModelRankingIgnored".into(),
            serde_json::json!(ranking.ignored),
        );
        object.insert(
            "toolSelectionModelRankingCandidateIds".into(),
            serde_json::json!(ranking.ranked_candidate_ids),
        );
        object.insert(
            "toolSelectionModelRankingResponseDigest".into(),
            ranking
                .model_response_digest
                .as_ref()
                .map(|digest| serde_json::Value::String(digest.clone()))
                .unwrap_or(serde_json::Value::Null),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn try_run_main_chat_react_agent_loop(
    state: &Arc<AppState>,
    task_session_id: &str,
    session_id: &str,
    user_text: &str,
    messages_for_generation: &[ChatMessage],
    life_model: &LifeModel,
    privacy_engine: &PrivacyEngine,
    privacy_map: &HashMap<String, String>,
    plan: &MainChatReactActionPlan,
    local_only_required: bool,
) -> Result<MainChatReactAgentLoopAttempt, String> {
    use openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind;

    let allow_cloud = !local_only_required;
    let scheduler = state.scheduler.lock().await.clone();
    let scripted_provider_response = scheduler.scripted_generation_response.is_some();
    let provider_endpoint_kind =
        main_chat_provider_endpoint_kind(&scheduler, scripted_provider_response);
    let live_provider_invoked =
        allow_cloud && !scripted_provider_response && provider_endpoint_kind == "external_provider";
    let deterministic_agent_loop_plan = {
        let registry = state.mcp_registry.lock().await;
        main_chat_react_agent_loop_execution_plan(&registry, plan)
    };
    let (agent_loop_plan, tool_selection_ranking) =
        rank_main_chat_react_tool_candidates_with_model(
            &scheduler,
            life_model,
            messages_for_generation,
            deterministic_agent_loop_plan,
            allow_cloud,
        )
        .await;
    let tool_selection_candidate_ids = agent_loop_plan.tool_candidate_ids();
    let tool_selection_allowlist = agent_loop_plan.allowed_tool_targets();
    let tool_selection_allowed_actions = agent_loop_plan.allowed_tool_action_metadata();
    let tool_selection_contract_digest =
        openlife_core::agent::react_beta::metadata_safe_value_digest(&serde_json::json!({
            "candidateIds": tool_selection_candidate_ids.clone(),
            "allowedTargets": tool_selection_allowlist.clone(),
            "allowedActions": tool_selection_allowed_actions.clone(),
            "allowWrites": false,
        }));
    let mut plan_metadata = serde_json::json!({
        "agentLoopAttempted": true,
        "structuredBlockerOnFailure": true,
        "allowWrites": false,
        "allowCloud": allow_cloud,
        "localOnlyRequired": local_only_required,
        "plannedActionType": plan.queue_action_type.clone(),
        "plannedTarget": plan.target.clone(),
        "argumentsDigest": openlife_core::agent::react_beta::metadata_safe_value_digest(&plan.arguments),
        "toolSelectionCandidateCount": agent_loop_plan.tool_candidate_count(),
        "toolSelectionCandidateIds": agent_loop_plan.tool_candidate_ids(),
        "toolSelectionAllowlist": agent_loop_plan.allowed_tool_targets(),
        "toolSelectionAllowedActions": agent_loop_plan.allowed_tool_action_metadata(),
        "toolSelectionContractDigest": tool_selection_contract_digest,
        "toolExecutionAllowed": true,
        "writeExecutionAllowed": false,
        "directWritesExecuted": false,
        "providerEndpointKind": provider_endpoint_kind,
        "scriptedProviderResponse": scripted_provider_response,
        "externalLiveProviderEvalPreflighted": false,
    });
    attach_tool_selection_ranking_metadata(&mut plan_metadata, &tool_selection_ranking);
    let mut transcript_entries = append_main_chat_agent_transcript(
        state,
        Some(task_session_id),
        ExecutionTranscriptEntryKind::Plan,
        "Governed ReAct AgentLoop attempt started.",
        plan_metadata,
    )
    .await;

    let failed_attempt = |metadata: serde_json::Value,
                          transcript_entries: Vec<
        openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntry,
    >| MainChatReactAgentLoopAttempt {
        reply: None,
        tool_calls: Vec::new(),
        model_route: None,
        transcript_entries,
        metadata,
        queue_status: None,
        blocker_reason: None,
    };

    let tools_prompt = {
        let registry = state.mcp_registry.lock().await;
        registry.tools_prompt()
    };
    let web_search_fixture_output = state.web_search_fixture_output.lock().await.clone();
    let (safe_paths, calendar_ics_paths, network_policy, agent_runtime, loop_config) = {
        let cfg = state.config.lock().await;
        let mut safe_paths = cfg.system.safe_paths.clone();
        if let Ok(workspace) =
            crate::workspace_file_resolver::resolve_workspace_root().or_else(|_| {
                std::env::current_dir()
                    .and_then(|dir| dir.canonicalize())
                    .map_err(|err| err.to_string())
            })
        {
            let workspace = workspace.to_string_lossy().to_string();
            if !safe_paths.iter().any(|path| path == &workspace) {
                safe_paths.push(workspace);
            }
        }
        (
            safe_paths,
            cfg.system.calendar_ics_paths.clone(),
            cfg.system.network_policy.clone(),
            openlife_core::agent::AgentRuntime::new(life_model.clone(), scheduler.clone(), &cfg),
            openlife_core::agent::AgentLoopConfig {
                max_steps: cfg.system.agent_loop_max_steps,
                max_tool_calls: cfg.system.agent_loop_max_tool_calls,
                timeout_seconds: cfg.system.agent_loop_timeout_seconds,
                allow_writes: false,
                allow_cloud,
                shutdown_notify: Some(state.shutdown_notify.clone()),
                toolset_allowlist: agent_loop_plan.allowed_tool_targets(),
                tool_action_allowlist: agent_loop_plan.allowed_tool_actions(),
                ..Default::default()
            },
        )
    };
    if plan.requires_network && !network_policy.enabled {
        let selected_tool_candidate = agent_loop_plan.default_tool_candidate();
        let selected_arguments_digest =
            openlife_core::agent::react_beta::metadata_safe_value_digest(
                &selected_tool_candidate.arguments,
            );
        let selected_arguments_digest_label = format!(
            "bytes:{} hash:{}",
            selected_arguments_digest.0, selected_arguments_digest.1
        );
        let selected_capabilities_digest_label =
            selected_tool_candidate.capabilities_digest_label();
        let selected_capability_labels_label = selected_tool_candidate.capability_labels_label();
        let selected_manifest_source_label = selected_tool_candidate.manifest_source_label();
        let selected_match_reason_label = selected_tool_candidate.match_reason_label();
        let mut metadata = serde_json::json!({
            "agentLoopAttempted": true,
            "agentLoopSucceeded": true,
            "singleStepFallbackUsed": false,
            "plannedActionObserved": true,
            "modelSelectedAllowedTool": true,
            "modelSelectedExecutionPolicyValidated": true,
            "modelSelectedExecutionAllowed": true,
            "modelSelectedExecutionPolicyLevel": "read",
            "modelSelectedExecutionPolicyReasonCode": "network_policy_checked_read",
            "modelSelectedRequiresProposal": false,
            "modelSelectedRequiresConfirmation": false,
            "modelSelectedSilentWriteAllowed": false,
            "modelSelectedArgumentsSource": "governed_candidate_contract",
            "modelSelectedGovernedArgumentsDigest": selected_arguments_digest_label,
            "toolSelectionCandidateId": selected_tool_candidate.candidate_id.clone(),
            "toolSelectionCandidateTarget": selected_tool_candidate.target.clone(),
            "toolSelectionCandidateActionType": selected_tool_candidate.executor_action_type.clone(),
            "toolSelectionCandidateRank": selected_tool_candidate.selection_rank,
            "toolSelectionCandidateSource": selected_manifest_source_label,
            "toolSelectionCandidateCapabilitiesDigest": selected_capabilities_digest_label,
            "toolSelectionCandidateCapabilityLabels": selected_capability_labels_label,
            "toolSelectionCandidateMatchReason": selected_match_reason_label,
            "toolSelectionCandidateCount": agent_loop_plan.tool_candidate_count(),
            "toolSelectionCandidateIds": agent_loop_plan.tool_candidate_ids(),
            "toolSelectionAllowlist": agent_loop_plan.allowed_tool_targets(),
            "toolSelectionAllowedActions": agent_loop_plan.allowed_tool_action_metadata(),
            "plannedTarget": plan.target.clone(),
            "executionTarget": selected_tool_candidate.target.clone(),
            "agentLoopActionStatus": "blocked",
            "observedActionStatus": "blocked",
            "permissionDecision": "network_policy_blocked",
            "blockerReason": "network_policy_blocked",
            "toolCallCount": 1,
            "stepCount": 1,
            "stopReason": "network_policy_blocked",
            "statusUpdateCount": 0,
            "directWritesExecuted": false,
            "providerEndpointKind": provider_endpoint_kind,
            "scriptedProviderResponse": scripted_provider_response,
            "liveProviderInvoked": live_provider_invoked,
            "externalLiveProviderEvalPreflighted": false,
        });
        attach_tool_selection_ranking_metadata(&mut metadata, &tool_selection_ranking);
        attach_main_chat_read_observation_metadata(
            &mut metadata,
            &agent_loop_plan.queue_action_type,
            &selected_tool_candidate.target,
            &selected_tool_candidate.arguments,
            "network_policy_blocked",
            Some(serde_json::json!({
                "status": "blocked",
                "permission_decision": "network_policy_blocked",
                "network_policy_blocked": true,
                "directWritesExecuted": false,
            })),
            false,
            false,
        );
        transcript_entries.extend(
            append_main_chat_agent_transcript(
                state,
                Some(task_session_id),
                ExecutionTranscriptEntryKind::Observation,
                "Governed ReAct AgentLoop completed with a network policy blocker.",
                metadata.clone(),
            )
            .await,
        );
        return Ok(MainChatReactAgentLoopAttempt {
            reply: Some("That web read is blocked by governance: network_policy_blocked".into()),
            tool_calls: vec![tool_call_from_action(
                &selected_tool_candidate.target,
                "network_policy_blocked",
                false,
                None,
                Some("network_policy_blocked".into()),
                ToolCallStatus::Blocked,
                false,
            )],
            model_route: Some(scheduler.preview_chat_route(Some(&tools_prompt)).await),
            transcript_entries,
            metadata,
            queue_status: Some(
                openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed,
            ),
            blocker_reason: Some("network_policy_blocked".into()),
        });
    }
    let action_executor =
        openlife_core::agent::ActionExecutor::new(openlife_core::agent::ActionExecutorConfig {
            allow_writes: false,
            allow_cloud,
            ..Default::default()
        });
    let agent_loop = openlife_core::agent::AgentLoop::new(
        agent_runtime,
        action_executor,
        scheduler.clone(),
        loop_config,
    );
    let agent_loop_messages =
        build_main_chat_react_agent_loop_messages(messages_for_generation, &agent_loop_plan);
    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.to_string(),
        user_text: user_text.to_string(),
        messages: agent_loop_messages,
        layer: Layer::L2,
    };
    let hs_packet = match build_chat_runtime_hs_packet(
        state,
        &task,
        life_model,
        &tools_prompt,
        None,
    )
    .await
    {
        Ok(packet) => packet,
        Err(err) => {
            let model_error_digest = openlife_core::agent::react_beta::metadata_safe_value_digest(
                &serde_json::json!({ "error": err.to_string() }),
            );
            let metadata = serde_json::json!({
                "agentLoopAttempted": true,
                "agentLoopSucceeded": false,
                "singleStepFallbackUsed": false,
                "modelErrorDigest": model_error_digest,
                "directWritesExecuted": false,
            });
            transcript_entries.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Error,
                    "Governed ReAct AgentLoop failed before execution; returning a structured blocker.",
                    metadata.clone(),
                )
                .await,
            );
            return Ok(failed_attempt(metadata, transcript_entries));
        }
    };

    let local_permission_store = if plan.uses_ephemeral_file_permission
        || plan.uses_ephemeral_mcp_wrapper_permission
    {
        match openlife_core::tool_permissions::ToolPermissionStore::new_in_memory() {
            Ok(store) => {
                if plan.uses_ephemeral_file_permission {
                    if let Err(err) = store.grant(
                        "file.read",
                        "builtin",
                        "low",
                        "read",
                        openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                        None,
                    ) {
                        let model_error_digest =
                            openlife_core::agent::react_beta::metadata_safe_value_digest(
                                &serde_json::json!({ "error": err.to_string() }),
                            );
                        let metadata = serde_json::json!({
                            "agentLoopAttempted": true,
                            "agentLoopSucceeded": false,
                            "singleStepFallbackUsed": false,
                            "modelErrorDigest": model_error_digest,
                            "directWritesExecuted": false,
                        });
                        transcript_entries.extend(
                            append_main_chat_agent_transcript(
                                state,
                                Some(task_session_id),
                                ExecutionTranscriptEntryKind::Error,
                                "Governed ReAct AgentLoop could not prepare file permission; returning a structured blocker.",
                                metadata.clone(),
                            )
                            .await,
                        );
                        return Ok(failed_attempt(metadata, transcript_entries));
                    }
                }
                if plan.uses_ephemeral_mcp_wrapper_permission {
                    if let Err(err) = store.grant(
                        "mcp.call_tool",
                        "builtin",
                        "medium",
                        "external_side_effect",
                        openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
                        None,
                    ) {
                        let model_error_digest =
                            openlife_core::agent::react_beta::metadata_safe_value_digest(
                                &serde_json::json!({ "error": err.to_string() }),
                            );
                        let metadata = serde_json::json!({
                            "agentLoopAttempted": true,
                            "agentLoopSucceeded": false,
                            "singleStepFallbackUsed": false,
                            "modelErrorDigest": model_error_digest,
                            "directWritesExecuted": false,
                        });
                        transcript_entries.extend(
                            append_main_chat_agent_transcript(
                                state,
                                Some(task_session_id),
                                ExecutionTranscriptEntryKind::Error,
                                "Governed ReAct AgentLoop could not prepare MCP wrapper permission; returning a structured blocker.",
                                metadata.clone(),
                            )
                            .await,
                        );
                        return Ok(failed_attempt(metadata, transcript_entries));
                    }
                }
                Some(store)
            }
            Err(err) => {
                let model_error_digest =
                    openlife_core::agent::react_beta::metadata_safe_value_digest(
                        &serde_json::json!({ "error": err.to_string() }),
                    );
                let metadata = serde_json::json!({
                    "agentLoopAttempted": true,
                    "agentLoopSucceeded": false,
                    "singleStepFallbackUsed": false,
                    "modelErrorDigest": model_error_digest,
                    "directWritesExecuted": false,
                });
                transcript_entries.extend(
                    append_main_chat_agent_transcript(
                        state,
                        Some(task_session_id),
                        ExecutionTranscriptEntryKind::Error,
                        "Governed ReAct AgentLoop could not create file permission context; returning a structured blocker.",
                        metadata.clone(),
                    )
                    .await,
                );
                return Ok(failed_attempt(metadata, transcript_entries));
            }
        }
    } else {
        None
    };
    let permission_store_guard = if local_permission_store.is_none() {
        Some(state.tool_permission_store.lock().await)
    } else {
        None
    };
    let permission_store_ref = match (&local_permission_store, &permission_store_guard) {
        (Some(store), _) => store,
        (None, Some(store)) => &**store,
        _ => return Err("tool permission store unavailable".into()),
    };

    let loop_result = {
        let (registry, audit_store) = state.get_mcp_state().await;
        let memory_store = state.memory_store.lock().await;
        let agent_run_store_guard = if let Some(ref store_arc) = state.agent_run_store {
            Some(store_arc.lock().await)
        } else {
            None
        };
        let mut action_ctx = openlife_core::agent::ActionExecutionContext::new(
            &registry,
            permission_store_ref,
            &audit_store,
            privacy_engine,
            &safe_paths,
        )
        .with_life_model(life_model)
        .with_memory_store(&memory_store)
        .with_calendar_ics_paths(&calendar_ics_paths)
        .with_network_policy(&network_policy);
        if let Some(ref agent_run_store) = agent_run_store_guard {
            action_ctx = action_ctx.with_agent_run_store(agent_run_store);
        }
        if let Some(ref packet) = hs_packet {
            action_ctx = action_ctx.with_hs_runtime_packet(packet);
        }
        if let Some(ref fixture_output) = web_search_fixture_output {
            action_ctx = action_ctx.with_web_search_fixture_output(fixture_output);
        }

        agent_loop
            .run(
                &task,
                life_model,
                &tools_prompt,
                None,
                privacy_engine.clone(),
                &action_ctx,
            )
            .await
    };

    match loop_result {
        Ok(result) => {
            if result.stop_reason == "tool_allowlist_blocked" {
                let mut metadata = serde_json::json!({
                    "agentLoopAttempted": true,
                    "agentLoopSucceeded": false,
                    "singleStepFallbackUsed": false,
                    "plannedActionObserved": false,
                    "modelSelectedAllowedTool": false,
                    "toolSelectionCandidateCount": agent_loop_plan.tool_candidate_count(),
                    "toolSelectionCandidateIds": agent_loop_plan.tool_candidate_ids(),
                    "toolSelectionAllowlist": agent_loop_plan.allowed_tool_targets(),
                    "toolSelectionAllowedActions": agent_loop_plan.allowed_tool_action_metadata(),
                    "agentLoopActionStatus": "blocked",
                    "blockerReason": "model_selected_disallowed_tool",
                    "toolCallCount": result.tool_call_count,
                    "stepCount": result.step_count,
                    "stopReason": result.stop_reason.clone(),
                    "directWritesExecuted": false,
                    "providerEndpointKind": provider_endpoint_kind,
                    "scriptedProviderResponse": scripted_provider_response,
                    "liveProviderInvoked": live_provider_invoked,
                    "externalLiveProviderEvalPreflighted": false,
                });
                attach_tool_selection_ranking_metadata(&mut metadata, &tool_selection_ranking);
                transcript_entries.extend(
                    append_main_chat_agent_transcript(
                        state,
                        Some(task_session_id),
                        ExecutionTranscriptEntryKind::Error,
                        "Governed ReAct AgentLoop blocked a disallowed model-selected tool.",
                        metadata.clone(),
                    )
                    .await,
                );
                return Ok(MainChatReactAgentLoopAttempt {
                    reply: Some(
                        "That tool call is blocked by governance: model_selected_disallowed_tool"
                            .into(),
                    ),
                    tool_calls: Vec::new(),
                    model_route: Some(scheduler.preview_chat_route(Some(&tools_prompt)).await),
                    transcript_entries,
                    metadata,
                    queue_status: Some(
                        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed,
                    ),
                    blocker_reason: Some("model_selected_disallowed_tool".into()),
                });
            }
            let observed_action = result.run.actions.iter().find(|action| {
                agent_loop_plan
                    .tool_candidate_for_action(&action.action_type, action.target.as_deref())
                    .is_some()
            });
            let planned_action_observed = observed_action.is_some();
            if !planned_action_observed {
                let mut metadata = serde_json::json!({
                    "agentLoopAttempted": true,
                    "agentLoopSucceeded": false,
                    "singleStepFallbackUsed": false,
                    "plannedActionObserved": false,
                    "modelSelectedAllowedTool": false,
                    "toolSelectionCandidateCount": agent_loop_plan.tool_candidate_count(),
                    "toolSelectionCandidateIds": agent_loop_plan.tool_candidate_ids(),
                    "toolSelectionAllowlist": agent_loop_plan.allowed_tool_targets(),
                    "toolSelectionAllowedActions": agent_loop_plan.allowed_tool_action_metadata(),
                    "toolCallCount": result.tool_call_count,
                    "stepCount": result.step_count,
                    "stopReason": result.stop_reason.clone(),
                    "directWritesExecuted": false,
                });
                attach_tool_selection_ranking_metadata(&mut metadata, &tool_selection_ranking);
                transcript_entries.extend(
                    append_main_chat_agent_transcript(
                        state,
                        Some(task_session_id),
                        ExecutionTranscriptEntryKind::Error,
                        "Governed ReAct AgentLoop did not observe the planned action; returning a structured blocker.",
                        metadata.clone(),
                    )
                    .await,
                );
                return Ok(failed_attempt(metadata, transcript_entries));
            }
            let observed_action = observed_action.expect("planned action observed above");
            let selected_tool_candidate = agent_loop_plan
                .tool_candidate_for_action(
                    &observed_action.action_type,
                    observed_action.target.as_deref(),
                )
                .unwrap_or_else(|| agent_loop_plan.default_tool_candidate());
            let selected_execution_policy =
                openlife_core::agent::main_chat_agent_v1::ExecutionPolicy.classify(
                    &openlife_core::agent::main_chat_agent_v1::ExecutionAction::new(
                        agent_loop_plan.queue_action_type.clone(),
                        format!(
                            "Model selected governed ReAct candidate {} for {}",
                            selected_tool_candidate.candidate_id, agent_loop_plan.description
                        ),
                    ),
                );
            let observed_action_status = observed_action.status.clone();
            let observed_action_error = observed_action.error.clone();
            let observed_action_id = Some(observed_action.id.clone());
            let selected_arguments_digest =
                openlife_core::agent::react_beta::metadata_safe_value_digest(
                    &selected_tool_candidate.arguments,
                );
            let selected_arguments_digest_label = format!(
                "bytes:{} hash:{}",
                selected_arguments_digest.0, selected_arguments_digest.1
            );
            let selected_capabilities_digest_label =
                selected_tool_candidate.capabilities_digest_label();
            let selected_capability_labels_label =
                selected_tool_candidate.capability_labels_label();
            let selected_manifest_source_label = selected_tool_candidate.manifest_source_label();
            let selected_match_reason_label = selected_tool_candidate.match_reason_label();
            if !selected_execution_policy.execution_allowed {
                let mut metadata = serde_json::json!({
                    "agentLoopAttempted": true,
                    "agentLoopSucceeded": false,
                    "singleStepFallbackUsed": false,
                    "plannedActionObserved": true,
                    "modelSelectedAllowedTool": true,
                    "modelSelectedExecutionPolicyValidated": true,
                    "modelSelectedExecutionAllowed": selected_execution_policy.execution_allowed,
                    "modelSelectedExecutionPolicyLevel": selected_execution_policy.level.as_str(),
                    "modelSelectedExecutionPolicyReasonCode": selected_execution_policy.reason_code.clone(),
                    "modelSelectedRequiresProposal": selected_execution_policy.requires_proposal,
                    "modelSelectedRequiresConfirmation": selected_execution_policy.requires_confirmation,
                    "modelSelectedSilentWriteAllowed": selected_execution_policy.silent_write_allowed,
                    "modelSelectedArgumentsSource": "governed_candidate_contract",
                    "modelSelectedGovernedArgumentsDigest": selected_arguments_digest_label,
                    "toolSelectionCandidateId": selected_tool_candidate.candidate_id.clone(),
                    "toolSelectionCandidateTarget": selected_tool_candidate.target.clone(),
                    "toolSelectionCandidateActionType": selected_tool_candidate.executor_action_type.clone(),
                    "toolSelectionCandidateRank": selected_tool_candidate.selection_rank,
                    "toolSelectionCandidateSource": selected_manifest_source_label,
                    "toolSelectionCandidateCapabilitiesDigest": selected_capabilities_digest_label,
                    "toolSelectionCandidateCapabilityLabels": selected_capability_labels_label,
                    "toolSelectionCandidateMatchReason": selected_match_reason_label,
                    "toolSelectionCandidateCount": agent_loop_plan.tool_candidate_count(),
                    "toolSelectionCandidateIds": agent_loop_plan.tool_candidate_ids(),
                    "toolSelectionAllowlist": agent_loop_plan.allowed_tool_targets(),
                    "toolSelectionAllowedActions": agent_loop_plan.allowed_tool_action_metadata(),
                    "agentLoopActionStatus": "blocked",
                    "observedActionStatus": observed_action_status,
                    "blockerReason": "model_selected_tool_policy_blocked",
                    "toolCallCount": result.tool_call_count,
                    "stepCount": result.step_count,
                    "stopReason": result.stop_reason.clone(),
                    "directWritesExecuted": false,
                    "providerEndpointKind": provider_endpoint_kind,
                    "scriptedProviderResponse": scripted_provider_response,
                    "liveProviderInvoked": live_provider_invoked,
                    "externalLiveProviderEvalPreflighted": false,
                });
                attach_tool_selection_ranking_metadata(&mut metadata, &tool_selection_ranking);
                transcript_entries.extend(
                    append_main_chat_agent_transcript(
                        state,
                        Some(task_session_id),
                        ExecutionTranscriptEntryKind::Error,
                        "Governed ReAct AgentLoop blocked the model-selected tool by ExecutionPolicy.",
                        metadata.clone(),
                    )
                    .await,
                );
                return Ok(MainChatReactAgentLoopAttempt {
                    reply: Some(
                        "That tool call is blocked by governance: model_selected_tool_policy_blocked"
                            .into(),
                    ),
                    tool_calls: Vec::new(),
                    model_route: Some(scheduler.preview_chat_route(Some(&tools_prompt)).await),
                    transcript_entries,
                    metadata,
                    queue_status: Some(
                        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed,
                    ),
                    blocker_reason: Some("model_selected_tool_policy_blocked".into()),
                });
            }
            let observed_observation = result
                .run
                .observations
                .iter()
                .find(|observation| {
                    observation.action_id.as_deref() == observed_action_id.as_deref()
                })
                .or_else(|| result.run.observations.first());
            let observed_permission_decision =
                observed_action.permission_decision.clone().or_else(|| {
                    observed_observation
                        .and_then(|observation| observation.structured_result.as_ref())
                        .and_then(|structured| structured.get("permission_decision"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                });
            let agent_loop_queue_status = match observed_action_status.as_str() {
                "succeeded" => openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed,
                "needs_confirmation" => {
                    openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::PendingPermission
                }
                "blocked" | "failed" => {
                    openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed
                }
                _ => openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Failed,
            };
            let agent_loop_blocker_reason = if observed_action_status == "succeeded" {
                None
            } else {
                observed_permission_decision
                    .clone()
                    .or(observed_action_error.clone())
                    .or_else(|| Some(result.stop_reason.clone()))
                    .or_else(|| Some(observed_action_status.clone()))
            };
            let observed_tool_scope = observed_action.tool_scope.as_ref();
            let mcp_read_target_resolved = plan.target == "mcp.call_tool"
                && observed_tool_scope
                    .map(|scope| {
                        scope.tool_name == selected_tool_candidate.target.as_str()
                            && scope.action_type.eq_ignore_ascii_case("read")
                    })
                    .unwrap_or(false);
            let resolved_target = observed_tool_scope
                .map(|scope| scope.tool_name.clone())
                .filter(|target| !target.trim().is_empty());
            let resolved_action_type = observed_tool_scope
                .map(|scope| scope.action_type.clone())
                .filter(|action_type| !action_type.trim().is_empty());
            let mut metadata = serde_json::json!({
                "agentLoopAttempted": true,
                "agentLoopSucceeded": true,
                "singleStepFallbackUsed": false,
                "plannedActionObserved": true,
                "modelSelectedAllowedTool": true,
                "modelSelectedExecutionPolicyValidated": true,
                "modelSelectedExecutionAllowed": selected_execution_policy.execution_allowed,
                "modelSelectedExecutionPolicyLevel": selected_execution_policy.level.as_str(),
                "modelSelectedExecutionPolicyReasonCode": selected_execution_policy.reason_code.clone(),
                "modelSelectedRequiresProposal": selected_execution_policy.requires_proposal,
                "modelSelectedRequiresConfirmation": selected_execution_policy.requires_confirmation,
                "modelSelectedSilentWriteAllowed": selected_execution_policy.silent_write_allowed,
                "modelSelectedArgumentsSource": "governed_candidate_contract",
                "modelSelectedGovernedArgumentsDigest": selected_arguments_digest_label,
                "toolSelectionCandidateId": selected_tool_candidate.candidate_id.clone(),
                "toolSelectionCandidateTarget": selected_tool_candidate.target.clone(),
                "toolSelectionCandidateActionType": selected_tool_candidate.executor_action_type.clone(),
                "toolSelectionCandidateRank": selected_tool_candidate.selection_rank,
                "toolSelectionCandidateSource": selected_manifest_source_label,
                "toolSelectionCandidateCapabilitiesDigest": selected_capabilities_digest_label,
                "toolSelectionCandidateCapabilityLabels": selected_capability_labels_label,
                "toolSelectionCandidateMatchReason": selected_match_reason_label,
                "toolSelectionCandidateCount": agent_loop_plan.tool_candidate_count(),
                "toolSelectionCandidateIds": agent_loop_plan.tool_candidate_ids(),
                "toolSelectionAllowlist": agent_loop_plan.allowed_tool_targets(),
                "toolSelectionAllowedActions": agent_loop_plan.allowed_tool_action_metadata(),
                "plannedTarget": plan.target.clone(),
                "executionTarget": selected_tool_candidate.target.clone(),
                "agentLoopActionStatus": observed_action_status.clone(),
                "permissionDecision": observed_permission_decision.clone(),
                "blockerReason": agent_loop_blocker_reason.clone(),
                "toolCallCount": result.tool_call_count,
                "stepCount": result.step_count,
                "stopReason": result.stop_reason.clone(),
                "statusUpdateCount": result.status_updates.len(),
                "directWritesExecuted": false,
                "providerEndpointKind": provider_endpoint_kind,
                "scriptedProviderResponse": scripted_provider_response,
                "liveProviderInvoked": live_provider_invoked,
                "externalLiveProviderEvalPreflighted": false,
            });
            if let Some(object) = metadata.as_object_mut() {
                object.insert(
                    "agentLoopActionCount".into(),
                    serde_json::json!(result.run.actions.len()),
                );
                object.insert(
                    "agentLoopObservationCount".into(),
                    serde_json::json!(result.run.observations.len()),
                );
            }
            attach_tool_selection_ranking_metadata(&mut metadata, &tool_selection_ranking);
            if plan.target == "mcp.call_tool" {
                if let Some(object) = metadata.as_object_mut() {
                    object.insert(
                        "mcpReadTargetResolved".into(),
                        serde_json::json!(mcp_read_target_resolved),
                    );
                    object.insert(
                        "requestedTarget".into(),
                        serde_json::json!(plan.target.clone()),
                    );
                    object.insert(
                        "resolvedTarget".into(),
                        resolved_target
                            .map(serde_json::Value::String)
                            .unwrap_or(serde_json::Value::Null),
                    );
                    object.insert(
                        "resolvedActionType".into(),
                        resolved_action_type
                            .map(serde_json::Value::String)
                            .unwrap_or(serde_json::Value::Null),
                    );
                }
            }
            let observed_output_preview = observed_observation
                .map(|observation| observation.content.as_str())
                .unwrap_or(&result.final_response);
            let observed_structured_result =
                observed_observation.and_then(|observation| observation.structured_result.clone());
            attach_main_chat_read_observation_metadata(
                &mut metadata,
                &agent_loop_plan.queue_action_type,
                &selected_tool_candidate.target,
                &selected_tool_candidate.arguments,
                observed_output_preview,
                observed_structured_result,
                web_search_fixture_output.is_some()
                    && agent_loop_plan.queue_action_type == "web.search",
                observed_action_status == "succeeded",
            );
            transcript_entries.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::FollowUp,
                    "Governed ReAct AgentLoop completed.",
                    metadata.clone(),
                )
                .await,
            );
            let model_route = Some(scheduler.preview_chat_route(Some(&tools_prompt)).await);
            let reply = if observed_action_status == "succeeded" {
                privacy_engine.reconstruct(&result.final_response, privacy_map)
            } else {
                format!(
                    "That read action is blocked by governance: {}",
                    agent_loop_blocker_reason
                        .clone()
                        .unwrap_or_else(|| observed_action_status.clone())
                )
            };
            let tool_calls =
                agent_actions_to_tool_call_results(&result.run.actions, &result.run.id);
            Ok(MainChatReactAgentLoopAttempt {
                reply: Some(reply),
                tool_calls,
                model_route,
                transcript_entries,
                metadata,
                queue_status: Some(agent_loop_queue_status),
                blocker_reason: agent_loop_blocker_reason,
            })
        }
        Err(err) => {
            let model_error_digest = openlife_core::agent::react_beta::metadata_safe_value_digest(
                &serde_json::json!({ "error": err.to_string() }),
            );
            let metadata = serde_json::json!({
                "agentLoopAttempted": true,
                "agentLoopSucceeded": false,
                "singleStepFallbackUsed": false,
                "modelErrorDigest": model_error_digest,
                "directWritesExecuted": false,
            });
            transcript_entries.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Error,
                    "Governed ReAct AgentLoop failed; returning a structured blocker.",
                    metadata.clone(),
                )
                .await,
            );
            Ok(failed_attempt(metadata, transcript_entries))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn synthesize_main_chat_react_follow_up(
    state: &Arc<AppState>,
    task_session_id: &str,
    session_id: &str,
    user_text: &str,
    messages_for_generation: &[ChatMessage],
    life_model: &LifeModel,
    privacy_engine: &PrivacyEngine,
    privacy_map: &HashMap<String, String>,
    observation: &MainChatObservation,
) -> Result<MainChatReactFollowUp, String> {
    use openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind;

    let mut transcript_entries = append_main_chat_agent_transcript(
        state,
        Some(task_session_id),
        ExecutionTranscriptEntryKind::FollowUp,
        "ReAct follow-up synthesis started after a governed observation.",
        serde_json::json!({
            "actionExecutorBacked": true,
            "observationDigest": openlife_core::agent::react_beta::metadata_safe_value_digest(&observation.metadata),
            "toolExecutionAllowed": false,
            "writeExecutionAllowed": false,
            "directWritesExecuted": false,
        }),
    )
    .await;

    let observation_preview = preview_text(&observation.output_preview, 1500);
    let mut follow_up_messages = messages_for_generation.to_vec();
    follow_up_messages.push(ChatMessage {
        role: "system".into(),
        content: "You are synthesizing the final answer for OpenLife Main Chat Agent v1 after a governed read-only ReAct observation. Do not call tools. Do not write memory, LifeModel, files, email, calendar, providers, or plugins. Use only the observation and the user's request. If the observation is insufficient, say what is missing and keep the answer concise.".into(),
    });
    follow_up_messages.push(ChatMessage {
        role: "user".into(),
        content: format!(
            "User request:\n{}\n\nGoverned observation:\n{}\n\nReturn the final answer now.",
            user_text, observation_preview
        ),
    });

    let scheduler = state.scheduler.lock().await.clone();
    let task = openlife_core::agent::AgentTask {
        kind: openlife_core::agent::AgentTaskKind::Conversation,
        session_id: session_id.to_string(),
        user_text: user_text.to_string(),
        messages: follow_up_messages.clone(),
        layer: Layer::L2,
    };
    let hs_packet = build_chat_runtime_hs_packet(state, &task, life_model, "", None).await?;

    let (reply, model_generated, model_error_digest) = match generate_non_stream_fallback(
        &scheduler,
        follow_up_messages,
        life_model,
        "",
        hs_packet.clone(),
    )
    .await
    {
        Ok(generated) => (
            privacy_engine.reconstruct(&generated, privacy_map),
            true,
            None,
        ),
        Err(err) => {
            let err_digest = openlife_core::agent::react_beta::metadata_safe_value_digest(
                &serde_json::json!({ "error": err.to_string() }),
            );
            (
                format!(
                    "I completed the governed read action and synthesized the available observation without executing any write.\n\n{}",
                    preview_text(&observation.output_preview, 900)
                ),
                false,
                Some(err_digest),
            )
        }
    };
    let model_route = if model_generated {
        Some(scheduler.preview_chat_route(None).await)
    } else {
        None
    };

    transcript_entries.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::FollowUp,
            "ReAct follow-up synthesis completed.",
            serde_json::json!({
                "modelGenerated": model_generated,
                "modelErrorDigest": model_error_digest,
                "hsPacketSelected": hs_packet.is_some(),
                "toolCallCount": 0,
                "directWritesExecuted": false,
                "legacyFallbackUsed": false,
                "failSoftFallbackUsed": !model_generated,
            }),
        )
        .await,
    );

    Ok(MainChatReactFollowUp {
        reply,
        model_route,
        transcript_entries,
    })
}

pub(crate) fn main_chat_permission_blocker_reason(
    plan: &MainChatReactActionPlan,
    blocker_reason: &str,
) -> String {
    if plan.queue_action_type == "mcp.read_only" {
        "tool_permission_required".into()
    } else {
        blocker_reason.into()
    }
}

pub(crate) fn blocked_main_chat_observation(
    plan: &MainChatReactActionPlan,
    blocker_reason: &str,
    extra_metadata: serde_json::Value,
) -> MainChatObservation {
    let metadata = serde_json::json!({
        "actionType": plan.queue_action_type.clone(),
        "executorActionType": plan.executor_action_type.clone(),
        "target": plan.target.clone(),
        "argumentsDigest": openlife_core::agent::react_beta::metadata_safe_value_digest(&plan.arguments),
        "actionExecutorBacked": true,
        "executorStatus": "blocked",
        "blockerReason": blocker_reason,
        "retryReplayable": false,
        "directWritesExecuted": false,
        "extra": extra_metadata,
    });

    MainChatObservation {
        summary: format!("Governed ReAct action {} blocked.", plan.target),
        output_preview: blocker_reason.into(),
        final_answer: format!(
            "That read action is blocked by governance: {}",
            blocker_reason
        ),
        metadata,
        executor_status: openlife_core::agent::ActionExecutionStatus::Blocked,
        blocker_reason: Some(blocker_reason.into()),
    }
}

pub(crate) fn tool_call_from_action(
    name: &str,
    action_id: &str,
    success: bool,
    output: Option<String>,
    error: Option<String>,
    status: ToolCallStatus,
    requires_confirmation: bool,
) -> ToolCallResult {
    ToolCallResult {
        name: name.into(),
        arguments: serde_json::json!({ "mainChatAgentV1": true }),
        sanitized_arguments: Some(serde_json::json!({ "mainChatAgentV1": true })),
        success,
        output,
        error,
        permission_level: if requires_confirmation {
            "confirmation_required".into()
        } else {
            "governed".into()
        },
        status,
        requires_confirmation,
        pii_found: false,
        privacy_warnings: Vec::new(),
        action_id: Some(action_id.into()),
        run_id: None,
        permission_decision: Some(if requires_confirmation {
            "blocked_pending_confirmation".into()
        } else {
            "policy_checked".into()
        }),
        react_trace: None,
    }
}

pub(crate) fn agent_actions_to_tool_call_results(
    actions: &[openlife_core::agent::AgentAction],
    run_id: &str,
) -> Vec<ToolCallResult> {
    actions
        .iter()
        .map(|action| {
            let output = action.output.as_ref().and_then(|value| {
                value
                    .get("text")
                    .and_then(|text| text.as_str())
                    .map(ToString::to_string)
                    .or_else(|| value.as_str().map(ToString::to_string))
            });
            ToolCallResult {
                name: action.target.clone().unwrap_or_default(),
                arguments: action.input.clone(),
                sanitized_arguments: None,
                success: matches!(
                    action.status.as_str(),
                    "succeeded" | "completed" | "success"
                ),
                output,
                error: action.error.clone(),
                permission_level: action
                    .tool_scope
                    .as_ref()
                    .map(|scope| scope.risk_level.clone())
                    .unwrap_or_else(|| "low".to_string()),
                status: match action.status.as_str() {
                    "success" | "succeeded" | "completed" => ToolCallStatus::Success,
                    "needs_confirmation" => ToolCallStatus::NeedsConfirmation,
                    "blocked" => ToolCallStatus::Blocked,
                    _ => ToolCallStatus::Error,
                },
                requires_confirmation: action.status == "needs_confirmation",
                pii_found: false,
                privacy_warnings: Vec::new(),
                action_id: Some(action.id.clone()),
                run_id: Some(run_id.to_string()),
                permission_decision: action.permission_decision.clone(),
                react_trace: action.react_trace.clone(),
            }
        })
        .collect()
}
