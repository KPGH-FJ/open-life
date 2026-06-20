#[test]
fn main_chat_stage3_execution_ux_report_covers_all_ux3_rows_without_readiness_claim() {
    let report = crate::main_chat_stage3_execution_ux::run_main_chat_stage3_execution_ux_report();

    assert_eq!(report.report_kind, "main_chat_stage3_execution_ux");
    assert_eq!(report.schema_version, "stage3-execution-ux-v1");
    assert_eq!(
        report.readiness_recommendation,
        "not_ready_for_limited_internal_trial"
    );
    assert!(!report.ready_for_limited_internal_trial);
    assert_eq!(report.total_scenario_count, 13);
    assert_eq!(report.coverage.len(), 13);

    for expected_id in [
        "UX3-01", "UX3-02", "UX3-03", "UX3-04", "UX3-05", "UX3-06", "UX3-07", "UX3-08", "UX3-09",
        "UX3-10", "UX3-11", "UX3-12", "UX3-13",
    ] {
        let row = report
            .coverage
            .iter()
            .find(|row| row.scenario_id == expected_id)
            .unwrap_or_else(|| panic!("missing Stage 3 scenario {expected_id}"));
        assert_eq!(
            row.status, "passed",
            "{expected_id} blockers: {:?}",
            row.blockers
        );
        assert!(
            !row.evidence.is_empty(),
            "{expected_id} must cite deterministic runtime/UI evidence"
        );
    }

    for execution_first_id in [
        "UX3-02", "UX3-03", "UX3-04", "UX3-06", "UX3-09", "UX3-11", "UX3-12",
    ] {
        assert!(
            report
                .execution_first_passed_ids
                .contains(&execution_first_id.to_string()),
            "{execution_first_id} must count toward the execution-first claim"
        );
    }

    assert!(report.execution_first_claim_valid);
    assert_eq!(
        report.stage2_readiness_preserved,
        "stage2_readiness_remains_fail_closed_without_manual_dogfood_and_current_commit_live_evidence"
    );
    assert!(report
        .non_goals
        .contains(&"manual_dogfood_rows_not_run_or_fabricated".to_string()));
}
