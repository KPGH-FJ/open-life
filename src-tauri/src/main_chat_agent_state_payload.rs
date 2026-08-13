use crate::main_chat_event_stream::{
    provider_remote_unknown_has_runtime_cancel_contract,
    provider_remote_unknown_has_runtime_kernel_failure_contract, MainChatAgentDurableEvent,
};
use crate::AppState;
use chrono::Utc;
use openlife_core::agent::main_chat_agent_v1::ExecutionTranscriptEntryKind;
use openlife_core::agent::main_chat_runtime_contract::{
    assemble_main_chat_agent_state, EvidenceGap, MainChatAgentProductStrategyRoute,
    MainChatAgentProductTaskStatus, MainChatAgentStateAssemblerInput, MainChatAgentStateEvent,
    MainChatAgentStateEventType, MainChatAgentStateSnapshot, ProviderRouteEvidence,
    StrategyEvidence, TaskSessionEvidence,
};
use std::collections::BTreeSet;
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
        match crate::terminal_owner_write_gateway::register_agent_run_store_result(
            state,
            run_store.get_run(run_id).map_err(|error| error.to_string()),
        ) {
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

    // Provider identity comes only from the same-run durable adapter lifecycle.
    // The AgentRun lock above has been released before this independent store
    // read, and its minimized model_route is deliberately not consulted.
    let provider = match run_id {
        Some(expected_run_id) if run.as_ref().is_some_and(|run| run.id != expected_run_id) => {
            let actual_run_id = run.as_ref().map(|run| run.id.as_str()).unwrap_or("unknown");
            assembly_diagnostics.push(gap(
                "agent_state_provider_run_identity_mismatch",
                "AgentRun identity did not match the run requested for provider lifecycle evidence.",
                Some(format!("expected:{expected_run_id}:actual:{actual_run_id}")),
            ));
            None
        }
        Some(expected_run_id) => {
            match validated_provider_route_evidence_for_run(state, task_session_id, expected_run_id)
                .await
            {
                Ok(provider) => provider,
                Err(provider_gap) => {
                    assembly_diagnostics.push(provider_gap);
                    None
                }
            }
        }
        None => None,
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
                        proposal_store
                            .terminal_owner_origin_binding(&proposal.id)
                            .ok()
                            .flatten()
                            .is_some_and(|origin| origin.task_session_id() == task_session_id)
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
            let explicit_memory_ids = transcript
                .iter()
                .filter(|entry| {
                    entry.kind == ExecutionTranscriptEntryKind::FinalResult
                        && entry
                            .metadata
                            .get("acceptedDurableTruthWritten")
                            .and_then(serde_json::Value::as_bool)
                            == Some(true)
                        && entry
                            .metadata
                            .get("directMemoryWrite")
                            .and_then(serde_json::Value::as_bool)
                            == Some(true)
                })
                .filter_map(|entry| {
                    entry
                        .metadata
                        .get("receiptId")
                        .and_then(serde_json::Value::as_str)
                })
                .map(str::to_string)
                .collect::<BTreeSet<_>>();
            for memory_id in explicit_memory_ids {
                match memory_lifecycle_store.get_record(&memory_id) {
                Ok(Some(record))
                    if record.source_task_session_id.as_deref() == Some(task_session_id)
                        && record.source_run_id.as_deref() == run_id =>
                {
                    if !records
                        .iter()
                        .any(|existing| existing.memory_id == record.memory_id)
                    {
                        records.push(record);
                    }
                }
                Ok(Some(_)) => assembly_diagnostics.push(gap(
                    "agent_state_explicit_memory_owner_mismatch",
                    "Explicit Memory receipt did not belong to the requested canonical task/run.",
                    Some(memory_id),
                )),
                Ok(None) => assembly_diagnostics.push(gap(
                    "agent_state_explicit_memory_owner_missing",
                    "Explicit Memory receipt referenced a missing canonical owner.",
                    Some(memory_id),
                )),
                Err(err) => assembly_diagnostics.push(gap(
                    "agent_state_explicit_memory_owner_load_failed",
                    &format!("Explicit Memory canonical owner could not be loaded: {err}"),
                    Some(memory_id),
                )),
            }
            }
            records
        } else {
            Vec::new()
        };

    let has_canonical_plan_item = transcript.iter().any(|entry| {
        entry.kind == ExecutionTranscriptEntryKind::Plan
            && entry.metadata.get("canonicalTaskId").is_some()
    });

    let mut snapshot = match assemble_main_chat_agent_state(MainChatAgentStateAssemblerInput {
        session,
        run_identity: run_id.map(str::to_string),
        run,
        provider,
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
    if has_canonical_plan_item {
        if let Some(plan) = snapshot.plan.as_mut() {
            plan.editable = false;
            plan.source = "canonical_task_item".into();
            plan.controls = vec!["open_trace".into()];
        }
    }
    append_diagnostics(&mut snapshot, assembly_diagnostics);
    Some(snapshot)
}

async fn validated_provider_route_evidence_for_run(
    state: &Arc<AppState>,
    task_session_id: &str,
    run_id: &str,
) -> Result<Option<ProviderRouteEvidence>, EvidenceGap> {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err(gap(
            "agent_state_provider_event_store_unavailable",
            "Durable provider lifecycle evidence is unavailable for this governed turn.",
            Some(run_id.to_string()),
        ));
    };
    let event = {
        let store = store_arc.lock().await;
        store
            .latest_provider_event_for_run(run_id)
            .map_err(|error| {
                gap(
                    "agent_state_provider_lifecycle_load_failed",
                    &format!("Validated provider lifecycle evidence could not be loaded: {error}"),
                    Some(run_id.to_string()),
                )
            })?
    };
    let Some(event) = event else {
        return Ok(None);
    };
    provider_route_evidence_from_lifecycle_event(&event, task_session_id, run_id)
        .map(Some)
        .map_err(|reason| {
            gap(
                "agent_state_provider_lifecycle_invalid",
                &format!("Durable provider lifecycle evidence was rejected: {reason}"),
                Some(event.event_id.clone()),
            )
        })
}

fn provider_route_evidence_from_lifecycle_event(
    event: &MainChatAgentDurableEvent,
    expected_task_session_id: &str,
    expected_run_id: &str,
) -> Result<ProviderRouteEvidence, &'static str> {
    if event.backfilled {
        return Err("backfilled_event_is_not_adapter_authority");
    }
    if event.task_session_id != expected_task_session_id || event.run_id != expected_run_id {
        return Err("task_or_run_identity_mismatch");
    }
    if event.object_type != "provider_request" {
        return Err("event_is_not_adapter_provider_lifecycle");
    }
    if !matches!(
        event.event_type.as_str(),
        "provider.started" | "provider.completed" | "provider.failed" | "provider.remote_unknown"
    ) {
        return Err("event_type_is_not_provider_lifecycle");
    }
    match event.source.as_str() {
        "provider_adapter" => {}
        "openlife_turn_runtime"
            if provider_remote_unknown_has_runtime_cancel_contract(event)
                || provider_remote_unknown_has_runtime_kernel_failure_contract(event) => {}
        "openlife_turn_runtime" => return Err("runtime_provider_lifecycle_contract_invalid"),
        _ => return Err("event_is_not_adapter_provider_lifecycle"),
    }
    let payload = event
        .payload
        .as_object()
        .ok_or("provider_lifecycle_payload_not_object")?;
    let provider = payload
        .get("provider")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or("provider_identity_missing")?;
    let model = payload
        .get("model")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or("model_identity_missing")?;
    let effective_data_route = payload
        .get("effectiveDataRoute")
        .and_then(serde_json::Value::as_str)
        .ok_or("effective_data_route_missing")?;
    let route_type = match (provider, effective_data_route) {
        ("ollama", "local_only" | "policy_allowed") => "local",
        (_, "policy_allowed") => "cloud",
        (_, "local_only") => return Err("local_only_route_contradicts_non_ollama_provider"),
        _ => return Err("effective_data_route_unknown"),
    };
    Ok(ProviderRouteEvidence {
        provider: provider.to_string(),
        model: model.to_string(),
        route_type: route_type.to_string(),
        provider_config_generation: payload
            .get("providerConfigGeneration")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        reason: format!("durable_provider_lifecycle:{}", event.event_type),
        evidence_id: event.event_id.clone(),
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn append_provider_adapter_event(
        state: &Arc<AppState>,
        task_session_id: &str,
        run_id: &str,
        request_id: &str,
        provider: &str,
        event_type: &str,
    ) -> MainChatAgentDurableEvent {
        crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
            state,
            task_session_id,
            run_id,
            event_type,
            "provider_request",
            request_id,
            "provider_adapter",
            json!({
                "requestId": request_id,
                "provider": provider,
                "model": format!("model-{provider}"),
                "status": event_type.strip_prefix("provider.").unwrap_or("unknown"),
                "effectiveDataRoute": "policy_allowed",
            }),
        )
        .await
        .expect("append provider adapter lifecycle event")
    }

    fn provider_lifecycle_event(
        provider: &str,
        effective_data_route: Option<&str>,
    ) -> MainChatAgentDurableEvent {
        let mut payload = json!({
            "requestId": "request-provider-authority-test",
            "provider": provider,
            "model": "model-provider-authority-test",
            "status": "completed",
        });
        if let Some(effective_data_route) = effective_data_route {
            payload
                .as_object_mut()
                .expect("provider payload")
                .insert("effectiveDataRoute".into(), json!(effective_data_route));
        }
        MainChatAgentDurableEvent {
            event_id: "event-provider-authority-test".into(),
            task_session_id: "task-provider-authority-test".into(),
            run_id: "run-provider-authority-test".into(),
            sequence: 1,
            event_type: "provider.completed".into(),
            object_type: "provider_request".into(),
            object_id: "request-provider-authority-test".into(),
            created_at: Utc::now(),
            source: "provider_adapter".into(),
            payload_digest: "sha256:test-provider-authority".into(),
            payload,
            backfilled: false,
        }
    }

    #[test]
    fn provider_lifecycle_route_type_uses_policy_route_and_exact_adapter_identity() {
        let ollama_policy_allowed = provider_route_evidence_from_lifecycle_event(
            &provider_lifecycle_event("ollama", Some("policy_allowed")),
            "task-provider-authority-test",
            "run-provider-authority-test",
        )
        .expect("policy-allowed local-first Ollama route");
        assert_eq!(ollama_policy_allowed.provider, "ollama");
        assert_eq!(ollama_policy_allowed.route_type, "local");
        assert_eq!(
            ollama_policy_allowed.evidence_id,
            "event-provider-authority-test"
        );

        let cloud = provider_route_evidence_from_lifecycle_event(
            &provider_lifecycle_event("openai", Some("policy_allowed")),
            "task-provider-authority-test",
            "run-provider-authority-test",
        )
        .expect("policy-authorized cloud route");
        assert_eq!(cloud.route_type, "cloud");

        let ollama_local_only = provider_route_evidence_from_lifecycle_event(
            &provider_lifecycle_event("ollama", Some("local_only")),
            "task-provider-authority-test",
            "run-provider-authority-test",
        )
        .expect("local-only Ollama route");
        assert_eq!(ollama_local_only.route_type, "local");
    }

    #[test]
    fn provider_lifecycle_route_type_fails_closed_on_missing_or_contradictory_policy() {
        let nonlocal_local_only = provider_route_evidence_from_lifecycle_event(
            &provider_lifecycle_event("openai", Some("local_only")),
            "task-provider-authority-test",
            "run-provider-authority-test",
        );
        assert_eq!(
            nonlocal_local_only,
            Err("local_only_route_contradicts_non_ollama_provider")
        );

        let missing = provider_route_evidence_from_lifecycle_event(
            &provider_lifecycle_event("openai", None),
            "task-provider-authority-test",
            "run-provider-authority-test",
        );
        assert_eq!(missing, Err("effective_data_route_missing"));

        let mut backfilled = provider_lifecycle_event("ollama", Some("local_only"));
        backfilled.backfilled = true;
        assert_eq!(
            provider_route_evidence_from_lifecycle_event(
                &backfilled,
                "task-provider-authority-test",
                "run-provider-authority-test",
            ),
            Err("backfilled_event_is_not_adapter_authority")
        );
    }

    #[test]
    fn provider_lifecycle_accepts_only_the_closed_runtime_cancel_terminal_contract() {
        let mut cancelled = provider_lifecycle_event("openai", Some("policy_allowed"));
        let observed_at = cancelled.created_at;
        cancelled.event_type = "provider.remote_unknown".into();
        cancelled.source = "openlife_turn_runtime".into();
        cancelled.payload["status"] = json!("remote_unknown");
        cancelled.payload["cancellationId"] = json!("cancellation:provider-authority-test");
        cancelled.payload["startedAt"] = json!(observed_at - chrono::Duration::milliseconds(1));
        cancelled.payload["observedAt"] = json!(observed_at);
        cancelled.payload["localWaitAborted"] = json!(true);
        cancelled.payload["localKernelFutureDropped"] = json!(true);
        cancelled.payload["remoteCancellationConfirmed"] = json!(false);

        let evidence = provider_route_evidence_from_lifecycle_event(
            &cancelled,
            "task-provider-authority-test",
            "run-provider-authority-test",
        )
        .expect("validated runtime cancellation terminal");
        assert_eq!(evidence.provider, "openai");
        assert_eq!(evidence.route_type, "cloud");

        let mut runtime_completed = provider_lifecycle_event("openai", Some("policy_allowed"));
        runtime_completed.source = "openlife_turn_runtime".into();
        assert_eq!(
            provider_route_evidence_from_lifecycle_event(
                &runtime_completed,
                "task-provider-authority-test",
                "run-provider-authority-test",
            ),
            Err("runtime_provider_lifecycle_contract_invalid")
        );

        cancelled
            .payload
            .as_object_mut()
            .unwrap()
            .remove("cancellationId");
        assert_eq!(
            provider_route_evidence_from_lifecycle_event(
                &cancelled,
                "task-provider-authority-test",
                "run-provider-authority-test",
            ),
            Err("runtime_provider_lifecycle_contract_invalid")
        );
    }

    #[tokio::test]
    async fn provider_state_uses_latest_validated_attempt_instead_of_an_older_completion() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let task_id = "task-provider-latest-attempt";
        let run_id = "run-provider-latest-attempt";
        append_provider_adapter_event(
            &state,
            task_id,
            run_id,
            "request-provider-completed-a",
            "openai",
            "provider.started",
        )
        .await;
        append_provider_adapter_event(
            &state,
            task_id,
            run_id,
            "request-provider-completed-a",
            "openai",
            "provider.completed",
        )
        .await;
        append_provider_adapter_event(
            &state,
            task_id,
            run_id,
            "request-provider-latest-b",
            "ollama",
            "provider.started",
        )
        .await;

        let latest_started = validated_provider_route_evidence_for_run(&state, task_id, run_id)
            .await
            .expect("validated provider lifecycle query")
            .expect("latest started attempt evidence");
        assert_eq!(latest_started.provider, "ollama");
        assert_eq!(latest_started.model, "model-ollama");
        assert_eq!(latest_started.route_type, "local");
        assert_eq!(
            latest_started.reason,
            "durable_provider_lifecycle:provider.started"
        );

        append_provider_adapter_event(
            &state,
            task_id,
            run_id,
            "request-provider-latest-b",
            "ollama",
            "provider.remote_unknown",
        )
        .await;
        let latest_unknown = validated_provider_route_evidence_for_run(&state, task_id, run_id)
            .await
            .expect("validated provider lifecycle query")
            .expect("latest unknown attempt evidence");
        assert_eq!(latest_unknown.provider, "ollama");
        assert_eq!(latest_unknown.model, "model-ollama");
        assert_eq!(
            latest_unknown.reason,
            "durable_provider_lifecycle:provider.remote_unknown"
        );
    }

    #[tokio::test]
    async fn agent_run_store_unavailable_does_not_erase_validated_provider_lifecycle() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("isolated state has one owner")
            .agent_run_store = None;
        let session = {
            let store = state
                .main_chat_agent_session_store
                .as_ref()
                .expect("session store")
                .lock()
                .await;
            store
                .create_session(
                    openlife_core::agent::main_chat_agent_v1::AgentTaskSessionDraft {
                        chat_session_id: "chat-provider-without-agent-run-store".into(),
                        user_goal: "Keep durable provider truth without AgentRunStore.".into(),
                        selected_strategy: openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy::DirectAnswer,
                        current_plan_summary: None,
                        context_snapshot_refs: Vec::new(),
                    },
                )
                .expect("create task session")
        };
        let run_id = "run-provider-without-agent-run-store";
        append_provider_adapter_event(
            &state,
            &session.id,
            run_id,
            "request-provider-without-agent-run-store",
            "openai",
            "provider.started",
        )
        .await;
        append_provider_adapter_event(
            &state,
            &session.id,
            run_id,
            "request-provider-without-agent-run-store",
            "openai",
            "provider.completed",
        )
        .await;

        let snapshot =
            assemble_main_chat_agent_state_for_turn(&state, Some(&session.id), Some(run_id))
                .await
                .expect("agent state snapshot");
        assert_eq!(snapshot.task.run_id, run_id);
        let provider = snapshot.provider.expect("validated provider lifecycle");
        assert_eq!(provider.provider, "openai");
        assert_eq!(provider.model, "model-openai");
        assert!(snapshot
            .diagnostics
            .iter()
            .any(|gap| gap.gap_code == "agent_state_run_store_unavailable"));
        assert!(!snapshot
            .diagnostics
            .iter()
            .any(|gap| gap.gap_code == "missing_run_identity"));
    }
}
