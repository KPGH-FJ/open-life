use openlife_core::agent::main_chat_agent_v1::{
    evaluate_main_chat_agent_execution_v1_acceptance_gate,
    main_chat_runtime_eval_report_with_live_provider_evidence,
    MainChatAgentExecutionV1AcceptanceCommandSurfaceEvidence,
    MainChatAgentExecutionV1AcceptanceInput, MainChatAgentExecutionV1AcceptanceLiveEvidence,
    MainChatAgentExecutionV1AcceptanceReport, MainChatRuntimeEvalReport,
};
use serde::Serialize;

const SYNTHETIC_METADATA_SAFE_DIGEST_LABEL: &str =
    "bytes:8 hash:sha256:0000000000000000000000000000000000000000000000000000000000000000";
const MAIN_CHAT_LIVE_PROVIDER_CONTRACT_SAFE_LABEL_MAX_LEN: usize = 96;
const MAIN_CHAT_LIVE_PROVIDER_RESPONSE_PREVIEW_MAX_CHARS: usize = 240;
const MAIN_CHAT_LIVE_PROVIDER_WRITE_LIKE_LABEL_TERMS: &[&str] = &[
    "write",
    "send",
    "delete",
    "remove",
    "update",
    "create",
    "modify",
    "mutate",
    "externalwrite",
    "externalsideeffect",
    "realwrite",
    "emailsend",
    "calendarsend",
    "calendarwrite",
    "providerwrite",
    "shellexec",
    "execute",
    "exec",
];

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
                "For this live eval, call the allowed web.search candidate exactly once before answering. Return only a JSON action envelope with actions[0].name=\"web.search\", actions[0].action_type=\"mcp_tool\", and actions[0].arguments={}; do not answer directly."
            }
            MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => {
                "For candidate ranking, if the candidate ids include builtin_echo, put builtin_echo first and include every allowed candidate id exactly once. For action execution, do not explain or answer directly. Return exactly this JSON envelope and nothing else: {\"final\":\"pending governed read\",\"actions\":[{\"name\":\"builtin_echo\",\"action_type\":\"mcp_tool\",\"arguments\":{}}],\"thought_summary\":\"Need a governed read.\",\"warnings\":[]}. This is a governed mcp read-only utility request."
            }
            MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
                "Use mcp memory.search now. For this live eval, do not explain or answer directly. Return exactly this JSON envelope and nothing else: {\"final\":\"pending governed read\",\"actions\":[{\"name\":\"memory.search\",\"action_type\":\"mcp_tool\",\"arguments\":{}}],\"thought_summary\":\"Need a governed MCP read.\",\"warnings\":[]}."
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
    pub(crate) provider_model: Option<String>,
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
    pub(crate) tool_selection_model_ranked: bool,
    pub(crate) tool_selection_ranking_source: Option<String>,
    pub(crate) tool_selection_ranking_provider: Option<String>,
    pub(crate) tool_selection_ranking_model: Option<String>,
    pub(crate) tool_selection_ranking_route_type: Option<String>,
    pub(crate) tool_selection_ranking_provider_backed: bool,
    pub(crate) tool_selection_model_ranking_ignored: bool,
    pub(crate) tool_selection_model_ranking_candidate_ids: Vec<String>,
    pub(crate) tool_selection_model_ranking_response_digest: Option<String>,
    pub(crate) model_selected_allowed_tool: bool,
    pub(crate) model_selected_execution_policy_validated: bool,
    pub(crate) model_selected_execution_allowed: bool,
    pub(crate) model_selected_governed_arguments: bool,
    pub(crate) model_selected_governed_arguments_digest: Option<String>,
    pub(crate) model_selected_candidate_id: Option<String>,
    pub(crate) model_selected_candidate_target: Option<String>,
    pub(crate) model_selected_candidate_action_type: Option<String>,
    pub(crate) model_selected_candidate_rank: Option<usize>,
    pub(crate) model_selected_candidate_source: Option<String>,
    pub(crate) model_selected_candidate_capabilities_digest: Option<String>,
    pub(crate) model_selected_candidate_capability_labels: Option<String>,
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
        provider_model: None,
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
    let synthetic_provider_ranked = matches!(
        scenario,
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop
    );

    MainChatLiveProviderEvalHarnessReport {
        scenario,
        ready: true,
        status: "completed".into(),
        provider: provider.into(),
        provider_model: Some("gpt-live-eval".into()),
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
        tool_selection_model_ranked: synthetic_provider_ranked,
        tool_selection_ranking_source: synthetic_provider_ranked.then(|| "provider_model".into()),
        tool_selection_ranking_provider: synthetic_provider_ranked.then(|| "openai".into()),
        tool_selection_ranking_model: synthetic_provider_ranked.then(|| "gpt-live-eval".into()),
        tool_selection_ranking_route_type: synthetic_provider_ranked.then(|| "cloud".into()),
        tool_selection_ranking_provider_backed: synthetic_provider_ranked,
        tool_selection_model_ranking_ignored: false,
        tool_selection_model_ranking_candidate_ids: if synthetic_provider_ranked {
            synthetic_candidate_targets.clone()
        } else {
            Vec::new()
        },
        tool_selection_model_ranking_response_digest: synthetic_provider_ranked
            .then(|| SYNTHETIC_METADATA_SAFE_DIGEST_LABEL.into()),
        model_selected_allowed_tool: react_governance_trace,
        model_selected_execution_policy_validated: react_governance_trace,
        model_selected_execution_allowed: react_governance_trace,
        model_selected_governed_arguments: react_governance_trace,
        model_selected_governed_arguments_digest: react_governance_trace
            .then(|| SYNTHETIC_METADATA_SAFE_DIGEST_LABEL.into()),
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
            .then(|| SYNTHETIC_METADATA_SAFE_DIGEST_LABEL.into()),
        model_selected_candidate_capability_labels: react_governance_trace.then(|| "read".into()),
        model_selected_candidate_match_reason: react_governance_trace.then(|| {
            if synthetic_provider_ranked {
                "provider_model_ranked".into()
            } else {
                "planned_action".into()
            }
        }),
        run_id: Some(run_id.into()),
        task_session_id: Some(task_session_id.into()),
        response_preview: Some(response_preview.into()),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatLiveProviderScenarioReport {
    pub(crate) scenario: String,
    pub(crate) ready: bool,
    pub(crate) credited: bool,
    pub(crate) status: String,
    pub(crate) provider_endpoint_kind: String,
    pub(crate) blockers: Vec<String>,
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
    pub(crate) tool_selection_candidate_count: usize,
    pub(crate) model_selected_allowed_tool: bool,
    pub(crate) model_selected_execution_policy_validated: bool,
    pub(crate) model_selected_execution_allowed: bool,
    pub(crate) model_selected_governed_arguments: bool,
    pub(crate) model_selected_candidate_id: Option<String>,
    pub(crate) model_selected_candidate_target: Option<String>,
    pub(crate) run_id_present: bool,
    pub(crate) task_session_id_present: bool,
    pub(crate) response_preview_present: bool,
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
    pub(crate) live_provider_scenario_reports: Vec<MainChatLiveProviderScenarioReport>,
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
    let live_provider_ready_count = if live_provider_attempted {
        live_reports
            .iter()
            .filter(|report| live_provider_report_has_creditable_scenario_evidence(report))
            .count()
    } else {
        0
    };
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
    let live_provider_scenario_reports = main_chat_live_provider_scenario_reports(&live_reports);
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
    if !live_provider_attempted && !live_reports.is_empty() {
        push_live_provider_blocker(
            &mut live_provider_blockers,
            "live_provider_reports_without_attempt",
        );
    }

    let live_provider = if live_provider_attempted {
        main_chat_live_provider_acceptance_evidence(&live_reports)
    } else {
        MainChatAgentExecutionV1AcceptanceLiveEvidence {
            generation_eval_executed: false,
            web_mcp_agent_loop_eval_executed: false,
            web_agent_loop_eval_executed: false,
            mcp_agent_loop_eval_executed: false,
            proposal_permission_eval_executed: false,
            no_silent_writes: !live_provider_direct_writes_executed,
        }
    };
    push_live_provider_evidence_blockers(&mut live_provider_blockers, &live_provider);
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
        live_provider_scenario_reports,
        acceptance,
    }
}

fn main_chat_live_provider_scenario_reports(
    reports: &[MainChatLiveProviderEvalHarnessReport],
) -> Vec<MainChatLiveProviderScenarioReport> {
    reports
        .iter()
        .map(|report| MainChatLiveProviderScenarioReport {
            scenario: report.scenario.as_str().to_string(),
            ready: report.ready,
            credited: live_provider_report_has_creditable_scenario_evidence(report),
            status: live_provider_summary_label(&report.status)
                .unwrap_or_else(|| "contract_unsafe_status".into()),
            provider_endpoint_kind: live_provider_summary_label(&report.provider_endpoint_kind)
                .unwrap_or_else(|| "contract_unsafe_endpoint_kind".into()),
            blockers: main_chat_live_provider_report_blockers(report)
                .into_iter()
                .map(live_provider_summary_blocker_label)
                .collect(),
            live_provider_invocation_allowed: report.live_provider_invocation_allowed,
            main_chat_invoked: report.main_chat_invoked,
            model_invoked: report.model_invoked,
            direct_writes_executed: report.direct_writes_executed,
            legacy_fallback_used: report.legacy_fallback_used,
            agent_loop_succeeded: report.agent_loop_succeeded,
            single_step_fallback_used: report.single_step_fallback_used,
            agent_loop_action_status: report
                .agent_loop_action_status
                .as_deref()
                .and_then(live_provider_summary_label),
            mcp_read_target_resolved: report.mcp_read_target_resolved,
            tool_permission_proposal_created: report.tool_permission_proposal_created,
            tool_selection_candidate_count: report.tool_selection_candidate_count,
            model_selected_allowed_tool: report.model_selected_allowed_tool,
            model_selected_execution_policy_validated: report
                .model_selected_execution_policy_validated,
            model_selected_execution_allowed: report.model_selected_execution_allowed,
            model_selected_governed_arguments: report.model_selected_governed_arguments,
            model_selected_candidate_id: report
                .model_selected_candidate_id
                .as_deref()
                .and_then(live_provider_summary_label),
            model_selected_candidate_target: report
                .model_selected_candidate_target
                .as_deref()
                .and_then(live_provider_summary_label),
            run_id_present: report.run_id.is_some(),
            task_session_id_present: report.task_session_id.is_some(),
            response_preview_present: report.response_preview.is_some(),
        })
        .collect()
}

fn live_provider_summary_label(value: &str) -> Option<String> {
    live_provider_contract_safe_label(value).then(|| value.to_string())
}

fn live_provider_summary_blocker_label(blocker: String) -> String {
    if live_provider_contract_safe_label(&blocker) {
        return blocker;
    }
    let (bytes, hash) = openlife_core::agent::react_beta::metadata_safe_text_digest(&blocker);
    let hash = hash.strip_prefix("sha256:").unwrap_or(hash.as_str());
    format!("unsafe_blocker_bytes_{bytes}_sha256_{hash}")
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

    evidence.final_completion_ready = evidence.total_cases >= 38
        && evidence.legacy_fallback_count == 0
        && evidence.silent_write_count == 0
        && evidence.send_stream_matrix_coverage >= 1.0
        && live_provider_ready;
    evidence
}

pub(crate) fn main_chat_live_provider_acceptance_evidence(
    reports: &[MainChatLiveProviderEvalHarnessReport],
) -> MainChatAgentExecutionV1AcceptanceLiveEvidence {
    let generation_eval_executed = reports.iter().any(|report| {
        live_provider_report_is_clean(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::DirectAnswer
            && direct_answer_generation_trace_present(report)
    });
    let web_agent_loop_eval_executed = reports.iter().any(|report| {
        live_provider_governed_react_report_is_clean(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::WebAgentLoop
            && report.agent_loop_succeeded
            && !report.single_step_fallback_used
            && report.agent_loop_action_status.as_deref() == Some("succeeded")
            && !report.mcp_read_target_resolved
            && !report.tool_permission_proposal_created
            && report.tool_permission_proposal_target.is_none()
            && web_agent_loop_target_trace_present(report)
    });
    let mcp_agent_loop_eval_executed = reports.iter().any(|report| {
        live_provider_governed_react_report_is_clean(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop
            && report.agent_loop_succeeded
            && !report.single_step_fallback_used
            && report.agent_loop_action_status.as_deref() == Some("succeeded")
            && report.mcp_read_target_resolved
            && !report.tool_permission_proposal_created
            && report.tool_permission_proposal_target.is_none()
            && registered_mcp_distinct_candidate_trace_present(report)
            && registered_mcp_provider_ranked_selection_trace_present(report)
    });
    let proposal_permission_eval_executed = reports.iter().any(|report| {
        live_provider_governed_react_report_is_clean(report)
            && report.scenario == MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal
            && report.agent_loop_succeeded
            && !report.single_step_fallback_used
            && report.agent_loop_action_status.as_deref() == Some("needs_confirmation")
            && !report.mcp_read_target_resolved
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

fn live_provider_report_has_creditable_scenario_evidence(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    match report.scenario {
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer => {
            live_provider_report_is_clean(report) && direct_answer_generation_trace_present(report)
        }
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop => {
            live_provider_governed_react_report_is_clean(report)
                && report.agent_loop_succeeded
                && !report.single_step_fallback_used
                && report.agent_loop_action_status.as_deref() == Some("succeeded")
                && !report.mcp_read_target_resolved
                && !report.tool_permission_proposal_created
                && report.tool_permission_proposal_target.is_none()
                && web_agent_loop_target_trace_present(report)
        }
        MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop => {
            live_provider_governed_react_report_is_clean(report)
                && report.agent_loop_succeeded
                && !report.single_step_fallback_used
                && report.agent_loop_action_status.as_deref() == Some("succeeded")
                && report.mcp_read_target_resolved
                && !report.tool_permission_proposal_created
                && report.tool_permission_proposal_target.is_none()
                && registered_mcp_distinct_candidate_trace_present(report)
                && registered_mcp_provider_ranked_selection_trace_present(report)
        }
        MainChatLiveProviderEvalHarnessScenario::McpToolPermissionProposal => {
            live_provider_governed_react_report_is_clean(report)
                && report.agent_loop_succeeded
                && !report.single_step_fallback_used
                && report.agent_loop_action_status.as_deref() == Some("needs_confirmation")
                && !report.mcp_read_target_resolved
                && report.tool_permission_proposal_created
                && proposal_permission_target_trace_present(report)
        }
    }
}

fn live_provider_report_is_clean(report: &MainChatLiveProviderEvalHarnessReport) -> bool {
    report.ready
        && report.status == "completed"
        && report.blockers.is_empty()
        && report.main_chat_invoked
        && report.live_provider_invocation_allowed
        && live_provider_external_provider_trace_present(report)
        && report.model_invoked
        && live_provider_model_identity_trace_present(report)
        && live_provider_required_evidence_trace_present(report)
        && !report.direct_writes_executed
        && !report.legacy_fallback_used
        && live_provider_traceable_report_present(report)
}

fn live_provider_governed_react_report_is_clean(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    live_provider_report_is_clean(report)
        && report.tool_selection_candidate_count > 0
        && report.model_selected_allowed_tool
        && report.model_selected_execution_policy_validated
        && report.model_selected_execution_allowed
        && governed_arguments_live_trace_present(report)
        && ranked_manifest_live_trace_present(report)
        && candidate_allowlist_live_trace_present(report)
}

pub(crate) fn main_chat_live_provider_report_blockers(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> Vec<String> {
    let mut blockers = report.blockers.clone();
    let claims_live_completion = report.ready || report.status == "completed";
    if claims_live_completion && !report.live_provider_invocation_allowed {
        push_live_provider_blocker(&mut blockers, "live_provider_invocation_not_allowed");
    }
    if claims_live_completion && !report.main_chat_invoked {
        push_live_provider_blocker(&mut blockers, "live_provider_main_chat_not_invoked");
    }
    if claims_live_completion && !report.model_invoked {
        push_live_provider_blocker(&mut blockers, "live_provider_model_not_invoked");
    }
    if (claims_live_completion || report.model_invoked)
        && !live_provider_model_identity_trace_present(report)
    {
        push_live_provider_blocker(&mut blockers, "live_provider_model_identity_missing");
    }
    if claims_live_completion && !live_provider_required_evidence_trace_present(report) {
        push_live_provider_blocker(&mut blockers, "live_provider_required_evidence_missing");
    }
    if report.direct_writes_executed {
        push_live_provider_blocker(&mut blockers, "live_provider_direct_writes_detected");
    }
    if report.legacy_fallback_used {
        push_live_provider_blocker(&mut blockers, "live_provider_legacy_fallback_detected");
    }
    if !live_provider_external_provider_trace_present(report) {
        push_live_provider_blocker(&mut blockers, "live_provider_external_provider_missing");
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
    if live_provider_model_ranked_selection_trace_missing(report) {
        push_live_provider_blocker(
            &mut blockers,
            "live_provider_model_ranked_selection_trace_missing",
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
    if !live_provider_traceable_report_present(report) {
        push_live_provider_blocker(&mut blockers, "live_provider_trace_missing");
    }
    match report.scenario {
        MainChatLiveProviderEvalHarnessScenario::DirectAnswer => {
            if !report.ready
                || report.status != "completed"
                || !report.model_invoked
                || !direct_answer_generation_trace_present(report)
            {
                push_live_provider_blocker(&mut blockers, "live_provider_generation_not_completed");
            }
        }
        MainChatLiveProviderEvalHarnessScenario::WebAgentLoop => {
            if !report.ready
                || report.status != "completed"
                || !report.agent_loop_succeeded
                || report.single_step_fallback_used
                || report.agent_loop_action_status.as_deref() != Some("succeeded")
                || report.mcp_read_target_resolved
                || report.tool_permission_proposal_created
                || report.tool_permission_proposal_target.is_some()
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
                || report.single_step_fallback_used
                || report.agent_loop_action_status.as_deref() != Some("succeeded")
                || !report.mcp_read_target_resolved
                || report.tool_permission_proposal_created
                || report.tool_permission_proposal_target.is_some()
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
                || report.single_step_fallback_used
                || report.agent_loop_action_status.as_deref() != Some("needs_confirmation")
                || report.mcp_read_target_resolved
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

fn push_live_provider_evidence_blockers(
    blockers: &mut Vec<String>,
    live: &MainChatAgentExecutionV1AcceptanceLiveEvidence,
) {
    if !live.generation_eval_executed {
        push_live_provider_blocker(blockers, "live_provider_generation_not_executed");
    }
    if !live.web_mcp_agent_loop_eval_executed {
        push_live_provider_blocker(blockers, "provider_backed_web_mcp_agent_loop_not_executed");
    }
    if !live.web_agent_loop_eval_executed {
        push_live_provider_blocker(blockers, "provider_backed_web_agent_loop_not_executed");
    }
    if !live.mcp_agent_loop_eval_executed {
        push_live_provider_blocker(blockers, "provider_backed_mcp_agent_loop_not_executed");
    }
    if !live.proposal_permission_eval_executed {
        push_live_provider_blocker(blockers, "provider_live_proposal_permission_not_executed");
    }
    if !live.no_silent_writes {
        push_live_provider_blocker(blockers, "live_provider_silent_writes_detected");
    }
}

fn live_provider_external_provider_trace_present(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    if report.provider_endpoint_kind != "external_provider" {
        return false;
    }
    normalized_external_provider_label(&report.provider).is_some()
}

fn live_provider_traceable_report_present(report: &MainChatLiveProviderEvalHarnessReport) -> bool {
    report
        .run_id
        .as_ref()
        .is_some_and(|run_id| live_provider_contract_safe_label(run_id))
        && report
            .task_session_id
            .as_ref()
            .is_some_and(|task_session_id| live_provider_contract_safe_label(task_session_id))
        && report
            .response_preview
            .as_ref()
            .is_some_and(|preview| live_provider_response_preview_trace_present(preview))
}

fn live_provider_response_preview_trace_present(preview: &str) -> bool {
    let normalized_preview = preview.split_whitespace().collect::<Vec<_>>().join(" ");
    !preview.is_empty()
        && normalized_preview == preview
        && preview.chars().count() <= MAIN_CHAT_LIVE_PROVIDER_RESPONSE_PREVIEW_MAX_CHARS
        && preview.chars().all(|ch| !ch.is_control())
}

fn normalized_external_provider_label(provider: &str) -> Option<String> {
    if !live_provider_contract_safe_label(provider) {
        return None;
    }
    let provider = provider.to_ascii_lowercase();
    if matches!(
        provider.as_str(),
        "" | "none"
            | "ollama"
            | "local"
            | "localhost"
            | "127.0.0.1"
            | "::1"
            | "0.0.0.0"
            | "local_test_http"
            | "local-test-http"
            | "local_http"
            | "local-http"
            | "mock"
            | "fixture"
            | "synthetic"
            | "scripted"
    ) {
        return None;
    }
    if provider_label_is_local_network_alias(&provider) {
        return None;
    }
    let has_local_token = provider
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| {
            matches!(
                token,
                "local" | "localhost" | "mock" | "fixture" | "synthetic" | "scripted"
            )
        });
    if has_local_token {
        return None;
    }
    if provider_label_has_embedded_synthetic_provider_alias(&provider) {
        return None;
    }
    Some(provider)
}

fn provider_label_has_embedded_synthetic_provider_alias(provider: &str) -> bool {
    [
        "ollama",
        "local",
        "localhost",
        "mock",
        "fixture",
        "synthetic",
        "scripted",
    ]
    .iter()
    .any(|alias| provider.contains(alias))
}

fn provider_label_is_local_network_alias(provider: &str) -> bool {
    let normalized = provider
        .chars()
        .map(|ch| {
            if matches!(ch, '-' | '_' | '/') {
                '.'
            } else {
                ch
            }
        })
        .collect::<String>();
    let parts = normalized.split('.').collect::<Vec<_>>();
    if parts.len() < 4 {
        return false;
    }
    parts.windows(4).any(|octets| {
        if octets
            .iter()
            .any(|octet| octet.is_empty() || !octet.chars().all(|ch| ch.is_ascii_digit()))
        {
            return false;
        }
        let Some(first) = octets.first().and_then(|octet| octet.parse::<u8>().ok()) else {
            return false;
        };
        let Some(second) = octets.get(1).and_then(|octet| octet.parse::<u8>().ok()) else {
            return false;
        };

        first == 0
            || first == 10
            || first == 127
            || (first == 169 && second == 254)
            || (first == 172 && (16..=31).contains(&second))
            || (first == 192 && second == 168)
    }) || provider_label_has_embedded_local_network_alias(provider)
}

fn provider_label_has_embedded_local_network_alias(provider: &str) -> bool {
    let mut octets = Vec::new();
    let mut current = String::new();
    for ch in provider.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(octet) = current.parse::<u16>() {
                octets.push(octet);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(octet) = current.parse::<u16>() {
            octets.push(octet);
        }
    }

    octets.windows(4).any(|window| {
        if window.iter().any(|octet| *octet > 255) {
            return false;
        }
        let first = window[0];
        let second = window[1];

        first == 0
            || first == 10
            || first == 127
            || (first == 169 && second == 254)
            || (first == 172 && (16..=31).contains(&second))
            || (first == 192 && second == 168)
    })
}

fn live_provider_model_identity_trace_present(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    report
        .provider_model
        .as_ref()
        .is_some_and(|model| live_provider_contract_safe_label(model))
}

fn live_provider_required_evidence_trace_present(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    let required_evidence = report
        .required_evidence
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let expected_required_evidence = main_chat_live_provider_required_evidence()
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();

    report.required_evidence.len() == expected_required_evidence.len()
        && required_evidence == expected_required_evidence
        && report
            .required_evidence
            .iter()
            .all(|evidence| live_provider_contract_safe_label(evidence))
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
        || !governed_arguments_live_trace_present(report)
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

fn live_provider_model_ranked_selection_trace_missing(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    report.scenario == MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop
        && !registered_mcp_provider_ranked_selection_trace_present(report)
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
        && selected_candidate_rank_matches_candidate_order(
            &report.tool_selection_candidate_ids,
            report.model_selected_candidate_id.as_deref(),
            report.model_selected_candidate_rank,
        )
        && report
            .model_selected_candidate_source
            .as_ref()
            .is_some_and(|source| live_provider_contract_safe_label(source))
        && report
            .model_selected_candidate_capabilities_digest
            .as_ref()
            .is_some_and(|digest| metadata_safe_digest_label_present(digest))
        && report
            .model_selected_candidate_capability_labels
            .as_ref()
            .is_some_and(|labels| live_provider_capability_labels_trace_present(labels))
        && report
            .model_selected_candidate_match_reason
            .as_ref()
            .is_some_and(|reason| live_provider_contract_safe_label(reason))
}

fn live_provider_capability_labels_trace_present(labels: &str) -> bool {
    live_provider_contract_safe_label(labels)
        && labels != "none"
        && labels
            .split('/')
            .any(|label| label.eq_ignore_ascii_case("read"))
        && labels.split('/').all(|label| {
            live_provider_contract_safe_label(label)
                && !live_provider_write_like_capability_label(label)
        })
}

fn live_provider_write_like_capability_label(label: &str) -> bool {
    let label = label.to_ascii_lowercase();
    MAIN_CHAT_LIVE_PROVIDER_WRITE_LIKE_LABEL_TERMS
        .iter()
        .any(|term| label.contains(term))
        || label.ends_with("write")
        || label.ends_with("send")
        || label.ends_with("delete")
}

fn selected_candidate_rank_matches_candidate_order(
    candidate_ids: &[String],
    selected_candidate_id: Option<&str>,
    selected_rank: Option<usize>,
) -> bool {
    let selected_candidate_id = match selected_candidate_id {
        Some(candidate_id) if live_provider_contract_safe_label(candidate_id) => candidate_id,
        _ => return false,
    };
    let Some(selected_rank) = selected_rank.filter(|rank| *rank > 0) else {
        return false;
    };

    candidate_ids
        .iter()
        .position(|candidate_id| candidate_id == selected_candidate_id)
        .is_some_and(|index| index + 1 == selected_rank)
}

fn direct_answer_generation_trace_present(report: &MainChatLiveProviderEvalHarnessReport) -> bool {
    if report.scenario != MainChatLiveProviderEvalHarnessScenario::DirectAnswer {
        return true;
    }
    !report.agent_loop_succeeded
        && !report.single_step_fallback_used
        && report.agent_loop_action_status.is_none()
        && !report.mcp_read_target_resolved
        && !report.tool_permission_proposal_created
        && report.tool_permission_proposal_target.is_none()
        && report.tool_selection_candidate_count == 0
        && report.tool_selection_candidate_ids.is_empty()
        && report.tool_selection_allowlist.is_empty()
        && report.tool_selection_allowed_actions.is_empty()
        && !report.tool_selection_model_ranked
        && report.tool_selection_ranking_source.is_none()
        && report.tool_selection_ranking_provider.is_none()
        && report.tool_selection_ranking_model.is_none()
        && report.tool_selection_ranking_route_type.is_none()
        && !report.tool_selection_ranking_provider_backed
        && !report.tool_selection_model_ranking_ignored
        && report.tool_selection_model_ranking_candidate_ids.is_empty()
        && report
            .tool_selection_model_ranking_response_digest
            .is_none()
        && !report.model_selected_allowed_tool
        && !report.model_selected_execution_policy_validated
        && !report.model_selected_execution_allowed
        && !report.model_selected_governed_arguments
        && report.model_selected_governed_arguments_digest.is_none()
        && report.model_selected_candidate_id.is_none()
        && report.model_selected_candidate_target.is_none()
        && report.model_selected_candidate_action_type.is_none()
        && report.model_selected_candidate_rank.is_none()
        && report.model_selected_candidate_source.is_none()
        && report
            .model_selected_candidate_capabilities_digest
            .is_none()
        && report.model_selected_candidate_capability_labels.is_none()
        && report.model_selected_candidate_match_reason.is_none()
}

fn governed_arguments_live_trace_present(report: &MainChatLiveProviderEvalHarnessReport) -> bool {
    report.model_selected_governed_arguments
        && report
            .model_selected_governed_arguments_digest
            .as_ref()
            .is_some_and(|digest| metadata_safe_digest_label_present(digest))
}

fn web_agent_loop_target_trace_present(report: &MainChatLiveProviderEvalHarnessReport) -> bool {
    if report.scenario != MainChatLiveProviderEvalHarnessScenario::WebAgentLoop {
        return true;
    }
    let selected_id = match report.model_selected_candidate_id.as_deref() {
        Some(id) if id.starts_with("web.") && live_provider_contract_safe_label(id) => id,
        _ => return false,
    };
    let selected_target = match report.model_selected_candidate_target.as_deref() {
        Some(target) if target.starts_with("web.") && live_provider_contract_safe_label(target) => {
            target
        }
        _ => return false,
    };
    if selected_id != selected_target {
        return false;
    }
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
            .any(|target| target == selected_target && target.starts_with("web."))
        && report
            .tool_selection_allowed_actions
            .iter()
            .filter_map(allowed_action_exact_pair)
            .any(|(action_type, target)| {
                action_type == selected_action_type
                    && target == selected_target
                    && target.starts_with("web.")
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
                && live_provider_contract_safe_label(target)
                && !target.starts_with("web.")
                && !target.starts_with("file.") =>
        {
            target
        }
        _ => return false,
    };
    let selected_id = match report.model_selected_candidate_id.as_deref() {
        Some(id) if live_provider_contract_safe_label(id) => id,
        _ => return false,
    };
    let selected_target = match report.model_selected_candidate_target.as_deref() {
        Some(target) if target == proposal_target => target,
        _ => return false,
    };
    if selected_id != selected_target {
        return false;
    }
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
        && report
            .tool_selection_allowed_actions
            .iter()
            .filter_map(allowed_action_exact_pair)
            .any(|(action_type, target)| {
                action_type == selected_action_type && target == selected_target
            })
}

fn candidate_allowlist_live_trace_present(report: &MainChatLiveProviderEvalHarnessReport) -> bool {
    let selected_id = match report.model_selected_candidate_id.as_deref() {
        Some(id) if live_provider_contract_safe_label(id) => id,
        _ => return false,
    };
    let selected_target = match report.model_selected_candidate_target.as_deref() {
        Some(target) if live_provider_contract_safe_label(target) => target,
        _ => return false,
    };
    let selected_action_type = match report.model_selected_candidate_action_type.as_deref() {
        Some(action_type) if live_provider_contract_safe_label(action_type) => action_type,
        _ => return false,
    };

    report.tool_selection_candidate_count > 0
        && report.tool_selection_candidate_ids.len() == report.tool_selection_candidate_count
        && report.tool_selection_allowlist.len() == report.tool_selection_candidate_count
        && report.tool_selection_allowed_actions.len() == report.tool_selection_candidate_count
        && exact_candidate_allowlist_sets_present(
            report.tool_selection_candidate_count,
            &report.tool_selection_candidate_ids,
            &report.tool_selection_allowlist,
            &report.tool_selection_allowed_actions,
        )
        && allowed_action_types_match_selected(
            &report.tool_selection_allowed_actions,
            selected_action_type,
        )
        && report
            .tool_selection_candidate_ids
            .iter()
            .any(|candidate_id| candidate_id == selected_id)
        && report
            .tool_selection_allowlist
            .iter()
            .any(|target| target == selected_target)
        && report
            .tool_selection_allowed_actions
            .iter()
            .filter_map(allowed_action_exact_pair)
            .any(|(action_type, target)| {
                action_type == selected_action_type && target == selected_target
            })
}

fn allowed_action_exact_pair(action: &serde_json::Value) -> Option<(&str, &str)> {
    let object = action.as_object()?;
    if object.len() != 2 {
        return None;
    }
    let action_type = object.get("actionType")?.as_str()?;
    let target = object.get("target")?.as_str()?;
    if !live_provider_contract_safe_label(action_type) || !live_provider_contract_safe_label(target)
    {
        return None;
    }
    Some((action_type, target))
}

fn exact_candidate_allowlist_sets_present(
    candidate_count: usize,
    candidate_ids: &[String],
    allowlist: &[String],
    allowed_actions: &[serde_json::Value],
) -> bool {
    let candidate_targets = candidate_ids
        .iter()
        .filter(|candidate_id| live_provider_contract_safe_label(candidate_id))
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let allowed_targets = allowlist
        .iter()
        .filter(|target| live_provider_contract_safe_label(target))
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let action_targets = allowed_actions
        .iter()
        .filter_map(allowed_action_exact_pair)
        .map(|(_, target)| target)
        .collect::<std::collections::BTreeSet<_>>();

    candidate_targets.len() == candidate_count
        && allowed_targets.len() == candidate_count
        && action_targets.len() == candidate_count
        && candidate_targets == allowed_targets
        && candidate_targets == action_targets
}

fn allowed_action_types_match_selected(
    allowed_actions: &[serde_json::Value],
    selected_action_type: &str,
) -> bool {
    if !live_provider_contract_safe_label(selected_action_type) {
        return false;
    }
    allowed_actions.iter().all(|action| {
        matches!(
            allowed_action_exact_pair(action),
            Some((action_type, _)) if action_type == selected_action_type
        )
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
        .filter(|candidate_id| live_provider_contract_safe_label(candidate_id))
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_allowed_targets = report
        .tool_selection_allowlist
        .iter()
        .filter(|target| live_provider_contract_safe_label(target))
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let distinct_allowed_action_pairs = report
        .tool_selection_allowed_actions
        .iter()
        .filter_map(allowed_action_exact_pair)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let candidate_targets = report
        .tool_selection_candidate_ids
        .iter()
        .filter(|candidate_id| live_provider_contract_safe_label(candidate_id))
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let allowed_targets = report
        .tool_selection_allowlist
        .iter()
        .filter(|target| live_provider_contract_safe_label(target))
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let action_targets = report
        .tool_selection_allowed_actions
        .iter()
        .filter_map(allowed_action_exact_pair)
        .filter_map(|(action_type, target)| {
            if action_type == "mcp_tool" {
                Some(target)
            } else {
                None
            }
        })
        .collect::<std::collections::BTreeSet<_>>();

    report.tool_selection_candidate_count >= 2
        && report.tool_selection_candidate_ids.len() == report.tool_selection_candidate_count
        && distinct_candidate_ids == report.tool_selection_candidate_count
        && report.tool_selection_allowlist.len() == report.tool_selection_candidate_count
        && distinct_allowed_targets == report.tool_selection_candidate_count
        && report.tool_selection_allowed_actions.len() == report.tool_selection_candidate_count
        && distinct_allowed_action_pairs == report.tool_selection_candidate_count
        && candidate_targets == allowed_targets
        && candidate_targets == action_targets
}

fn registered_mcp_provider_ranked_selection_trace_present(
    report: &MainChatLiveProviderEvalHarnessReport,
) -> bool {
    if report.scenario != MainChatLiveProviderEvalHarnessScenario::RegisteredMcpAgentLoop {
        return true;
    }
    if !report.tool_selection_model_ranked || report.tool_selection_model_ranking_ignored {
        return false;
    }
    if !report.tool_selection_ranking_provider_backed {
        return false;
    }
    if report.tool_selection_ranking_route_type.as_deref() != Some("cloud") {
        return false;
    }
    if normalized_external_provider_label(&report.provider).is_none() {
        return false;
    }
    let Some(ranking_provider) = report
        .tool_selection_ranking_provider
        .as_ref()
        .filter(|provider| normalized_external_provider_label(provider).is_some())
        .map(String::as_str)
    else {
        return false;
    };
    if ranking_provider != report.provider.as_str() {
        return false;
    }
    let Some(live_model) = report
        .provider_model
        .as_ref()
        .filter(|model| live_provider_contract_safe_label(model))
        .map(String::as_str)
    else {
        return false;
    };
    let Some(ranking_model) = report
        .tool_selection_ranking_model
        .as_ref()
        .filter(|model| live_provider_contract_safe_label(model))
        .map(String::as_str)
    else {
        return false;
    };
    if ranking_model != live_model {
        return false;
    }
    let selected_candidate_id = match report.model_selected_candidate_id.as_deref() {
        Some(candidate_id) if live_provider_contract_safe_label(candidate_id) => candidate_id,
        _ => return false,
    };
    if report.model_selected_candidate_target.as_deref() != Some(selected_candidate_id) {
        return false;
    }
    if report.model_selected_candidate_action_type.as_deref() != Some("mcp_tool") {
        return false;
    }
    if report.model_selected_candidate_match_reason.as_deref() != Some("provider_model_ranked") {
        return false;
    }
    if report.tool_selection_ranking_source.as_deref() != Some("provider_model") {
        return false;
    }
    if !report
        .tool_selection_model_ranking_response_digest
        .as_ref()
        .is_some_and(|digest| metadata_safe_digest_label_present(digest))
    {
        return false;
    }
    if report.tool_selection_model_ranking_candidate_ids.len() < 2 {
        return false;
    }
    let selected_provider_rank = report
        .tool_selection_model_ranking_candidate_ids
        .iter()
        .position(|candidate_id| candidate_id == selected_candidate_id)
        .map(|index| index + 1);
    if selected_provider_rank != report.model_selected_candidate_rank {
        return false;
    }
    let ranked_candidate_id_count = report.tool_selection_model_ranking_candidate_ids.len();
    let candidate_id_count = report.tool_selection_candidate_ids.len();
    let ranked_candidate_ids = report
        .tool_selection_model_ranking_candidate_ids
        .iter()
        .filter(|candidate_id| live_provider_contract_safe_label(candidate_id))
        .map(String::as_str)
        .collect::<Vec<_>>();
    let candidate_ids = report
        .tool_selection_candidate_ids
        .iter()
        .filter(|candidate_id| live_provider_contract_safe_label(candidate_id))
        .map(String::as_str)
        .collect::<Vec<_>>();
    let ranked_candidate_set = ranked_candidate_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let candidate_set = candidate_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();

    ranked_candidate_ids.len() >= 2
        && ranked_candidate_ids.len() == ranked_candidate_id_count
        && candidate_ids.len() == candidate_id_count
        && ranked_candidate_ids.len() == candidate_ids.len()
        && ranked_candidate_set.len() == ranked_candidate_ids.len()
        && candidate_set.len() == candidate_ids.len()
        && ranked_candidate_set == candidate_set
        && ranked_candidate_ids == candidate_ids
        && ranked_candidate_ids.contains(&selected_candidate_id)
}

fn metadata_safe_digest_label_present(digest: &str) -> bool {
    if digest.chars().any(|ch| ch.is_control()) {
        return false;
    }
    let Some((bytes_label, hex_digest)) = digest.split_once(" hash:sha256:") else {
        return false;
    };
    let bytes_label_present = bytes_label
        .strip_prefix("bytes:")
        .and_then(|byte_count| {
            if byte_count.is_empty() || !byte_count.chars().all(|ch| ch.is_ascii_digit()) {
                return None;
            }
            if byte_count.len() > 1 && byte_count.starts_with('0') {
                return None;
            }
            byte_count.parse::<usize>().ok()
        })
        .is_some_and(|byte_count| byte_count > 0);
    bytes_label_present
        && hex_digest.len() == 64
        && hex_digest.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn live_provider_contract_safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAIN_CHAT_LIVE_PROVIDER_CONTRACT_SAFE_LABEL_MAX_LEN
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'))
}
