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
                    model_selected_allowed_tool: false,
                    model_selected_execution_policy_validated: false,
                    model_selected_execution_allowed: false,
                    model_selected_governed_arguments: false,
                    model_selected_candidate_id: None,
                    model_selected_candidate_target: None,
                    model_selected_candidate_action_type: None,
                    model_selected_candidate_rank: None,
                    model_selected_candidate_source: None,
                    model_selected_candidate_capabilities_digest: None,
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
        config.llm.provider =
            std::env::var("OPENLIFE_LIVE_EVAL_PROVIDER").unwrap_or_else(|_| "openai".into());
        config.llm.openai_base = std::env::var("OPENLIFE_LIVE_EVAL_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".into());
        config.llm.chat_model =
            std::env::var("OPENLIFE_LIVE_EVAL_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
        config.llm.openai_key = std::env::var("OPENLIFE_LIVE_EVAL_API_KEY")
            .unwrap_or_else(|_| std::env::var("OPENAI_API_KEY").unwrap_or_default());
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
    assert_eq!(report.command_surface_total_cases, 24);
    assert!(!report.live_provider_attempted);
    assert_eq!(report.live_provider_report_count, 0);
    assert_eq!(report.live_provider_ready_count, 0);
    assert!(!report.live_provider_direct_writes_executed);
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
    let command_surface_report = MainChatCommandSurfaceEvalReport {
        total_cases: 24,
        failed_cases: 0,
        send_coverage: 0.5,
        stream_coverage: 0.5,
        provider_generation_coverage: 1.0 / 24.0,
        file_read_coverage: 1.0 / 24.0,
        plan_execute_coverage: 1.0 / 24.0,
        proposal_coverage: 1.0 / 24.0,
        web_policy_blocker_coverage: 1.0 / 24.0,
        web_agent_loop_blocker_coverage: 1.0 / 24.0,
        web_agent_loop_success_coverage: 1.0 / 24.0,
        mcp_missing_read_target_blocker_coverage: 1.0 / 24.0,
        mcp_registered_read_success_coverage: 1.0 / 24.0,
        mcp_agent_loop_success_coverage: 1.0 / 24.0,
        mcp_tool_permission_proposal_coverage: 1.0 / 24.0,
        mcp_agent_loop_tool_permission_proposal_coverage: 1.0 / 24.0,
        final_completion_ready: false,
        final_completion_blockers: vec![
            "live_provider_generation_not_executed".into(),
            "provider_backed_web_mcp_agent_loop_not_executed".into(),
            "provider_backed_web_agent_loop_not_executed".into(),
            "provider_backed_mcp_agent_loop_not_executed".into(),
            "provider_live_proposal_permission_not_executed".into(),
        ],
        ..Default::default()
    };

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
fn main_chat_final_acceptance_gate_report_preserves_live_provider_failure_audit() {
    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let command_surface_report = MainChatCommandSurfaceEvalReport {
        total_cases: 24,
        failed_cases: 0,
        send_coverage: 0.5,
        stream_coverage: 0.5,
        provider_generation_coverage: 1.0 / 24.0,
        file_read_coverage: 1.0 / 24.0,
        plan_execute_coverage: 1.0 / 24.0,
        proposal_coverage: 1.0 / 24.0,
        web_policy_blocker_coverage: 1.0 / 24.0,
        web_agent_loop_blocker_coverage: 1.0 / 24.0,
        web_agent_loop_success_coverage: 1.0 / 24.0,
        mcp_missing_read_target_blocker_coverage: 1.0 / 24.0,
        mcp_registered_read_success_coverage: 1.0 / 24.0,
        mcp_agent_loop_success_coverage: 1.0 / 24.0,
        mcp_tool_permission_proposal_coverage: 1.0 / 24.0,
        mcp_agent_loop_tool_permission_proposal_coverage: 1.0 / 24.0,
        ..Default::default()
    };

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
    assert!(!report.acceptance.ready);
}

#[test]
fn main_chat_final_acceptance_gate_report_derives_post_invocation_live_provider_blockers() {
    let runtime_report =
        openlife_core::agent::main_chat_agent_v1::run_main_chat_agent_v1_runtime_eval_suite(
            openlife_core::agent::main_chat_agent_v1::main_chat_runtime_eval_cases(),
        );
    let command_surface_report = MainChatCommandSurfaceEvalReport {
        total_cases: 24,
        failed_cases: 0,
        send_coverage: 0.5,
        stream_coverage: 0.5,
        provider_generation_coverage: 1.0 / 24.0,
        file_read_coverage: 1.0 / 24.0,
        plan_execute_coverage: 1.0 / 24.0,
        proposal_coverage: 1.0 / 24.0,
        web_policy_blocker_coverage: 1.0 / 24.0,
        web_agent_loop_blocker_coverage: 1.0 / 24.0,
        web_agent_loop_success_coverage: 1.0 / 24.0,
        mcp_missing_read_target_blocker_coverage: 1.0 / 24.0,
        mcp_registered_read_success_coverage: 1.0 / 24.0,
        mcp_agent_loop_success_coverage: 1.0 / 24.0,
        mcp_tool_permission_proposal_coverage: 1.0 / 24.0,
        mcp_agent_loop_tool_permission_proposal_coverage: 1.0 / 24.0,
        ..Default::default()
    };
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
fn main_chat_live_provider_report_blockers_rejects_inconsistent_ready_report() {
    let mut inconsistent = successful_live_provider_harness_report(
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer,
    );
    inconsistent.direct_writes_executed = true;
    inconsistent.legacy_fallback_used = true;
    inconsistent.response_preview = None;

    let blockers = main_chat_live_provider_report_blockers(&inconsistent);

    assert!(blockers.contains(&"live_provider_direct_writes_detected".to_string()));
    assert!(blockers.contains(&"live_provider_legacy_fallback_detected".to_string()));
    assert!(blockers.contains(&"live_provider_trace_missing".to_string()));
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
