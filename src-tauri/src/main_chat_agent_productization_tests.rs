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
        mock_ipc_origin.clone(),
    );
    context.runtime_authority_mut().__allow_command(
        "run_main_chat_external_live_productization_gate".into(),
        mock_ipc_origin.clone(),
    );
    context.runtime_authority_mut().__allow_command(
        "run_main_chat_agent_product_maturity_v2_final_readiness_gate".into(),
        mock_ipc_origin.clone(),
    );
    context.runtime_authority_mut().__allow_command(
        "run_main_chat_agent_beta_v1_readiness_gate".into(),
        mock_ipc_origin.clone(),
    );
    context.runtime_authority_mut().__allow_command(
        "run_main_chat_agent_stage2_readiness_gate".into(),
        mock_ipc_origin.clone(),
    );
    context.runtime_authority_mut().__allow_command(
        "validate_main_chat_agent_stage2_manual_dogfood_artifact".into(),
        mock_ipc_origin.clone(),
    );
    context.runtime_authority_mut().__allow_command(
        "run_main_chat_agent_stage1_dogfood_gate".into(),
        mock_ipc_origin,
    );
    context
}

#[test]
fn main_chat_agent_productization_v1_gate_accounts_for_all_default_scenarios_without_live_credit() {
    let report =
        crate::main_chat_agent_productization_eval::run_main_chat_agent_productization_v1_gate_report();

    assert_eq!(report.total_scenario_count, 93);
    assert_eq!(report.default_deterministic_scenario_count, 92);
    assert_eq!(report.external_live_excluded_count, 1);
    assert!(report.runtime_payload_snapshot_event_gate_passed);
    assert_eq!(
        report.readiness_semantics,
        "full_deterministic_productization_v1_runtime_ready"
    );
    assert_eq!(
        report.runtime_execution_scope,
        "default_deterministic_scenarios_runtime_backed_external_live_excluded"
    );
    assert!(
        report.full_productization_v1_complete,
        "full Productization v1 deterministic completion requires all supported default scenarios to carry runtime proof"
    );
    assert_eq!(report.representative_runtime_group_count, 0);
    assert_eq!(report.representative_runtime_group_passed_count, 0);
    assert_eq!(report.runtime_required_group_count, 92);
    assert_eq!(report.runtime_required_group_passed_count, 92);
    assert_eq!(report.full_deterministic_runtime_scenario_count, 92);
    assert_eq!(
        report.full_deterministic_runtime_scenario_executed_count,
        92
    );
    assert!(report.future_work.is_empty());
    assert_eq!(
        report.event_semantics,
        "durable_replayable_delta_events_available_snapshot_backfill_excluded_from_live_credit"
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

    assert!(
        report.unsupported_scenarios.is_empty(),
        "Phase A requires MP-06 rollback to be supported by real lifecycle/materialized context evidence: {:?}",
        report.unsupported_scenarios
    );
}

#[test]
fn main_chat_agent_productization_v1_gate_requires_runtime_backed_default_scenarios() {
    let report =
        crate::main_chat_agent_productization_eval::run_main_chat_agent_productization_v1_gate_report();
    let scenarios =
        openlife_core::agent::main_chat_agent_productization_v1::main_chat_agent_product_scenarios(
        );

    let supported_default_ids = scenarios
        .iter()
        .filter(|scenario| {
            scenario.included_in_default_gate
                && scenario.run_mode
                    == openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductScenarioRunMode::DeterministicFixture
                && scenario.expectation
                    != openlife_core::agent::main_chat_agent_productization_v1::MainChatAgentProductScenarioExpectation::OptionalUnsupported
        })
        .map(|scenario| scenario.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(supported_default_ids.len(), 92);
    assert_eq!(report.runtime_required_group_evidence.len(), 92);

    for scenario_id in supported_default_ids {
        let proof = report
            .runtime_required_group_evidence
            .iter()
            .find(|proof| proof.scenario_id == scenario_id)
            .unwrap_or_else(|| panic!("missing runtime proof for {scenario_id}"));
        assert!(
            proof.passed,
            "runtime proof for {scenario_id} must pass: {:?}",
            proof.diagnostics
        );
        assert!(
            proof.runtime_object_count > 0,
            "runtime proof for {scenario_id} must load/create concrete runtime objects"
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
                created_memory_ids: Vec::new(),
                rollback_event_ids: Vec::new(),
                materialized_view_versions: Vec::new(),
                inactive_memory_ids: Vec::new(),
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

#[test]
fn main_chat_product_maturity_v2_memory_lifecycle_eval_covers_mr_matrix() {
    let report = crate::main_chat_memory_lifecycle_eval::run_main_chat_memory_lifecycle_eval_gate();

    assert_eq!(report.scenario_count, 9);
    assert_eq!(report.default_gate_scenario_count, 9);
    assert_eq!(report.executed_scenario_count, 9);
    assert_eq!(report.passed_scenario_count, 9, "{:?}", report.proofs);
    assert_eq!(report.expected_blocker_count, 2);
    assert!(report.ready, "{:?}", report.blockers);
    for id in [
        "MR-01", "MR-02", "MR-03", "MR-04", "MR-05", "MR-06", "MR-07", "MR-08", "MR-09",
    ] {
        assert!(
            report.proofs.iter().any(|proof| proof.scenario_id == id),
            "missing {id}"
        );
    }
    let rollback = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "MR-03")
        .expect("MR-03 rollback proof");
    assert_eq!(rollback.rollback_event_ids.len(), 1);
    assert_eq!(rollback.memory_ids.len(), 1);
    assert!(
        rollback.materialized_view_versions.len() >= 2
            && rollback.materialized_view_versions[1] > rollback.materialized_view_versions[0],
        "MR-03 must prove changed materialized view version: {:?}",
        rollback.materialized_view_versions
    );
    assert!(rollback.ui_state.contains(&"memory_inactive".to_string()));
}

#[tokio::test]
async fn main_chat_product_maturity_v2_task_continuity_eval_covers_lt2_matrix() {
    let report =
        crate::main_chat_task_continuity_eval::run_main_chat_agent_product_maturity_v2_task_continuity_gate()
            .await;

    assert_eq!(report.scenario_count, 8);
    assert_eq!(report.default_gate_scenario_count, 8);
    assert_eq!(report.passed_scenario_count, 8, "{:?}", report.proofs);
    assert_eq!(report.expected_blocker_count, 3);
    assert!(report.ready, "{:?}", report.blockers);
    for id in [
        "LT2-01", "LT2-02", "LT2-03", "LT2-04", "LT2-05", "LT2-06", "LT2-07", "LT2-08",
    ] {
        assert!(
            report.proofs.iter().any(|proof| proof.scenario_id == id),
            "missing {id}"
        );
    }
    let stale = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LT2-06")
        .expect("LT2-06 stale proof");
    assert!(
        stale.diagnostics.contains(&"stale_context".to_string()),
        "LT2-06 must prove stale context diagnostic: {:?}",
        stale
    );
    assert!(
        stale
            .negative_assertions
            .contains(&"no_automatic_replay".to_string()),
        "LT2-06 must prove stale tasks are not automatically replayed: {:?}",
        stale
    );
    let changed_target = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LT2-04")
        .expect("LT2-04 changed-target proof");
    assert!(
        changed_target
            .diagnostics
            .contains(&"permission_scope_mismatch".to_string()),
        "LT2-04 must prove exact permission scope mismatch: {:?}",
        changed_target
    );
    let reopened = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LT2-08")
        .expect("LT2-08 reopen proof");
    assert!(
        reopened
            .runtime_evidence
            .contains(&"fresh_app_state_instance".to_string())
            && reopened
                .runtime_evidence
                .contains(&"persisted_task_detail".to_string()),
        "LT2-08 must prove task detail loads from a fresh app/state instance: {:?}",
        reopened
    );
}

#[tokio::test]
async fn main_chat_product_maturity_v2_skills_tool_eval_covers_sk2_matrix() {
    let report =
        crate::main_chat_skills_tools::run_main_chat_agent_product_maturity_v2_skills_gate().await;

    assert_eq!(report.scenario_count, 8);
    assert_eq!(report.default_gate_scenario_count, 8);
    assert_eq!(report.passed_scenario_count, 8, "{:?}", report.proofs);
    assert_eq!(report.expected_blocker_count, 2);
    assert!(report.ready, "{:?}", report.blockers);
    for id in [
        "SK2-01", "SK2-02", "SK2-03", "SK2-04", "SK2-05", "SK2-06", "SK2-07", "SK2-08",
    ] {
        let proof = report
            .proofs
            .iter()
            .find(|proof| proof.scenario_id == id)
            .unwrap_or_else(|| panic!("missing {id}"));
        assert!(
            proof.runtime_object_count > 0,
            "{id} must create or load real runtime objects, not schema-only proof: {:?}",
            proof
        );
        assert!(
            !proof.ui_state.is_empty(),
            "{id} must include UI-state proof: {:?}",
            proof
        );
    }

    let selected = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "SK2-01")
        .expect("SK2-01 selected skill proof");
    assert_eq!(selected.selected_skill_ids, vec!["phase_e_review"]);
    assert!(
        selected
            .runtime_evidence
            .contains(&"bounded_instruction_digest".to_string())
            && selected
                .runtime_evidence
                .contains(&"selected_skill_context_included".to_string()),
        "selected skill proof must include bounded context digest evidence: {:?}",
        selected
    );
    assert!(
        selected
            .negative_assertions
            .contains(&"skill_does_not_override_policy".to_string()),
        "selected skill must remain context, not authority: {:?}",
        selected
    );

    let unselected = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "SK2-05")
        .expect("SK2-05 unselected proof");
    assert!(
        unselected
            .negative_assertions
            .contains(&"unselected_skill_not_injected".to_string()),
        "unselected skills must be absent from prompt/context evidence: {:?}",
        unselected
    );

    let write_like = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "SK2-04")
        .expect("SK2-04 write-like proof");
    assert!(
        write_like.expected_blocker
            && write_like.blocker_ids.len() == 1
            && write_like
                .negative_assertions
                .contains(&"write_like_tool_not_rendered_as_safe_read".to_string()),
        "write-like tools must be blocker/proposal/permission paths, not normal safe reads: {:?}",
        write_like
    );

    let failure = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "SK2-07")
        .expect("SK2-07 failure proof");
    assert!(
        failure
            .controls
            .iter()
            .any(|control| control == "retry_tool" || control == "switch_tool"),
        "tool failure must expose retry or alternative control: {:?}",
        failure
    );
}

#[test]
fn main_chat_external_live_productization_gate_defines_six_opt_in_live_prod_rows() {
    let scenarios = crate::main_chat_live_productization_eval::main_chat_live_product_scenarios();
    let ids = scenarios
        .iter()
        .map(|scenario| scenario.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "LIVE-PROD-01",
            "LIVE-PROD-02",
            "LIVE-PROD-03",
            "LIVE-PROD-04",
            "LIVE-PROD-05",
            "LIVE-PROD-06",
        ]
    );
    assert!(scenarios.iter().all(|scenario| !scenario.default_gate));
    assert!(scenarios
        .iter()
        .all(|scenario| scenario.run_mode == "external_live_opt_in"));
    assert!(scenarios
        .iter()
        .all(|scenario| scenario.expected_outcome == "pass"));
}

#[test]
fn main_chat_external_live_productization_gate_blocks_without_opt_in() {
    let report = crate::main_chat_live_productization_eval::
        build_main_chat_external_live_productization_gate_report(
            false,
            vec!["explicit_live_eval_required".into()],
            Vec::new(),
        );

    assert_eq!(report.scenario_count, 6);
    assert_eq!(report.default_gate_scenario_count, 0);
    assert!(!report.ready);
    assert!(!report.live_provider_attempted);
    assert_eq!(report.passed_scenario_count, 0);
    assert_eq!(report.blocked_scenario_count, 6);
    assert!(report
        .blockers
        .contains(&"explicit_live_eval_required".to_string()));
    assert!(report
        .proofs
        .iter()
        .all(|proof| proof.status == "blocked" && !proof.passed));
}

#[test]
fn main_chat_external_live_productization_gate_rejects_local_provider_credit() {
    let mut evidence =
        crate::main_chat_live_productization_eval::test_live_product_evidence_for_scenario(
            "LIVE-PROD-01",
        );
    evidence.provider = "local_test_http".into();
    evidence.provider_endpoint_kind = "local_test_http".into();

    let report = crate::main_chat_live_productization_eval::
        build_main_chat_external_live_productization_gate_report(true, Vec::new(), vec![evidence]);

    assert!(!report.ready);
    assert_eq!(report.passed_scenario_count, 0);
    assert!(report
        .blockers
        .contains(&"live_product_external_provider_missing".to_string()));
    let proof = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LIVE-PROD-01")
        .expect("LIVE-PROD-01 proof");
    assert!(!proof.passed);
    assert!(proof
        .blockers
        .contains(&"live_product_external_provider_missing".to_string()));
}

#[test]
fn main_chat_external_live_productization_gate_requires_product_runtime_mapping() {
    let evidence = [
        "LIVE-PROD-01",
        "LIVE-PROD-02",
        "LIVE-PROD-03",
        "LIVE-PROD-04",
        "LIVE-PROD-05",
        "LIVE-PROD-06",
    ]
    .into_iter()
    .map(crate::main_chat_live_productization_eval::test_live_product_evidence_for_scenario)
    .collect::<Vec<_>>();

    let report = crate::main_chat_live_productization_eval::
        build_main_chat_external_live_productization_gate_report(true, Vec::new(), evidence);

    assert!(report.ready, "{:?}", report.blockers);
    assert!(report.live_provider_attempted);
    assert_eq!(report.scenario_count, 6);
    assert_eq!(report.passed_scenario_count, 6, "{:?}", report.proofs);
    assert_eq!(report.blocked_scenario_count, 0);
    assert_eq!(report.failed_scenario_count, 0);
    assert!(report.external_provider_invoked);
    assert!(!report.direct_writes_executed);
    assert!(!report.legacy_fallback_used);
    assert_eq!(
        report.readiness_semantics,
        "opt_in_external_live_product_evidence_only_default_readiness_unchanged"
    );

    let direct = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LIVE-PROD-01")
        .expect("LIVE-PROD-01 proof");
    assert!(direct.final_delivery_id.is_some());
    assert!(direct.action_ids.is_empty());
    assert!(direct.observation_ids.is_empty());
    assert!(direct
        .ui_state_assertions
        .contains(&"direct_answer_no_tool_timeline".to_string()));

    let web = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LIVE-PROD-02")
        .expect("LIVE-PROD-02 proof");
    assert!(!web.action_ids.is_empty());
    assert!(!web.observation_ids.is_empty());
    assert!(web
        .runtime_evidence
        .contains(&"web_observation_source".to_string()));
    assert!(web
        .negative_assertions
        .contains(&"no_fake_source".to_string()));

    let mcp = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LIVE-PROD-03")
        .expect("LIVE-PROD-03 proof");
    assert!(mcp
        .runtime_evidence
        .contains(&"candidate_ranking_trace".to_string()));
    assert!(mcp
        .runtime_evidence
        .contains(&"selected_target".to_string()));

    let proposal = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LIVE-PROD-04")
        .expect("LIVE-PROD-04 proof");
    assert_eq!(proposal.proposal_ids.len(), 1);
    assert!(proposal
        .runtime_evidence
        .contains(&"exact_action_proposal".to_string()));
    assert!(proposal
        .negative_assertions
        .contains(&"no_overlapping_read_success".to_string()));

    let recovery = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LIVE-PROD-05")
        .expect("LIVE-PROD-05 proof");
    assert!(!recovery.blocker_ids.is_empty());
    assert!(recovery.controls.contains(&"retry".to_string()));
    assert!(recovery.controls.contains(&"cancel".to_string()));

    let deltas = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LIVE-PROD-06")
        .expect("LIVE-PROD-06 proof");
    assert!(deltas.event_sequence_start.is_some());
    assert!(deltas.event_sequence_end > deltas.event_sequence_start);
    for event_type in [
        "route.selected",
        "action.queued",
        "observation.created",
        "final_delivery.created",
    ] {
        assert!(
            deltas.event_types.contains(&event_type.to_string()),
            "LIVE-PROD-06 must include {event_type}: {:?}",
            deltas.event_types
        );
    }
}

#[test]
fn main_chat_external_live_productization_gate_allows_tool_permission_proposal_observation() {
    let mut evidence =
        crate::main_chat_live_productization_eval::test_live_product_evidence_for_scenario(
            "LIVE-PROD-04",
        );
    evidence.observation_ids = vec!["proposal-observation".into()];
    evidence.ui_state_assertions.push("observation_card".into());

    let report = crate::main_chat_live_productization_eval::
        build_main_chat_external_live_productization_gate_report(true, Vec::new(), vec![evidence]);

    let proof = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "LIVE-PROD-04")
        .expect("LIVE-PROD-04 proof");
    assert!(
        proof.passed,
        "proposal observation should not count as read-success overlap: {:?}",
        proof.blockers
    );
    assert!(!proof
        .blockers
        .contains(&"live_product_tool_permission_read_overlap".to_string()));
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
        92
    );
    assert_eq!(
        response["eventSemantics"].as_str().unwrap(),
        "durable_replayable_delta_events_available_snapshot_backfill_excluded_from_live_credit"
    );
    assert_eq!(response["externalLiveExcludedCount"], 1);
    assert!(
        response["fullProductizationV1Complete"]
            .as_bool()
            .unwrap_or(false),
        "command report must claim full deterministic Productization v1 completion only after runtime proof passes"
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

#[tokio::test]
async fn run_main_chat_external_live_productization_gate_command_fails_closed_without_opt_in() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            crate::commands::agent_runtime::run_main_chat_external_live_productization_gate
        ])
        .build(productization_command_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");

    let response = tauri::test::get_ipc_response(
        &webview,
        productization_invoke_request(
            "run_main_chat_external_live_productization_gate",
            serde_json::json!({}),
        ),
    )
    .expect("live productization gate response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize live productization gate response");

    assert_eq!(
        response["reportKind"].as_str().unwrap(),
        "main_chat_external_live_productization_gate"
    );
    assert_eq!(response["scenarioCount"].as_u64().unwrap(), 6);
    assert_eq!(response["defaultGateScenarioCount"].as_u64().unwrap(), 0);
    assert!(!response["ready"].as_bool().unwrap());
    assert!(!response["liveProviderAttempted"].as_bool().unwrap());
    assert_eq!(response["blockedScenarioCount"].as_u64().unwrap(), 6);
    assert!(response["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|blocker| blocker.as_str() == Some("explicit_live_eval_required")));
    assert!(response["deterministicReadinessUnchanged"]
        .as_bool()
        .unwrap());

    let run_count = state
        .agent_run_store
        .as_ref()
        .expect("agent run store")
        .lock()
        .await
        .list_runs(10, 0)
        .expect("list runs")
        .len();
    let event_count = state
        .main_chat_agent_event_store
        .as_ref()
        .expect("event store")
        .lock()
        .await
        .list("missing-live-product-task", 0, 100)
        .expect("list events")
        .len();
    assert_eq!(run_count, 0);
    assert_eq!(event_count, 0);
}

#[tokio::test]
async fn main_chat_product_maturity_v2_final_readiness_aggregates_all_phase_gates_with_live_separate(
) {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let report = crate::main_chat_product_maturity_v2_final_readiness::run_main_chat_agent_product_maturity_v2_final_readiness_report_with_state(
        &state,
        false,
    )
    .await
    .expect("final readiness report");

    assert_eq!(
        report.report_kind,
        "main_chat_agent_product_maturity_v2_final_readiness_gate"
    );
    assert_eq!(report.default_deterministic_scenario_count, 43);
    assert_eq!(report.external_live_scenario_count, 6);
    assert_eq!(
        report.default_readiness_scope,
        "MR_EV_PI_LT2_SK2_deterministic_only"
    );
    assert_eq!(
        report.opt_in_live_readiness_scope,
        "LIVE_PROD_external_live_opt_in_only"
    );
    assert_eq!(report.default_live_prod_excluded_count, 6);
    assert_eq!(report.deterministic_readiness_status, "ready");
    assert!(report.deterministic_ready);
    assert_eq!(report.opt_in_live_readiness_status, "blocked");
    assert!(!report.opt_in_live_ready);
    assert_eq!(
        report.final_readiness_status,
        "blocked_live_productization_not_ready"
    );
    assert!(!report.final_ready);
    assert!(report.unsupported_scenarios.is_empty());
    assert!(report.future_scenarios.is_empty());
    assert!(report
        .blockers
        .contains(&"explicit_live_eval_required".to_string()));

    let phase_ids = report
        .phase_counts
        .iter()
        .map(|phase| phase.phase_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        phase_ids,
        vec!["phase_a", "phase_b", "phase_c", "phase_d", "phase_e", "phase_f"]
    );

    let memory = report
        .phase_counts
        .iter()
        .find(|phase| phase.capability_group == "memory_lifecycle")
        .expect("memory lifecycle phase");
    assert_eq!(memory.scenario_count, 9);
    assert_eq!(memory.passed, 7);
    assert_eq!(memory.expected_blocker, 2);
    assert_eq!(memory.failed, 0);
    assert_eq!(memory.blocked, 0);
    assert_eq!(memory.status, "ready");
    assert!(memory.unsupported_scenarios.is_empty());

    let event = report
        .phase_counts
        .iter()
        .find(|phase| phase.capability_group == "event_delta_stream")
        .expect("event delta phase");
    assert_eq!(event.scenario_count, 8);
    assert_eq!(event.passed, 8);
    assert_eq!(event.expected_blocker, 0);
    assert_eq!(event.status, "ready");

    let plan = report
        .phase_counts
        .iter()
        .find(|phase| phase.capability_group == "plan_interaction")
        .expect("plan phase");
    assert_eq!(plan.scenario_count, 10);
    assert_eq!(plan.passed, 7);
    assert_eq!(plan.expected_blocker, 3);

    let task = report
        .phase_counts
        .iter()
        .find(|phase| phase.capability_group == "task_continuity")
        .expect("task phase");
    assert_eq!(task.scenario_count, 8);
    assert_eq!(task.passed, 5);
    assert_eq!(task.expected_blocker, 3);

    let skills = report
        .phase_counts
        .iter()
        .find(|phase| phase.capability_group == "skills_tools_surface")
        .expect("skills phase");
    assert_eq!(skills.scenario_count, 8);
    assert_eq!(skills.passed, 6);
    assert_eq!(skills.expected_blocker, 2);

    let live = report
        .phase_counts
        .iter()
        .find(|phase| phase.capability_group == "external_live_productization")
        .expect("live phase");
    assert_eq!(live.scenario_count, 6);
    assert_eq!(live.passed, 0);
    assert_eq!(live.expected_blocker, 0);
    assert_eq!(live.failed, 0);
    assert_eq!(live.blocked, 6);
    assert_eq!(live.status, "blocked");
    assert!(live
        .blockers
        .contains(&"explicit_live_eval_required".to_string()));

    assert!(report
        .supported_scenarios
        .iter()
        .any(|scenario| scenario.scenario_id == "MR-03"));
    assert!(report
        .supported_scenarios
        .iter()
        .any(|scenario| scenario.scenario_id == "EV-05"));
    assert!(report
        .blocked_scenarios
        .iter()
        .any(|scenario| scenario.scenario_id == "MR-04" && scenario.reason == "expected_blocker"));
    assert!(report
        .blocked_scenarios
        .iter()
        .any(|scenario| scenario.scenario_id == "LIVE-PROD-01"
            && scenario.reason == "explicit_live_eval_required"));
}

#[tokio::test]
async fn run_main_chat_product_maturity_v2_final_readiness_command_returns_read_only_report() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            crate::commands::agent_runtime::run_main_chat_agent_product_maturity_v2_final_readiness_gate
        ])
        .build(productization_command_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");

    let response = tauri::test::get_ipc_response(
        &webview,
        productization_invoke_request(
            "run_main_chat_agent_product_maturity_v2_final_readiness_gate",
            serde_json::json!({}),
        ),
    )
    .expect("final readiness response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize final readiness response");

    assert_eq!(
        response["reportKind"].as_str().unwrap(),
        "main_chat_agent_product_maturity_v2_final_readiness_gate"
    );
    assert_eq!(
        response["deterministicReadinessStatus"].as_str().unwrap(),
        "ready"
    );
    assert_eq!(
        response["optInLiveReadinessStatus"].as_str().unwrap(),
        "blocked"
    );
    assert_eq!(
        response["finalReadinessStatus"].as_str().unwrap(),
        "blocked_live_productization_not_ready"
    );
    assert_eq!(
        response["defaultDeterministicScenarioCount"]
            .as_u64()
            .unwrap(),
        43
    );
    assert_eq!(response["externalLiveScenarioCount"].as_u64().unwrap(), 6);
    assert!(response["unsupportedScenarios"]
        .as_array()
        .unwrap()
        .is_empty());

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
    assert_eq!(run_count, 0);
    assert_eq!(proposal_count, 0);
}

#[tokio::test]
#[ignore = "requires OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1, network, and a real external provider API key"]
async fn main_chat_external_live_productization_gate_invokes_external_provider_when_opted_in() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.llm.provider = std::env::var("OPENLIFE_LIVE_EVAL_PROVIDER").unwrap_or_default();
        config.llm.openai_base = std::env::var("OPENLIFE_LIVE_EVAL_BASE").unwrap_or_default();
        config.llm.chat_model = std::env::var("OPENLIFE_LIVE_EVAL_MODEL").unwrap_or_default();
        config.llm.openai_key = std::env::var("OPENLIFE_LIVE_EVAL_API_KEY").unwrap_or_default();
        config.system.network_policy.enabled = true;
    }
    {
        let config = state.config.lock().await.clone();
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            config.local_model.clone(),
            false,
            config.llm.provider.clone(),
            config.llm.openai_base.clone(),
            config.llm.openai_key.clone(),
            config.llm.chat_model.clone(),
            config.llm.embedding_model.clone(),
            false,
        );
    }

    let report =
        crate::main_chat_live_productization_eval::run_main_chat_external_live_productization_gate_with_state(
            &state,
            true,
        )
        .await
        .expect("external live productization report");
    let audit = serde_json::to_string_pretty(&serde_json::json!({
        "ready": report.ready,
        "blockers": report.blockers,
        "proofs": report.proofs,
    }))
    .unwrap_or_else(|error| format!("serialize live product audit failed: {error}"));

    assert!(report.ready, "{audit}");
    assert_eq!(report.scenario_count, 6, "{audit}");
    assert_eq!(report.passed_scenario_count, 6, "{audit}");
    assert_eq!(report.default_gate_scenario_count, 0, "{audit}");
    assert!(report.live_provider_attempted, "{audit}");
    assert!(report.external_provider_invoked, "{audit}");
    assert!(!report.direct_writes_executed, "{audit}");
    assert!(!report.legacy_fallback_used, "{audit}");
    assert!(report.deterministic_readiness_unchanged, "{audit}");
    for id in [
        "LIVE-PROD-01",
        "LIVE-PROD-02",
        "LIVE-PROD-03",
        "LIVE-PROD-04",
        "LIVE-PROD-05",
        "LIVE-PROD-06",
    ] {
        assert!(
            report
                .proofs
                .iter()
                .any(|proof| proof.scenario_id == id && proof.passed),
            "{id} must pass with external live product evidence: {audit}"
        );
    }
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

#[tokio::test]
async fn main_chat_direct_answer_final_delivery_cites_bounded_context_sources_without_fake_actions()
{
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let result = crate::main_chat_send::send_message_with_state(
        "stage1-d25-context-inspection".into(),
        vec![openlife_core::llm::ChatMessage {
            role: "user".into(),
            content: "Inspect loaded knowledge assets.".into(),
        }],
        None,
        &state,
    )
    .await
    .expect("send context inspection message");

    let agent_state = result.agent_state.expect("context inspection agent state");
    assert_eq!(agent_state.route.strategy.as_str(), "direct_answer");
    assert!(
        agent_state.actions.is_empty(),
        "context source citation must not create fake action evidence"
    );
    assert!(
        agent_state.observations.is_empty(),
        "context source citation must not create fake action observation rows"
    );
    assert!(
        agent_state.final_delivery.as_ref().is_some_and(|delivery| {
            delivery.observations_used.iter().any(|observation| {
                observation.source_kind == "workspace_instruction"
                    || observation.source_kind == "materialized_file"
                    || observation.source_kind == "selected_personal_context"
            })
        }),
        "DirectAnswer final delivery should cite bounded context sources: {:?}",
        agent_state.final_delivery
    );
}

#[tokio::test]
async fn main_chat_agent_state_payload_exposes_plan_execute_controls_from_later_plan_transcript() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let result = crate::main_chat_send::send_message_with_state(
        "productization-plan-agent-state".into(),
        vec![openlife_core::llm::ChatMessage {
            role: "user".into(),
            content: "Draft a weekly plan and break this goal into steps.".into(),
        }],
        None,
        &state,
    )
    .await
    .expect("send PlanExecute message");

    assert_eq!(
        result
            .agent_ingress
            .as_ref()
            .expect("agent ingress")
            .selected_strategy,
        MainChatAgentStrategy::PlanExecute
    );
    let agent_state = result
        .agent_state
        .expect("PlanExecute send result must include agent_state");
    let plan = agent_state
        .plan
        .expect("PlanExecute agent_state must expose plan evidence");
    assert!(
        plan.plan_session_id.is_some(),
        "plan evidence should be enriched from the Plan transcript entry that carries planExecuteSessionId: {plan:?}"
    );
    assert!(
        plan.controls
            .iter()
            .any(|control| control == "confirm_plan"),
        "PlanExecute draft controls should be visible in agent_state: {:?}",
        plan.controls
    );
    let artifact = plan
        .artifact_view
        .as_ref()
        .expect("PlanExecute agent_state must expose product artifact view");
    assert_eq!(artifact.plan_id.as_str(), plan.plan_id.as_str());
    assert_eq!(
        artifact.plan_session_id,
        plan.plan_session_id
            .as_deref()
            .expect("PlanExecute session id")
    );
    assert_eq!(
        artifact.task_session_id.as_str(),
        agent_state.task.task_id.as_str()
    );
    assert_eq!(
        artifact.run_id.as_str(),
        plan.run_id.as_deref().expect("PlanExecute product run id")
    );
    assert!(
        artifact.body.contains(&artifact.plan_id)
            && artifact.body.contains(&artifact.plan_session_id)
            && artifact.body.contains("Steps"),
        "artifact body should be backend-built from plan/read-model evidence: {}",
        artifact.body
    );
    assert!(
        artifact
            .steps
            .iter()
            .any(|step| step.title == "Review current priorities"),
        "artifact should expose actual PlanExecute steps, not a debug-only summary: {:?}",
        artifact.steps
    );
    assert!(
        artifact
            .unknowns
            .iter()
            .any(|unknown| unknown.label == "opening hours"
                && unknown.detail.contains("source/tool evidence")),
        "realtime facts without source evidence must remain unknowns: {:?}",
        artifact.unknowns
    );
    assert_eq!(artifact.route_evidence.strategy, "plan_execute");
    assert!(
        !artifact.run_evidence.action_ids.is_empty()
            && !artifact.run_evidence.observation_ids.is_empty(),
        "artifact should bind to run/action evidence: {:?}",
        artifact.run_evidence
    );
    assert!(
        plan.steps
            .iter()
            .any(|step| step.controls.iter().any(|control| control == "skip_step")),
        "PlanExecute draft step controls should expose skip_step for Stage 1 D09: {:?}",
        plan.steps
    );
    assert!(
        agent_state.observations.iter().any(|observation| {
            observation.source_kind == "plan_execute"
                && observation.source_label == "plan_execute.create_session"
        }),
        "PlanExecute should expose governed observation evidence for Stage 1 D08: {:?}",
        agent_state.observations
    );
    assert!(
        agent_state
            .final_delivery
            .as_ref()
            .is_some_and(|delivery| !delivery.observations_used.is_empty()),
        "PlanExecute final delivery should cite observation evidence: {:?}",
        agent_state.final_delivery
    );
}

#[tokio::test]
async fn main_chat_stage1_d30_read_plus_memory_proposal_uses_real_read_and_review_proposal() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let result = crate::main_chat_send::send_message_with_state(
        "stage1-d30-read-plus-proposal".into(),
        vec![openlife_core::llm::ChatMessage {
            role: "user".into(),
            content:
                "Read file `dogfood/planning_notes.md` and create a memory proposal if useful."
                    .into(),
        }],
        None,
        &state,
    )
    .await
    .expect("send D30 message");

    let agent_state = result.agent_state.expect("D30 agent state");
    assert_eq!(agent_state.route.strategy.as_str(), "react_tool_execution");
    assert!(
        agent_state
            .actions
            .iter()
            .any(|action| action.action_type == "file.read" && action.status == "succeeded"),
        "D30 should perform a real governed file read: {:?}",
        agent_state.actions
    );
    assert!(
        agent_state
            .proposals
            .iter()
            .any(|proposal| proposal.proposal_type == "memory"),
        "D30 should create a Mailbox memory proposal after the read: {:?}",
        agent_state.proposals
    );
    assert!(
        agent_state.final_delivery.as_ref().is_some_and(|delivery| {
            !delivery.observations_used.is_empty()
                && !delivery.proposals_created.is_empty()
                && !delivery.pending_user_actions.is_empty()
        }),
        "D30 final delivery should cite read evidence and pending proposal: {:?}",
        agent_state.final_delivery
    );
}

#[tokio::test]
async fn main_chat_stage1_d31_plan_execute_blocks_risky_external_publish_step() {
    for (session_id, prompt) in [
        (
            "stage1-d31-plan-publish-blocker",
            "Plan the seeded policy-note publication task, but ask me before any risky external publish step.",
        ),
        (
            "stage1-d31-plan-post-blocker",
            "Plan the launch checklist and post to the external status page only after review.",
        ),
    ] {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let result = crate::main_chat_send::send_message_with_state(
            session_id.into(),
            vec![openlife_core::llm::ChatMessage {
                role: "user".into(),
                content: prompt.into(),
            }],
            None,
            &state,
        )
        .await
        .expect("send D31 message");

        let agent_state = result.agent_state.expect("D31 agent state");
        assert_eq!(agent_state.route.strategy.as_str(), "plan_execute");
        assert!(
            agent_state.plan.is_some(),
            "D31 should expose PlanExecute evidence"
        );
        assert!(
            agent_state.actions.iter().any(|action| {
                action.action_type == "external.write" && action.status == "blocked"
            }),
            "D31 should block the external publish/post action for {session_id}: {:?}",
            agent_state.actions
        );
        assert!(
            agent_state
                .blockers
                .iter()
                .any(|blocker| blocker.reason_code == "external_write_requires_confirmation"),
            "D31 should expose the external write blocker for {session_id}: {:?}",
            agent_state.blockers
        );
        assert!(
            agent_state.final_delivery.as_ref().is_some_and(|delivery| {
                !delivery.blockers.is_empty() && !delivery.pending_user_actions.is_empty()
            }),
            "D31 final delivery should show blocked work and pending user action for {session_id}: {:?}",
            agent_state.final_delivery
        );
    }
}

#[tokio::test]
async fn main_chat_agent_beta_v1_default_experience_maps_required_states_to_runtime_ui_evidence() {
    let report =
        crate::main_chat_agent_beta_v1_default_experience::run_main_chat_agent_beta_v1_default_experience_report()
            .await;

    assert_eq!(
        report.report_kind,
        "main_chat_agent_beta_v1_default_experience"
    );
    assert!(
        report.ready,
        "default experience blockers: {:?}",
        report.blockers
    );
    assert_eq!(report.phase, "phase_1_default_agent_experience");
    assert!(report.productization_v1_complete);
    assert_eq!(report.command_surface_failed_cases, 0);
    assert_eq!(report.command_surface_legacy_fallback_count, 0);
    assert_eq!(report.command_surface_silent_write_count, 0);
    assert!(report.command_surface_total_cases >= 38);

    let required_states = [
        "classifying",
        "answering",
        "planning",
        "action_queued",
        "action_running",
        "observation_ready",
        "permission_needed",
        "memory_candidate",
        "blocked",
        "retry_available",
        "completed",
    ];
    let covered_states = report
        .state_mappings
        .iter()
        .map(|mapping| mapping.state.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        covered_states,
        required_states
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    );

    for mapping in &report.state_mappings {
        assert!(
            mapping.verified,
            "{} mapping should be verified: {:?}",
            mapping.state, mapping.blockers
        );
        assert!(
            !mapping.runtime_evidence.is_empty(),
            "{} needs runtime evidence",
            mapping.state
        );
        assert!(
            !mapping.command_surface_evidence.is_empty(),
            "{} needs command-surface evidence",
            mapping.state
        );
        assert!(
            !mapping.ui_evidence.is_empty(),
            "{} needs UI evidence",
            mapping.state
        );
        let combined = format!(
            "{} {} {}",
            mapping.runtime_evidence.join(" "),
            mapping.command_surface_evidence.join(" "),
            mapping.ui_evidence.join(" ")
        );
        assert!(
            !combined.contains("assistant_text"),
            "{} must not use assistant text as state proof",
            mapping.state
        );
    }
}

#[tokio::test]
async fn main_chat_agent_beta_v1_readiness_gate_aggregates_default_evidence_with_live_separate() {
    let report =
        crate::main_chat_agent_beta_v1_readiness::run_main_chat_agent_beta_v1_readiness_report()
            .await
            .expect("beta readiness report");

    assert_eq!(report.report_kind, "main_chat_agent_beta_v1_readiness_gate");
    assert_eq!(
        report.readiness_semantics,
        "beta_v1_execution_first_default_deterministic_live_opt_in_separate"
    );
    assert_eq!(
        report.default_readiness_scope,
        "beta_v1_default_deterministic_local_only"
    );
    assert_eq!(
        report.opt_in_live_readiness_scope,
        "beta_v1_external_live_opt_in_only"
    );
    assert!(report.foundation_inventory_exists);
    assert!(report
        .foundation_inventory_items
        .iter()
        .any(
            |item| item.component == "Knowledge assets and context inventory"
                && item.status == "partial"
        ));
    assert_eq!(report.workstreams.len(), 5);
    assert!(report
        .workstreams
        .iter()
        .any(|workstream| workstream.workstream_id == "phase_5"
            && workstream.ready
            && workstream.status == "ready"));
    assert!(report
        .product_maturity_phase_counts
        .iter()
        .any(|phase| phase.capability_group == "memory_lifecycle"
            && phase.scenario_count == 9
            && phase.ready));
    assert_eq!(report.default_readiness_status, "ready");
    assert!(report.default_ready);
    assert!(!report.opt_in_live_ready);
    assert!(!report.external_live_attempted);
    assert_eq!(report.default_real_task_scenario_count, 28);
    assert_eq!(report.default_real_task_passed_count, 28);
    assert_eq!(report.opt_in_live_real_task_scenario_count, 2);
    assert_eq!(report.default_experience_required_state_count, 11);
    assert_eq!(report.default_experience_verified_state_count, 11);
    assert_eq!(report.product_maturity_default_scenario_count, 43);
    assert_eq!(report.command_surface_failed_cases, 0);
    assert!(report.command_surface_total_cases >= 38);
    assert_eq!(report.legacy_fallback_count, 0);
    assert_eq!(report.silent_durable_write_count, 0);
    assert!(report.no_silent_durable_writes);
    assert!(
        report.default_blockers.is_empty(),
        "{:?}",
        report.default_blockers
    );
    assert!(report
        .opt_in_live_blockers
        .contains(&"explicit_live_eval_required".to_string()));

    let dimensions = report
        .readiness_dimensions
        .iter()
        .map(|dimension| dimension.dimension.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        dimensions,
        [
            "Routing",
            "UI",
            "Events",
            "Memory",
            "Plan",
            "Tools",
            "Permissions",
            "Recovery",
            "Final delivery",
            "Live provider",
            "No silent writes",
            "No legacy bypass",
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
    );
    for dimension in &report.readiness_dimensions {
        if dimension.opt_in_only {
            assert_eq!(dimension.status, "blocked_opt_in_not_attempted");
        } else {
            assert_eq!(
                dimension.status, "ready",
                "{} should be ready: {:?}",
                dimension.dimension, dimension.blockers
            );
            assert!(
                !dimension.evidence.is_empty(),
                "{} needs structured evidence",
                dimension.dimension
            );
        }
    }
}

#[tokio::test]
async fn run_main_chat_agent_beta_v1_readiness_command_returns_isolated_report() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            crate::commands::agent_runtime::run_main_chat_agent_beta_v1_readiness_gate
        ])
        .build(productization_command_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");

    let response = tauri::test::get_ipc_response(
        &webview,
        productization_invoke_request(
            "run_main_chat_agent_beta_v1_readiness_gate",
            serde_json::json!({}),
        ),
    )
    .expect("beta readiness response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize beta readiness response");

    assert_eq!(
        response["reportKind"].as_str().unwrap(),
        "main_chat_agent_beta_v1_readiness_gate"
    );
    assert_eq!(
        response["defaultReadinessStatus"].as_str().unwrap(),
        "ready"
    );
    assert!(response["defaultReady"].as_bool().unwrap());
    assert!(!response["optInLiveReady"].as_bool().unwrap());
    assert!(!response["externalLiveAttempted"].as_bool().unwrap());
    assert_eq!(response["defaultRealTaskPassedCount"].as_u64().unwrap(), 28);
    assert_eq!(
        response["productMaturityDefaultScenarioCount"]
            .as_u64()
            .unwrap(),
        43
    );
    assert_eq!(response["legacyFallbackCount"].as_u64().unwrap(), 0);
    assert_eq!(response["silentDurableWriteCount"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn run_main_chat_agent_stage1_dogfood_command_returns_isolated_report() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            crate::commands::agent_runtime::run_main_chat_agent_stage1_dogfood_gate
        ])
        .build(productization_command_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");

    let response = tauri::test::get_ipc_response(
        &webview,
        productization_invoke_request(
            "run_main_chat_agent_stage1_dogfood_gate",
            serde_json::json!({}),
        ),
    )
    .expect("stage1 dogfood response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize stage1 dogfood response");

    assert_eq!(
        response["reportKind"].as_str().unwrap(),
        "main_chat_agent_stage1_dogfood_gate"
    );
    assert_eq!(response["defaultScenarioCount"].as_u64().unwrap(), 36);
    assert_eq!(response["scenarioCount"].as_u64().unwrap(), 40);
    assert_eq!(response["ordinaryChatScenarioCount"].as_u64().unwrap(), 24);
    assert_eq!(
        response["seededTaskControlScenarioCount"].as_u64().unwrap(),
        12
    );
    if response["defaultReady"].as_bool().unwrap() {
        assert!(response["defaultReady"].as_bool().unwrap());
        assert_eq!(
            response["readinessRecommendation"].as_str().unwrap(),
            "ready_for_engineering_dogfood"
        );
        assert_eq!(
            response["browserE2eReportPath"].as_str().unwrap(),
            "frontend/test-results/main-chat-stage1-dogfood-report.json"
        );
    } else {
        assert!(!response["defaultReady"].as_bool().unwrap());
        assert_eq!(
            response["readinessRecommendation"].as_str().unwrap(),
            "not_ready"
        );
        assert!(response["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|blocker| { blocker.as_str() == Some("not_ready_browser_e2e_blocked") }));
    }
    assert_eq!(
        response["seedManifest"]["seedWorkspaceRootKind"]
            .as_str()
            .unwrap(),
        "temp_isolated"
    );
}

#[tokio::test]
async fn run_stage2_readiness_gate_command_returns_auditable_report() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            crate::commands::agent_runtime::run_main_chat_agent_stage2_readiness_gate
        ])
        .build(productization_command_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");

    let response = tauri::test::get_ipc_response(
        &webview,
        productization_invoke_request(
            "run_main_chat_agent_stage2_readiness_gate",
            serde_json::json!({}),
        ),
    )
    .expect("stage 2 readiness response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize stage 2 readiness response");

    assert_eq!(
        response["reportKind"].as_str().unwrap(),
        "main_chat_agent_stage2_readiness_gate"
    );
    assert_eq!(
        response["schemaVersion"].as_str().unwrap(),
        "stage2-readiness-v1"
    );
    let recommendation = response["recommendation"].as_str().unwrap();
    assert!(
        matches!(
            recommendation,
            "ready_for_limited_internal_trial" | "not_ready_for_limited_internal_trial"
        ),
        "unexpected Stage 2 recommendation: {recommendation}"
    );
    assert_eq!(
        response["manualDogfood"]["requiredScenarioCount"]
            .as_u64()
            .unwrap(),
        24
    );
    assert_eq!(
        response["liveProvider"]["requiredScenarioCount"]
            .as_u64()
            .unwrap(),
        10
    );
    let blockers = response["blockers"].as_array().unwrap();
    if recommendation == "ready_for_limited_internal_trial" {
        assert!(
            blockers.is_empty(),
            "ready report has blockers: {blockers:?}"
        );
        assert!(response["manualDogfood"]["ready"].as_bool().unwrap());
        assert!(response["liveProvider"]["ready"].as_bool().unwrap());
    } else {
        assert!(
            !blockers.is_empty(),
            "not-ready report must expose named blockers"
        );
    }
    let live_ready = response["liveProvider"]["ready"].as_bool().unwrap();
    if live_ready {
        assert_eq!(
            response["liveProvider"]["passedScenarioCount"]
                .as_u64()
                .unwrap(),
            10
        );
        assert_eq!(
            response["liveProvider"]["modelInvokedCount"]
                .as_u64()
                .unwrap(),
            10
        );
        assert_eq!(
            response["liveProvider"]["mainChatInvokedCount"]
                .as_u64()
                .unwrap(),
            10
        );
        assert!(
            !blockers.iter().any(|blocker| blocker
                .as_str()
                .is_some_and(|label| label.starts_with("stage2_live_"))),
            "credited live artifact should not leave live blockers: {blockers:?}"
        );
    } else {
        assert!(
            blockers.iter().any(|blocker| blocker
                .as_str()
                .is_some_and(|label| label.starts_with("stage2_live_"))),
            "missing or blocked live evidence must remain visible: {blockers:?}"
        );
    }
}

#[tokio::test]
async fn validate_stage2_manual_dogfood_artifact_command_returns_focused_summary() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let app = tauri::test::mock_builder()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            crate::commands::agent_runtime::validate_main_chat_agent_stage2_manual_dogfood_artifact
        ])
        .build(productization_command_test_context())
        .expect("build mock tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock webview");

    let response = tauri::test::get_ipc_response(
        &webview,
        productization_invoke_request(
            "validate_main_chat_agent_stage2_manual_dogfood_artifact",
            serde_json::json!({}),
        ),
    )
    .expect("manual dogfood artifact validation response")
    .deserialize::<serde_json::Value>()
    .expect("deserialize manual dogfood artifact validation response");

    assert_eq!(
        response["requiredScenarioCount"].as_u64().unwrap(),
        24,
        "validator should return the focused manual dogfood summary"
    );
    assert!(
        response["missingScenarioIds"].as_array().is_some(),
        "validator should expose missing P0 manual scenario ids for operators"
    );
    assert!(
        response["blockers"].as_array().is_some(),
        "validator must expose named blockers without running full readiness"
    );
}

#[tokio::test]
async fn main_chat_agent_beta_v1_readiness_live_opt_in_is_audited_but_separate() {
    let report =
        crate::main_chat_agent_beta_v1_readiness::run_main_chat_agent_beta_v1_readiness_report_with_live_opt_in(true)
            .await
            .expect("beta readiness report");

    assert!(report.default_ready);
    assert_eq!(report.default_readiness_status, "ready");
    assert!(report.external_live_attempted);
    assert!(!report.opt_in_live_ready);
    assert!(report
        .opt_in_live_blockers
        .contains(&"beta_real_task_external_live_not_attempted".to_string()));
    assert!(
        report.default_blockers.is_empty(),
        "live opt-in blockers must not pollute deterministic default readiness: {:?}",
        report.default_blockers
    );
}

#[tokio::test]
async fn main_chat_agent_beta_v1_real_task_harness_defines_b1_b30_and_marks_phase2_ready() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    assert_eq!(
        report.report_kind,
        "main_chat_agent_beta_v1_real_task_verticals"
    );
    assert_eq!(report.phase, "phase_2_real_task_verticals");
    assert_eq!(report.fixture_count, 30);
    assert_eq!(report.default_readiness_scenario_count, 28);
    assert_eq!(report.opt_in_live_scenario_count, 2);
    assert_eq!(report.executed_default_scenario_count, 28);
    assert!(!report.external_live_attempted);
    assert!(
        report.ready,
        "Phase 2 should be ready once every required default real task has runtime/product evidence: {:?}",
        report.blockers
    );
    assert_eq!(report.failed_default_scenario_count, 0);
    assert!(
        report.blockers.is_empty(),
        "Phase 2 readiness should not carry stale blockers: {:?}",
        report.blockers
    );

    let expected_ids = (1..=30).map(|n| format!("B{n}")).collect::<Vec<_>>();
    let actual_ids = report
        .fixtures
        .iter()
        .map(|fixture| fixture.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(actual_ids, expected_ids);

    for fixture in &report.fixtures {
        assert!(
            !fixture.expected_outcome.is_empty(),
            "{} missing expected_outcome",
            fixture.id
        );
        assert!(
            matches!(
                fixture.expected_outcome.as_str(),
                "success" | "proposal" | "expected_blocker" | "opt_in_live"
            ),
            "{} has invalid expected_outcome {}",
            fixture.id,
            fixture.expected_outcome
        );
        assert!(
            !fixture.command_surface.is_empty(),
            "{} missing command_surface",
            fixture.id
        );
        if fixture.command_surface == "not_applicable_with_reason" {
            assert!(
                fixture
                    .not_applicable_with_reason
                    .as_deref()
                    .is_some_and(|reason| !reason.trim().is_empty()),
                "{} must explain not_applicable command_surface",
                fixture.id
            );
        }
        assert!(
            !fixture.required_ui_states.is_empty(),
            "{} must declare UI states",
            fixture.id
        );
        assert!(
            fixture
                .forbidden_evidence
                .iter()
                .any(|item| item == "silent_durable_write"),
            "{} must forbid silent durable writes",
            fixture.id
        );
    }

    for live_id in ["B25", "B26"] {
        let fixture = report
            .fixtures
            .iter()
            .find(|fixture| fixture.id == live_id)
            .expect("live fixture");
        assert!(!fixture.default_readiness);
        assert!(fixture.requires_live_provider);
        assert_eq!(fixture.expected_outcome, "opt_in_live");
    }

    for proof in &report.proofs {
        if proof.default_readiness {
            assert_eq!(proof.legacy_fallback_count, 0, "{}", proof.scenario_id);
            assert_eq!(proof.silent_durable_write_count, 0, "{}", proof.scenario_id);
            assert!(
                proof.fixture_contract_valid,
                "{} fixture contract should be valid: {:?}",
                proof.scenario_id, proof.blockers
            );
            assert!(
                proof.runtime_evidence_count > 0 || !proof.blockers.is_empty(),
                "{} must carry evidence or fail closed",
                proof.scenario_id
            );
        }
    }
}

#[tokio::test]
async fn main_chat_agent_beta_v1_real_task_command_surface_proofs_use_runtime_task_sessions() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    let command_surface_proofs = report
        .proofs
        .iter()
        .filter(|proof| {
            proof.default_readiness
                && proof
                    .evidence_sources
                    .iter()
                    .any(|source| source == "command_surface")
                && proof.runtime_evidence_count > 0
        })
        .collect::<Vec<_>>();
    assert!(
        !command_surface_proofs.is_empty(),
        "Real-task report should map command-surface scenarios to concrete runtime evidence"
    );

    for proof in command_surface_proofs {
        let task_session_id = proof.task_session_id.as_deref().unwrap_or_else(|| {
            panic!("{} missing command-surface task session", proof.scenario_id)
        });
        assert!(
            !task_session_id.starts_with("covered_by_existing_runtime_gate:"),
            "{} must not use placeholder task-session evidence: {}",
            proof.scenario_id,
            task_session_id
        );
        assert!(
            !task_session_id.trim().is_empty() && task_session_id == task_session_id.trim(),
            "{} task-session evidence must be a raw bounded runtime id: {:?}",
            proof.scenario_id,
            task_session_id
        );
        assert!(
            proof
                .evidence_sources
                .iter()
                .any(|source| source.starts_with("command_surface_case:")),
            "{} should retain the specific command-surface eval case used as proof: {:?}",
            proof.scenario_id,
            proof.evidence_sources
        );
    }
}

#[tokio::test]
async fn main_chat_agent_beta_v1_real_task_command_surface_proofs_include_runtime_record_counts() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    for scenario_id in ["B1", "B2", "B5", "B7", "B10", "B16", "B23", "B24"] {
        let proof = report
            .proofs
            .iter()
            .find(|proof| proof.scenario_id == scenario_id)
            .unwrap_or_else(|| panic!("missing proof for {scenario_id}"));
        assert!(
            proof
                .evidence_sources
                .iter()
                .any(|source| source.starts_with("command_surface_records:transcript=")),
            "{scenario_id} should expose runtime record counts from its command-surface case: {:?}",
            proof.evidence_sources
        );
    }
}

#[tokio::test]
async fn main_chat_agent_beta_v1_b3_session_search_runs_through_command_surface() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    let proof = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "B3")
        .expect("B3 proof");

    assert!(
        proof.passed,
        "B3 session search should pass with ordinary command-surface runtime evidence: {:?}",
        proof.blockers
    );
    assert_eq!(proof.actual_outcome, "success");
    assert_eq!(proof.command_surface, "both");
    assert!(
        proof.task_session_id.is_some(),
        "B3 should expose the runtime task session used for session.search"
    );
    assert!(
        proof.actions_attempted >= 1 && proof.actions_executed >= 1,
        "B3 should execute a governed session.search action: {:?}",
        proof
    );
    assert!(
        proof.observations_recorded >= 1,
        "B3 should record a session search observation"
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "command_surface"),
        "B3 should use ordinary send/stream command-surface evidence: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source.ends_with(":session_search_success")),
        "B3 should retain the concrete session_search_success eval case: {:?}",
        proof.evidence_sources
    );
}

#[tokio::test]
async fn main_chat_agent_beta_v1_b4_memory_context_runs_through_command_surface() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    let proof = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "B4")
        .expect("B4 proof");

    assert!(
        proof.passed,
        "B4 memory context should pass with ordinary command-surface runtime evidence: {:?}",
        proof.blockers
    );
    assert_eq!(proof.actual_outcome, "success");
    assert_eq!(proof.command_surface, "both");
    assert!(
        proof.task_session_id.is_some(),
        "B4 should expose the runtime task session used for bounded memory context"
    );
    assert!(
        proof.actions_attempted == 0 && proof.actions_executed == 0,
        "B4 should remain a traceable DirectAnswer, not invent a tool action: {:?}",
        proof
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "command_surface"),
        "B4 should use ordinary send/stream command-surface evidence: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source.ends_with(":memory_context_direct_answer_success")),
        "B4 should retain the concrete memory_context_direct_answer_success eval case: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "memory_context:active_records=1:loaded=true"),
        "B4 should prove one active accepted memory record was loaded as bounded context: {:?}",
        proof.evidence_sources
    );
}

#[tokio::test]
async fn main_chat_agent_beta_v1_b6_selected_skill_runs_through_command_surface() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    let proof = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "B6")
        .expect("B6 proof");

    assert!(
        proof.passed,
        "B6 selected skill should pass with ordinary command-surface runtime evidence: {:?}",
        proof.blockers
    );
    assert_eq!(proof.actual_outcome, "success");
    assert_eq!(proof.command_surface, "both");
    assert!(
        proof.task_session_id.is_some(),
        "B6 should expose the runtime task session used for selected-skill context"
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "command_surface"),
        "B6 should use ordinary send/stream command-surface evidence: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source.ends_with(":selected_skill_context_success")),
        "B6 should retain the concrete selected_skill_context_success eval case: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source
                == "selected_skill_context:phase_e_review:loaded=true:unselected=false"),
        "B6 should prove only the selected skill instruction was loaded: {:?}",
        proof.evidence_sources
    );
}

#[tokio::test]
async fn main_chat_agent_beta_v1_b8_plan_execute_runs_through_command_surface() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    let proof = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "B8")
        .expect("B8 proof");

    assert!(
        proof.passed,
        "B8 plan execute should pass with PlanInteraction and ordinary command-surface evidence: {:?}",
        proof.blockers
    );
    assert_eq!(proof.actual_outcome, "success");
    assert_eq!(proof.command_surface, "both");
    assert!(
        proof.task_session_id.is_some(),
        "B8 should expose the runtime task session used for PlanExecute draft"
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "command_surface"),
        "B8 should use ordinary send/stream command-surface evidence: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "plan_interaction"),
        "B8 should keep PlanInteraction gate evidence for execute/step coverage: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source.ends_with(":plan_execute_draft")),
        "B8 should retain the concrete plan_execute_draft eval case: {:?}",
        proof.evidence_sources
    );
}

#[tokio::test]
async fn main_chat_agent_beta_v1_b22_multi_read_runs_through_command_surface() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    let proof = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "B22")
        .expect("B22 proof");

    assert!(
        proof.passed,
        "B22 multi-read should pass with ordinary command-surface kernel-loop evidence: {:?}",
        proof.blockers
    );
    assert_eq!(proof.actual_outcome, "success");
    assert_eq!(proof.command_surface, "both");
    assert!(
        proof.task_session_id.is_some(),
        "B22 should expose the runtime task session used for the multi-read kernel loop"
    );
    assert!(
        proof.actions_attempted >= 2 && proof.actions_executed >= 2,
        "B22 should represent at least two read actions: {:?}",
        proof
    );
    assert!(
        proof.observations_recorded >= 2,
        "B22 should record at least two observations"
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "command_surface"),
        "B22 should use ordinary send/stream command-surface evidence: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source.ends_with(":multi_read_agent_loop_success")),
        "B22 should retain the concrete multi_read_agent_loop_success eval case: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "multi_read_agent_loop:tool_calls=2:observations=2"),
        "B22 should prove two tool calls and two observations from the kernel read loop: {:?}",
        proof.evidence_sources
    );
}

#[tokio::test]
async fn main_chat_agent_beta_v1_b21_memory_conflict_uses_evidence_graph_runtime() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    let proof = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "B21")
        .expect("B21 proof");

    assert!(
        proof.passed,
        "B21 memory conflict should pass with memory lifecycle and evidence graph evidence: {:?}",
        proof.blockers
    );
    assert_eq!(proof.actual_outcome, "success");
    assert_eq!(proof.command_surface, "both");
    assert!(
        proof.actions_attempted >= 1 && proof.actions_executed >= 1,
        "B21 should represent a governed memory.compare action: {:?}",
        proof
    );
    assert!(
        proof.observations_recorded >= 1,
        "B21 should record a conflict_state observation"
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "memory_lifecycle"),
        "B21 should use memory lifecycle gate evidence: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "memory_conflict:evidence_graph_conflict_count=2"),
        "B21 should prove visible conflict state from Evidence Graph: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "memory_conflict:lifecycle_records=2:conflict_ids=2"),
        "B21 should prove conflict ids are attached to accepted memory lifecycle records: {:?}",
        proof.evidence_sources
    );
}

#[tokio::test]
async fn main_chat_agent_beta_v1_b27_knowledge_assets_run_through_command_surface() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    let proof = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "B27")
        .expect("B27 proof");

    assert!(
        proof.passed,
        "B27 knowledge asset inspection should pass with ordinary command-surface context evidence: {:?}",
        proof.blockers
    );
    assert_eq!(proof.actual_outcome, "success");
    assert_eq!(proof.command_surface, "both");
    assert!(
        proof.task_session_id.is_some(),
        "B27 should expose the runtime task session used for knowledge asset inspection"
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "command_surface"),
        "B27 should use ordinary send/stream command-surface evidence: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source.ends_with(":knowledge_asset_context_success")),
        "B27 should retain the concrete knowledge_asset_context_success eval case: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "knowledge_assets:loaded=4:scope_digest_loaded=true"),
        "B27 should prove scoped AGENTS/SOUL/USER/MEMORY knowledge assets loaded without policy override: {:?}",
        proof.evidence_sources
    );
}

#[tokio::test]
async fn main_chat_agent_beta_v1_b28_knowledge_asset_edit_creates_review_proposal() {
    let report =
        crate::main_chat_agent_beta_v1_real_tasks::run_main_chat_agent_beta_v1_real_task_report()
            .await;

    let proof = report
        .proofs
        .iter()
        .find(|proof| proof.scenario_id == "B28")
        .expect("B28 proof");

    assert!(
        proof.passed,
        "B28 knowledge asset edit should pass with proposal-first command-surface evidence: {:?}",
        proof.blockers
    );
    assert_eq!(proof.actual_outcome, "proposal");
    assert_eq!(proof.command_surface, "both");
    assert!(
        proof.task_session_id.is_some(),
        "B28 should expose the runtime task session used for the edit proposal"
    );
    assert!(
        proof.proposals_created >= 1,
        "B28 should create a Mailbox proposal: {:?}",
        proof
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source == "command_surface"),
        "B28 should use ordinary send/stream command-surface evidence: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof
            .evidence_sources
            .iter()
            .any(|source| source.ends_with(":knowledge_asset_edit_proposal")),
        "B28 should retain the concrete knowledge_asset_edit_proposal eval case: {:?}",
        proof.evidence_sources
    );
    assert!(
        proof.evidence_sources.iter().any(|source| {
            source == "knowledge_asset_edit:proposal_created=true:proposed_diff=true:direct_write=false"
        }),
        "B28 should prove a proposed diff and no direct knowledge-file write: {:?}",
        proof.evidence_sources
    );
}
