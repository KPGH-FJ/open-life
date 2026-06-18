use crate::main_chat_agent_stage1_dogfood::{
    passing_stage1_browser_e2e_evidence_for_tests,
    run_main_chat_agent_stage1_dogfood_report_with_browser_evidence,
    run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests,
    run_stage1_runtime_evidence_bundle_for_tests, MainChatStage1BrowserE2eEvidence,
};

fn is_digest_label(value: &str) -> bool {
    let Some((bytes, hash)) = value.split_once(" hash:sha256:") else {
        return false;
    };
    bytes
        .strip_prefix("bytes:")
        .and_then(|count| count.parse::<usize>().ok())
        .is_some_and(|count| count > 0)
        && hash.len() == 64
        && hash.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn browser_evidence_with_source(source: &str) -> MainChatStage1BrowserE2eEvidence {
    let mut evidence = passing_stage1_browser_e2e_evidence_for_tests();
    evidence.evidence_source = source.into();
    evidence
}

#[tokio::test]
async fn main_chat_agent_stage1_dogfood_gate_builds_full_default_matrix_and_seed_manifest() {
    let report = run_main_chat_agent_stage1_dogfood_report_with_browser_evidence(Some(
        passing_stage1_browser_e2e_evidence_for_tests(),
    ))
    .await
    .expect("stage 1 dogfood report");

    assert_eq!(report.report_kind, "main_chat_agent_stage1_dogfood_gate");
    assert!(report.default_ready, "blockers: {:?}", report.blockers);
    assert_eq!(
        report.readiness_recommendation,
        "ready_for_engineering_dogfood"
    );
    assert_eq!(report.default_scenario_count, 36);
    assert_eq!(report.scenario_count, 40);
    assert_eq!(report.default_passed_count, 36);
    assert_eq!(report.default_failed_count, 0);
    assert_eq!(report.ordinary_chat_scenario_count, 24);
    assert_eq!(report.seeded_task_control_scenario_count, 12);
    assert_eq!(report.task_session_created_count, 36);
    assert_eq!(report.ui_verified_scenario_count, 36);
    assert_eq!(report.final_delivery_verified_scenario_count, 36);
    assert_eq!(report.legacy_fallback_count, 0);
    assert_eq!(report.silent_durable_write_count, 0);
    assert_eq!(report.fake_execution_detected_count, 0);
    assert!(report.browser_e2e_environment_ready);
    assert_eq!(
        report.browser_e2e_report_path.as_deref(),
        Some("frontend/test-results/main-chat-stage1-dogfood-report.json")
    );
    assert!(!report.external_live_attempted);
    assert!(!report.opt_in_live_ready);
    assert!(report.default_readiness_unaffected_by_live);

    assert_eq!(
        report.seed_manifest.seed_workspace_root_kind,
        "temp_isolated"
    );
    assert_eq!(report.seed_manifest.knowledge_asset_count, 9);
    assert_eq!(report.seed_manifest.skill_count, 3);
    assert_eq!(report.seed_manifest.session_seed_count, 1);
    assert!(report.seed_manifest.memory_seed_count >= 5);
    assert_eq!(report.seed_manifest.proposal_seed_count, 2);
    assert_eq!(report.seed_manifest.task_seed_count, 5);
    assert_eq!(report.seed_manifest.plan_seed_count, 1);
    assert_eq!(report.seed_manifest.mcp_manifest_seed_count, 2);
    assert_eq!(report.seed_manifest.web_fixture_seed_count, 1);
    assert!(!report.seed_manifest.secrets_detected);
    assert!(is_digest_label(&report.seed_manifest.seed_digest));
    for digest in report.seed_manifest.file_digests.values() {
        assert!(is_digest_label(digest), "bad file digest: {digest}");
    }
    for path in [
        "AGENTS.md",
        "SOUL.md",
        "USER.md",
        "MEMORY.md",
        "project_brief.md",
        "planning_notes.md",
        "policy_note.md",
        "memories/USER.md",
        "memories/MEMORY.md",
        "skills/phase_e_review/SKILL.md",
        "skills/planning_review/SKILL.md",
        "skills/unselected_sensitive/SKILL.md",
    ] {
        assert!(
            report.seed_manifest.file_digests.contains_key(path),
            "missing seed file digest for {path}"
        );
    }
}

#[tokio::test]
async fn main_chat_agent_stage1_dogfood_static_rows_alone_cannot_pass_readiness() {
    let report = run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(
        Some(passing_stage1_browser_e2e_evidence_for_tests()),
        None,
        false,
    )
    .await
    .expect("stage 1 dogfood report");

    assert!(!report.default_ready);
    assert_eq!(report.readiness_recommendation, "not_ready");
    assert_eq!(report.default_passed_count, 0);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage1_default_scenarios_not_executed"));
    assert!(report
        .scenarios
        .iter()
        .filter(|row| row.live_provider_evidence.as_deref() == Some("default_deterministic"))
        .all(|row| {
            !row.runtime_evidence_passed
                && !row.non_fake_evidence_passed
                && !row.passed
                && row.failure_reason.as_deref() == Some("stage1_runtime_execution_missing")
        }));
}

#[tokio::test]
async fn main_chat_agent_stage1_dogfood_gate_fails_closed_without_browser_smoke() {
    let report = run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(None, None, false)
        .await
        .expect("stage 1 dogfood report");

    assert!(!report.default_ready);
    assert_eq!(report.readiness_recommendation, "not_ready");
    assert!(!report.browser_e2e_environment_ready);
    assert!(report.browser_e2e_report_path.is_none());
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "not_ready_browser_e2e_blocked"));
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "required_browser_e2e_smoke_not_run"));
}

#[tokio::test]
async fn main_chat_agent_stage1_dogfood_gate_rejects_stale_or_malformed_browser_report() {
    let mut stale = passing_stage1_browser_e2e_evidence_for_tests();
    stale.generated_at = Some("2000-01-01T00:00:00Z".into());
    let stale_report =
        run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(Some(stale), None, false)
            .await
            .expect("stage 1 dogfood report");
    assert!(!stale_report.default_ready);
    assert!(stale_report
        .blockers
        .iter()
        .any(|blocker| blocker == "browser_e2e_report_stale_or_untraceable"));

    let mut malformed = passing_stage1_browser_e2e_evidence_for_tests();
    malformed.run_id = Some(" bad-run ".into());
    let malformed_report = run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(
        Some(malformed),
        None,
        false,
    )
    .await
    .expect("stage 1 dogfood report");
    assert!(!malformed_report.default_ready);
    assert!(malformed_report
        .blockers
        .iter()
        .any(|blocker| blocker == "browser_e2e_report_stale_or_untraceable"));
}

#[tokio::test]
async fn main_chat_agent_stage1_dogfood_gate_rejects_frontend_only_fixture_browser_report() {
    let report = run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(
        Some(browser_evidence_with_source("frontend_only_fixture")),
        None,
        false,
    )
    .await
    .expect("stage 1 dogfood report");

    assert!(!report.default_ready);
    assert_eq!(report.fake_execution_detected_count, 1);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "browser_e2e_frontend_only_fixture_report"));
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "fake_execution_detected"));
}

#[tokio::test]
async fn main_chat_agent_stage1_dogfood_gate_rejects_incomplete_browser_journeys() {
    let mut evidence = passing_stage1_browser_e2e_evidence_for_tests();
    evidence.passed_journeys.pop();
    let report = run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(
        Some(evidence),
        None,
        false,
    )
    .await
    .expect("stage 1 dogfood report");

    assert!(!report.default_ready);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "browser_e2e_required_journeys_incomplete"));
}

#[tokio::test]
async fn main_chat_agent_stage1_dogfood_gate_rejects_journey_only_browser_report() {
    let mut evidence = passing_stage1_browser_e2e_evidence_for_tests();
    evidence.observed_scenarios.clear();
    let report = run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(
        Some(evidence),
        None,
        false,
    )
    .await
    .expect("stage 1 dogfood report");

    assert!(!report.default_ready);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "browser_e2e_observed_scenarios_missing"));
}

#[tokio::test]
async fn main_chat_agent_stage1_dogfood_default_readiness_is_unaffected_by_live_opt_in_status() {
    let no_live = run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(
        Some(passing_stage1_browser_e2e_evidence_for_tests()),
        None,
        false,
    )
    .await
    .expect("stage 1 dogfood report");
    let live_opt_in = run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(
        Some(passing_stage1_browser_e2e_evidence_for_tests()),
        None,
        true,
    )
    .await
    .expect("stage 1 dogfood report");

    assert_ne!(
        no_live.external_live_attempted,
        live_opt_in.external_live_attempted
    );
    assert_eq!(no_live.default_ready, live_opt_in.default_ready);
    assert_eq!(
        no_live.readiness_recommendation,
        live_opt_in.readiness_recommendation
    );
    assert_eq!(no_live.blockers, live_opt_in.blockers);
    assert!(no_live.default_readiness_unaffected_by_live);
    assert!(live_opt_in.default_readiness_unaffected_by_live);
}

#[tokio::test]
async fn main_chat_agent_stage1_dogfood_report_serializes_without_recursive_state() {
    let report = run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(None, None, false)
        .await
        .expect("stage 1 dogfood report");
    let value = serde_json::to_value(&report).expect("serialize stage 1 report");

    assert_eq!(
        value["reportKind"].as_str(),
        Some("main_chat_agent_stage1_dogfood_gate")
    );
    assert_eq!(value["scenarioCount"].as_u64(), Some(40));
    assert_eq!(value["defaultScenarioCount"].as_u64(), Some(36));
}

#[tokio::test]
async fn main_chat_agent_stage1_dogfood_rows_have_runtime_ui_final_delivery_evidence() {
    let runtime_evidence = run_stage1_runtime_evidence_bundle_for_tests()
        .await
        .expect("stage 1 runtime evidence");
    let report = run_main_chat_agent_stage1_dogfood_report_with_inputs_for_tests(
        Some(passing_stage1_browser_e2e_evidence_for_tests()),
        Some(runtime_evidence),
        false,
    )
    .await
    .expect("stage 1 dogfood report");

    let default_rows = report
        .scenarios
        .iter()
        .filter(|row| row.live_provider_evidence.as_deref() == Some("default_deterministic"))
        .collect::<Vec<_>>();
    assert_eq!(default_rows.len(), 36);

    for row in default_rows {
        assert!(
            !row.task_session_id.is_empty() && !row.task_session_id.starts_with("stage1_task_"),
            "missing real task/control id for {}: {}",
            row.scenario_id,
            row.task_session_id
        );
        assert!(
            !row.run_id.is_empty() && !row.run_id.starts_with("stage1_run_"),
            "missing real run/control id for {}: {}",
            row.scenario_id,
            row.run_id
        );
        assert!(
            !row.runtime_events.is_empty(),
            "no runtime events for {}",
            row.scenario_id
        );
        assert!(
            !row.ui_states.is_empty(),
            "no UI states for {}",
            row.scenario_id
        );
        assert!(
            !row.final_delivery_sections.is_empty(),
            "no final delivery sections for {}",
            row.scenario_id
        );
        assert!(
            row.runtime_evidence_passed,
            "runtime evidence failed for {}",
            row.scenario_id
        );
        assert!(
            row.ui_evidence_passed,
            "UI evidence failed for {}",
            row.scenario_id
        );
        assert!(
            row.final_delivery_evidence_passed,
            "final delivery evidence failed for {}",
            row.scenario_id
        );
        assert!(
            row.non_fake_evidence_passed,
            "non-fake evidence failed for {}",
            row.scenario_id
        );
        assert!(
            !row.legacy_fallback_used,
            "legacy fallback used for {}",
            row.scenario_id
        );
        assert!(
            !row.silent_durable_write_detected,
            "silent write detected for {}",
            row.scenario_id
        );
        assert!(
            !row.fake_execution_detected,
            "fake execution detected for {}",
            row.scenario_id
        );
        assert!(
            row.passed,
            "row did not pass: {} {:?}",
            row.scenario_id, row.failure_reason
        );
        assert!(is_digest_label(&row.user_prompt_digest));
        assert_eq!(row.seed_manifest_digest, report.seed_manifest.seed_digest);
        if row.expected_outcome == "expected_blocker" {
            assert!(
                !row.blockers.is_empty(),
                "expected blocker row has no visible blocker: {}",
                row.scenario_id
            );
            assert_ne!(row.actual_outcome, "success");
        }
    }
}
