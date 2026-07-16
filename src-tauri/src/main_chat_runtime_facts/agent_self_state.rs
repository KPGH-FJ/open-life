use openlife_core::agent::main_chat_agent_v1::{
    AgentTaskSession, AgentTaskSessionStatus, ExecutionQueueStatus, ExecutionTranscriptEntry,
    ExecutionTranscriptEntryKind, QueuedExecutionAction,
};
use openlife_core::agent::{AgentProposal, AgentRun, AgentRunStatus, ProposalStatus};
use serde_json::Value;
use std::sync::Arc;

use super::contract::{
    bounded_runtime_fact_label, label_or_unknown, matches_exact_runtime_fact_phrase,
    merge_json_object, trim_outer_punctuation, MainChatAgentSelfStateIntent,
    MainChatRuntimeFactAnswer, MainChatRuntimeFactBinding,
    RUNTIME_FACT_AGENT_SELF_STATE_GENERATION_PATH, RUNTIME_FACT_KEY_AGENT_BLOCKER_CODES,
    RUNTIME_FACT_KEY_AGENT_CHAT_SESSION_ID, RUNTIME_FACT_KEY_AGENT_DELIVERY_STATUS,
    RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS, RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY,
    RUNTIME_FACT_KEY_AGENT_PENDING_PERMISSION_COUNT, RUNTIME_FACT_KEY_AGENT_PENDING_PROPOSAL_COUNT,
    RUNTIME_FACT_KEY_AGENT_RUN_ID, RUNTIME_FACT_KEY_AGENT_TASK_SESSION_ID,
    RUNTIME_FACT_KEY_AGENT_TASK_STATUS, RUNTIME_FACT_KEY_AGENT_TRACE_GAP,
};
use crate::AppState;

#[derive(Debug, Clone)]
struct AgentSelfStateFactSnapshot {
    chat_session_id: String,
    task_session_id: Option<String>,
    run_id: Option<String>,
    task_status: Option<String>,
    run_status: Option<String>,
    delivery_status: String,
    final_delivery_evidence: bool,
    completed_response: bool,
    pending_permission_count: usize,
    pending_permission_target_labels: Vec<String>,
    pending_proposal_count: usize,
    durable_change_status: String,
    durable_change_completed: bool,
    blocker_codes: Vec<String>,
    safe_next_controls: Vec<String>,
    safe_automatic_control_available: bool,
    action_count: usize,
    completed_action_count: usize,
    observation_count: usize,
    transcript_observation_count: usize,
    final_result_count: usize,
    last_action_type: Option<String>,
    last_action_status: Option<String>,
    last_observation_source: Option<String>,
    last_action_summary: Option<String>,
    evidence_labels: Vec<String>,
    trace_gap: bool,
    trace_gap_code: Option<String>,
}

pub(crate) async fn resolve_agent_self_state_fact_answer(
    user_text: &str,
    state: &Arc<AppState>,
    chat_session_id: &str,
    current_task_session_id: Option<&str>,
) -> Option<MainChatRuntimeFactAnswer> {
    let intent = classify_agent_self_state_query(user_text)?;
    let snapshot = agent_self_state_snapshot(state, chat_session_id, current_task_session_id).await;
    let mut fact_keys = intent.fact_keys();
    if snapshot.trace_gap {
        fact_keys.push(RUNTIME_FACT_KEY_AGENT_TRACE_GAP);
    }
    let facts = agent_self_state_fact_bindings(&snapshot, &fact_keys);
    let reply = agent_self_state_reply(intent, &snapshot);
    let ui_primary_source_chip = if snapshot.trace_gap {
        "任务状态未知"
    } else if snapshot.pending_permission_count > 0 {
        "等待确认"
    } else if intent == MainChatAgentSelfStateIntent::AskLastActionSummary
        && snapshot.observation_count > 0
    {
        "工具观察"
    } else if snapshot.pending_proposal_count > 0 {
        "提案待审"
    } else if snapshot.task_status.as_deref() == Some("blocked")
        || !snapshot.blocker_codes.is_empty()
    {
        "已阻塞"
    } else {
        "任务状态"
    };
    let ui_status = if snapshot.trace_gap {
        "unknown"
    } else if snapshot.pending_permission_count > 0 || snapshot.pending_proposal_count > 0 {
        "waiting_for_user"
    } else if snapshot.task_status.as_deref() == Some("blocked") {
        "restricted"
    } else if snapshot.completed_response {
        "completed"
    } else {
        "unknown"
    };
    let source = if snapshot.trace_gap {
        vec!["task_session", "agent_run", "transcript"]
    } else if intent == MainChatAgentSelfStateIntent::AskLastActionSummary {
        vec!["task_session", "action_queue", "transcript"]
    } else if snapshot.pending_permission_count > 0 {
        vec!["task_session", "action_queue", "tool_permission_store"]
    } else if !snapshot.blocker_codes.is_empty() {
        vec!["task_session", "action_queue", "transcript"]
    } else {
        vec!["task_session", "agent_run", "final_delivery", "transcript"]
    };
    let mut extra_metadata = serde_json::json!({
        "providerGenerationPath": RUNTIME_FACT_AGENT_SELF_STATE_GENERATION_PATH,
        "modelGenerated": false,
        "schedulerGenerationCalled": false,
        "toolCalled": false,
        "directWritesExecuted": false,
        "legacyFallbackUsed": false,
        "taskSessionId": snapshot.task_session_id.clone(),
        "runId": snapshot.run_id.clone(),
        "taskStatus": snapshot.task_status.clone(),
        "runStatus": snapshot.run_status.clone(),
        "deliveryStatus": snapshot.delivery_status.clone(),
        "finalDeliveryEvidence": snapshot.final_delivery_evidence,
        "completedResponse": snapshot.completed_response,
        "pendingPermissionCount": snapshot.pending_permission_count,
        "pendingPermissionTargetLabels": snapshot.pending_permission_target_labels.clone(),
        "pendingPermissionTargetLabel": snapshot.pending_permission_target_labels.first().cloned(),
        "pendingProposalCount": snapshot.pending_proposal_count,
        "durableChangeStatus": snapshot.durable_change_status.clone(),
        "durableChangeCompleted": snapshot.durable_change_completed,
        "blockerCodes": snapshot.blocker_codes.clone(),
        "safeNextControls": snapshot.safe_next_controls.clone(),
        "safeAutomaticControlAvailable": snapshot.safe_automatic_control_available,
        "actionCount": snapshot.action_count,
        "completedActionCount": snapshot.completed_action_count,
        "observationCount": snapshot.observation_count,
        "transcriptObservationCount": snapshot.transcript_observation_count,
        "finalResultCount": snapshot.final_result_count,
        "lastActionType": snapshot.last_action_type.clone(),
        "lastActionStatus": snapshot.last_action_status.clone(),
        "lastObservationSource": snapshot.last_observation_source.clone(),
        "lastActionSummary": snapshot.last_action_summary.clone(),
        "selfStateEvidenceLabels": snapshot.evidence_labels.clone(),
        "assistantProseUsedForTaskStatus": false,
        "memoryOrHsOverrideAllowed": false,
        "uiPrimarySourceChip": ui_primary_source_chip,
        "uiStatus": ui_status,
        "runtimeFactTtl": "turn",
        "runtimeFactTtlStatus": if snapshot.trace_gap { "not_observed" } else { "fresh" },
        "runtimeFactMissingBehavior": if snapshot.trace_gap { "trace_gap" } else { "answer_unknown" },
        "runtimeFactModelFallbackAllowed": false,
        "runtimeFactTraceGap": snapshot.trace_gap,
    });
    if let Some(trace_gap_code) = snapshot.trace_gap_code.as_ref() {
        merge_json_object(
            &mut extra_metadata,
            serde_json::json!({ "traceGapCode": trace_gap_code }),
        );
    }

    Some(MainChatRuntimeFactAnswer {
        reply,
        intent: intent.as_str().into(),
        fact_keys,
        facts,
        observed_at: Some(chrono::Utc::now().to_rfc3339()),
        source,
        authority: if snapshot.trace_gap {
            "task_state"
        } else if snapshot.pending_permission_count > 0 || snapshot.pending_proposal_count > 0 {
            "policy"
        } else {
            "task_state"
        },
        freshness: if snapshot.trace_gap {
            "unknown"
        } else {
            "turn_snapshot"
        },
        visibility: vec!["answer", "ui_badge", "trace_only"],
        privacy: vec!["public", "internal"],
        timezone: None,
        trace_gap: snapshot.trace_gap,
        extra_metadata,
    })
}

async fn agent_self_state_snapshot(
    state: &Arc<AppState>,
    chat_session_id: &str,
    current_task_session_id: Option<&str>,
) -> AgentSelfStateFactSnapshot {
    let Some(session_store_arc) = state.main_chat_agent_session_store.as_ref() else {
        return missing_agent_self_state_snapshot(chat_session_id, "task_session_store_missing");
    };

    let (target_session, transcript) = {
        let store = session_store_arc.lock().await;
        let sessions = match store.list_sessions(None, 100, 0) {
            Ok(sessions) => sessions,
            Err(_) => {
                return missing_agent_self_state_snapshot(
                    chat_session_id,
                    "task_session_list_failed",
                );
            }
        };
        let target_session = sessions.into_iter().find(|session| {
            session.chat_session_id == chat_session_id
                && current_task_session_id != Some(session.id.as_str())
                && classify_agent_self_state_query(&session.user_goal).is_none()
        });
        let Some(target_session) = target_session else {
            return missing_agent_self_state_snapshot(chat_session_id, "task_session_missing");
        };
        let transcript = match store.list_transcript_entries(&target_session.id) {
            Ok(entries) => entries,
            Err(_) => {
                return missing_agent_self_state_snapshot(
                    chat_session_id,
                    "transcript_load_failed",
                );
            }
        };
        (target_session, transcript)
    };

    let actions = if let Some(queue_arc) = state.main_chat_action_queue_store.as_ref() {
        let queue = queue_arc.lock().await;
        queue
            .list_for_session(&target_session.id)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let run_id = match transcript_run_id(&transcript) {
        Some(run_id) => Some(run_id),
        None => latest_matching_run_id_from_store(state, chat_session_id, &target_session).await,
    };
    let run = if let (Some(run_store_arc), Some(run_id)) =
        (state.agent_run_store.as_ref(), run_id.as_deref())
    {
        let run_store = run_store_arc.lock().await;
        run_store.get_run(run_id).ok().flatten()
    } else {
        None
    };
    let proposals = load_self_state_proposals(
        state,
        &target_session.id,
        run.as_ref().map(|run| run.id.as_str()),
        &target_session.pending_blockers,
    )
    .await;

    agent_self_state_snapshot_from_evidence(
        chat_session_id,
        target_session,
        transcript,
        actions,
        run,
        proposals,
    )
}

fn missing_agent_self_state_snapshot(
    chat_session_id: &str,
    trace_gap_code: &str,
) -> AgentSelfStateFactSnapshot {
    AgentSelfStateFactSnapshot {
        chat_session_id: bounded_runtime_fact_label(chat_session_id),
        task_session_id: None,
        run_id: None,
        task_status: None,
        run_status: None,
        delivery_status: "unknown".into(),
        final_delivery_evidence: false,
        completed_response: false,
        pending_permission_count: 0,
        pending_permission_target_labels: Vec::new(),
        pending_proposal_count: 0,
        durable_change_status: "unknown".into(),
        durable_change_completed: false,
        blocker_codes: Vec::new(),
        safe_next_controls: vec!["no_safe_automatic_control".into()],
        safe_automatic_control_available: false,
        action_count: 0,
        completed_action_count: 0,
        observation_count: 0,
        transcript_observation_count: 0,
        final_result_count: 0,
        last_action_type: None,
        last_action_status: None,
        last_observation_source: None,
        last_action_summary: None,
        evidence_labels: vec![bounded_runtime_fact_label(trace_gap_code)],
        trace_gap: true,
        trace_gap_code: Some(trace_gap_code.into()),
    }
}

fn agent_self_state_snapshot_from_evidence(
    chat_session_id: &str,
    session: AgentTaskSession,
    transcript: Vec<ExecutionTranscriptEntry>,
    actions: Vec<QueuedExecutionAction>,
    run: Option<AgentRun>,
    proposals: Vec<AgentProposal>,
) -> AgentSelfStateFactSnapshot {
    let final_result_count = transcript
        .iter()
        .filter(|entry| entry.kind == ExecutionTranscriptEntryKind::FinalResult)
        .count();
    let transcript_observation_count = transcript
        .iter()
        .filter(|entry| entry.kind == ExecutionTranscriptEntryKind::Observation)
        .count();
    let observation_count = actions
        .iter()
        .filter(|action| action.observation_metadata.is_some())
        .count()
        .max(transcript_observation_count);
    let completed_action_count = actions
        .iter()
        .filter(|action| action.status == ExecutionQueueStatus::Completed)
        .count();
    let pending_permission_actions = actions
        .iter()
        .filter(|action| action.status == ExecutionQueueStatus::PendingPermission)
        .collect::<Vec<_>>();
    let pending_permission_count = pending_permission_actions.len();
    let mut pending_permission_target_labels = pending_permission_actions
        .iter()
        .map(|action| pending_permission_target_label(action))
        .collect::<Vec<_>>();
    pending_permission_target_labels.sort();
    pending_permission_target_labels.dedup();
    let pending_proposal_count = proposals
        .iter()
        .filter(|proposal| proposal.status == ProposalStatus::Pending)
        .count();
    let run_status = run.as_ref().map(|run| run.status.to_string());
    let final_delivery_override = transcript.iter().rev().find_map(|entry| {
        entry
            .metadata
            .get("finalDeliveryStatus")
            .or_else(|| entry.metadata.get("final_delivery_status"))
            .and_then(Value::as_str)
    });
    let final_delivery_evidence = final_result_count > 0
        && final_delivery_override != Some("failed")
        && run.as_ref().is_some_and(|run| {
            run.status == AgentRunStatus::Completed && run.output_preview.is_some()
        });
    let completed_response = final_delivery_evidence
        && matches!(
            session.status,
            AgentTaskSessionStatus::Completed | AgentTaskSessionStatus::WaitingPermission
        );
    let delivery_status = if let Some(status) = final_delivery_override {
        status
    } else if final_delivery_evidence && pending_proposal_count > 0 {
        "response_delivered_pending_review"
    } else if final_delivery_evidence {
        "delivered"
    } else if session.status == AgentTaskSessionStatus::Blocked {
        "blocked"
    } else if pending_permission_count > 0
        || session.status == AgentTaskSessionStatus::WaitingPermission
    {
        "waiting_permission"
    } else if run.is_some() || final_result_count > 0 {
        "trace_gap"
    } else {
        "unknown"
    }
    .to_string();
    let durable_change_status = if pending_proposal_count > 0 {
        "pending_review"
    } else if !proposals.is_empty() {
        "review_resolved_or_not_pending"
    } else {
        "none"
    }
    .to_string();
    let durable_change_completed = !proposals.is_empty()
        && proposals
            .iter()
            .all(|proposal| proposal.status == ProposalStatus::Accepted);
    let blocker_codes = session
        .pending_blockers
        .iter()
        .map(|blocker| {
            if blocker.starts_with("proposal:") {
                "proposal_pending".to_string()
            } else {
                bounded_runtime_fact_label(blocker)
            }
        })
        .collect::<Vec<_>>();
    let safe_next_controls =
        agent_self_state_safe_next_controls(&session, &actions, pending_proposal_count);
    let safe_automatic_control_available = safe_next_controls
        .iter()
        .any(|control| control == "retry_failed_action");
    let last_action = actions.last();
    let last_action_type =
        last_action.map(|action| bounded_runtime_fact_label(&action.action.action_type));
    let last_action_status = last_action.map(|action| action.status.as_str().to_string());
    let last_observation_source = last_action
        .and_then(|action| action.observation_metadata.as_ref())
        .and_then(|metadata| {
            metadata
                .get("sourceKind")
                .or_else(|| metadata.get("sourceLabel"))
                .or_else(|| metadata.get("toolName"))
                .and_then(Value::as_str)
        })
        .map(bounded_runtime_fact_label)
        .or_else(|| {
            transcript
                .iter()
                .rev()
                .find(|entry| entry.kind == ExecutionTranscriptEntryKind::Observation)
                .map(|_| "transcript_observation".to_string())
        });
    let last_action_summary = last_action.map(|action| {
        let action_type = bounded_runtime_fact_label(&action.action.action_type);
        let status = action.status.as_str();
        let source = last_observation_source
            .as_deref()
            .unwrap_or("observation_metadata");
        format!("action={action_type} status={status} observation_source={source}")
    });
    let mut evidence_labels = vec![
        "task_session".to_string(),
        "execution_transcript".to_string(),
    ];
    if run.is_some() {
        evidence_labels.push("agent_run".to_string());
    }
    if !actions.is_empty() {
        evidence_labels.push("action_queue".to_string());
    }
    if !proposals.is_empty() {
        evidence_labels.push("proposal_store".to_string());
    }
    if final_delivery_override.is_some() {
        evidence_labels.push("terminal_delivery_receipt".to_string());
    }
    evidence_labels.sort();
    evidence_labels.dedup();

    AgentSelfStateFactSnapshot {
        chat_session_id: bounded_runtime_fact_label(chat_session_id),
        task_session_id: Some(bounded_runtime_fact_label(&session.id)),
        run_id: run.as_ref().map(|run| bounded_runtime_fact_label(&run.id)),
        task_status: Some(session.status.as_str().into()),
        run_status,
        delivery_status,
        final_delivery_evidence,
        completed_response,
        pending_permission_count,
        pending_permission_target_labels,
        pending_proposal_count,
        durable_change_status,
        durable_change_completed,
        blocker_codes,
        safe_next_controls,
        safe_automatic_control_available,
        action_count: actions.len(),
        completed_action_count,
        observation_count,
        transcript_observation_count,
        final_result_count,
        last_action_type,
        last_action_status,
        last_observation_source,
        last_action_summary,
        evidence_labels,
        trace_gap: false,
        trace_gap_code: None,
    }
}

fn pending_permission_target_label(action: &QueuedExecutionAction) -> String {
    match action.action.action_type.as_str() {
        "mcp.read_only" | "mcp_tool" | "mcp.call_tool" => "mcp.read_only".into(),
        "web.search" | "web.fetch" => "web.read".into(),
        "file.read" => "workspace.file_read".into(),
        other => bounded_runtime_fact_label(other),
    }
}

fn agent_self_state_safe_next_controls(
    session: &AgentTaskSession,
    actions: &[QueuedExecutionAction],
    pending_proposal_count: usize,
) -> Vec<String> {
    let mut controls = Vec::new();

    if actions
        .iter()
        .any(|action| action.status == ExecutionQueueStatus::PendingPermission)
    {
        controls.push("review_permission".to_string());
    }
    if pending_proposal_count > 0 {
        controls.push("review_proposal".to_string());
    }
    if actions.iter().any(|action| {
        let decision = openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_action_retry(
            Some(session),
            Some(action),
        );
        decision.allowed && !decision.manual_blocker_required
    }) {
        controls.push("retry_failed_action".to_string());
    }
    if matches!(
        session.status,
        AgentTaskSessionStatus::Running
            | AgentTaskSessionStatus::WaitingPermission
            | AgentTaskSessionStatus::Blocked
            | AgentTaskSessionStatus::Failed
    ) {
        controls.push("cancel_task".to_string());
    }
    if controls.is_empty() && !session.pending_blockers.is_empty() {
        controls.push("no_safe_automatic_control".to_string());
    }

    controls.sort();
    controls.dedup();
    controls
}

async fn latest_matching_run_id_from_store(
    state: &Arc<AppState>,
    chat_session_id: &str,
    session: &AgentTaskSession,
) -> Option<String> {
    let run_store_arc = state.agent_run_store.as_ref()?;
    let run_store = run_store_arc.lock().await;
    run_store
        .list_runs_for_session(chat_session_id, 20)
        .ok()?
        .into_iter()
        .find(|run| {
            run.user_input.as_deref() == Some(session.user_goal.as_str())
                && classify_agent_self_state_query(run.user_input.as_deref().unwrap_or_default())
                    .is_none()
        })
        .map(|run| run.id)
}

async fn load_self_state_proposals(
    state: &Arc<AppState>,
    task_session_id: &str,
    run_id: Option<&str>,
    pending_blockers: &[String],
) -> Vec<AgentProposal> {
    let Some(proposal_store_arc) = state.proposal_store.as_ref() else {
        return Vec::new();
    };
    let proposal_ids = pending_blockers
        .iter()
        .filter_map(|blocker| blocker.strip_prefix("proposal:"))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let proposal_store = proposal_store_arc.lock().await;
    proposal_store
        .list_all_proposals(100, 0)
        .unwrap_or_default()
        .into_iter()
        .filter(|proposal| {
            proposal_store
                .terminal_owner_origin_binding(&proposal.id)
                .ok()
                .flatten()
                .is_some_and(|origin| origin.task_session_id() == task_session_id)
                || run_id
                    .map(|run_id| proposal.run_id.as_deref() == Some(run_id))
                    .unwrap_or(false)
                || proposal_ids
                    .iter()
                    .any(|proposal_id| proposal_id == &proposal.id)
        })
        .collect()
}

fn transcript_run_id(transcript: &[ExecutionTranscriptEntry]) -> Option<String> {
    transcript.iter().rev().find_map(|entry| {
        if entry.kind != ExecutionTranscriptEntryKind::FinalResult {
            return None;
        }
        entry
            .metadata
            .get("runId")
            .or_else(|| entry.metadata.get("run_id"))
            .and_then(Value::as_str)
            .map(bounded_runtime_fact_label)
    })
}

fn agent_self_state_fact_bindings(
    snapshot: &AgentSelfStateFactSnapshot,
    fact_keys: &[&'static str],
) -> Vec<MainChatRuntimeFactBinding> {
    fact_keys
        .iter()
        .copied()
        .map(|key| {
            let value = match key {
                RUNTIME_FACT_KEY_AGENT_CHAT_SESSION_ID => Some(snapshot.chat_session_id.clone()),
                RUNTIME_FACT_KEY_AGENT_TASK_SESSION_ID => snapshot.task_session_id.clone(),
                RUNTIME_FACT_KEY_AGENT_RUN_ID => snapshot.run_id.clone(),
                RUNTIME_FACT_KEY_AGENT_TASK_STATUS => snapshot.task_status.clone(),
                RUNTIME_FACT_KEY_AGENT_DELIVERY_STATUS => Some(snapshot.delivery_status.clone()),
                RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY => snapshot.last_action_summary.clone(),
                RUNTIME_FACT_KEY_AGENT_PENDING_PERMISSION_COUNT => {
                    Some(snapshot.pending_permission_count.to_string())
                }
                RUNTIME_FACT_KEY_AGENT_BLOCKER_CODES => {
                    Some(snapshot.blocker_codes.join(",")).filter(|value| !value.is_empty())
                }
                RUNTIME_FACT_KEY_AGENT_PENDING_PROPOSAL_COUNT => {
                    Some(snapshot.pending_proposal_count.to_string())
                }
                RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS => {
                    Some(snapshot.durable_change_status.clone())
                }
                RUNTIME_FACT_KEY_AGENT_TRACE_GAP => snapshot.trace_gap_code.clone(),
                _ => None,
            };
            let missing =
                value.is_none() || key == RUNTIME_FACT_KEY_AGENT_TRACE_GAP || snapshot.trace_gap;
            let (value_shape, source, authority, freshness, visibility, privacy) = match key {
                RUNTIME_FACT_KEY_AGENT_CHAT_SESSION_ID => (
                    "bounded_id",
                    vec!["task_session"],
                    "task_state",
                    "turn_snapshot",
                    "trace_only",
                    "internal",
                ),
                RUNTIME_FACT_KEY_AGENT_TASK_SESSION_ID => (
                    "bounded_id",
                    vec!["task_session"],
                    "task_state",
                    "turn_snapshot",
                    "trace_only",
                    "internal",
                ),
                RUNTIME_FACT_KEY_AGENT_RUN_ID => (
                    "bounded_id",
                    vec!["agent_run", "transcript"],
                    "run_trace",
                    "run_trace",
                    "trace_only",
                    "internal",
                ),
                RUNTIME_FACT_KEY_AGENT_TASK_STATUS => (
                    "canonical_task_status",
                    vec!["task_session", "action_queue"],
                    "task_state",
                    "turn_snapshot",
                    "ui_badge",
                    "public",
                ),
                RUNTIME_FACT_KEY_AGENT_DELIVERY_STATUS => (
                    "canonical_delivery_status",
                    vec!["agent_run", "final_delivery", "transcript"],
                    "run_trace",
                    "run_trace",
                    "ui_badge",
                    "public",
                ),
                RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY => (
                    "bounded_summary",
                    vec!["action_queue", "transcript"],
                    "task_state",
                    "turn_snapshot",
                    "answer",
                    "internal",
                ),
                RUNTIME_FACT_KEY_AGENT_PENDING_PERMISSION_COUNT => (
                    "integer",
                    vec!["task_session", "action_queue"],
                    "policy",
                    "turn_snapshot",
                    "ui_badge",
                    "public",
                ),
                RUNTIME_FACT_KEY_AGENT_BLOCKER_CODES => (
                    "bounded_labels",
                    vec!["task_session", "transcript"],
                    "task_state",
                    "turn_snapshot",
                    "ui_badge",
                    "internal",
                ),
                RUNTIME_FACT_KEY_AGENT_PENDING_PROPOSAL_COUNT => (
                    "integer",
                    vec!["task_session", "proposal_store"],
                    "policy",
                    "turn_snapshot",
                    "ui_badge",
                    "public",
                ),
                RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS => (
                    "none_pending_review_or_resolved",
                    vec!["proposal_store", "task_session"],
                    "policy",
                    "turn_snapshot",
                    "answer",
                    "public",
                ),
                RUNTIME_FACT_KEY_AGENT_TRACE_GAP => (
                    "trace_gap_code",
                    vec!["task_session", "agent_run", "transcript"],
                    "task_state",
                    "unknown",
                    "answer",
                    "public",
                ),
                _ => (
                    "unknown",
                    vec!["task_session"],
                    "task_state",
                    "unknown",
                    "trace_only",
                    "internal",
                ),
            };
            MainChatRuntimeFactBinding {
                key,
                value_shape,
                value,
                source,
                authority,
                freshness: if snapshot.trace_gap {
                    "unknown"
                } else {
                    freshness
                },
                visibility,
                privacy,
                missing,
            }
        })
        .collect()
}

fn agent_self_state_reply(
    intent: MainChatAgentSelfStateIntent,
    snapshot: &AgentSelfStateFactSnapshot,
) -> String {
    if snapshot.trace_gap {
        let code = snapshot
            .trace_gap_code
            .as_deref()
            .unwrap_or("self_state_trace_gap");
        return format!(
            "我不能确认上一项任务状态：缺少 task session / run / transcript 证据（trace_gap={code}）。我不会根据助手文字臆造历史。"
        );
    }

    match intent {
        MainChatAgentSelfStateIntent::AskTaskCompletion => {
            if snapshot.pending_permission_count > 0 {
                let target_labels = if snapshot.pending_permission_target_labels.is_empty() {
                    "unknown".to_string()
                } else {
                    snapshot.pending_permission_target_labels.join(",")
                };
                format!(
                    "这个任务正在等待用户确认，不能标为完成：task_status={}，delivery_status={}，pendingPermissionCount={}，pendingPermissionTargetLabel={}。我没有执行 pending action；需要用户先 review_permission。",
                    label_or_unknown(snapshot.task_status.as_deref()),
                    snapshot.delivery_status,
                    snapshot.pending_permission_count,
                    target_labels
                )
            } else if snapshot.pending_proposal_count > 0 {
                format!(
                    "这次回答已经交付：task_status={}，run_status={}，delivery_status={}。但还有 {} 个提案处于 pending review，durable_change_status=pending_review；我没有把待审变更当作已完成的持久写入。",
                    label_or_unknown(snapshot.task_status.as_deref()),
                    label_or_unknown(snapshot.run_status.as_deref()),
                    snapshot.delivery_status,
                    snapshot.pending_proposal_count
                )
            } else if snapshot.task_status.as_deref() == Some("blocked") {
                let controls = if snapshot.safe_next_controls.is_empty() {
                    "no_safe_automatic_control".to_string()
                } else {
                    snapshot.safe_next_controls.join(",")
                };
                format!(
                    "这个任务没有完成：task_status={}，delivery_status={}，blockerCodes={}，safeNextControls={}，safeAutomaticControlAvailable={}.",
                    label_or_unknown(snapshot.task_status.as_deref()),
                    snapshot.delivery_status,
                    if snapshot.blocker_codes.is_empty() {
                        "none".into()
                    } else {
                        snapshot.blocker_codes.join(",")
                    },
                    controls,
                    snapshot.safe_automatic_control_available
                )
            } else if snapshot.completed_response {
                format!(
                    "这个任务的回答已完成：task_status={}，run_status={}，delivery_status={}，final_delivery_evidence=true。没有待审提案或待确认权限。",
                    label_or_unknown(snapshot.task_status.as_deref()),
                    label_or_unknown(snapshot.run_status.as_deref()),
                    snapshot.delivery_status
                )
            } else {
                format!(
                    "这个任务还不能标为完成：task_status={}，run_status={}，delivery_status={}，blockers={}。",
                    label_or_unknown(snapshot.task_status.as_deref()),
                    label_or_unknown(snapshot.run_status.as_deref()),
                    snapshot.delivery_status,
                    if snapshot.blocker_codes.is_empty() {
                        "none".into()
                    } else {
                        snapshot.blocker_codes.join(",")
                    }
                )
            }
        }
        MainChatAgentSelfStateIntent::AskLastActionSummary => {
            if snapshot.action_count == 0 && snapshot.transcript_observation_count == 0 {
                return "上一项任务没有记录到工具/action observation；我只能确认它有任务/运行证据，不能编造工具历史。".into();
            }
            format!(
                "我刚刚做的是受治理的运行步骤：{}。证据来自 action_queue/transcript；observation_count={}，final_delivery_evidence={}，directWritesExecuted=false。",
                snapshot
                    .last_action_summary
                    .as_deref()
                    .unwrap_or("transcript observation recorded"),
                snapshot.observation_count,
                snapshot.final_delivery_evidence
            )
        }
    }
}

pub(crate) fn classify_agent_self_state_query(
    user_text: &str,
) -> Option<MainChatAgentSelfStateIntent> {
    let normalized = user_text.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    let compact = normalized
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let compact = trim_outer_punctuation(&compact);
    let english_phrase = trim_outer_punctuation(&normalized);

    if matches_exact_runtime_fact_phrase(
        compact,
        &[
            "你刚刚做了什么",
            "刚刚做了什么",
            "你刚才做了什么",
            "刚才做了什么",
            "上一轮做了什么",
            "你上一步做了什么",
            "刚刚执行了什么",
            "刚刚用了什么工具",
        ],
    ) || matches_exact_runtime_fact_phrase(
        english_phrase,
        &[
            "what did you just do",
            "what was your last action",
            "what did you do last turn",
            "what tool did you just use",
            "what happened in the previous turn",
        ],
    ) {
        return Some(MainChatAgentSelfStateIntent::AskLastActionSummary);
    }

    if matches_exact_runtime_fact_phrase(
        compact,
        &[
            "这个任务完成了吗",
            "任务完成了吗",
            "刚刚的任务完成了吗",
            "上一项任务完成了吗",
            "你完成了吗",
            "完成了吗",
            "这个请求完成了吗",
        ],
    ) || matches_exact_runtime_fact_phrase(
        english_phrase,
        &[
            "is this task done",
            "is this task complete",
            "did you complete the task",
            "did you finish the task",
            "is the previous task complete",
        ],
    ) {
        return Some(MainChatAgentSelfStateIntent::AskTaskCompletion);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::main_chat_agent_v1::{
        ActionQueueStore, ActionReplayEffectCertainty, AgentTaskSessionDraft,
        AgentTaskSessionStore, ExecutionAction, ExecutionPolicy, ExecutionQueueStatus,
        InitialToolExecutionProjection, MainChatAgentStrategy,
    };
    use openlife_core::agent::{ActionExecutionStatus, ToolActionEffect};
    use openlife_core::tool_execution_receipt::ToolExecutionReceipt;
    use openlife_core::tool_manifest::ToolIdempotencyContract;

    #[test]
    fn dispatched_unknown_failure_never_becomes_a_safe_retry_control() {
        let sessions = AgentTaskSessionStore::new_in_memory().expect("task sessions");
        let session = sessions
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "agent-self-unknown-effect".into(),
                user_goal: "Read without duplicating an uncertain effect.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create task session");
        let actions = ActionQueueStore::new_in_memory().expect("action queue");
        let action = ExecutionAction::new("file.read", "Read one governed resource.");
        let queued = actions
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue action");
        let initially_failed = actions
            .fail(&queued.id, "pre-dispatch setup failure", None)
            .expect("enter failed");
        let replay_execution_id = uuid::Uuid::new_v4().to_string();
        let claim = actions
            .claim_replay_for_test_fixture(
                &queued.id,
                initially_failed.status,
                initially_failed.revision,
                &replay_execution_id,
            )
            .expect("claim replay");
        let retrying = actions
            .transition_claimed_replay(
                &queued.id,
                &claim.claim_id,
                initially_failed.status,
                claim.revision,
                ExecutionQueueStatus::Retrying,
                None,
            )
            .expect("enter retrying");
        let executing = actions
            .transition_claimed_replay(
                &queued.id,
                &claim.claim_id,
                retrying.status,
                retrying.revision,
                ExecutionQueueStatus::Executing,
                None,
            )
            .expect("enter executing");
        let fenced = actions
            .fence_replay_dispatch_commit(
                &queued.id,
                &claim.claim_id,
                claim.owner_generation,
                executing.revision,
            )
            .expect("persist replay pre-edge dispatch fence");
        let dispatched = actions
            .record_replay_dispatch_started(&queued.id, &claim.claim_id, fenced.revision)
            .expect("record physical dispatch boundary");
        let failed = actions
            .fail_claimed_replay(
                &queued.id,
                &claim.claim_id,
                dispatched.status,
                dispatched.revision,
                "remote effect unknown",
                Some(serde_json::json!({"retryReplayable": true})),
            )
            .expect("persist unknown effect");
        assert_eq!(
            failed.replay_effect_certainty,
            ActionReplayEffectCertainty::DispatchedUnknown
        );

        let controls = agent_self_state_safe_next_controls(&session, &[failed], 0);
        assert!(!controls
            .iter()
            .any(|control| control == "retry_failed_action"));
    }

    #[test]
    fn non_idempotent_pre_dispatch_failure_is_manual_only_not_an_automatic_control() {
        let sessions = AgentTaskSessionStore::new_in_memory().expect("task sessions");
        let session = sessions
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "agent-self-non-idempotent-pre-dispatch".into(),
                user_goal: "Do not replay a non-idempotent tool automatically.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create task session");
        let actions = ActionQueueStore::new_in_memory().expect("action queue");
        let action = ExecutionAction::new("mcp.read_only", "Opaque tool name is not authority.");
        let queued = actions
            .enqueue(
                &session.id,
                action.clone(),
                ExecutionPolicy.classify(&action),
            )
            .expect("enqueue action");
        let receipt = ToolExecutionReceipt::test_gateway_failed_before_dispatch(
            Some("run-agent-self-non-idempotent".into()),
            Some("manifest-agent-self-non-idempotent".into()),
            "agent-self-non-idempotent-pre-dispatch".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::NonIdempotent,
        );
        let failed = actions
            .project_initial_tool_execution_receipt(
                &queued.id,
                queued.status,
                queued.revision,
                InitialToolExecutionProjection {
                    execution_status: ActionExecutionStatus::Failed,
                    receipt: &receipt,
                    observation_metadata: None,
                    error: Some("failed before dispatch".into()),
                },
            )
            .expect("project typed pre-dispatch receipt");
        assert_eq!(
            failed.replay_effect_certainty,
            ActionReplayEffectCertainty::EffectNotAttempted
        );
        let decision = openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_action_retry(
            Some(&session),
            Some(&failed),
        );
        assert!(decision.allowed);
        assert!(decision.manual_blocker_required);

        let chat_session_id = session.chat_session_id.clone();
        let snapshot = agent_self_state_snapshot_from_evidence(
            &chat_session_id,
            session,
            Vec::new(),
            vec![failed],
            None,
            Vec::new(),
        );
        assert!(!snapshot
            .safe_next_controls
            .iter()
            .any(|control| control == "retry_failed_action"));
        assert!(!snapshot.safe_automatic_control_available);
    }
}
