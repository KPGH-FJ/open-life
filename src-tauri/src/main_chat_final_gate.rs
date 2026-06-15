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
                "Use an mcp echo utility read-only tool now and call one governed MCP read candidate before answering."
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
    pub(crate) tool_permission_proposal_target: Option<String>,
    pub(crate) tool_selection_candidate_count: usize,
    pub(crate) tool_selection_candidate_ids: Vec<String>,
    pub(crate) tool_selection_allowlist: Vec<String>,
    pub(crate) tool_selection_allowed_actions: Vec<serde_json::Value>,
    pub(crate) model_selected_allowed_tool: bool,
    pub(crate) model_selected_execution_policy_validated: bool,
    pub(crate) model_selected_execution_allowed: bool,
    pub(crate) model_selected_governed_arguments: bool,
    pub(crate) model_selected_candidate_id: Option<String>,
    pub(crate) model_selected_candidate_target: Option<String>,
    pub(crate) model_selected_candidate_action_type: Option<String>,
    pub(crate) model_selected_candidate_rank: Option<usize>,
    pub(crate) model_selected_candidate_source: Option<String>,
    pub(crate) model_selected_candidate_capabilities_digest: Option<String>,
    pub(crate) model_selected_candidate_match_reason: Option<String>,
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
    let react_governance_trace = !matches!(
        scenario,
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer
    );
    let synthetic_candidate_targets: Vec<String> = match scenario {
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer => Vec::new(),
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop => vec!["web.search".into()],
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => {
            vec!["builtin_echo".into(), "tool.list_available".into()]
        }
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
            vec!["memory.search".into()]
        }
    };
    let synthetic_allowed_actions = synthetic_candidate_targets
        .iter()
        .map(|target| {
            serde_json::json!({
                "actionType": "mcp_tool",
                "target": target,
            })
        })
        .collect::<Vec<_>>();
    let selected_synthetic_target = synthetic_candidate_targets.first().cloned();

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
        tool_permission_proposal_target: matches!(
            scenario,
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal
        )
        .then(|| "memory.search".into()),
        tool_selection_candidate_count: match scenario {
            MainChatLiveProviderEvalHarnessScenario::DirectAnswer => 0,
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => 2,
            MainChatLiveProviderEvalHarnessScenario::WebAgentLoop
            | MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => 1,
        },
        tool_selection_candidate_ids: synthetic_candidate_targets.clone(),
        tool_selection_allowlist: synthetic_candidate_targets.clone(),
        tool_selection_allowed_actions: synthetic_allowed_actions,
        model_selected_allowed_tool: react_governance_trace,
        model_selected_execution_policy_validated: react_governance_trace,
        model_selected_execution_allowed: react_governance_trace,
        model_selected_governed_arguments: react_governance_trace,
        model_selected_candidate_id: react_governance_trace
            .then(|| selected_synthetic_target.clone())
            .flatten(),
        model_selected_candidate_target: react_governance_trace
            .then(|| selected_synthetic_target.clone())
            .flatten(),
        model_selected_candidate_action_type: react_governance_trace.then(|| "mcp_tool".into()),
        model_selected_candidate_rank: react_governance_trace.then_some(1),
        model_selected_candidate_source: react_governance_trace.then(|| "planned_action".into()),
        model_selected_candidate_capabilities_digest: react_governance_trace
            .then(|| "bytes:8 hash:synthetic".into()),
        model_selected_candidate_match_reason: react_governance_trace
            .then(|| "planned_action".into()),
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
    let governed_react_live_report = |report: &MainChatLiveProviderEvalHarnessReport| {
        clean_live_report(report)
            && report.tool_selection_candidate_count > 0
            && report.model_selected_allowed_tool
            && report.model_selected_execution_policy_validated
            && report.model_selected_execution_allowed
            && report.model_selected_governed_arguments
            && ranked_manifest_live_trace_present(report)
            && candidate_allowlist_live_trace_present(report)
    };
    let generation_eval_executed = reports.iter().any(|report| {
        clean_live_report(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::DirectAnswer
            && !report.agent_loop_succeeded
    });
    let web_agent_loop_eval_executed = reports.iter().any(|report| {
        governed_react_live_report(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::WebAgentLoop
            && report.agent_loop_succeeded
            && !report.single_step_fallback_used
            && report.agent_loop_action_status.as_deref() == Some("succeeded")
            && !report.mcp_read_target_resolved
            && !report.tool_permission_proposal_created
            && web_agent_loop_target_trace_present(report)
    });
    let mcp_agent_loop_eval_executed = reports.iter().any(|report| {
        governed_react_live_report(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop
            && report.agent_loop_succeeded
            && !report.single_step_fallback_used
            && report.agent_loop_action_status.as_deref() == Some("succeeded")
            && report.mcp_read_target_resolved
            && registered_mcp_distinct_candidate_trace_present(report)
    });
    let proposal_permission_eval_executed = reports.iter().any(|report| {
        governed_react_live_report(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal
            && report.agent_loop_succeeded
            && !report.single_step_fallback_used
            && report.agent_loop_action_status.as_deref() == Some("needs_confirmation")
            && report.tool_permission_proposal_created
            && proposal_permission_target_trace_present(report)
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
    if live_provider_react_governance_trace_missing(report) {
        push_live_provider_blocker(&mut blockers, "live_provider_tool_selection_trace_missing");
    }
    if live_provider_ranked_manifest_trace_missing(report) {
        push_live_provider_blocker(&mut blockers, "live_provider_ranked_manifest_trace_missing");
    }
    if live_provider_candidate_allowlist_trace_missing(report) {
        push_live_provider_blocker(
            &mut blockers,
            "live_provider_candidate_allowlist_trace_missing",
        );
    }
    if live_provider_model_ranked_mcp_candidate_trace_missing(report) {
        push_live_provider_blocker(
            &mut blockers,
            "live_provider_model_ranked_mcp_candidate_trace_missing",
        );
    }
    if live_provider_web_tool_target_trace_missing(report) {
        push_live_provider_blocker(&mut blockers, "live_provider_web_tool_target_trace_missing");
    }
    if live_provider_proposal_permission_target_trace_missing(report) {
        push_live_provider_blocker(
            &mut blockers,
            "live_provider_proposal_permission_target_trace_missing",
        );
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

fn live_provider_react_governance_trace_missing(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    if matches!(
        report.scenario,
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer
    ) {
        return false;
    }
    report.tool_selection_candidate_count == 0
        || !report.model_selected_allowed_tool
        || !report.model_selected_execution_policy_validated
        || !report.model_selected_execution_allowed
        || !report.model_selected_governed_arguments
}

fn live_provider_ranked_manifest_trace_missing(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    if matches!(
        report.scenario,
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer
    ) {
        return false;
    }
    !ranked_manifest_live_trace_present(report)
}

fn live_provider_candidate_allowlist_trace_missing(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    if matches!(
        report.scenario,
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer
    ) {
        return false;
    }
    !candidate_allowlist_live_trace_present(report)
}

fn live_provider_model_ranked_mcp_candidate_trace_missing(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    report.scenario == MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop
        && !registered_mcp_distinct_candidate_trace_present(report)
}

fn live_provider_web_tool_target_trace_missing(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    report.scenario == MainChatLiveProviderEvalHarnessScenario::WebAgentLoop
        && !web_agent_loop_target_trace_present(report)
}

fn live_provider_proposal_permission_target_trace_missing(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    report.scenario == MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal
        && !proposal_permission_target_trace_present(report)
}

fn ranked_manifest_live_trace_present(report: &MainChatLiveProviderEvalHarnessReport) -> bool {
    report
        .model_selected_candidate_rank
        .is_some_and(|rank| rank > 0)
        && report
            .model_selected_candidate_source
            .as_ref()
            .is_some_and(|source| !source.trim().is_empty())
        && report
            .model_selected_candidate_capabilities_digest
            .as_ref()
            .is_some_and(|digest| !digest.trim().is_empty())
        && report
            .model_selected_candidate_match_reason
            .as_ref()
            .is_some_and(|reason| !reason.trim().is_empty())
}

fn web_agent_loop_target_trace_present(report: &MainChatLiveProviderEvalHarnessReport) -> bool {
    if report.scenario != MainChatLiveProviderEvalHarnessScenario::WebAgentLoop {
        return true;
    }
    let selected_id = match report.model_selected_candidate_id.as_deref() {
        Some(id) if id.starts_with("web.") => id,
        _ => return false,
    };
    let selected_target = match report.model_selected_candidate_target.as_deref() {
        Some(target) if target.starts_with("web.") => target,
        _ => return false,
    };
    let selected_action_type = match report.model_selected_candidate_action_type.as_deref() {
        Some(action_type) if !action_type.trim().is_empty() => action_type,
        _ => return false,
    };

    report
        .tool_selection_candidate_ids
        .iter()
        .any(|candidate_id| candidate_id == selected_id)
        && report
            .tool_selection_allowlist
            .iter()
            .any(|target| target == selected_target && target.starts_with("web."))
        && report.tool_selection_allowed_actions.iter().any(|action| {
            action.get("actionType").and_then(serde_json::Value::as_str)
                == Some(selected_action_type)
                && action
                    .get("target")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|target| target == selected_target && target.starts_with("web."))
        })
}

fn proposal_permission_target_trace_present(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    if report.scenario != MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal {
        return true;
    }
    let proposal_target = match report.tool_permission_proposal_target.as_deref() {
        Some(target)
            if !target.trim().is_empty()
                && !target.starts_with("web.")
                && !target.starts_with("file.") =>
        {
            target
        }
        _ => return false,
    };
    let selected_id = match report.model_selected_candidate_id.as_deref() {
        Some(id) if !id.trim().is_empty() => id,
        _ => return false,
    };
    let selected_target = match report.model_selected_candidate_target.as_deref() {
        Some(target) if target == proposal_target => target,
        _ => return false,
    };
    let selected_action_type = match report.model_selected_candidate_action_type.as_deref() {
        Some("mcp_tool") => "mcp_tool",
        _ => return false,
    };

    report
        .tool_selection_candidate_ids
        .iter()
        .any(|candidate_id| candidate_id == selected_id)
        && report
            .tool_selection_allowlist
            .iter()
            .any(|target| target == selected_target)
        && report.tool_selection_allowed_actions.iter().any(|action| {
            action.get("actionType").and_then(serde_json::Value::as_str)
                == Some(selected_action_type)
                && action.get("target").and_then(serde_json::Value::as_str) == Some(selected_target)
        })
}

fn candidate_allowlist_live_trace_present(report: &MainChatLiveProviderEvalHarnessReport) -> bool {
    let selected_id = match report.model_selected_candidate_id.as_deref() {
        Some(id) if !id.trim().is_empty() => id,
        _ => return false,
    };
    let selected_target = match report.model_selected_candidate_target.as_deref() {
        Some(target) if !target.trim().is_empty() => target,
        _ => return false,
    };
    let selected_action_type = match report.model_selected_candidate_action_type.as_deref() {
        Some(action_type) if !action_type.trim().is_empty() => action_type,
        _ => return false,
    };

    report.tool_selection_candidate_count > 0
        && report.tool_selection_candidate_ids.len() == report.tool_selection_candidate_count
        && report
            .tool_selection_candidate_ids
            .iter()
            .any(|candidate_id| candidate_id == selected_id)
        && report
            .tool_selection_allowlist
            .iter()
            .any(|target| target == selected_target)
        && report.tool_selection_allowed_actions.iter().any(|action| {
            action.get("actionType").and_then(serde_json::Value::as_str)
                == Some(selected_action_type)
                && action.get("target").and_then(serde_json::Value::as_str) == Some(selected_target)
        })
}

fn registered_mcp_distinct_candidate_trace_present(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    if report.scenario != MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop {
        return true;
    }
    let distinct_candidate_ids = report
        .tool_selection_candidate_ids
        .iter()
        .filter(|candidate_id| !candidate_id.trim().is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_allowed_targets = report
        .tool_selection_allowlist
        .iter()
        .filter(|target| !target.trim().is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_allowed_action_pairs = report
        .tool_selection_allowed_actions
        .iter()
        .filter_map(|action| {
            let action_type = action.get("actionType")?.as_str()?.trim();
            let target = action.get("target")?.as_str()?.trim();
            if action_type.is_empty() || target.is_empty() {
                return None;
            }
            Some((action_type, target))
        })
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    report.tool_selection_candidate_count >= 2
        && distinct_candidate_ids >= 2
        && distinct_allowed_targets >= 2
        && distinct_allowed_action_pairs >= 2
}
