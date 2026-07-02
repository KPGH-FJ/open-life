use crate::main_chat_agent_stage2_readiness::{
    collect_stage2_failure_recovery_coverage_for_tests,
    collect_stage2_final_delivery_summary_for_tests,
    complete_stage2_live_provider_evidence_for_tests, complete_stage2_manual_dogfood_records,
    evaluate_stage2_manual_dogfood_artifact_for_tests,
    evaluate_stage2_manual_dogfood_records_for_tests,
    read_or_run_stage2_live_provider_summary_with_artifact_path_for_tests,
    read_stage2_live_provider_artifact_from_path_with_expected_commit_for_tests,
    read_stage2_manual_dogfood_artifact_from_path_for_tests,
    read_stage2_manual_dogfood_artifact_from_path_with_expected_commit_for_tests,
    run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests, stage2_artifacts_for_tests,
    stage2_live_provider_attempted_p0_matrix_evidence_for_tests,
    stage2_live_provider_attempted_p0_matrix_evidence_with_blockers_for_tests,
    stage2_live_provider_evidence_from_harness_reports_for_tests,
    stage2_live_provider_summary_for_tests, Stage2ManualDogfoodArtifact, Stage2ManualDogfoodRecord,
    Stage2ReadinessTestInputs,
};

fn complete_manual_records() -> Vec<Stage2ManualDogfoodRecord> {
    complete_stage2_manual_dogfood_records("reviewer-a", "reviewer-b", "abc123")
}

fn stage2_test_current_build_commit() -> Option<String> {
    std::env::var("GITHUB_SHA")
        .or_else(|_| std::env::var("OPENLIFE_BUILD_COMMIT"))
        .ok()
        .filter(|value| {
            crate::main_chat_agent_stage2_readiness::known_stage2_commit_label_for_tests(value)
        })
}

#[test]
fn main_chat_stage2_manual_dogfood_template_is_non_evidence_and_covers_required_p0_rows() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root");
    let template_path =
        repo_root.join("plans/main_chat_stage2_manual_dogfood_artifact_template.json");

    assert!(
        template_path.exists(),
        "manual dogfood artifact template should live outside frontend/test-results evidence path"
    );

    let bytes = std::fs::read(&template_path).expect("read manual dogfood artifact template");
    let artifact: Stage2ManualDogfoodArtifact =
        serde_json::from_slice(&bytes).expect("template parses as manual dogfood artifact shape");
    let scenario_ids = artifact
        .reviewer_records
        .iter()
        .map(|record| record.scenario_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let expected_ids = (1..=24)
        .map(|index| format!("S2-D{index:02}"))
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(artifact.schema_version, "stage2-manual-dogfood-v1");
    assert_eq!(artifact.reviewer_records.len(), 24);
    assert_eq!(
        scenario_ids,
        expected_ids.iter().map(String::as_str).collect()
    );
    assert!(artifact
        .reviewer_records
        .iter()
        .all(|record| record.result == "not attempted" && record.severity == "P0"));

    let summary = evaluate_stage2_manual_dogfood_artifact_for_tests(&artifact);

    assert!(
        !summary.ready,
        "template must never count as reviewer evidence"
    );
    assert_eq!(summary.required_scenario_count, 24);
    assert_eq!(summary.attempted_scenario_count, 0);
    assert!(summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_required_scenarios_missing"));
    assert!(summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_artifact_commit_missing"));
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_fails_closed_without_manual_or_live_evidence() {
    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(
        Stage2ReadinessTestInputs::mechanism_ready_without_manual_or_live(),
    )
    .await
    .expect("stage 2 readiness report");

    assert_eq!(report.schema_version, "stage2-readiness-v1");
    assert_eq!(
        report.recommendation,
        "not_ready_for_limited_internal_trial"
    );
    assert_eq!(
        report.implementation_status,
        "implementation_complete_for_stage2_mechanism"
    );
    assert!(report.deterministic_stage1_ready);
    assert!(report.beta_foundation_ready);
    assert!(
        report.control_plane.ready,
        "{:?}",
        report.control_plane.blockers
    );
    assert!(
        report.memory_proposal.ready,
        "{:?}",
        report.memory_proposal.blockers
    );
    assert!(
        report.failure_recovery.ready,
        "{:?}",
        report.failure_recovery.blockers
    );
    assert!(!report.manual_dogfood.attempted);
    assert!(!report.live_provider.attempted);
    assert_eq!(report.live_provider.scenario_reports.len(), 10);
    assert!(report
        .live_provider
        .scenario_reports
        .iter()
        .all(|row| !row.credited && row.status == "blocked"));
    assert!(report
        .live_provider
        .scenario_reports
        .iter()
        .any(|row| row.scenario_id == "L2-L10"
            && row
                .blockers
                .iter()
                .any(|blocker| blocker == "stage2_live_provider_p0_evidence_missing")));
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_dogfood_evidence_missing"));
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_provider_p0_evidence_missing"));
    assert!(!report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_provider_p0_runner_incomplete"));
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_fails_closed_when_manual_summary_has_no_blockers() {
    let mut inputs = Stage2ReadinessTestInputs::fully_ready_for_tests(
        complete_manual_records(),
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live"),
    );
    inputs.inject_attempted_manual_not_ready_without_blockers_for_tests();

    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(inputs)
        .await
        .expect("stage 2 readiness report");

    assert_eq!(
        report.recommendation,
        "not_ready_for_limited_internal_trial"
    );
    assert!(report.manual_dogfood.attempted);
    assert!(!report.manual_dogfood.ready);
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker == "stage2_manual_dogfood_evidence_incomplete"),
        "manual not-ready summaries must add an aggregate blocker even when an adapter supplied no section blockers: {:?}",
        report.blockers
    );
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_report_includes_browser_manual_and_live_artifacts() {
    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(
        Stage2ReadinessTestInputs::mechanism_ready_without_manual_or_live(),
    )
    .await
    .expect("stage 2 readiness report");

    let artifact_kinds = report
        .artifacts
        .iter()
        .map(|artifact| artifact.kind.as_str())
        .collect::<Vec<_>>();
    assert!(
        artifact_kinds.contains(&"stage1_browser_dogfood"),
        "Stage 2 readiness artifacts must include browser evidence ref: {artifact_kinds:?}"
    );
    assert!(
        artifact_kinds.contains(&"manual_dogfood"),
        "Stage 2 readiness artifacts must include manual dogfood artifact ref"
    );
    assert!(
        artifact_kinds.contains(&"live_provider"),
        "Stage 2 readiness artifacts must include live provider artifact ref"
    );
    let browser = report
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == "stage1_browser_dogfood")
        .expect("browser artifact ref");
    assert_eq!(
        browser.path,
        "frontend/test-results/main-chat-stage1-dogfood-report.json"
    );
    assert!(browser
        .digest
        .as_deref()
        .is_some_and(metadata_safe_test_digest));
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_artifact_refs_do_not_label_fake_browser_evidence_as_loaded(
) {
    let mut inputs = Stage2ReadinessTestInputs::mechanism_ready_without_manual_or_live();
    inputs.inject_fake_browser_evidence_for_tests();
    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(inputs)
        .await
        .expect("stage 2 readiness report");

    let browser = report
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == "stage1_browser_dogfood")
        .expect("browser artifact ref");

    assert_eq!(browser.status, "blocked");
    assert_eq!(report.safety.fake_browser_evidence_count, 1);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_fake_browser_evidence_detected"));
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_artifact_refs_do_not_label_stage1_blocked_browser_evidence_as_loaded(
) {
    let mut inputs = Stage2ReadinessTestInputs::mechanism_ready_without_manual_or_live();
    inputs.inject_stage1_browser_blocker_for_tests();
    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(inputs)
        .await
        .expect("stage 2 readiness report");

    let browser = report
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == "stage1_browser_dogfood")
        .expect("browser artifact ref");

    assert_eq!(browser.status, "blocked");
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage1_browser_evidence_blocked"));
}

#[test]
fn main_chat_agent_stage2_readiness_artifact_refs_do_not_label_blocked_evidence_as_loaded() {
    let mut manual = evaluate_stage2_manual_dogfood_records_for_tests(&complete_manual_records());
    manual.artifact_digest = Some(
        crate::main_chat_agent_stage2_readiness::digest_bytes_for_tests(
            b"manual artifact with schema blocker",
        ),
    );
    manual.ready = false;
    manual
        .blockers
        .push("stage2_manual_artifact_schema_invalid".into());

    let mut live = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live"),
    );
    live.artifact_digest = Some(
        crate::main_chat_agent_stage2_readiness::digest_bytes_for_tests(
            b"live artifact with schema blocker",
        ),
    );
    live.ready = false;
    live.blockers
        .push("stage2_live_artifact_schema_invalid".into());

    let artifacts = stage2_artifacts_for_tests(&manual, &live);
    let manual_artifact = artifacts
        .iter()
        .find(|artifact| artifact.kind == "manual_dogfood")
        .expect("manual artifact ref");
    let live_artifact = artifacts
        .iter()
        .find(|artifact| artifact.kind == "live_provider")
        .expect("live artifact ref");

    assert_eq!(manual_artifact.status, "blocked");
    assert_eq!(live_artifact.status, "blocked");
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_redacts_unsafe_upstream_blockers() {
    let mut inputs = Stage2ReadinessTestInputs::mechanism_ready_without_manual_or_live();
    inputs.inject_unsafe_upstream_blockers_for_tests();
    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(inputs)
        .await
        .expect("stage 2 readiness report");

    assert!(!report
        .blockers
        .iter()
        .any(|blocker| blocker.contains("raw upstream")));
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_metadata_unsafe_blocker_label"));
    assert!(report.blockers.iter().all(|blocker| blocker
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'))));
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_redacts_nested_section_blockers() {
    let mut inputs = Stage2ReadinessTestInputs::mechanism_ready_without_manual_or_live();
    inputs.inject_unsafe_upstream_blockers_for_tests();
    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(inputs)
        .await
        .expect("stage 2 readiness report");

    let nested_blockers = report
        .control_plane
        .blockers
        .iter()
        .chain(report.memory_proposal.blockers.iter())
        .chain(report.failure_recovery.blockers.iter())
        .chain(report.final_delivery.blockers.iter())
        .collect::<Vec<_>>();

    assert!(!nested_blockers
        .iter()
        .any(|blocker| blocker.contains("raw upstream")));
    assert!(nested_blockers
        .iter()
        .any(|blocker| blocker.as_str() == "stage2_metadata_unsafe_blocker_label"));
    assert!(nested_blockers
        .iter()
        .all(|blocker| metadata_safe_test_label(blocker)));
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_report_coverage_evidence_is_metadata_safe() {
    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(
        Stage2ReadinessTestInputs::mechanism_ready_without_manual_or_live(),
    )
    .await
    .expect("stage 2 readiness report");

    let mut unsafe_evidence = Vec::new();
    for (section, summary) in [
        ("control_plane", &report.control_plane),
        ("memory_proposal", &report.memory_proposal),
        ("failure_recovery", &report.failure_recovery),
    ] {
        for item in &summary.coverage {
            for evidence in &item.evidence {
                if !metadata_safe_test_label(evidence) {
                    unsafe_evidence.push(format!("{section}:{}:{evidence}", item.id));
                }
            }
        }
    }

    assert!(
        unsafe_evidence.is_empty(),
        "Stage 2 readiness coverage evidence must be metadata-safe: {unsafe_evidence:?}"
    );
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_rejects_unsafe_coverage_evidence_credit() {
    let mut inputs = Stage2ReadinessTestInputs::fully_ready_for_tests(
        complete_manual_records(),
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live"),
    );
    inputs.inject_unsafe_coverage_evidence_for_tests();

    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(inputs)
        .await
        .expect("stage 2 readiness report");

    assert_eq!(
        report.recommendation,
        "not_ready_for_limited_internal_trial"
    );
    assert!(!report.control_plane.ready);
    assert!(report
        .control_plane
        .blockers
        .contains(&"stage2_metadata_unsafe_evidence_label".to_string()));
    assert!(report
        .blockers
        .contains(&"stage2_control_plane_not_ready".to_string()));
}

fn metadata_safe_test_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'))
}

fn metadata_safe_test_digest(value: &str) -> bool {
    value
        .strip_prefix("bytes:")
        .and_then(|suffix| suffix.split_once(" hash:sha256:"))
        .is_some_and(|(bytes, hash)| {
            !bytes.is_empty()
                && bytes.chars().all(|ch| ch.is_ascii_digit())
                && hash.len() == 64
                && hash.chars().all(|ch| ch.is_ascii_hexdigit())
        })
}

#[test]
fn main_chat_stage2_manual_dogfood_requires_two_reviewers_all_p0_and_trace_ids() {
    let complete = evaluate_stage2_manual_dogfood_records_for_tests(&complete_manual_records());
    assert!(complete.attempted);
    assert!(complete.ready, "{:?}", complete.blockers);
    assert_eq!(complete.reviewer_count, 2);
    assert_eq!(complete.required_scenario_count, 24);
    assert_eq!(complete.attempted_scenario_count, 24);
    assert_eq!(complete.passed_scenario_count, 24);
    assert!(complete.missing_scenario_ids.is_empty());
    assert!(complete.trace_ids_present);

    let mut missing_reviewer = complete_manual_records();
    for record in &mut missing_reviewer {
        record.reviewer_id = "reviewer-a".into();
    }
    let missing_reviewer_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&missing_reviewer);
    assert!(!missing_reviewer_summary.ready);
    assert!(missing_reviewer_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_reviewer_count_below_2"));

    let mut optional_only_second_reviewer = missing_reviewer.clone();
    let mut optional_review = optional_only_second_reviewer[0].clone();
    optional_review.scenario_id = "S2-D25".into();
    optional_review.severity = "P1".into();
    optional_review.reviewer_id = "reviewer-b".into();
    optional_only_second_reviewer.push(optional_review);
    let optional_only_second_reviewer_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&optional_only_second_reviewer);
    assert!(!optional_only_second_reviewer_summary.ready);
    assert_eq!(optional_only_second_reviewer_summary.reviewer_count, 2);
    assert!(!optional_only_second_reviewer_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_reviewer_count_below_2"));
    assert!(optional_only_second_reviewer_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_p0_reviewer_count_below_2"));

    let mut invalid_reviewer_id = complete_manual_records();
    invalid_reviewer_id[0].reviewer_id = "reviewer a".into();
    let invalid_reviewer_id_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&invalid_reviewer_id);
    assert!(!invalid_reviewer_id_summary.ready);
    assert!(invalid_reviewer_id_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_reviewer_id_invalid"));

    let mut unknown_reviewer_id = complete_manual_records();
    unknown_reviewer_id[0].reviewer_id = "unknown".into();
    let unknown_reviewer_id_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&unknown_reviewer_id);
    assert!(!unknown_reviewer_id_summary.ready);
    assert!(unknown_reviewer_id_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(unknown_reviewer_id_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_reviewer_id_invalid"));

    let mut missing_trace = complete_manual_records();
    missing_trace[0].run_id.clear();
    let missing_trace_summary = evaluate_stage2_manual_dogfood_records_for_tests(&missing_trace);
    assert!(!missing_trace_summary.ready);
    assert!(!missing_trace_summary.trace_ids_present);
    assert_eq!(missing_trace_summary.passed_scenario_count, 23);
    assert!(missing_trace_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(missing_trace_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_trace_ids_missing"));

    let mut unknown_trace = complete_manual_records();
    unknown_trace[0].task_id = "unknown".into();
    unknown_trace[0].run_id = "unknown".into();
    let unknown_trace_summary = evaluate_stage2_manual_dogfood_records_for_tests(&unknown_trace);
    assert!(!unknown_trace_summary.ready);
    assert!(!unknown_trace_summary.trace_ids_present);
    assert_eq!(unknown_trace_summary.passed_scenario_count, 23);
    assert!(unknown_trace_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(unknown_trace_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_trace_ids_missing"));

    let mut missing_commit = complete_manual_records();
    missing_commit[0].build_commit.clear();
    let missing_commit_summary = evaluate_stage2_manual_dogfood_records_for_tests(&missing_commit);
    assert!(!missing_commit_summary.ready);
    assert_eq!(missing_commit_summary.passed_scenario_count, 23);
    assert!(missing_commit_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(missing_commit_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_build_commit_missing"));

    let mut missing_provider_mode = complete_manual_records();
    missing_provider_mode[0].provider_mode.clear();
    let missing_provider_mode_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&missing_provider_mode);
    assert!(!missing_provider_mode_summary.ready);
    assert!(missing_provider_mode_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_provider_mode_missing"));

    let mut invalid_provider_mode = complete_manual_records();
    invalid_provider_mode[0].provider_mode = "sandbox".into();
    let invalid_provider_mode_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&invalid_provider_mode);
    assert!(!invalid_provider_mode_summary.ready);
    assert!(invalid_provider_mode_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_provider_mode_invalid"));

    let mut wrapped_provider_mode = complete_manual_records();
    wrapped_provider_mode[0].provider_mode = " deterministic ".into();
    let wrapped_provider_mode_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&wrapped_provider_mode);
    assert!(!wrapped_provider_mode_summary.ready);
    assert!(wrapped_provider_mode_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(wrapped_provider_mode_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_provider_mode_invalid"));

    let mut alias_provider_mode = complete_manual_records();
    alias_provider_mode[0].provider_mode = "live_provider".into();
    let alias_provider_mode_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&alias_provider_mode);
    assert!(!alias_provider_mode_summary.ready);
    assert!(alias_provider_mode_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(alias_provider_mode_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_provider_mode_invalid"));

    let mut missing_prompt = complete_manual_records();
    missing_prompt[0].prompt.clear();
    let missing_prompt_summary = evaluate_stage2_manual_dogfood_records_for_tests(&missing_prompt);
    assert!(!missing_prompt_summary.ready);
    assert!(missing_prompt_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_prompt_missing"));

    let mut missing_notes = complete_manual_records();
    missing_notes[0].notes.clear();
    let missing_notes_summary = evaluate_stage2_manual_dogfood_records_for_tests(&missing_notes);
    assert!(!missing_notes_summary.ready);
    assert!(missing_notes_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_notes_missing"));

    let mut missing_user_visible_problem = complete_manual_records();
    missing_user_visible_problem[0].user_visible_problem.clear();
    let missing_user_visible_problem_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&missing_user_visible_problem);
    assert!(!missing_user_visible_problem_summary.ready);
    assert!(missing_user_visible_problem_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_user_visible_problem_missing"));

    let mut missing_backend_runtime_problem = complete_manual_records();
    missing_backend_runtime_problem[0]
        .backend_runtime_problem
        .clear();
    let missing_backend_runtime_problem_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&missing_backend_runtime_problem);
    assert!(!missing_backend_runtime_problem_summary.ready);
    assert!(missing_backend_runtime_problem_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_backend_runtime_problem_missing"));

    let mut unknown_manual_text = complete_manual_records();
    unknown_manual_text[0].prompt = "unknown".into();
    unknown_manual_text[0].notes = "unknown".into();
    unknown_manual_text[0].user_visible_problem = "unknown".into();
    unknown_manual_text[0].backend_runtime_problem = "unknown".into();
    let unknown_manual_text_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&unknown_manual_text);
    assert!(!unknown_manual_text_summary.ready);
    assert!(unknown_manual_text_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(unknown_manual_text_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_prompt_missing"));
    assert!(unknown_manual_text_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_notes_missing"));
    assert!(unknown_manual_text_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_user_visible_problem_missing"));
    assert!(unknown_manual_text_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_backend_runtime_problem_missing"));

    let mut invalid_row_blocker = complete_manual_records();
    invalid_row_blocker[0].blockers = vec!["raw blocker text".into()];
    let invalid_row_blocker_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&invalid_row_blocker);
    assert!(!invalid_row_blocker_summary.ready);
    assert!(invalid_row_blocker_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_blocker_label_invalid"));

    let mut invalid_result = complete_manual_records();
    invalid_result[0].result = "success".into();
    let invalid_result_summary = evaluate_stage2_manual_dogfood_records_for_tests(&invalid_result);
    assert!(!invalid_result_summary.ready);
    assert!(invalid_result_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_result_invalid"));

    let mut wrapped_result = complete_manual_records();
    wrapped_result[0].result = " pass ".into();
    let wrapped_result_summary = evaluate_stage2_manual_dogfood_records_for_tests(&wrapped_result);
    assert!(!wrapped_result_summary.ready);
    assert_eq!(wrapped_result_summary.passed_scenario_count, 23);
    assert!(wrapped_result_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(wrapped_result_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_result_invalid"));

    let mut not_attempted = complete_manual_records();
    not_attempted[0].result = "not attempted".into();
    let not_attempted_summary = evaluate_stage2_manual_dogfood_records_for_tests(&not_attempted);
    assert!(!not_attempted_summary.ready);
    assert_eq!(not_attempted_summary.attempted_scenario_count, 23);
    assert_eq!(not_attempted_summary.passed_scenario_count, 23);
    assert_eq!(
        not_attempted_summary.missing_scenario_ids,
        vec!["S2-D01".to_string()]
    );
    assert!(not_attempted_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(not_attempted_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_required_scenarios_missing"));

    let mut duplicate_not_attempted = complete_manual_records();
    let mut extra_not_attempted = duplicate_not_attempted[0].clone();
    extra_not_attempted.reviewer_id = "reviewer-b".into();
    extra_not_attempted.result = "not attempted".into();
    duplicate_not_attempted.push(extra_not_attempted);
    let duplicate_not_attempted_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&duplicate_not_attempted);
    assert!(
        duplicate_not_attempted_summary.ready,
        "{:?}",
        duplicate_not_attempted_summary.blockers
    );
    assert_eq!(duplicate_not_attempted_summary.attempted_scenario_count, 24);
    assert_eq!(duplicate_not_attempted_summary.passed_scenario_count, 24);

    let mut non_p0 = complete_manual_records();
    non_p0[0].severity = "P1".into();
    let non_p0_summary = evaluate_stage2_manual_dogfood_records_for_tests(&non_p0);
    assert!(!non_p0_summary.ready);
    assert!(non_p0_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(non_p0_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_required_scenarios_not_p0"));

    let mut invalid_severity = complete_manual_records();
    invalid_severity[0].severity = "critical".into();
    let invalid_severity_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&invalid_severity);
    assert!(!invalid_severity_summary.ready);
    assert!(invalid_severity_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_severity_invalid"));

    let mut wrapped_severity = complete_manual_records();
    wrapped_severity[0].severity = " P0 ".into();
    let wrapped_severity_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&wrapped_severity);
    assert!(!wrapped_severity_summary.ready);
    assert!(wrapped_severity_summary
        .failed_scenario_ids
        .contains(&"S2-D01".to_string()));
    assert!(wrapped_severity_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_severity_invalid"));
}

#[test]
fn main_chat_stage2_manual_dogfood_rejects_unknown_scenario_rows() {
    let mut with_optional_p1 = complete_manual_records();
    let mut optional = with_optional_p1[0].clone();
    optional.scenario_id = "S2-D25".into();
    optional.result = "blocked".into();
    optional.severity = "P1".into();
    optional.blockers = vec!["manual_optional_p1_blocker".into()];
    with_optional_p1.push(optional);
    let optional_summary = evaluate_stage2_manual_dogfood_records_for_tests(&with_optional_p1);
    assert!(optional_summary.ready, "{:?}", optional_summary.blockers);

    let mut unsafe_optional = complete_manual_records();
    let mut optional = unsafe_optional[0].clone();
    optional.scenario_id = "S2-D25".into();
    optional.result = "blocked".into();
    optional.severity = "P1".into();
    optional.blockers = vec!["raw optional blocker text".into()];
    unsafe_optional.push(optional);
    let unsafe_optional_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&unsafe_optional);
    assert!(!unsafe_optional_summary.ready);
    assert_eq!(unsafe_optional_summary.passed_scenario_count, 24);
    assert!(unsafe_optional_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_blocker_label_invalid"));

    let mut invalid_optional_result = complete_manual_records();
    let mut optional = invalid_optional_result[0].clone();
    optional.scenario_id = "S2-D25".into();
    optional.result = "success".into();
    optional.severity = "P1".into();
    invalid_optional_result.push(optional);
    let invalid_optional_result_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&invalid_optional_result);
    assert!(!invalid_optional_result_summary.ready);
    assert_eq!(invalid_optional_result_summary.passed_scenario_count, 24);
    assert!(invalid_optional_result_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_result_invalid"));

    let mut wrong_optional_priority = complete_manual_records();
    let mut optional = wrong_optional_priority[0].clone();
    optional.scenario_id = "S2-D25".into();
    optional.severity = "P0".into();
    wrong_optional_priority.push(optional);
    let wrong_optional_priority_summary =
        evaluate_stage2_manual_dogfood_records_for_tests(&wrong_optional_priority);
    assert!(!wrong_optional_priority_summary.ready);
    assert_eq!(wrong_optional_priority_summary.passed_scenario_count, 24);
    assert!(wrong_optional_priority_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_optional_scenarios_not_p1"));

    let mut with_unknown = complete_manual_records();
    let mut unknown = with_unknown[0].clone();
    unknown.scenario_id = "S2-D99".into();
    with_unknown.push(unknown);
    let unknown_summary = evaluate_stage2_manual_dogfood_records_for_tests(&with_unknown);
    assert!(!unknown_summary.ready);
    assert!(unknown_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_unknown_scenario_id"));
}

#[test]
fn main_chat_stage2_manual_dogfood_rejects_unknown_build_commit_rows() {
    let mut records = complete_manual_records();
    records[0].build_commit = "unknown".into();

    let summary = evaluate_stage2_manual_dogfood_records_for_tests(&records);

    assert!(!summary.ready);
    assert!(summary
        .failed_scenario_ids
        .contains(&records[0].scenario_id));
    assert!(summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_build_commit_missing"));
}

#[test]
fn main_chat_stage2_manual_dogfood_rejects_none_placeholder_identity_labels() {
    let mut records = complete_manual_records();
    records[0].reviewer_id = "none".into();
    records[0].task_id = "none".into();
    records[0].run_id = "none".into();
    records[0].build_commit = "none".into();

    let summary = evaluate_stage2_manual_dogfood_records_for_tests(&records);

    assert!(!summary.ready);
    assert!(summary
        .failed_scenario_ids
        .contains(&records[0].scenario_id));
    assert!(summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_reviewer_id_invalid"));
    assert!(summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_trace_ids_missing"));
    assert!(summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_build_commit_missing"));
}

#[test]
fn main_chat_stage2_manual_dogfood_rejects_fake_reviewer_and_trace_labels() {
    let mut records = complete_manual_records();
    records[0].reviewer_id = "mock-reviewer".into();
    records[0].task_id = "missing-trace".into();
    records[0].run_id = "mock-run".into();

    let summary = evaluate_stage2_manual_dogfood_records_for_tests(&records);

    assert!(!summary.ready);
    assert!(summary
        .failed_scenario_ids
        .contains(&records[0].scenario_id));
    assert!(summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_reviewer_id_invalid"));
    assert!(summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_trace_ids_missing"));
}

#[test]
fn main_chat_stage2_manual_dogfood_artifact_requires_schema_and_commit() {
    let complete_artifact = Stage2ManualDogfoodArtifact {
        schema_version: "stage2-manual-dogfood-v1".into(),
        commit: "abc123".into(),
        reviewer_records: complete_manual_records(),
    };
    let complete = evaluate_stage2_manual_dogfood_artifact_for_tests(&complete_artifact);
    assert!(complete.ready, "{:?}", complete.blockers);

    let mut wrong_schema = complete_artifact.clone();
    wrong_schema.schema_version = "stage1-manual-dogfood-v1".into();
    let wrong_schema_summary = evaluate_stage2_manual_dogfood_artifact_for_tests(&wrong_schema);
    assert!(!wrong_schema_summary.ready);
    assert!(wrong_schema_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_artifact_schema_invalid"));

    let mut missing_commit = complete_artifact.clone();
    missing_commit.commit.clear();
    let missing_commit_summary = evaluate_stage2_manual_dogfood_artifact_for_tests(&missing_commit);
    assert!(!missing_commit_summary.ready);
    assert!(missing_commit_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_artifact_commit_missing"));

    let mut unknown_commit = complete_artifact.clone();
    unknown_commit.commit = "unknown".into();
    for record in &mut unknown_commit.reviewer_records {
        record.build_commit = "unknown".into();
    }
    let unknown_commit_summary = evaluate_stage2_manual_dogfood_artifact_for_tests(&unknown_commit);
    assert!(!unknown_commit_summary.ready);
    assert!(unknown_commit_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_artifact_commit_missing"));

    let mut mismatched_row_commit = complete_artifact.clone();
    mismatched_row_commit.reviewer_records[0].build_commit = "different-build".into();
    let mismatched_row_commit_summary =
        evaluate_stage2_manual_dogfood_artifact_for_tests(&mismatched_row_commit);
    assert!(!mismatched_row_commit_summary.ready);
    assert!(mismatched_row_commit_summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_artifact_commit_mismatch"));
}

#[test]
fn main_chat_stage2_manual_dogfood_invalid_artifact_uses_metadata_safe_blocker() {
    let artifact_path = std::env::temp_dir().join(format!(
        "openlife-stage2-invalid-manual-artifact-{}.json",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&artifact_path, b"{ invalid json").expect("write invalid manual artifact");
    let summary = read_stage2_manual_dogfood_artifact_from_path_for_tests(&artifact_path);
    let _ = std::fs::remove_file(&artifact_path);

    assert!(summary.attempted);
    assert!(!summary.ready);
    assert!(summary.artifact_digest.is_some());
    assert_eq!(
        summary.blockers,
        vec!["stage2_manual_dogfood_artifact_invalid".to_string()]
    );
    assert!(summary.blockers.iter().all(|blocker| blocker
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'))));
}

#[test]
fn main_chat_stage2_manual_dogfood_artifact_rejects_stale_build_commit() {
    let artifact_path = std::env::temp_dir().join(format!(
        "openlife-stage2-stale-manual-artifact-{}.json",
        uuid::Uuid::new_v4()
    ));
    let artifact = Stage2ManualDogfoodArtifact {
        schema_version: "stage2-manual-dogfood-v1".into(),
        commit: "old-build".into(),
        reviewer_records: complete_stage2_manual_dogfood_records(
            "reviewer-a",
            "reviewer-b",
            "old-build",
        ),
    };
    let bytes = serde_json::to_vec(&artifact).expect("serialize stale manual artifact");
    std::fs::write(&artifact_path, bytes).expect("write stale manual artifact");

    let summary = read_stage2_manual_dogfood_artifact_from_path_with_expected_commit_for_tests(
        &artifact_path,
        Some("current-build"),
    );
    let _ = std::fs::remove_file(&artifact_path);

    assert!(summary.attempted);
    assert!(!summary.ready);
    assert!(summary.artifact_digest.is_some());
    assert!(summary
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_manual_artifact_current_commit_mismatch"));
}

#[test]
fn main_chat_stage2_live_provider_requires_l2_p0_matrix_and_rejects_local_credit() {
    let complete = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live"),
    );
    assert!(complete.ready, "{:?}", complete.blockers);
    assert_eq!(complete.required_scenario_count, 10);
    assert_eq!(complete.passed_scenario_count, 10);
    assert_eq!(complete.scenario_reports.len(), 10);
    assert!(complete.scenario_reports.iter().all(|row| row.credited));
    assert_eq!(complete.model_invoked_count, 10);
    assert_eq!(complete.main_chat_invoked_count, 10);
    assert_eq!(complete.local_or_mock_credit_rejected, 0);

    let local = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("local_test_http", "gpt-local"),
    );
    assert!(!local.ready);
    assert_eq!(local.scenario_reports.len(), 10);
    assert!(local.scenario_reports.iter().all(|row| !row.credited));
    assert_eq!(local.local_or_mock_credit_rejected, 10);
    assert_eq!(local.model_invoked_count, 0);
    assert_eq!(local.main_chat_invoked_count, 0);
    assert_eq!(local.provider, None);
    assert!(local
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_provider_local_or_mock_credit_rejected"));

    let private_ip = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("10.0.0.7", "gpt-4.1-live"),
    );
    assert!(!private_ip.ready);
    assert_eq!(private_ip.passed_scenario_count, 0);
    assert_eq!(private_ip.local_or_mock_credit_rejected, 10);
    assert_eq!(private_ip.model_invoked_count, 0);
    assert_eq!(private_ip.main_chat_invoked_count, 0);
    assert_eq!(private_ip.provider, None);
    assert!(private_ip.scenario_reports.iter().all(|row| {
        !row.credited
            && row
                .blockers
                .contains(&"stage2_live_external_provider_missing".to_string())
    }));

    let embedded_private_ip = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("provider-10.0.0.7", "gpt-4.1-live"),
    );
    assert!(!embedded_private_ip.ready);
    assert_eq!(embedded_private_ip.passed_scenario_count, 0);
    assert_eq!(embedded_private_ip.local_or_mock_credit_rejected, 10);
    assert_eq!(embedded_private_ip.provider, None);

    let separated_private_ip = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("provider_10_0_0_7", "gpt-4.1-live"),
    );
    assert!(!separated_private_ip.ready);
    assert_eq!(separated_private_ip.passed_scenario_count, 0);
    assert_eq!(separated_private_ip.local_or_mock_credit_rejected, 10);
    assert_eq!(separated_private_ip.provider, None);

    let suffixed_private_ip = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("provider_10_0_0_7_999", "gpt-4.1-live"),
    );
    assert!(!suffixed_private_ip.ready);
    assert_eq!(suffixed_private_ip.passed_scenario_count, 0);
    assert_eq!(suffixed_private_ip.local_or_mock_credit_rejected, 10);
    assert_eq!(suffixed_private_ip.provider, None);

    let mut non_external_endpoint_evidence =
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    for row in &mut non_external_endpoint_evidence {
        row.provider_endpoint_kind = "local_test_http".into();
    }
    let non_external_endpoint =
        stage2_live_provider_summary_for_tests(true, non_external_endpoint_evidence);
    assert!(!non_external_endpoint.ready);
    assert_eq!(non_external_endpoint.passed_scenario_count, 0);
    assert_eq!(non_external_endpoint.local_or_mock_credit_rejected, 10);
    assert_eq!(non_external_endpoint.model_invoked_count, 0);
    assert_eq!(non_external_endpoint.main_chat_invoked_count, 0);
    assert_eq!(non_external_endpoint.provider, None);
    assert_eq!(non_external_endpoint.model, None);

    let mut partial = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    partial.retain(|row| row.scenario_id != "L2-L10");
    let partial = stage2_live_provider_summary_for_tests(true, partial);
    assert!(!partial.ready);
    assert!(partial.failed_scenario_ids.contains(&"L2-L10".into()));
    assert!(partial
        .scenario_reports
        .iter()
        .any(|row| row.scenario_id == "L2-L10"
            && row.status == "missing"
            && !row.credited
            && row
                .blockers
                .iter()
                .any(|blocker| blocker == "live_provider_failure_hidden")));

    let mut duplicated = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let mut bad_l04 = duplicated
        .iter()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 evidence")
        .clone();
    bad_l04.status = "failed".into();
    bad_l04.model_invoked = false;
    bad_l04.required_evidence.clear();
    bad_l04
        .blockers
        .push("provider_backed_web_agent_loop_not_executed".into());
    duplicated.insert(0, bad_l04);
    let duplicated = stage2_live_provider_summary_for_tests(true, duplicated);
    assert!(!duplicated.ready);
    assert!(duplicated.failed_scenario_ids.contains(&"L2-L04".into()));
    assert!(duplicated
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_provider_duplicate_scenario_evidence"));

    let mut runtime_partial =
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    runtime_partial.retain(|row| {
        matches!(
            row.scenario_id.as_str(),
            "L2-L01" | "L2-L04" | "L2-L05" | "L2-L06"
        )
    });
    let explicit_runtime_matrix = stage2_live_provider_attempted_p0_matrix_evidence_for_tests(
        runtime_partial,
        "openai",
        "gpt-4.1-live",
        "external_provider",
    );
    let explicit_runtime =
        stage2_live_provider_summary_for_tests(true, explicit_runtime_matrix.clone());
    assert!(!explicit_runtime.ready);
    assert_eq!(explicit_runtime.scenario_reports.len(), 10);
    assert_eq!(explicit_runtime.passed_scenario_count, 4);
    assert!(explicit_runtime
        .scenario_reports
        .iter()
        .all(|row| row.status != "missing"));
    assert!(explicit_runtime
        .scenario_reports
        .iter()
        .any(|row| row.scenario_id == "L2-L02"
            && row.status == "blocked"
            && !row.credited
            && row
                .blockers
                .iter()
                .any(|blocker| blocker == "live_provider_read_action_missing")
            && row
                .blockers
                .iter()
                .any(|blocker| blocker == "stage2_live_scenario_runner_not_implemented_L2-L02")));
    assert!(explicit_runtime
        .scenario_reports
        .iter()
        .any(|row| row.scenario_id == "L2-L10"
            && row.status == "blocked"
            && !row.credited
            && row
                .blockers
                .iter()
                .any(|blocker| blocker == "live_provider_failure_hidden")));
    assert!(explicit_runtime_matrix
        .iter()
        .filter(|row| row.status == "blocked")
        .all(|row| row
            .blockers
            .iter()
            .any(|blocker| blocker.starts_with("stage2_live_scenario_runner_not_implemented_"))));

    let unsafe_global_blockers = vec!["raw provider preflight: leaked detail".to_string()];
    let sanitized_blocked_rows =
        stage2_live_provider_attempted_p0_matrix_evidence_with_blockers_for_tests(
            Vec::new(),
            "openai",
            "gpt-4.1-live",
            "external_provider",
            &unsafe_global_blockers,
        );
    assert!(sanitized_blocked_rows
        .iter()
        .all(|row| row.blockers.iter().all(|blocker| blocker
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/')))));
    assert!(sanitized_blocked_rows.iter().any(|row| row
        .blockers
        .contains(&"stage2_metadata_unsafe_blocker_label".to_string())));
    assert!(!sanitized_blocked_rows.iter().any(|row| row
        .blockers
        .iter()
        .any(|blocker| blocker.contains("raw provider preflight"))));
}

#[test]
fn main_chat_stage2_live_provider_rejects_unknown_scenario_evidence_rows() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let mut unknown = evidence[0].clone();
    unknown.scenario_id = "L2-L99".into();
    evidence.push(unknown);

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    assert_eq!(summary.passed_scenario_count, 10);
    assert_eq!(summary.scenario_reports.len(), 10);
    assert!(summary
        .blockers
        .contains(&"stage2_live_provider_unknown_scenario_evidence".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_requires_consistent_provider_and_model_identity() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l10 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L10")
        .expect("L2-L10 evidence");
    l10.provider = "anthropic".into();
    l10.model = "claude-live".into();

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    assert_eq!(summary.passed_scenario_count, 9);
    assert_eq!(summary.provider, None);
    assert_eq!(summary.model, None);
    assert!(summary.failed_scenario_ids.contains(&"L2-L10".to_string()));
    assert!(summary
        .blockers
        .contains(&"stage2_live_provider_identity_inconsistent".to_string()));
    let l10_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L10")
        .expect("L2-L10 report");
    assert!(!l10_report.credited);
    assert!(l10_report
        .blockers
        .contains(&"stage2_live_provider_identity_inconsistent".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_rejects_mock_or_local_model_identity() {
    let summary = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("openai", "mock-model"),
    );

    assert!(!summary.ready);
    assert_eq!(summary.passed_scenario_count, 0);
    assert_eq!(summary.local_or_mock_credit_rejected, 10);
    assert_eq!(summary.model, None);
    assert!(summary
        .blockers
        .contains(&"stage2_live_provider_local_or_mock_credit_rejected".to_string()));
    assert!(summary.scenario_reports.iter().all(|row| {
        !row.credited
            && row
                .blockers
                .contains(&"stage2_live_model_identity_missing".to_string())
    }));

    let ip_model_summary = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("openai", "model_10_0_0_7"),
    );

    assert!(!ip_model_summary.ready);
    assert_eq!(ip_model_summary.passed_scenario_count, 0);
    assert_eq!(ip_model_summary.local_or_mock_credit_rejected, 10);
    assert_eq!(ip_model_summary.model, None);
    assert!(ip_model_summary.scenario_reports.iter().all(|row| {
        !row.credited
            && row
                .blockers
                .contains(&"stage2_live_model_identity_missing".to_string())
    }));

    let dotted_ip_model_summary = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("openai", "10.0.0.7"),
    );

    assert!(!dotted_ip_model_summary.ready);
    assert_eq!(dotted_ip_model_summary.passed_scenario_count, 0);
    assert_eq!(dotted_ip_model_summary.local_or_mock_credit_rejected, 10);
    assert_eq!(dotted_ip_model_summary.model, None);

    let unknown_provider_summary = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("unknown", "gpt-4.1-live"),
    );
    assert!(!unknown_provider_summary.ready);
    assert_eq!(unknown_provider_summary.passed_scenario_count, 0);
    assert_eq!(unknown_provider_summary.local_or_mock_credit_rejected, 10);
    assert_eq!(unknown_provider_summary.provider, None);
    assert!(unknown_provider_summary.scenario_reports.iter().all(|row| {
        !row.credited
            && row
                .blockers
                .contains(&"stage2_live_external_provider_missing".to_string())
    }));

    let placeholder_provider_summary = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("none-provider", "gpt-4.1-live"),
    );
    assert!(!placeholder_provider_summary.ready);
    assert_eq!(placeholder_provider_summary.passed_scenario_count, 0);
    assert_eq!(
        placeholder_provider_summary.local_or_mock_credit_rejected,
        10
    );
    assert_eq!(placeholder_provider_summary.provider, None);
    assert!(placeholder_provider_summary
        .scenario_reports
        .iter()
        .all(|row| {
            !row.credited
                && row
                    .blockers
                    .contains(&"stage2_live_external_provider_missing".to_string())
        }));

    let loopback_provider_summary = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("loopback-provider", "gpt-4.1-live"),
    );
    assert!(!loopback_provider_summary.ready);
    assert_eq!(loopback_provider_summary.passed_scenario_count, 0);
    assert_eq!(loopback_provider_summary.local_or_mock_credit_rejected, 10);
    assert_eq!(loopback_provider_summary.provider, None);
    assert!(loopback_provider_summary
        .scenario_reports
        .iter()
        .all(|row| {
            !row.credited
                && row
                    .blockers
                    .contains(&"stage2_live_external_provider_missing".to_string())
        }));

    let private_network_model_summary = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("openai", "private-network-model"),
    );
    assert!(!private_network_model_summary.ready);
    assert_eq!(private_network_model_summary.passed_scenario_count, 0);
    assert_eq!(
        private_network_model_summary.local_or_mock_credit_rejected,
        10
    );
    assert_eq!(private_network_model_summary.model, None);
    assert!(private_network_model_summary
        .scenario_reports
        .iter()
        .all(|row| {
            !row.credited
                && row
                    .blockers
                    .contains(&"stage2_live_model_identity_missing".to_string())
        }));

    let unknown_model_summary = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("openai", "unknown"),
    );
    assert!(!unknown_model_summary.ready);
    assert_eq!(unknown_model_summary.passed_scenario_count, 0);
    assert_eq!(unknown_model_summary.local_or_mock_credit_rejected, 10);
    assert_eq!(unknown_model_summary.model, None);
    assert!(unknown_model_summary.scenario_reports.iter().all(|row| {
        !row.credited
            && row
                .blockers
                .contains(&"stage2_live_model_identity_missing".to_string())
    }));

    let none_model_summary = stage2_live_provider_summary_for_tests(
        true,
        complete_stage2_live_provider_evidence_for_tests("openai", "none"),
    );
    assert!(!none_model_summary.ready);
    assert_eq!(none_model_summary.passed_scenario_count, 0);
    assert_eq!(none_model_summary.local_or_mock_credit_rejected, 10);
    assert_eq!(none_model_summary.model, None);
    assert!(none_model_summary.scenario_reports.iter().all(|row| {
        !row.credited
            && row
                .blockers
                .contains(&"stage2_live_model_identity_missing".to_string())
    }));
}

#[test]
fn main_chat_stage2_live_provider_requires_completed_runtime_status_for_credit() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l01 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 evidence");
    l01.status = "passed".into();

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    assert_eq!(summary.passed_scenario_count, 9);
    assert!(summary.failed_scenario_ids.contains(&"L2-L01".to_string()));
    let l01_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 report");
    assert!(!l01_report.credited);
    assert_eq!(l01_report.status, "failed");
    assert!(l01_report
        .blockers
        .contains(&"stage2_live_status_not_completed".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_rejects_unknown_trace_ids() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l01 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 evidence");
    l01.task_session_id = "unknown".into();
    l01.run_id = "unknown".into();

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    assert_eq!(summary.passed_scenario_count, 9);
    assert_eq!(summary.model_invoked_count, 9);
    assert_eq!(summary.main_chat_invoked_count, 9);
    let l01_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 report");
    assert!(!l01_report.credited);
    assert!(!l01_report.run_id_present);
    assert!(!l01_report.task_session_id_present);
    assert!(l01_report
        .blockers
        .contains(&"stage2_live_trace_ids_missing".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_rejects_unknown_response_preview() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l01 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 evidence");
    l01.response_preview = "unknown".into();

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    assert_eq!(summary.passed_scenario_count, 9);
    let l01_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 report");
    assert!(!l01_report.credited);
    assert!(!l01_report.response_preview_present);
    assert!(l01_report
        .blockers
        .contains(&"stage2_live_response_preview_missing".to_string()));

    let mut none_preview_evidence =
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    none_preview_evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 evidence")
        .response_preview = "none".into();

    let none_preview_summary = stage2_live_provider_summary_for_tests(true, none_preview_evidence);
    assert!(!none_preview_summary.ready);
    assert_eq!(none_preview_summary.passed_scenario_count, 9);
    let none_l01_report = none_preview_summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 report");
    assert!(!none_l01_report.credited);
    assert!(!none_l01_report.response_preview_present);
    assert!(none_l01_report
        .blockers
        .contains(&"stage2_live_response_preview_missing".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_rejects_fake_response_preview_labels() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l01 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 evidence");
    l01.response_preview = "scripted response from fake harness".into();

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    assert_eq!(summary.passed_scenario_count, 9);
    let l01_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 report");
    assert!(!l01_report.credited);
    assert!(!l01_report.response_preview_present);
    assert!(l01_report
        .blockers
        .contains(&"stage2_live_response_preview_missing".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_invocation_counts_only_credited_proofs() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l04 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 evidence");
    l04.direct_writes_executed = true;

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    assert_eq!(summary.passed_scenario_count, 9);
    assert_eq!(summary.model_invoked_count, 9);
    assert_eq!(summary.main_chat_invoked_count, 9);
    let l04_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 report");
    assert!(!l04_report.credited);
    assert!(l04_report
        .blockers
        .contains(&"stage2_live_direct_writes_detected".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_blockers_are_metadata_safe_labels() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l04 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 evidence");
    l04.required_evidence = vec!["L2-L04".into(), "real_provider_model_invoked".into()];
    evidence.retain(|row| row.scenario_id != "L2-L02");

    let evidence = stage2_live_provider_attempted_p0_matrix_evidence_for_tests(
        evidence,
        "openai",
        "gpt-4.1-live",
        "external_provider",
    );
    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    for blocker in summary
        .scenario_reports
        .iter()
        .flat_map(|row| row.blockers.iter())
    {
        assert!(
            blocker
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/')),
            "Stage 2 live blocker must be metadata-safe: {blocker}"
        );
    }
}

#[test]
fn main_chat_stage2_live_provider_redacts_unsafe_provider_endpoint_kind() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l04 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 evidence");
    l04.provider_endpoint_kind = "external_provider\nraw-endpoint-detail".into();

    let summary = stage2_live_provider_summary_for_tests(true, evidence);
    let l04_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 report");

    assert!(!l04_report.credited);
    assert_eq!(l04_report.provider_endpoint_kind, None);
    assert!(l04_report
        .blockers
        .contains(&"stage2_live_external_provider_missing".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_reports_scenario_scoped_p0_runner_plan() {
    let summary = stage2_live_provider_summary_for_tests(false, Vec::new());

    assert_eq!(summary.scenario_plans.len(), 10);
    let implemented_ids = summary
        .scenario_plans
        .iter()
        .filter(|plan| plan.runner_status == "implemented")
        .map(|plan| plan.scenario_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        implemented_ids,
        vec![
            "L2-L01", "L2-L02", "L2-L03", "L2-L04", "L2-L05", "L2-L06", "L2-L07", "L2-L08",
            "L2-L09", "L2-L10"
        ]
    );

    let l02 = summary
        .scenario_plans
        .iter()
        .find(|plan| plan.scenario_id == "L2-L02")
        .expect("L2-L02 plan");
    assert_eq!(
        l02.scenario_setup,
        "seeded_workspace_file_or_missing_file_fixture"
    );
    assert_eq!(l02.execution_source, "stage2_live_file_read_runner");
    assert_eq!(l02.runner_status, "implemented");
    assert_eq!(l02.fail_closed_blocker, "live_provider_read_action_missing");
    assert!(l02
        .required_runtime_evidence
        .contains(&"file_action_or_blocker".to_string()));

    let l03 = summary
        .scenario_plans
        .iter()
        .find(|plan| plan.scenario_id == "L2-L03")
        .expect("L2-L03 plan");
    assert_eq!(l03.scenario_setup, "web_network_policy_disabled");
    assert_eq!(l03.execution_source, "stage2_live_web_policy_runner");
    assert_eq!(l03.runner_status, "implemented");
    assert_eq!(l03.fail_closed_blocker, "live_provider_web_policy_bypass");
    assert!(l03
        .required_runtime_evidence
        .contains(&"web_policy_blocker".to_string()));

    let l04 = summary
        .scenario_plans
        .iter()
        .find(|plan| plan.scenario_id == "L2-L04")
        .expect("L2-L04 plan");
    assert_eq!(l04.scenario_setup, "governed_web_read_enabled");
    assert_eq!(l04.execution_source, "existing_v1_live_harness");
    assert_eq!(l04.runner_status, "implemented");
    assert_eq!(
        l04.fail_closed_blocker,
        "provider_backed_web_agent_loop_not_executed"
    );

    let l07 = summary
        .scenario_plans
        .iter()
        .find(|plan| plan.scenario_id == "L2-L07")
        .expect("L2-L07 plan");
    assert_eq!(l07.scenario_setup, "two_safe_read_sources_available");
    assert_eq!(l07.execution_source, "stage2_live_multistep_react_runner");
    assert_eq!(l07.runner_status, "implemented");
    assert_eq!(
        l07.fail_closed_blocker,
        "live_provider_multistep_observation_missing"
    );
    assert!(l07
        .required_runtime_evidence
        .contains(&"two_observations".to_string()));

    let l08 = summary
        .scenario_plans
        .iter()
        .find(|plan| plan.scenario_id == "L2-L08")
        .expect("L2-L08 plan");
    assert_eq!(
        l08.scenario_setup,
        "memory_proposal_enabled_no_auto_materialization"
    );
    assert_eq!(l08.execution_source, "stage2_live_memory_proposal_runner");
    assert_eq!(l08.runner_status, "implemented");
    assert_eq!(
        l08.fail_closed_blocker,
        "live_provider_memory_proposal_missing"
    );
    assert!(l08
        .required_runtime_evidence
        .contains(&"no_memory_materialization".to_string()));

    let l09 = summary
        .scenario_plans
        .iter()
        .find(|plan| plan.scenario_id == "L2-L09")
        .expect("L2-L09 plan");
    assert_eq!(l09.scenario_setup, "pending_safe_read_permission_denial");
    assert_eq!(l09.execution_source, "stage2_live_permission_denial_runner");
    assert_eq!(l09.runner_status, "implemented");
    assert_eq!(
        l09.fail_closed_blocker,
        "live_provider_permission_denial_bypassed"
    );
    assert!(l09
        .required_runtime_evidence
        .contains(&"denied_permission_state".to_string()));

    let l10 = summary
        .scenario_plans
        .iter()
        .find(|plan| plan.scenario_id == "L2-L10")
        .expect("L2-L10 plan");
    assert_eq!(l10.scenario_setup, "induced_bad_tool_or_safe_tool_failure");
    assert_eq!(l10.execution_source, "stage2_live_failure_recovery_runner");
    assert_eq!(l10.runner_status, "implemented");
    assert_eq!(l10.fail_closed_blocker, "live_provider_failure_hidden");
    assert!(l10
        .required_runtime_evidence
        .contains(&"retry_or_cancel_state".to_string()));
}

#[test]
fn main_chat_stage2_live_runner_blockers_do_not_embed_raw_errors() {
    let module_path = format!(
        "{}/src/main_chat_agent_stage2_readiness.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(module_path).expect("read stage2 readiness module");

    for forbidden in [
        "runner_failed:{error}",
        "reject_failed:{error}",
        "resume_failed:{error}",
    ] {
        assert!(
            !source.contains(forbidden),
            "Stage 2 live runner blockers must be stable metadata-safe IDs, found {forbidden}"
        );
    }
}

#[test]
fn main_chat_stage2_coverage_blockers_do_not_use_colon_suffixes() {
    let module_path = format!(
        "{}/src/main_chat_agent_stage2_readiness.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(module_path).expect("read stage2 readiness module");

    for forbidden in [
        "stage2_memory_scenario_missing:{id}",
        "stage2_recovery_scenario_missing:{id}",
    ] {
        assert!(
            !source.contains(forbidden),
            "Stage 2 coverage blockers must be metadata-safe labels, found {forbidden}"
        );
    }
}

#[test]
fn main_chat_stage2_permission_denial_runner_exercises_resume_denial_path() {
    let module_path = format!(
        "{}/src/main_chat_agent_stage2_readiness.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(module_path).expect("read stage2 readiness module");

    assert!(
        source.contains("resume_main_chat_agent_task_with_state"),
        "L2-L09 must exercise the real Main Chat resume control path after denial"
    );
    assert!(
        source.contains("\"resumeBlockedByPendingPermission\""),
        "L2-L09 must require visible resume-blocked evidence"
    );
    assert!(
        source.contains("\"automaticResumeReplayCompleted\""),
        "L2-L09 must prove the denied permission did not complete an automatic replay"
    );
}

#[test]
fn main_chat_stage2_final_delivery_uses_real_overclaim_guard_contract() {
    let module_path = format!(
        "{}/src/main_chat_agent_stage2_readiness.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(module_path).expect("read stage2 readiness module");
    let final_delivery_collector = source
        .split("async fn collect_stage2_final_delivery_summary")
        .nth(1)
        .and_then(|body| body.split("fn coverage_summary").next())
        .expect("final delivery collector source");

    assert!(
        !final_delivery_collector.contains("trait Stage2RealTaskProofExt"),
        "Stage 2 final delivery must not hide forbidden-evidence checks behind a placeholder trait"
    );
    assert!(
        !final_delivery_collector.contains("final_done_overclaim_count: 0"),
        "Stage 2 final delivery must compute overclaim count from evidence instead of hard-coding zero"
    );
    assert!(
        final_delivery_collector.contains("stage2_final_delivery_overclaim_guard_missing"),
        "Stage 2 final delivery must fail closed when no overclaim guard contract is present"
    );
}

#[tokio::test]
async fn main_chat_stage2_final_delivery_reports_p0_honesty_guards() {
    let summary = collect_stage2_final_delivery_summary_for_tests().await;

    assert!(summary.ready, "{:?}", summary.blockers);
    assert_eq!(summary.p0_scenario_count, 28);
    assert_eq!(summary.final_delivery_evidence_count, 28);
    assert_eq!(summary.final_done_overclaim_count, 0);
    assert!(!summary
        .blockers
        .contains(&"stage2_final_delivery_overclaim_guard_missing".to_string()));
    assert!(!summary
        .blockers
        .contains(&"stage2_final_delivery_overclaim_detected".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_credit_requires_scenario_runtime_evidence() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l04 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 evidence");
    l04.required_evidence = vec!["L2-L04".into(), "real_provider_model_invoked".into()];

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    assert_eq!(summary.passed_scenario_count, 9);
    let l04_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 report");
    assert!(!l04_report.credited);
    assert_eq!(l04_report.status, "failed");
    assert!(l04_report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_required_evidence_missing_selected_web_candidate"));
    assert!(l04_report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_required_evidence_missing_observation"));
}

#[test]
fn main_chat_stage2_live_provider_credit_requires_shared_provider_invocation_evidence() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l01 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 evidence");
    l01.required_evidence
        .retain(|evidence| evidence != "real_provider_model_invoked");

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    assert_eq!(summary.passed_scenario_count, 9);
    let l01_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 report");
    assert!(!l01_report.credited);
    assert!(l01_report.blockers.iter().any(|blocker| {
        blocker == "stage2_live_required_evidence_missing_real_provider_model_invoked"
    }));
}

#[test]
fn main_chat_stage2_live_provider_rejects_duplicate_required_evidence_labels() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l04 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 evidence");
    l04.required_evidence.push("selected_web_candidate".into());

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    let l04_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 report");
    assert!(!l04_report.credited);
    assert!(l04_report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_required_evidence_manifest_invalid"));
}

#[test]
fn main_chat_stage2_live_provider_rejects_extra_required_evidence_labels() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l04 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 evidence");
    l04.required_evidence
        .push("unrelated_freeform_evidence".into());

    let summary = stage2_live_provider_summary_for_tests(true, evidence);

    assert!(!summary.ready);
    let l04_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 report");
    assert!(!l04_report.credited);
    assert!(l04_report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_required_evidence_manifest_invalid"));
}

#[test]
fn main_chat_stage2_live_provider_redacts_unsafe_row_blocker_labels() {
    let mut evidence = complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    let l04 = evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 evidence");
    l04.blockers
        .push("raw provider error: HTTP 500 / leaked detail".into());

    let summary = stage2_live_provider_summary_for_tests(true, evidence);
    let l04_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 report");

    assert!(!l04_report.credited);
    assert!(!l04_report
        .blockers
        .iter()
        .any(|blocker| blocker.contains("raw provider error")));
    assert!(l04_report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_metadata_unsafe_blocker_label"));
    assert!(l04_report.blockers.iter().all(|blocker| blocker
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'))));
}

#[test]
fn main_chat_stage2_live_provider_adapter_preserves_harness_not_ready_status() {
    let mut harness_report =
        crate::main_chat_final_gate::completed_main_chat_live_provider_eval_harness_report(
            crate::main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            "openai",
            "external_provider",
            "run-live-direct",
            "task-live-direct",
            "bounded live response",
        );
    harness_report.ready = false;

    let evidence = stage2_live_provider_evidence_from_harness_reports_for_tests(
        vec![harness_report],
        "gpt-live",
    );
    let summary = stage2_live_provider_summary_for_tests(true, evidence);
    let l01_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 report");

    assert!(!l01_report.credited);
    assert!(l01_report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_harness_report_not_ready"));
}

#[test]
fn main_chat_stage2_live_provider_adapter_preserves_harness_required_evidence_manifest() {
    let mut harness_report =
        crate::main_chat_final_gate::completed_main_chat_live_provider_eval_harness_report(
            crate::main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            "openai",
            "external_provider",
            "run-live-direct",
            "task-live-direct",
            "bounded live response",
        );
    harness_report.required_evidence.clear();

    let evidence = stage2_live_provider_evidence_from_harness_reports_for_tests(
        vec![harness_report],
        "gpt-live",
    );
    let summary = stage2_live_provider_summary_for_tests(true, evidence);
    let l01_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L01")
        .expect("L2-L01 report");

    assert!(!l01_report.credited);
    assert!(l01_report
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_harness_required_evidence_manifest_invalid"));
}

#[test]
fn main_chat_stage2_live_provider_adapter_preserves_harness_final_gate_blockers() {
    let harness_report =
        crate::main_chat_final_gate::completed_main_chat_live_provider_eval_harness_report(
            crate::main_chat_final_gate::MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
            "anthropic",
            "external_provider",
            "run-live-mcp",
            "task-live-mcp",
            "bounded live mcp response",
        );
    let final_gate_blockers =
        crate::main_chat_final_gate::main_chat_live_provider_report_blockers(&harness_report);
    assert!(final_gate_blockers
        .iter()
        .any(|blocker| blocker == "live_provider_model_ranked_selection_trace_missing"));

    let evidence = stage2_live_provider_evidence_from_harness_reports_for_tests(
        vec![harness_report],
        "gpt-live",
    );
    let summary = stage2_live_provider_summary_for_tests(true, evidence);
    let l05_report = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L05")
        .expect("L2-L05 report");

    assert!(!l05_report.credited);
    assert!(l05_report
        .blockers
        .iter()
        .any(|blocker| blocker == "live_provider_model_ranked_selection_trace_missing"));
}

#[tokio::test]
async fn main_chat_stage2_live_provider_preflight_blocker_applies_to_all_l2_rows() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.llm.provider = "custom".into();
        config.llm.openai_base = "https://example.invalid/v1".into();
        config.llm.openai_key.clear();
        config.system.network_policy.enabled = false;
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "custom".into(),
            "https://example.invalid/v1".into(),
            String::new(),
            "gpt-stage2-live-preflight".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("scripted response must block stage 2 live eval");
    }

    let artifact_path = std::env::temp_dir().join(format!(
        "openlife-stage2-live-preflight-{}.json",
        uuid::Uuid::new_v4()
    ));
    let summary = read_or_run_stage2_live_provider_summary_with_artifact_path_for_tests(
        &state,
        true,
        &artifact_path,
    )
    .await
    .expect("stage 2 live provider summary");
    let _ = std::fs::remove_file(&artifact_path);

    assert!(summary.attempted);
    assert!(!summary.ready);
    assert_eq!(summary.scenario_reports.len(), 10);
    assert_eq!(summary.model_invoked_count, 0);
    assert_eq!(summary.main_chat_invoked_count, 0);
    assert!(summary
        .blockers
        .contains(&"stage2_live_provider_required_scenarios_not_all_passed".to_string()));
    assert!(summary
        .blockers
        .contains(&"stage2_live_provider_model_invocation_missing".to_string()));
    assert!(summary
        .blockers
        .contains(&"stage2_live_provider_main_chat_invocation_missing".to_string()));
    if stage2_test_current_build_commit().is_none() {
        assert!(summary
            .blockers
            .contains(&"stage2_live_artifact_commit_missing".to_string()));
    }
    assert!(summary.scenario_reports.iter().all(|row| {
        row.status == "blocked"
            && !row.credited
            && !row.main_chat_invoked
            && !row.model_invoked
            && row
                .blockers
                .contains(&"provider_api_key_missing".to_string())
            && row.blockers.contains(&"network_disabled".to_string())
            && row
                .blockers
                .contains(&"scripted_provider_response_not_allowed".to_string())
    }));
    let l03 = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L03")
        .expect("L2-L03 preflight row");
    assert!(l03
        .blockers
        .contains(&"live_provider_web_policy_bypass".to_string()));
    assert!(!l03
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_scenario_runner_not_implemented_L2-L03"));
    let l02 = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L02")
        .expect("L2-L02 preflight row");
    assert!(l02
        .blockers
        .contains(&"live_provider_read_action_missing".to_string()));
    assert!(!l02
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_scenario_runner_not_implemented_L2-L02"));
    let l10 = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L10")
        .expect("L2-L10 preflight row");
    assert!(l10
        .blockers
        .contains(&"live_provider_failure_hidden".to_string()));
    assert!(!l10
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_scenario_runner_not_implemented_L2-L10"));
    let l09 = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L09")
        .expect("L2-L09 preflight row");
    assert!(l09
        .blockers
        .contains(&"live_provider_permission_denial_bypassed".to_string()));
    assert!(!l09
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_scenario_runner_not_implemented_L2-L09"));
    let l08 = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L08")
        .expect("L2-L08 preflight row");
    assert!(l08
        .blockers
        .contains(&"live_provider_memory_proposal_missing".to_string()));
    assert!(!l08
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_scenario_runner_not_implemented_L2-L08"));
    let l07 = summary
        .scenario_reports
        .iter()
        .find(|row| row.scenario_id == "L2-L07")
        .expect("L2-L07 preflight row");
    assert!(l07
        .blockers
        .contains(&"live_provider_multistep_observation_missing".to_string()));
    assert!(!l07
        .blockers
        .iter()
        .any(|blocker| blocker == "stage2_live_scenario_runner_not_implemented_L2-L07"));
}

#[tokio::test]
async fn main_chat_stage2_live_provider_opt_in_writes_machine_readable_artifact() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let mut config = state.config.lock().await;
        config.llm.provider = "custom".into();
        config.llm.openai_base = "https://example.invalid/v1".into();
        config.llm.openai_key.clear();
        config.system.network_policy.enabled = false;
    }
    {
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            "unused-local-model".into(),
            false,
            "custom".into(),
            "https://example.invalid/v1".into(),
            String::new(),
            "gpt-stage2-live-artifact".into(),
            "text-embedding-test".into(),
            false,
        )
        .with_scripted_generation_response("scripted response must block stage 2 live eval");
    }

    let artifact_path = std::env::temp_dir().join(format!(
        "openlife-stage2-live-artifact-{}.json",
        uuid::Uuid::new_v4()
    ));
    let summary = read_or_run_stage2_live_provider_summary_with_artifact_path_for_tests(
        &state,
        true,
        &artifact_path,
    )
    .await
    .expect("stage 2 live provider summary with artifact");

    let bytes = std::fs::read(&artifact_path).expect("live provider artifact written");
    let parsed: crate::main_chat_agent_stage2_readiness::Stage2LiveProviderArtifact =
        serde_json::from_slice(&bytes).expect("typed live provider artifact");
    let _ = std::fs::remove_file(&artifact_path);

    assert_eq!(parsed.schema_version, "stage2-live-provider-evidence-v1");
    assert!(crate::main_chat_agent_stage2_readiness::metadata_safe_label_for_tests(&parsed.commit));
    assert_eq!(parsed.required_scenario_count, 10);
    assert_eq!(parsed.scenario_evidence.len(), 10);
    assert_eq!(
        summary.artifact_digest.as_deref(),
        Some(crate::main_chat_agent_stage2_readiness::digest_bytes_for_tests(&bytes).as_str())
    );
    assert!(parsed
        .scenario_evidence
        .iter()
        .any(|row| row.scenario_id == "L2-L03"
            && row.blockers.contains(&"network_disabled".to_string())));
}

#[tokio::test]
#[ignore = "requires OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1, network, and a real external provider API key"]
async fn main_chat_stage2_live_provider_summary_invokes_external_provider_when_opted_in() {
    assert!(
        crate::main_chat_live_provider_harness::main_chat_live_provider_eval_opt_in_from_env(),
        "set OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1 before running this ignored live test"
    );
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

    let artifact_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("frontend/test-results/main-chat-stage2-live-provider-report.json");
    let summary = read_or_run_stage2_live_provider_summary_with_artifact_path_for_tests(
        &state,
        true,
        &artifact_path,
    )
    .await
    .expect("stage 2 live provider summary with external live provider");
    let audit = serde_json::to_string_pretty(&serde_json::json!({
        "artifactPath": artifact_path.display().to_string(),
        "liveProvider": &summary,
    }))
    .unwrap_or_else(|error| format!("serialize stage 2 live audit failed: {error}"));

    assert!(summary.attempted, "{audit}");
    assert!(
        summary.ready
            || summary
                .blockers
                .iter()
                .all(|blocker| blocker == "stage2_live_artifact_commit_missing"),
        "{audit}"
    );
    assert_eq!(summary.required_scenario_count, 10, "{audit}");
    assert_eq!(summary.passed_scenario_count, 10, "{audit}");
    assert!(summary.failed_scenario_ids.is_empty(), "{audit}");
    assert_eq!(summary.main_chat_invoked_count, 10, "{audit}");
    assert_eq!(summary.model_invoked_count, 10, "{audit}");
    assert_eq!(summary.local_or_mock_credit_rejected, 0, "{audit}");
    assert!(
        !summary.blockers.iter().any(|blocker| blocker
            == "stage2_live_provider_required_scenarios_not_all_passed"
            || blocker == "stage2_live_provider_model_invocation_missing"
            || blocker == "stage2_live_provider_main_chat_invocation_missing"),
        "{audit}"
    );
    assert!(
        summary.artifact_digest.is_some(),
        "Stage 2 live provider artifact digest missing: {audit}"
    );
    assert!(
        summary
            .scenario_reports
            .iter()
            .all(|row| row.credited && row.main_chat_invoked && row.model_invoked),
        "{audit}"
    );
}

#[tokio::test]
async fn main_chat_stage2_live_provider_loads_existing_artifact_without_invoking_provider() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let artifact_path = std::env::temp_dir().join(format!(
        "openlife-stage2-existing-live-artifact-{}.json",
        uuid::Uuid::new_v4()
    ));
    let artifact_commit = stage2_test_current_build_commit().unwrap_or_else(|| "abc123".into());
    let valid_artifact = crate::main_chat_agent_stage2_readiness::Stage2LiveProviderArtifact {
        schema_version: "stage2-live-provider-evidence-v1".into(),
        commit: artifact_commit.clone(),
        required_scenario_count: 10,
        scenario_evidence: complete_stage2_live_provider_evidence_for_tests(
            "openai",
            "gpt-4.1-live",
        ),
    };
    let valid_bytes = serde_json::to_vec_pretty(&valid_artifact).expect("serialize live artifact");
    std::fs::write(&artifact_path, &valid_bytes).expect("write live artifact");

    let summary = read_or_run_stage2_live_provider_summary_with_artifact_path_for_tests(
        &state,
        false,
        &artifact_path,
    )
    .await
    .expect("loaded existing live provider artifact");

    assert!(summary.attempted);
    assert!(summary.ready, "{:?}", summary.blockers);
    assert_eq!(summary.passed_scenario_count, 10);
    assert_eq!(
        summary.artifact_digest.as_deref(),
        Some(
            crate::main_chat_agent_stage2_readiness::digest_bytes_for_tests(&valid_bytes).as_str()
        )
    );

    let invalid_artifact = crate::main_chat_agent_stage2_readiness::Stage2LiveProviderArtifact {
        schema_version: "stage2-live-provider-evidence-v0".into(),
        commit: artifact_commit,
        required_scenario_count: 9,
        scenario_evidence: complete_stage2_live_provider_evidence_for_tests(
            "openai",
            "gpt-4.1-live",
        ),
    };
    std::fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&invalid_artifact).expect("serialize invalid live artifact"),
    )
    .expect("write invalid live artifact");

    let invalid = read_or_run_stage2_live_provider_summary_with_artifact_path_for_tests(
        &state,
        false,
        &artifact_path,
    )
    .await
    .expect("loaded invalid live provider artifact");
    let _ = std::fs::remove_file(&artifact_path);

    assert!(invalid.attempted);
    assert!(!invalid.ready);
    assert!(invalid
        .blockers
        .contains(&"stage2_live_artifact_schema_invalid".to_string()));
    assert!(invalid
        .blockers
        .contains(&"stage2_live_artifact_required_scenario_count_invalid".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_artifact_rejects_unknown_build_commit() {
    let artifact_path = std::env::temp_dir().join(format!(
        "openlife-stage2-unknown-live-artifact-{}.json",
        uuid::Uuid::new_v4()
    ));
    let artifact = crate::main_chat_agent_stage2_readiness::Stage2LiveProviderArtifact {
        schema_version: "stage2-live-provider-evidence-v1".into(),
        commit: "unknown".into(),
        required_scenario_count: 10,
        scenario_evidence: complete_stage2_live_provider_evidence_for_tests(
            "openai",
            "gpt-4.1-live",
        ),
    };
    let bytes = serde_json::to_vec_pretty(&artifact).expect("serialize unknown live artifact");
    std::fs::write(&artifact_path, bytes).expect("write unknown live artifact");

    let summary = read_stage2_live_provider_artifact_from_path_with_expected_commit_for_tests(
        &artifact_path,
        None,
    );
    let _ = std::fs::remove_file(&artifact_path);

    assert!(summary.attempted);
    assert!(!summary.ready);
    assert!(summary
        .blockers
        .contains(&"stage2_live_artifact_commit_missing".to_string()));
}

#[test]
fn main_chat_stage2_live_provider_artifact_rejects_stale_build_commit() {
    let artifact_path = std::env::temp_dir().join(format!(
        "openlife-stage2-stale-live-artifact-{}.json",
        uuid::Uuid::new_v4()
    ));
    let artifact = crate::main_chat_agent_stage2_readiness::Stage2LiveProviderArtifact {
        schema_version: "stage2-live-provider-evidence-v1".into(),
        commit: "old-build".into(),
        required_scenario_count: 10,
        scenario_evidence: complete_stage2_live_provider_evidence_for_tests(
            "openai",
            "gpt-4.1-live",
        ),
    };
    let bytes = serde_json::to_vec_pretty(&artifact).expect("serialize stale live artifact");
    std::fs::write(&artifact_path, bytes).expect("write stale live artifact");

    let summary = read_stage2_live_provider_artifact_from_path_with_expected_commit_for_tests(
        &artifact_path,
        Some("current-build"),
    );
    let _ = std::fs::remove_file(&artifact_path);

    assert!(summary.attempted);
    assert!(!summary.ready);
    assert!(summary
        .blockers
        .contains(&"stage2_live_artifact_current_commit_mismatch".to_string()));
}

#[tokio::test]
async fn main_chat_stage2_failure_recovery_reports_required_r2_matrix() {
    let coverage = collect_stage2_failure_recovery_coverage_for_tests().await;

    assert!(coverage.ready, "{:?}", coverage.blockers);
    assert_eq!(coverage.required_count, 10);
    assert_eq!(coverage.passed_count, 10);
    for id in [
        "R2-01", "R2-02", "R2-03", "R2-04", "R2-05", "R2-06", "R2-07", "R2-08", "R2-09", "R2-10",
    ] {
        assert!(
            coverage
                .coverage
                .iter()
                .any(|row| row.id == id && row.passed),
            "missing recovery row {id}: {:?}",
            coverage.coverage
        );
    }
}

#[tokio::test]
async fn main_chat_stage2_failure_recovery_r2_01_uses_missing_source_blocker() {
    let coverage = collect_stage2_failure_recovery_coverage_for_tests().await;
    let r2_01 = coverage
        .coverage
        .iter()
        .find(|row| row.id == "R2-01")
        .expect("R2-01 recovery row");

    assert!(
        r2_01
            .evidence
            .iter()
            .any(|evidence| evidence == "missing_workspace_file_blocker"),
        "R2-01 must prove missing-source blocker evidence, got evidence={:?} blockers={:?}",
        r2_01.evidence,
        r2_01.blockers
    );
    assert!(
        !r2_01
            .evidence
            .iter()
            .any(|evidence| evidence.contains("success")),
        "R2-01 must not be credited from file-read success evidence: {:?}",
        r2_01.evidence
    );
}

#[tokio::test]
async fn main_chat_stage2_failure_recovery_r2_04_uses_disallowed_tool_blocker() {
    let coverage = collect_stage2_failure_recovery_coverage_for_tests().await;
    let r2_04 = coverage
        .coverage
        .iter()
        .find(|row| row.id == "R2-04")
        .expect("R2-04 recovery row");

    for required in [
        "model_selected_disallowed_tool_blocker",
        "no_single_step_fallback",
        "no_direct_write",
    ] {
        assert!(
            r2_04.evidence.iter().any(|evidence| evidence == required),
            "R2-04 must prove {required}, got evidence={:?} blockers={:?}",
            r2_04.evidence,
            r2_04.blockers
        );
    }
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_only_ready_with_manual_and_external_live_p0_evidence() {
    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(
        Stage2ReadinessTestInputs::fully_ready_for_tests(
            complete_manual_records(),
            complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live"),
        ),
    )
    .await
    .expect("stage 2 ready report");

    assert_eq!(report.recommendation, "ready_for_limited_internal_trial");
    assert_eq!(
        report.implementation_status,
        "ready_for_limited_internal_trial"
    );
    assert!(report.blockers.is_empty(), "{:?}", report.blockers);
    assert_eq!(report.safety.silent_durable_write_count, 0);
    assert_eq!(report.safety.hidden_legacy_fallback_count, 0);
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_requires_known_report_commit_for_ready_recommendation() {
    let mut inputs = Stage2ReadinessTestInputs::fully_ready_for_tests(
        complete_manual_records(),
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live"),
    );
    inputs.inject_report_commit_for_tests("unknown");

    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(inputs)
        .await
        .expect("stage 2 report with unknown commit");

    assert_eq!(report.commit, "unknown");
    assert_eq!(
        report.recommendation,
        "not_ready_for_limited_internal_trial"
    );
    assert_eq!(
        report.implementation_status,
        "implementation_complete_for_stage2_mechanism"
    );
    assert!(
        report
            .blockers
            .contains(&"stage2_readiness_commit_missing".to_string()),
        "unknown report commit must be a readiness blocker: {:?}",
        report.blockers
    );

    let mut mock_commit_inputs = Stage2ReadinessTestInputs::fully_ready_for_tests(
        complete_manual_records(),
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live"),
    );
    mock_commit_inputs.inject_report_commit_for_tests("mock-build");
    let mock_commit_report =
        run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(mock_commit_inputs)
            .await
            .expect("stage 2 report with mock commit");

    assert_eq!(mock_commit_report.commit, "mock-build");
    assert_eq!(
        mock_commit_report.recommendation,
        "not_ready_for_limited_internal_trial"
    );
    assert!(
        mock_commit_report
            .blockers
            .contains(&"stage2_readiness_commit_missing".to_string()),
        "mock report commit must be a readiness blocker: {:?}",
        mock_commit_report.blockers
    );

    let mut private_network_commit_inputs = Stage2ReadinessTestInputs::fully_ready_for_tests(
        complete_manual_records(),
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live"),
    );
    private_network_commit_inputs.inject_report_commit_for_tests("private-network-build");
    let private_network_commit_report =
        run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(
            private_network_commit_inputs,
        )
        .await
        .expect("stage 2 report with private-network commit");

    assert_eq!(
        private_network_commit_report.commit,
        "private-network-build"
    );
    assert_eq!(
        private_network_commit_report.recommendation,
        "not_ready_for_limited_internal_trial"
    );
    assert!(
        private_network_commit_report
            .blockers
            .contains(&"stage2_readiness_commit_missing".to_string()),
        "private-network report commit must be a readiness blocker: {:?}",
        private_network_commit_report.blockers
    );
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_counts_live_write_and_legacy_safety_flags() {
    let mut live_evidence =
        complete_stage2_live_provider_evidence_for_tests("openai", "gpt-4.1-live");
    live_evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L04")
        .expect("L2-L04 evidence")
        .direct_writes_executed = true;
    live_evidence
        .iter_mut()
        .find(|row| row.scenario_id == "L2-L05")
        .expect("L2-L05 evidence")
        .legacy_fallback_used = true;

    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(
        Stage2ReadinessTestInputs::fully_ready_for_tests(complete_manual_records(), live_evidence),
    )
    .await
    .expect("stage 2 safety report");

    assert_eq!(
        report.recommendation,
        "not_ready_for_limited_internal_trial"
    );
    assert_eq!(report.safety.silent_durable_write_count, 1);
    assert_eq!(report.safety.hidden_legacy_fallback_count, 1);
    assert!(report
        .blockers
        .contains(&"stage2_silent_durable_write_detected".to_string()));
    assert!(report
        .blockers
        .contains(&"stage2_hidden_legacy_fallback_detected".to_string()));
}

#[tokio::test]
async fn main_chat_agent_stage2_readiness_fake_live_evidence_keeps_mechanism_incomplete() {
    let report = run_main_chat_agent_stage2_readiness_report_with_inputs_for_tests(
        Stage2ReadinessTestInputs::fully_ready_for_tests(
            complete_manual_records(),
            complete_stage2_live_provider_evidence_for_tests("local_test_http", "gpt-local"),
        ),
    )
    .await
    .expect("stage 2 fake live safety report");

    assert_eq!(
        report.recommendation,
        "not_ready_for_limited_internal_trial"
    );
    assert_eq!(
        report.implementation_status,
        "implementation_incomplete_for_stage2_mechanism"
    );
    assert_eq!(report.safety.fake_live_evidence_count, 10);
    assert!(report
        .blockers
        .contains(&"stage2_fake_live_evidence_detected".to_string()));
}
