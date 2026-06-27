use crate::main_chat_step6_product_acceptance::{
    build_step6_product_acceptance_report_for_tests, clean_step6_final_gate_summary_for_tests,
    prepare_main_chat_step6_live_provider_eval_state_with_env,
    refresh_step6_browser_report_digest_for_tests, step6_browser_report_for_tests,
    step6_live_provider_scenario_credit, step6_observed_journey_for_tests,
    Step6LiveProviderEvalEnv, Step6ObservedJourney,
};

fn required_step6_ids() -> [&'static str; 11] {
    [
        "S6-CLOCK",
        "S6-ROUTE",
        "S6-TOOLS",
        "S6-FILE",
        "S6-DIRECT-SELF",
        "S6-PROPOSAL",
        "S6-BLOCKED",
        "S6-PERMISSION",
        "S6-LIVE-WEB",
        "S6-LIVE-MCP",
        "S6-RECOVERY",
    ]
}

fn passing_rows() -> Vec<Step6ObservedJourney> {
    required_step6_ids()
        .into_iter()
        .map(step6_observed_journey_for_tests)
        .collect()
}

fn live_scenario_report_for_step6_tests(
    scenario: &str,
    credited: bool,
) -> crate::main_chat_final_gate::MainChatLiveProviderScenarioReport {
    crate::main_chat_final_gate::MainChatLiveProviderScenarioReport {
        scenario: scenario.into(),
        ready: credited,
        credited,
        status: "completed".into(),
        provider_endpoint_kind: "external_provider".into(),
        blockers: Vec::new(),
        live_provider_invocation_allowed: true,
        main_chat_invoked: true,
        model_invoked: true,
        direct_writes_executed: false,
        legacy_fallback_used: false,
        agent_loop_succeeded: credited,
        single_step_fallback_used: false,
        agent_loop_action_status: Some("succeeded".into()),
        mcp_read_target_resolved: false,
        tool_permission_proposal_created: false,
        tool_selection_candidate_count: 1,
        model_selected_allowed_tool: credited,
        model_selected_execution_policy_validated: credited,
        model_selected_execution_allowed: credited,
        model_selected_governed_arguments: credited,
        model_selected_candidate_id: Some(scenario.into()),
        model_selected_candidate_target: Some(scenario.into()),
        run_id_present: true,
        task_session_id_present: true,
        response_preview_present: true,
    }
}

fn mark_live_rows_blocked(rows: &mut [Step6ObservedJourney]) {
    for id in ["S6-LIVE-WEB", "S6-LIVE-MCP"] {
        let index = rows
            .iter()
            .position(|row| row.journey_id == id)
            .expect("live row");
        rows[index].answer_evidence.clear();
        rows[index].runtime_evidence.clear();
        rows[index].ui_status_evidence = vec!["blocked_live_evidence".into()];
        rows[index].trace_evidence.clear();
        rows[index].blockers = vec!["explicit_live_eval_required".into()];
        rows[index].live_evidence_kind.clear();
        rows[index].external_live_credit = false;
        rows[index].blocked_live_evidence_report = true;
        rows[index].external_live_status = "blocked_live_evidence".into();
        rows[index].external_live_provider_kind = None;
        rows[index].observed_via = "blocked_live_evidence_report".into();
        rows[index].entry_point = "blocked_live_evidence_report".into();
        rows[index].route_strategy = "blocked_external_live".into();
        rows[index].task_session_id.clear();
        rows[index].run_id.clear();
    }
}

#[test]
fn main_chat_step6_product_acceptance_credits_complete_structured_product_journeys() {
    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(passing_rows())),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(report.local_deterministic_ready, "{:?}", report.blockers);
    assert!(report.external_live_ready, "{:?}", report.blockers);
    assert!(report.overall_ready, "{:?}", report.blockers);
    assert_eq!(report.required_journey_count, 11);
    assert_eq!(report.local_journey_count, 9);
    assert_eq!(report.external_live_journey_count, 2);
    assert_eq!(report.passed_journey_count, 11);
    assert_eq!(report.blockers, Vec::<String>::new());
}

#[test]
fn main_chat_step6_product_acceptance_accepts_final_gate_kebab_live_scenario_labels() {
    let reports = vec![
        live_scenario_report_for_step6_tests("web-agent-loop", true),
        live_scenario_report_for_step6_tests("registered-mcp-agent-loop", true),
        live_scenario_report_for_step6_tests("web_agent_loop", false),
    ];

    assert!(step6_live_provider_scenario_credit(
        &reports,
        &["web-agent-loop", "web_agent_loop"]
    ));
    assert!(step6_live_provider_scenario_credit(
        &reports,
        &["registered-mcp-agent-loop", "registered_mcp_agent_loop"]
    ));
    assert!(!step6_live_provider_scenario_credit(
        &reports,
        &[
            "mcp-tool-permission-proposal",
            "mcp_tool_permission_proposal"
        ]
    ));
}

#[test]
fn main_chat_step6_product_acceptance_keeps_missing_live_evidence_blocked_not_passed() {
    let mut rows = passing_rows();
    mark_live_rows_blocked(&mut rows);
    let mut browser_report = step6_browser_report_for_tests(rows);
    browser_report.passed_journeys = required_step6_ids()
        .into_iter()
        .filter(|id| !id.starts_with("S6-LIVE-"))
        .map(str::to_string)
        .collect();
    browser_report.blocked_live_journeys = vec!["S6-LIVE-WEB".into(), "S6-LIVE-MCP".into()];
    browser_report.failed_journeys = Vec::new();
    browser_report.external_live_ready = false;
    browser_report.overall_ready = false;
    let mut final_gate = clean_step6_final_gate_summary_for_tests();
    final_gate.live_provider_ready_count = 0;
    final_gate.live_provider_web_credit = false;
    final_gate.live_provider_mcp_credit = false;

    let report = build_step6_product_acceptance_report_for_tests(Some(browser_report), final_gate);

    assert!(report.local_deterministic_ready, "{:?}", report.blockers);
    assert!(!report.external_live_ready);
    assert!(!report.overall_ready);
    assert_eq!(report.blocked_live_journey_count, 2);
    assert!(report
        .blockers
        .contains(&"step6_external_live_journeys_not_all_passed".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_browser_pass_claims_that_disagree_with_rows() {
    let mut rows = passing_rows();
    let file = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-FILE")
        .expect("file row");
    file.answer_evidence.clear();
    file.answer_observed = false;

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.local_deterministic_ready);
    assert!(!report.overall_ready);
    assert!(report.failed_journeys.contains(&"S6-FILE".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_passed_journeys_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_failed_journeys_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_local_ready_claim_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_overall_ready_claim_mismatch".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_wrong_journey_evidence_labels() {
    let mut rows = passing_rows();
    let clock = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-CLOCK")
        .expect("clock row");
    clock.answer_evidence = vec!["answer.route_summary".into()];
    clock.runtime_evidence = vec![
        "source.runtime_fact".into(),
        "runtime.provider_route".into(),
    ];
    clock.ui_status_evidence = vec!["proposal_pending".into()];
    clock.visible_ui_states = vec!["proposal_pending".into()];
    clock.final_delivery_sections = vec!["proposals_created".into()];

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_answer_evidence_missing:S6-CLOCK:answer.clock_value".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_runtime_evidence_missing:S6-CLOCK:runtime.clock".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_ui_status_missing:S6-CLOCK".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_final_delivery_section_missing:S6-CLOCK".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_wrong_journey_kind_or_observation_path() {
    let mut rows = passing_rows();
    let clock = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-CLOCK")
        .expect("clock row");
    clock.kind = "external_live".into();
    clock.observed_via = "screenshot_only_claim".into();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_kind_mismatch:S6-CLOCK".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_local_not_real_tauri_observed:S6-CLOCK".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_local_row_with_live_metadata() {
    let mut rows = passing_rows();
    let clock = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-CLOCK")
        .expect("clock row");
    clock.external_live_status = "credited_external_live".into();
    clock.external_live_provider_kind = Some("external_provider".into());
    clock.local_fixture_credited_as_external_live = true;

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(!report.no_local_fixture_marked_external_live);
    assert!(report
        .blockers
        .contains(&"step6_local_fixture_credited_as_live:S6-CLOCK".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_local_journey_has_live_status:S6-CLOCK".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_local_journey_has_provider_kind:S6-CLOCK".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_unsafe_ui_status_or_trace_labels() {
    let mut rows = passing_rows();
    let clock = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-CLOCK")
        .expect("clock row");
    clock.ui_status_evidence = vec!["completed\nunsafe".into()];
    clock.trace_evidence = vec!["trace\tunsafe".into()];

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_ui_status_unsafe:S6-CLOCK".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_trace_evidence_unsafe:S6-CLOCK".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_visible_ui_state_without_ui_status_evidence() {
    let mut rows = passing_rows();
    let clock = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-CLOCK")
        .expect("clock row");
    clock.ui_status_evidence.clear();
    clock.visible_ui_states = vec!["completed".into()];
    clock.ui_state_observed = true;

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_ui_state_missing:S6-CLOCK".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_ui_status_missing:S6-CLOCK".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_ui_status_not_structured".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_trace_only_final_delivery_claim() {
    let mut rows = passing_rows();
    let clock = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-CLOCK")
        .expect("clock row");
    clock.final_delivery_sections.clear();
    clock.final_delivery_observed = true;
    clock.trace_evidence = vec!["structured_trace".into(), "final_delivery_claim".into()];

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_final_delivery_missing:S6-CLOCK".to_string()));
    let clock_report = report
        .journeys
        .iter()
        .find(|row| row.journey_id == "S6-CLOCK")
        .expect("clock report");
    assert_eq!(clock_report.final_delivery_section_count, 0);
}

#[test]
fn main_chat_step6_product_acceptance_rejects_wrong_entry_point_or_legacy_route() {
    let mut rows = passing_rows();
    let clock = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-CLOCK")
        .expect("clock row");
    clock.entry_point = "legacy_strategy_adapter".into();
    clock.route_strategy = "legacy_fallback".into();
    let recovery = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-RECOVERY")
        .expect("recovery row");
    recovery.entry_point = "ordinary_main_chat_input".into();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_entry_point_mismatch:S6-CLOCK".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_route_legacy_or_fallback:S6-CLOCK".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_entry_point_mismatch:S6-RECOVERY".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_browser_summary_claim_mismatch() {
    let mut rows = passing_rows();
    rows[0].silent_durable_write_detected = true;
    rows[1].legacy_fallback_used = true;
    rows[2].unavailable_evidence_invented = true;
    rows[2].no_invented_unavailable_evidence = false;
    rows[3].ui_status_evidence.clear();
    let web = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-LIVE-WEB")
        .expect("web row");
    web.local_fixture_credited_as_external_live = true;
    web.live_evidence_kind = "local_fixture".into();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_browser_no_silent_write_claim_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_no_legacy_fallback_claim_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_no_invented_unavailable_claim_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_ui_status_structured_claim_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_no_local_fixture_live_claim_mismatch".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_external_credit_with_inconsistent_status() {
    let mut rows = passing_rows();
    let web = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-LIVE-WEB")
        .expect("web row");
    web.external_live_credit = true;
    web.external_live_status = "not_applicable".into();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_live_evidence_missing:S6-LIVE-WEB".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_reused_runtime_identity_across_journeys() {
    let mut rows = passing_rows();
    let duplicate_task_session_id = rows[0].task_session_id.clone();
    let duplicate_run_id = rows[0].run_id.clone();
    rows[1].task_session_id = duplicate_task_session_id;
    rows[1].run_id = duplicate_run_id;

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_observed_task_session_ids_not_distinct".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_observed_run_ids_not_distinct".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_synthetic_local_runtime_ids() {
    let mut rows = passing_rows();
    let clock = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-CLOCK")
        .expect("clock row");
    clock.task_session_id = "step6_task_placeholder".into();
    clock.run_id = "stage1_run_placeholder".into();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_task_session_missing:S6-CLOCK".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_run_missing:S6-CLOCK".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_synthetic_live_runtime_ids() {
    let mut rows = passing_rows();
    let web = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-LIVE-WEB")
        .expect("web row");
    web.task_session_id = "stage1_task_placeholder".into();
    web.run_id = "step6_run_placeholder".into();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_task_session_missing:S6-LIVE-WEB".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_run_missing:S6-LIVE-WEB".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_observed_journey_order_mismatch() {
    let mut rows = passing_rows();
    rows.swap(0, 1);

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_observed_journey_order_mismatch".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_observed_journey_count_mismatch() {
    let mut rows = passing_rows();
    rows.retain(|row| row.journey_id != "S6-TOOLS");

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_observed_journey_count_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_observed_journeys_incomplete".to_string()));
    assert!(report.failed_journeys.contains(&"S6-TOOLS".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_browser_ready_claims_for_blocked_live_rows() {
    let mut rows = passing_rows();
    mark_live_rows_blocked(&mut rows);
    let mut final_gate = clean_step6_final_gate_summary_for_tests();
    final_gate.live_provider_ready_count = 0;
    final_gate.live_provider_web_credit = false;
    final_gate.live_provider_mcp_credit = false;

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        final_gate,
    );

    assert!(!report.external_live_ready);
    assert!(!report.overall_ready);
    assert_eq!(report.blocked_live_journey_count, 2);
    assert!(report
        .blockers
        .contains(&"step6_browser_passed_journeys_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_blocked_live_journeys_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_external_ready_claim_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_overall_ready_claim_mismatch".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_blocked_live_without_ui_status() {
    let mut rows = passing_rows();
    mark_live_rows_blocked(&mut rows);
    let web = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-LIVE-WEB")
        .expect("web row");
    web.ui_status_evidence.clear();

    let mut browser_report = step6_browser_report_for_tests(rows);
    browser_report.passed_journeys = required_step6_ids()
        .into_iter()
        .filter(|id| !id.starts_with("S6-LIVE-"))
        .map(str::to_string)
        .collect();
    browser_report.blocked_live_journeys = vec!["S6-LIVE-WEB".into(), "S6-LIVE-MCP".into()];
    browser_report.failed_journeys = Vec::new();
    browser_report.external_live_ready = false;
    browser_report.overall_ready = false;
    let mut final_gate = clean_step6_final_gate_summary_for_tests();
    final_gate.live_provider_ready_count = 0;
    final_gate.live_provider_web_credit = false;
    final_gate.live_provider_mcp_credit = false;

    let report = build_step6_product_acceptance_report_for_tests(Some(browser_report), final_gate);

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_blocked_live_ui_status_missing:S6-LIVE-WEB".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_missing_browser_schema_version() {
    let mut browser_report = step6_browser_report_for_tests(passing_rows());
    browser_report.schema_version.clear();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(browser_report),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_browser_schema_invalid".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_missing_readiness_semantics() {
    let mut browser_report = step6_browser_report_for_tests(passing_rows());
    browser_report.readiness_semantics.clear();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(browser_report),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_browser_readiness_semantics_invalid".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_missing_smoke_proof() {
    let mut browser_report = step6_browser_report_for_tests(passing_rows());
    browser_report.smoke_passed = false;

    let report = build_step6_product_acceptance_report_for_tests(
        Some(browser_report),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_browser_smoke_not_passed".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_stale_browser_report_digest() {
    let mut browser_report = step6_browser_report_for_tests(passing_rows());
    browser_report.passed_journeys.pop();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(browser_report),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_browser_report_digest_mismatch".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_digest_covers_report_level_claims() {
    let mut browser_report = step6_browser_report_for_tests(passing_rows());
    browser_report.external_live_blockers = vec!["S6-LIVE-WEB:tampered".into()];

    let report = build_step6_product_acceptance_report_for_tests(
        Some(browser_report),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_browser_report_digest_mismatch".to_string()));
    assert!(report
        .blockers
        .contains(&"S6-LIVE-WEB:tampered".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_old_browser_report_even_with_matching_digest() {
    let mut browser_report = step6_browser_report_for_tests(passing_rows());
    browser_report.generated_at = (chrono::Utc::now() - chrono::Duration::hours(25)).to_rfc3339();
    refresh_step6_browser_report_digest_for_tests(&mut browser_report);

    let report = build_step6_product_acceptance_report_for_tests(
        Some(browser_report),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_browser_report_stale_or_untraceable".to_string()));
    assert!(!report
        .blockers
        .contains(&"step6_browser_report_digest_mismatch".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_unapproved_evidence_source() {
    let mut browser_report = step6_browser_report_for_tests(passing_rows());
    browser_report.evidence_source = "screenshot_only_claim".into();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(browser_report),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_browser_evidence_source_invalid".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_ready_source_not_observed".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_blocked_report_with_observed_source() {
    let mut browser_report = step6_browser_report_for_tests(passing_rows());
    browser_report.browser_e2e_environment_ready = false;
    browser_report.self_contained_runner = false;
    browser_report.evidence_source = "tauri_command_surface_step6_browser_observed".into();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(browser_report),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_browser_environment_not_ready".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_browser_blocked_source_not_unavailable".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_local_fixture_as_external_live_credit() {
    let mut rows = passing_rows();
    let web = rows
        .iter_mut()
        .find(|row| row.journey_id == "S6-LIVE-WEB")
        .expect("web row");
    web.live_evidence_kind = "local_fixture".into();
    web.external_live_credit = true;
    web.external_live_status = "credited_external_live".into();

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.external_live_ready);
    assert!(!report.no_local_fixture_marked_external_live);
    assert!(report
        .blockers
        .contains(&"step6_fake_external_live_credit:S6-LIVE-WEB".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_local_fixture_marked_external_live".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_hidden_fallback_and_silent_writes() {
    let mut rows = passing_rows();
    rows[0].legacy_fallback_used = true;
    rows[1].silent_durable_write_detected = true;
    let mut final_gate = clean_step6_final_gate_summary_for_tests();
    final_gate.command_surface_legacy_fallback_count = 1;
    final_gate.command_surface_silent_write_count = 1;

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(rows)),
        final_gate,
    );

    assert!(!report.no_hidden_legacy_fallback);
    assert!(!report.no_silent_durable_write);
    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_hidden_legacy_fallback_detected".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_silent_durable_write_detected".to_string()));
}

#[test]
fn main_chat_step6_product_acceptance_rejects_incomplete_nested_final_gate() {
    let mut final_gate = clean_step6_final_gate_summary_for_tests();
    final_gate.final_acceptance_ready = false;
    final_gate.final_acceptance_blockers =
        vec!["provider_backed_web_agent_loop_not_executed".into()];

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(passing_rows())),
        final_gate,
    );

    assert!(report.local_deterministic_ready, "{:?}", report.blockers);
    assert!(report.external_live_ready, "{:?}", report.blockers);
    assert!(!report.overall_ready);
    assert!(report
        .blockers
        .contains(&"step6_final_acceptance_not_ready".to_string()));
    assert_eq!(
        report.final_gate_summary.final_acceptance_blockers,
        vec!["provider_backed_web_agent_loop_not_executed".to_string()]
    );
}

#[test]
fn main_chat_step6_product_acceptance_fails_closed_without_browser_report() {
    let report = build_step6_product_acceptance_report_for_tests(
        None,
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert_eq!(report.passed_journey_count, 0);
    assert!(report
        .blockers
        .contains(&"step6_browser_report_missing".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_local_journeys_not_all_passed".to_string()));
}

#[tokio::test]
async fn main_chat_step6_live_provider_eval_state_prep_uses_dedicated_key_without_leaking_it() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let report = prepare_main_chat_step6_live_provider_eval_state_with_env(
        &state,
        Step6LiveProviderEvalEnv {
            explicit_live_eval_requested: true,
            provider: "openai".into(),
            base: "https://api.openai.com/v1".into(),
            model: "gpt-4o-mini".into(),
            api_key: "sk-step6-secret-test-key".into(),
        },
    )
    .await
    .expect("prep report");

    assert!(report.configured, "{report:?}");
    assert!(report.ready, "{report:?}");
    assert!(report.api_key_present);
    assert!(report.network_enabled);
    assert_eq!(report.provider_endpoint_kind, "external_provider");
    assert!(!report.app_config_persisted);
    assert!(!report.direct_writes_executed);
    let serialized = serde_json::to_string(&report).expect("serialize report");
    assert!(!serialized.contains("sk-step6-secret-test-key"));

    let scheduler = state.scheduler.lock().await.clone();
    assert_eq!(scheduler.openai_key, "sk-step6-secret-test-key");
    assert!(scheduler.scripted_generation_response.is_none());
    assert!(!scheduler.prefer_local);
    let permissions = state
        .tool_permission_store
        .lock()
        .await
        .list()
        .expect("tool permissions");
    assert!(permissions.iter().any(|permission| {
        permission.tool_name == "builtin_echo"
            && permission.source == "builtin"
            && permission.risk_level == "low"
            && permission.action_type == "read"
            && permission.consumed_at.is_none()
    }));
}

#[tokio::test]
async fn main_chat_step6_live_provider_eval_state_prep_disables_network_without_dedicated_key() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let report = prepare_main_chat_step6_live_provider_eval_state_with_env(
        &state,
        Step6LiveProviderEvalEnv {
            explicit_live_eval_requested: true,
            provider: "openai".into(),
            base: "https://api.openai.com/v1".into(),
            model: "gpt-4o-mini".into(),
            api_key: String::new(),
        },
    )
    .await
    .expect("prep report");

    assert!(report.configured, "{report:?}");
    assert!(!report.ready);
    assert!(!report.api_key_present);
    assert!(!report.network_enabled);
    assert!(report
        .blockers
        .contains(&"openlife_live_eval_api_key_missing".to_string()));
    assert!(report
        .preflight_blockers
        .contains(&"provider_api_key_missing".to_string()));
    assert!(report
        .preflight_blockers
        .contains(&"network_disabled".to_string()));

    let config = state.config.lock().await.clone();
    assert!(!config.system.network_policy.enabled);
    let scheduler = state.scheduler.lock().await.clone();
    assert!(scheduler.scripted_generation_response.is_none());
}
