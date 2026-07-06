use super::*;
use crate::main_chat_command_surface_eval::MainChatCommandSurfaceEvalReport;
use crate::main_chat_final_gate::{
    self, main_chat_live_provider_acceptance_evidence, main_chat_live_provider_report_blockers,
    MainChatAgentExecutionV1FinalGateReport, MainChatLiveProviderEvalHarnessReport,
    MainChatLiveProviderEvalHarnessScenario,
};
use crate::main_chat_live_provider_harness::{
    run_main_chat_live_provider_eval_harness, MainChatLiveProviderEvalHarnessInput,
};
use crate::main_chat_step6_product_acceptance::{
    build_step6_product_acceptance_report_for_tests, clean_step6_final_gate_summary_for_tests,
    step6_browser_report_for_tests, step6_observed_journey_for_tests,
};
use std::sync::Arc;

#[test]
fn main_chat_final_acceptance_helpers_are_not_concentrated_in_lib_rs() {
    let lib_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");

    assert!(
        !source.contains("\n    async fn run_main_chat_agent_execution_v1_final_acceptance_gate("),
        "final acceptance test runner helpers should live outside src/lib.rs"
    );
    assert!(
        !source.contains("\n    async fn run_main_chat_command_surface_eval_gate("),
        "command-surface acceptance runner helpers should live outside src/lib.rs"
    );
    assert!(
        !source.contains(
            "\n    async fn main_chat_final_acceptance_gate_uses_real_command_surface_eval_evidence("
        ),
        "final acceptance test cases should live outside src/lib.rs"
    );
    assert!(
        !source.contains(
            "\n    fn main_chat_live_provider_harness_reports_build_structured_acceptance_evidence("
        ),
        "live-provider acceptance evidence test cases should live outside src/lib.rs"
    );
}

pub(crate) fn successful_live_provider_harness_report(
    scenario: MainChatLiveProviderEvalHarnessScenario,
) -> MainChatLiveProviderEvalHarnessReport {
    main_chat_final_gate::completed_main_chat_live_provider_eval_harness_report(
        scenario,
        "openai",
        "external_provider",
        format!("live-run-{}", scenario.as_str()),
        format!("live-task-{}", scenario.as_str()),
        "Live provider response.",
    )
}

pub(crate) fn blocked_live_provider_harness_report(
    blocker: &str,
) -> MainChatLiveProviderEvalHarnessReport {
    main_chat_final_gate::blocked_main_chat_live_provider_eval_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
        "openai",
        "external_provider",
        vec![blocker.into()],
        main_chat_final_gate::main_chat_live_provider_required_evidence(),
    )
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum MainChatLiveProviderEvalConfigMode {
    FromEnvironment,
    NoCredentials,
}

pub(crate) async fn run_main_chat_agent_execution_v1_final_acceptance_gate(
    include_live_provider: bool,
) -> MainChatAgentExecutionV1FinalGateReport {
    run_main_chat_agent_execution_v1_final_acceptance_gate_with_config_mode(
        include_live_provider,
        MainChatLiveProviderEvalConfigMode::FromEnvironment,
    )
    .await
}

pub(crate) async fn run_main_chat_agent_execution_v1_final_acceptance_gate_with_config_mode(
    include_live_provider: bool,
    live_config_mode: MainChatLiveProviderEvalConfigMode,
) -> MainChatAgentExecutionV1FinalGateReport {
    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let command_surface_report = run_main_chat_command_surface_eval_gate().await;
    let live_reports = if include_live_provider {
        let mut reports = Vec::new();
        for scenario in [
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
        ] {
            let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
            match live_config_mode {
                MainChatLiveProviderEvalConfigMode::FromEnvironment => {
                    configure_live_provider_eval_state(&state).await;
                }
                MainChatLiveProviderEvalConfigMode::NoCredentials => {
                    configure_live_provider_eval_state_without_credentials(&state).await;
                }
            }
            match run_main_chat_live_provider_eval_harness(
                state,
                MainChatLiveProviderEvalHarnessInput {
                    scenario,
                    session_id: format!("final-acceptance-live-{}", scenario.as_str()),
                    prompt: scenario.prompt().into(),
                    explicit_live_eval_requested: true,
                    local_only_required: false,
                },
            )
            .await
            {
                Ok(report) => reports.push(report),
                Err(error) => reports.push(MainChatLiveProviderEvalHarnessReport {
                    scenario,
                    ready: false,
                    status: "failed".into(),
                    provider: String::new(),
                    provider_model: None,
                    provider_endpoint_kind: "error".into(),
                    blockers: vec![error],
                    required_evidence: Vec::new(),
                    live_provider_invocation_allowed: false,
                    main_chat_invoked: false,
                    model_invoked: false,
                    direct_writes_executed: false,
                    legacy_fallback_used: false,
                    agent_loop_succeeded: false,
                    single_step_fallback_used: false,
                    agent_loop_action_status: None,
                    mcp_read_target_resolved: false,
                    tool_permission_proposal_created: false,
                    tool_permission_proposal_target: None,
                    tool_selection_candidate_count: 0,
                    tool_selection_candidate_ids: Vec::new(),
                    tool_selection_allowlist: Vec::new(),
                    tool_selection_allowed_actions: Vec::new(),
                    tool_selection_model_ranked: false,
                    tool_selection_ranking_source: None,
                    tool_selection_ranking_provider: None,
                    tool_selection_ranking_model: None,
                    tool_selection_ranking_route_type: None,
                    tool_selection_ranking_provider_backed: false,
                    tool_selection_model_ranking_ignored: false,
                    tool_selection_model_ranking_candidate_ids: Vec::new(),
                    tool_selection_model_ranking_response_digest: None,
                    model_selected_allowed_tool: false,
                    model_selected_execution_policy_validated: false,
                    model_selected_execution_allowed: false,
                    model_selected_governed_arguments: false,
                    model_selected_governed_arguments_digest: None,
                    model_selected_candidate_id: None,
                    model_selected_candidate_target: None,
                    model_selected_candidate_action_type: None,
                    model_selected_candidate_rank: None,
                    model_selected_candidate_source: None,
                    model_selected_candidate_capabilities_digest: None,
                    model_selected_candidate_capability_labels: None,
                    model_selected_candidate_match_reason: None,
                    run_id: None,
                    task_session_id: None,
                    response_preview: None,
                }),
            }
        }
        reports
    } else {
        Vec::new()
    };
    main_chat_final_gate::build_main_chat_agent_execution_v1_final_gate_report(
        runtime_report,
        command_surface_report.total_cases,
        command_surface_report.acceptance_evidence(),
        include_live_provider,
        live_reports,
    )
}

pub(crate) fn main_chat_agent_execution_v1_final_gate_report_from_parts(
    runtime_report: openlife_core::agent::main_chat_agent_v1::MainChatRuntimeEvalReport,
    command_surface_report: MainChatCommandSurfaceEvalReport,
    live_provider_attempted: bool,
    live_reports: Vec<MainChatLiveProviderEvalHarnessReport>,
) -> MainChatAgentExecutionV1FinalGateReport {
    main_chat_final_gate::build_main_chat_agent_execution_v1_final_gate_report(
        runtime_report,
        command_surface_report.total_cases,
        command_surface_report.acceptance_evidence(),
        live_provider_attempted,
        live_reports,
    )
}

pub(crate) async fn configure_live_provider_eval_state(state: &Arc<AppState>) {
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
}

async fn configure_live_provider_eval_state_without_credentials(state: &Arc<AppState>) {
    {
        let mut config = state.config.lock().await;
        config.llm.provider = "openai".into();
        config.llm.openai_base = "https://api.openai.com/v1".into();
        config.llm.chat_model = "gpt-4o-mini".into();
        config.llm.openai_key.clear();
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
            String::new(),
            config.llm.chat_model.clone(),
            config.llm.embedding_model.clone(),
            false,
        );
    }
}

pub(crate) async fn configure_live_provider_eval_state_with_local_http_provider(
    state: &Arc<AppState>,
    reply: &'static str,
) {
    let provider_base = fake_local_chat_provider_endpoint(reply).await;
    {
        let mut config = state.config.lock().await;
        config.llm.provider = "openai".into();
        config.llm.openai_base = provider_base.clone();
        config.llm.chat_model = "gpt-local-provider-harness".into();
        config.llm.openai_key = "test-key".into();
        config.system.network_policy.enabled = true;
    }
    {
        let config = state.config.lock().await.clone();
        let mut scheduler = state.scheduler.lock().await;
        *scheduler = openlife_core::scheduler::InferenceScheduler::new(
            config.local_model.clone(),
            false,
            config.llm.provider.clone(),
            provider_base,
            config.llm.openai_key.clone(),
            config.llm.chat_model.clone(),
            config.llm.embedding_model.clone(),
            false,
        );
    }
}

async fn fake_local_chat_provider_endpoint(reply: &'static str) -> String {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind local fake chat provider");
    let addr = listener.local_addr().expect("local fake provider addr");
    std::thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut handled = 0usize;
        while handled < 8 && std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    handled += 1;
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
                    let mut buffer = [0u8; 8192];
                    let _ = std::io::Read::read(&mut stream, &mut buffer);
                    let body = serde_json::json!({
                        "id": "chatcmpl-main-chat-live-provider-local",
                        "object": "chat.completion",
                        "choices": [{
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": reply
                            },
                            "finish_reason": "stop"
                        }]
                    })
                    .to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    format!("http://{addr}/v1")
}

pub(crate) async fn run_main_chat_command_surface_eval_gate() -> MainChatCommandSurfaceEvalReport {
    main_chat_command_surface_eval::run_main_chat_command_surface_eval_report().await
}

fn complete_local_kernel_command_surface_report() -> MainChatCommandSurfaceEvalReport {
    MainChatCommandSurfaceEvalReport {
        total_cases: 38,
        failed_cases: 0,
        send_coverage: 0.5,
        stream_coverage: 0.5,
        provider_generation_coverage: 1.0 / 38.0,
        file_read_coverage: 1.0 / 38.0,
        plan_execute_coverage: 1.0 / 38.0,
        proposal_coverage: 1.0 / 38.0,
        web_policy_blocker_coverage: 1.0 / 38.0,
        web_agent_loop_blocker_coverage: 1.0 / 38.0,
        web_agent_loop_success_coverage: 1.0 / 38.0,
        mcp_missing_read_target_blocker_coverage: 1.0 / 38.0,
        mcp_registered_read_success_coverage: 1.0 / 38.0,
        mcp_agent_loop_success_coverage: 1.0 / 38.0,
        mcp_tool_permission_proposal_coverage: 1.0 / 38.0,
        mcp_agent_loop_tool_permission_proposal_coverage: 1.0 / 38.0,
        final_completion_ready: false,
        final_completion_blockers: vec![
            "live_provider_generation_not_executed".into(),
            "provider_backed_web_mcp_agent_loop_not_executed".into(),
            "provider_backed_web_agent_loop_not_executed".into(),
            "provider_backed_mcp_agent_loop_not_executed".into(),
            "provider_live_proposal_permission_not_executed".into(),
        ],
        kernel_backed_case_count: 38,
        kernel_direct_answer_case_count: 8,
        kernel_read_only_tool_case_count: 18,
        kernel_proposal_write_case_count: 4,
        kernel_plan_execute_case_count: 4,
        kernel_blocker_case_count: 10,
        kernel_hs_context_case_count: 38,
        kernel_web_tool_case_count: 6,
        kernel_mcp_tool_case_count: 8,
        ..Default::default()
    }
}

#[tokio::test]
async fn main_chat_final_acceptance_gate_uses_real_command_surface_eval_evidence() {
    let command_surface_report = run_main_chat_command_surface_eval_gate().await;
    assert_eq!(
        command_surface_report.failed_cases, 0,
        "{:?}",
        command_surface_report.failures
    );

    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let report =
        openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_agent_execution_v1_acceptance_gate(
            openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceInput {
                runtime_report,
                command_surface: command_surface_report.acceptance_evidence(),
                live_provider:
                    openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceLiveEvidence {
                        generation_eval_executed: false,
                        web_mcp_agent_loop_eval_executed: false,
                        web_agent_loop_eval_executed: false,
                        mcp_agent_loop_eval_executed: false,
                        proposal_permission_eval_executed: false,
                        no_silent_writes: true,
                    },
            },
        );

    assert!(!report.ready);
    assert_eq!(report.status, "blocked");
    assert!(!report.command_surface_gate_ready);
    assert!(!report.live_provider_gate_ready);
    assert!(!report.direct_writes_executed);
    assert!(report
        .blockers
        .contains(&"command_surface_final_completion_not_ready".to_string()));
    assert!(report
        .blockers
        .contains(&"live_provider_generation_not_executed".to_string()));
}

#[tokio::test]
async fn main_chat_final_acceptance_gate_runner_fails_closed_without_live_provider_opt_in() {
    let report = run_main_chat_agent_execution_v1_final_acceptance_gate(false).await;

    assert_eq!(report.runtime_total_cases, 100);
    assert_eq!(report.command_surface_total_cases, 38);
    assert!(!report.live_provider_attempted);
    assert_eq!(report.live_provider_report_count, 0);
    assert_eq!(report.live_provider_ready_count, 0);
    assert!(!report.live_provider_direct_writes_executed);
    assert!(report
        .live_provider_blockers
        .contains(&"live_provider_generation_not_executed".to_string()));
    assert!(report
        .live_provider_blockers
        .contains(&"provider_backed_web_agent_loop_not_executed".to_string()));
    assert!(report
        .live_provider_blockers
        .contains(&"provider_backed_mcp_agent_loop_not_executed".to_string()));
    assert!(report
        .live_provider_blockers
        .contains(&"provider_live_proposal_permission_not_executed".to_string()));
    assert!(!report.acceptance.ready);
    assert_eq!(report.acceptance.status, "blocked");
    assert!(!report.acceptance.runtime_gate_ready);
    assert!(!report.acceptance.command_surface_gate_ready);
    assert!(!report.acceptance.live_provider_gate_ready);
    assert!(!report.acceptance.direct_writes_executed);
    assert!(report
        .acceptance
        .blockers
        .contains(&"runtime_eval_final_completion_not_ready".to_string()));
    assert!(report
        .acceptance
        .blockers
        .contains(&"command_surface_final_completion_not_ready".to_string()));
    assert!(report
        .acceptance
        .blockers
        .contains(&"live_provider_generation_not_executed".to_string()));
    assert!(report
        .acceptance
        .blockers
        .contains(&"provider_backed_web_agent_loop_not_executed".to_string()));
    assert!(report
        .acceptance
        .blockers
        .contains(&"provider_backed_mcp_agent_loop_not_executed".to_string()));
    assert!(report
        .acceptance
        .blockers
        .contains(&"provider_live_proposal_permission_not_executed".to_string()));
}

#[test]
fn main_chat_final_acceptance_gate_accepts_complete_live_evidence_overlaying_local_gates() {
    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let live_provider = main_chat_live_provider_acceptance_evidence(&[
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
        ),
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        ),
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
        ),
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
        ),
    ]);
    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_report_with_live_provider_evidence(
            runtime_report,
            &live_provider,
        );
    let command_surface_report = complete_local_kernel_command_surface_report();

    let report = openlife_core::agent::main_chat_agent_v1::evaluate_main_chat_agent_execution_v1_acceptance_gate(
        openlife_core::agent::main_chat_agent_v1::MainChatAgentExecutionV1AcceptanceInput {
            runtime_report,
            command_surface: command_surface_report
                .acceptance_evidence_with_live_provider(&live_provider),
            live_provider,
        },
    );

    assert!(report.ready, "{:?}", report.blockers);
    assert!(report.runtime_gate_ready);
    assert!(report.command_surface_gate_ready);
    assert!(report.live_provider_gate_ready);
    assert!(!report.direct_writes_executed);
}

#[test]
fn main_chat_final_acceptance_step6_report_accepts_full_structured_product_evidence() {
    let observed = [
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
    .into_iter()
    .map(step6_observed_journey_for_tests)
    .collect::<Vec<_>>();
    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(observed)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(report.overall_ready, "{:?}", report.blockers);
    assert!(report.local_deterministic_ready);
    assert!(report.external_live_ready);
    assert_eq!(report.passed_journey_count, 11);
    assert_eq!(report.blocked_live_journey_count, 0);
    assert!(report.no_silent_durable_write);
    assert!(report.no_hidden_legacy_fallback);
    assert!(report.no_local_evidence_credited_as_external_live);
    assert!(report.no_invented_unavailable_evidence);
    assert!(report.ui_status_from_structured_evidence);
}

#[test]
fn main_chat_final_acceptance_step6_report_rejects_blocked_or_fake_live_credit() {
    let mut observed = [
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
    .into_iter()
    .map(step6_observed_journey_for_tests)
    .collect::<Vec<_>>();
    let live_web = observed
        .iter_mut()
        .find(|row| row.journey_id == "S6-LIVE-WEB")
        .expect("S6-LIVE-WEB");
    live_web.external_live_status = "blocked_live_evidence".into();
    live_web.external_live_provider_kind = None;
    live_web.observed_via = "blocked_live_evidence_report".into();
    live_web.task_session_id.clear();
    live_web.run_id.clear();
    live_web.blockers = vec!["explicit_live_eval_required".into()];

    let live_mcp = observed
        .iter_mut()
        .find(|row| row.journey_id == "S6-LIVE-MCP")
        .expect("S6-LIVE-MCP");
    live_mcp.external_live_provider_kind = Some("local_test_http".into());
    live_mcp.local_fixture_credited_as_external_live = true;

    let report = build_step6_product_acceptance_report_for_tests(
        Some(step6_browser_report_for_tests(observed)),
        clean_step6_final_gate_summary_for_tests(),
    );

    assert!(!report.overall_ready);
    assert!(report.local_deterministic_ready);
    assert!(!report.external_live_ready);
    assert_eq!(report.blocked_live_journey_count, 1);
    assert!(!report.no_local_evidence_credited_as_external_live);
    assert!(report
        .blockers
        .contains(&"step6_external_live_journeys_not_all_passed".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_local_fixture_credited_as_live:S6-LIVE-MCP".to_string()));
    assert!(report
        .blockers
        .contains(&"step6_external_provider_missing:S6-LIVE-MCP".to_string()));
}

#[test]
fn main_chat_final_acceptance_gate_report_rejects_live_reports_without_attempt_proof() {
    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let command_surface_report = complete_local_kernel_command_surface_report();

    let report = main_chat_agent_execution_v1_final_gate_report_from_parts(
        runtime_report,
        command_surface_report,
        false,
        vec![
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
            ),
            successful_live_provider_harness_report(
                MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
            ),
        ],
    );

    assert!(!report.live_provider_attempted);
    assert_eq!(report.live_provider_report_count, 4);
    assert_eq!(
        report.live_provider_ready_count, 0,
        "live reports must not receive ready credit unless the runner records a live-provider attempt"
    );
    assert!(report
        .live_provider_blockers
        .contains(&"live_provider_reports_without_attempt".to_string()));
    assert!(!report.acceptance.ready);
    assert!(!report.acceptance.live_provider_gate_ready);
    assert!(report
        .acceptance
        .blockers
        .contains(&"live_provider_generation_not_executed".to_string()));
}

#[test]
fn main_chat_final_acceptance_gate_report_preserves_live_provider_failure_audit() {
    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let command_surface_report = complete_local_kernel_command_surface_report();

    let report = main_chat_agent_execution_v1_final_gate_report_from_parts(
        runtime_report,
        command_surface_report,
        true,
        vec![
            blocked_live_provider_harness_report("provider_api_key_missing"),
            blocked_live_provider_harness_report("network_disabled"),
        ],
    );

    assert!(report.live_provider_attempted);
    assert_eq!(report.live_provider_report_count, 2);
    assert_eq!(report.live_provider_ready_count, 0);
    assert_eq!(report.live_provider_main_chat_invoked_count, 0);
    assert_eq!(report.live_provider_model_invoked_count, 0);
    assert!(!report.live_provider_direct_writes_executed);
    assert!(report
        .live_provider_blockers
        .contains(&"provider_api_key_missing".to_string()));
    assert!(report
        .live_provider_blockers
        .contains(&"network_disabled".to_string()));
    assert_eq!(report.live_provider_scenario_reports.len(), 2);
    assert!(report
        .live_provider_scenario_reports
        .iter()
        .any(|scenario| {
            scenario.scenario == "direct-answer"
                && scenario.status == "blocked"
                && scenario
                    .blockers
                    .contains(&"provider_api_key_missing".to_string())
                && !scenario.main_chat_invoked
                && !scenario.model_invoked
        }));
    assert!(report
        .live_provider_scenario_reports
        .iter()
        .any(|scenario| {
            scenario.scenario == "direct-answer"
                && scenario.status == "blocked"
                && scenario.blockers.contains(&"network_disabled".to_string())
                && !scenario.main_chat_invoked
                && !scenario.model_invoked
        }));
    assert!(!report.acceptance.ready);
}

#[test]
fn main_chat_final_acceptance_gate_report_derives_post_invocation_live_provider_blockers() {
    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let command_surface_report = complete_local_kernel_command_surface_report();
    let mut failed_web = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    failed_web.ready = false;
    failed_web.status = "failed".into();
    failed_web.blockers.clear();
    failed_web.agent_loop_succeeded = false;
    failed_web.agent_loop_action_status = None;

    let report = main_chat_agent_execution_v1_final_gate_report_from_parts(
        runtime_report,
        command_surface_report,
        true,
        vec![failed_web],
    );

    assert!(report.live_provider_attempted);
    assert_eq!(report.live_provider_report_count, 1);
    assert_eq!(report.live_provider_ready_count, 0);
    assert!(report.live_provider_main_chat_invoked_count > 0);
    assert!(report
        .live_provider_blockers
        .contains(&"live_provider_web_agent_loop_not_completed".to_string()));
    assert!(!report.acceptance.ready);
}

#[test]
fn main_chat_final_acceptance_gate_report_counts_only_auditable_live_ready_reports() {
    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let command_surface_report = complete_local_kernel_command_surface_report();
    let mut untraceable_ready = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    untraceable_ready.response_preview = None;

    let report = main_chat_agent_execution_v1_final_gate_report_from_parts(
        runtime_report,
        command_surface_report,
        true,
        vec![untraceable_ready],
    );

    assert_eq!(report.live_provider_report_count, 1);
    assert_eq!(
        report.live_provider_ready_count, 0,
        "uncreditable ready claims must not inflate the final live-provider ready summary"
    );
    assert!(report
        .live_provider_blockers
        .contains(&"live_provider_trace_missing".to_string()));
    assert!(!report.acceptance.ready);
}

#[test]
fn main_chat_final_acceptance_gate_rejects_local_synthetic_live_ready_credit() {
    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let command_surface_report = complete_local_kernel_command_surface_report();

    let mut uncreditable_reports = Vec::new();
    for scenario in [
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    ] {
        for provider in [
            "local",
            "scripted",
            "fixture",
            "synthetic",
            "openai127-0-0-1",
        ] {
            let mut report = successful_live_provider_harness_report(scenario);
            report.provider = provider.into();
            uncreditable_reports.push(report);
        }

        let mut local_http_endpoint = successful_live_provider_harness_report(scenario);
        local_http_endpoint.provider_endpoint_kind = "local_test_http".into();
        uncreditable_reports.push(local_http_endpoint);
    }

    let evidence = main_chat_live_provider_acceptance_evidence(&uncreditable_reports);
    assert!(!evidence.generation_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);
    assert!(evidence.no_silent_writes);

    let report = main_chat_agent_execution_v1_final_gate_report_from_parts(
        runtime_report,
        command_surface_report,
        true,
        uncreditable_reports,
    );

    assert_eq!(
        report.live_provider_ready_count, 0,
        "local, scripted, fixture, synthetic, loopback, and local-test HTTP reports must not count as external live ready evidence"
    );
    assert!(report
        .live_provider_blockers
        .contains(&"live_provider_external_provider_missing".to_string()));
    assert!(report
        .live_provider_scenario_reports
        .iter()
        .all(|scenario| !scenario.credited));
    assert!(!report.acceptance.live_provider_gate_ready);
}

#[test]
fn main_chat_live_provider_report_blockers_rejects_inconsistent_ready_report() {
    let mut inconsistent = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    inconsistent.direct_writes_executed = true;
    let detected_legacy_usage = true;
    inconsistent.legacy_fallback_used = detected_legacy_usage;
    inconsistent.response_preview = None;

    let blockers = main_chat_live_provider_report_blockers(&inconsistent);

    assert!(blockers.contains(&"live_provider_direct_writes_detected".to_string()));
    assert!(blockers.contains(&"live_provider_legacy_fallback_detected".to_string()));
    assert!(blockers.contains(&"live_provider_trace_missing".to_string()));
}

#[test]
fn main_chat_live_provider_report_blockers_rejects_completed_report_without_invocation_proof() {
    let mut invocation_not_allowed = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    invocation_not_allowed.live_provider_invocation_allowed = false;

    let mut main_chat_not_invoked = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    main_chat_not_invoked.main_chat_invoked = false;

    let mut model_not_invoked = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    model_not_invoked.model_invoked = false;

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        invocation_not_allowed.clone(),
        main_chat_not_invoked.clone(),
        model_not_invoked.clone(),
    ]);
    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&invocation_not_allowed);
    assert!(blockers.contains(&"live_provider_invocation_not_allowed".to_string()));

    let blockers = main_chat_live_provider_report_blockers(&main_chat_not_invoked);
    assert!(blockers.contains(&"live_provider_main_chat_not_invoked".to_string()));

    let blockers = main_chat_live_provider_report_blockers(&model_not_invoked);
    assert!(blockers.contains(&"live_provider_model_not_invoked".to_string()));
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_direct_answer_generation_trace() {
    let mut fallback_direct_answer = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    fallback_direct_answer.single_step_fallback_used = true;
    fallback_direct_answer.agent_loop_action_status = Some("succeeded".into());
    fallback_direct_answer.tool_selection_candidate_count = 1;
    fallback_direct_answer.tool_selection_candidate_ids = vec!["web.search".into()];

    let evidence = main_chat_live_provider_acceptance_evidence(&[fallback_direct_answer.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&fallback_direct_answer);
    assert!(
        blockers.contains(&"live_provider_generation_not_completed".to_string()),
        "DirectAnswer live credit must prove direct provider generation rather than fallback/tool execution metadata"
    );
}

#[tokio::test]
async fn main_chat_final_acceptance_gate_runner_reports_live_preflight_blockers_without_invocation()
{
    let report = run_main_chat_agent_execution_v1_final_acceptance_gate_with_config_mode(
        true,
        MainChatLiveProviderEvalConfigMode::NoCredentials,
    )
    .await;

    assert!(report.live_provider_attempted);
    assert_eq!(report.live_provider_report_count, 4);
    assert_eq!(report.live_provider_ready_count, 0);
    assert_eq!(report.live_provider_main_chat_invoked_count, 0);
    assert_eq!(report.live_provider_model_invoked_count, 0);
    assert!(!report.live_provider_direct_writes_executed);
    assert!(report
        .live_provider_blockers
        .contains(&"provider_api_key_missing".to_string()));
    assert!(!report.acceptance.ready);
    assert!(!report.acceptance.live_provider_gate_ready);
    assert!(report
        .acceptance
        .blockers
        .contains(&"live_provider_generation_not_executed".to_string()));
}

#[tokio::test]
#[ignore = "requires OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1, network, and a real external provider API key"]
async fn main_chat_final_acceptance_gate_runner_accepts_external_live_provider_when_opted_in() {
    let report = run_main_chat_agent_execution_v1_final_acceptance_gate_with_config_mode(
        true,
        MainChatLiveProviderEvalConfigMode::FromEnvironment,
    )
    .await;
    let live_audit = serde_json::to_string_pretty(&serde_json::json!({
        "liveProviderBlockers": report.live_provider_blockers,
        "liveProviderScenarioReports": report.live_provider_scenario_reports,
        "acceptanceBlockers": report.acceptance.blockers,
    }))
    .unwrap_or_else(|error| format!("serialize live audit failed: {error}"));

    assert_eq!(report.live_provider_report_count, 4, "{live_audit}");
    assert_eq!(report.live_provider_ready_count, 4, "{live_audit}");
    assert_eq!(
        report.live_provider_main_chat_invoked_count, 4,
        "{live_audit}"
    );
    assert_eq!(report.live_provider_model_invoked_count, 4, "{live_audit}");
    assert!(!report.live_provider_direct_writes_executed, "{live_audit}");
    assert!(report.live_provider_blockers.is_empty(), "{live_audit}");
    assert!(report.acceptance.ready, "{live_audit}");
    assert!(report.acceptance.live_provider_gate_ready, "{live_audit}");
    assert!(report.acceptance.command_surface_gate_ready, "{live_audit}");
    assert!(report.acceptance.runtime_gate_ready, "{live_audit}");
}

#[test]
fn main_chat_live_provider_harness_reports_build_structured_acceptance_evidence() {
    let complete = main_chat_live_provider_acceptance_evidence(&[
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
        ),
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        ),
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
        ),
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
        ),
    ]);

    assert!(complete.generation_eval_executed);
    assert!(complete.web_mcp_agent_loop_eval_executed);
    assert!(complete.web_agent_loop_eval_executed);
    assert!(complete.mcp_agent_loop_eval_executed);
    assert!(complete.proposal_permission_eval_executed);
    assert!(complete.no_silent_writes);

    let missing_mcp = main_chat_live_provider_acceptance_evidence(&[
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
        ),
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
        ),
        successful_live_provider_harness_report(
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
        ),
    ]);

    assert!(missing_mcp.web_agent_loop_eval_executed);
    assert!(!missing_mcp.mcp_agent_loop_eval_executed);
    assert!(!missing_mcp.web_mcp_agent_loop_eval_executed);
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_matching_scenario_identity() {
    let mut mislabeled_web = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    mislabeled_web.scenario = MainChatLiveProviderEvalHarnessScenario::DirectAnswer;
    let evidence = main_chat_live_provider_acceptance_evidence(&[mislabeled_web]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_traceable_run_task_for_all_live_scenarios() {
    let mut missing_web_run = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    missing_web_run.run_id = None;

    let mut missing_mcp_task = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    missing_mcp_task.task_session_id = None;

    let mut empty_proposal_preview = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    empty_proposal_preview.response_preview = Some("   ".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        missing_web_run,
        missing_mcp_task,
        empty_proposal_preview,
    ]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_unbounded_response_preview_trace() {
    let mut raw_response_trace = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    raw_response_trace.response_preview = Some("x".repeat(241));

    let evidence = main_chat_live_provider_acceptance_evidence(&[raw_response_trace.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&raw_response_trace);
    assert!(
        blockers.contains(&"live_provider_trace_missing".to_string()),
        "external live evidence must preserve a bounded response preview trace"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_control_char_response_preview_trace() {
    let mut multiline_response_trace = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    multiline_response_trace.response_preview = Some("Live provider\nresponse.".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[multiline_response_trace.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&multiline_response_trace);
    assert!(
        blockers.contains(&"live_provider_trace_missing".to_string()),
        "external live evidence must preserve a bounded single-line response preview trace"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_control_char_response_preview_trace() {
    let mut wrapped_response_trace = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    wrapped_response_trace.response_preview = Some("\nLive provider response.".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[wrapped_response_trace.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_response_trace);
    assert!(
        blockers.contains(&"live_provider_trace_missing".to_string()),
        "external live evidence must reject control characters before trimming"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_whitespace_response_preview_trace() {
    let mut wrapped_response_trace = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    wrapped_response_trace.response_preview = Some(" Live provider response. ".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[wrapped_response_trace.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_response_trace);
    assert!(
        blockers.contains(&"live_provider_trace_missing".to_string()),
        "external live evidence must reject wrapping whitespace in response preview trace"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_unnormalized_whitespace_response_preview_trace()
{
    let mut unnormalized_response_trace = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    unnormalized_response_trace.response_preview = Some("Live  provider response.".into());

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[unnormalized_response_trace.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&unnormalized_response_trace);
    assert!(
        blockers.contains(&"live_provider_trace_missing".to_string()),
        "external live evidence must reject response preview traces that do not match harness whitespace normalization"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_contract_unsafe_run_task_trace_ids() {
    let mut unsafe_run_trace = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    unsafe_run_trace.run_id = Some("live run direct".into());

    let mut unsafe_task_trace = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    unsafe_task_trace.task_session_id = Some("live task web".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        unsafe_run_trace.clone(),
        unsafe_task_trace.clone(),
    ]);

    assert!(!evidence.generation_eval_executed);
    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&unsafe_run_trace);
    assert!(
        blockers.contains(&"live_provider_trace_missing".to_string()),
        "external live evidence must preserve a metadata-safe run id"
    );
    let blockers = main_chat_live_provider_report_blockers(&unsafe_task_trace);
    assert!(
        blockers.contains(&"live_provider_trace_missing".to_string()),
        "external live evidence must preserve a metadata-safe task session id"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_control_char_run_task_trace_ids() {
    let mut wrapped_run_trace = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    wrapped_run_trace.run_id = Some("\nlive-run-direct-answer".into());

    let mut wrapped_task_trace = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    wrapped_task_trace.task_session_id = Some("\nlive-task-web-agent-loop".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        wrapped_run_trace.clone(),
        wrapped_task_trace.clone(),
    ]);

    assert!(!evidence.generation_eval_executed);
    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_run_trace);
    assert!(
        blockers.contains(&"live_provider_trace_missing".to_string()),
        "external live evidence must reject control characters before trimming run ids"
    );
    let blockers = main_chat_live_provider_report_blockers(&wrapped_task_trace);
    assert!(
        blockers.contains(&"live_provider_trace_missing".to_string()),
        "external live evidence must reject control characters before trimming task ids"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_external_provider_endpoint() {
    let mut local_http_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    local_http_report.provider_endpoint_kind = "local_test_http".into();

    let evidence = main_chat_live_provider_acceptance_evidence(&[local_http_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&local_http_report);
    assert!(
        blockers.contains(&"live_provider_external_provider_missing".to_string()),
        "local HTTP provider proof must remain explicitly uncredited as external live evidence"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_external_provider_identity() {
    let mut local_provider_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    local_provider_report.provider = "ollama".into();
    local_provider_report.provider_endpoint_kind = "external_provider".into();

    let evidence = main_chat_live_provider_acceptance_evidence(&[local_provider_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&local_provider_report);
    assert!(
        blockers.contains(&"live_provider_external_provider_missing".to_string()),
        "local provider identity must not receive external live evidence credit"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_embedded_synthetic_provider_identity() {
    let mut synthetic_provider_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    synthetic_provider_report.provider = "mockopenai".into();
    synthetic_provider_report.provider_endpoint_kind = "external_provider".into();

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[synthetic_provider_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&synthetic_provider_report);
    assert!(
        blockers.contains(&"live_provider_external_provider_missing".to_string()),
        "provider identities embedding synthetic provider labels must not receive external live evidence credit"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_localhost_provider_identity() {
    let mut localhost_provider_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    localhost_provider_report.provider = "localhost".into();
    localhost_provider_report.provider_endpoint_kind = "external_provider".into();

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[localhost_provider_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&localhost_provider_report);
    assert!(
        blockers.contains(&"live_provider_external_provider_missing".to_string()),
        "localhost provider identity must not receive external live evidence credit"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_loopback_provider_alias_identity() {
    let mut loopback_provider_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    loopback_provider_report.provider = "127-0-0-1".into();
    loopback_provider_report.provider_endpoint_kind = "external_provider".into();

    let evidence = main_chat_live_provider_acceptance_evidence(&[loopback_provider_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&loopback_provider_report);
    assert!(
        blockers.contains(&"live_provider_external_provider_missing".to_string()),
        "loopback provider aliases must not receive external live evidence credit"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_embedded_local_provider_alias_identity() {
    let mut loopback_provider_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    loopback_provider_report.provider = "openai-127-0-0-1".into();
    loopback_provider_report.provider_endpoint_kind = "external_provider".into();

    let evidence = main_chat_live_provider_acceptance_evidence(&[loopback_provider_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&loopback_provider_report);
    assert!(
        blockers.contains(&"live_provider_external_provider_missing".to_string()),
        "provider identities embedding loopback aliases must not receive external live evidence credit"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_alphanumeric_embedded_local_provider_alias_identity(
) {
    let mut loopback_provider_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    loopback_provider_report.provider = "openai127-0-0-1".into();
    loopback_provider_report.provider_endpoint_kind = "external_provider".into();

    let evidence = main_chat_live_provider_acceptance_evidence(&[loopback_provider_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&loopback_provider_report);
    assert!(
        blockers.contains(&"live_provider_external_provider_missing".to_string()),
        "provider identities embedding loopback aliases inside alphanumeric labels must not receive external live evidence credit"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_contract_unsafe_external_provider_identity() {
    let mut unsafe_provider_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    unsafe_provider_report.provider = "open ai".into();
    unsafe_provider_report.provider_endpoint_kind = "external_provider".into();

    let evidence = main_chat_live_provider_acceptance_evidence(&[unsafe_provider_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&unsafe_provider_report);
    assert!(
        blockers.contains(&"live_provider_external_provider_missing".to_string()),
        "external live evidence must preserve a metadata-safe non-local provider identity"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_control_char_external_provider_identity(
) {
    let mut wrapped_provider_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    wrapped_provider_report.provider = "\nopenai".into();
    wrapped_provider_report.provider_endpoint_kind = "external_provider".into();

    let evidence = main_chat_live_provider_acceptance_evidence(&[wrapped_provider_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_provider_report);
    assert!(
        blockers.contains(&"live_provider_external_provider_missing".to_string()),
        "external live evidence must reject control characters before trimming provider identity"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_whitespace_external_provider_identity()
{
    let mut wrapped_provider_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    wrapped_provider_report.provider = " openai".into();
    wrapped_provider_report.provider_endpoint_kind = "external_provider".into();

    let evidence = main_chat_live_provider_acceptance_evidence(&[wrapped_provider_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_provider_report);
    assert!(
        blockers.contains(&"live_provider_external_provider_missing".to_string()),
        "external live evidence must reject wrapping whitespace before normalizing provider identity"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_provider_model_identity() {
    let mut missing_model_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    missing_model_report.provider_model = None;

    let evidence = main_chat_live_provider_acceptance_evidence(&[missing_model_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&missing_model_report);
    assert!(
        blockers.contains(&"live_provider_model_identity_missing".to_string()),
        "external live evidence must preserve the provider model identity"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_contract_unsafe_provider_model_identity() {
    let mut unsafe_model_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    unsafe_model_report.provider_model = Some("gpt live eval".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[unsafe_model_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&unsafe_model_report);
    assert!(
        blockers.contains(&"live_provider_model_identity_missing".to_string()),
        "external live evidence must preserve a metadata-safe provider model identity"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_control_char_provider_model_identity()
{
    let mut wrapped_model_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    wrapped_model_report.provider_model = Some("\ngpt-live-eval".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[wrapped_model_report.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_model_report);
    assert!(
        blockers.contains(&"live_provider_model_identity_missing".to_string()),
        "external live evidence must reject control characters before trimming provider model identity"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_required_evidence_manifest() {
    let mut missing_required_evidence = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    missing_required_evidence.required_evidence.clear();

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[missing_required_evidence.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&missing_required_evidence);
    assert!(
        blockers.contains(&"live_provider_required_evidence_missing".to_string()),
        "external live evidence must preserve the live-provider required-evidence manifest"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_extra_required_evidence_labels() {
    let mut unsafe_required_evidence = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    unsafe_required_evidence
        .required_evidence
        .push("external live proof".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[unsafe_required_evidence.clone()]);

    assert!(!evidence.generation_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&unsafe_required_evidence);
    assert!(
        blockers.contains(&"live_provider_required_evidence_missing".to_string()),
        "external live evidence must preserve only the metadata-safe required-evidence manifest"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_react_governance_trace() {
    let mut missing_allowed_tool = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    missing_allowed_tool.model_selected_allowed_tool = false;

    let mut missing_policy_trace = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    missing_policy_trace.model_selected_execution_policy_validated = false;

    let mut missing_governed_arguments = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    missing_governed_arguments.model_selected_governed_arguments = false;

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        missing_allowed_tool.clone(),
        missing_policy_trace.clone(),
        missing_governed_arguments.clone(),
    ]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&missing_allowed_tool);
    assert!(
        blockers.contains(&"live_provider_tool_selection_trace_missing".to_string()),
        "live ReAct reports missing governed model-selection trace must not receive credit"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_hashed_governed_arguments_digest() {
    let mut weak_arguments_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    weak_arguments_digest.model_selected_governed_arguments_digest = Some("bytes:12".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[weak_arguments_digest.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&weak_arguments_digest);
    assert!(
        blockers.contains(&"live_provider_tool_selection_trace_missing".to_string()),
        "live ReAct reports must preserve the metadata-safe hash for governed candidate arguments"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_ranked_manifest_trace() {
    let mut missing_rank = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    missing_rank.model_selected_candidate_rank = None;

    let mut missing_source = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    missing_source.model_selected_candidate_source = None;

    let mut missing_capability_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    missing_capability_digest.model_selected_candidate_capabilities_digest = None;

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        missing_rank.clone(),
        missing_source,
        missing_capability_digest,
    ]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&missing_rank);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "live ReAct reports missing ranked manifest/capability trace must not receive credit"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_selected_rank_to_match_candidate_order() {
    let mut mismatched_rank_web = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    mismatched_rank_web.tool_selection_candidate_count = 2;
    mismatched_rank_web.tool_selection_candidate_ids =
        vec!["web.search".into(), "web.fetch".into()];
    mismatched_rank_web.tool_selection_allowlist = vec!["web.search".into(), "web.fetch".into()];
    mismatched_rank_web.tool_selection_allowed_actions = vec![
        serde_json::json!({
            "actionType": "mcp_tool",
            "target": "web.search",
        }),
        serde_json::json!({
            "actionType": "mcp_tool",
            "target": "web.fetch",
        }),
    ];
    mismatched_rank_web.model_selected_candidate_id = Some("web.fetch".into());
    mismatched_rank_web.model_selected_candidate_target = Some("web.fetch".into());
    mismatched_rank_web.model_selected_candidate_action_type = Some("mcp_tool".into());
    mismatched_rank_web.model_selected_candidate_rank = Some(1);

    let evidence = main_chat_live_provider_acceptance_evidence(&[mismatched_rank_web.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&mismatched_rank_web);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "live ReAct credit must prove the selected candidate rank matches the bounded candidate order"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_contract_unsafe_ranked_manifest_labels() {
    let mut unsafe_source_web = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    unsafe_source_web.model_selected_candidate_source = Some("planned action".into());

    let mut unsafe_reason_proposal = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    unsafe_reason_proposal.model_selected_candidate_match_reason = Some("manifest match".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        unsafe_source_web.clone(),
        unsafe_reason_proposal.clone(),
    ]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&unsafe_source_web);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "live ReAct reports must preserve a metadata-safe candidate source label"
    );
    let blockers = main_chat_live_provider_report_blockers(&unsafe_reason_proposal);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "live ReAct reports must preserve a metadata-safe candidate match reason label"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_control_char_ranked_manifest_labels() {
    let mut wrapped_source_web = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    wrapped_source_web.model_selected_candidate_source = Some("\nplanned_action".into());

    let mut wrapped_reason_proposal = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    wrapped_reason_proposal.model_selected_candidate_match_reason = Some("\nplanned_action".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        wrapped_source_web.clone(),
        wrapped_reason_proposal.clone(),
    ]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_source_web);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "live ReAct reports must reject control characters before trimming candidate source labels"
    );
    let blockers = main_chat_live_provider_report_blockers(&wrapped_reason_proposal);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "live ReAct reports must reject control characters before trimming candidate match reasons"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_hashed_candidate_capability_digest() {
    let mut weak_capability_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    weak_capability_digest.model_selected_candidate_capabilities_digest = Some("bytes:12".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[weak_capability_digest.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&weak_capability_digest);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "live ReAct reports must preserve the metadata-safe hash for selected candidate capabilities"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_selected_candidate_capability_labels() {
    let mut missing_capability_labels = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    missing_capability_labels.model_selected_candidate_capability_labels = None;

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[missing_capability_labels.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&missing_capability_labels);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "live ReAct reports must preserve bounded safe selected-candidate capability labels"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_write_like_candidate_capability_labels() {
    let mut write_like_capability_labels = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    write_like_capability_labels.model_selected_candidate_capability_labels =
        Some("read/delete".into());

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[write_like_capability_labels.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&write_like_capability_labels);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "live ReAct reports must reject write-like selected-candidate capability labels"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_read_candidate_capability_label() {
    let mut non_read_capability_labels = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    non_read_capability_labels.model_selected_candidate_capability_labels = Some("utility".into());

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[non_read_capability_labels.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&non_read_capability_labels);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "live ReAct reports must prove the selected candidate has a read capability label"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_freeform_digest_hash_labels() {
    let mut unsafe_arguments_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    unsafe_arguments_digest.model_selected_governed_arguments_digest =
        Some("bytes:12 hash:raw prompt contents".into());

    let mut unsafe_capability_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    unsafe_capability_digest.model_selected_candidate_capabilities_digest =
        Some("bytes:12 hash:raw capability contents".into());

    let mut unsafe_ranking_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    unsafe_ranking_digest.tool_selection_model_ranking_response_digest =
        Some("bytes:12 hash:raw provider ranking response".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        unsafe_arguments_digest.clone(),
        unsafe_capability_digest.clone(),
        unsafe_ranking_digest.clone(),
    ]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&unsafe_arguments_digest);
    assert!(
        blockers.contains(&"live_provider_tool_selection_trace_missing".to_string()),
        "governed argument digest labels must contain only compact metadata-safe hash tokens"
    );
    let blockers = main_chat_live_provider_report_blockers(&unsafe_capability_digest);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "candidate capability digest labels must contain only compact metadata-safe hash tokens"
    );
    let blockers = main_chat_live_provider_report_blockers(&unsafe_ranking_digest);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "provider ranking digest labels must contain only compact metadata-safe hash tokens"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_control_char_digest_labels() {
    let mut wrapped_arguments_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    wrapped_arguments_digest.model_selected_governed_arguments_digest = Some(
        "\nbytes:12 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .into(),
    );

    let evidence = main_chat_live_provider_acceptance_evidence(&[wrapped_arguments_digest.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_arguments_digest);
    assert!(
        blockers.contains(&"live_provider_tool_selection_trace_missing".to_string()),
        "digest labels must reject control characters before trimming"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_whitespace_digest_labels() {
    let mut wrapped_arguments_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    wrapped_arguments_digest.model_selected_governed_arguments_digest = Some(
        " bytes:12 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .into(),
    );

    let mut wrapped_capability_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    wrapped_capability_digest.model_selected_candidate_capabilities_digest = Some(
        "bytes:12 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000 "
            .into(),
    );

    let mut wrapped_ranking_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    wrapped_ranking_digest.tool_selection_model_ranking_response_digest = Some(
        "bytes:12 hash: sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .into(),
    );

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        wrapped_arguments_digest.clone(),
        wrapped_capability_digest.clone(),
        wrapped_ranking_digest.clone(),
    ]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_arguments_digest);
    assert!(
        blockers.contains(&"live_provider_tool_selection_trace_missing".to_string()),
        "governed argument digest labels must not be normalized before credit"
    );
    let blockers = main_chat_live_provider_report_blockers(&wrapped_capability_digest);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "candidate capability digest labels must not be normalized before credit"
    );
    let blockers = main_chat_live_provider_report_blockers(&wrapped_ranking_digest);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "provider ranking digest labels must not be normalized before credit"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_zero_byte_digest_labels() {
    let zero_byte_digest =
        "bytes:0 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let mut zero_arguments_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    zero_arguments_digest.model_selected_governed_arguments_digest = Some(zero_byte_digest.into());

    let mut zero_capability_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    zero_capability_digest.model_selected_candidate_capabilities_digest =
        Some(zero_byte_digest.into());

    let mut zero_ranking_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    zero_ranking_digest.tool_selection_model_ranking_response_digest =
        Some(zero_byte_digest.into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        zero_arguments_digest.clone(),
        zero_capability_digest.clone(),
        zero_ranking_digest.clone(),
    ]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&zero_arguments_digest);
    assert!(
        blockers.contains(&"live_provider_tool_selection_trace_missing".to_string()),
        "governed argument digest labels must prove a non-zero serialized metadata payload"
    );
    let blockers = main_chat_live_provider_report_blockers(&zero_capability_digest);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "candidate capability digest labels must prove a non-zero serialized metadata payload"
    );
    let blockers = main_chat_live_provider_report_blockers(&zero_ranking_digest);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "provider ranking digest labels must prove a non-zero serialized metadata payload"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_leading_zero_byte_count_digest_labels() {
    let leading_zero_digest =
        "bytes:012 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let mut leading_zero_arguments_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    leading_zero_arguments_digest.model_selected_governed_arguments_digest =
        Some(leading_zero_digest.into());

    let mut leading_zero_capability_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    leading_zero_capability_digest.model_selected_candidate_capabilities_digest =
        Some(leading_zero_digest.into());

    let mut leading_zero_ranking_digest = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    leading_zero_ranking_digest.tool_selection_model_ranking_response_digest =
        Some(leading_zero_digest.into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        leading_zero_arguments_digest.clone(),
        leading_zero_capability_digest.clone(),
        leading_zero_ranking_digest.clone(),
    ]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&leading_zero_arguments_digest);
    assert!(
        blockers.contains(&"live_provider_tool_selection_trace_missing".to_string()),
        "governed argument digest byte counts must use canonical decimal form"
    );
    let blockers = main_chat_live_provider_report_blockers(&leading_zero_capability_digest);
    assert!(
        blockers.contains(&"live_provider_ranked_manifest_trace_missing".to_string()),
        "candidate capability digest byte counts must use canonical decimal form"
    );
    let blockers = main_chat_live_provider_report_blockers(&leading_zero_ranking_digest);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "provider ranking digest byte counts must use canonical decimal form"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_candidate_allowlist_trace() {
    let mut missing_candidate_ids = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    missing_candidate_ids.tool_selection_candidate_ids.clear();

    let mut missing_allowlist = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    missing_allowlist.tool_selection_allowlist.clear();

    let mut missing_action_pair = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    missing_action_pair.tool_selection_allowed_actions.clear();
    missing_action_pair.model_selected_candidate_target = None;
    missing_action_pair.model_selected_candidate_action_type = None;

    let evidence = main_chat_live_provider_acceptance_evidence(&[
        missing_candidate_ids.clone(),
        missing_allowlist,
        missing_action_pair,
    ]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&missing_candidate_ids);
    assert!(
        blockers.contains(&"live_provider_candidate_allowlist_trace_missing".to_string()),
        "live ReAct credit must prove the selected tool came from the bounded candidate list and exact action-target allowlist"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_exact_candidate_allowlist_cardinality() {
    let mut extra_action_web = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    extra_action_web
        .tool_selection_allowed_actions
        .push(serde_json::json!({
            "actionType": "mcp_tool",
            "target": "memory.search",
        }));

    let evidence = main_chat_live_provider_acceptance_evidence(&[extra_action_web.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&extra_action_web);
    assert!(
        blockers.contains(&"live_provider_candidate_allowlist_trace_missing".to_string()),
        "live ReAct credit must prove the bounded candidate ids, target allowlist, and action-target allowlist describe the same exact set"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_distinct_exact_candidate_allowlist_sets() {
    let mut duplicate_target_web = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    duplicate_target_web.tool_selection_candidate_count = 2;
    duplicate_target_web.tool_selection_candidate_ids =
        vec!["web.search".into(), "web.fetch".into()];
    duplicate_target_web.tool_selection_allowlist = vec!["web.search".into(), "web.search".into()];
    duplicate_target_web.tool_selection_allowed_actions = vec![
        serde_json::json!({
            "actionType": "web_search",
            "target": "web.search",
        }),
        serde_json::json!({
            "actionType": "web_search",
            "target": "web.search",
        }),
    ];
    duplicate_target_web.model_selected_candidate_id = Some("web.search".into());
    duplicate_target_web.model_selected_candidate_target = Some("web.search".into());
    duplicate_target_web.model_selected_candidate_action_type = Some("web_search".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[duplicate_target_web.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&duplicate_target_web);
    assert!(
        blockers.contains(&"live_provider_candidate_allowlist_trace_missing".to_string()),
        "live ReAct credit must prove distinct candidate ids, allowlist targets, and action targets describe the same exact bounded set"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_exact_allowlist_action_types() {
    let mut wrong_action_type_web = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    wrong_action_type_web.tool_selection_candidate_count = 2;
    wrong_action_type_web.tool_selection_candidate_ids =
        vec!["web.search".into(), "web.fetch".into()];
    wrong_action_type_web.tool_selection_allowlist = vec!["web.search".into(), "web.fetch".into()];
    wrong_action_type_web.tool_selection_allowed_actions = vec![
        serde_json::json!({
            "actionType": "mcp_tool",
            "target": "web.search",
        }),
        serde_json::json!({
            "actionType": "file_read",
            "target": "web.fetch",
        }),
    ];
    wrong_action_type_web.model_selected_candidate_id = Some("web.search".into());
    wrong_action_type_web.model_selected_candidate_target = Some("web.search".into());
    wrong_action_type_web.model_selected_candidate_action_type = Some("mcp_tool".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[wrong_action_type_web.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrong_action_type_web);
    assert!(
        blockers.contains(&"live_provider_candidate_allowlist_trace_missing".to_string()),
        "live ReAct credit must prove every allowed action-target pair uses the governed selected action type"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_extra_allowed_action_fields() {
    let mut extra_field_web = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    extra_field_web.tool_selection_allowed_actions = vec![serde_json::json!({
        "actionType": "mcp_tool",
        "target": "web.search",
        "arguments": {
            "q": "untrusted model-supplied payload"
        }
    })];

    let evidence = main_chat_live_provider_acceptance_evidence(&[extra_field_web.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&extra_field_web);
    assert!(
        blockers.contains(&"live_provider_candidate_allowlist_trace_missing".to_string()),
        "live ReAct credit must prove action-target allowlist entries are exact metadata-safe pairs without smuggled fields"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_multi_candidate_registered_mcp_selection() {
    let mut single_candidate_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    single_candidate_mcp.tool_selection_candidate_count = 1;

    let evidence = main_chat_live_provider_acceptance_evidence(&[single_candidate_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&single_candidate_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_mcp_candidate_trace_missing".to_string()),
        "registered MCP live credit must prove model selection from a bounded multi-candidate set"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_provider_ranked_registered_mcp_selection() {
    let mut deterministic_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    deterministic_mcp.tool_selection_model_ranked = false;
    deterministic_mcp.tool_selection_ranking_source = Some("deterministic_local".into());
    deterministic_mcp
        .tool_selection_model_ranking_candidate_ids
        .clear();
    deterministic_mcp.tool_selection_model_ranking_response_digest = None;

    let evidence = main_chat_live_provider_acceptance_evidence(&[deterministic_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&deterministic_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must prove provider-ranked candidate ordering, not only local deterministic ordering"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_provider_backed_ranking_route() {
    let mut local_ranked_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    local_ranked_mcp.tool_selection_ranking_provider_backed = false;
    local_ranked_mcp.tool_selection_ranking_route_type = Some("local".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[local_ranked_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&local_ranked_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must prove provider-ranked selection used a cloud/provider-backed ranking route"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_matching_ranking_provider_identity() {
    let mut mismatched_provider_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    mismatched_provider_mcp.provider = "openai".into();
    mismatched_provider_mcp.tool_selection_ranking_provider = Some("deepseek".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[mismatched_provider_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&mismatched_provider_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must prove provider-ranked selection came from the same external provider as the live run"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_raw_exact_ranking_provider_identity() {
    let mut case_mismatched_provider_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    case_mismatched_provider_mcp.provider = "OpenAI".into();
    case_mismatched_provider_mcp.tool_selection_ranking_provider = Some("openai".into());

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[case_mismatched_provider_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&case_mismatched_provider_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must prove raw-exact ranking provider identity, not a case-normalized match"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_control_char_ranking_provider_identity(
) {
    let mut wrapped_provider_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    wrapped_provider_mcp.provider = "openai".into();
    wrapped_provider_mcp.tool_selection_ranking_provider = Some("\nopenai".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[wrapped_provider_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_provider_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must reject control characters before trimming ranking provider identity"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_matching_ranking_model_identity() {
    let mut mismatched_model_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    mismatched_model_mcp.provider_model = Some("gpt-live-eval".into());
    mismatched_model_mcp.tool_selection_ranking_model = Some("other-live-model".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[mismatched_model_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&mismatched_model_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must prove provider-ranked selection came from the same model as the live run"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_wrapping_control_char_ranking_model_identity() {
    let mut wrapped_model_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    wrapped_model_mcp.provider_model = Some("gpt-live-eval".into());
    wrapped_model_mcp.tool_selection_ranking_model = Some("\ngpt-live-eval".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[wrapped_model_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrapped_model_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must reject control characters before trimming ranking model identity"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_hashed_ranking_response_digest() {
    let mut weak_digest_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    weak_digest_mcp.tool_selection_model_ranking_response_digest = Some("bytes:8".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[weak_digest_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&weak_digest_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must preserve the metadata-safe hash for the provider ranking response"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_provider_ranked_selected_mcp_candidate() {
    let mut locally_selected_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    locally_selected_mcp.model_selected_candidate_match_reason = Some("planned_action".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[locally_selected_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&locally_selected_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must prove the selected candidate came from the provider-ranked list"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_selected_mcp_candidate_target_identity() {
    let mut mismatched_selection_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    mismatched_selection_mcp.model_selected_candidate_id = Some("builtin_echo".into());
    mismatched_selection_mcp.model_selected_candidate_target = Some("tool.list_available".into());
    mismatched_selection_mcp.model_selected_candidate_action_type = Some("mcp_tool".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[mismatched_selection_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&mismatched_selection_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must prove the selected candidate id and target describe the same MCP candidate"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_selected_rank_to_match_provider_order() {
    let mut mismatched_rank_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    mismatched_rank_mcp.tool_selection_model_ranking_candidate_ids =
        vec!["tool.list_available".into(), "builtin_echo".into()];
    mismatched_rank_mcp.model_selected_candidate_id = Some("builtin_echo".into());
    mismatched_rank_mcp.model_selected_candidate_target = Some("builtin_echo".into());
    mismatched_rank_mcp.model_selected_candidate_rank = Some(1);

    let evidence = main_chat_live_provider_acceptance_evidence(&[mismatched_rank_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&mismatched_rank_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must prove the selected candidate rank matches the provider-ranked candidate order"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_candidate_order_to_match_provider_ranking() {
    let mut mismatched_order_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    mismatched_order_mcp.tool_selection_model_ranking_candidate_ids =
        vec!["tool.list_available".into(), "builtin_echo".into()];
    mismatched_order_mcp.model_selected_candidate_id = Some("tool.list_available".into());
    mismatched_order_mcp.model_selected_candidate_target = Some("tool.list_available".into());
    mismatched_order_mcp.model_selected_candidate_rank = Some(1);

    let evidence = main_chat_live_provider_acceptance_evidence(&[mismatched_order_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&mismatched_order_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must prove AgentLoop candidate order preserved the provider-ranked order"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_complete_provider_ranked_candidate_set() {
    let mut partially_ranked_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    partially_ranked_mcp
        .tool_selection_candidate_ids
        .push("session.search".into());
    partially_ranked_mcp
        .tool_selection_allowlist
        .push("session.search".into());
    partially_ranked_mcp
        .tool_selection_allowed_actions
        .push(serde_json::json!({
            "actionType": "mcp_tool",
            "target": "session.search",
        }));
    partially_ranked_mcp.tool_selection_candidate_count = 3;
    partially_ranked_mcp.tool_selection_model_ranking_candidate_ids =
        vec!["builtin_echo".into(), "tool.list_available".into()];

    let evidence = main_chat_live_provider_acceptance_evidence(&[partially_ranked_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&partially_ranked_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_selection_trace_missing".to_string()),
        "registered MCP live credit must prove the provider ranked the complete bounded candidate set"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_mcp_allowlist_targets_match_candidate_ids() {
    let mut mismatched_allowlist_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    mismatched_allowlist_mcp.tool_selection_candidate_ids =
        vec!["builtin_echo".into(), "tool.list_available".into()];
    mismatched_allowlist_mcp.tool_selection_allowlist =
        vec!["builtin_echo".into(), "memory.search".into()];
    mismatched_allowlist_mcp.tool_selection_allowed_actions = vec![
        serde_json::json!({
            "actionType": "mcp_tool",
            "target": "builtin_echo",
        }),
        serde_json::json!({
            "actionType": "mcp_tool",
            "target": "memory.search",
        }),
    ];
    mismatched_allowlist_mcp.tool_selection_candidate_count = 2;
    mismatched_allowlist_mcp.tool_selection_model_ranking_candidate_ids =
        vec!["builtin_echo".into(), "tool.list_available".into()];
    mismatched_allowlist_mcp.model_selected_candidate_id = Some("builtin_echo".into());
    mismatched_allowlist_mcp.model_selected_candidate_target = Some("builtin_echo".into());
    mismatched_allowlist_mcp.model_selected_candidate_rank = Some(1);

    let evidence = main_chat_live_provider_acceptance_evidence(&[mismatched_allowlist_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&mismatched_allowlist_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_mcp_candidate_trace_missing".to_string()),
        "registered MCP live credit must prove the bounded target/action allowlists match the bounded candidate ids"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_contract_unsafe_candidate_labels() {
    let mut unsafe_label_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    unsafe_label_mcp.tool_selection_candidate_ids =
        vec!["builtin echo".into(), "tool.list_available".into()];
    unsafe_label_mcp.tool_selection_allowlist =
        vec!["builtin echo".into(), "tool.list_available".into()];
    unsafe_label_mcp.tool_selection_allowed_actions = vec![
        serde_json::json!({
            "actionType": "mcp_tool",
            "target": "builtin echo",
        }),
        serde_json::json!({
            "actionType": "mcp_tool",
            "target": "tool.list_available",
        }),
    ];
    unsafe_label_mcp.tool_selection_candidate_count = 2;
    unsafe_label_mcp.tool_selection_model_ranking_candidate_ids =
        vec!["builtin echo".into(), "tool.list_available".into()];
    unsafe_label_mcp.model_selected_candidate_id = Some("builtin echo".into());
    unsafe_label_mcp.model_selected_candidate_target = Some("builtin echo".into());
    unsafe_label_mcp.model_selected_candidate_rank = Some(1);

    let evidence = main_chat_live_provider_acceptance_evidence(&[unsafe_label_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&unsafe_label_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_mcp_candidate_trace_missing".to_string()),
        "registered MCP live credit must reject contract-unsafe candidate and allowlist labels"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_distinct_registered_mcp_candidates() {
    let mut duplicate_candidate_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    duplicate_candidate_mcp.tool_selection_candidate_count = 2;
    duplicate_candidate_mcp.tool_selection_candidate_ids =
        vec!["builtin_echo".into(), "builtin_echo".into()];
    duplicate_candidate_mcp.tool_selection_allowlist =
        vec!["builtin_echo".into(), "builtin_echo".into()];
    duplicate_candidate_mcp.tool_selection_allowed_actions = vec![
        serde_json::json!({
            "actionType": "mcp_tool",
            "target": "builtin_echo",
        }),
        serde_json::json!({
            "actionType": "mcp_tool",
            "target": "builtin_echo",
        }),
    ];
    duplicate_candidate_mcp.model_selected_candidate_id = Some("builtin_echo".into());
    duplicate_candidate_mcp.model_selected_candidate_target = Some("builtin_echo".into());
    duplicate_candidate_mcp.model_selected_candidate_action_type = Some("mcp_tool".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[duplicate_candidate_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&duplicate_candidate_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_mcp_candidate_trace_missing".to_string()),
        "registered MCP live credit must prove distinct bounded model-selectable MCP candidates"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_unique_registered_mcp_candidate_ids() {
    let mut duplicate_candidate_id_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    duplicate_candidate_id_mcp.tool_selection_candidate_count = 3;
    duplicate_candidate_id_mcp.tool_selection_candidate_ids = vec![
        "builtin_echo".into(),
        "tool.list_available".into(),
        "tool.list_available".into(),
    ];

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[duplicate_candidate_id_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&duplicate_candidate_id_mcp);
    assert!(
        blockers.contains(&"live_provider_model_ranked_mcp_candidate_trace_missing".to_string()),
        "registered MCP live credit must prove the bounded candidate id list has no duplicates"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_mcp_success_without_permission_proposal() {
    let mut mixed_outcome_mcp = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
    );
    mixed_outcome_mcp.tool_permission_proposal_created = true;
    mixed_outcome_mcp.tool_permission_proposal_target = Some("builtin_echo".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[mixed_outcome_mcp.clone()]);

    assert!(!evidence.mcp_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&mixed_outcome_mcp);
    assert!(
        blockers.contains(&"live_provider_mcp_agent_loop_not_completed".to_string()),
        "registered MCP live credit must prove a successful read outcome without an overlapping ToolPermission proposal outcome"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_react_without_single_step_fallback() {
    for (scenario, evidence_missing, expected_blocker) in [
        (
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
            "web",
            "live_provider_web_agent_loop_not_completed",
        ),
        (
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop,
            "mcp",
            "live_provider_mcp_agent_loop_not_completed",
        ),
        (
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
            "proposal",
            "live_provider_proposal_permission_not_completed",
        ),
    ] {
        let mut fallback_report = successful_live_provider_harness_report(scenario);
        fallback_report.single_step_fallback_used = true;

        let evidence = main_chat_live_provider_acceptance_evidence(&[fallback_report.clone()]);
        match evidence_missing {
            "web" => {
                assert!(!evidence.web_agent_loop_eval_executed);
                assert!(!evidence.web_mcp_agent_loop_eval_executed);
            }
            "mcp" => {
                assert!(!evidence.mcp_agent_loop_eval_executed);
                assert!(!evidence.web_mcp_agent_loop_eval_executed);
            }
            "proposal" => assert!(!evidence.proposal_permission_eval_executed),
            _ => unreachable!("covered scenario"),
        }

        let blockers = main_chat_live_provider_report_blockers(&fallback_report);
        assert!(
            blockers.contains(&expected_blocker.to_string()),
            "{scenario:?} live credit must not allow single-step fallback reports"
        );
    }
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_web_target_for_web_agent_loop() {
    let mut non_web_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    non_web_report.tool_selection_candidate_ids = vec!["builtin_echo".into()];
    non_web_report.tool_selection_allowlist = vec!["builtin_echo".into()];
    non_web_report.tool_selection_allowed_actions = vec![serde_json::json!({
        "actionType": "mcp_tool",
        "target": "builtin_echo",
    })];
    non_web_report.model_selected_candidate_id = Some("builtin_echo".into());
    non_web_report.model_selected_candidate_target = Some("builtin_echo".into());
    non_web_report.model_selected_candidate_action_type = Some("mcp_tool".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[non_web_report.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&non_web_report);
    assert!(
        blockers.contains(&"live_provider_web_tool_target_trace_missing".to_string()),
        "web AgentLoop live credit must prove the selected allowed target is a governed web tool"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_web_selected_candidate_identity() {
    let mut mismatched_web_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    mismatched_web_report.tool_selection_candidate_ids = vec!["web.search".into()];
    mismatched_web_report.tool_selection_candidate_count = 1;
    mismatched_web_report.tool_selection_allowlist = vec!["web.fetch".into()];
    mismatched_web_report.tool_selection_allowed_actions = vec![serde_json::json!({
        "actionType": "mcp_tool",
        "target": "web.fetch",
    })];
    mismatched_web_report.model_selected_candidate_id = Some("web.search".into());
    mismatched_web_report.model_selected_candidate_target = Some("web.fetch".into());
    mismatched_web_report.model_selected_candidate_action_type = Some("mcp_tool".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[mismatched_web_report.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&mismatched_web_report);
    assert!(
        blockers.contains(&"live_provider_web_tool_target_trace_missing".to_string()),
        "web AgentLoop live credit must prove the selected candidate id is the selected governed web target"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_web_action_type_to_be_governed_tool() {
    let mut wrong_action_web_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    wrong_action_web_report.tool_selection_candidate_ids = vec!["web.search".into()];
    wrong_action_web_report.tool_selection_candidate_count = 1;
    wrong_action_web_report.tool_selection_allowlist = vec!["web.search".into()];
    wrong_action_web_report.tool_selection_allowed_actions = vec![serde_json::json!({
        "actionType": "file_read",
        "target": "web.search",
    })];
    wrong_action_web_report.model_selected_candidate_id = Some("web.search".into());
    wrong_action_web_report.model_selected_candidate_target = Some("web.search".into());
    wrong_action_web_report.model_selected_candidate_action_type = Some("file_read".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[wrong_action_web_report.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&wrong_action_web_report);
    assert!(
        blockers.contains(&"live_provider_web_tool_target_trace_missing".to_string()),
        "web AgentLoop live credit must prove the selected web target uses the governed tool action type"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_web_without_mcp_success() {
    let mut overlapping_mcp_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    overlapping_mcp_report.mcp_read_target_resolved = true;

    let evidence = main_chat_live_provider_acceptance_evidence(&[overlapping_mcp_report.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&overlapping_mcp_report);
    assert!(
        blockers.contains(&"live_provider_web_agent_loop_not_completed".to_string()),
        "web AgentLoop live credit must not overlap with a completed MCP read target"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_web_without_permission_proposal_trace() {
    let mut overlapping_proposal_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    overlapping_proposal_report.tool_permission_proposal_target = Some("builtin_echo".into());

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[overlapping_proposal_report.clone()]);

    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&overlapping_proposal_report);
    assert!(
        blockers.contains(&"live_provider_web_agent_loop_not_completed".to_string()),
        "web AgentLoop live credit must not overlap with a ToolPermission proposal target"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_mcp_target_for_permission_proposal() {
    let mut web_target_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    web_target_report.tool_selection_candidate_ids = vec!["web.search".into()];
    web_target_report.tool_selection_allowlist = vec!["web.search".into()];
    web_target_report.tool_selection_allowed_actions = vec![serde_json::json!({
        "actionType": "mcp_tool",
        "target": "web.search",
    })];
    web_target_report.model_selected_candidate_id = Some("web.search".into());
    web_target_report.model_selected_candidate_target = Some("web.search".into());
    web_target_report.model_selected_candidate_action_type = Some("mcp_tool".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[web_target_report.clone()]);

    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&web_target_report);
    assert!(
        blockers.contains(&"live_provider_proposal_permission_target_trace_missing".to_string()),
        "MCP ToolPermission proposal live credit must prove the selected target matches the governed permission proposal target"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_proposal_permission_selected_candidate_identity(
) {
    let mut mismatched_candidate_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    mismatched_candidate_report.tool_selection_candidate_ids = vec!["builtin_echo".into()];
    mismatched_candidate_report.tool_selection_candidate_count = 1;
    mismatched_candidate_report.tool_selection_allowlist = vec!["memory.search".into()];
    mismatched_candidate_report.tool_selection_allowed_actions = vec![serde_json::json!({
        "actionType": "mcp_tool",
        "target": "memory.search",
    })];
    mismatched_candidate_report.model_selected_candidate_id = Some("builtin_echo".into());
    mismatched_candidate_report.model_selected_candidate_target = Some("memory.search".into());
    mismatched_candidate_report.model_selected_candidate_action_type = Some("mcp_tool".into());

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[mismatched_candidate_report.clone()]);

    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&mismatched_candidate_report);
    assert!(
        blockers.contains(&"live_provider_proposal_permission_target_trace_missing".to_string()),
        "MCP ToolPermission proposal live credit must prove the selected candidate id is the governed proposal target"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_requires_proposal_without_mcp_success() {
    let mut overlapping_success_report = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal,
    );
    overlapping_success_report.mcp_read_target_resolved = true;

    let evidence =
        main_chat_live_provider_acceptance_evidence(&[overlapping_success_report.clone()]);

    assert!(!evidence.proposal_permission_eval_executed);

    let blockers = main_chat_live_provider_report_blockers(&overlapping_success_report);
    assert!(
        blockers.contains(&"live_provider_proposal_permission_not_completed".to_string()),
        "MCP ToolPermission proposal live credit must not overlap with a completed MCP read target"
    );
}

#[test]
fn main_chat_live_provider_harness_evidence_rejects_ready_report_with_failed_status_or_blockers() {
    let mut failed_status = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    failed_status.status = "failed".into();

    let mut blocked_web = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop,
    );
    blocked_web
        .blockers
        .push("provider_returned_no_tool_action".into());

    let evidence = main_chat_live_provider_acceptance_evidence(&[failed_status, blocked_web]);

    assert!(!evidence.generation_eval_executed);
    assert!(!evidence.web_agent_loop_eval_executed);
    assert!(!evidence.web_mcp_agent_loop_eval_executed);
}
