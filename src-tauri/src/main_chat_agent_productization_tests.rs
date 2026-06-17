use openlife_core::agent::main_chat_agent_v1::{
    AgentTaskSessionDraft, ExecutionTranscriptEntryDraft, ExecutionTranscriptEntryKind,
    MainChatAgentStrategy,
};
use std::sync::Arc;

fn productization_invoke_request(
    cmd: &str,
    body: serde_json::Value,
) -> tauri::webview::InvokeRequest {
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

fn productization_command_test_context() -> tauri::Context<tauri::test::MockRuntime> {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    let mock_ipc_origin = tauri::utils::acl::ExecutionContext::Remote {
        url: "http://tauri.localhost"
            .parse()
            .expect("valid mock IPC origin pattern"),
    };
    context.runtime_authority_mut().__allow_command(
        "run_main_chat_agent_productization_v1_gate".into(),
        mock_ipc_origin,
    );
    context
}

#[test]
fn main_chat_agent_productization_v1_gate_accounts_for_default_scenarios_without_live_credit() {
    let report =
        crate::main_chat_agent_productization_eval::run_main_chat_agent_productization_v1_gate_report();

    assert_eq!(report.total_scenario_count, 93);
    assert_eq!(report.default_deterministic_scenario_count, 92);
    assert_eq!(report.external_live_excluded_count, 1);
    assert!(report.runtime_payload_snapshot_event_gate_passed);
    assert_eq!(
        report.readiness_semantics,
        "acceptance_hardening_representative_gate_ready"
    );
    assert_eq!(
        report.runtime_execution_scope,
        "representative_runtime_groups_only_full_92_scenario_runtime_execution_future_work"
    );
    assert!(
        !report.full_productization_v1_complete,
        "representative acceptance hardening must not be reported as full Productization v1 completion"
    );
    assert_eq!(report.representative_runtime_group_count, 11);
    assert_eq!(report.full_deterministic_runtime_scenario_count, 92);
    assert_eq!(
        report.full_deterministic_runtime_scenario_executed_count,
        11
    );
    assert!(
        report
            .future_work
            .contains(&"full_92_scenario_runtime_execution".to_string()),
        "report must make the remaining full deterministic execution work explicit"
    );
    assert_eq!(
        report.event_semantics,
        "snapshot_derived_ordered_events_not_live_delta_stream"
    );
    assert!(
        report.final_readiness_ready,
        "representative acceptance hardening gate should be ready when schema checks and required runtime groups pass"
    );
    assert!(
        report.blockers.is_empty(),
        "ready deterministic productization gate must not retain stale blockers: {:?}",
        report.blockers
    );
    assert!(
        !report
            .blockers
            .contains(&"ui_control_plane_not_implemented".to_string()),
        "implemented Agent Control Plane must not keep reporting the old UI blocker"
    );
    assert!(
        !report
            .blockers
            .contains(&"frontend_agent_control_plane_tests_missing".to_string()),
        "frontend Agent Control Plane coverage must be reflected in the gate blockers"
    );

    for route in [
        "direct_answer",
        "read_action",
        "react_tool_execution",
        "plan_execute",
        "memory_proposal",
        "permission_request",
        "task_control",
        "blocked",
    ] {
        assert!(
            report.route_counts.contains_key(route),
            "route accounting must include {route}"
        );
    }

    let task_control = report
        .route_counts
        .get("task_control")
        .expect("task_control route count");
    assert!(
        task_control.passed > 0,
        "mandatory task_control scenarios must execute and pass with prior-object references"
    );
    for (route, counts) in &report.route_counts {
        assert_eq!(
            counts.failed, 0,
            "{route} must not retain failed deterministic scenario rows after execution"
        );
    }
    assert!(
        report
            .route_counts
            .get("blocked")
            .expect("blocked route count")
            .expected_blocker
            > 0,
        "blocked scenarios should pass as expected blockers, not successful execution"
    );

    assert_eq!(report.unsupported_scenarios.len(), 1);
    assert_eq!(report.unsupported_scenarios[0].scenario_id, "MP-06");
    assert!(report.unsupported_scenarios[0].reason.contains("Rollback"));
}

#[test]
fn main_chat_agent_productization_v1_gate_requires_runtime_backed_representative_groups() {
    let report =
        crate::main_chat_agent_productization_eval::run_main_chat_agent_productization_v1_gate_report();

    for required_group in [
        "direct_answer",
        "file_read",
        "memory_session_read",
        "fixture_web_read",
        "registered_mcp_read",
        "multi_step_react_two_observations",
        "plan_execute_mvp",
        "memory_proposal_lifecycle_or_mp06_unsupported",
        "permission_request_exact_action",
        "task_control_resume_retry_cancel",
        "final_delivery_separation",
    ] {
        let proof = report
            .runtime_required_group_evidence
            .iter()
            .find(|proof| proof.group == required_group)
            .unwrap_or_else(|| panic!("missing runtime proof for {required_group}"));
        assert!(
            proof.passed,
            "runtime proof for {required_group} must pass: {:?}",
            proof.diagnostics
        );
        assert!(
            proof.runtime_object_count > 0,
            "runtime proof for {required_group} must load/create concrete runtime objects"
        );
    }
}

#[test]
fn main_chat_agent_productization_v1_gate_fails_schema_only_runtime_executor() {
    let report =
        crate::main_chat_agent_productization_eval::run_main_chat_agent_productization_v1_gate_report_with_runtime(
            |_| Ok(crate::main_chat_agent_productization_eval::ProductScenarioRuntimeProof {
                scenario_id: "schema-only".into(),
                group: "schema_only".into(),
                passed: true,
                runtime_object_count: 0,
                observation_count: 0,
                created_action_ids: Vec::new(),
                created_observation_ids: Vec::new(),
                created_proposal_ids: Vec::new(),
                final_delivery_id: None,
                diagnostics: Vec::new(),
            }),
        );

    assert!(
        !report.final_readiness_ready,
        "productization readiness must not be earned by schema-only proof"
    );
    assert!(
        report
            .blockers
            .contains(&"runtime_required_scenarios_not_executed".to_string()),
        "missing runtime objects should be a visible readiness blocker: {:?}",
        report.blockers
    );
}

#[tokio::test]
async fn run_main_chat_agent_productization_v1_gate_command_returns_auditable_read_only_report() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            crate::commands::agent_runtime::run_main_chat_agent_productization_v1_gate
        ])
        .build(productization_command_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");

    let response = tauri::test::get_ipc_response(
        &webview,
        productization_invoke_request(
            "run_main_chat_agent_productization_v1_gate",
            serde_json::json!({}),
        ),
    )
    .expect("productization gate response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize productization gate response");

    assert_eq!(response["totalScenarioCount"], 93);
    assert_eq!(
        response["runtimeRequiredGroupEvidence"]
            .as_array()
            .expect("runtime evidence array")
            .len(),
        11
    );
    assert_eq!(
        response["eventSemantics"].as_str().unwrap(),
        "snapshot_derived_ordered_events_not_live_delta_stream"
    );
    assert_eq!(response["externalLiveExcludedCount"], 1);
    assert!(
        !response["fullProductizationV1Complete"]
            .as_bool()
            .unwrap_or(true),
        "command report must not claim full Productization v1 completion"
    );

    let run_count = state
        .agent_run_store
        .as_ref()
        .expect("agent run store")
        .lock()
        .await
        .list_runs(10, 0)
        .expect("list runs")
        .len();
    let proposal_count = state
        .proposal_store
        .as_ref()
        .expect("proposal store")
        .lock()
        .await
        .list_all_proposals(10, 0)
        .expect("list proposals")
        .len();
    assert_eq!(
        run_count, 0,
        "gate command must not write app AgentRun state"
    );
    assert_eq!(
        proposal_count, 0,
        "gate command must not write app proposal state"
    );
}

#[test]
fn main_chat_agent_productization_v1_task_control_requires_existing_target_runtime_object() {
    let proof =
        crate::main_chat_agent_productization_eval::productization_task_control_missing_target_runtime_proof();

    assert!(
        !proof.passed,
        "task_control proof must fail when the target runtime object is missing"
    );
    assert!(
        proof
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic == "target_object_missing"),
        "missing target must be visible in diagnostics: {:?}",
        proof.diagnostics
    );
    assert!(
        proof
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic == "action_missing"),
        "task_control retry must use real prior action lookup, not fixture target text: {:?}",
        proof.diagnostics
    );
}

#[tokio::test]
async fn main_chat_agent_state_payload_fails_closed_when_task_session_is_missing() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let snapshot = crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn(
        &state,
        Some("missing-productization-task-session"),
        Some("run-missing-productization-task-session"),
    )
    .await
    .expect("governed task session id should produce diagnostics snapshot");

    let gap_codes = snapshot
        .diagnostics
        .iter()
        .map(|gap| gap.gap_code.as_str())
        .collect::<Vec<_>>();
    assert!(
        gap_codes.contains(&"agent_state_session_not_found"),
        "missing session must be visible instead of silently dropping agent_state: {:?}",
        snapshot.diagnostics
    );
    assert_eq!(snapshot.route.strategy.as_str(), "unknown");
    assert!(snapshot.actions.is_empty());
    assert!(snapshot.observations.is_empty());
    assert!(snapshot.final_delivery.is_none());
}

#[tokio::test]
async fn main_chat_agent_state_payload_reports_missing_run_evidence() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let session_store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("session store");
        session_store
            .lock()
            .await
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "productization-missing-run".into(),
                user_goal: "Assemble state with a missing run.".into(),
                selected_strategy: MainChatAgentStrategy::DirectAnswer,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create session")
    };

    let snapshot = crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn(
        &state,
        Some(&session.id),
        Some("run-does-not-exist"),
    )
    .await
    .expect("missing run should still assemble diagnostic state");
    assert!(
        snapshot
            .diagnostics
            .iter()
            .any(|gap| gap.gap_code == "missing_run_identity"),
        "missing run must remain visible in diagnostics: {:?}",
        snapshot.diagnostics
    );
}

#[tokio::test]
async fn main_chat_agent_state_payload_reports_missing_action_queue_store() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let session_store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("session store");
        let store = session_store.lock().await;
        let session = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "productization-missing-action-queue".into(),
                user_goal: "Assemble state with missing queue store.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: Some("Queue a read action.".into()),
                context_snapshot_refs: vec![],
            })
            .expect("create session");
        store
            .record_action_queue_id(&session.id, "action-queue-store-missing")
            .expect("record action id")
    };
    let mut state_without_queue = (*state).clone();
    state_without_queue.main_chat_action_queue_store = None;
    let state_without_queue = Arc::new(state_without_queue);

    let snapshot = crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn(
        &state_without_queue,
        Some(&session.id),
        None,
    )
    .await
    .expect("missing action queue store should produce diagnostics");
    let gap_codes = snapshot
        .diagnostics
        .iter()
        .map(|gap| gap.gap_code.as_str())
        .collect::<Vec<_>>();
    assert!(
        gap_codes.contains(&"agent_state_action_queue_store_unavailable"),
        "missing queue store must be distinguished from an empty action list: {:?}",
        snapshot.diagnostics
    );
    assert!(gap_codes.contains(&"missing_action_evidence"));
}

#[tokio::test]
async fn main_chat_agent_state_payload_reports_missing_proposal_evidence() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let session_store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("session store");
        let store = session_store.lock().await;
        let session = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "productization-missing-proposal".into(),
                user_goal: "Assemble state with a missing proposal reference.".into(),
                selected_strategy: MainChatAgentStrategy::MemoryProposal,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create session");
        store
            .set_pending_blockers(&session.id, vec!["proposal:proposal-not-found".into()])
            .expect("record pending proposal blocker")
    };

    let snapshot = crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn(
        &state,
        Some(&session.id),
        None,
    )
    .await
    .expect("missing proposal should produce diagnostics");
    assert!(snapshot.proposals.is_empty());
    assert!(
        snapshot
            .diagnostics
            .iter()
            .any(|gap| gap.gap_code == "missing_proposal_evidence"),
        "missing proposal references must be visible in diagnostics: {:?}",
        snapshot.diagnostics
    );
}

#[tokio::test]
async fn main_chat_agent_state_payload_reports_transcript_observation_without_action_evidence() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let session = {
        let session_store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("session store");
        let store = session_store.lock().await;
        let session = store
            .create_session(AgentTaskSessionDraft {
                chat_session_id: "productization-missing-action-evidence".into(),
                user_goal: "Observation transcript lacks matching action evidence.".into(),
                selected_strategy: MainChatAgentStrategy::ReActToolExecution,
                current_plan_summary: None,
                context_snapshot_refs: vec![],
            })
            .expect("create session");
        store
            .append_transcript_entry(ExecutionTranscriptEntryDraft {
                session_id: session.id.clone(),
                kind: ExecutionTranscriptEntryKind::Observation,
                summary: "Assistant text claims an observation.".into(),
                metadata: serde_json::json!({
                    "actionId": "missing-runtime-action",
                    "sourceKind": "file",
                    "sourceLabel": "AGENTS.md"
                }),
            })
            .expect("append observation");
        session
    };

    let snapshot = crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn(
        &state,
        Some(&session.id),
        None,
    )
    .await
    .expect("missing action evidence should produce diagnostics");
    assert!(snapshot.actions.is_empty());
    assert!(snapshot.observations.is_empty());
    assert!(
        snapshot
            .diagnostics
            .iter()
            .any(|gap| gap.gap_code == "missing_observation_evidence"),
        "transcript/action mismatch must remain visible: {:?}",
        snapshot.diagnostics
    );
}

#[tokio::test]
async fn main_chat_agent_productization_v1_send_result_includes_runtime_backed_agent_state() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let result = crate::main_chat_send::send_message_with_state(
        "productization-send-agent-state".into(),
        vec![openlife_core::llm::ChatMessage {
            role: "user".into(),
            content: "用两句话解释 ReAct。".into(),
        }],
        None,
        &state,
    )
    .await
    .expect("send message");

    let agent_state = result
        .agent_state
        .expect("ordinary send result must include runtime-backed agent_state payload");
    assert_eq!(
        agent_state.task.conversation_id,
        "productization-send-agent-state"
    );
    assert_eq!(agent_state.route.strategy.as_str(), "direct_answer");
    assert!(agent_state.final_delivery.is_some());
    assert!(
        agent_state.actions.is_empty(),
        "DirectAnswer must not render a fake action timeline"
    );
    assert!(
        agent_state.observations.is_empty(),
        "DirectAnswer context/model transcript must not become fake action observations"
    );
}
