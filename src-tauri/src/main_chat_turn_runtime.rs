use std::sync::Arc;

use openlife_core::agent::main_chat_agent_v1::{
    AgentIngressDecision, MainChatAgentStrategy, PolicyRouteKind,
};
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::main_chat_event_stream::MainChatAgentDurableEvent;
use crate::main_chat_kernel::{
    main_chat_kernel_support_disposition,
    main_chat_live_provider_eval_requires_provider_backed_react,
    main_chat_react_turn_requires_governed_agent_loop_candidate_selection,
    run_main_chat_kernel_direct_answer_with_state, BufferedMainChatEventSink,
    MainChatKernelSupportDisposition, StreamingMainChatEventSink,
};
use crate::main_chat_runtime_support::start_main_chat_agent_turn;
use crate::{AppState, SendMessageResult};

pub(crate) const OPENLIFE_TURN_RUNTIME_OWNER: &str = "OpenLifeTurnRuntime";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MainChatExecutionPath {
    DirectAnswer,
    ReadOnlyTool,
    WriteOutcome,
    PlanExecute,
    GovernedBlocker,
}

impl MainChatExecutionPath {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DirectAnswer => "DirectAnswer",
            Self::ReadOnlyTool => "ReadOnlyTool",
            Self::WriteOutcome => "WriteOutcome",
            Self::PlanExecute => "PlanExecute",
            Self::GovernedBlocker => "GovernedBlocker",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatTurnRouteDecision {
    pub(crate) path: MainChatExecutionPath,
    pub(crate) strategy_label: String,
    pub(crate) reason_code: String,
    pub(crate) kernel_supported: bool,
    pub(crate) kernel_support_disposition: String,
    pub(crate) fallback_allowed: bool,
    pub(crate) requires_provider: bool,
    pub(crate) requires_tool_loop: bool,
    pub(crate) live_provider_backed_react_required: bool,
    pub(crate) governed_agent_loop_candidate_selection_required: bool,
}

impl MainChatTurnRouteDecision {
    pub(crate) fn execution_path_label(&self) -> &'static str {
        self.path.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MainChatTurnStreamMode {
    Buffered,
    Streaming,
}

impl MainChatTurnStreamMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Buffered => "buffered",
            Self::Streaming => "streaming",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenLifeTurnInput {
    pub(crate) session_id: String,
    pub(crate) messages: Vec<ChatMessage>,
    #[serde(default)]
    pub(crate) selected_skill_id: Option<String>,
    pub(crate) stream_mode: MainChatTurnStreamMode,
}

pub(crate) struct OpenLifeTurnOutput {
    pub(crate) route_decision: MainChatTurnRouteDecision,
    pub(crate) terminal: OpenLifeTurnTerminal,
    pub(crate) delivery: MainChatTurnDelivery,
}

pub(crate) enum MainChatTurnDelivery {
    Buffered {
        result: Box<SendMessageResult>,
    },
    Streamed {
        run_id: Option<String>,
        legacy_fallback_used: bool,
        kernel_event_count: Option<usize>,
        durable_event_count: usize,
        done_payload: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenLifeTurnTerminal {
    pub runtime_owner: String,
    pub status: String,
    pub state: String,
    pub final_delivery: CanonicalFinalDeliveryView,
    pub run_id: Option<String>,
    pub task_session_id: Option<String>,
    pub blockers: Vec<String>,
    pub proposals: Vec<String>,
    pub legacy_fallback_used: bool,
    pub legacy_runtime_invoked: bool,
    pub single_step_fallback_used: bool,
    pub direct_writes_executed: bool,
    pub model_invoked: bool,
    pub tool_invoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalFinalDeliveryView {
    pub delivery_id: String,
    pub task_id: String,
    pub run_id: String,
    pub status: String,
    pub headline: String,
    pub answer: String,
    pub completed_actions: Vec<CanonicalCompletedActionSummary>,
    pub observations_used: Vec<CanonicalObservationSummary>,
    pub proposals_created: Vec<CanonicalProposalSummary>,
    pub blockers: Vec<CanonicalBlockerSummary>,
    pub pending_user_actions: Vec<CanonicalPendingUserActionSummary>,
    pub durable_changes: Vec<CanonicalDurableChangeSummary>,
    pub next_steps: Vec<String>,
    pub trace_available: bool,
    pub kernel_event_count: Option<usize>,
    pub durable_event_count: usize,
    pub reply_preview: String,
    pub has_assistant_message: bool,
    pub tool_call_count: usize,
    pub blocker_count: usize,
    pub proposal_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalCompletedActionSummary {
    pub action_id: String,
    pub action_type: String,
    pub tool_label: String,
    pub target: String,
    pub status: String,
    pub observation_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalObservationSummary {
    pub observation_id: String,
    pub source_kind: String,
    pub source_label: String,
    pub preview: String,
    pub citation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalProposalSummary {
    pub proposal_id: String,
    pub proposal_type: String,
    pub status: String,
    pub summary: String,
    pub review_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalBlockerSummary {
    pub blocker_id: String,
    pub reason_code: String,
    pub affected_action_or_step: Option<String>,
    pub user_resolvable: bool,
    pub valid_next_controls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalPendingUserActionSummary {
    pub pending_id: String,
    pub kind: String,
    pub summary: String,
    pub controls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalDurableChangeSummary {
    pub change_type: String,
    pub target: String,
    pub provenance: String,
    pub timestamp: Option<String>,
    pub rollback_available: bool,
}

pub(crate) struct OpenLifeTurnRuntime<'a> {
    state: &'a Arc<AppState>,
}

struct OpenLifeKernelExecution {
    session_id: String,
    route_decision: MainChatTurnRouteDecision,
    terminal: OpenLifeTurnTerminal,
    result: SendMessageResult,
    run_id: Option<String>,
    legacy_fallback_used: bool,
    kernel_event_count: usize,
    durable_events: Vec<MainChatAgentDurableEvent>,
}

impl<'a> OpenLifeTurnRuntime<'a> {
    pub(crate) fn new(state: &'a Arc<AppState>) -> Self {
        Self { state }
    }

    pub(crate) async fn run_buffered(
        &self,
        input: OpenLifeTurnInput,
    ) -> Result<OpenLifeTurnOutput, String> {
        debug_assert_eq!(input.stream_mode, MainChatTurnStreamMode::Buffered);
        let mut event_sink = BufferedMainChatEventSink::default();
        let execution = self
            .run_with_event_sink(input, &mut event_sink, MainChatTurnStreamMode::Buffered)
            .await?;
        Ok(OpenLifeTurnOutput {
            route_decision: execution.route_decision,
            terminal: execution.terminal,
            delivery: MainChatTurnDelivery::Buffered {
                result: Box::new(execution.result),
            },
        })
    }

    pub(crate) async fn run_streaming(
        &self,
        input: OpenLifeTurnInput,
        emit_stream_event: &mut (impl FnMut(&str, serde_json::Value) + Send),
    ) -> Result<OpenLifeTurnOutput, String> {
        debug_assert_eq!(input.stream_mode, MainChatTurnStreamMode::Streaming);
        let execution = {
            let mut event_sink = StreamingMainChatEventSink::new(emit_stream_event);
            self.run_with_event_sink(input, &mut event_sink, MainChatTurnStreamMode::Streaming)
                .await?
        };
        let durable_event_count = execution.durable_events.len();
        let done_payload = emit_stream_send_message_result(
            &execution.session_id,
            execution.result,
            Some(execution.kernel_event_count),
            execution.durable_events,
            false,
            emit_stream_event,
        )?;
        Ok(OpenLifeTurnOutput {
            route_decision: execution.route_decision,
            terminal: execution.terminal,
            delivery: MainChatTurnDelivery::Streamed {
                run_id: execution.run_id,
                legacy_fallback_used: execution.legacy_fallback_used,
                kernel_event_count: Some(execution.kernel_event_count),
                durable_event_count,
                done_payload,
            },
        })
    }

    async fn run_with_event_sink<S>(
        &self,
        input: OpenLifeTurnInput,
        event_sink: &mut S,
        stream_mode: MainChatTurnStreamMode,
    ) -> Result<OpenLifeKernelExecution, String>
    where
        S: crate::main_chat_kernel::MainChatEventSink + ?Sized,
    {
        let OpenLifeTurnInput {
            session_id,
            messages,
            selected_skill_id,
            stream_mode: _,
        } = input;
        let user_msg = messages.last().cloned();
        let main_chat_agent_turn = start_main_chat_agent_turn(
            &session_id,
            user_msg.as_ref(),
            openlife_core::agent::AgentTaskKind::Conversation,
            self.state,
        )
        .await?;

        let route_decision =
            decide_main_chat_turn_route(&main_chat_agent_turn.decision, &messages, self.state)
                .await;
        let command_result = run_main_chat_kernel_direct_answer_with_state(
            &session_id,
            messages,
            selected_skill_id,
            self.state,
            &main_chat_agent_turn,
            event_sink,
            stream_mode.as_str(),
        )
        .await?;
        let kernel_event_count = command_result.kernel_events.len();
        crate::main_chat_runtime_status::record_main_chat_kernel_event_count(
            self.state,
            kernel_event_count,
        )
        .await;
        crate::main_chat_runtime_status::record_main_chat_turn_route_evidence(
            self.state,
            &route_decision,
            stream_mode,
            false,
            false,
            Some(kernel_event_count),
        )
        .await;

        let durable_events = command_result.durable_events.clone();
        let durable_event_count = durable_events.len();
        let run_id = command_result.run_id.clone();
        let legacy_fallback_used = command_result.legacy_fallback_used;
        let mut result = command_result.into_send_message_result();
        let terminal = finalize_openlife_turn_result(
            &route_decision,
            &mut result,
            Some(kernel_event_count),
            durable_event_count,
        );

        Ok(OpenLifeKernelExecution {
            session_id,
            route_decision,
            terminal,
            result,
            run_id,
            legacy_fallback_used,
            kernel_event_count,
            durable_events,
        })
    }
}

pub(crate) async fn decide_main_chat_turn_route(
    agent_decision: &AgentIngressDecision,
    messages: &[ChatMessage],
    state: &Arc<AppState>,
) -> MainChatTurnRouteDecision {
    let selected_strategy = agent_decision.selected_strategy;
    let kernel_support_disposition =
        main_chat_kernel_support_disposition(&selected_strategy, messages);
    let live_provider_backed_react_required =
        main_chat_live_provider_eval_requires_provider_backed_react(&selected_strategy, state)
            .await;
    let governed_agent_loop_candidate_selection_required =
        main_chat_react_turn_requires_governed_agent_loop_candidate_selection(
            &selected_strategy,
            messages,
            state,
        )
        .await;

    decide_main_chat_turn_route_from_disposition(
        agent_decision.policy_route,
        selected_strategy,
        kernel_support_disposition,
        live_provider_backed_react_required,
        governed_agent_loop_candidate_selection_required,
    )
}

pub(crate) fn decide_main_chat_turn_route_from_disposition(
    policy_route: PolicyRouteKind,
    selected_strategy: MainChatAgentStrategy,
    kernel_support_disposition: MainChatKernelSupportDisposition,
    live_provider_backed_react_required: bool,
    governed_agent_loop_candidate_selection_required: bool,
) -> MainChatTurnRouteDecision {
    let (path, reason_code) = match kernel_support_disposition {
        MainChatKernelSupportDisposition::GovernedBlocker => (
            MainChatExecutionPath::GovernedBlocker,
            "openlife_runtime_governed_blocker",
        ),
        MainChatKernelSupportDisposition::KernelSupported => match policy_route {
            PolicyRouteKind::DirectAnswer => (
                MainChatExecutionPath::DirectAnswer,
                "openlife_runtime_direct_answer",
            ),
            PolicyRouteKind::ReadOnlyTool => (
                MainChatExecutionPath::ReadOnlyTool,
                "openlife_runtime_read_only_tool",
            ),
            PolicyRouteKind::PlanDraft => (
                MainChatExecutionPath::PlanExecute,
                "openlife_runtime_plan_execute",
            ),
            PolicyRouteKind::ProposalOnlyWrite => (
                MainChatExecutionPath::WriteOutcome,
                "openlife_runtime_proposal_only_write",
            ),
            PolicyRouteKind::ConfirmationRequest => (
                MainChatExecutionPath::WriteOutcome,
                "openlife_runtime_confirmation_request",
            ),
            PolicyRouteKind::AskClarification => (
                MainChatExecutionPath::DirectAnswer,
                "openlife_runtime_ask_clarification",
            ),
            PolicyRouteKind::GovernedBlocker => (
                MainChatExecutionPath::GovernedBlocker,
                "openlife_runtime_governed_blocker",
            ),
        },
    };

    MainChatTurnRouteDecision {
        path,
        strategy_label: selected_strategy.as_str().into(),
        reason_code: reason_code.into(),
        kernel_supported: true,
        kernel_support_disposition: kernel_support_disposition.as_str().into(),
        fallback_allowed: false,
        requires_provider: live_provider_backed_react_required,
        requires_tool_loop: false,
        live_provider_backed_react_required,
        governed_agent_loop_candidate_selection_required,
    }
}

pub(crate) fn finalize_openlife_turn_result(
    route_decision: &MainChatTurnRouteDecision,
    result: &mut SendMessageResult,
    kernel_event_count: Option<usize>,
    durable_event_count: usize,
) -> OpenLifeTurnTerminal {
    let generation = result.reasoning_trace.generation_result.as_ref();
    let mut blockers = result.blockers.clone();
    if blockers.is_empty() {
        blockers = string_array_from_generation(generation, "blockers");
    }
    let proposals = proposal_ids_from_result(result, generation);
    let pending_blocker_count = generation
        .and_then(|value| value.get("pendingBlockerCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let single_step_fallback_used = false;
    let direct_writes_executed = generation_flag(generation, "directWritesExecuted");
    let model_invoked = generation_flag(generation, "modelGenerated")
        || generation_flag(generation, "schedulerGenerationCalled");
    let tool_invoked = !result.tool_calls.is_empty()
        || generation
            .and_then(|value| value.get("toolCallCount"))
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0);
    let pending_permission = result.tool_calls.iter().any(|call| {
        call.requires_confirmation
            || matches!(call.status, crate::ToolCallStatus::NeedsConfirmation)
    });
    let status = if result.legacy_fallback_used
        || result.legacy_runtime_invoked
        || single_step_fallback_used
    {
        "failed"
    } else if result.status == "cancelled" {
        "cancelled"
    } else if !blockers.is_empty() || pending_blocker_count > 0 || pending_permission {
        "blocked"
    } else if result.status == "failed" {
        "failed"
    } else if !proposals.is_empty() {
        "completed_with_pending_items"
    } else {
        "completed"
    }
    .to_string();

    result.status = status.clone();
    result.blockers = blockers.clone();
    result.model_invoked = model_invoked;
    result.tool_invoked = tool_invoked;
    let task_session_id = result
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.clone());
    let final_delivery = canonical_final_delivery_from_result(
        route_decision,
        result,
        &status,
        &blockers,
        &proposals,
        task_session_id.as_deref(),
        kernel_event_count,
        durable_event_count,
    );
    result.turn_terminal = Some(OpenLifeTurnTerminal {
        runtime_owner: OPENLIFE_TURN_RUNTIME_OWNER.into(),
        status: status.clone(),
        state: route_decision.path.as_str().into(),
        final_delivery,
        run_id: result.run_id.clone(),
        task_session_id,
        blockers,
        proposals,
        legacy_fallback_used: result.legacy_fallback_used,
        legacy_runtime_invoked: result.legacy_runtime_invoked,
        single_step_fallback_used,
        direct_writes_executed,
        model_invoked,
        tool_invoked,
    });

    result
        .turn_terminal
        .clone()
        .expect("terminal set by OpenLifeTurnRuntime")
}

fn emit_stream_send_message_result(
    session_id: &str,
    result: SendMessageResult,
    kernel_event_count: Option<usize>,
    durable_events: Vec<MainChatAgentDurableEvent>,
    emit_empty_chunk: bool,
    emit_stream_event: &mut (impl FnMut(&str, serde_json::Value) + Send),
) -> Result<serde_json::Value, String> {
    let run_id = result.run_id.clone().unwrap_or_default();
    let agent_state = result.agent_state.clone();
    let mut start_payload = serde_json::json!({
        "session_id": session_id,
        "run_id": run_id,
        "reasoning_trace": result.reasoning_trace.clone(),
        "tool_calls": result.tool_calls.clone(),
        "agent_ingress": result.agent_ingress.clone(),
        "agent_state": agent_state.clone(),
        "execution_transcript": result.execution_transcript.clone(),
        "legacy_fallback_used": result.legacy_fallback_used,
        "turn_terminal": result.turn_terminal.clone(),
    });
    if let Some(count) = kernel_event_count {
        start_payload["kernel_event_count"] = serde_json::json!(count);
    }
    emit_stream_event("stream-message-start", start_payload);
    if emit_empty_chunk || !result.reply.is_empty() {
        emit_stream_event(
            "stream-message-chunk",
            serde_json::json!({
                "session_id": session_id,
                "run_id": run_id,
                "chunk": result.reply.clone(),
            }),
        );
    }
    let result_status = result.status.clone();
    let result_blockers = result.blockers.clone();
    let legacy_runtime_invoked = result.legacy_runtime_invoked;
    let model_invoked = result.model_invoked;
    let tool_invoked = result.tool_invoked;
    let mut done_payload = serde_json::json!({
        "session_id": session_id,
        "run_id": run_id,
        "reply": result.reply,
        "reasoning_trace": result.reasoning_trace,
        "tool_calls": result.tool_calls,
        "agent_ingress": result.agent_ingress,
        "agent_state": agent_state,
        "execution_transcript": result.execution_transcript,
        "legacy_fallback_used": result.legacy_fallback_used,
        "status": result_status,
        "blockers": result_blockers,
        "legacy_runtime_invoked": legacy_runtime_invoked,
        "model_invoked": model_invoked,
        "tool_invoked": tool_invoked,
        "turn_terminal": result.turn_terminal,
    });
    if let Some(count) = kernel_event_count {
        done_payload["kernel_event_count"] = serde_json::json!(count);
    }
    emit_stream_event("stream-message-done", done_payload.clone());
    for event in durable_events {
        emit_stream_event(
            "main-chat-agent-event",
            serde_json::to_value(event).map_err(|err| err.to_string())?,
        );
    }
    Ok(done_payload)
}

fn generation_flag(generation: Option<&Value>, key: &str) -> bool {
    generation
        .and_then(|value| value.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn string_array_from_generation(generation: Option<&Value>, key: &str) -> Vec<String> {
    generation
        .and_then(|value| value.get(key))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn proposal_ids_from_result(result: &SendMessageResult, generation: Option<&Value>) -> Vec<String> {
    let mut proposals = string_array_from_generation(generation, "proposalIds");
    if let Some(memory_governance) = generation.and_then(|value| value.get("memoryGovernance")) {
        for key in ["memoryProposalIds", "lifeModelProposalIds"] {
            if let Some(ids) = memory_governance.get(key).and_then(Value::as_array) {
                for proposal_id in ids.iter().filter_map(Value::as_str) {
                    let proposal_ref = format!("proposal:{proposal_id}");
                    if !proposals.contains(&proposal_ref) {
                        proposals.push(proposal_ref);
                    }
                }
            }
        }
    }
    for blocker in &result.blockers {
        if blocker.starts_with("proposal:") && !proposals.contains(blocker) {
            proposals.push(blocker.clone());
        }
    }
    proposals
}

#[allow(clippy::too_many_arguments)]
fn canonical_final_delivery_from_result(
    route_decision: &MainChatTurnRouteDecision,
    result: &SendMessageResult,
    status: &str,
    blockers: &[String],
    proposals: &[String],
    task_session_id: Option<&str>,
    kernel_event_count: Option<usize>,
    durable_event_count: usize,
) -> CanonicalFinalDeliveryView {
    let run_id = result.run_id.clone().unwrap_or_else(|| "unknown".into());
    let task_id = task_session_id.unwrap_or("unknown").to_string();
    let completed_actions = canonical_completed_actions(result);
    let observations_used = canonical_observations_used(result);
    let proposals_created = canonical_proposals_created(proposals);
    let blocker_summaries = canonical_blockers(blockers, result);
    let pending_user_actions =
        canonical_pending_user_actions(proposals, result, &blocker_summaries);

    CanonicalFinalDeliveryView {
        delivery_id: format!("delivery:{run_id}:{task_id}"),
        task_id,
        run_id,
        status: status.into(),
        headline: canonical_delivery_headline(status).into(),
        answer: bounded_preview(&result.reply, 1200),
        completed_actions,
        observations_used,
        proposals_created,
        blockers: blocker_summaries,
        pending_user_actions,
        durable_changes: Vec::new(),
        next_steps: canonical_next_steps(status, route_decision, proposals),
        trace_available: !result.execution_transcript.is_empty()
            || result.reasoning_trace.generation_result.is_some(),
        kernel_event_count,
        durable_event_count,
        reply_preview: bounded_preview(&result.reply, 240),
        has_assistant_message: !result.reply.trim().is_empty(),
        tool_call_count: result.tool_calls.len(),
        blocker_count: blockers.len(),
        proposal_count: proposals.len(),
    }
}

fn canonical_completed_actions(result: &SendMessageResult) -> Vec<CanonicalCompletedActionSummary> {
    result
        .tool_calls
        .iter()
        .enumerate()
        .filter(|(_, call)| call.success && matches!(call.status, crate::ToolCallStatus::Success))
        .map(|(index, call)| {
            let observation_ids = call
                .react_trace
                .as_ref()
                .and_then(|trace| trace.observation_id.clone())
                .map(|id| vec![id])
                .unwrap_or_default();
            CanonicalCompletedActionSummary {
                action_id: call
                    .action_id
                    .clone()
                    .or_else(|| {
                        call.react_trace
                            .as_ref()
                            .map(|trace| trace.action_id.clone())
                    })
                    .unwrap_or_else(|| format!("action:{index}:{}", call.name)),
                action_type: call
                    .react_trace
                    .as_ref()
                    .map(|trace| trace.action_type.clone())
                    .unwrap_or_else(|| "tool".into()),
                tool_label: call.name.clone(),
                target: call
                    .react_trace
                    .as_ref()
                    .map(|trace| trace.tool_name.clone())
                    .unwrap_or_else(|| call.name.clone()),
                status: "succeeded".into(),
                observation_ids,
            }
        })
        .collect()
}

fn canonical_observations_used(result: &SendMessageResult) -> Vec<CanonicalObservationSummary> {
    result
        .tool_calls
        .iter()
        .enumerate()
        .filter(|(_, call)| {
            call.success
                && call
                    .output
                    .as_ref()
                    .is_some_and(|value| !value.trim().is_empty())
        })
        .map(|(index, call)| {
            let observation_id = call
                .react_trace
                .as_ref()
                .and_then(|trace| trace.observation_id.clone())
                .or_else(|| {
                    call.action_id
                        .as_ref()
                        .map(|id| format!("observation:{id}"))
                })
                .unwrap_or_else(|| format!("observation:{index}:{}", call.name));
            CanonicalObservationSummary {
                observation_id,
                source_kind: call
                    .react_trace
                    .as_ref()
                    .map(|trace| trace.tool_source.clone())
                    .unwrap_or_else(|| "tool".into()),
                source_label: call.name.clone(),
                preview: bounded_preview(call.output.as_deref().unwrap_or_default(), 360),
                citation: call.run_id.clone(),
            }
        })
        .collect()
}

fn canonical_proposals_created(proposals: &[String]) -> Vec<CanonicalProposalSummary> {
    proposals
        .iter()
        .map(|proposal| {
            let proposal_id = proposal.strip_prefix("proposal:").unwrap_or(proposal);
            CanonicalProposalSummary {
                proposal_id: proposal_id.into(),
                proposal_type: "review_proposal".into(),
                status: "pending_review".into(),
                summary: format!("Pending review item {proposal_id}"),
                review_ref: format!("review:{proposal_id}"),
            }
        })
        .collect()
}

fn canonical_blockers(
    blockers: &[String],
    result: &SendMessageResult,
) -> Vec<CanonicalBlockerSummary> {
    let mut blocker_summaries = blockers
        .iter()
        .enumerate()
        .map(|(index, blocker)| CanonicalBlockerSummary {
            blocker_id: format!("blocker:{index}:{}", bounded_preview(blocker, 64)),
            reason_code: bounded_preview(blocker, 120),
            affected_action_or_step: None,
            user_resolvable: true,
            valid_next_controls: vec!["inspect".into(), "retry_if_safe".into(), "cancel".into()],
        })
        .collect::<Vec<_>>();

    for call in &result.tool_calls {
        if matches!(call.status, crate::ToolCallStatus::NeedsConfirmation) {
            blocker_summaries.push(CanonicalBlockerSummary {
                blocker_id: format!("blocker:permission:{}", call.name),
                reason_code: call
                    .permission_decision
                    .clone()
                    .unwrap_or_else(|| "tool_permission_required".into()),
                affected_action_or_step: call.action_id.clone(),
                user_resolvable: true,
                valid_next_controls: vec![
                    "review_permission".into(),
                    "deny".into(),
                    "cancel".into(),
                ],
            });
        }
    }
    blocker_summaries
}

fn canonical_pending_user_actions(
    proposals: &[String],
    result: &SendMessageResult,
    blockers: &[CanonicalBlockerSummary],
) -> Vec<CanonicalPendingUserActionSummary> {
    let mut pending = proposals
        .iter()
        .map(|proposal| {
            let proposal_id = proposal.strip_prefix("proposal:").unwrap_or(proposal);
            CanonicalPendingUserActionSummary {
                pending_id: format!("pending:proposal:{proposal_id}"),
                kind: "proposal_review".into(),
                summary: format!(
                    "Review proposal {proposal_id} before any durable change is applied."
                ),
                controls: vec![
                    "accept".into(),
                    "edit".into(),
                    "reject".into(),
                    "postpone".into(),
                ],
            }
        })
        .collect::<Vec<_>>();

    for blocker in blockers {
        if blocker.reason_code.contains("permission") {
            pending.push(CanonicalPendingUserActionSummary {
                pending_id: format!("pending:{}", blocker.blocker_id),
                kind: "permission_review".into(),
                summary: "Review the exact tool permission before continuing.".into(),
                controls: blocker.valid_next_controls.clone(),
            });
        }
    }
    for call in &result.tool_calls {
        if matches!(call.status, crate::ToolCallStatus::NeedsConfirmation) {
            pending.push(CanonicalPendingUserActionSummary {
                pending_id: format!("pending:tool:{}", call.name),
                kind: "permission_review".into(),
                summary: format!("Review permission for {}", call.name),
                controls: vec!["review_permission".into(), "deny".into(), "cancel".into()],
            });
        }
    }
    pending
}

fn canonical_delivery_headline(status: &str) -> &'static str {
    match status {
        "completed" => "Completed",
        "completed_with_pending_items" => "Completed with pending review",
        "blocked" => "Blocked",
        "failed" => "Failed",
        "cancelled" => "Cancelled",
        _ => "Failed",
    }
}

fn canonical_next_steps(
    status: &str,
    route_decision: &MainChatTurnRouteDecision,
    proposals: &[String],
) -> Vec<String> {
    match status {
        "completed" => Vec::new(),
        "completed_with_pending_items" if !proposals.is_empty() => {
            vec!["Review pending proposal items before any durable change is applied.".into()]
        }
        "blocked" => vec![format!(
            "Resolve the blocker for {} or cancel the task.",
            route_decision.path.as_str()
        )],
        "failed" => vec!["Retry after inspecting the runtime trace.".into()],
        "cancelled" => vec!["Start a new request if you still need this work.".into()],
        _ => Vec::new(),
    }
}

fn bounded_preview(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in value.chars().take(max_chars) {
        if ch.is_control() {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
