use crate::main_chat_capability_eval::{
    run_main_chat_capability_eval_case, run_main_chat_capability_eval_report,
    MainChatCapabilityEvalCaseReport, MainChatCapabilityEvalCaseStatus,
    MainChatCapabilityEvalFixtureMode, MainChatCapabilityEvalScenario,
};

fn case<'a>(
    report: &'a crate::main_chat_capability_eval::MainChatCapabilityEvalReport,
    scenario_id: &str,
) -> &'a MainChatCapabilityEvalCaseReport {
    report
        .cases
        .iter()
        .find(|case| case.scenario_id == scenario_id)
        .unwrap_or_else(|| panic!("missing capability eval case {scenario_id}"))
}

#[tokio::test]
async fn main_chat_capability_eval_report_credits_first_cf_scenarios() {
    let report = run_main_chat_capability_eval_report().await;

    assert_eq!(report.report_kind, "main_chat_capability_eval");
    assert_eq!(report.total_case_count, 4);
    assert_eq!(report.passed_case_count, 4, "{report:#?}");
    assert_eq!(report.blocked_case_count, 0, "{report:#?}");
    assert_eq!(report.failed_case_count, 0, "{report:#?}");
    assert!(report.local_deterministic_ready, "{report:#?}");
    assert!(!report.allow_writes);
    assert!(!report.live_provider_required);
    assert!(report.stream_coverage_reused_from_command_surface_gate);
    assert_eq!(report.legacy_fallback_count, 0);
    assert_eq!(report.silent_write_count, 0);
    assert_eq!(report.direct_durable_write_count, 0);
    assert_eq!(report.fake_observation_count, 0);
    assert_eq!(report.live_only_proof_count, 0);

    for scenario_id in ["CF-DIRECT-01", "CF-FILE-01", "CF-WEB-01", "CF-MCP-01"] {
        assert_eq!(
            case(&report, scenario_id).status,
            MainChatCapabilityEvalCaseStatus::Passed,
            "{scenario_id} should pass with typed evidence"
        );
    }
}

#[tokio::test]
async fn main_chat_capability_eval_direct_requires_provider_scheduler_trace_and_no_tools() {
    let case = run_main_chat_capability_eval_case(
        MainChatCapabilityEvalScenario::CfDirect01,
        MainChatCapabilityEvalFixtureMode::Default,
    )
    .await;

    assert_eq!(
        case.status,
        MainChatCapabilityEvalCaseStatus::Passed,
        "{case:#?}"
    );
    assert_eq!(case.actual_route.as_deref(), Some("direct_answer"));
    assert!(case.route_decision_observed);
    assert!(case.deterministic_route_used);
    assert!(case.advisory_route_trace_only);
    assert!(case.generation_result_observed);
    assert!(case.provider_scheduler_trace_observed);
    assert!(case.final_assistant_delivery_observed);
    assert_eq!(case.tool_action_count, 0);
    assert_eq!(case.proposal_record_count, 0);
    assert!(!case.legacy_fallback_used);
    assert!(!case.silent_write_detected);
    assert!(!case.direct_durable_write_detected);
    assert!(!case.live_only_proof_used);
    assert!(case
        .evidence
        .contains(&"no_tool_action_or_tool_call".to_string()));
}

#[tokio::test]
async fn main_chat_capability_eval_file_read_uses_real_file_observation_and_synthesis() {
    let case = run_main_chat_capability_eval_case(
        MainChatCapabilityEvalScenario::CfFile01,
        MainChatCapabilityEvalFixtureMode::Default,
    )
    .await;

    assert_eq!(
        case.status,
        MainChatCapabilityEvalCaseStatus::Passed,
        "{case:#?}"
    );
    assert_eq!(case.actual_route.as_deref(), Some("react_tool_execution"));
    assert_eq!(
        case.read_execution_kind.as_deref(),
        Some("file_system_read")
    );
    assert_eq!(case.read_source_kind.as_deref(), Some("file"));
    assert_eq!(case.read_real_read_only_execution, Some(true));
    assert_eq!(case.read_fixture_backed, Some(false));
    assert!(case.final_assistant_delivery_observed);
    assert!(case
        .evidence
        .iter()
        .any(|evidence| evidence == "file.read_final_synthesis_generation"));
    assert!(!case.fake_observation_detected);
    assert!(!case.direct_durable_write_detected);
}

#[tokio::test]
async fn main_chat_capability_eval_web_read_is_fixture_backed_not_fake_live_web() {
    let case = run_main_chat_capability_eval_case(
        MainChatCapabilityEvalScenario::CfWeb01,
        MainChatCapabilityEvalFixtureMode::Default,
    )
    .await;

    assert_eq!(
        case.status,
        MainChatCapabilityEvalCaseStatus::Passed,
        "{case:#?}"
    );
    assert_eq!(case.actual_route.as_deref(), Some("react_tool_execution"));
    assert_eq!(case.network_policy_enabled, Some(true));
    assert_eq!(
        case.read_execution_kind.as_deref(),
        Some("web_search_fixture")
    );
    assert_eq!(case.read_source_kind.as_deref(), Some("web"));
    assert_eq!(case.read_real_read_only_execution, Some(false));
    assert_eq!(case.read_fixture_backed, Some(true));
    assert!(case
        .evidence
        .contains(&"web_network_policy_enabled".to_string()));
    assert!(!case.live_only_proof_used);
    assert!(!case.fake_observation_detected);
}

#[tokio::test]
async fn main_chat_capability_eval_mcp_read_uses_registered_read_only_fixture() {
    let case = run_main_chat_capability_eval_case(
        MainChatCapabilityEvalScenario::CfMcp01,
        MainChatCapabilityEvalFixtureMode::Default,
    )
    .await;

    assert_eq!(
        case.status,
        MainChatCapabilityEvalCaseStatus::Passed,
        "{case:#?}"
    );
    assert_eq!(case.actual_route.as_deref(), Some("react_tool_execution"));
    assert_eq!(
        case.read_execution_kind.as_deref(),
        Some("registered_mcp_read")
    );
    assert_eq!(case.read_source_kind.as_deref(), Some("mcp"));
    assert_eq!(case.read_real_read_only_execution, Some(true));
    assert_eq!(case.read_fixture_backed, Some(false));
    assert!(case
        .evidence
        .contains(&"registered_mcp_target_resolved".to_string()));
    assert!(!case.fake_observation_detected);
    assert!(!case.direct_durable_write_detected);
}

#[tokio::test]
async fn main_chat_capability_eval_mcp_missing_fixture_returns_structured_blocker() {
    let case = run_main_chat_capability_eval_case(
        MainChatCapabilityEvalScenario::CfMcp01,
        MainChatCapabilityEvalFixtureMode::MissingMcpFixture,
    )
    .await;

    assert_eq!(
        case.status,
        MainChatCapabilityEvalCaseStatus::Blocked,
        "{case:#?}"
    );
    assert_eq!(
        case.structured_blocker.as_deref(),
        Some("cf_mcp_fixture_unavailable")
    );
    assert!(case
        .blockers
        .contains(&"expected_blocker:cf_mcp_fixture_unavailable".to_string()));
    assert!(!case.legacy_fallback_used);
    assert!(!case.silent_write_detected);
    assert!(!case.direct_durable_write_detected);
    assert!(!case.fake_observation_detected);
    assert!(!case.live_only_proof_used);
}
