use crate::main_chat_runtime_facts::{
    classify_runtime_clock_query, run_main_chat_runtime_facts_slice_a_backend_report,
    MainChatRuntimeClockIntent, RUNTIME_FACT_KEY_DATE, RUNTIME_FACT_KEY_TIME,
    RUNTIME_FACT_KEY_TIMEZONE, RUNTIME_FACT_KEY_TRACE_GAP, RUNTIME_FACT_KEY_WEEKDAY,
    RUNTIME_FACT_PROVIDER_GENERATION_PATH, RUNTIME_FACT_SOURCE_TYPE,
};

#[tokio::test]
async fn main_chat_runtime_facts_runtime_clock_slice_a_backend_report_covers_rf_01_to_rf_06() {
    let report = run_main_chat_runtime_facts_slice_a_backend_report().await;

    assert_eq!(report.report_kind, "main_chat_runtime_facts_slice");
    assert_eq!(report.slice_id, "slice_a_backend");
    assert!(report.runtime_facts_slice_ready, "{report:#?}");
    assert!(
        !report.runtime_facts_ready,
        "Slice A backend must not claim full Runtime Facts readiness"
    );
    assert!(!report.ui_included);
    assert!(report
        .out_of_scope_scenario_ids
        .iter()
        .any(|id| id == "RF-22"));
    assert_eq!(report.scenario_count, 6);
    assert_eq!(report.passed_scenario_count, 6);
    assert!(report.blockers.is_empty(), "{:?}", report.blockers);
    assert!(report.command_surface_proof.send_runtime_clock_path);
    assert!(report.command_surface_proof.stream_runtime_clock_path);
    assert!(report.no_silent_write_proof);

    for scenario_id in ["RF-01", "RF-02", "RF-03", "RF-04", "RF-05", "RF-06"] {
        let row = report
            .scenario_evidence
            .iter()
            .find(|row| row.scenario_id == scenario_id)
            .unwrap_or_else(|| panic!("missing scenario evidence {scenario_id}"));
        assert!(row.passed, "{row:#?}");
        assert_eq!(row.source_type.as_deref(), Some(RUNTIME_FACT_SOURCE_TYPE));
        assert_eq!(
            row.provider_generation_path.as_deref(),
            Some(RUNTIME_FACT_PROVIDER_GENERATION_PATH)
        );
        assert_eq!(row.model_generated, Some(false));
        assert_eq!(row.scheduler_generation_called, Some(false));
        assert_eq!(row.tool_called, Some(false));
        assert_eq!(row.direct_writes_executed, Some(false));
        assert!(!row.legacy_fallback_used);
        assert!(row
            .runtime_fact_source
            .iter()
            .any(|source| source == "local_clock"));
        assert!(row.runtime_fact_binding_count > 0);
        assert_eq!(row.runtime_fact_authority.as_deref(), Some("runtime"));
    }

    let weekday = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-01")
        .expect("RF-01 evidence");
    assert!(weekday
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_DATE));
    assert!(weekday
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_WEEKDAY));
    assert!(weekday
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_TIMEZONE));

    let time = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-03")
        .expect("RF-03 evidence");
    assert!(time
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_TIME));
    assert!(time.answer_preview.contains("09:15"));

    let context_conflict = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-05")
        .expect("RF-05 evidence");
    assert!(context_conflict.context_conflict_ignored);
    assert!(context_conflict.answer_preview.contains("2026-06-23"));
    assert!(!context_conflict.answer_preview.contains("1999-01-01"));

    let unavailable = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-06")
        .expect("RF-06 evidence");
    assert!(unavailable.trace_gap);
    assert_eq!(
        unavailable.runtime_fact_freshness.as_deref(),
        Some("unknown")
    );
    assert!(unavailable
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_TRACE_GAP));
    assert!(unavailable.answer_preview.contains("当前时间未知"));

    assert!(
        report
            .negative_assertion_summary
            .planning_question_not_captured
    );
    assert!(
        report
            .negative_assertion_summary
            .no_provider_call_for_runtime_facts
    );
    assert!(
        report
            .negative_assertion_summary
            .no_tool_call_for_runtime_facts
    );
    assert!(
        report
            .negative_assertion_summary
            .no_direct_write_for_runtime_facts
    );
    assert!(
        report
            .negative_assertion_summary
            .no_legacy_fallback_for_runtime_facts
    );
    assert!(
        report
            .negative_assertion_summary
            .context_cannot_override_runtime_clock
    );
    assert!(
        report
            .negative_assertion_summary
            .missing_clock_does_not_use_model
    );
}

#[test]
fn main_chat_runtime_clock_classifier_is_bounded_and_keeps_planning_question_out() {
    assert_eq!(
        classify_runtime_clock_query("今天星期几"),
        Some(MainChatRuntimeClockIntent::AskCurrentWeekday)
    );
    assert_eq!(
        classify_runtime_clock_query("今天几号"),
        Some(MainChatRuntimeClockIntent::AskCurrentDate)
    );
    assert_eq!(
        classify_runtime_clock_query("现在几点"),
        Some(MainChatRuntimeClockIntent::AskCurrentTime)
    );
    assert_eq!(
        classify_runtime_clock_query("what time is it"),
        Some(MainChatRuntimeClockIntent::AskCurrentTime)
    );
    assert_eq!(
        classify_runtime_clock_query("What time should I leave tomorrow?"),
        None
    );
    assert_eq!(classify_runtime_clock_query("我今天完成了写周报"), None);
}
