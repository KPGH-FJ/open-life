use crate::agent::{
    run_low_energy_support_golden_path, run_preference_correction_golden_path,
    run_weekly_planning_golden_path, LowEnergySupportGoldenPathInput,
    PreferenceCorrectionGoldenPathInput, WeeklyPlanningGoldenPathInput,
};

fn assert_no_raw_content(serialized: &str) {
    for raw in [
        "RAW_USER_TEXT_SECRET",
        "RAW_MEMORY_SECRET",
        "RAW_ASSISTANT_OUTPUT_SECRET",
        "RAW_TOOL_PAYLOAD_SECRET",
        "alice@example.com",
        "private planning context",
    ] {
        assert!(
            !serialized.contains(raw),
            "Goal 7 golden path report leaked raw marker {raw}: {serialized}"
        );
    }
}

#[test]
fn w144_weekly_planning_golden_path_links_guidance_plan_proposals_outcome_and_future_guidance() {
    let report = run_weekly_planning_golden_path(WeeklyPlanningGoldenPathInput {
        source_run_id: "run-w144-weekly".into(),
        source_chat_session_id: Some("chat-w144-weekly".into()),
        raw_user_text: "RAW_USER_TEXT_SECRET plan my week around alice@example.com".into(),
        raw_memory_context: Some("RAW_MEMORY_SECRET private planning context".into()),
    })
    .unwrap();

    assert!(report.golden_path_ready);
    assert_eq!(report.report_kind, "w144.weeklyPlanningGoldenPath.v1");
    assert!(report.default_chat_unchanged);
    assert!(report.guidance_selected);
    assert!(report.explicit_guidance_changed_plan);
    assert!(!report.disabled_guidance_changed_plan);
    assert!(report.trace_shows_selected_guidance_metadata);
    assert_eq!(report.selected_guidance_count, 1);
    assert!(report.plan_session_finalized);
    assert!(report.write_like_step_created_proposal);
    assert!(report.proposal_first_write_boundary_preserved);
    assert_eq!(report.external_write_count, 0);
    assert_eq!(report.life_model_write_count, 0);
    assert_eq!(report.memory_write_count, 0);
    assert!(report.outcome_evidence_recorded);
    assert_eq!(report.outcome_evidence_ids.len(), 1);
    assert_eq!(report.linked_plan_proposal_ids.len(), 1);
    assert_eq!(report.linked_agent_run_ids, vec!["run-w144-weekly"]);
    assert!(report.future_planning_guidance_ready);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.blocking_reasons.is_empty());
    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}

#[test]
fn w145_low_energy_support_golden_path_flows_from_signal_to_accepted_guidance_behavior_change() {
    let report = run_low_energy_support_golden_path(LowEnergySupportGoldenPathInput {
        source_run_id: "run-w145-low-energy".into(),
        raw_user_text: "RAW_USER_TEXT_SECRET I am exhausted; keep planning tiny".into(),
        raw_assistant_output: "RAW_ASSISTANT_OUTPUT_SECRET long private draft".into(),
    })
    .unwrap();

    assert!(report.golden_path_ready);
    assert_eq!(report.report_kind, "w145.lowEnergySupportGoldenPath.v1");
    assert!(report.default_chat_unchanged);
    assert!(report.life_event_recorded);
    assert!(report.signal_extracted);
    assert!(report.evidence_bridged);
    assert!(report.maturation_candidate_generated);
    assert!(report.accepted_guidance_created);
    assert!(report.guidance_selected);
    assert!(report.explicit_runtime_behavior_changed);
    assert!(!report.disabled_runtime_behavior_changed);
    assert!(report.suggestions_smaller_and_gentler);
    assert!(report.guidance_impact_metadata_visible);
    assert_eq!(report.life_model_write_count, 0);
    assert_eq!(report.memory_write_count, 0);
    assert_eq!(report.high_risk_truth_materialization_count, 0);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.blocking_reasons.is_empty());
    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}

#[test]
fn w146_preference_correction_golden_path_suppresses_rejected_and_applies_corrective_evidence() {
    let report = run_preference_correction_golden_path(PreferenceCorrectionGoldenPathInput {
        source_run_id: "run-w146-correction".into(),
        raw_wrong_inference: "RAW_ASSISTANT_OUTPUT_SECRET user wants very detailed reminders"
            .into(),
        raw_user_correction: "RAW_USER_TEXT_SECRET no, shorter reminders only".into(),
    })
    .unwrap();

    assert!(report.golden_path_ready);
    assert_eq!(report.report_kind, "w146.preferenceCorrectionGoldenPath.v1");
    assert!(report.default_chat_unchanged);
    assert!(report.rejection_evidence_recorded);
    assert!(report.corrective_evidence_recorded);
    assert!(report.evidence_conflict_visible);
    assert!(report.rejected_similar_candidate_suppressed);
    assert!(report.corrected_candidate_generated);
    assert!(report.future_behavior_changed);
    assert_eq!(report.life_model_write_count, 0);
    assert_eq!(report.memory_write_count, 0);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.blocking_reasons.is_empty());
    assert_no_raw_content(&serde_json::to_string(&report).unwrap());
}
