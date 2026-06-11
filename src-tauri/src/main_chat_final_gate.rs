use openlife_core::agent::main_chat_agent_v1::{
    evaluate_main_chat_agent_execution_v1_acceptance_gate,
    main_chat_runtime_eval_report_with_live_provider_evidence,
    MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence,
    MainChatAgentExecutionV1AcceptanceInput, MainChatAgentExecutionV1AcceptanceLiveEvidence,
    MainChatAgentExecutionV1AcceptanceReport, MainChatRuntimeEvalReport,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatLiveProviderEvalHarnessScenario {
    DirectAnswer,
    WebAgentLoop,
    RegisteredMcpAgentLoop,
    McpToolPermissionProposal,
}

impl MainChatLiveProviderEvalHarnessScenario {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer => "direct-answer",
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop => "web-agent-loop",
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => {
                "registered-mcp-agent-loop"
            }
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
                "mcp-tool-permission-proposal"
            }
        }
    }

    pub(crate) fn prompt(self) -> &'static str {
        match self {
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer => {
                "Answer in one short sentence: what is this live provider eval proving?"
            }
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop => {
                "Please web search OpenLife release notes and use the governed web.search action before answering."
            }
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => {
                "Use mcp builtin_echo read-only now and call the governed MCP read action before answering."
            }
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
                "Use mcp memory.search now and create a governed permission request if the tool requires review."
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MainChatLiveProviderEvalHarnessReport {
    pub(crate) scenario: MainChatLiveProviderEvalHarnessScenario,
    pub(crate) ready: bool,
    pub(crate) status: String,
    pub(crate) provider: String,
    pub(crate) provider_endpoint_kind: String,
    pub(crate) blockers: Vec<String>,
    pub(crate) required_evidence: Vec<String>,
    pub(crate) live_provider_invocation_allowed: bool,
    pub(crate) main_chat_invoked: bool,
    pub(crate) model_invoked: bool,
    pub(crate) direct_writes_executed: bool,
    pub(crate) legacy_fallback_used: bool,
    pub(crate) agent_loop_succeeded: bool,
    pub(crate) single_step_fallback_used: bool,
    pub(crate) agent_loop_action_status: Option<String>,
    pub(crate) mcp_read_target_resolved: bool,
    pub(crate) tool_permission_proposal_created: bool,
    pub(crate) run_id: Option<String>,
    pub(crate) task_session_id: Option<String>,
    pub(crate) response_preview: Option<String>,
}

pub(crate) fn main_chat_live_provider_required_evidence() -> Vec<String> {
    vec![
        "live_provider_generation".into(),
        "provider_backed_web_mcp_agent_loop".into(),
        "provider_backed_web_agent_loop".into(),
        "provider_backed_mcp_agent_loop".into(),
        "provider_live_proposal_permission".into(),
    ]
}

pub(crate) fn blocked_main_chat_live_provider_eval_harness_report(
    scenario: MainChatLiveProviderEvalHarnessScenario,
    provider: impl Into<String>,
    provider_endpoint_kind: impl Into<String>,
    blockers: Vec<String>,
    required_evidence: Vec<String>,
) -> MainChatLiveProviderEvalHarnessReport {
    MainChatLiveProviderEvalHarnessReport {
        scenario,
        ready: false,
        status: "blocked".into(),
        provider: provider.into(),
        provider_endpoint_kind: provider_endpoint_kind.into(),
        blockers,
        required_evidence,
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
        run_id: None,
        task_session_id: None,
        response_preview: None,
    }
}

pub(crate) fn completed_main_chat_live_provider_eval_harness_report(
    scenario: MainChatLiveProviderEvalHarnessScenario,
    provider: impl Into<String>,
    provider_endpoint_kind: impl Into<String>,
    run_id: impl Into<String>,
    task_session_id: impl Into<String>,
    response_preview: impl Into<String>,
) -> MainChatLiveProviderEvalHarnessReport {
    let agent_loop_succeeded = !matches!(
        scenario,
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer
    );
    let agent_loop_action_status = match scenario {
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer => None,
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
            Some("needs_confirmation".to_string())
        }
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop
        | MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => {
            Some("succeeded".to_string())
        }
    };

    MainChatLiveProviderEvalHarnessReport {
        scenario,
        ready: true,
        status: "completed".into(),
        provider: provider.into(),
        provider_endpoint_kind: provider_endpoint_kind.into(),
        blockers: Vec::new(),
        required_evidence: main_chat_live_provider_required_evidence(),
        live_provider_invocation_allowed: true,
        main_chat_invoked: true,
        model_invoked: true,
        direct_writes_executed: false,
        legacy_fallback_used: false,
        agent_loop_succeeded,
        single_step_fallback_used: false,
        agent_loop_action_status,
        mcp_read_target_resolved: matches!(
            scenario,
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop
        ),
        tool_permission_proposal_created: matches!(
            scenario,
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal
        ),
        run_id: Some(run_id.into()),
        task_session_id: Some(task_session_id.into()),
        response_preview: Some(response_preview.into()),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentExecutionV1FinalGateReport {
    pub(crate) runtime_total_cases: usize,
    pub(crate) command_surface_total_cases: usize,
    pub(crate) live_provider_attempted: bool,
    pub(crate) live_provider_report_count: usize,
    pub(crate) live_provider_ready_count: usize,
    pub(crate) live_provider_main_chat_invoked_count: usize,
    pub(crate) live_provider_model_invoked_count: usize,
    pub(crate) live_provider_direct_writes_executed: bool,
    pub(crate) live_provider_blockers: Vec<String>,
    pub(crate) acceptance: MainChatAgentExecutionV1AcceptanceReport,
}

pub(crate) fn build_main_chat_agent_execution_v1_final_gate_report(
    runtime_report: MainChatRuntimeEvalReport,
    command_surface_total_cases: usize,
    command_surface_evidence: MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence,
    live_provider_attempted: bool,
    live_reports: Vec<MainChatLiveProviderEvalHarnessReport>,
) -> MainChatAgentExecutionV1FinalGateReport {
    let live_provider_report_count = live_reports.len();
    let live_provider_ready_count = live_reports.iter().filter(|report| report.ready).count();
    let live_provider_main_chat_invoked_count = live_reports
        .iter()
        .filter(|report| report.main_chat_invoked)
        .count();
    let live_provider_model_invoked_count = live_reports
        .iter()
        .filter(|report| report.model_invoked)
        .count();
    let live_provider_direct_writes_executed = live_reports
        .iter()
        .any(|report| report.direct_writes_executed);
    let mut live_provider_blockers = Vec::new();
    for report in &live_reports {
        for blocker in main_chat_live_provider_report_blockers(report) {
            if !live_provider_blockers
                .iter()
                .any(|existing| existing == &blocker)
            {
                live_provider_blockers.push(blocker);
            }
        }
    }

    let live_provider = main_chat_live_provider_acceptance_evidence(&live_reports);
    let runtime_report =
        main_chat_runtime_eval_report_with_live_provider_evidence(runtime_report, &live_provider);
    let command_surface =
        command_surface_evidence_with_live_provider(command_surface_evidence, &live_provider);
    let acceptance = evaluate_main_chat_agent_execution_v1_acceptance_gate(
        MainChatAgentExecutionV1AcceptanceInput {
            runtime_report: runtime_report.clone(),
            command_surface,
            live_provider,
        },
    );

    MainChatAgentExecutionV1FinalGateReport {
        runtime_total_cases: runtime_report.total_cases,
        command_surface_total_cases,
        live_provider_attempted,
        live_provider_report_count,
        live_provider_ready_count,
        live_provider_main_chat_invoked_count,
        live_provider_model_invoked_count,
        live_provider_direct_writes_executed,
        live_provider_blockers,
        acceptance,
    }
}

pub(crate) fn command_surface_evidence_with_live_provider(
    mut evidence: MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence,
    live_provider: &MainChatAgentExecutionV1AcceptanceLiveEvidence,
) -> MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence {
    let live_provider_ready = live_provider.generation_eval_executed
        && live_provider.web_mcp_agent_loop_eval_executed
        && live_provider.web_agent_loop_eval_executed
        && live_provider.mcp_agent_loop_eval_executed
        && live_provider.proposal_permission_eval_executed
        && live_provider.no_silent_writes;

    evidence.final_completion_ready = evidence.total_cases >= 24
        && evidence.legacy_fallback_count == 0
        && evidence.silent_write_count == 0
        && evidence.send_stream_matrix_coverage >= 1.0
        && live_provider_ready;
    evidence
}

pub(crate) fn main_chat_live_provider_acceptance_evidence(
    reports: &[MainChatLiveProviderEvalHarnessReport],
) -> MainChatAgentExecutionV1AcceptanceLiveEvidence {
    let traceable_live_report = |report: &MainChatLiveProviderEvalHarnessReport| {
        report
            .run_id
            .as_ref()
            .is_some_and(|run_id| !run_id.trim().is_empty())
            && report
                .task_session_id
                .as_ref()
                .is_some_and(|task_session_id| !task_session_id.trim().is_empty())
            && report
                .response_preview
                .as_ref()
                .is_some_and(|preview| !preview.trim().is_empty())
    };
    let clean_live_report = |report: &MainChatLiveProviderEvalHarnessReport| {
        report.ready
            && report.status == "completed"
            && report.blockers.is_empty()
            && report.main_chat_invoked
            && report.live_provider_invocation_allowed
            && report.provider_endpoint_kind == "external_provider"
            && report.model_invoked
            && !report.direct_writes_executed
            && !report.legacy_fallback_used
            && traceable_live_report(report)
    };
    let generation_eval_executed = reports.iter().any(|report| {
        clean_live_report(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::DirectAnswer
            && !report.agent_loop_succeeded
    });
    let web_agent_loop_eval_executed = reports.iter().any(|report| {
        clean_live_report(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::WebAgentLoop
            && report.agent_loop_succeeded
            && !report.single_step_fallback_used
            && report.agent_loop_action_status.as_deref() == Some("succeeded")
            && !report.mcp_read_target_resolved
            && !report.tool_permission_proposal_created
    });
    let mcp_agent_loop_eval_executed = reports.iter().any(|report| {
        clean_live_report(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop
            && report.agent_loop_succeeded
            && !report.single_step_fallback_used
            && report.agent_loop_action_status.as_deref() == Some("succeeded")
            && report.mcp_read_target_resolved
    });
    let proposal_permission_eval_executed = reports.iter().any(|report| {
        clean_live_report(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal
            && report.agent_loop_succeeded
            && !report.single_step_fallback_used
            && report.agent_loop_action_status.as_deref() == Some("needs_confirmation")
            && report.tool_permission_proposal_created
    });
    let no_silent_writes = reports.iter().all(|report| !report.direct_writes_executed);

    MainChatAgentExecutionV1AcceptanceLiveEvidence {
        generation_eval_executed,
        web_mcp_agent_loop_eval_executed: web_agent_loop_eval_executed
            && mcp_agent_loop_eval_executed,
        web_agent_loop_eval_executed,
        mcp_agent_loop_eval_executed,
        proposal_permission_eval_executed,
        no_silent_writes,
    }
}

pub(crate) fn main_chat_live_provider_report_blockers(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> Vec<String> {
    let mut blockers = report.blockers.clone();
    if report.direct_writes_executed {
        push_live_provider_blocker(&mut blockers, "live_provider_direct_writes_detected");
    }
    if report.legacy_fallback_used {
        push_live_provider_blocker(&mut blockers, "live_provider_legacy_fallback_detected");
    }
    if !report
        .run_id
        .as_ref()
        .is_some_and(|run_id| !run_id.trim().is_empty())
        || !report
            .task_session_id
            .as_ref()
            .is_some_and(|task_session_id| !task_session_id.trim().is_empty())
        || !report
            .response_preview
            .as_ref()
            .is_some_and(|preview| !preview.trim().is_empty())
    {
        push_live_provider_blocker(&mut blockers, "live_provider_trace_missing");
    }
    match report.scenario {
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer => {
            if !report.ready || report.status != "completed" || !report.model_invoked {
                push_live_provider_blocker(&mut blockers, "live_provider_generation_not_completed");
            }
        }
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop => {
            if !report.ready
                || report.status != "completed"
                || !report.agent_loop_succeeded
                || report.agent_loop_action_status.as_deref() != Some("succeeded")
            {
                push_live_provider_blocker(
                    &mut blockers,
                    "live_provider_web_agent_loop_not_completed",
                );
            }
        }
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => {
            if !report.ready
                || report.status != "completed"
                || !report.agent_loop_succeeded
                || report.agent_loop_action_status.as_deref() != Some("succeeded")
                || !report.mcp_read_target_resolved
            {
                push_live_provider_blocker(
                    &mut blockers,
                    "live_provider_mcp_agent_loop_not_completed",
                );
            }
        }
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
            if !report.ready
                || report.status != "completed"
                || !report.agent_loop_succeeded
                || report.agent_loop_action_status.as_deref() != Some("needs_confirmation")
                || !report.tool_permission_proposal_created
            {
                push_live_provider_blocker(
                    &mut blockers,
                    "live_provider_proposal_permission_not_completed",
                );
            }
        }
    }
    blockers
}

fn push_live_provider_blocker(blockers: &mut Vec<String>, blocker: &str) {
    if !blockers.iter().any(|existing| existing == blocker) {
        blockers.push(blocker.to_string());
    }
}
