use std::collections::HashMap;
use std::sync::Arc;

use openlife_core::agent::ReasoningTrace;
use openlife_core::layer_router::Layer;
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::privacy::PrivacyEngine;

use crate::commands;
use crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn;
use crate::main_chat_context_loader::compile_main_chat_context;
use crate::main_chat_generation_support::{
    finalize_chat_agent_run, generate_non_stream_fallback, main_chat_provider_endpoint_kind,
    preview_text,
};
use crate::main_chat_hs_runtime::build_chat_runtime_hs_packet;
use crate::main_chat_proposal_support::{
    attach_main_chat_tool_permission_proposal_metadata, create_main_chat_agent_proposal,
};
use crate::main_chat_react_execution::execute_main_chat_react_action_with_executor;
use crate::main_chat_react_runtime::{
    bind_main_chat_observation_metadata_to_queue_action, main_chat_permission_blocker_reason,
    synthesize_main_chat_react_follow_up, tool_call_from_action,
    try_run_main_chat_react_agent_loop,
};
use crate::main_chat_react_tool_selection::build_main_chat_react_action_plan;
use crate::main_chat_runtime_support::{
    append_main_chat_agent_transcript, enqueue_main_chat_agent_action, fail_main_chat_action,
    transition_main_chat_action, MainChatAgentTurn,
};
use crate::{AppState, SendMessageResult, ToolCallStatus};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn try_run_main_chat_agent_strategy(
    session_id: &str,
    user_msg: Option<&ChatMessage>,
    messages_for_generation: &[ChatMessage],
    life_model: &LifeModel,
    context_summary: openlife_core::agent::ContextSummary,
    embed_err: Option<String>,
    auto_checkin_msg: Option<String>,
    main_chat_agent_turn: &MainChatAgentTurn,
    state: &Arc<AppState>,
    privacy_engine: &PrivacyEngine,
    privacy_map: &HashMap<String, String>,
    existing_agent_run: Option<openlife_core::agent::AgentRun>,
    selected_skill_id: Option<&str>,
) -> Result<Option<SendMessageResult>, String> {
    use openlife_core::agent::main_chat_agent_v1::{
        ExecutionQueueStatus, ExecutionTranscriptEntryKind, MainChatAgentStrategy,
    };

    let strategy = main_chat_agent_turn.decision.selected_strategy;
    let user_text = user_msg
        .map(|message| message.content.as_str())
        .unwrap_or("");
    let task_session_id = main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref()
        .ok_or_else(|| "Main Chat Agent task session missing".to_string())?;
    let mut execution_transcript = main_chat_agent_turn.transcript_entries.clone();
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::Plan,
            "Main Chat Agent strategy execution started.",
            serde_json::json!({
                "selectedStrategy": strategy.as_str(),
                "policyReasonCode": main_chat_agent_turn.decision.privacy_risk.policy_reason_code,
                "silentWritesAllowed": false,
            }),
        )
        .await,
    );

    let compiled_context = compile_main_chat_context(
        state,
        &main_chat_agent_turn.decision,
        task_session_id,
        user_text,
        selected_skill_id,
    )
    .await;
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::Observation,
            "Bounded context was selected for this strategy.",
            serde_json::json!({
                "contextSnapshotRef": compiled_context.context_snapshot_ref,
                "selectedSourceCount": compiled_context.selected_sources.len(),
                "totalTokenEstimate": compiled_context.total_token_estimate,
                "rawLifeModelYamlIncluded": compiled_context.raw_life_model_yaml_included,
                "rawTopKMemoryTrusted": compiled_context.raw_topk_memory_trusted,
                "workspacePolicyOverrideBlocked": compiled_context.workspace_policy_override_blocked,
                "selectedSkillInstructionLoaded": compiled_context.selected_skill_instruction_loaded,
                "sources": compiled_context.selected_sources,
            }),
        )
        .await,
    );

    let mut tool_calls = Vec::new();
    let mut reply = "I could not complete the requested Main Chat task.".to_string();
    let mut pending_blockers = Vec::new();
    let mut completed = false;
    let mut hard_blocked = false;
    let mut model_route_override = None;
    let mut direct_answer_generation_metadata: Option<serde_json::Value> = None;

    match strategy {
        MainChatAgentStrategy::DirectAnswer => {
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Plan,
                    "DirectAnswer prompt contract was prepared.",
                    serde_json::json!({
                        "selectedStrategy": strategy.as_str(),
                        "contextSnapshotRef": compiled_context.context_snapshot_ref,
                        "toolExecutionAllowed": false,
                        "writeExecutionAllowed": false,
                        "silentWritesAllowed": false,
                        "localOnlyRequired": main_chat_agent_turn.decision.privacy_risk.local_only_required,
                    }),
                )
                .await,
            );
            let scheduler = state.scheduler.lock().await.clone();
            let task = openlife_core::agent::AgentTask {
                kind: openlife_core::agent::AgentTaskKind::Conversation,
                session_id: session_id.to_string(),
                user_text: user_text.to_string(),
                messages: messages_for_generation.to_vec(),
                layer: Layer::L2,
            };
            let hs_packet =
                build_chat_runtime_hs_packet(state, &task, life_model, "", None).await?;
            let direct_answer_model_route = scheduler.preview_chat_route(None).await;
            let scripted_provider_response = scheduler.scripted_generation_response.is_some();
            let provider_endpoint_kind =
                main_chat_provider_endpoint_kind(&scheduler, scripted_provider_response);
            let live_provider_invoked = !scripted_provider_response
                && direct_answer_model_route.provider != "none"
                && direct_answer_model_route.route_type == "cloud";
            let mut routed_generation_messages = messages_for_generation.to_vec();
            routed_generation_messages.insert(
                0,
                ChatMessage {
                    role: "system".into(),
                    content: format!(
                        "Runtime route facts for this turn: provider={}, model={}, routeType={}, reason={}, localModel={}, preferLocal={}. If the user asks what model/provider is being used or whether external network/search is available, answer only from these route facts and explicit tool availability. Do not infer web/search/API capabilities from provider configuration.",
                        direct_answer_model_route.provider,
                        direct_answer_model_route.model,
                        direct_answer_model_route.route_type,
                        direct_answer_model_route.reason,
                        direct_answer_model_route.local_model,
                        direct_answer_model_route.prefer_local
                    ),
                },
            );
            let generated = generate_non_stream_fallback(
                &scheduler,
                routed_generation_messages,
                life_model,
                "",
                hs_packet.clone(),
            )
            .await?;
            reply = privacy_engine.reconstruct(&generated, privacy_map);
            model_route_override = Some(direct_answer_model_route.clone());
            let generation_metadata = serde_json::json!({
                "hsPacketSelected": hs_packet.is_some(),
                "toolCallCount": 0,
                "directWritesExecuted": false,
                "legacyFallbackUsed": false,
                "modelGenerated": true,
                "schedulerGenerationCalled": true,
                "providerGenerationPath": "main_chat_direct_answer_scheduler",
                "provider": direct_answer_model_route.provider,
                "model": direct_answer_model_route.model,
                "routeType": direct_answer_model_route.route_type,
                "routeReason": direct_answer_model_route.reason,
                "providerHealthEstimated": direct_answer_model_route.provider_health_is_estimated,
                "scriptedProviderResponse": scripted_provider_response,
                "liveProviderInvoked": live_provider_invoked,
                "providerEndpointKind": provider_endpoint_kind,
                "localProviderHttpHarness": live_provider_invoked
                    && provider_endpoint_kind == "local_test_http",
                "externalLiveProviderEvalPreflighted": false,
            });
            direct_answer_generation_metadata = Some(generation_metadata.clone());
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Observation,
                    "DirectAnswer generated a model response without tools or writes.",
                    generation_metadata,
                )
                .await,
            );
            completed = true;
        }
        MainChatAgentStrategy::ReActToolExecution => {
            let action_plan = match build_main_chat_react_action_plan(session_id, user_text) {
                Ok(action_plan) => Some(action_plan),
                Err(error) => {
                    let blocker_reason = main_chat_react_action_plan_error_blocker_reason(&error);
                    hard_blocked = true;
                    pending_blockers.push(blocker_reason.clone());
                    execution_transcript.extend(
                        append_main_chat_agent_transcript(
                            state,
                            Some(task_session_id),
                            ExecutionTranscriptEntryKind::Error,
                            "ReAct tool action was blocked before execution.",
                            serde_json::json!({
                                "blockerReason": blocker_reason,
                                "sourceMissing": blocker_reason == "workspace_file_read_source_missing",
                                "directWritesExecuted": false,
                                "legacyFallbackUsed": false,
                            }),
                        )
                        .await,
                    );
                    reply = main_chat_react_action_plan_blocked_reply(&blocker_reason);
                    None
                }
            };
            if let Some(action_plan) = action_plan {
                let queued = enqueue_main_chat_agent_action(
                    state,
                    task_session_id,
                    &action_plan.queue_action_type,
                    &action_plan.description,
                    &mut execution_transcript,
                )
                .await?;
                if queued.policy.execution_allowed {
                    transition_main_chat_action(
                        state,
                        &queued.id,
                        ExecutionQueueStatus::Executing,
                        None,
                    )
                    .await?;
                    let agent_loop_attempt = try_run_main_chat_react_agent_loop(
                        state,
                        task_session_id,
                        session_id,
                        user_text,
                        messages_for_generation,
                        life_model,
                        privacy_engine,
                        privacy_map,
                        &action_plan,
                        main_chat_agent_turn
                            .decision
                            .privacy_risk
                            .local_only_required,
                    )
                    .await?;
                    execution_transcript.extend(agent_loop_attempt.transcript_entries);
                    let mut agent_loop_metadata = agent_loop_attempt.metadata.clone();
                    bind_main_chat_observation_metadata_to_queue_action(
                        &mut agent_loop_metadata,
                        &queued.id,
                    );
                    if let Some(queue_status) = agent_loop_attempt.queue_status {
                        match queue_status {
                            ExecutionQueueStatus::Completed => {
                                transition_main_chat_action(
                                    state,
                                    &queued.id,
                                    ExecutionQueueStatus::Observed,
                                    Some(agent_loop_metadata.clone()),
                                )
                                .await?;
                                transition_main_chat_action(
                                    state,
                                    &queued.id,
                                    ExecutionQueueStatus::Completed,
                                    None,
                                )
                                .await?;
                                completed = true;
                            }
                            ExecutionQueueStatus::PendingPermission => {
                                let blocker_reason = agent_loop_attempt
                                    .blocker_reason
                                    .clone()
                                    .unwrap_or_else(|| "tool_permission_required".into());
                                let permission_blocker = main_chat_permission_blocker_reason(
                                    &action_plan,
                                    &blocker_reason,
                                );
                                let (metadata, permission_transcript) =
                                    attach_main_chat_tool_permission_proposal_metadata(
                                        state,
                                        task_session_id,
                                        &action_plan,
                                        Some(&permission_blocker),
                                        agent_loop_metadata.clone(),
                                    )
                                    .await?;
                                execution_transcript.extend(permission_transcript);
                                transition_main_chat_action(
                                    state,
                                    &queued.id,
                                    ExecutionQueueStatus::PendingPermission,
                                    Some(metadata),
                                )
                                .await?;
                                pending_blockers.push(permission_blocker);
                            }
                            ExecutionQueueStatus::Failed => {
                                if agent_loop_metadata
                                    .get("agentLoopActionStatus")
                                    .and_then(serde_json::Value::as_str)
                                    == Some("blocked")
                                {
                                    hard_blocked = true;
                                }
                                let blocker_reason = agent_loop_attempt
                                    .blocker_reason
                                    .clone()
                                    .unwrap_or_else(|| "agent_loop_action_failed".into());
                                pending_blockers.push(blocker_reason.clone());
                                fail_main_chat_action(
                                    state,
                                    &queued.id,
                                    &blocker_reason,
                                    agent_loop_metadata.clone(),
                                )
                                .await?;
                            }
                            ExecutionQueueStatus::Planned
                            | ExecutionQueueStatus::Executing
                            | ExecutionQueueStatus::Observed
                            | ExecutionQueueStatus::Retrying
                            | ExecutionQueueStatus::Cancelled => {
                                fail_main_chat_action(
                                    state,
                                    &queued.id,
                                    "agent_loop_action_incomplete",
                                    agent_loop_metadata.clone(),
                                )
                                .await?;
                            }
                        }
                        if agent_loop_metadata.get("sourceKind").is_some() {
                            execution_transcript.extend(
                            append_main_chat_agent_transcript(
                                state,
                                Some(task_session_id),
                                ExecutionTranscriptEntryKind::Observation,
                                "Governed ReAct AgentLoop observation recorded for the queued action.",
                                agent_loop_metadata.clone(),
                            )
                            .await,
                        );
                        }
                        if let Some(model_route) = agent_loop_attempt.model_route {
                            model_route_override = Some(model_route);
                        }
                        tool_calls.extend(agent_loop_attempt.tool_calls);
                        reply = agent_loop_attempt.reply.unwrap_or_else(|| {
                            "The governed ReAct AgentLoop completed without a final response."
                                .into()
                        });
                        if agent_loop_attempt.queue_status == Some(ExecutionQueueStatus::Completed)
                            && user_text_requests_memory_proposal_after_read(user_text)
                        {
                            let proposal = create_main_chat_agent_proposal(
                                state,
                                task_session_id,
                                MainChatAgentStrategy::MemoryProposal,
                                user_text,
                            )
                            .await?;
                            pending_blockers.push(format!("proposal:{}", proposal.id));
                            reply = format!(
                            "{reply}\n\nI also created a Memory proposal for review after the read. I did not write it into long-term memory."
                        );
                        }
                    } else {
                        match execute_main_chat_react_action_with_executor(
                            state,
                            &action_plan,
                            main_chat_agent_turn
                                .decision
                                .privacy_risk
                                .local_only_required,
                        )
                        .await
                        {
                            Ok(observation) => {
                                let mut observation_metadata = observation.metadata.clone();
                                bind_main_chat_observation_metadata_to_queue_action(
                                    &mut observation_metadata,
                                    &queued.id,
                                );
                                if observation.executor_status
                                == openlife_core::agent::ActionExecutionStatus::Succeeded
                            {
                                transition_main_chat_action(
                                    state,
                                    &queued.id,
                                    ExecutionQueueStatus::Observed,
                                    Some(observation_metadata.clone()),
                                )
                                .await?;
                                transition_main_chat_action(
                                    state,
                                    &queued.id,
                                    ExecutionQueueStatus::Completed,
                                    None,
                                )
                                .await?;
                                completed = true;
                            } else if observation.executor_status
                                == openlife_core::agent::ActionExecutionStatus::NeedsConfirmation
                            {
                                let blocker_reason = observation
                                    .blocker_reason
                                    .clone()
                                    .unwrap_or_else(|| "tool_permission_required".into());
                                let permission_blocker = main_chat_permission_blocker_reason(
                                    &action_plan,
                                    &blocker_reason,
                                );
                                let (permission_metadata, permission_transcript) =
                                    attach_main_chat_tool_permission_proposal_metadata(
                                        state,
                                        task_session_id,
                                        &action_plan,
                                        Some(&permission_blocker),
                                        observation_metadata.clone(),
                                    )
                                    .await?;
                                observation_metadata = permission_metadata;
                                execution_transcript.extend(permission_transcript);
                                transition_main_chat_action(
                                    state,
                                    &queued.id,
                                    ExecutionQueueStatus::PendingPermission,
                                    Some(observation_metadata.clone()),
                                )
                                .await?;
                                pending_blockers.push(permission_blocker);
                            } else {
                                if observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::Blocked
                                {
                                    hard_blocked = true;
                                    pending_blockers.push(
                                        observation
                                            .blocker_reason
                                            .clone()
                                            .unwrap_or_else(|| "read_action_blocked".into()),
                                    );
                                }
                                fail_main_chat_action(
                                    state,
                                    &queued.id,
                                    observation
                                        .blocker_reason
                                        .as_deref()
                                        .unwrap_or("ActionExecutor read action failed."),
                                    observation_metadata.clone(),
                                )
                                .await?;
                            }
                                execution_transcript.extend(
                                    append_main_chat_agent_transcript(
                                        state,
                                        Some(task_session_id),
                                        ExecutionTranscriptEntryKind::Observation,
                                        observation.summary.clone(),
                                        observation_metadata.clone(),
                                    )
                                    .await,
                                );
                                tool_calls.push(tool_call_from_action(
                                &action_plan.target,
                                &queued.id,
                                observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::Succeeded,
                                if observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::Succeeded
                                {
                                    Some(observation.output_preview.clone())
                                } else {
                                    None
                                },
                                if observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::Succeeded
                                {
                                    None
                                } else {
                                    observation.blocker_reason.clone()
                                },
                                match observation.executor_status {
                                    openlife_core::agent::ActionExecutionStatus::Succeeded => {
                                        ToolCallStatus::Success
                                    }
                                    openlife_core::agent::ActionExecutionStatus::NeedsConfirmation
                                    | openlife_core::agent::ActionExecutionStatus::Blocked => {
                                        ToolCallStatus::Blocked
                                    }
                                    openlife_core::agent::ActionExecutionStatus::Failed => {
                                        ToolCallStatus::Error
                                    }
                                },
                                observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
                            ));
                                if observation.executor_status
                                    == openlife_core::agent::ActionExecutionStatus::Succeeded
                                {
                                    let follow_up = synthesize_main_chat_react_follow_up(
                                        state,
                                        task_session_id,
                                        session_id,
                                        user_text,
                                        messages_for_generation,
                                        life_model,
                                        privacy_engine,
                                        privacy_map,
                                        &observation,
                                    )
                                    .await?;
                                    if let Some(model_route) = follow_up.model_route {
                                        model_route_override = Some(model_route);
                                    }
                                    execution_transcript.extend(follow_up.transcript_entries);
                                    reply = follow_up.reply;
                                    if user_text_requests_memory_proposal_after_read(user_text) {
                                        let proposal = create_main_chat_agent_proposal(
                                            state,
                                            task_session_id,
                                            MainChatAgentStrategy::MemoryProposal,
                                            user_text,
                                        )
                                        .await?;
                                        pending_blockers.push(format!("proposal:{}", proposal.id));
                                        reply = format!(
                                        "{reply}\n\nI also created a Memory proposal for review after the read. I did not write it into long-term memory."
                                    );
                                    }
                                } else {
                                    reply = observation.final_answer.clone();
                                }
                            }
                            Err(error) => {
                                fail_main_chat_action(
                                    state,
                                    &queued.id,
                                    &error,
                                    serde_json::json!({
                                        "error": error,
                                        "actionExecutorBacked": true,
                                        "failSoft": true,
                                    }),
                                )
                                .await?;
                                execution_transcript.extend(
                                    append_main_chat_agent_transcript(
                                        state,
                                        Some(task_session_id),
                                        ExecutionTranscriptEntryKind::Error,
                                        "Read-only tool action could not complete.",
                                        serde_json::json!({
                                            "actionId": queued.id,
                                            "error": error,
                                            "retryAvailable": true,
                                        }),
                                    )
                                    .await,
                                );
                                tool_calls.push(tool_call_from_action(
                                    &action_plan.target,
                                    &queued.id,
                                    false,
                                    None,
                                    Some(error.clone()),
                                    ToolCallStatus::Error,
                                    false,
                                ));
                                reply = format!(
                                "I could not complete that read-only action yet. Blocker: {error}\n\nYou can retry or narrow the request."
                            );
                            }
                        }
                    }
                } else {
                    pending_blockers.push(queued.policy.reason_code.clone());
                    reply = "This action needs review before it can run.".into();
                }
            }
        }
        MainChatAgentStrategy::PlanExecute => {
            let queued = enqueue_main_chat_agent_action(
                state,
                task_session_id,
                "plan_execute.create_session",
                "Create a governed PlanExecute draft session from Main Chat.",
                &mut execution_transcript,
            )
            .await?;
            transition_main_chat_action(
                state,
                &queued.id,
                ExecutionQueueStatus::Executing,
                Some(serde_json::json!({
                    "executor": "plan_execute.create_session",
                    "directWritesExecuted": false,
                })),
            )
            .await?;
            let plan_session = commands::agent_runtime::create_plan_execute_session_with_state(
                commands::agent_runtime::CreatePlanExecuteSessionInput {
                    scenario_id: Some("weekly_planning".into()),
                    source_chat_session_id: Some(session_id.to_string()),
                    max_steps: Some(5),
                },
                state,
            )
            .await?;
            transition_main_chat_action(
                state,
                &queued.id,
                ExecutionQueueStatus::Observed,
                Some(serde_json::json!({
                    "planExecuteSessionId": plan_session.session_id,
                    "stepCount": plan_session.steps.len(),
                })),
            )
            .await?;
            transition_main_chat_action(state, &queued.id, ExecutionQueueStatus::Completed, None)
                .await?;
            if let Some(ref store_arc) = state.main_chat_agent_session_store {
                let store = store_arc.lock().await;
                if let Err(err) = store.update_plan_summary(
                    task_session_id,
                    Some(format!(
                        "PlanExecute draft {} has {} steps.",
                        plan_session.session_id,
                        plan_session.steps.len()
                    )),
                ) {
                    log::warn!("[MainChatAgent] update plan summary failed: {}", err);
                }
            }
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Plan,
                    "Governed PlanExecute draft session was created.",
                    serde_json::json!({
                        "planExecuteSessionId": plan_session.session_id,
                        "status": plan_session.status,
                        "stepCount": plan_session.steps.len(),
                        "directWritesExecuted": false,
                    }),
                )
                .await,
            );
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Observation,
                    "Governed PlanExecute draft observation recorded for the queued action.",
                    serde_json::json!({
                        "actionId": queued.id,
                        "sourceKind": "plan_execute",
                        "sourceLabel": "plan_execute.create_session",
                        "preview": format!(
                            "PlanExecute draft with {} steps",
                            plan_session.steps.len()
                        ),
                        "planExecuteSessionId": plan_session.session_id,
                        "stepCount": plan_session.steps.len(),
                        "directWritesExecuted": false,
                    }),
                )
                .await,
            );
            tool_calls.push(tool_call_from_action(
                "plan_execute.create_session",
                &queued.id,
                true,
                Some(format!(
                    "PlanExecute draft with {} steps",
                    plan_session.steps.len()
                )),
                None,
                ToolCallStatus::Success,
                false,
            ));
            reply = format!(
                "I created a governed draft plan with {} steps. It is not saved as accepted truth yet; review or adjust it before executing any write-like step.",
                plan_session.steps.len()
            );
            completed = true;
            if user_text_requests_risky_external_publish_confirmation(user_text) {
                let blocked_publish = enqueue_main_chat_agent_action(
                    state,
                    task_session_id,
                    "external.write",
                    "Risky external publish step from a PlanExecute draft requires explicit confirmation.",
                    &mut execution_transcript,
                )
                .await?;
                pending_blockers.push(blocked_publish.policy.reason_code.clone());
                execution_transcript.extend(
                    append_main_chat_agent_transcript(
                        state,
                        Some(task_session_id),
                        ExecutionTranscriptEntryKind::PermissionRequest,
                        "Risky external publish step is blocked pending explicit confirmation.",
                        serde_json::json!({
                            "actionId": blocked_publish.id,
                            "policyLevel": blocked_publish.policy.level.as_str(),
                            "reasonCode": blocked_publish.policy.reason_code.clone(),
                            "requiresConfirmation": blocked_publish.policy.requires_confirmation,
                            "externalWritesExecuted": false,
                        }),
                    )
                    .await,
                );
                tool_calls.push(tool_call_from_action(
                    "external.write",
                    &blocked_publish.id,
                    false,
                    None,
                    Some("Risky external publish requires explicit confirmation and was not executed.".into()),
                    ToolCallStatus::Blocked,
                    true,
                ));
                reply = format!(
                    "{reply}\n\nThe risky external publish step is blocked until you explicitly confirm it. No external write was executed."
                );
            }
        }
        MainChatAgentStrategy::MemoryProposal | MainChatAgentStrategy::LifeModelProposal => {
            let proposal =
                create_main_chat_agent_proposal(state, task_session_id, strategy, user_text)
                    .await?;
            pending_blockers.push(format!("proposal:{}", proposal.id));
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::ProposalRequest,
                    "A governed proposal was created for review.",
                    serde_json::json!({
                        "proposalId": proposal.id,
                        "proposalType": proposal.proposal_type,
                        "riskLevel": proposal.risk_level,
                        "status": proposal.status,
                        "directWritesExecuted": false,
                    }),
                )
                .await,
            );
            reply = match strategy {
                MainChatAgentStrategy::MemoryProposal => {
                    "I created a Memory proposal for review. I did not write it into long-term memory.".into()
                }
                _ => {
                    "I created a LifeModel proposal for review. I did not update accepted LifeModel truth.".into()
                }
            };
        }
        MainChatAgentStrategy::ReviewMaturation => {
            let queued = enqueue_main_chat_agent_action(
                state,
                task_session_id,
                "review.maturation_read",
                "Review metadata-safe maturation/evidence surfaces.",
                &mut execution_transcript,
            )
            .await?;
            transition_main_chat_action(
                state,
                &queued.id,
                ExecutionQueueStatus::Observed,
                Some(serde_json::json!({
                    "reviewSurface": "metadata_safe",
                    "directWritesExecuted": false,
                })),
            )
            .await?;
            transition_main_chat_action(state, &queued.id, ExecutionQueueStatus::Completed, None)
                .await?;
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::Observation,
                    "Review request completed as a metadata-safe summary.",
                    serde_json::json!({
                        "reviewedAcceptedTruth": false,
                        "proposalCreated": false,
                        "directWritesExecuted": false,
                    }),
                )
                .await,
            );
            reply = "I can review the relevant metadata-safe trace, evidence, and proposal surfaces here. I did not promote any observation into accepted guidance or LifeModel truth.".into();
            completed = true;
        }
        MainChatAgentStrategy::BlockedConfirmation => {
            let unselected_skill_boundary = user_text_requests_unselected_skill(user_text);
            let action_type = if unselected_skill_boundary {
                "skill.boundary"
            } else {
                "external.write"
            };
            let action_description = if unselected_skill_boundary {
                "Unselected skill instruction requested from Main Chat."
            } else {
                "External or sensitive write requested from Main Chat."
            };
            let queued = enqueue_main_chat_agent_action(
                state,
                task_session_id,
                action_type,
                action_description,
                &mut execution_transcript,
            )
            .await?;
            pending_blockers.push(queued.policy.reason_code.clone());
            execution_transcript.extend(
                append_main_chat_agent_transcript(
                    state,
                    Some(task_session_id),
                    ExecutionTranscriptEntryKind::PermissionRequest,
                    if unselected_skill_boundary {
                        "Unselected skill instruction is blocked because it was not explicitly selected for this turn."
                    } else {
                        "External or sensitive write is blocked pending explicit confirmation and provider support."
                    },
                    serde_json::json!({
                        "actionId": queued.id,
                        "policyLevel": queued.policy.level.as_str(),
                        "reasonCode": queued.policy.reason_code.clone(),
                        "requiresConfirmation": queued.policy.requires_confirmation,
                        "externalWritesExecuted": false,
                        "unselectedSkillInjected": false,
                    }),
                )
                .await,
            );
            tool_calls.push(tool_call_from_action(
                action_type,
                &queued.id,
                false,
                None,
                Some(if unselected_skill_boundary {
                    "Unselected skill instructions are not injected unless explicitly selected for the turn.".into()
                } else {
                    "External or sensitive write requires explicit confirmation and is not executed in Main Chat v1.".into()
                }),
                ToolCallStatus::Blocked,
                true,
            ));
            reply = if unselected_skill_boundary {
                "I cannot use a skill that was not explicitly selected for this turn. No unselected skill instruction was injected.".into()
            } else {
                "I cannot send or write that directly. It requires explicit confirmation and a governed provider path; no external write was executed.".into()
            };
        }
    }

    if !pending_blockers.is_empty() {
        if let Some(ref store_arc) = state.main_chat_agent_session_store {
            let store = store_arc.lock().await;
            if let Err(err) = store.set_pending_blockers(task_session_id, pending_blockers.clone())
            {
                log::warn!("[MainChatAgent] set blockers failed: {}", err);
            }
            let transition_result = if hard_blocked {
                store.block_session(task_session_id, "Main Chat Agent v1 blocked by governance.")
            } else {
                store.mark_waiting_permission(task_session_id)
            };
            if let Err(err) = transition_result {
                log::warn!("[MainChatAgent] mark blocked/waiting failed: {}", err);
            }
        }
    } else if completed {
        if let Some(ref store_arc) = state.main_chat_agent_session_store {
            let store = store_arc.lock().await;
            if let Err(err) =
                store.complete_session(task_session_id, "Main Chat Agent v1 completed.")
            {
                log::warn!("[MainChatAgent] complete session failed: {}", err);
            }
        }
    }

    if let Some(msg) = auto_checkin_msg {
        if !reply.contains(&msg) {
            reply = format!("{}\n\n[系统] {}", reply, msg);
        }
    }

    let user_input_text = user_msg
        .map(|message| message.content.clone())
        .unwrap_or_default();
    let mut agent_run = existing_agent_run.unwrap_or_else(|| {
        openlife_core::agent::AgentRun::new_chat_run(session_id, &user_input_text)
    });
    agent_run.reasoning_strategy = Some(format!("main_chat_agent_v1_{}", strategy.as_str()));
    let model_route =
        model_route_override.unwrap_or_else(|| openlife_core::agent::ModelRouteTrace {
            provider: "openlife".to_string(),
            model: "main_chat_agent_v1".to_string(),
            route_type: "agent_runtime".to_string(),
            prefer_local: main_chat_agent_turn
                .decision
                .privacy_risk
                .local_only_required,
            local_model: "".to_string(),
            reason: format!("strategy={}", strategy.as_str()),
            privacy_level: if main_chat_agent_turn
                .decision
                .privacy_risk
                .local_only_required
            {
                openlife_core::agent::types::RedactionLevel::LocalOnly
            } else {
                openlife_core::agent::types::RedactionLevel::None
            },
            latency_ms: None,
            retry_count: 0,
            fallback_reason: None,
            provider_health_is_estimated: Some(false),
        });
    let generation_result = if strategy == MainChatAgentStrategy::DirectAnswer {
        let mut generation_result = serde_json::json!({
            "text": reply,
            "mainChatAgentV1": true,
            "selectedStrategy": strategy.as_str(),
            "legacyFallbackUsed": false,
            "modelGenerated": true,
            "schedulerGenerationCalled": true,
            "providerGenerationPath": "main_chat_direct_answer_scheduler",
            "provider": model_route.provider,
            "model": model_route.model,
            "routeType": model_route.route_type,
            "routeReason": model_route.reason,
            "providerHealthEstimated": model_route.provider_health_is_estimated,
        });
        if let (Some(serde_json::Value::Object(extra)), Some(target)) = (
            direct_answer_generation_metadata,
            generation_result.as_object_mut(),
        ) {
            for (key, value) in extra {
                target.insert(key, value);
            }
        }
        generation_result
    } else {
        serde_json::json!({
            "text": reply,
            "mainChatAgentV1": true,
            "selectedStrategy": strategy.as_str(),
            "legacyFallbackUsed": false,
        })
    };
    agent_run.complete(&preview_text(&reply, 200), model_route, context_summary);
    let mut reasoning_trace = ReasoningTrace {
        generation_result: Some(generation_result),
        ..Default::default()
    };
    if let Some(err) = embed_err {
        reasoning_trace.errors.push(err);
    }
    let assistant_message = ChatMessage {
        role: "assistant".into(),
        content: reply.clone(),
    };
    finalize_chat_agent_run(
        session_id,
        &assistant_message,
        &reply,
        &mut reasoning_trace,
        &mut agent_run,
        life_model,
        state,
    )
    .await?;
    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::FinalResult,
            "Main Chat Agent v1 response was delivered.",
            serde_json::json!({
                "runId": agent_run.id,
                "legacyFallbackUsed": false,
                "pendingBlockerCount": pending_blockers.len(),
            }),
        )
        .await,
    );
    let agent_state =
        assemble_main_chat_agent_state_for_turn(state, Some(task_session_id), Some(&agent_run.id))
            .await;

    Ok(Some(SendMessageResult {
        reply,
        reasoning_trace,
        tool_calls,
        run_id: Some(agent_run.id),
        agent_ingress: Some(main_chat_agent_turn.decision.clone()),
        agent_state,
        execution_transcript,
        legacy_fallback_used: false,
    }))
}

fn user_text_requests_unselected_skill(user_text: &str) -> bool {
    let lower = user_text.to_ascii_lowercase();
    lower.contains("skill that is not selected")
        || lower.contains("unselected skill")
        || lower.contains("not selected skill")
}

fn user_text_requests_memory_proposal_after_read(user_text: &str) -> bool {
    let lower = user_text.to_ascii_lowercase();
    (lower.contains("memory proposal") || lower.contains("create a memory proposal"))
        && (lower.contains("read") || lower.contains("file"))
}

fn main_chat_react_action_plan_error_blocker_reason(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("file is not readable in workspace")
        || lower.contains("no such file")
        || lower.contains("not found")
    {
        "workspace_file_read_source_missing".into()
    } else if lower.contains("absolute file read paths are blocked")
        || lower.contains("path traversal")
        || lower.contains("outside workspace")
    {
        "workspace_file_read_policy_blocked".into()
    } else {
        "react_action_plan_unavailable".into()
    }
}

fn main_chat_react_action_plan_blocked_reply(blocker_reason: &str) -> String {
    match blocker_reason {
        "workspace_file_read_source_missing" => {
            "I could not read that workspace file because it is missing or unreadable. Check the path, choose another source, or cancel this task. I did not infer file contents.".into()
        }
        "workspace_file_read_policy_blocked" => {
            "I could not read that workspace file because the requested path is outside the allowed workspace boundary. Use a workspace-relative path or choose another source.".into()
        }
        _ => {
            "I could not prepare the requested tool action. You can revise the request, retry with a narrower source, or cancel this task.".into()
        }
    }
}

fn user_text_requests_risky_external_publish_confirmation(user_text: &str) -> bool {
    let lower = user_text.to_ascii_lowercase();
    lower.contains("risky external publish")
        || (lower.contains("ask me before")
            && lower.contains("external")
            && lower.contains("publish"))
}
