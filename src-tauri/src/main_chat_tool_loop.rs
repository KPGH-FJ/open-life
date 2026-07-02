use std::collections::HashMap;
use std::sync::Arc;

use openlife_core::agent::main_chat_agent_v1::{
    ExecutionQueueStatus, ExecutionTranscriptEntry, ExecutionTranscriptEntryKind,
    MainChatAgentStrategy,
};
use openlife_core::agent::{ContextSummary, ReasoningTrace};
use openlife_core::life_model::LifeModel;
use openlife_core::llm::ChatMessage;
use openlife_core::privacy::PrivacyEngine;

use crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn;
use crate::main_chat_context_loader::compile_main_chat_context;
use crate::main_chat_generation_support::{finalize_chat_agent_run, preview_text};
use crate::main_chat_proposal_support::{
    attach_main_chat_tool_permission_proposal_metadata, create_main_chat_agent_proposal,
};
use crate::main_chat_react_execution::execute_main_chat_react_action_with_executor;
use crate::main_chat_react_runtime::{
    bind_main_chat_observation_metadata_to_queue_action, main_chat_permission_blocker_reason,
    synthesize_main_chat_react_follow_up, tool_call_from_action,
    try_run_main_chat_react_agent_loop,
};
use crate::main_chat_react_tool_selection::{
    build_main_chat_react_action_plan, MainChatReactActionPlan,
};
use crate::main_chat_runtime_support::{
    append_main_chat_agent_transcript, enqueue_main_chat_agent_action, fail_main_chat_action,
    transition_main_chat_action, MainChatAgentTurn,
};
use crate::{AppState, SendMessageResult, ToolCallResult, ToolCallStatus};

pub(crate) struct MainChatToolLoopInput<'a> {
    pub(crate) session_id: &'a str,
    pub(crate) user_msg: Option<&'a ChatMessage>,
    pub(crate) desensitized_messages: &'a [ChatMessage],
    pub(crate) life_model: &'a LifeModel,
    pub(crate) context_summary: ContextSummary,
    pub(crate) embed_err: Option<String>,
    pub(crate) auto_checkin_msg: Option<String>,
    pub(crate) main_chat_agent_turn: &'a MainChatAgentTurn,
    pub(crate) privacy_engine: &'a PrivacyEngine,
    pub(crate) privacy_map: &'a HashMap<String, String>,
    pub(crate) existing_agent_run: Option<openlife_core::agent::AgentRun>,
    pub(crate) selected_skill_id: Option<&'a str>,
}

pub(crate) enum MainChatToolLoopOutcome {
    AgentLoopSuccess(SendMessageResult),
    GovernedBlocker(SendMessageResult),
    ToolPermissionProposal(SendMessageResult),
    SingleStepFallback(SendMessageResult),
    ExplicitFallbackAvailable { reason_code: String },
    NoResult { reason_code: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainChatToolLoopOutcomeKind {
    AgentLoopSuccess,
    GovernedBlocker,
    ToolPermissionProposal,
    SingleStepFallback,
    NoResult,
}

impl MainChatToolLoopOutcomeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::AgentLoopSuccess => "agent_loop_success",
            Self::GovernedBlocker => "governed_blocker",
            Self::ToolPermissionProposal => "tool_permission_proposal",
            Self::SingleStepFallback => "single_step_fallback",
            Self::NoResult => "no_result",
        }
    }

    fn wrap_result(self, result: SendMessageResult) -> MainChatToolLoopOutcome {
        match self {
            Self::AgentLoopSuccess => MainChatToolLoopOutcome::AgentLoopSuccess(result),
            Self::GovernedBlocker => MainChatToolLoopOutcome::GovernedBlocker(result),
            Self::ToolPermissionProposal => MainChatToolLoopOutcome::ToolPermissionProposal(result),
            Self::SingleStepFallback => MainChatToolLoopOutcome::SingleStepFallback(result),
            Self::NoResult => MainChatToolLoopOutcome::NoResult {
                reason_code: "tool_loop_no_result".into(),
            },
        }
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn run_main_chat_tool_loop_adapter(
    input: MainChatToolLoopInput<'_>,
    state: &Arc<AppState>,
) -> Result<MainChatToolLoopOutcome, String> {
    let MainChatToolLoopInput {
        session_id,
        user_msg,
        desensitized_messages,
        life_model,
        context_summary,
        embed_err,
        auto_checkin_msg,
        main_chat_agent_turn,
        privacy_engine,
        privacy_map,
        existing_agent_run,
        selected_skill_id,
    } = input;

    let strategy = main_chat_agent_turn.decision.selected_strategy;
    if strategy != MainChatAgentStrategy::ReActToolExecution {
        return Ok(MainChatToolLoopOutcome::NoResult {
            reason_code: "tool_loop_adapter_non_react_strategy".into(),
        });
    }

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
                "executionPath": "ToolLoop",
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
    let mut outcome_kind = MainChatToolLoopOutcomeKind::NoResult;
    let mut fallback_reason_code: Option<String> = None;

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
                        "executionPath": "ToolLoop",
                        "toolLoopOutcome": MainChatToolLoopOutcomeKind::GovernedBlocker.as_str(),
                        "blockerReason": blocker_reason,
                        "sourceMissing": blocker_reason == "workspace_file_read_source_missing",
                        "directWritesExecuted": false,
                        "legacyFallbackUsed": false,
                    }),
                )
                .await,
            );
            reply = main_chat_react_action_plan_blocked_reply(&blocker_reason);
            outcome_kind = MainChatToolLoopOutcomeKind::GovernedBlocker;
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
            transition_main_chat_action(state, &queued.id, ExecutionQueueStatus::Executing, None)
                .await?;
            let agent_loop_attempt = try_run_main_chat_react_agent_loop(
                state,
                task_session_id,
                session_id,
                user_text,
                desensitized_messages,
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
                outcome_kind = classify_main_chat_tool_loop_agent_loop_status(queue_status);
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
                        let permission_blocker =
                            main_chat_permission_blocker_reason(&action_plan, &blocker_reason);
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
                        fallback_reason_code = Some("agent_loop_action_incomplete".into());
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
                    "The governed ReAct AgentLoop completed without a final response.".into()
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
                outcome_kind = run_single_step_react_fallback(
                    state,
                    session_id,
                    user_text,
                    desensitized_messages,
                    life_model,
                    privacy_engine,
                    privacy_map,
                    task_session_id,
                    &action_plan,
                    &queued.id,
                    &mut execution_transcript,
                    &mut tool_calls,
                    &mut reply,
                    &mut pending_blockers,
                    &mut completed,
                    &mut hard_blocked,
                    &mut model_route_override,
                    main_chat_agent_turn
                        .decision
                        .privacy_risk
                        .local_only_required,
                )
                .await?;
            }
        } else {
            pending_blockers.push(queued.policy.reason_code.clone());
            reply = "This action needs review before it can run.".into();
            outcome_kind = MainChatToolLoopOutcomeKind::GovernedBlocker;
        }
    }

    if !pending_blockers.is_empty() {
        if let Some(ref store_arc) = state.main_chat_agent_session_store {
            let store = store_arc.lock().await;
            if let Err(err) = store.set_pending_blockers(task_session_id, pending_blockers.clone())
            {
                log::warn!("[MainChatToolLoop] set blockers failed: {}", err);
            }
            let transition_result = if hard_blocked {
                store.block_session(task_session_id, "Main Chat ToolLoop blocked by governance.")
            } else {
                store.mark_waiting_permission(task_session_id)
            };
            if let Err(err) = transition_result {
                log::warn!("[MainChatToolLoop] mark blocked/waiting failed: {}", err);
            }
        }
    } else if completed {
        if let Some(ref store_arc) = state.main_chat_agent_session_store {
            let store = store_arc.lock().await;
            if let Err(err) =
                store.complete_session(task_session_id, "Main Chat ToolLoop completed.")
            {
                log::warn!("[MainChatToolLoop] complete session failed: {}", err);
            }
        }
    }

    if outcome_kind == MainChatToolLoopOutcomeKind::NoResult {
        let reason_code = fallback_reason_code.unwrap_or_else(|| "tool_loop_no_result".into());
        return Ok(MainChatToolLoopOutcome::ExplicitFallbackAvailable { reason_code });
    }
    if !main_chat_tool_loop_result_has_runtime_evidence(
        outcome_kind,
        &tool_calls,
        &pending_blockers,
        &execution_transcript,
    ) {
        return Ok(MainChatToolLoopOutcome::NoResult {
            reason_code: "tool_loop_runtime_evidence_missing".into(),
        });
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
    let generation_result = serde_json::json!({
        "text": reply,
        "mainChatAgentV1": true,
        "selectedStrategy": strategy.as_str(),
        "executionPath": "ToolLoop",
        "toolLoopOutcome": outcome_kind.as_str(),
        "legacyFallbackUsed": false,
    });
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
            "Main Chat ToolLoop response was delivered.",
            serde_json::json!({
                "runId": agent_run.id,
                "executionPath": "ToolLoop",
                "toolLoopOutcome": outcome_kind.as_str(),
                "legacyFallbackUsed": false,
                "pendingBlockerCount": pending_blockers.len(),
            }),
        )
        .await,
    );
    let agent_state =
        assemble_main_chat_agent_state_for_turn(state, Some(task_session_id), Some(&agent_run.id))
            .await;

    Ok(outcome_kind.wrap_result(SendMessageResult {
        reply,
        status: "completed".into(),
        blockers: Vec::new(),
        reasoning_trace,
        tool_calls,
        run_id: Some(agent_run.id),
        agent_ingress: Some(main_chat_agent_turn.decision.clone()),
        agent_state,
        execution_transcript,
        legacy_fallback_used: false,
        legacy_runtime_invoked: false,
        model_invoked: true,
        tool_invoked: true,
    }))
}

#[allow(clippy::too_many_arguments)]
async fn run_single_step_react_fallback(
    state: &Arc<AppState>,
    session_id: &str,
    user_text: &str,
    messages_for_generation: &[ChatMessage],
    life_model: &LifeModel,
    privacy_engine: &PrivacyEngine,
    privacy_map: &HashMap<String, String>,
    task_session_id: &str,
    action_plan: &MainChatReactActionPlan,
    queued_id: &str,
    execution_transcript: &mut Vec<ExecutionTranscriptEntry>,
    tool_calls: &mut Vec<ToolCallResult>,
    reply: &mut String,
    pending_blockers: &mut Vec<String>,
    completed: &mut bool,
    hard_blocked: &mut bool,
    model_route_override: &mut Option<openlife_core::agent::ModelRouteTrace>,
    local_only_required: bool,
) -> Result<MainChatToolLoopOutcomeKind, String> {
    match execute_main_chat_react_action_with_executor(state, action_plan, local_only_required)
        .await
    {
        Ok(observation) => {
            let mut observation_metadata = observation.metadata.clone();
            mark_single_step_fallback_metadata(&mut observation_metadata);
            bind_main_chat_observation_metadata_to_queue_action(
                &mut observation_metadata,
                queued_id,
            );
            let outcome_kind =
                classify_main_chat_tool_loop_single_step_status(&observation.executor_status);
            if observation.executor_status == openlife_core::agent::ActionExecutionStatus::Succeeded
            {
                transition_main_chat_action(
                    state,
                    queued_id,
                    ExecutionQueueStatus::Observed,
                    Some(observation_metadata.clone()),
                )
                .await?;
                transition_main_chat_action(
                    state,
                    queued_id,
                    ExecutionQueueStatus::Completed,
                    None,
                )
                .await?;
                *completed = true;
            } else if observation.executor_status
                == openlife_core::agent::ActionExecutionStatus::NeedsConfirmation
            {
                let blocker_reason = observation
                    .blocker_reason
                    .clone()
                    .unwrap_or_else(|| "tool_permission_required".into());
                let permission_blocker =
                    main_chat_permission_blocker_reason(action_plan, &blocker_reason);
                let (permission_metadata, permission_transcript) =
                    attach_main_chat_tool_permission_proposal_metadata(
                        state,
                        task_session_id,
                        action_plan,
                        Some(&permission_blocker),
                        observation_metadata.clone(),
                    )
                    .await?;
                observation_metadata = permission_metadata;
                mark_single_step_fallback_metadata(&mut observation_metadata);
                execution_transcript.extend(permission_transcript);
                transition_main_chat_action(
                    state,
                    queued_id,
                    ExecutionQueueStatus::PendingPermission,
                    Some(observation_metadata.clone()),
                )
                .await?;
                pending_blockers.push(permission_blocker);
            } else {
                if observation.executor_status
                    == openlife_core::agent::ActionExecutionStatus::Blocked
                {
                    *hard_blocked = true;
                    pending_blockers.push(
                        observation
                            .blocker_reason
                            .clone()
                            .unwrap_or_else(|| "read_action_blocked".into()),
                    );
                }
                fail_main_chat_action(
                    state,
                    queued_id,
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
                queued_id,
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
                    openlife_core::agent::ActionExecutionStatus::Failed => ToolCallStatus::Error,
                },
                observation.executor_status
                    == openlife_core::agent::ActionExecutionStatus::NeedsConfirmation,
            ));
            if observation.executor_status == openlife_core::agent::ActionExecutionStatus::Succeeded
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
                    *model_route_override = Some(model_route);
                }
                execution_transcript.extend(follow_up.transcript_entries);
                *reply = follow_up.reply;
                if user_text_requests_memory_proposal_after_read(user_text) {
                    let proposal = create_main_chat_agent_proposal(
                        state,
                        task_session_id,
                        MainChatAgentStrategy::MemoryProposal,
                        user_text,
                    )
                    .await?;
                    pending_blockers.push(format!("proposal:{}", proposal.id));
                    let current_reply = reply.clone();
                    *reply = format!(
                        "{current_reply}\n\nI also created a Memory proposal for review after the read. I did not write it into long-term memory."
                    );
                }
            } else {
                *reply = observation.final_answer.clone();
            }
            Ok(outcome_kind)
        }
        Err(error) => {
            fail_main_chat_action(
                state,
                queued_id,
                &error,
                serde_json::json!({
                    "error": error,
                    "actionExecutorBacked": true,
                    "singleStepFallbackUsed": true,
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
                        "actionId": queued_id,
                        "error": error,
                        "singleStepFallbackUsed": true,
                        "retryAvailable": true,
                    }),
                )
                .await,
            );
            tool_calls.push(tool_call_from_action(
                &action_plan.target,
                queued_id,
                false,
                None,
                Some(error.clone()),
                ToolCallStatus::Error,
                false,
            ));
            *reply = format!(
                "I could not complete that read-only action yet. Blocker: {error}\n\nYou can retry or narrow the request."
            );
            Ok(MainChatToolLoopOutcomeKind::SingleStepFallback)
        }
    }
}

fn classify_main_chat_tool_loop_agent_loop_status(
    queue_status: ExecutionQueueStatus,
) -> MainChatToolLoopOutcomeKind {
    match queue_status {
        ExecutionQueueStatus::Completed => MainChatToolLoopOutcomeKind::AgentLoopSuccess,
        ExecutionQueueStatus::PendingPermission => {
            MainChatToolLoopOutcomeKind::ToolPermissionProposal
        }
        ExecutionQueueStatus::Failed => MainChatToolLoopOutcomeKind::GovernedBlocker,
        ExecutionQueueStatus::Planned
        | ExecutionQueueStatus::Executing
        | ExecutionQueueStatus::Observed
        | ExecutionQueueStatus::Retrying
        | ExecutionQueueStatus::Cancelled => MainChatToolLoopOutcomeKind::NoResult,
    }
}

fn classify_main_chat_tool_loop_single_step_status(
    status: &openlife_core::agent::ActionExecutionStatus,
) -> MainChatToolLoopOutcomeKind {
    match status {
        openlife_core::agent::ActionExecutionStatus::NeedsConfirmation => {
            MainChatToolLoopOutcomeKind::ToolPermissionProposal
        }
        openlife_core::agent::ActionExecutionStatus::Blocked => {
            MainChatToolLoopOutcomeKind::GovernedBlocker
        }
        openlife_core::agent::ActionExecutionStatus::Succeeded
        | openlife_core::agent::ActionExecutionStatus::Failed => {
            MainChatToolLoopOutcomeKind::SingleStepFallback
        }
    }
}

fn main_chat_tool_loop_result_has_runtime_evidence(
    outcome_kind: MainChatToolLoopOutcomeKind,
    tool_calls: &[ToolCallResult],
    pending_blockers: &[String],
    execution_transcript: &[ExecutionTranscriptEntry],
) -> bool {
    match outcome_kind {
        MainChatToolLoopOutcomeKind::AgentLoopSuccess
        | MainChatToolLoopOutcomeKind::SingleStepFallback => !tool_calls.is_empty(),
        MainChatToolLoopOutcomeKind::GovernedBlocker
        | MainChatToolLoopOutcomeKind::ToolPermissionProposal => {
            !pending_blockers.is_empty()
                || execution_transcript.iter().any(|entry| {
                    entry.metadata.get("blockerReason").is_some()
                        || entry.metadata.get("proposalId").is_some()
                })
        }
        MainChatToolLoopOutcomeKind::NoResult => false,
    }
}

fn mark_single_step_fallback_metadata(metadata: &mut serde_json::Value) {
    if let Some(object) = metadata.as_object_mut() {
        object.insert("singleStepFallbackUsed".into(), serde_json::json!(true));
        object.insert("agentLoopSucceeded".into(), serde_json::json!(false));
        object.insert("executionPath".into(), serde_json::json!("ToolLoop"));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::llm::ChatMessage;

    use crate::main_chat_send::send_message_with_state;

    async fn configure_scripted_tool_loop_scheduler(
        state: &Arc<AppState>,
        model: &str,
        tool_name: &str,
        action_type: &str,
    ) {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "openai".into(),
            "https://example.invalid/v1".into(),
            "test-key".into(),
            model.into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response(
            serde_json::json!({
                "final": "I will run the governed ToolLoop action first.",
                "actions": [{
                    "name": tool_name,
                    "action_type": action_type,
                    "arguments": {}
                }],
                "thought_summary": "Use the governed candidate contract.",
                "warnings": []
            })
            .to_string(),
        );
    }

    async fn grant_builtin_echo(state: &Arc<AppState>) {
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
            .expect("grant builtin_echo read permission");
    }

    async fn register_confirmation_required_read_tool(state: &Arc<AppState>) {
        let mut registry = state.mcp_registry.lock().await;
        registry.register_builtin(
            openlife_core::tool_manifest::ToolManifest {
                id: "approval_read".into(),
                name: "approval_read".into(),
                description: "Read tool that requires explicit ToolPermission approval.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
                permission_level: "medium".into(),
                risk_level: "medium".into(),
                version: "1.0.0".into(),
                source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["read".into(), "memory".into()],
                requires_confirmation: true,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                tags: vec!["read".into(), "approval".into()],
            },
            Box::new(|_| Ok("approval read should not execute before permission".into())),
        );
    }

    async fn run_tool_loop_send(
        state: Arc<AppState>,
        session_id: &str,
        user_text: &str,
    ) -> SendMessageResult {
        send_message_with_state(
            session_id.into(),
            vec![ChatMessage {
                role: "user".into(),
                content: user_text.into(),
            }],
            None,
            &state,
        )
        .await
        .expect("send_message ToolLoop response")
    }

    fn generation_result(response: &SendMessageResult) -> &serde_json::Value {
        response
            .reasoning_trace
            .generation_result
            .as_ref()
            .expect("generation result")
    }

    fn task_session_id(response: &SendMessageResult) -> &str {
        response
            .agent_ingress
            .as_ref()
            .and_then(|decision| decision.agent_task_session_id.as_deref())
            .expect("task session id")
    }

    #[tokio::test]
    async fn main_chat_tool_loop_file_read_success_uses_agent_loop_adapter() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        configure_scripted_tool_loop_scheduler(
            &state,
            "gpt-tool-loop-file-read",
            "file.read",
            "mcp_tool",
        )
        .await;

        let response = run_tool_loop_send(
            state,
            "tool-loop-file-read",
            "Read file Cargo.toml with argument_guard.",
        )
        .await;

        assert!(!response.legacy_fallback_used);
        let generation = generation_result(&response);
        assert_eq!(generation["executionPath"], serde_json::json!("ToolLoop"));
        assert_eq!(
            generation["toolLoopOutcome"],
            serde_json::json!("agent_loop_success")
        );
        assert!(response.tool_calls.iter().any(|call| call.success));
    }

    #[tokio::test]
    async fn main_chat_tool_loop_web_blocker_is_governed_blocker() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        {
            let mut config = state.config.lock().await;
            config.system.network_policy.enabled = false;
        }
        configure_scripted_tool_loop_scheduler(
            &state,
            "gpt-tool-loop-web-blocker",
            "web.search",
            "mcp_tool",
        )
        .await;

        let response = run_tool_loop_send(
            state,
            "tool-loop-web-blocker",
            "Please web search OpenLife release notes with argument_guard.",
        )
        .await;

        let generation = generation_result(&response);
        assert_eq!(generation["executionPath"], serde_json::json!("ToolLoop"));
        assert_eq!(
            generation["toolLoopOutcome"],
            serde_json::json!("governed_blocker")
        );
        assert!(response
            .tool_calls
            .iter()
            .any(|call| call.error.as_deref() == Some("network_policy_blocked")));
    }

    #[tokio::test]
    async fn main_chat_tool_loop_registered_mcp_read_success_uses_agent_loop_adapter() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        grant_builtin_echo(&state).await;
        configure_scripted_tool_loop_scheduler(
            &state,
            "gpt-tool-loop-mcp-success",
            "builtin_echo",
            "mcp_tool",
        )
        .await;

        let response = run_tool_loop_send(
            state,
            "tool-loop-mcp-success",
            "Use mcp builtin_echo read-only now.",
        )
        .await;

        let generation = generation_result(&response);
        assert_eq!(generation["executionPath"], serde_json::json!("ToolLoop"));
        assert_eq!(
            generation["toolLoopOutcome"],
            serde_json::json!("agent_loop_success")
        );
        assert!(response
            .tool_calls
            .iter()
            .any(|call| call.name == "builtin_echo"));
    }

    #[tokio::test]
    async fn main_chat_tool_loop_registered_mcp_tool_permission_proposal_is_distinct_outcome() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        register_confirmation_required_read_tool(&state).await;
        configure_scripted_tool_loop_scheduler(
            &state,
            "gpt-tool-loop-mcp-permission",
            "approval_read",
            "mcp_tool",
        )
        .await;

        let response = run_tool_loop_send(
            state.clone(),
            "tool-loop-mcp-permission",
            "Use mcp approval_read read-only now.",
        )
        .await;

        let generation = generation_result(&response);
        assert_eq!(generation["executionPath"], serde_json::json!("ToolLoop"));
        assert_eq!(
            generation["toolLoopOutcome"],
            serde_json::json!("tool_permission_proposal")
        );
        let session = {
            let store_arc = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("main chat session store");
            let store = store_arc.lock().await;
            store
                .load_session(task_session_id(&response))
                .expect("load task session")
                .expect("task session exists")
        };
        assert_eq!(
            session.status,
            openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::WaitingPermission
        );
    }

    #[tokio::test]
    async fn main_chat_tool_loop_model_selected_disallowed_tool_is_governed_blocker() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        grant_builtin_echo(&state).await;
        configure_scripted_tool_loop_scheduler(
            &state,
            "gpt-tool-loop-disallowed",
            "file.write",
            "mcp_tool",
        )
        .await;

        let response = run_tool_loop_send(
            state,
            "tool-loop-disallowed",
            "Use mcp builtin_echo read-only now.",
        )
        .await;

        let generation = generation_result(&response);
        assert_eq!(
            generation["toolLoopOutcome"],
            serde_json::json!("governed_blocker")
        );
        assert!(response.reply.contains("model_selected_disallowed_tool"));
    }

    #[test]
    fn main_chat_tool_loop_policy_blocked_selected_candidate_classifies_as_blocker() {
        assert_eq!(
            classify_main_chat_tool_loop_agent_loop_status(ExecutionQueueStatus::Failed),
            MainChatToolLoopOutcomeKind::GovernedBlocker
        );
    }

    #[test]
    fn main_chat_tool_loop_single_step_fallback_is_explicit_outcome() {
        assert_eq!(
            classify_main_chat_tool_loop_single_step_status(
                &openlife_core::agent::ActionExecutionStatus::Succeeded,
            ),
            MainChatToolLoopOutcomeKind::SingleStepFallback
        );
    }

    #[test]
    fn main_chat_tool_loop_no_runtime_evidence_cannot_claim_success() {
        assert!(!main_chat_tool_loop_result_has_runtime_evidence(
            MainChatToolLoopOutcomeKind::AgentLoopSuccess,
            &[],
            &[],
            &[],
        ));
    }

    #[test]
    fn main_chat_tool_loop_adapter_does_not_use_action_executor_config_default() {
        let adapter_source = include_str!("main_chat_tool_loop.rs");
        let forbidden_default = ["ActionExecutorConfig", "::default()"].concat();
        assert!(
            !adapter_source.contains(&forbidden_default),
            "ToolLoop adapter must not rely on the default action executor config"
        );
        let runtime_source = include_str!("main_chat_react_runtime.rs");
        let execution_source = include_str!("main_chat_react_execution.rs");
        for (label, source) in [
            ("main_chat_react_runtime", runtime_source),
            ("main_chat_react_execution", execution_source),
        ] {
            assert!(
                source.contains("ActionExecutorConfig {\n            allow_writes: false,"),
                "{label} must construct ActionExecutor with explicit allow_writes=false"
            );
        }
    }
}
