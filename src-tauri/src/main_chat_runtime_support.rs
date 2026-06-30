use crate::{main_chat_context_loader::compile_main_chat_context, AppState};
use openlife_core::agent::main_chat_agent_v1::{
    AgentIngress, AgentIngressDecision, AgentTaskSessionDraft, ExecutionAction, ExecutionPolicy,
    ExecutionQueueStatus, ExecutionTranscriptEntry, ExecutionTranscriptEntryDraft,
    ExecutionTranscriptEntryKind, QueuedExecutionAction,
};
use openlife_core::agent::{AgentRunError, AgentRunStatus, AgentTaskKind};
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct MainChatAgentTurn {
    pub(crate) decision: AgentIngressDecision,
    pub(crate) transcript_entries: Vec<ExecutionTranscriptEntry>,
}

pub(crate) async fn start_main_chat_agent_turn(
    session_id: &str,
    user_msg: Option<&ChatMessage>,
    task_kind: AgentTaskKind,
    state: &Arc<AppState>,
) -> Result<MainChatAgentTurn, String> {
    let user_text = user_msg
        .filter(|message| message.role == "user")
        .map(|message| message.content.as_str())
        .unwrap_or_default();
    let ingress = AgentIngress::default();
    let decision = ingress.decide(session_id, user_text, None, task_kind);
    let mut transcript_entries = Vec::new();

    if let Some(task_session_id) = decision.agent_task_session_id.as_deref() {
        if let Some(ref store_arc) = state.main_chat_agent_session_store {
            let store = store_arc.lock().await;
            match store.load_session(task_session_id) {
                Ok(Some(_)) => {
                    if let Err(err) = store.resume_session(task_session_id) {
                        log::warn!("[MainChatAgent] resume session failed: {}", err);
                    }
                }
                Ok(None) => {
                    if let Err(err) = store.create_session(AgentTaskSessionDraft {
                        chat_session_id: session_id.to_string(),
                        user_goal: if user_text.trim().is_empty() {
                            "Main Chat request".into()
                        } else {
                            user_text.to_string()
                        },
                        selected_strategy: decision.selected_strategy,
                        current_plan_summary: None,
                        context_snapshot_refs: Vec::new(),
                    }) {
                        log::warn!("[MainChatAgent] create session failed: {}", err);
                    }
                }
                Err(err) => {
                    log::warn!("[MainChatAgent] load session failed: {}", err);
                }
            }
        }

        transcript_entries.extend(
            append_main_chat_agent_transcript(
                state,
                Some(task_session_id),
                ExecutionTranscriptEntryKind::UserInput,
                "User submitted a Main Chat request.",
                serde_json::json!({
                    "chatSessionId": session_id,
                    "userMessagePresent": !user_text.trim().is_empty(),
                    "rawUserTextStored": false,
                }),
            )
            .await,
        );
        transcript_entries.extend(
            append_main_chat_agent_transcript(
                state,
                Some(task_session_id),
                ExecutionTranscriptEntryKind::RouteDecision,
                "AgentIngress selected a Main Chat strategy.",
                serde_json::json!({
                    "requestId": decision.request_id,
                    "selectedStrategy": decision.selected_strategy.as_str(),
                    "confidence": decision.confidence,
                    "fallbackEligible": decision.fallback_eligible,
                    "riskLevel": decision.privacy_risk.risk_level,
                    "privacyClass": decision.privacy_risk.privacy_class,
                    "policyReasonCode": decision.privacy_risk.policy_reason_code,
                    "rawUserTextStored": false,
                }),
            )
            .await,
        );
    }

    Ok(MainChatAgentTurn {
        decision,
        transcript_entries,
    })
}

pub(crate) async fn append_main_chat_agent_transcript(
    state: &Arc<AppState>,
    task_session_id: Option<&str>,
    kind: ExecutionTranscriptEntryKind,
    summary: impl Into<String>,
    metadata: serde_json::Value,
) -> Vec<ExecutionTranscriptEntry> {
    let Some(task_session_id) = task_session_id else {
        return Vec::new();
    };
    let Some(ref store_arc) = state.main_chat_agent_session_store else {
        return Vec::new();
    };
    let store = store_arc.lock().await;
    match store.append_transcript_entry(ExecutionTranscriptEntryDraft {
        session_id: task_session_id.to_string(),
        kind,
        summary: summary.into(),
        metadata,
    }) {
        Ok(entry) => vec![entry],
        Err(err) => {
            log::warn!("[MainChatAgent] append transcript failed: {}", err);
            Vec::new()
        }
    }
}

pub(crate) async fn append_main_chat_direct_answer_contract_transcript(
    state: &Arc<AppState>,
    main_chat_agent_turn: &MainChatAgentTurn,
    user_text: &str,
    selected_skill_id: Option<&str>,
) -> Vec<ExecutionTranscriptEntry> {
    let Some(task_session_id) = main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref()
    else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    entries.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::Plan,
            "DirectAnswer prompt contract was prepared.",
            serde_json::json!({
                "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                "promptContract": "direct_answer_reflex",
                "toolExecutionAllowed": false,
                "writeExecutionAllowed": false,
                "silentWritesAllowed": false,
                "legacyFallbackUsed": false,
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
    entries.extend(
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
    entries
}

pub(crate) async fn complete_main_chat_agent_turn_session(
    state: &Arc<AppState>,
    main_chat_agent_turn: &MainChatAgentTurn,
    final_summary: &str,
) {
    let Some(task_session_id) = main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref()
    else {
        return;
    };
    let Some(ref store_arc) = state.main_chat_agent_session_store else {
        return;
    };
    let store = store_arc.lock().await;
    if let Err(err) = store.complete_session(task_session_id, final_summary) {
        log::warn!(
            "[MainChatAgent] complete direct answer session failed: {}",
            err
        );
    }
}

pub(crate) async fn enqueue_main_chat_agent_action(
    state: &Arc<AppState>,
    task_session_id: &str,
    action_type: &str,
    description: &str,
    execution_transcript: &mut Vec<ExecutionTranscriptEntry>,
) -> Result<QueuedExecutionAction, String> {
    let policy = ExecutionPolicy.classify(&ExecutionAction::new(
        action_type.to_string(),
        description.to_string(),
    ));
    let queue_arc = state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "Main Chat action queue store not available".to_string())?;
    let queue = queue_arc.lock().await;
    let queued = queue
        .enqueue(
            task_session_id,
            ExecutionAction::new(action_type.to_string(), description.to_string()),
            policy,
        )
        .map_err(|err| format!("enqueue Main Chat action failed: {err}"))?;
    drop(queue);

    if let Some(ref store_arc) = state.main_chat_agent_session_store {
        let store = store_arc.lock().await;
        if let Err(err) = store.record_action_queue_id(task_session_id, &queued.id) {
            log::warn!("[MainChatAgent] record action id failed: {}", err);
        }
    }

    execution_transcript.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::Action,
            "Execution action entered the governed queue.",
            serde_json::json!({
                "actionId": queued.id,
                "actionType": queued.action.action_type,
                "queueStatus": queued.status,
                "policyLevel": queued.policy.level.as_str(),
                "policyReasonCode": queued.policy.reason_code,
                "executionAllowed": queued.policy.execution_allowed,
                "requiresProposal": queued.policy.requires_proposal,
                "requiresConfirmation": queued.policy.requires_confirmation,
                "silentWriteAllowed": queued.policy.silent_write_allowed,
            }),
        )
        .await,
    );
    Ok(queued)
}

pub(crate) async fn transition_main_chat_action(
    state: &Arc<AppState>,
    action_id: &str,
    status: ExecutionQueueStatus,
    observation_metadata: Option<serde_json::Value>,
) -> Result<(), String> {
    let queue_arc = state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "Main Chat action queue store not available".to_string())?;
    let queue = queue_arc.lock().await;
    queue
        .transition(action_id, status, observation_metadata)
        .map_err(|err| format!("transition Main Chat action failed: {err}"))?;
    Ok(())
}

pub(crate) async fn fail_main_chat_action(
    state: &Arc<AppState>,
    action_id: &str,
    error: &str,
    observation_metadata: serde_json::Value,
) -> Result<(), String> {
    let queue_arc = state
        .main_chat_action_queue_store
        .as_ref()
        .ok_or_else(|| "Main Chat action queue store not available".to_string())?;
    let queue = queue_arc.lock().await;
    queue
        .fail(action_id, error.to_string(), Some(observation_metadata))
        .map_err(|err| format!("fail Main Chat action failed: {err}"))?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainChatTaskFailureKind {
    Timeout,
    Cancelled,
    ProviderError,
    ToolError,
    PolicyBlocker,
    UnknownError,
}

impl MainChatTaskFailureKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::ProviderError => "provider_error",
            Self::ToolError => "tool_error",
            Self::PolicyBlocker => "policy_blocker",
            Self::UnknownError => "unknown_error",
        }
    }

    pub(crate) fn normalized_lifecycle_state(self) -> &'static str {
        match self {
            Self::Timeout => "timed_out",
            Self::Cancelled => "cancelled",
            Self::PolicyBlocker => "blocked",
            Self::ProviderError | Self::ToolError | Self::UnknownError => "failed",
        }
    }

    fn run_error_phase(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::ProviderError => "provider_error",
            Self::ToolError => "tool_error",
            Self::PolicyBlocker => "policy_blocker",
            Self::UnknownError => "unknown_error",
        }
    }

    fn recoverable(self) -> bool {
        !matches!(self, Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatTaskFailureFinalization {
    pub(crate) run_id: Option<String>,
    pub(crate) task_session_id: Option<String>,
    pub(crate) lifecycle_state: String,
    pub(crate) transcript_entry_id: Option<String>,
}

pub(crate) async fn finalize_main_chat_task_failure(
    state: &Arc<AppState>,
    run_id: Option<&str>,
    task_session_id: Option<&str>,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &str,
    source_ref: &str,
) -> Result<MainChatTaskFailureFinalization, String> {
    let safe_reason = metadata_safe_failure_label(safe_reason, 240);
    let source_ref = metadata_safe_failure_label(source_ref, 120);
    let resolved_task_session_id = task_session_id
        .map(|value| metadata_safe_failure_label(value, 96))
        .filter(|value| !value.is_empty());
    let mut resolved_run_id = run_id
        .map(|value| metadata_safe_failure_label(value, 96))
        .filter(|value| !value.is_empty());

    let task_session = if let Some(task_id) = resolved_task_session_id.as_deref() {
        load_main_chat_task_session_for_failure(state, task_id).await?
    } else {
        None
    };

    if resolved_run_id.is_none() {
        if let Some(task_id) = resolved_task_session_id.as_deref() {
            resolved_run_id = run_id_from_main_chat_failure_transcript(state, task_id).await?;
        }
    }
    if resolved_run_id.is_none() {
        if let Some(session) = task_session.as_ref() {
            resolved_run_id =
                latest_nonterminal_agent_run_for_chat_session(state, &session.chat_session_id)
                    .await?;
        }
    }

    let route_evidence =
        runtime_route_evidence_value_for_run_id(state, resolved_run_id.as_deref()).await?;
    let route_evidence_ref = route_evidence
        .as_ref()
        .and_then(|value| value.get("evidence_id"))
        .and_then(serde_json::Value::as_str)
        .map(|value| metadata_safe_failure_label(value, 160));

    finalize_agent_run_failure(
        state,
        resolved_run_id.as_deref(),
        failure_kind,
        &safe_reason,
    )
    .await?;
    finalize_task_session_failure(
        state,
        resolved_task_session_id.as_deref(),
        failure_kind,
        &safe_reason,
    )
    .await?;

    let transcript_entry_id =
        append_failure_finalizer_transcript(FailureFinalizerTranscriptInput {
            state,
            run_id: resolved_run_id.as_deref(),
            task_session_id: resolved_task_session_id.as_deref(),
            failure_kind,
            safe_reason: &safe_reason,
            source_ref: &source_ref,
            route_evidence,
            route_evidence_ref,
        })
        .await?;

    Ok(MainChatTaskFailureFinalization {
        run_id: resolved_run_id,
        task_session_id: resolved_task_session_id,
        lifecycle_state: failure_kind.normalized_lifecycle_state().to_string(),
        transcript_entry_id,
    })
}

async fn load_main_chat_task_session_for_failure(
    state: &Arc<AppState>,
    task_session_id: &str,
) -> Result<Option<openlife_core::agent::main_chat_agent_v1::AgentTaskSession>, String> {
    let Some(ref store_arc) = state.main_chat_agent_session_store else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    store
        .load_session(task_session_id)
        .map_err(|err| format!("load Main Chat task for failure finalizer failed: {err}"))
}

async fn run_id_from_main_chat_failure_transcript(
    state: &Arc<AppState>,
    task_session_id: &str,
) -> Result<Option<String>, String> {
    let Some(ref store_arc) = state.main_chat_agent_session_store else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    let transcript = store
        .list_transcript_entries(task_session_id)
        .map_err(|err| format!("load Main Chat transcript for failure finalizer failed: {err}"))?;
    Ok(transcript
        .iter()
        .rev()
        .find_map(|entry| string_from_failure_metadata(&entry.metadata, &["runId", "run_id"])))
}

async fn latest_nonterminal_agent_run_for_chat_session(
    state: &Arc<AppState>,
    chat_session_id: &str,
) -> Result<Option<String>, String> {
    let Some(ref store_arc) = state.agent_run_store else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    let runs = store
        .list_runs_for_session(chat_session_id, 10)
        .map_err(|err| format!("list AgentRuns for failure finalizer failed: {err}"))?;
    Ok(runs
        .into_iter()
        .find(|run| run.status != AgentRunStatus::Completed)
        .map(|run| run.id))
}

async fn runtime_route_evidence_value_for_run_id(
    state: &Arc<AppState>,
    run_id: Option<&str>,
) -> Result<Option<serde_json::Value>, String> {
    let Some(run_id) = run_id else {
        return Ok(None);
    };
    let Some(ref store_arc) = state.agent_run_store else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    let Some(run) = store
        .get_run(run_id)
        .map_err(|err| format!("load AgentRun route evidence failed: {err}"))?
    else {
        return Ok(None);
    };
    Ok(run
        .reasoning_trace
        .as_ref()
        .and_then(|trace| trace.generation_result.as_ref())
        .and_then(|value| value.get("runtimeRouteEvidence").cloned()))
}

async fn finalize_agent_run_failure(
    state: &Arc<AppState>,
    run_id: Option<&str>,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &str,
) -> Result<(), String> {
    let Some(run_id) = run_id else {
        return Ok(());
    };
    let Some(ref store_arc) = state.agent_run_store else {
        return Ok(());
    };
    let store = store_arc.lock().await;
    let Some(mut run) = store
        .get_run(run_id)
        .map_err(|err| format!("load AgentRun for failure finalizer failed: {err}"))?
    else {
        return Ok(());
    };
    if run.status == AgentRunStatus::Completed {
        return Ok(());
    }

    match failure_kind {
        MainChatTaskFailureKind::Cancelled => {
            if run.status != AgentRunStatus::Cancelled {
                run.cancel();
            }
        }
        _ => {
            run.fail(AgentRunError {
                message: safe_reason.to_string(),
                phase: failure_kind.run_error_phase().to_string(),
                recoverable: failure_kind.recoverable(),
            });
        }
    }
    store
        .update_run(&run)
        .map_err(|err| format!("update AgentRun failure finalizer failed: {err}"))
}

async fn finalize_task_session_failure(
    state: &Arc<AppState>,
    task_session_id: Option<&str>,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &str,
) -> Result<(), String> {
    let Some(task_session_id) = task_session_id else {
        return Ok(());
    };
    let Some(ref store_arc) = state.main_chat_agent_session_store else {
        return Ok(());
    };
    let store = store_arc.lock().await;
    let Some(session) = store
        .load_session(task_session_id)
        .map_err(|err| format!("load task session for failure finalizer failed: {err}"))?
    else {
        return Ok(());
    };
    if matches!(
        session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
    ) {
        return Ok(());
    }
    if session.status == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled
        && failure_kind != MainChatTaskFailureKind::Cancelled
    {
        return Ok(());
    }

    match failure_kind {
        MainChatTaskFailureKind::Cancelled => {
            store
                .cancel_session(task_session_id, safe_reason)
                .map(|_| ())
                .map_err(|err| format!("cancel task failure finalizer failed: {err}"))?;
        }
        MainChatTaskFailureKind::PolicyBlocker => {
            let mut blockers = session.pending_blockers.clone();
            blockers.push(safe_reason.to_string());
            blockers.sort();
            blockers.dedup();
            store
                .set_pending_blockers(task_session_id, blockers)
                .map_err(|err| format!("set policy blocker finalizer failed: {err}"))?;
            store
                .block_session(task_session_id, safe_reason)
                .map(|_| ())
                .map_err(|err| format!("block task failure finalizer failed: {err}"))?;
        }
        MainChatTaskFailureKind::Timeout
        | MainChatTaskFailureKind::ProviderError
        | MainChatTaskFailureKind::ToolError
        | MainChatTaskFailureKind::UnknownError => {
            store
                .fail_session(task_session_id, safe_reason)
                .map(|_| ())
                .map_err(|err| format!("fail task failure finalizer failed: {err}"))?;
        }
    }
    Ok(())
}

struct FailureFinalizerTranscriptInput<'a> {
    state: &'a Arc<AppState>,
    run_id: Option<&'a str>,
    task_session_id: Option<&'a str>,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &'a str,
    source_ref: &'a str,
    route_evidence: Option<serde_json::Value>,
    route_evidence_ref: Option<String>,
}

async fn append_failure_finalizer_transcript(
    input: FailureFinalizerTranscriptInput<'_>,
) -> Result<Option<String>, String> {
    let Some(task_session_id) = input.task_session_id else {
        return Ok(None);
    };
    let Some(ref store_arc) = input.state.main_chat_agent_session_store else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    let existing = store
        .list_transcript_entries(task_session_id)
        .map_err(|err| format!("load transcript before failure finalizer failed: {err}"))?;
    if let Some(entry) = existing.iter().rev().find(|entry| {
        string_from_failure_metadata(&entry.metadata, &["failureKind", "failure_kind"]).as_deref()
            == Some(input.failure_kind.as_str())
            && string_from_failure_metadata(&entry.metadata, &["sourceRef", "source_ref"])
                .as_deref()
                == Some(input.source_ref)
    }) {
        return Ok(Some(entry.id.clone()));
    }

    let mut metadata = serde_json::json!({
        "failureKind": input.failure_kind.as_str(),
        "failure_kind": input.failure_kind.as_str(),
        "normalizedLifecycleState": input.failure_kind.normalized_lifecycle_state(),
        "normalized_lifecycle_state": input.failure_kind.normalized_lifecycle_state(),
        "safeReason": input.safe_reason,
        "safe_reason": input.safe_reason,
        "sourceRef": input.source_ref,
        "source_ref": input.source_ref,
        "directWritesExecuted": false,
    });
    if let Some(run_id) = input.run_id {
        metadata["runId"] = serde_json::json!(run_id);
        metadata["run_id"] = serde_json::json!(run_id);
    }
    if let Some(route_evidence_ref) = input.route_evidence_ref {
        metadata["routeEvidenceRef"] = serde_json::json!(route_evidence_ref);
    } else if let Some(run_id) = input.run_id {
        metadata["routeEvidenceRef"] =
            serde_json::json!(format!("agent_run:{run_id}:runtimeRouteEvidence"));
    }
    if let Some(route_evidence) = input.route_evidence {
        metadata["routeEvidence"] = route_evidence;
    }

    let summary = match input.failure_kind {
        MainChatTaskFailureKind::Timeout => "Main Chat task timed out and was finalized.",
        MainChatTaskFailureKind::Cancelled => "Main Chat task was cancelled and finalized.",
        MainChatTaskFailureKind::ProviderError => {
            "Main Chat task failed because the provider path returned an error."
        }
        MainChatTaskFailureKind::ToolError => {
            "Main Chat task failed because a governed tool returned an error."
        }
        MainChatTaskFailureKind::PolicyBlocker => {
            "Main Chat task was blocked by governed policy evidence."
        }
        MainChatTaskFailureKind::UnknownError => {
            "Main Chat task failed with an unknown governed runtime error."
        }
    };
    let entry = store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: task_session_id.to_string(),
            kind: ExecutionTranscriptEntryKind::Error,
            summary: summary.to_string(),
            metadata,
        })
        .map_err(|err| format!("append failure finalizer transcript failed: {err}"))?;
    Ok(Some(entry.id))
}

fn string_from_failure_metadata(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let object = value.as_object()?;
    for key in keys {
        if let Some(value) = object.get(*key).and_then(serde_json::Value::as_str) {
            return Some(value.to_string());
        }
    }
    None
}

fn metadata_safe_failure_label(value: &str, max_chars: usize) -> String {
    let collapsed = value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let mut output = collapsed
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    output.push_str("...");
    output
}
