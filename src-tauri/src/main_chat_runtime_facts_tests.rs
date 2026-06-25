use crate::main_chat_runtime_facts::{
    classify_provider_route_query, classify_runtime_clock_query,
    run_main_chat_runtime_facts_slice_a_backend_report,
    run_main_chat_runtime_facts_slice_b_provider_route_report, MainChatProviderRouteIntent,
    MainChatRuntimeClockIntent, RUNTIME_FACT_KEY_DATE,
    RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER,
    RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED, RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER,
    RUNTIME_FACT_KEY_TIME, RUNTIME_FACT_KEY_TIMEZONE, RUNTIME_FACT_KEY_TRACE_GAP,
    RUNTIME_FACT_KEY_WEEKDAY, RUNTIME_FACT_PROVIDER_GENERATION_PATH, RUNTIME_FACT_SOURCE_TYPE,
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
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .no_provider_call_for_runtime_facts
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .no_tool_call_for_runtime_facts
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .no_direct_write_for_runtime_facts
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .no_legacy_fallback_for_runtime_facts
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .context_cannot_override_runtime_clock
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .missing_clock_does_not_use_model
            == Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .current_route_requires_current_generation_evidence,
        None
    );
}

#[tokio::test]
async fn main_chat_runtime_facts_provider_route_slice_b_covers_rf_07_to_rf_10() {
    let report = run_main_chat_runtime_facts_slice_b_provider_route_report().await;

    assert_eq!(report.report_kind, "main_chat_runtime_facts_slice");
    assert_eq!(report.slice_id, "slice_b_provider_route_semantics");
    assert!(report.runtime_facts_slice_ready, "{report:#?}");
    assert!(
        !report.runtime_facts_ready,
        "Slice B must not claim full Runtime Facts readiness"
    );
    assert!(report.ui_included);
    assert_eq!(report.scenario_count, 4);
    assert_eq!(report.passed_scenario_count, 4);
    assert!(report.blockers.is_empty(), "{:?}", report.blockers);
    assert!(report.command_surface_proof.send_provider_route_path);
    assert!(
        report
            .command_surface_proof
            .send_provider_route_preflight_blocker_path
    );
    assert!(report.no_silent_write_proof);

    for scenario_id in ["RF-07", "RF-08", "RF-09", "RF-10"] {
        let row = report
            .scenario_evidence
            .iter()
            .find(|row| row.scenario_id == scenario_id)
            .unwrap_or_else(|| panic!("missing scenario evidence {scenario_id}"));
        assert!(row.passed, "{row:#?}");
        assert_eq!(row.source_type.as_deref(), Some(RUNTIME_FACT_SOURCE_TYPE));
        assert_eq!(row.tool_called, Some(false));
        assert_eq!(row.direct_writes_executed, Some(false));
        assert!(!row.legacy_fallback_used);
        assert!(row
            .runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED));
        assert!(row
            .runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER));
        assert!(row
            .runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER));
        assert!(row
            .runtime_fact_source
            .iter()
            .any(|source| source == "provider_route"));
        assert!(row
            .route_labels
            .iter()
            .any(|label| label.starts_with("current_turn_generation:")));
        assert!(row
            .route_labels
            .iter()
            .any(|label| label.starts_with("configured_default_route:")));
        assert!(row
            .route_labels
            .iter()
            .any(|label| label.starts_with("planned_route_if_model_needed:")));
        assert_eq!(row.ui_primary_source_chip.as_deref(), Some("运行时路线"));
    }

    let current = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-07")
        .expect("RF-07 evidence");
    assert_eq!(current.model_generated, Some(true));
    assert_eq!(current.scheduler_generation_called, Some(true));
    assert_eq!(
        current.current_turn_generation_provider.as_deref(),
        Some("openai")
    );
    assert_eq!(
        current.current_turn_generation_model.as_deref(),
        Some("gpt-slice-b-current")
    );
    assert_eq!(
        current.current_turn_generation_route_type.as_deref(),
        Some("cloud")
    );
    assert_eq!(
        current.configured_model.as_deref(),
        Some("gpt-configured-default")
    );

    let after_clock = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-08")
        .expect("RF-08 evidence");
    assert_eq!(after_clock.model_generated, Some(false));
    assert_eq!(after_clock.scheduler_generation_called, Some(false));
    assert!(after_clock.current_turn_generation_provider.is_none());
    assert!(after_clock.current_turn_generation_model.is_none());
    assert_eq!(
        after_clock.current_turn_generation_route_type.as_deref(),
        Some("none")
    );
    assert!(after_clock.answer_preview.contains("没有调用模型"));

    let separated = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-09")
        .expect("RF-09 evidence");
    assert_eq!(separated.configured_provider.as_deref(), Some("deepseek"));
    assert_eq!(
        separated.current_turn_generation_provider.as_deref(),
        Some("openai")
    );
    assert_eq!(
        separated.last_completed_generation_provider.as_deref(),
        Some("anthropic")
    );
    assert_eq!(
        separated.planned_route_if_model_needed_provider.as_deref(),
        Some("openai")
    );

    let blocked = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-10")
        .expect("RF-10 evidence");
    assert_eq!(blocked.model_generated, Some(false));
    assert_eq!(
        blocked.provider_preflight_status.as_deref(),
        Some("blocked")
    );
    assert!(!blocked.provider_preflight_blockers.is_empty());
    assert_eq!(blocked.ui_status.as_deref(), Some("restricted"));
    assert!(blocked.current_turn_generation_provider.is_none());
    assert!(!blocked
        .answer_preview
        .contains("provider.preflight.status=ready"));

    assert!(
        report
            .negative_assertion_summary
            .current_route_requires_current_generation_evidence
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .no_current_route_for_model_generated_false
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .configured_route_not_invocation_proof
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .planned_route_not_invocation_proof
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .last_completed_route_not_current_turn
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .provider_preflight_blocker_not_fake_readiness
            == Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .context_cannot_override_runtime_clock,
        None
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

#[test]
fn main_chat_provider_route_classifier_is_bounded_and_separates_previous_turn() {
    assert_eq!(
        classify_provider_route_query("你现在用什么模型"),
        Some(MainChatProviderRouteIntent::AskCurrentModelRoute)
    );
    assert_eq!(
        classify_provider_route_query("what model are you using now"),
        Some(MainChatProviderRouteIntent::AskCurrentModelRoute)
    );
    assert_eq!(
        classify_provider_route_query("刚才回答今天星期几时用了什么模型"),
        Some(MainChatProviderRouteIntent::AskPreviousTurnModelRoute)
    );
    assert_eq!(classify_provider_route_query("我想比较几个模型"), None);
}
