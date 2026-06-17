use openlife_core::agent::main_chat_agent_productization_v1::{
    MainChatAgentProductStrategyRoute, MainChatAgentStateEventType,
};
use openlife_core::agent::main_chat_agent_v1::{
    AgentTaskSessionDraft, ExecutionTranscriptEntryDraft, ExecutionTranscriptEntryKind,
    MainChatAgentStrategy,
};
use openlife_core::agent::types::RedactionLevel;
use openlife_core::agent::{AgentRun, AgentRunStatus, ContextSummary, ModelRouteTrace};
use openlife_core::llm::ChatMessage;

fn event_gate_invoke_request(cmd: &str, body: serde_json::Value) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: cmd.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body: tauri::ipc::InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: tauri::test::INVOKE_KEY.to_string(),
    }
}

fn event_gate_command_test_context() -> tauri::Context<tauri::test::MockRuntime> {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    let mock_ipc_origin = tauri::utils::acl::ExecutionContext::Remote {
        url: "http://tauri.localhost"
            .parse()
            .expect("valid mock IPC origin pattern"),
    };
    context.runtime_authority_mut().__allow_command(
        "run_main_chat_agent_product_maturity_v2_event_gate".into(),
        mock_ipc_origin,
    );
    context
}

fn direct_answer_fixture_snapshot() -> (
    openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentStateSnapshot,
    String,
) {
    let session_store =
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStore::new_in_memory()
            .expect("session store");
    let session = session_store
        .create_session(AgentTaskSessionDraft {
            chat_session_id: "chat-ev-direct".into(),
            user_goal: "Simple direct answer.".into(),
            selected_strategy: MainChatAgentStrategy::DirectAnswer,
            current_plan_summary: None,
            context_snapshot_refs: vec![],
        })
        .expect("create session");
    session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::RouteDecision,
            summary: "Direct answer route selected.".into(),
            metadata: serde_json::json!({
                "selectedStrategy": "direct_answer",
                "reason": "ordinary conversational answer"
            }),
        })
        .expect("route transcript");
    let final_entry = session_store
        .append_transcript_entry(ExecutionTranscriptEntryDraft {
            session_id: session.id.clone(),
            kind: ExecutionTranscriptEntryKind::FinalResult,
            summary: "A concise direct answer.".into(),
            metadata: serde_json::json!({"directWritesExecuted": false}),
        })
        .expect("final transcript");
    session_store
        .complete_session(&session.id, "A concise direct answer.")
        .expect("complete session");
    let session = session_store
        .load_session(&session.id)
        .expect("load session")
        .expect("session exists");
    let snapshot = openlife_core::agent::main_chat_agent_productization_v1::assemble_main_chat_agent_state(
        openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentStateAssemblerInput {
            session,
            run: Some(fixture_run("chat-ev-direct", &final_entry.summary)),
            transcript: session_store
                .list_transcript_entries(&final_entry.session_id)
                .expect("transcript"),
            actions: vec![],
            proposals: vec![],
            memory_lifecycle_records: vec![],
        },
    )
    .expect("assemble snapshot");
    (snapshot, final_entry.id)
}

fn fixture_run(session_id: &str, output_preview: &str) -> AgentRun {
    let mut run = AgentRun::new_chat_run(session_id, "Simple direct answer.");
    run.id = "run-ev-direct".into();
    run.status = AgentRunStatus::Completed;
    run.output_preview = Some(output_preview.into());
    run.model_route = Some(ModelRouteTrace {
        provider: "scripted_eval".into(),
        model: "event-stream-fixture".into(),
        route_type: "local".into(),
        prefer_local: true,
        local_model: "event-stream-fixture".into(),
        reason: "deterministic event stream fixture".into(),
        privacy_level: RedactionLevel::LocalOnly,
        latency_ms: Some(1),
        retry_count: 0,
        fallback_reason: None,
        provider_health_is_estimated: Some(false),
    });
    run.context_summary = Some(ContextSummary {
        life_model_empty: true,
        included_life_model_sections: Vec::new(),
        memory_hit_count: 0,
        memory_sources: Vec::new(),
        used_tools_prompt: false,
        redaction_applied: true,
        redaction_level: RedactionLevel::LocalOnly,
    });
    run
}

#[tokio::test]
async fn main_chat_event_stream_materializes_replayable_events_with_stable_ids() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let (snapshot, final_entry_id) = direct_answer_fixture_snapshot();

    assert_eq!(
        snapshot.route.strategy,
        MainChatAgentProductStrategyRoute::DirectAnswer
    );
    assert!(
        snapshot
            .events
            .iter()
            .any(|event| event.event_type == MainChatAgentStateEventType::FinalDeliveryCreated),
        "fixture must still prove the existing snapshot event surface"
    );

    let emitted = crate::main_chat_event_stream::materialize_main_chat_agent_events_for_snapshot(
        &state, &snapshot,
    )
    .await
    .expect("materialize events");
    assert!(
        emitted
            .iter()
            .any(|event| event.event_type == "route.selected"),
        "EV-01 needs a route.selected durable event: {emitted:?}"
    );
    assert!(
        emitted
            .iter()
            .any(|event| event.event_type == "final_delivery.created"
                && event.object_id == final_entry_id),
        "EV-01 needs a final_delivery.created durable event tied to final result evidence: {emitted:?}"
    );
    assert!(
        emitted.iter().all(|event| {
            event.event_id.starts_with("mainchat_event:")
                && event.task_session_id == snapshot.task.task_id
                && event.run_id == snapshot.task.run_id
                && event.payload_digest.starts_with("bytes:")
                && event.payload_digest.contains(" hash:sha256:")
                && !event.backfilled
        }),
        "durable events must carry stable ids, run/task identity, digest, and live/backfill distinction: {emitted:?}"
    );
    let sequences = emitted
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>();
    assert_eq!(
        sequences,
        (1..=emitted.len() as u64).collect::<Vec<_>>(),
        "event sequences must be monotonic per task"
    );

    let replay = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        snapshot.task.task_id.clone(),
        Some(0),
        Some(100),
    )
    .await
    .expect("replay events");
    assert_eq!(
        replay
            .iter()
            .map(|event| &event.event_id)
            .collect::<Vec<_>>(),
        emitted
            .iter()
            .map(|event| &event.event_id)
            .collect::<Vec<_>>(),
        "replay must return the exact emitted event ids"
    );
    assert_eq!(
        replay
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        sequences,
        "replay must return the exact emitted sequences"
    );

    let emitted_again =
        crate::main_chat_event_stream::materialize_main_chat_agent_events_for_snapshot(
            &state, &snapshot,
        )
        .await
        .expect("re-materialize same snapshot");
    assert_eq!(
        emitted_again
            .iter()
            .map(|event| &event.event_id)
            .collect::<Vec<_>>(),
        replay
            .iter()
            .map(|event| &event.event_id)
            .collect::<Vec<_>>(),
        "materializing the same runtime evidence must dedupe instead of allocating new ids"
    );
}

#[tokio::test]
async fn main_chat_event_stream_replays_since_sequence_and_reports_snapshot_recovery() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let (snapshot, _) = direct_answer_fixture_snapshot();
    let emitted = crate::main_chat_event_stream::materialize_main_chat_agent_events_for_snapshot(
        &state, &snapshot,
    )
    .await
    .expect("materialize events");
    let first_sequence = emitted.first().expect("first event").sequence;

    let missed = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        snapshot.task.task_id.clone(),
        Some(first_sequence),
        Some(100),
    )
    .await
    .expect("replay missed events");
    assert!(
        missed.iter().all(|event| event.sequence > first_sequence),
        "replay since sequence must only return missed events"
    );
    assert!(
        missed.len() < emitted.len(),
        "replay since a later sequence should not resend already applied events"
    );

    let recovery = crate::main_chat_event_stream::evaluate_main_chat_event_gap_recovery(
        &emitted,
        first_sequence,
        first_sequence + 2,
    );
    assert_eq!(recovery.status, "replaying_events");
    assert_eq!(recovery.replay_after_sequence, first_sequence);
    assert!(!recovery.snapshot_required);

    let truncated = crate::main_chat_event_stream::evaluate_main_chat_event_gap_recovery(
        &[],
        first_sequence,
        first_sequence + 2,
    );
    assert_eq!(truncated.status, "snapshot_refresh_required");
    assert!(truncated.snapshot_required);
}

#[tokio::test]
async fn main_chat_stream_emits_only_replayable_durable_events() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let mut emitted_payloads = Vec::<serde_json::Value>::new();

    crate::main_chat_streaming::start_stream_message_with_state(
        "event-stream-replayable-stream".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }],
        None,
        &state,
        |event, payload| {
            if event == "main-chat-agent-event" {
                emitted_payloads.push(payload);
            }
        },
    )
    .await
    .expect("stream message");

    let emitted = emitted_payloads
        .into_iter()
        .map(serde_json::from_value::<crate::main_chat_event_stream::MainChatAgentDurableEvent>)
        .collect::<Result<Vec<_>, _>>()
        .expect("deserialize emitted events");
    assert!(
        !emitted.is_empty(),
        "streaming command must emit durable backend events"
    );
    assert!(
        emitted.iter().all(|event| !event.backfilled),
        "streamed live events must not be backfilled compatibility events"
    );
    let task_session_id = emitted[0].task_session_id.clone();
    let replay = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id,
        Some(0),
        Some(100),
    )
    .await
    .expect("replay emitted events");
    assert_eq!(
        emitted
            .iter()
            .map(|event| event.event_id.clone())
            .collect::<Vec<_>>(),
        replay
            .iter()
            .map(|event| event.event_id.clone())
            .collect::<Vec<_>>(),
        "events emitted to UI must replay with the same ids"
    );
    assert_eq!(
        emitted
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        replay
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        "events emitted to UI must replay with the same sequence"
    );
}

#[test]
fn main_chat_event_stream_keeps_backfill_and_live_namespaces_separate() {
    let store = crate::main_chat_event_stream::MainChatAgentEventStore::new_in_memory()
        .expect("event store");
    let (snapshot, _) = direct_answer_fixture_snapshot();

    let backfilled =
        crate::main_chat_event_stream::materialize_main_chat_agent_backfill_events_for_snapshot_in_store(
            &store,
            &snapshot,
        )
        .expect("backfill events");
    assert!(!backfilled.is_empty());
    assert!(
        backfilled.iter().all(|event| event.backfilled),
        "snapshot recovery events must not look like live delta proof"
    );
    assert!(
        backfilled.iter().all(|event| event.source == "diagnostic"),
        "snapshot recovery events must use the diagnostic backfill source"
    );

    let live_attempt =
        crate::main_chat_event_stream::materialize_main_chat_agent_events_for_snapshot_in_store(
            &store, &snapshot,
        )
        .expect("live materialization after backfill");
    assert!(
        live_attempt.iter().all(|event| !event.backfilled),
        "live materialization after backfill must allocate live events instead of reusing diagnostic backfill"
    );
    assert!(
        live_attempt
            .iter()
            .zip(backfilled.iter())
            .all(|(live, backfill)| live.event_id != backfill.event_id
                && live.sequence > backfill.sequence),
        "live and backfilled compatibility events must not collide: live={live_attempt:?} backfill={backfilled:?}"
    );

    let replay = store
        .list(&snapshot.task.task_id, 0, 100)
        .expect("replay backfilled and live events");
    assert_eq!(replay.len(), backfilled.len() + live_attempt.len());

    let live_first_store = crate::main_chat_event_stream::MainChatAgentEventStore::new_in_memory()
        .expect("live-first event store");
    let live_first =
        crate::main_chat_event_stream::materialize_main_chat_agent_events_for_snapshot_in_store(
            &live_first_store,
            &snapshot,
        )
        .expect("live first events");
    let backfill_after_live =
        crate::main_chat_event_stream::materialize_main_chat_agent_backfill_events_for_snapshot_in_store(
            &live_first_store,
            &snapshot,
        )
        .expect("backfill after live");
    assert_eq!(
        backfill_after_live
            .iter()
            .map(|event| event.event_id.clone())
            .collect::<Vec<_>>(),
        live_first
            .iter()
            .map(|event| event.event_id.clone())
            .collect::<Vec<_>>(),
        "snapshot recovery must reuse existing live durable events when they already exist"
    );
    assert!(
        backfill_after_live.iter().all(|event| !event.backfilled),
        "existing live events must not be relabeled as diagnostic backfill"
    );
    let live_first_replay = live_first_store
        .list(&snapshot.task.task_id, 0, 100)
        .expect("replay live-first events");
    assert_eq!(live_first_replay.len(), live_first.len());
}

#[test]
fn main_chat_product_maturity_v2_event_eval_covers_ev_matrix() {
    let report =
        crate::main_chat_event_stream::run_main_chat_agent_product_maturity_v2_event_gate();

    assert_eq!(report.scenario_count, 8);
    assert_eq!(report.default_gate_scenario_count, 8);
    assert!(report.ready, "{:?}", report.blockers);
    for id in [
        "EV-01", "EV-02", "EV-03", "EV-04", "EV-05", "EV-06", "EV-07", "EV-08",
    ] {
        let proof = report
            .proofs
            .iter()
            .find(|proof| proof.scenario_id == id)
            .unwrap_or_else(|| panic!("missing {id} proof"));
        assert!(proof.passed, "{id} failed: {:?}", proof.diagnostics);
        assert!(
            proof.emitted_event_ids == proof.replayed_event_ids,
            "{id} must compare emitted event ids with replayed event ids"
        );
        assert!(
            proof.emitted_sequences == proof.replayed_sequences,
            "{id} must compare emitted sequences with replayed sequences"
        );
        assert!(
            proof.runtime_object_count > 0,
            "{id} must prove runtime object evidence, not schema-only assertions"
        );
    }
    let duplicate = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "EV-06")
        .expect("EV-06");
    assert!(duplicate
        .ui_state
        .contains(&"duplicate_ignored".to_string()));
    let gap = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "EV-07")
        .expect("EV-07");
    assert!(
        gap.ui_state
            .iter()
            .any(|state| state == "replaying_events" || state == "snapshot_refresh_required"),
        "EV-07 must prove replay or snapshot recovery: {:?}",
        gap.ui_state
    );
    assert!(
        gap.ui_state
            .contains(&"snapshot_backfill_excluded_from_live_credit".to_string()),
        "EV-07 must prove snapshot backfill is excluded from live delta credit"
    );
}

#[tokio::test]
async fn run_main_chat_product_maturity_v2_event_gate_command_returns_auditable_report() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            crate::commands::agent_runtime::run_main_chat_agent_product_maturity_v2_event_gate
        ])
        .build(event_gate_command_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");

    let response = tauri::test::get_ipc_response(
        &webview,
        event_gate_invoke_request(
            "run_main_chat_agent_product_maturity_v2_event_gate",
            serde_json::json!({}),
        ),
    )
    .expect("event gate response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize event gate response");

    assert_eq!(response["scenarioCount"], 8);
    assert_eq!(response["defaultGateScenarioCount"], 8);
    assert_eq!(response["passedScenarioCount"], 8);
    assert_eq!(response["expectedBlockerCount"], 0);
    assert!(response["ready"].as_bool().unwrap_or(false));
    assert!(response["blockers"]
        .as_array()
        .expect("blockers array")
        .is_empty());
    let proofs = response["proofs"].as_array().expect("proofs array");
    assert_eq!(proofs.len(), 8);
    for proof in proofs {
        assert!(proof["passed"].as_bool().unwrap_or(false));
        assert_eq!(proof["capabilityGroup"], "event_delta_stream");
        assert_eq!(proof["emittedEventIds"], proof["replayedEventIds"]);
        assert_eq!(proof["emittedSequences"], proof["replayedSequences"]);
        assert!(
            proof["runtimeObjectCount"].as_u64().unwrap_or(0) > 0,
            "EV proof must be backed by runtime objects: {proof:?}"
        );
    }
}
