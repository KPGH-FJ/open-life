use crate::{main_chat_context_loader::compile_main_chat_context, AppState};
use openlife_core::agent::main_chat_agent_v1::{
    AgentIngress, AgentIngressDecision, AgentTaskSessionDraft, ExecutionAction, ExecutionPolicy,
    ExecutionQueueStatus, ExecutionTranscriptEntry, ExecutionTranscriptEntryDraft,
    ExecutionTranscriptEntryKind, QueuedExecutionAction,
};
use openlife_core::agent::AgentTaskKind;
use openlife_core::llm::ChatMessage;
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
