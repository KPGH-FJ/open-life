use crate::{main_chat_context_loader::compile_main_chat_context, AppState};
use openlife_core::agent::main_chat_agent_v1::{
    AgentIngress, AgentIngressDecision, AgentTaskSessionDraft, ExecutionAction, ExecutionPolicy,
    ExecutionQueueStatus, ExecutionTranscriptEntry, ExecutionTranscriptEntryDraft,
    ExecutionTranscriptEntryKind, QueuedExecutionAction,
};
use openlife_core::agent::{AgentRunStatus, AgentTaskKind};
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct MainChatAgentTurn {
    pub(crate) decision: AgentIngressDecision,
    pub(crate) transcript_entries: Vec<ExecutionTranscriptEntry>,
}

pub(crate) async fn start_main_chat_agent_turn(
    operation_id: &str,
    canonical_user_message: &openlife_core::memory::CanonicalConversationMessageCommit,
    messages: &[ChatMessage],
    task_kind: AgentTaskKind,
    state: &Arc<AppState>,
) -> Result<MainChatAgentTurn, String> {
    let user_text = messages
        .last()
        .filter(|message| message.role == "user")
        .map(|message| message.content.as_str())
        .unwrap_or_default();
    let ingress = AgentIngress::default();
    let mut decision = ingress
        .decide_with_canonical_user_message(
            operation_id,
            canonical_user_message,
            user_text,
            messages,
            task_kind,
        )
        .map_err(|error| format!("canonical Main Chat policy admission failed: {error}"))?;
    let session_id = canonical_user_message.receipt().session_id.as_str();
    let provisional_task_session_id = decision
        .agent_task_session_id
        .clone()
        .ok_or_else(|| "main_chat_task_session_id_missing".to_string())?;
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?;
    let store = store_arc.lock().await;
    match store
        .load_session(&provisional_task_session_id)
        .map_err(|err| format!("load canonical Main Chat task session failed: {err}"))?
    {
        Some(existing) => {
            if existing.chat_session_id != session_id
                || existing.id != operation_id
                || existing.selected_strategy != decision.selected_strategy
                || existing.user_goal != user_text
            {
                return Err("turn_operation_task_payload_drift".into());
            }
            if existing.status
                != openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Running
            {
                return Err(format!(
                    "turn_operation_task_reconciliation_required:{}",
                    existing.status.as_str()
                ));
            }
        }
        None => {
            let created = store
                .create_session_with_id(
                    provisional_task_session_id.clone(),
                    AgentTaskSessionDraft {
                        chat_session_id: session_id.to_string(),
                        user_goal: if user_text.trim().is_empty() {
                            "Main Chat request".into()
                        } else {
                            user_text.to_string()
                        },
                        selected_strategy: decision.selected_strategy,
                        current_plan_summary: None,
                        context_snapshot_refs: Vec::new(),
                    },
                )
                .map_err(|err| format!("create canonical Main Chat task session failed: {err}"))?;
            decision.agent_task_session_id = Some(created.id);
        }
    }
    store
        .bind_session_canonical_user_message(
            &provisional_task_session_id,
            &canonical_user_message.receipt().canonical_ref,
            user_text,
        )
        .map_err(|error| format!("bind task session to canonical user message failed: {error}"))?;

    Ok(MainChatAgentTurn {
        decision,
        transcript_entries: Vec::new(),
    })
}

pub(crate) async fn record_main_chat_agent_turn_ingress(
    state: &Arc<AppState>,
    main_chat_agent_turn: &mut MainChatAgentTurn,
    session_id: &str,
    user_text: &str,
    run_id: &str,
) -> Result<(), String> {
    let task_session_id = main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref()
        .ok_or_else(|| "main_chat_task_session_id_missing".to_string())?;
    main_chat_agent_turn.transcript_entries.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::UserInput,
            "User submitted a Main Chat request.",
            serde_json::json!({
                "chatSessionId": session_id,
                "runId": run_id,
                "userMessagePresent": !user_text.trim().is_empty(),
                "rawUserTextStored": false,
            }),
        )
        .await,
    );
    main_chat_agent_turn.transcript_entries.extend(
        append_main_chat_agent_transcript(
            state,
            Some(task_session_id),
            ExecutionTranscriptEntryKind::RouteDecision,
            "PolicyRouter selected the Main Chat product route.",
            serde_json::json!({
                "runId": run_id,
                "requestId": main_chat_agent_turn.decision.request_id,
                "policyRoute": main_chat_agent_turn.decision.policy_route.as_str(),
                "policyRouteReasonCode": main_chat_agent_turn.decision.policy_reason_code.as_str(),
                "selectedStrategy": main_chat_agent_turn.decision.selected_strategy.as_str(),
                "selectedStrategySource": "policy_route_bridge",
                "confidence": main_chat_agent_turn.decision.confidence,
                "fallbackEligible": main_chat_agent_turn.decision.fallback_eligible,
                "intentFrame": main_chat_agent_turn.decision.intent_frame.clone(),
                "riskLevel": main_chat_agent_turn.decision.privacy_risk.risk_level,
                "privacyClass": main_chat_agent_turn.decision.privacy_risk.privacy_class,
                "policyReasonCode": main_chat_agent_turn.decision.privacy_risk.policy_reason_code,
                "rawUserTextStored": false,
            }),
        )
        .await,
    );
    Ok(())
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
    if state.main_chat_agent_session_store.is_none() {
        return Vec::new();
    }
    match crate::terminal_owner_write_gateway::append_task_transcript(
        state,
        ExecutionTranscriptEntryDraft {
            session_id: task_session_id.to_string(),
            kind,
            summary: summary.into(),
            metadata,
        },
    )
    .await
    {
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
    conversation_owner_id: &str,
    user_text: &str,
    selected_skill_id: Option<&str>,
) -> Result<Vec<ExecutionTranscriptEntry>, String> {
    let Some(task_session_id) = main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref()
    else {
        return Ok(Vec::new());
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
        conversation_owner_id,
        user_text,
        selected_skill_id,
    )
    .await?;
    crate::terminal_owner_write_gateway::write_task_session(
        state,
        task_session_id,
        crate::terminal_owner_write_gateway::TaskSessionWrite::RecordContextSnapshotRef(
            compiled_context.context_snapshot_ref.clone(),
        ),
    )
    .await
    .map_err(|error| format!("persist main chat context snapshot ref failed: {error}"))?;
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
    Ok(entries)
}

pub(crate) async fn complete_main_chat_agent_turn_session(
    state: &Arc<AppState>,
    main_chat_agent_turn: &MainChatAgentTurn,
    final_summary: &str,
) -> Result<(), String> {
    let Some(task_session_id) = main_chat_agent_turn
        .decision
        .agent_task_session_id
        .as_deref()
    else {
        return Err("main_chat_task_session_id_missing_at_completion".into());
    };
    if state.main_chat_agent_session_store.is_none() {
        return Err("main_chat_agent_session_store_unavailable_at_completion".into());
    }
    crate::terminal_owner_write_gateway::write_task_session(
        state,
        task_session_id,
        crate::terminal_owner_write_gateway::TaskSessionWrite::Complete(final_summary.into()),
    )
    .await
    .map_err(|err| format!("complete Main Chat task session failed: {err}"))?;
    Ok(())
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
    let queued = crate::terminal_owner_write_gateway::enqueue_action(
        state,
        task_session_id,
        ExecutionAction::new(action_type.to_string(), description.to_string()),
        policy,
    )
    .await
    .map_err(|err| format!("enqueue Main Chat action failed: {err}"))?;

    if state.main_chat_agent_session_store.is_some() {
        if let Err(err) = crate::terminal_owner_write_gateway::write_task_session(
            state,
            task_session_id,
            crate::terminal_owner_write_gateway::TaskSessionWrite::RecordActionQueueId(
                queued.id.clone(),
            ),
        )
        .await
        {
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
    Interrupted,
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
            Self::Interrupted => "interrupted",
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
            Self::Interrupted => "interrupted",
            Self::PolicyBlocker => "blocked",
            Self::ProviderError | Self::ToolError | Self::UnknownError => "failed",
        }
    }

    /// Status persisted on the canonical terminal event. This is deliberately
    /// separate from the normalized task/run lifecycle projection: for
    /// example, a timeout is a `failed` event whose product lifecycle is
    /// `timed_out`, while cancellation is a `local_aborted` event whose product
    /// lifecycle is `cancelled`.
    pub(crate) fn durable_terminal_event_status(self) -> &'static str {
        match self {
            Self::Cancelled => "local_aborted",
            Self::Interrupted => "interrupted",
            Self::Timeout
            | Self::ProviderError
            | Self::ToolError
            | Self::PolicyBlocker
            | Self::UnknownError => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatTaskFailureFinalization {
    pub(crate) run_id: Option<String>,
    pub(crate) task_session_id: Option<String>,
    pub(crate) lifecycle_state: String,
    pub(crate) transcript_entry_id: Option<String>,
    pub(crate) durable_event: crate::main_chat_event_stream::MainChatAgentDurableEvent,
}

struct MainChatTaskFailureFinalizationRequest<'a> {
    state: &'a Arc<AppState>,
    run_id: Option<&'a str>,
    task_session_id: Option<&'a str>,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &'a str,
    source_ref: &'a str,
    durable_event: Option<crate::main_chat_event_stream::MainChatAgentDurableEvent>,
    agent_run_write_lane: AgentRunFailureWriteLane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentRunFailureWriteLane {
    Normal,
    StartupReconciliation,
}

pub(crate) async fn finalize_main_chat_task_failure(
    state: &Arc<AppState>,
    run_id: Option<&str>,
    task_session_id: Option<&str>,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &str,
    source_ref: &str,
) -> Result<MainChatTaskFailureFinalization, String> {
    finalize_main_chat_task_failure_inner(MainChatTaskFailureFinalizationRequest {
        state,
        run_id,
        task_session_id,
        failure_kind,
        safe_reason,
        source_ref,
        durable_event: None,
        agent_run_write_lane: AgentRunFailureWriteLane::Normal,
    })
    .await
}

pub(crate) async fn finalize_main_chat_task_failure_at_startup_reconciliation(
    state: &Arc<AppState>,
    run_id: Option<&str>,
    task_session_id: Option<&str>,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &str,
    source_ref: &str,
) -> Result<MainChatTaskFailureFinalization, String> {
    finalize_main_chat_task_failure_inner(MainChatTaskFailureFinalizationRequest {
        state,
        run_id,
        task_session_id,
        failure_kind,
        safe_reason,
        source_ref,
        durable_event: None,
        agent_run_write_lane: AgentRunFailureWriteLane::StartupReconciliation,
    })
    .await
}

/// Project a failure/cancellation only after its terminal receipt was already
/// committed in the caller's atomic lifecycle batch.
pub(crate) async fn finalize_main_chat_task_failure_after_durable_receipt(
    state: &Arc<AppState>,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &str,
    source_ref: &str,
    durable_event: crate::main_chat_event_stream::MainChatAgentDurableEvent,
) -> Result<MainChatTaskFailureFinalization, String> {
    let run_id = durable_event.run_id.clone();
    let task_session_id = durable_event.task_session_id.clone();
    finalize_main_chat_task_failure_inner(MainChatTaskFailureFinalizationRequest {
        state,
        run_id: Some(&run_id),
        task_session_id: Some(&task_session_id),
        failure_kind,
        safe_reason,
        source_ref,
        durable_event: Some(durable_event),
        agent_run_write_lane: AgentRunFailureWriteLane::Normal,
    })
    .await
}

pub(crate) async fn finalize_main_chat_task_failure_after_durable_receipt_at_startup_reconciliation(
    state: &Arc<AppState>,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &str,
    source_ref: &str,
    durable_event: crate::main_chat_event_stream::MainChatAgentDurableEvent,
) -> Result<MainChatTaskFailureFinalization, String> {
    let run_id = durable_event.run_id.clone();
    let task_session_id = durable_event.task_session_id.clone();
    finalize_main_chat_task_failure_inner(MainChatTaskFailureFinalizationRequest {
        state,
        run_id: Some(&run_id),
        task_session_id: Some(&task_session_id),
        failure_kind,
        safe_reason,
        source_ref,
        durable_event: Some(durable_event),
        agent_run_write_lane: AgentRunFailureWriteLane::StartupReconciliation,
    })
    .await
}

async fn finalize_main_chat_task_failure_inner(
    request: MainChatTaskFailureFinalizationRequest<'_>,
) -> Result<MainChatTaskFailureFinalization, String> {
    let MainChatTaskFailureFinalizationRequest {
        state,
        run_id,
        task_session_id,
        failure_kind,
        safe_reason,
        source_ref,
        durable_event,
        agent_run_write_lane,
    } = request;
    let safe_reason = metadata_safe_failure_label(safe_reason, 240);
    let source_ref = metadata_safe_failure_label(source_ref, 120);
    let resolved_task_session_id = task_session_id
        .map(|value| metadata_safe_failure_label(value, 96))
        .filter(|value| !value.is_empty());
    let mut resolved_run_id = run_id
        .map(|value| metadata_safe_failure_label(value, 96))
        .filter(|value| !value.is_empty());

    let task_id = resolved_task_session_id
        .as_deref()
        .ok_or_else(|| "canonical_task_session_id_required_for_failure".to_string())?;
    if resolved_run_id.is_none() {
        resolved_run_id = canonical_agent_run_id_for_task(state, task_id).await?;
    }
    let run_id = resolved_run_id
        .as_deref()
        .ok_or_else(|| format!("canonical_agent_run_missing_for_task:{task_id}"))?;
    validate_failure_run_task_binding(state, run_id, task_id).await?;
    validate_failure_task_session(state, task_id).await?;

    let route_evidence =
        runtime_route_evidence_value_for_run_id(state, resolved_run_id.as_deref()).await?;
    let route_evidence_ref = route_evidence
        .as_ref()
        .and_then(|value| value.get("evidence_id"))
        .and_then(serde_json::Value::as_str)
        .map(|value| metadata_safe_failure_label(value, 160));

    let durable_event = if let Some(durable_event) = durable_event {
        validate_prepersisted_failure_terminal_receipt(
            &durable_event,
            run_id,
            task_id,
            failure_kind,
        )?;
        durable_event
    } else {
        persist_main_chat_failure_terminal_receipt(
            state,
            run_id,
            task_id,
            failure_kind,
            &safe_reason,
            &source_ref,
        )
        .await?
    };

    finalize_agent_run_failure(
        state,
        resolved_run_id.as_deref(),
        task_id,
        failure_kind,
        &safe_reason,
        agent_run_write_lane,
        Some(&durable_event),
    )
    .await?;
    finalize_task_session_failure(
        state,
        resolved_task_session_id.as_deref(),
        failure_kind,
        &safe_reason,
        agent_run_write_lane,
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
        durable_event,
    })
}

fn validate_prepersisted_failure_terminal_receipt(
    event: &crate::main_chat_event_stream::MainChatAgentDurableEvent,
    run_id: &str,
    task_session_id: &str,
    failure_kind: MainChatTaskFailureKind,
) -> Result<(), String> {
    let expected_event_type = failure_kind.durable_terminal_event_status();
    let identity_matches = event.run_id == run_id
        && event.task_session_id == task_session_id
        && event.object_type == "turn"
        && event.event_type == expected_event_type;
    let payload_matches = event
        .payload
        .get("status")
        .and_then(serde_json::Value::as_str)
        == Some(failure_kind.durable_terminal_event_status())
        && event
            .payload
            .get("kind")
            .and_then(serde_json::Value::as_str)
            == Some(failure_kind.as_str());
    if !identity_matches || !payload_matches {
        return Err("prepersisted_failure_terminal_receipt_mismatch".into());
    }
    Ok(())
}

/// If the durable event store fails before any provider/tool dispatch, keep the
/// cross-store task/run projections fail-closed and record only a digest of the
/// persistence error. There is intentionally no synthetic durable event: the
/// missing event-store fact remains observable as degraded truth.
pub(crate) async fn mark_main_chat_pre_dispatch_event_store_failure(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
    error: &str,
) -> Result<(), String> {
    validate_failure_run_task_binding(state, run_id, task_session_id).await?;
    validate_failure_task_session(state, task_session_id).await?;
    let safe_reason = "durable event store failed before external dispatch";
    let error_digest = format!("sha256:{:x}", Sha256::digest(error.as_bytes()));
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?;
    store_arc
        .lock()
        .await
        .record_pre_dispatch_persistence_failure(task_session_id, run_id, &error_digest)
        .map_err(|error| {
            format!("record typed pre-dispatch persistence failure failed: {error}")
        })?;
    // AgentRun is a separate canonical database. It is intentionally updated
    // after the typed marker + task projection transaction; if the process
    // stops here, startup recovery has enough exact durable identity to finish
    // this projection without inventing a cancellation.
    finalize_agent_run_failure(
        state,
        Some(run_id),
        task_session_id,
        MainChatTaskFailureKind::UnknownError,
        safe_reason,
        AgentRunFailureWriteLane::Normal,
        None,
    )
    .await
}

async fn persist_main_chat_failure_terminal_receipt(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &str,
    source_ref: &str,
) -> Result<crate::main_chat_event_stream::MainChatAgentDurableEvent, String> {
    let event_type = failure_kind.durable_terminal_event_status();
    let reason_digest = format!("sha256:{:x}", Sha256::digest(safe_reason.as_bytes()));
    let object_id = if matches!(
        failure_kind,
        MainChatTaskFailureKind::Cancelled | MainChatTaskFailureKind::Interrupted
    ) {
        format!("cancellation:{task_session_id}:{run_id}")
    } else {
        format!("terminal:{run_id}:{}", failure_kind.as_str())
    };
    crate::terminal_owner_write_gateway::append_runtime_event(
        state,
        task_session_id,
        run_id,
        event_type,
        "turn",
        object_id,
        source_ref,
        serde_json::json!({
            "status": failure_kind.durable_terminal_event_status(),
            "kind": failure_kind.as_str(),
            "errorDigest": reason_digest,
            "durableCommitAllowedAfterFailure": false,
        }),
    )
    .await
    .map_err(|error| format!("persist failure terminal receipt before projection failed: {error}"))
}

pub(crate) async fn canonical_main_chat_run_status(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
) -> Result<AgentRunStatus, String> {
    validate_failure_run_task_binding(state, run_id, task_session_id).await?;
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?;
    let store = store_arc.lock().await;
    crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store
            .get_run(run_id)
            .map_err(|error| format!("load canonical AgentRun status failed: {error}")),
    )?
    .map(|run| run.status)
    .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))
}

pub(crate) async fn record_main_chat_post_commit_degradation(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
    source_ref: &str,
    error: &str,
) -> Result<String, String> {
    let run_status = canonical_main_chat_run_status(state, run_id, task_session_id).await?;
    if matches!(
        run_status,
        AgentRunStatus::Running | AgentRunStatus::WaitingPermission
    ) {
        return Err(format!(
            "post_commit_degradation_requires_terminal_run:{run_id}:status={run_status}"
        ));
    }

    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?;
    let source_ref = metadata_safe_failure_label(source_ref, 120);
    let (task_lifecycle_status, existing) = {
        let store = store_arc.lock().await;
        let task_session = store
            .load_session(task_session_id)
            .map_err(|error| format!("load task for post-commit degradation failed: {error}"))?
            .ok_or_else(|| format!("canonical_task_session_missing:{task_session_id}"))?;
        let existing = store
            .list_transcript_entries(task_session_id)
            .map_err(|error| format!("load post-commit degradation transcript failed: {error}"))?;
        (task_session.status, existing)
    };
    if let Some(entry) = existing.iter().rev().find(|entry| {
        string_from_failure_metadata(&entry.metadata, &["runId", "run_id"]).as_deref()
            == Some(run_id)
            && string_from_failure_metadata(&entry.metadata, &["sourceRef", "source_ref"])
                .as_deref()
                == Some(source_ref.as_str())
            && string_from_failure_metadata(
                &entry.metadata,
                &["persistenceStatus", "persistence_status"],
            )
            .as_deref()
                == Some("projection_degraded")
    }) {
        return Ok(entry.id.clone());
    }

    let error_digest = format!("sha256:{:x}", Sha256::digest(error.as_bytes()));
    let entry = crate::terminal_owner_write_gateway::append_task_transcript(
        state,
        ExecutionTranscriptEntryDraft {
            session_id: task_session_id.to_string(),
            kind: ExecutionTranscriptEntryKind::Error,
            summary: "Canonical execution reached a terminal state, but durable event projection or terminal delivery failed."
                .into(),
            metadata: serde_json::json!({
                "runId": run_id,
                "run_id": run_id,
                "taskSessionId": task_session_id,
                "task_session_id": task_session_id,
                "canonicalExecutionStatus": run_status.to_string(),
                "canonical_execution_status": run_status.to_string(),
                "taskLifecycleStatus": task_lifecycle_status.as_str(),
                "task_lifecycle_status": task_lifecycle_status.as_str(),
                "persistenceStatus": "projection_degraded",
                "persistence_status": "projection_degraded",
                "finalDeliveryStatus": "failed",
                "final_delivery_status": "failed",
                "sourceRef": source_ref,
                "source_ref": source_ref,
                "errorDigest": error_digest,
                "error_digest": error_digest,
                "rawErrorStored": false,
                "canonicalExecutionPreserved": true,
                "safeReplay": "reconcile_projection_not_reexecute_effects",
            }),
        },
    )
        .await
        .map_err(|error| format!("append post-commit degradation transcript failed: {error}"))?;
    Ok(entry.id)
}

async fn canonical_agent_run_id_for_task(
    state: &Arc<AppState>,
    task_session_id: &str,
) -> Result<Option<String>, String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?;
    let store = store_arc.lock().await;
    crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store
            .get_run_for_task_id(task_session_id)
            .map(|run| run.map(|run| run.id))
            .map_err(|err| format!("load canonical AgentRun for task failed: {err}")),
    )
}

async fn validate_failure_run_task_binding(
    state: &Arc<AppState>,
    run_id: &str,
    task_session_id: &str,
) -> Result<(), String> {
    let store_arc = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "agent_run_store_unavailable".to_string())?;
    let store = store_arc.lock().await;
    let run = crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store
            .get_run(run_id)
            .map_err(|error| format!("load canonical AgentRun for failure failed: {error}")),
    )?
    .ok_or_else(|| format!("canonical_agent_run_missing:{run_id}"))?;
    if run.task_id != task_session_id {
        return Err(format!(
            "canonical_agent_run_task_mismatch:{run_id}:expected={task_session_id}"
        ));
    }
    Ok(())
}

async fn validate_failure_task_session(
    state: &Arc<AppState>,
    task_session_id: &str,
) -> Result<(), String> {
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?;
    let store = store_arc.lock().await;
    store
        .load_session(task_session_id)
        .map_err(|error| format!("load canonical task for failure failed: {error}"))?
        .map(|_| ())
        .ok_or_else(|| format!("canonical_task_session_missing:{task_session_id}"))
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
    let Some(run) = crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store
            .get_run(run_id)
            .map_err(|err| format!("load AgentRun route evidence failed: {err}")),
    )?
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
    task_session_id: &str,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &str,
    write_lane: AgentRunFailureWriteLane,
    durable_evidence: Option<&crate::main_chat_event_stream::MainChatAgentDurableEvent>,
) -> Result<(), String> {
    let Some(run_id) = run_id else {
        return Ok(());
    };
    match write_lane {
        AgentRunFailureWriteLane::Normal => crate::terminal_owner_write_gateway::project_main_chat_agent_run_failure(
            state,
            run_id,
            task_session_id,
            match failure_kind {
                MainChatTaskFailureKind::Timeout => crate::terminal_owner_write_gateway::AgentRunMainChatFailureKind::Timeout,
                MainChatTaskFailureKind::Cancelled => crate::terminal_owner_write_gateway::AgentRunMainChatFailureKind::Cancelled,
                MainChatTaskFailureKind::Interrupted => crate::terminal_owner_write_gateway::AgentRunMainChatFailureKind::Interrupted,
                MainChatTaskFailureKind::ProviderError => crate::terminal_owner_write_gateway::AgentRunMainChatFailureKind::ProviderError,
                MainChatTaskFailureKind::ToolError => crate::terminal_owner_write_gateway::AgentRunMainChatFailureKind::ToolError,
                MainChatTaskFailureKind::PolicyBlocker => crate::terminal_owner_write_gateway::AgentRunMainChatFailureKind::PolicyBlocker,
                MainChatTaskFailureKind::UnknownError => crate::terminal_owner_write_gateway::AgentRunMainChatFailureKind::UnknownError,
            },
            safe_reason,
        )
        .await,
        AgentRunFailureWriteLane::StartupReconciliation => {
            let evidence = durable_evidence.ok_or_else(|| {
                "startup_agent_run_failure_projection_durable_evidence_missing".to_string()
            })?;
            crate::terminal_owner_write_gateway::project_agent_run_from_startup_durable_event(
                state, evidence,
            )
            .await
        }
    }
    .map_err(|err| format!("update AgentRun failure finalizer failed: {err}"))
}

async fn finalize_task_session_failure(
    state: &Arc<AppState>,
    task_session_id: Option<&str>,
    failure_kind: MainChatTaskFailureKind,
    safe_reason: &str,
    write_lane: AgentRunFailureWriteLane,
) -> Result<(), String> {
    let Some(task_session_id) = task_session_id else {
        return Ok(());
    };
    let store_arc = state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?;
    let session = {
        let store = store_arc.lock().await;
        store
            .load_session(task_session_id)
            .map_err(|err| format!("load task session for failure finalizer failed: {err}"))?
            .ok_or_else(|| format!("canonical_task_session_missing:{task_session_id}"))?
    };
    if session.status == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Completed
        && write_lane == AgentRunFailureWriteLane::Normal
    {
        return Ok(());
    }
    if session.status == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Cancelled
        && write_lane == AgentRunFailureWriteLane::Normal
        && !matches!(
            failure_kind,
            MainChatTaskFailureKind::Cancelled | MainChatTaskFailureKind::Interrupted
        )
    {
        return Ok(());
    }

    let write = match failure_kind {
        MainChatTaskFailureKind::Cancelled => {
            crate::terminal_owner_write_gateway::TaskSessionWrite::Cancel(safe_reason.into())
        }
        MainChatTaskFailureKind::PolicyBlocker => {
            let mut blockers = session.pending_blockers.clone();
            blockers.push(safe_reason.to_string());
            blockers.sort();
            blockers.dedup();
            crate::terminal_owner_write_gateway::TaskSessionWrite::SetPendingBlockersAndTransition {
                blockers,
                transition: crate::terminal_owner_write_gateway::TaskSessionTransition::Block(
                    safe_reason.into(),
                ),
            }
        }
        MainChatTaskFailureKind::Timeout
        | MainChatTaskFailureKind::Interrupted
        | MainChatTaskFailureKind::ProviderError
        | MainChatTaskFailureKind::ToolError
        | MainChatTaskFailureKind::UnknownError => {
            crate::terminal_owner_write_gateway::TaskSessionWrite::Fail(safe_reason.into())
        }
    };
    crate::terminal_owner_write_gateway::write_task_session(state, task_session_id, write)
        .await
        .map_err(|err| format!("write task failure finalizer failed: {err}"))?;
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
    let store_arc = input
        .state
        .main_chat_agent_session_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_session_store_unavailable".to_string())?;
    let existing = {
        let store = store_arc.lock().await;
        store
            .list_transcript_entries(task_session_id)
            .map_err(|err| format!("load transcript before failure finalizer failed: {err}"))?
    };
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
        MainChatTaskFailureKind::Interrupted => {
            "Main Chat task was interrupted after cancellation; durable effect facts prevent a pure cancelled claim."
        }
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
    let entry = crate::terminal_owner_write_gateway::append_task_transcript(
        input.state,
        ExecutionTranscriptEntryDraft {
            session_id: task_session_id.to_string(),
            kind: ExecutionTranscriptEntryKind::Error,
            summary: summary.to_string(),
            metadata,
        },
    )
    .await
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

#[cfg(test)]
mod tests {
    use super::MainChatTaskFailureKind;

    #[test]
    fn durable_terminal_event_status_is_not_the_product_lifecycle_projection() {
        let cases = [
            (MainChatTaskFailureKind::Timeout, "failed", "timed_out"),
            (
                MainChatTaskFailureKind::Cancelled,
                "local_aborted",
                "cancelled",
            ),
            (
                MainChatTaskFailureKind::Interrupted,
                "interrupted",
                "interrupted",
            ),
            (MainChatTaskFailureKind::ProviderError, "failed", "failed"),
            (MainChatTaskFailureKind::ToolError, "failed", "failed"),
            (MainChatTaskFailureKind::PolicyBlocker, "failed", "blocked"),
            (MainChatTaskFailureKind::UnknownError, "failed", "failed"),
        ];

        for (failure_kind, durable_status, lifecycle_state) in cases {
            assert_eq!(
                failure_kind.durable_terminal_event_status(),
                durable_status,
                "durable event status must describe the immutable event fact"
            );
            assert_eq!(
                failure_kind.normalized_lifecycle_state(),
                lifecycle_state,
                "task/run lifecycle must remain an independent projection"
            );
        }
    }
}
