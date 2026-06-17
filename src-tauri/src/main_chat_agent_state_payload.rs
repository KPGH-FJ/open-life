use crate::AppState;
use chrono::Utc;
use openlife_core::agent::main_chat_agent_productization_v1::{
    assemble_main_chat_agent_state, EvidenceGap, MainChatAgentProductStrategyRoute,
    MainChatAgentProductTaskStatus, MainChatAgentStateAssemblerInput, MainChatAgentStateEvent,
    MainChatAgentStateEventType, MainChatAgentStateSnapshot, StrategyEvidence, TaskSessionEvidence,
};
use std::sync::Arc;

pub(crate) async fn assemble_main_chat_agent_state_for_turn(
    state: &Arc<AppState>,
    task_session_id: Option<&str>,
    run_id: Option<&str>,
) -> Option<MainChatAgentStateSnapshot> {
    let task_session_id = task_session_id?;
    let Some(session_store_arc) = state.main_chat_agent_session_store.as_ref() else {
        return Some(diagnostic_agent_state_snapshot(
            task_session_id,
            run_id,
            "agent_state_session_store_unavailable",
            "Main Chat Agent task session store is unavailable for this governed turn.",
        ));
    };

    let (session, transcript, mut assembly_diagnostics) = {
        let store = session_store_arc.lock().await;
        let session = match store.load_session(task_session_id) {
            Ok(Some(session)) => session,
            Ok(None) => {
                return Some(diagnostic_agent_state_snapshot(
                    task_session_id,
                    run_id,
                    "agent_state_session_not_found",
                    "Main Chat Agent task session evidence was not found for this governed turn.",
                ));
            }
            Err(err) => {
                return Some(diagnostic_agent_state_snapshot(
                    task_session_id,
                    run_id,
                    "agent_state_session_load_failed",
                    &format!("Main Chat Agent task session evidence could not be loaded: {err}"),
                ));
            }
        };
        let mut diagnostics = Vec::new();
        let transcript = match store.list_transcript_entries(task_session_id) {
            Ok(transcript) => transcript,
            Err(err) => {
                diagnostics.push(gap(
                    "agent_state_transcript_load_failed",
                    &format!("Main Chat Agent transcript evidence could not be loaded: {err}"),
                    Some(task_session_id.to_string()),
                ));
                Vec::new()
            }
        };
        (session, transcript, diagnostics)
    };

    let actions = if let Some(queue_arc) = state.main_chat_action_queue_store.as_ref() {
        let queue = queue_arc.lock().await;
        match queue.list_for_session(task_session_id) {
            Ok(actions) => actions,
            Err(err) => {
                assembly_diagnostics.push(gap(
                    "agent_state_action_queue_load_failed",
                    &format!("Main Chat Agent action queue evidence could not be loaded: {err}"),
                    Some(task_session_id.to_string()),
                ));
                Vec::new()
            }
        }
    } else {
        assembly_diagnostics.push(gap(
            "agent_state_action_queue_store_unavailable",
            "Main Chat Agent action queue store is unavailable for this governed turn.",
            Some(task_session_id.to_string()),
        ));
        Vec::new()
    };

    let run = if let (Some(run_store_arc), Some(run_id)) = (state.agent_run_store.as_ref(), run_id)
    {
        let run_store = run_store_arc.lock().await;
        match run_store.get_run(run_id) {
            Ok(run) => run,
            Err(err) => {
                assembly_diagnostics.push(gap(
                    "agent_state_run_load_failed",
                    &format!("AgentRun evidence could not be loaded: {err}"),
                    Some(run_id.to_string()),
                ));
                None
            }
        }
    } else {
        if run_id.is_some() && state.agent_run_store.is_none() {
            assembly_diagnostics.push(gap(
                "agent_state_run_store_unavailable",
                "AgentRun store is unavailable for this governed turn.",
                run_id.map(str::to_string),
            ));
        }
        None
    };

    let referenced_proposal_ids = session
        .pending_blockers
        .iter()
        .filter_map(|blocker| blocker.strip_prefix("proposal:"))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let proposals = if let Some(proposal_store_arc) = state.proposal_store.as_ref() {
        let proposal_store = proposal_store_arc.lock().await;
        match proposal_store.list_all_proposals(100, 0) {
            Ok(proposals) => {
                let proposals = proposals
                    .into_iter()
                    .filter(|proposal| {
                        proposal.source_detail.as_deref() == Some(task_session_id)
                            || run_id
                                .map(|run_id| proposal.run_id.as_deref() == Some(run_id))
                                .unwrap_or(false)
                            || referenced_proposal_ids
                                .iter()
                                .any(|proposal_id| proposal_id == &proposal.id)
                    })
                    .collect::<Vec<_>>();
                for proposal_id in &referenced_proposal_ids {
                    if !proposals.iter().any(|proposal| &proposal.id == proposal_id) {
                        assembly_diagnostics.push(gap(
                            "missing_proposal_evidence",
                            "Session referenced a proposal id that was not provided to the assembler.",
                            Some(proposal_id.clone()),
                        ));
                    }
                }
                proposals
            }
            Err(err) => {
                assembly_diagnostics.push(gap(
                    "agent_state_proposal_load_failed",
                    &format!("Proposal evidence could not be loaded: {err}"),
                    Some(task_session_id.to_string()),
                ));
                Vec::new()
            }
        }
    } else {
        if !referenced_proposal_ids.is_empty() {
            assembly_diagnostics.push(gap(
                "agent_state_proposal_store_unavailable",
                "Proposal store is unavailable for pending proposal evidence.",
                Some(task_session_id.to_string()),
            ));
        }
        Vec::new()
    };

    let memory_lifecycle_records =
        if let Some(memory_lifecycle_store_arc) = state.memory_lifecycle_store.as_ref() {
            let memory_lifecycle_store = memory_lifecycle_store_arc.lock().await;
            let mut records = Vec::new();
            for proposal in &proposals {
                match memory_lifecycle_store.get_record_by_proposal_id(&proposal.id) {
                    Ok(Some(record)) => records.push(record),
                    Ok(None) => {}
                    Err(err) => assembly_diagnostics.push(gap(
                        "agent_state_memory_lifecycle_load_failed",
                        &format!(
                            "Memory lifecycle evidence could not be loaded for proposal {}: {err}",
                            proposal.id
                        ),
                        Some(proposal.id.clone()),
                    )),
                }
            }
            records
        } else {
            Vec::new()
        };

    let mut snapshot = match assemble_main_chat_agent_state(MainChatAgentStateAssemblerInput {
        session,
        run,
        transcript,
        actions,
        proposals,
        memory_lifecycle_records,
    }) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return Some(diagnostic_agent_state_snapshot(
                task_session_id,
                run_id,
                "agent_state_assembly_failed",
                &format!("Main Chat Agent state assembly failed: {err}"),
            ));
        }
    };
    append_diagnostics(&mut snapshot, assembly_diagnostics);
    Some(snapshot)
}

fn gap(gap_code: &str, detail: &str, evidence_id: Option<String>) -> EvidenceGap {
    EvidenceGap {
        gap_id: format!("agent-state-{}", gap_code.replace('_', "-")),
        gap_code: gap_code.into(),
        detail: detail.into(),
        evidence_id,
    }
}

fn append_diagnostics(snapshot: &mut MainChatAgentStateSnapshot, gaps: Vec<EvidenceGap>) {
    for gap in gaps {
        if snapshot
            .diagnostics
            .iter()
            .any(|existing| existing.gap_code == gap.gap_code)
        {
            continue;
        }
        snapshot.sequence += 1;
        snapshot.events.push(MainChatAgentStateEvent {
            event_type: MainChatAgentStateEventType::DiagnosticCreated,
            sequence: snapshot.sequence,
            object_id: gap.gap_id.clone(),
            evidence_id: gap
                .evidence_id
                .clone()
                .unwrap_or_else(|| snapshot.task.task_id.clone()),
        });
        snapshot.diagnostics.push(gap);
    }
    if snapshot.sequence > 0 {
        snapshot.emitted_at = Utc::now();
    }
}

fn diagnostic_agent_state_snapshot(
    task_session_id: &str,
    run_id: Option<&str>,
    gap_code: &str,
    detail: &str,
) -> MainChatAgentStateSnapshot {
    let now = Utc::now();
    let gap = gap(gap_code, detail, Some(task_session_id.to_string()));
    MainChatAgentStateSnapshot {
        task: TaskSessionEvidence {
            task_id: task_session_id.into(),
            run_id: run_id.unwrap_or("unknown").into(),
            conversation_id: "unknown".into(),
            user_message_id: format!("user:{task_session_id}"),
            title: "Agent state assembly diagnostics".into(),
            strategy: MainChatAgentProductStrategyRoute::Unknown,
            status: MainChatAgentProductTaskStatus::Failed,
            created_at: now,
            updated_at: now,
            trace_available: false,
            controls: Vec::new(),
            action_ids: Vec::new(),
            observation_ids: Vec::new(),
            blocker_ids: Vec::new(),
            proposal_ids: Vec::new(),
            final_delivery_id: None,
        },
        route: StrategyEvidence {
            strategy: MainChatAgentProductStrategyRoute::Unknown,
            reason: "agent_state_assembly_failed".into(),
            confidence: None,
        },
        context: Vec::new(),
        provider: None,
        plan: None,
        actions: Vec::new(),
        observations: Vec::new(),
        blockers: Vec::new(),
        proposals: Vec::new(),
        final_delivery: None,
        diagnostics: vec![gap.clone()],
        sequence: 1,
        emitted_at: now,
        events: vec![MainChatAgentStateEvent {
            event_type: MainChatAgentStateEventType::DiagnosticCreated,
            sequence: 1,
            object_id: gap.gap_id,
            evidence_id: task_session_id.into(),
        }],
    }
}
